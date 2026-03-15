use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::models::quality::{QualityTier, TierRange, SetTierRange};
use crate::state::AppState;

/// List all 5 quality tiers (global reference data)
pub async fn list_tiers(
    State(state): State<AppState>,
) -> Result<Json<Vec<QualityTier>>, StatusCode> {
    let conn = state.conn()?;
    QualityTier::list(&conn)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get project tier range
pub async fn get_project_tiers(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<TierRange>, StatusCode> {
    let conn = state.conn()?;
    TierRange::for_project(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Set project tier range
pub async fn set_project_tiers(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(input): Json<SetTierRange>,
) -> Result<Json<TierRange>, StatusCode> {
    let conn = state.conn()?;
    TierRange::set_project(&conn, &pid, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Reset project tier range to defaults (1-5)
pub async fn reset_project_tiers(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<TierRange>, StatusCode> {
    let conn = state.conn()?;
    TierRange::reset_project(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get team effective tier range (inherits from project if not set)
pub async fn get_team_tiers(
    State(state): State<AppState>,
    Path(tid): Path<String>,
) -> Result<Json<TierRange>, StatusCode> {
    let conn = state.conn()?;
    TierRange::for_team(&conn, &tid)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Set team tier range override (null values = inherit from project)
pub async fn set_team_tiers(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<SetTierRange>,
) -> Result<Json<TierRange>, StatusCode> {
    let conn = state.conn()?;
    TierRange::set_team(&conn, &tid, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
