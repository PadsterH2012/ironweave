use axum::{extract::{Path, Query, State}, Json};
use axum::http::StatusCode;
use serde::Deserialize;
use rusqlite;
use crate::state::AppState;
use crate::models::loom::{LoomEntry, CreateLoomEntry};

#[derive(Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
}

pub async fn list_recent(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<LoomEntry>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(LoomEntry::list_recent(&conn, params.limit).unwrap_or_default()))
}

pub async fn list_by_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<LoomEntry>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(LoomEntry::list_by_project(&conn, &project_id, params.limit).unwrap_or_default()))
}

pub async fn list_by_team(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<LoomEntry>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(LoomEntry::list_by_team(&conn, &team_id, params.limit).unwrap_or_default()))
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateLoomEntry>,
) -> Result<Json<LoomEntry>, StatusCode> {
    let conn = state.conn()?;
    LoomEntry::create(&conn, &input)
        .map(|e| Json(e))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn pending_questions(
    Path(pid): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<LoomEntry>>, StatusCode> {
    let conn = state.conn()?;
    let mut stmt = conn.prepare(
        "SELECT l.*, tas.role, a.runtime, a.model FROM loom_entries l \
         LEFT JOIN agent_sessions a ON l.agent_id = a.id \
         LEFT JOIN team_agent_slots tas ON a.slot_id = tas.id \
         WHERE l.project_id = ?1 AND l.entry_type = 'question' \
         ORDER BY l.timestamp DESC LIMIT 20"
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let entries = stmt.query_map(rusqlite::params![pid], LoomEntry::from_row)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(Json(entries))
}

#[derive(Deserialize)]
pub struct AnswerRequest {
    pub question_id: String,
    pub content: String,
    pub team_id: String,
    pub project_id: String,
}

pub async fn post_answer(
    State(state): State<AppState>,
    Json(input): Json<AnswerRequest>,
) -> Result<(StatusCode, Json<LoomEntry>), StatusCode> {
    let conn = state.conn()?;
    let content = format!("[Re: {}] {}", input.question_id, input.content);
    let entry = LoomEntry::create(&conn, &CreateLoomEntry {
        agent_id: None,
        team_id: input.team_id,
        project_id: input.project_id,
        workflow_instance_id: None,
        entry_type: "answer".to_string(),
        content,
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(entry)))
}
