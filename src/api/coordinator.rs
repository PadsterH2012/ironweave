use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::models::coordinator::{CoordinatorMemory, WakeCoordinator};
use crate::state::AppState;

/// Get coordinator state for a project
pub async fn get_coordinator(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<CoordinatorMemory>, StatusCode> {
    let conn = state.conn()?;
    CoordinatorMemory::get_or_create(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Wake the coordinator for a project
pub async fn wake_coordinator(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(input): Json<WakeCoordinator>,
) -> Result<Json<CoordinatorMemory>, StatusCode> {
    let conn = state.conn()?;
    CoordinatorMemory::wake(&conn, &pid, &input.session_id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Put the coordinator to sleep
pub async fn sleep_coordinator(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<CoordinatorMemory>, StatusCode> {
    let conn = state.conn()?;
    CoordinatorMemory::sleep(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// List all coordinator states
pub async fn list_coordinators(
    State(state): State<AppState>,
) -> Result<Json<Vec<CoordinatorMemory>>, StatusCode> {
    let conn = state.conn()?;
    CoordinatorMemory::list(&conn)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
