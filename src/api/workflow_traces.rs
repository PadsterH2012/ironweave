use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use crate::models::workflow_trace::{WorkflowTrace, TraceStep, CreateTraceStep, Chokepoint, ChokepointSummary};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TraceQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct StartTrace {
    pub agent_session_id: String,
    pub issue_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteTrace {
    pub performance_log_id: Option<String>,
    pub success: bool,
}

/// List traces for a project
pub async fn list_traces(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<TraceQuery>,
) -> Result<Json<Vec<WorkflowTrace>>, StatusCode> {
    let conn = state.conn()?;
    let limit = query.limit.unwrap_or(50);
    WorkflowTrace::list_by_project(&conn, &pid, limit)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Start a new trace
pub async fn start_trace(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(input): Json<StartTrace>,
) -> Result<(StatusCode, Json<WorkflowTrace>), StatusCode> {
    let conn = state.conn()?;
    WorkflowTrace::start(&conn, &pid, &input.agent_session_id, input.issue_id.as_deref())
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get a trace with its steps
pub async fn get_trace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowTrace>, StatusCode> {
    let conn = state.conn()?;
    WorkflowTrace::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Get steps for a trace
pub async fn get_trace_steps(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<TraceStep>>, StatusCode> {
    let conn = state.conn()?;
    TraceStep::list_by_trace(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Add a step to a trace
pub async fn add_step(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CreateTraceStep>,
) -> Result<(StatusCode, Json<TraceStep>), StatusCode> {
    let conn = state.conn()?;
    WorkflowTrace::add_step(&conn, &id, &input)
        .map(|s| (StatusCode::CREATED, Json(s)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Complete a trace
pub async fn complete_trace(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CompleteTrace>,
) -> Result<Json<WorkflowTrace>, StatusCode> {
    let conn = state.conn()?;
    WorkflowTrace::complete(&conn, &id, input.performance_log_id.as_deref(), input.success)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Detect chokepoints from trace data
pub async fn detect_chokepoints(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<Vec<ChokepointSummary>>, StatusCode> {
    let conn = state.conn()?;
    Chokepoint::detect(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// List stored chokepoints
pub async fn list_chokepoints(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<Vec<Chokepoint>>, StatusCode> {
    let conn = state.conn()?;
    Chokepoint::list(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
