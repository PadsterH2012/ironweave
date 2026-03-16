use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use crate::models::knowledge_pattern::{
    KnowledgePattern, CreateKnowledgePattern, UpdateKnowledgePattern, KnowledgeSearchQuery, KnowledgeSearchResult,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub pattern_type: Option<String>,
    pub role: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CrossProjectQuery {
    pub query: Option<String>,
    pub role: Option<String>,
    pub task_type: Option<String>,
    pub pattern_type: Option<String>,
    pub limit: Option<i64>,
}

/// List knowledge patterns for a project
pub async fn list_patterns(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<KnowledgePattern>>, StatusCode> {
    let conn = state.conn()?;
    let limit = query.limit.unwrap_or(100);
    KnowledgePattern::list_by_project(&conn, &pid, query.pattern_type.as_deref(), query.role.as_deref(), limit)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get a single knowledge pattern
pub async fn get_pattern(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<KnowledgePattern>, StatusCode> {
    let conn = state.conn()?;
    KnowledgePattern::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Create a knowledge pattern
pub async fn create_pattern(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(mut input): Json<CreateKnowledgePattern>,
) -> Result<(StatusCode, Json<KnowledgePattern>), StatusCode> {
    input.project_id = pid;
    let conn = state.conn()?;
    KnowledgePattern::create(&conn, &input)
        .map(|p| (StatusCode::CREATED, Json(p)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Search knowledge patterns within a project
pub async fn search_patterns(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(query): Json<KnowledgeSearchQuery>,
) -> Result<Json<Vec<KnowledgeSearchResult>>, StatusCode> {
    let conn = state.conn()?;
    KnowledgePattern::search(&conn, &pid, &query)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Search shared knowledge patterns across all projects
pub async fn cross_project_search(
    State(state): State<AppState>,
    Query(params): Query<CrossProjectQuery>,
) -> Result<Json<Vec<KnowledgeSearchResult>>, StatusCode> {
    let query = KnowledgeSearchQuery {
        query: params.query.unwrap_or_default(),
        role: params.role,
        task_type: params.task_type,
        pattern_type: params.pattern_type,
        files: None,
        limit: params.limit,
    };
    let conn = state.conn()?;
    KnowledgePattern::search_cross_project(&conn, &query)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Update a knowledge pattern
pub async fn update_pattern(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
    Json(input): Json<UpdateKnowledgePattern>,
) -> Result<Json<KnowledgePattern>, StatusCode> {
    let conn = state.conn()?;
    KnowledgePattern::update(&conn, &id, &input)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Delete a knowledge pattern
pub async fn delete_pattern(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    KnowledgePattern::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Trigger knowledge extraction (placeholder)
pub async fn trigger_extraction(
    State(_state): State<AppState>,
    Path(_pid): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"extracted": 0}))
}
