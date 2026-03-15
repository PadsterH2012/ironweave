use axum::{extract::{Path, State}, Json, http::StatusCode};
use rusqlite;
use crate::state::AppState;
use crate::models::workflow::{
    WorkflowDefinition, CreateWorkflowDefinition,
    WorkflowInstance, CreateWorkflowInstance,
};

pub async fn create_definition(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(mut input): Json<CreateWorkflowDefinition>,
) -> Result<(StatusCode, Json<WorkflowDefinition>), StatusCode> {
    input.project_id = project_id;
    let conn = state.conn()?;
    WorkflowDefinition::create(&conn, &input)
        .map(|d| (StatusCode::CREATED, Json(d)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list_definitions(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<WorkflowDefinition>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(WorkflowDefinition::list_by_project(&conn, &project_id).unwrap_or_default()))
}

pub async fn get_definition(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<WorkflowDefinition>, StatusCode> {
    let conn = state.conn()?;
    WorkflowDefinition::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn create_instance(
    State(state): State<AppState>,
    Path(wid): Path<String>,
    Json(mut input): Json<CreateWorkflowInstance>,
) -> Result<(StatusCode, Json<WorkflowInstance>), StatusCode> {
    input.definition_id = wid.clone();
    let instance = {
        let conn = state.conn()?;
        WorkflowInstance::create(&conn, &input)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    // Notify orchestrator
    state.orchestrator.notify_instance_created(
        instance.id.clone(),
        wid,
    ).await;

    Ok((StatusCode::CREATED, Json(instance)))
}

pub async fn list_instances(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> Result<Json<Vec<WorkflowInstance>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(WorkflowInstance::list_by_definition(&conn, &wid).unwrap_or_default()))
}

pub async fn approve_gate(
    State(state): State<AppState>,
    Path((_wid, instance_id, stage_id)): Path<(String, String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let conn = state.conn().map_err(|s| (s, "database unavailable".into()))?;
    conn.execute(
        "INSERT OR REPLACE INTO workflow_gate_approvals (instance_id, stage_id, approved_at) VALUES (?1, ?2, datetime('now'))",
        rusqlite::params![instance_id, stage_id],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

pub async fn pause_instance(
    State(state): State<AppState>,
    Path((_wid, instance_id)): Path<(String, String)>,
) -> Result<Json<WorkflowInstance>, StatusCode> {
    let conn = state.conn()?;
    let instance = WorkflowInstance::get_by_id(&conn, &instance_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if instance.state != "running" {
        return Err(StatusCode::BAD_REQUEST);
    }
    WorkflowInstance::update_state(&conn, &instance_id, "paused")
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn resume_instance(
    State(state): State<AppState>,
    Path((_wid, instance_id)): Path<(String, String)>,
) -> Result<Json<WorkflowInstance>, StatusCode> {
    let conn = state.conn()?;
    let instance = WorkflowInstance::get_by_id(&conn, &instance_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if instance.state != "paused" {
        return Err(StatusCode::BAD_REQUEST);
    }
    WorkflowInstance::update_state(&conn, &instance_id, "running")
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn cancel_instance(
    State(state): State<AppState>,
    Path((_wid, instance_id)): Path<(String, String)>,
) -> Result<Json<WorkflowInstance>, StatusCode> {
    let conn = state.conn()?;
    let instance = WorkflowInstance::get_by_id(&conn, &instance_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if instance.state == "completed" || instance.state == "cancelled" {
        return Err(StatusCode::BAD_REQUEST);
    }
    WorkflowInstance::update_state(&conn, &instance_id, "cancelled")
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
