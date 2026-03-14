use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::loom::{LoomEntry, CreateLoomEntry};

#[derive(Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
}

pub async fn list_recent(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Json<Vec<LoomEntry>> {
    let conn = state.db.lock().unwrap();
    Json(LoomEntry::list_recent(&conn, params.limit).unwrap_or_default())
}

pub async fn list_by_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(params): Query<ListParams>,
) -> Json<Vec<LoomEntry>> {
    let conn = state.db.lock().unwrap();
    Json(LoomEntry::list_by_project(&conn, &project_id, params.limit).unwrap_or_default())
}

pub async fn list_by_team(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
    Query(params): Query<ListParams>,
) -> Json<Vec<LoomEntry>> {
    let conn = state.db.lock().unwrap();
    Json(LoomEntry::list_by_team(&conn, &team_id, params.limit).unwrap_or_default())
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateLoomEntry>,
) -> Json<LoomEntry> {
    let conn = state.db.lock().unwrap();
    Json(LoomEntry::create(&conn, &input).unwrap())
}
