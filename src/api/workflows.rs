use axum::{extract::{Path, State}, Json, http::StatusCode};
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
    let conn = state.db.lock().unwrap();
    WorkflowDefinition::create(&conn, &input)
        .map(|d| (StatusCode::CREATED, Json(d)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list_definitions(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Json<Vec<WorkflowDefinition>> {
    let conn = state.db.lock().unwrap();
    Json(WorkflowDefinition::list_by_project(&conn, &project_id).unwrap_or_default())
}

pub async fn get_definition(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<WorkflowDefinition>, StatusCode> {
    let conn = state.db.lock().unwrap();
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
        let conn = state.db.lock().unwrap();
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
) -> Json<Vec<WorkflowInstance>> {
    let conn = state.db.lock().unwrap();
    Json(WorkflowInstance::list_by_definition(&conn, &wid).unwrap_or_default())
}
