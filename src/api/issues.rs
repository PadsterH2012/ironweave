use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::issue::{Issue, CreateIssue, UpdateIssue};

pub async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(mut input): Json<CreateIssue>,
) -> Result<(StatusCode, Json<Issue>), StatusCode> {
    input.project_id = project_id;
    let conn = state.db.lock().unwrap();
    Issue::create(&conn, &input)
        .map(|i| (StatusCode::CREATED, Json(i)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list(
    State(state): State<AppState>,
    Path(_project_id): Path<String>,
) -> Json<Vec<Issue>> {
    let conn = state.db.lock().unwrap();
    // Issue::list returns all issues; filter by project in memory
    let all = Issue::list(&conn).unwrap_or_default();
    let filtered: Vec<Issue> = all.into_iter().filter(|i| i.project_id == _project_id).collect();
    Json(filtered)
}

pub async fn get(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Issue>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn update(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
    Json(input): Json<UpdateIssue>,
) -> Result<Json<Issue>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::update(&conn, &id, &input)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

#[derive(Debug, Deserialize)]
pub struct ClaimBody {
    pub agent_session_id: String,
}

pub async fn claim(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
    Json(body): Json<ClaimBody>,
) -> Result<Json<Issue>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::claim(&conn, &id, &body.agent_session_id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn unclaim(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Issue>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::unclaim(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn children(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Vec<Issue>>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::get_children(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn ready(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Json<Vec<Issue>> {
    let conn = state.db.lock().unwrap();
    Json(Issue::get_ready(&conn, &project_id).unwrap_or_default())
}
