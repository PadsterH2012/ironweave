use axum::{
    extract::{Path, State, ws::{WebSocket, WebSocketUpgrade, Message}},
    response::IntoResponse,
    Json, http::StatusCode,
};
use futures_util::{SinkExt, StreamExt};
use portable_pty::PtySize;
use serde::{Deserialize, Serialize};
use std::io::{Read as IoRead, Write as IoWrite};
use std::path::PathBuf;

use crate::runtime::adapter::AgentConfig;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SpawnRequest {
    pub runtime: String,
    pub working_directory: String,
    pub prompt: String,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub runtime: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

pub async fn spawn(
    State(state): State<AppState>,
    Json(input): Json<SpawnRequest>,
) -> Result<(StatusCode, Json<AgentInfo>), StatusCode> {
    let session_id = uuid::Uuid::new_v4().to_string();

    let config = AgentConfig {
        working_directory: PathBuf::from(&input.working_directory),
        prompt: input.prompt,
        environment: input.env,
        allowed_tools: None,
        skills: None,
        extra_args: None,
        playwright_env: None,
        model: input.model,
    };

    let size = PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };

    state
        .process_manager
        .spawn_agent(&session_id, &input.runtime, config, size)
        .await
        .map_err(|e| {
            tracing::error!("Agent spawn failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let info = AgentInfo {
        id: session_id,
        runtime: input.runtime,
        state: "running".to_string(),
        role: None,
    };

    Ok((StatusCode::CREATED, Json(info)))
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<AgentInfo>> {
    let active = state.process_manager.list_active().await;
    let conn = state.db.lock().unwrap();
    let infos: Vec<AgentInfo> = active
        .into_iter()
        .map(|(id, runtime)| {
            let role = conn.query_row(
                "SELECT tas.role FROM agent_sessions a \
                 JOIN team_agent_slots tas ON a.slot_id = tas.id \
                 WHERE a.id = ?1",
                rusqlite::params![&id],
                |row| row.get::<_, String>(0),
            ).ok();
            AgentInfo {
                id,
                runtime,
                state: "running".to_string(),
                role,
            }
        })
        .collect();
    Json(infos)
}

pub async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentInfo>, StatusCode> {
    let agent = state
        .process_manager
        .get_agent(&id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let locked = agent.lock().await;
    let role = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT tas.role FROM agent_sessions a \
             JOIN team_agent_slots tas ON a.slot_id = tas.id \
             WHERE a.id = ?1",
            rusqlite::params![&id],
            |row| row.get::<_, String>(0),
        ).ok()
    };
    let info = AgentInfo {
        id: locked.session_id.clone(),
        runtime: locked.runtime.clone(),
        state: "running".to_string(),
        role,
    };
    Ok(Json(info))
}

pub async fn stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state
        .process_manager
        .stop_agent(&id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn ws_agent_output(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_agent_ws(socket, state, session_id))
}

async fn handle_agent_ws(socket: WebSocket, state: AppState, session_id: String) {
    // 1. Get the ManagedAgent
    let agent = match state.process_manager.get_agent(&session_id).await {
        Some(a) => a,
        None => return,
    };

    // 2. Split WebSocket into sender + receiver
    let (ws_sender, ws_receiver) = socket.split();

    // 3. Get PTY reader
    let reader = {
        let locked = agent.lock().await;
        match locked.master.try_clone_reader() {
            Ok(r) => r,
            Err(_) => return,
        }
    };

    // 4. Get PTY writer (take it from ManagedAgent so nudge writes won't work via WS)
    let writer = {
        let mut locked = agent.lock().await;
        match locked.writer.take() {
            Some(w) => w,
            None => return,
        }
    };

    // 5. Create mpsc channel for PTY->WS bridge
    let (pty_tx, mut pty_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(256);

    // Task A (blocking): PTY reader -> mpsc channel
    let task_a = tokio::task::spawn_blocking(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if pty_tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Task B (async): mpsc channel -> WebSocket binary frames
    let mut ws_sender = ws_sender;
    let task_b = tokio::spawn(async move {
        while let Some(data) = pty_rx.recv().await {
            if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
        // Send exit text frame
        let exit_msg = serde_json::json!({"type": "exit", "code": 0});
        let _ = ws_sender
            .send(Message::Text(exit_msg.to_string().into()))
            .await;
    });

    // Task C (async): WebSocket -> PTY writer + control
    let agent_ref = agent.clone();
    let mut ws_receiver = ws_receiver;
    let task_c = tokio::spawn(async move {
        let mut writer = writer;
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Binary(data) => {
                    let _ = writer.write_all(&data);
                }
                Message::Text(text) => {
                    if let Ok(ctrl) = serde_json::from_str::<serde_json::Value>(&text) {
                        if ctrl.get("type").and_then(|t| t.as_str()) == Some("resize") {
                            if let (Some(cols), Some(rows)) = (
                                ctrl.get("cols").and_then(|c| c.as_u64()),
                                ctrl.get("rows").and_then(|r| r.as_u64()),
                            ) {
                                let size = PtySize {
                                    rows: rows as u16,
                                    cols: cols as u16,
                                    pixel_width: 0,
                                    pixel_height: 0,
                                };
                                let a = agent_ref.lock().await;
                                let _ = a.master.resize(size);
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for any task to finish, then clean up
    tokio::select! {
        _ = task_a => {}
        _ = task_b => {}
        _ = task_c => {}
    }

    // Do NOT remove the agent here — the orchestrator sweep handles cleanup
    // and needs to see the exit status. Removing here causes a race condition
    // where the sweep sees the agent as "disappeared" instead of "completed".
}
