use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use crate::models::routing_override::{RoutingOverride, CreateRoutingOverride};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct OverrideQuery {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DetectQuery {
    pub min_observations: Option<i64>,
    pub days: Option<i64>,
}

/// List routing overrides for a project
pub async fn list_overrides(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<OverrideQuery>,
) -> Result<Json<Vec<RoutingOverride>>, StatusCode> {
    let conn = state.conn()?;
    RoutingOverride::list_by_project(&conn, &pid, query.status.as_deref())
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Create a routing override manually
pub async fn create_override(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(mut input): Json<CreateRoutingOverride>,
) -> Result<(StatusCode, Json<RoutingOverride>), StatusCode> {
    input.project_id = pid;
    let conn = state.conn()?;
    RoutingOverride::create(&conn, &input)
        .map(|o| (StatusCode::CREATED, Json(o)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Accept a suggested override
pub async fn accept_override(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RoutingOverride>, StatusCode> {
    let conn = state.conn()?;
    RoutingOverride::accept(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Reject a suggested override
pub async fn reject_override(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RoutingOverride>, StatusCode> {
    let conn = state.conn()?;
    RoutingOverride::reject(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Trigger pattern detection — analyse performance log and generate suggestions
pub async fn detect_patterns(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<DetectQuery>,
) -> Result<Json<Vec<RoutingOverride>>, StatusCode> {
    let conn = state.conn()?;
    let min_obs = query.min_observations.unwrap_or(10);
    let days = query.days.unwrap_or(30);
    RoutingOverride::detect_patterns(&conn, &pid, min_obs, days)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
