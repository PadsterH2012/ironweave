use axum::{extract::{Path, State}, Json, http::StatusCode};
use crate::state::AppState;
use crate::models::project::{Project, CreateProject, UpdateProject};

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), StatusCode> {
    let conn = state.conn()?;
    Project::create(&conn, &input)
        .map(|p| (StatusCode::CREATED, Json(p)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Project>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(Project::list(&conn).unwrap_or_default()))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.conn()?;
    Project::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    Project::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProject>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.conn()?;
    Project::update(&conn, &id, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
