use axum::{extract::{Path, State}, Json, http::StatusCode};
use crate::state::AppState;
use crate::models::setting::{Setting, UpsertSetting};

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Setting>>, StatusCode> {
    let conn = state.conn()?;
    let settings = Setting::list(&conn).unwrap_or_default();
    Ok(Json(settings.into_iter().map(|s| s.redacted()).collect()))
}

pub async fn get(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Setting>, StatusCode> {
    let conn = state.conn()?;
    Setting::get_by_key(&conn, &key)
        .map(|s| Json(s.redacted()))
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn upsert(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<UpsertSetting>,
) -> Result<Json<Setting>, StatusCode> {
    if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(StatusCode::BAD_REQUEST);
    }
    if input.value.len() > 4096 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let conn = state.conn()?;
    Setting::upsert(&conn, &key, &input)
        .map(|s| Json(s.redacted()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    Setting::delete(&conn, &key)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}
