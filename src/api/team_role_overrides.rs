use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::models::team_role_override::{TeamRoleOverride, SetTeamRoleOverride};
use crate::state::AppState;

/// List all role overrides for a team
pub async fn list_overrides(
    State(state): State<AppState>,
    Path(tid): Path<String>,
) -> Result<Json<Vec<TeamRoleOverride>>, StatusCode> {
    let conn = state.conn()?;
    TeamRoleOverride::list_by_team(&conn, &tid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Set or update a role override for a team
pub async fn set_override(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<SetTeamRoleOverride>,
) -> Result<Json<TeamRoleOverride>, StatusCode> {
    let conn = state.conn()?;
    TeamRoleOverride::set(&conn, &tid, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Clear a specific role override
pub async fn clear_override(
    State(state): State<AppState>,
    Path((tid, role)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    TeamRoleOverride::clear(&conn, &tid, &role)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
