use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use crate::models::performance_log::{PerformanceLogEntry, PerformanceQuery, ModelStats, CreatePerformanceLog};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub days: Option<i64>,
}

/// List performance log entries for a project with optional filters
pub async fn list_logs(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<PerformanceQuery>,
) -> Result<Json<Vec<PerformanceLogEntry>>, StatusCode> {
    let conn = state.conn()?;
    PerformanceLogEntry::list_by_project(&conn, &pid, &query)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get aggregated model stats for a project
pub async fn model_stats(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<Vec<ModelStats>>, StatusCode> {
    let conn = state.conn()?;
    let days = query.days.unwrap_or(30);
    PerformanceLogEntry::model_stats(&conn, &pid, days)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Manually create a performance log entry (for testing/backfill)
pub async fn create_log(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(mut input): Json<CreatePerformanceLog>,
) -> Result<(StatusCode, Json<PerformanceLogEntry>), StatusCode> {
    input.project_id = pid;
    let conn = state.conn()?;
    PerformanceLogEntry::create(&conn, &input)
        .map(|e| (StatusCode::CREATED, Json(e)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
