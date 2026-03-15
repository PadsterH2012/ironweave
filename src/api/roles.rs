use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::models::role::{Role, CreateRole, UpdateRole};
use crate::state::AppState;

pub async fn list_roles(
    State(state): State<AppState>,
) -> Result<Json<Vec<Role>>, StatusCode> {
    let conn = state.conn()?;
    Role::list(&conn)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_role(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Role>, StatusCode> {
    let conn = state.conn()?;
    Role::get_by_name(&conn, &name)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn create_role(
    State(state): State<AppState>,
    Json(input): Json<CreateRole>,
) -> Result<(StatusCode, Json<Role>), StatusCode> {
    let conn = state.conn()?;
    Role::create(&conn, &input)
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn update_role(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(input): Json<UpdateRole>,
) -> Result<Json<Role>, StatusCode> {
    let conn = state.conn()?;
    Role::update(&conn, &name, &input)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete_role(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    Role::delete(&conn, &name)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
