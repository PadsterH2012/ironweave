use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use crate::models::code_graph::{GraphNode, GraphEdge, CreateGraphNode, CreateGraphEdge, FileComplexity, BlastRadius};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct NodeQuery {
    pub node_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BlastRadiusQuery {
    pub path: String,
}

/// List graph nodes for a project
pub async fn list_nodes(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<NodeQuery>,
) -> Result<Json<Vec<GraphNode>>, StatusCode> {
    let conn = state.conn()?;
    GraphNode::list_by_project(&conn, &pid, query.node_type.as_deref())
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Add or update a graph node
pub async fn upsert_node(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(mut input): Json<CreateGraphNode>,
) -> Result<Json<GraphNode>, StatusCode> {
    input.project_id = pid;
    let conn = state.conn()?;
    GraphNode::upsert(&conn, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Add or update a graph edge
pub async fn upsert_edge(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(input): Json<CreateGraphEdge>,
) -> Result<Json<GraphEdge>, StatusCode> {
    let conn = state.conn()?;
    GraphEdge::upsert(&conn, &pid, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get file complexity rankings
pub async fn file_complexity(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<Vec<FileComplexity>>, StatusCode> {
    let conn = state.conn()?;
    GraphNode::file_complexity(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get blast radius for a file
pub async fn blast_radius(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<BlastRadiusQuery>,
) -> Result<Json<BlastRadius>, StatusCode> {
    let conn = state.conn()?;
    GraphNode::blast_radius(&conn, &pid, &query.path)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Recompute complexity scores from graph connectivity
pub async fn recompute_complexity(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    GraphNode::recompute_complexity(&conn, &pid)
        .map(|_| StatusCode::OK)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Clear all graph data for a project (for re-indexing)
pub async fn clear_graph(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    GraphNode::clear_project(&conn, &pid)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
