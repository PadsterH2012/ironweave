use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::prompt_template::{
    PromptTemplate, CreatePromptTemplate, UpdatePromptTemplate,
    PromptTemplateAssignment, CreateAssignment,
};

#[derive(Deserialize)]
pub struct ListParams {
    pub project_id: Option<String>,
}

pub async fn list_templates(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<PromptTemplate>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(PromptTemplate::list(&conn, params.project_id.as_deref()).unwrap_or_default()))
}

pub async fn get_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PromptTemplate>, StatusCode> {
    let conn = state.conn()?;
    PromptTemplate::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn create_template(
    State(state): State<AppState>,
    Json(input): Json<CreatePromptTemplate>,
) -> Result<Json<PromptTemplate>, StatusCode> {
    let conn = state.conn()?;
    PromptTemplate::create(&conn, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn update_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdatePromptTemplate>,
) -> Result<Json<PromptTemplate>, StatusCode> {
    let conn = state.conn()?;
    PromptTemplate::update(&conn, &id, &input)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::Validation(_) => StatusCode::FORBIDDEN,
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

pub async fn delete_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    PromptTemplate::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| match e {
            crate::error::IronweaveError::Validation(_) => StatusCode::FORBIDDEN,
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

// --- Assignments ---

pub async fn list_assignments(
    State(state): State<AppState>,
    Path(role): Path<String>,
) -> Result<Json<Vec<PromptTemplateAssignment>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(PromptTemplateAssignment::list_by_role(&conn, &role).unwrap_or_default()))
}

pub async fn create_assignment(
    State(state): State<AppState>,
    Json(input): Json<CreateAssignment>,
) -> Result<Json<PromptTemplateAssignment>, StatusCode> {
    let conn = state.conn()?;
    PromptTemplateAssignment::create(&conn, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn delete_assignment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    PromptTemplateAssignment::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn build_prompt(
    State(state): State<AppState>,
    Path(role): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn = state.conn()?;
    let prompt = PromptTemplate::build_prompt_for_role(&conn, &role, params.project_id.as_deref())
        .unwrap_or_default();
    Ok(Json(serde_json::json!({ "role": role, "prompt": prompt })))
}
