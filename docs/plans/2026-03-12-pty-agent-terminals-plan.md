# PTY-Based Agent Terminals Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace piped-stdout agent spawning with PTY-based bidirectional terminals so users can spawn AI agents from the web UI and interact with them through xterm.js.

**Architecture:** Agents are spawned inside pseudo-terminals using `portable-pty`. The PTY master fd is read/written by a WebSocket handler that bridges to xterm.js in the browser. Binary WS frames carry raw PTY bytes (zero encoding overhead), text WS frames carry JSON control messages (resize, exit). Agents are ephemeral — spawned for a task, torn down after.

**Tech Stack:** Rust (`portable-pty`, `axum` WebSocket, `tokio`), Svelte 5 (`@xterm/xterm` 6.x, `@xterm/addon-fit`)

**Design doc:** `docs/plans/2026-03-12-pty-agent-terminals-design.md`

---

## Task 1: Add `portable-pty` dependency

**Files:**
- Modify: `Cargo.toml:6-30`

**Step 1: Add the dependency**

Add to `[dependencies]` in `Cargo.toml`:

```toml
portable-pty = "0.8"
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors.

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat: add portable-pty dependency for PTY-based agent terminals"
```

---

## Task 2: Rewrite RuntimeAdapter trait for PTY spawning

**Files:**
- Modify: `src/runtime/adapter.rs` (full rewrite)

**Step 1: Rewrite adapter.rs**

Replace the entire file. The trait's `spawn` method now returns a `SpawnedPty` instead of `SpawnedAgent`. Remove `AgentOutput`, `OutputStream`, `SpawnedAgent` — they're no longer needed. Keep `AgentConfig` and `PlaywrightEnv`.

```rust
use async_trait::async_trait;
use portable_pty::{CommandBuilder, MasterPty, PtySize, Child as PtyChild};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaywrightEnv {
    pub browsers_path: String,
    pub skip_download: bool,
}

impl Default for PlaywrightEnv {
    fn default() -> Self {
        Self {
            browsers_path: "/home/paddy/ironweave/browsers".to_string(),
            skip_download: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub working_directory: PathBuf,
    pub prompt: String,
    pub allowed_tools: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub extra_args: Option<Vec<String>>,
    pub playwright_env: Option<PlaywrightEnv>,
}

impl AgentConfig {
    pub fn merged_env(&self) -> HashMap<String, String> {
        let mut env = self.environment.clone().unwrap_or_default();
        if let Some(ref pw) = self.playwright_env {
            env.insert(
                "PLAYWRIGHT_BROWSERS_PATH".to_string(),
                pw.browsers_path.clone(),
            );
            if pw.skip_download {
                env.insert(
                    "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD".to_string(),
                    "1".to_string(),
                );
            }
        }
        env
    }
}

pub struct SpawnedPty {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn PtyChild + Send + Sync>,
}

#[async_trait]
pub trait RuntimeAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn binary(&self) -> &str;
    async fn check_available(&self) -> bool;
    fn build_command(&self, config: &AgentConfig) -> CommandBuilder;
    fn spawn_pty(
        &self,
        config: &AgentConfig,
        size: PtySize,
    ) -> crate::error::Result<SpawnedPty>;
}
```

Note: `spawn_pty` is synchronous (not async) because `portable-pty` operations are synchronous. It will be called from `tokio::task::spawn_blocking` by the process manager.

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Errors in adapter consumers (claude.rs, opencode.rs, gemini.rs, process manager) — that's expected, we'll fix them next.

**Step 3: Commit**

```bash
git add src/runtime/adapter.rs
git commit -m "feat: rewrite RuntimeAdapter trait for PTY-based spawning"
```

---

## Task 3: Rewrite Claude, OpenCode, and Gemini adapters for PTY

**Files:**
- Modify: `src/runtime/claude.rs` (full rewrite)
- Modify: `src/runtime/opencode.rs` (full rewrite)
- Modify: `src/runtime/gemini.rs` (full rewrite)

**Step 1: Rewrite claude.rs**

```rust
use async_trait::async_trait;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::process::Command;

use super::adapter::*;

pub struct ClaudeAdapter;

#[async_trait]
impl RuntimeAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "Claude Code"
    }

    fn binary(&self) -> &str {
        "claude"
    }

    async fn check_available(&self) -> bool {
        Command::new(self.binary())
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn build_command(&self, config: &AgentConfig) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(self.binary());
        cmd.arg("--dangerously-skip-permissions");

        if let Some(ref tools) = config.allowed_tools {
            cmd.arg("--allowedTools");
            cmd.arg(tools.join(" "));
        }

        cmd.arg(config.prompt.clone());
        cmd.cwd(&config.working_directory);

        for (k, v) in config.merged_env() {
            cmd.env(k, v);
        }

        cmd
    }

    fn spawn_pty(
        &self,
        config: &AgentConfig,
        size: PtySize,
    ) -> crate::error::Result<SpawnedPty> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(size)
            .map_err(|e| crate::error::IronweaveError::Internal(format!("PTY open failed: {}", e)))?;

        let cmd = self.build_command(config);
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| crate::error::IronweaveError::Internal(format!("PTY spawn failed: {}", e)))?;

        // Drop the slave side — the child process owns it now
        drop(pair.slave);

        Ok(SpawnedPty {
            master: pair.master,
            child,
        })
    }
}
```

**Step 2: Rewrite opencode.rs**

Same pattern as claude.rs but with `build_command` returning:

```rust
fn build_command(&self, config: &AgentConfig) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(self.binary());
    cmd.arg("--non-interactive");
    cmd.arg(config.prompt.clone());
    cmd.cwd(&config.working_directory);
    for (k, v) in config.merged_env() {
        cmd.env(k, v);
    }
    cmd
}
```

The `spawn_pty` implementation is identical to ClaudeAdapter — extract into a helper if desired, but keeping it duplicated is fine for 3 adapters.

**Step 3: Rewrite gemini.rs**

Same pattern, `build_command` returns:

```rust
fn build_command(&self, config: &AgentConfig) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(self.binary());
    cmd.arg(config.prompt.clone());
    cmd.cwd(&config.working_directory);
    for (k, v) in config.merged_env() {
        cmd.env(k, v);
    }
    cmd
}
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: Errors in process manager only — adapters should compile.

**Step 5: Commit**

```bash
git add src/runtime/claude.rs src/runtime/opencode.rs src/runtime/gemini.rs
git commit -m "feat: rewrite all runtime adapters for PTY spawning"
```

---

## Task 4: Rewrite ProcessManager for PTY lifecycle

**Files:**
- Modify: `src/process/manager.rs` (full rewrite)

**Step 1: Rewrite manager.rs**

The process manager stores PTY masters and child handles per session. It no longer uses `broadcast::channel` — WebSocket handlers read/write the PTY master directly.

```rust
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use portable_pty::{MasterPty, PtySize, Child as PtyChild};

use crate::runtime::adapter::AgentConfig;
use crate::runtime::RuntimeRegistry;

pub struct ManagedAgent {
    pub session_id: String,
    pub runtime: String,
    pub config: AgentConfig,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn PtyChild + Send + Sync>,
}

pub struct ProcessManager {
    registry: Arc<RuntimeRegistry>,
    agents: Arc<RwLock<HashMap<String, Arc<tokio::sync::Mutex<ManagedAgent>>>>>,
}

impl ProcessManager {
    pub fn new(registry: Arc<RuntimeRegistry>) -> Self {
        Self {
            registry,
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn spawn_agent(
        &self,
        session_id: &str,
        runtime_name: &str,
        config: AgentConfig,
        size: PtySize,
    ) -> crate::error::Result<()> {
        let adapter = self.registry.get(runtime_name).ok_or_else(|| {
            crate::error::IronweaveError::NotFound(format!("runtime adapter: {}", runtime_name))
        })?;

        // spawn_pty is synchronous — run in blocking thread
        let spawned = tokio::task::spawn_blocking({
            let adapter = adapter.clone();
            let config = config.clone();
            move || adapter.spawn_pty(&config, size)
        })
        .await
        .map_err(|e| crate::error::IronweaveError::Internal(format!("spawn task failed: {}", e)))??;

        let managed = ManagedAgent {
            session_id: session_id.to_string(),
            runtime: runtime_name.to_string(),
            config,
            master: spawned.master,
            child: spawned.child,
        };

        self.agents
            .write()
            .await
            .insert(session_id.to_string(), Arc::new(tokio::sync::Mutex::new(managed)));

        Ok(())
    }

    pub async fn get_agent(
        &self,
        session_id: &str,
    ) -> Option<Arc<tokio::sync::Mutex<ManagedAgent>>> {
        self.agents.read().await.get(session_id).cloned()
    }

    pub async fn stop_agent(&self, session_id: &str) -> crate::error::Result<()> {
        let agent = self.agents.write().await.remove(session_id);
        match agent {
            Some(managed) => {
                let mut agent = managed.lock().await;
                agent.child.kill().ok();
                Ok(())
            }
            None => Err(crate::error::IronweaveError::NotFound(format!(
                "agent session: {}",
                session_id
            ))),
        }
    }

    pub async fn list_active(&self) -> Vec<(String, String)> {
        let agents = self.agents.read().await;
        let mut result = Vec::new();
        for (id, agent) in agents.iter() {
            let a = agent.lock().await;
            result.push((id.clone(), a.runtime.clone()));
        }
        result
    }

    pub async fn remove_agent(&self, session_id: &str) {
        self.agents.write().await.remove(session_id);
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Errors in `src/api/agents.rs` and `src/main.rs` — those call the old API.

**Step 3: Commit**

```bash
git add src/process/manager.rs
git commit -m "feat: rewrite ProcessManager for PTY lifecycle with child kill support"
```

---

## Task 5: Rewrite agent API routes

**Files:**
- Modify: `src/api/agents.rs` (full rewrite)

**Step 1: Rewrite agents.rs**

New routes: `POST /api/agents/spawn` (spawn), `GET /api/agents` (list), `GET /api/agents/{id}` (get info), `POST /api/agents/{id}/stop` (stop), `GET /ws/agents/{id}` (bidirectional PTY WebSocket).

```rust
use axum::{
    extract::{Path, State, ws::{WebSocket, WebSocketUpgrade, Message}},
    response::IntoResponse,
    Json, http::StatusCode,
};
use portable_pty::PtySize;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use crate::state::AppState;
use crate::runtime::adapter::AgentConfig;

#[derive(Debug, Deserialize)]
pub struct SpawnRequest {
    pub runtime: String,
    pub working_directory: String,
    pub prompt: String,
    pub env: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub runtime: String,
    pub state: String,
}

pub async fn spawn(
    State(state): State<AppState>,
    Json(input): Json<SpawnRequest>,
) -> Result<Json<AgentInfo>, StatusCode> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let config = AgentConfig {
        working_directory: input.working_directory.into(),
        prompt: input.prompt,
        allowed_tools: None,
        skills: None,
        environment: input.env,
        extra_args: None,
        playwright_env: None,
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
            tracing::error!("Failed to spawn agent: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(AgentInfo {
        id: session_id,
        runtime: input.runtime,
        state: "running".to_string(),
    }))
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<AgentInfo>> {
    let agents = state.process_manager.list_active().await;
    Json(
        agents
            .into_iter()
            .map(|(id, runtime)| AgentInfo {
                id,
                runtime,
                state: "running".to_string(),
            })
            .collect(),
    )
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

    let a = agent.lock().await;
    Ok(Json(AgentInfo {
        id: a.session_id.clone(),
        runtime: a.runtime.clone(),
        state: "running".to_string(),
    }))
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
    let agent = match state.process_manager.get_agent(&session_id).await {
        Some(a) => a,
        None => return,
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Get reader and writer from PTY master
    let reader = {
        let mut agent_lock = agent.lock().await;
        match agent_lock.master.try_clone_reader() {
            Ok(r) => r,
            Err(_) => return,
        }
    };

    let writer = {
        let mut agent_lock = agent.lock().await;
        match agent_lock.master.take_writer() {
            Ok(w) => w,
            Err(_) => return,
        }
    };

    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};

    // PTY → WebSocket (binary frames)
    let pty_to_ws = tokio::task::spawn_blocking({
        let session_id = session_id.clone();
        move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        // We need to send this to the WS sender — use a channel
                        // This is handled via the shared channel pattern below
                    }
                    Err(_) => break,
                }
            }
        }
    });

    // Actually, the PTY read is blocking so we need a channel bridge.
    // Better pattern: spawn_blocking reads PTY into a tokio mpsc channel,
    // then an async task forwards from channel to WebSocket.

    // This will be the actual implementation — see full code in step.

    // Clean up when done
    state.process_manager.remove_agent(&session_id).await;
}
```

**Important:** The WebSocket handler needs a channel bridge between the blocking PTY reader and the async WebSocket sender. The full implementation pattern is:

1. `spawn_blocking` thread: reads PTY master in a loop → sends bytes into `tokio::sync::mpsc::Sender<Vec<u8>>`
2. Async task 1: receives from mpsc → sends as binary WS frames via `ws_sender`
3. Async task 2: receives WS frames from `ws_receiver` → binary frames written to PTY master writer (via another `spawn_blocking` or direct write), text frames parsed as JSON control messages

```rust
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

async fn handle_agent_ws(socket: WebSocket, state: AppState, session_id: String) {
    let agent = match state.process_manager.get_agent(&session_id).await {
        Some(a) => a,
        None => return,
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Get reader and writer from PTY master
    let (reader, writer) = {
        let mut agent_lock = agent.lock().await;
        let reader = match agent_lock.master.try_clone_reader() {
            Ok(r) => r,
            Err(_) => return,
        };
        let writer = match agent_lock.master.take_writer() {
            Ok(w) => w,
            Err(_) => return,
        };
        (reader, writer)
    };

    let (pty_tx, mut pty_rx) = mpsc::channel::<Vec<u8>>(256);

    // Task 1: Blocking PTY reader → mpsc channel
    let read_handle = tokio::task::spawn_blocking(move || {
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

    // Task 2: mpsc channel → WebSocket binary frames
    let ws_send_handle = tokio::spawn(async move {
        while let Some(data) = pty_rx.recv().await {
            if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
        // Send exit notification
        let exit_msg = serde_json::json!({"type": "exit", "code": 0});
        let _ = ws_sender
            .send(Message::Text(exit_msg.to_string().into()))
            .await;
    });

    // Task 3: WebSocket → PTY writer (input + control)
    let agent_ref = agent.clone();
    let ws_recv_handle = tokio::spawn(async move {
        let mut writer = writer;
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Binary(data) => {
                    // User keyboard input → PTY
                    if writer.write_all(&data).is_err() {
                        break;
                    }
                }
                Message::Text(text) => {
                    // Control message (resize, etc.)
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
                                let mut a = agent_ref.lock().await;
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

    // Wait for any task to finish (PTY exit or WS close)
    tokio::select! {
        _ = read_handle => {},
        _ = ws_send_handle => {},
        _ = ws_recv_handle => {},
    }

    // Clean up
    state.process_manager.remove_agent(&session_id).await;
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: May need `futures-util` dependency. If so, add `futures-util = "0.3"` to `Cargo.toml`.

**Step 3: Commit**

```bash
git add src/api/agents.rs Cargo.toml
git commit -m "feat: rewrite agent API with PTY WebSocket handler and bidirectional streaming"
```

---

## Task 6: Update routes in main.rs

**Files:**
- Modify: `src/main.rs:112-116`

**Step 1: Update agent routes to match new API**

Replace lines 112-116:

```rust
        // Agents
        .route("/api/agents", get(api::agents::list))
        .route("/api/agents/spawn", post(api::agents::spawn))
        .route("/api/agents/{id}", get(api::agents::get_agent))
        .route("/api/agents/{id}/stop", post(api::agents::stop))
        // Agent WebSocket
        .route("/ws/agents/{id}", get(api::agents::ws_agent_output))
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles successfully.

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: update agent routes — spawn, get, stop, WebSocket"
```

---

## Task 7: Update frontend API client

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Update agent types and API calls**

Replace the `AgentSession` interface with `AgentInfo`:

```typescript
export interface AgentInfo {
  id: string;
  runtime: string;
  state: string;
}

export interface SpawnAgentRequest {
  runtime: string;
  working_directory: string;
  prompt: string;
  env?: Record<string, string>;
}
```

Replace the `agents` API object:

```typescript
export const agents = {
  list: () => get<AgentInfo[]>('/agents'),
  get: (id: string) => get<AgentInfo>(`/agents/${id}`),
  spawn: (data: SpawnAgentRequest) => post<AgentInfo>('/agents/spawn', data),
  stop: (id: string) => post<void>(`/agents/${id}/stop`, {}),
};
```

Remove the old `AgentSession`, `SpawnAgent` interfaces.

Update `connectAgentWs` — no changes needed, it already returns a raw WebSocket.

**Step 2: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Build errors in Agents.svelte (uses old types) — that's expected, we fix it next.

**Step 3: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: update frontend API client for new agent types and routes"
```

---

## Task 8: Update Terminal.svelte for bidirectional PTY

**Files:**
- Modify: `frontend/src/lib/components/Terminal.svelte`

**Step 1: Rewrite Terminal.svelte**

The terminal now sends keyboard input and handles binary/text frame distinction.

```svelte
<script lang="ts">
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import '@xterm/xterm/css/xterm.css';
  import { connectAgentWs } from '../api';

  interface Props {
    agentId: string;
  }

  let { agentId }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!containerEl) return;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: 'block',
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      theme: {
        background: 'transparent',
        foreground: '#e4e4e7',
        cursor: '#22c55e',
        selectionBackground: '#3f3f4640',
        black: '#18181b',
        red: '#ef4444',
        green: '#22c55e',
        yellow: '#eab308',
        blue: '#3b82f6',
        magenta: '#a855f7',
        cyan: '#06b6d4',
        white: '#e4e4e7',
      },
      allowTransparency: true,
      scrollback: 5000,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerEl);

    requestAnimationFrame(() => {
      try { fitAddon.fit(); } catch {}
    });

    const resizeObserver = new ResizeObserver(() => {
      try { fitAddon.fit(); } catch {}
    });
    resizeObserver.observe(containerEl);

    // Connect WebSocket
    const ws = connectAgentWs(agentId, () => {});
    ws.binaryType = 'arraybuffer';

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        // Binary frame: raw PTY output
        term.write(new Uint8Array(event.data));
      } else {
        // Text frame: control message
        try {
          const ctrl = JSON.parse(event.data);
          if (ctrl.type === 'exit') {
            term.write(`\r\n\x1b[33m[Process exited with code ${ctrl.code ?? '?'}]\x1b[0m\r\n`);
          } else if (ctrl.type === 'error') {
            term.write(`\r\n\x1b[31m[Error: ${ctrl.message ?? 'unknown'}]\x1b[0m\r\n`);
          }
        } catch {}
      }
    };

    ws.onerror = () => {
      term.write('\r\n\x1b[31m[WebSocket error]\x1b[0m\r\n');
    };

    ws.onclose = () => {
      term.write('\r\n\x1b[33m[Connection closed]\x1b[0m\r\n');
    };

    // Send keyboard input as binary frames
    term.onData((data: string) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(new TextEncoder().encode(data));
      }
    });

    // Send resize events as text frames
    term.onResize(({ cols, rows }) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'resize', cols, rows }));
      }
    });

    // Trigger initial resize report
    fitAddon.fit();

    return () => {
      resizeObserver.disconnect();
      if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
        ws.close();
      }
      term.dispose();
    };
  });
</script>

<div
  bind:this={containerEl}
  class="terminal-container w-full h-full min-h-0"
></div>

<style>
  .terminal-container :global(.xterm) {
    height: 100%;
    padding: 4px;
  }
  .terminal-container :global(.xterm-viewport) {
    background-color: transparent !important;
  }
</style>
```

Note on keyboard input: `term.onData` fires with the string the user typed. We encode it to bytes and send as a binary WebSocket frame. On the server side, this gets written to the PTY master, which delivers it to the agent's stdin.

Note on resize: `term.onResize` fires when xterm's dimensions change (from `fitAddon.fit()`). We send a JSON text frame to the server, which calls `master.resize()`.

**Step 2: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: May still error on Agents.svelte — that's next.

**Step 3: Commit**

```bash
git add frontend/src/lib/components/Terminal.svelte
git commit -m "feat: bidirectional Terminal.svelte with binary PTY streaming and resize"
```

---

## Task 9: Update Agents.svelte for new API

**Files:**
- Modify: `frontend/src/routes/Agents.svelte`

**Step 1: Update to use new types and routes**

Key changes:
- Import `AgentInfo` and `SpawnAgentRequest` instead of `AgentSession` and `SpawnAgent`
- `fetchAgents` calls `agents.list()` which now returns `AgentInfo[]` directly (no more fetching IDs then getting each)
- `handleSpawn` sends `SpawnAgentRequest` (runtime, working_directory, prompt) — server generates ID
- Remove `crypto.randomUUID()` — server generates session ID now

Replace the `<script>` section:

```svelte
<script lang="ts">
  import { agents, projects, type AgentInfo, type SpawnAgentRequest, type Project } from '../lib/api';
  import Terminal from '../lib/components/Terminal.svelte';

  let agentList: AgentInfo[] = $state([]);
  let projectList: Project[] = $state([]);
  let error: string | null = $state(null);
  let expandedId: string | null = $state(null);
  let showSpawnForm: boolean = $state(false);

  // Spawn form fields
  let spawnRuntime: string = $state('claude');
  let spawnPrompt: string = $state('');
  let spawnDir: string = $state('/home/paddy');
  let spawnProjectId: string = $state('');
  let spawning: boolean = $state(false);

  async function fetchAgents() {
    try {
      agentList = await agents.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch agents';
    }
  }

  async function fetchProjects() {
    try {
      projectList = await projects.list();
    } catch {}
  }

  $effect(() => {
    fetchAgents();
    fetchProjects();
    const interval = setInterval(fetchAgents, 5000);
    return () => clearInterval(interval);
  });

  // Auto-fill working directory from selected project
  $effect(() => {
    if (spawnProjectId) {
      const proj = projectList.find(p => p.id === spawnProjectId);
      if (proj) spawnDir = proj.directory;
    }
  });

  function truncateId(id: string): string {
    return id.length > 8 ? id.slice(0, 8) : id;
  }

  function runtimeColor(runtime: string): string {
    switch (runtime.toLowerCase()) {
      case 'claude': return 'bg-purple-600 text-purple-100';
      case 'opencode': return 'bg-blue-600 text-blue-100';
      case 'gemini': return 'bg-green-600 text-green-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function stateColor(state: string): string {
    switch (state.toLowerCase()) {
      case 'running': return 'bg-green-400 animate-pulse';
      case 'exited': return 'bg-gray-400';
      case 'crashed': return 'bg-red-500';
      default: return 'bg-gray-400';
    }
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  async function handleSpawn() {
    if (!spawnPrompt.trim()) return;
    spawning = true;
    try {
      const data: SpawnAgentRequest = {
        runtime: spawnRuntime,
        working_directory: spawnDir,
        prompt: spawnPrompt,
      };
      await agents.spawn(data);
      showSpawnForm = false;
      spawnPrompt = '';
      await fetchAgents();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to spawn agent';
    } finally {
      spawning = false;
    }
  }

  async function handleStop(e: MouseEvent, id: string) {
    e.stopPropagation();
    try {
      await agents.stop(id);
      await fetchAgents();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to stop agent';
    }
  }
</script>
```

Update the spawn form template to include an optional project selector:

```svelte
  <!-- Spawn form -->
  {#if showSpawnForm}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <h2 class="text-lg font-semibold text-white">Spawn New Agent</h2>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <label for="spawn-runtime" class="block text-sm font-medium text-gray-400 mb-1">Runtime</label>
          <select
            id="spawn-runtime"
            bind:value={spawnRuntime}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          >
            <option value="claude">Claude Code</option>
            <option value="opencode">OpenCode</option>
            <option value="gemini">Gemini CLI</option>
          </select>
        </div>

        <div>
          <label for="spawn-project" class="block text-sm font-medium text-gray-400 mb-1">Project (optional)</label>
          <select
            id="spawn-project"
            bind:value={spawnProjectId}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          >
            <option value="">None — manual directory</option>
            {#each projectList as proj (proj.id)}
              <option value={proj.id}>{proj.name}</option>
            {/each}
          </select>
        </div>

        <div class="md:col-span-2">
          <label for="spawn-dir" class="block text-sm font-medium text-gray-400 mb-1">Working Directory</label>
          <input
            id="spawn-dir"
            type="text"
            bind:value={spawnDir}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500"
          />
        </div>
      </div>

      <div>
        <label for="spawn-prompt" class="block text-sm font-medium text-gray-400 mb-1">Prompt</label>
        <textarea
          id="spawn-prompt"
          bind:value={spawnPrompt}
          rows="3"
          placeholder="Enter the task prompt for the agent..."
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500 resize-none"
        ></textarea>
      </div>

      <div class="flex justify-end">
        <button
          onclick={handleSpawn}
          disabled={spawning || !spawnPrompt.trim()}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
        >
          {spawning ? 'Spawning...' : 'Spawn'}
        </button>
      </div>
    </div>
  {/if}
```

Update the agent card list to use `agentList` and `AgentInfo` fields:

```svelte
  {#if agentList.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      No active agent sessions. Click "Spawn Agent" to start one.
    </div>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {#each agentList as agent (agent.id)}
        <div
          class="rounded-xl bg-gray-900 border border-gray-800 overflow-hidden flex flex-col transition-all duration-200"
          class:col-span-full={expandedId === agent.id}
          class:md:col-span-full={expandedId === agent.id}
          class:lg:col-span-full={expandedId === agent.id}
        >
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            onclick={() => toggleExpand(agent.id)}
            class="flex items-center justify-between px-4 py-2.5 bg-gray-800/60 border-b border-gray-800 hover:bg-gray-800 transition-colors w-full text-left cursor-pointer"
            role="button"
            tabindex="0"
          >
            <div class="flex items-center gap-3 min-w-0">
              <span class="inline-block h-2.5 w-2.5 rounded-full shrink-0 {stateColor(agent.state)}"></span>
              <span class="font-mono text-sm text-gray-200 truncate" title={agent.id}>
                {truncateId(agent.id)}
              </span>
              <span class="text-xs font-medium px-2 py-0.5 rounded-full shrink-0 {runtimeColor(agent.runtime)}">
                {agent.runtime}
              </span>
            </div>
            <div class="flex items-center gap-2 shrink-0 ml-2">
              <span class="text-xs text-gray-500 capitalize">{agent.state}</span>
              <button
                onclick={(e: MouseEvent) => { e.stopPropagation(); handleStop(e, agent.id); }}
                class="text-xs px-2 py-1 rounded bg-red-900/40 hover:bg-red-800/60 text-red-400 hover:text-red-300 transition-colors"
                title="Stop agent"
              >
                Stop
              </button>
            </div>
          </div>

          <div
            class="bg-gray-950"
            style="height: {expandedId === agent.id ? '600px' : '300px'}"
          >
            <Terminal agentId={agent.id} />
          </div>
        </div>
      {/each}
    </div>
  {/if}
```

**Step 2: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Clean build.

**Step 3: Commit**

```bash
git add frontend/src/routes/Agents.svelte
git commit -m "feat: update Agents page for new spawn API with optional project selector"
```

---

## Task 10: Build, deploy, and verify end-to-end

**Files:**
- No new files

**Step 1: Build frontend**

```bash
cd frontend && npm run build && cd ..
```

**Step 2: Run cargo tests**

```bash
cargo test
```

Fix any test failures — the existing `agent.rs` model tests may need updating if the DB schema changes, but since we're not changing the migration (just making team_id/slot_id usage optional in the API layer), they should still pass.

**Step 3: Build release binary**

Deploy to hl-ironweave following the established pattern:

```bash
# On Mac: rsync source to server
rsync -az --exclude target --exclude node_modules --exclude .git . paddy@10.202.28.205:/home/paddy/ironweave/src/

# On server: build
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave/src && rm -rf target/release/.fingerprint/ironweave-* target/release/deps/ironweave-* target/release/ironweave && cargo build --release'

# On server: deploy (atomic swap)
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave && cp src/target/release/ironweave target/release/ironweave.new && mv -f target/release/ironweave.new target/release/ironweave && sudo systemctl restart ironweave'
```

**Step 4: Verify with Playwright**

1. Navigate to Agents page
2. Click "Spawn Agent"
3. Select Claude Code runtime
4. Enter a simple prompt (e.g. "echo hello world")
5. Set working directory to `/home/paddy/ironweave`
6. Click Spawn
7. Verify terminal shows Claude Code output with ANSI formatting
8. Verify typing in terminal sends input to agent
9. Verify resize works (expand the agent card)

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: post-deployment adjustments for PTY agent terminals"
```

---

## Summary of all gaps addressed

| Gap | Fix | Task |
|-----|-----|------|
| GAP 1: Two disconnected systems | Simplified — API returns `AgentInfo` from ProcessManager directly. DB audit log can be added later. | Tasks 5, 6 |
| GAP 2: Route mismatches | Routes aligned: `/agents/spawn`, `/agents/{id}`, `/agents/{id}/stop` | Tasks 6, 7 |
| GAP 3: No PID/kill | `ManagedAgent` stores `child` handle, `stop_agent()` calls `child.kill()` | Task 4 |
| GAP 4: Terminal JSON parsing | Binary frames for PTY output, text frames for control. xterm writes raw bytes. | Task 8 |
| GAP 5: `--print` no streaming | Replaced with PTY — full interactive terminal | Tasks 2, 3 |
| GAP 6: Config deserialization | Simplified `SpawnRequest` with only needed fields | Task 5 |
