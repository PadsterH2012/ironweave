use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::models::feature::{Feature, CreateFeature, UpdateFeature, FeatureSummary};
use crate::models::feature_task::{FeatureTask, CreateFeatureTask, UpdateFeatureTask};
use crate::models::issue::{Issue, CreateIssue};
use crate::state::AppState;

// --- Request/Response types ---

#[derive(Deserialize)]
pub struct FeatureListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct FeatureWithTasks {
    #[serde(flatten)]
    pub feature: Feature,
    pub tasks: Vec<FeatureTask>,
}

#[derive(Deserialize)]
pub struct ParkRequest {
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct ImportPrdRequest {
    pub text: String,
    pub default_role: Option<String>,
}

// --- Feature handlers ---

/// List features for a project
pub async fn list_features(
    Path(pid): Path<String>,
    Query(params): Query<FeatureListQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<Feature>>, StatusCode> {
    let conn = state.conn()?;
    let limit = params.limit.unwrap_or(100);
    Feature::list_by_project(&conn, &pid, params.status.as_deref(), limit)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Get a single feature with its tasks
pub async fn get_feature(
    Path((_pid, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<FeatureWithTasks>, StatusCode> {
    let conn = state.conn()?;
    let feature = Feature::get_by_id(&conn, &id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let tasks = FeatureTask::list_by_feature(&conn, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(FeatureWithTasks { feature, tasks }))
}

/// Create a feature
pub async fn create_feature(
    Path(pid): Path<String>,
    State(state): State<AppState>,
    Json(mut input): Json<CreateFeature>,
) -> Result<(StatusCode, Json<Feature>), StatusCode> {
    input.project_id = pid;
    let conn = state.conn()?;
    Feature::create(&conn, &input)
        .map(|f| (StatusCode::CREATED, Json(f)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Update a feature
pub async fn update_feature(
    Path((_pid, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(input): Json<UpdateFeature>,
) -> Result<Json<Feature>, StatusCode> {
    let conn = state.conn()?;
    Feature::update(&conn, &id, &input)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Delete (soft) a feature
pub async fn delete_feature(
    Path((_pid, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<Feature>, StatusCode> {
    let conn = state.conn()?;
    Feature::delete(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Park a feature
pub async fn park_feature(
    Path((_pid, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(body): Json<ParkRequest>,
) -> Result<Json<Feature>, StatusCode> {
    let conn = state.conn()?;
    Feature::park(&conn, &id, body.reason.as_deref())
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Verify a feature
pub async fn verify_feature(
    Path((_pid, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<Feature>, StatusCode> {
    let conn = state.conn()?;
    Feature::verify(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Import a PRD: create a feature and extract tasks from bullet/numbered lines
pub async fn import_prd(
    Path(pid): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<ImportPrdRequest>,
) -> Result<(StatusCode, Json<FeatureWithTasks>), StatusCode> {
    let conn = state.conn()?;

    // Extract task lines from the text
    let task_lines: Vec<String> = body.text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") {
                Some(trimmed.trim_start_matches("- ").to_string())
            } else if trimmed.len() > 2 {
                // Check for numbered lines like "1. " or "12. "
                if let Some(dot_pos) = trimmed.find(". ") {
                    let prefix = &trimmed[..dot_pos];
                    if prefix.chars().all(|c| c.is_ascii_digit()) {
                        return Some(trimmed[dot_pos + 2..].to_string());
                    }
                }
                None
            } else {
                None
            }
        })
        .collect();

    // Create the feature with the full text as prd_content
    let first_line = body.text.lines().next().unwrap_or("Imported PRD");
    let create_input = CreateFeature {
        project_id: pid,
        title: first_line.to_string(),
        description: None,
        status: Some("idea".to_string()),
        prd_content: Some(body.text.clone()),
        priority: Some(5),
        keywords: None,
    };
    let feature = Feature::create(&conn, &create_input)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create tasks from extracted lines
    let mut tasks = Vec::new();
    for (i, title) in task_lines.into_iter().enumerate() {
        let task_input = CreateFeatureTask {
            feature_id: feature.id.clone(),
            title,
            sort_order: Some(i as i64),
        };
        let task = FeatureTask::create(&conn, &task_input)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        tasks.push(task);
    }

    Ok((StatusCode::CREATED, Json(FeatureWithTasks { feature, tasks })))
}

/// Get feature summary across all projects
pub async fn feature_summary(
    State(state): State<AppState>,
) -> Result<Json<Vec<FeatureSummary>>, StatusCode> {
    let conn = state.conn()?;
    Feature::summary(&conn)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// --- Feature Task handlers ---

/// List tasks for a feature
pub async fn list_tasks(
    Path(fid): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<FeatureTask>>, StatusCode> {
    let conn = state.conn()?;
    FeatureTask::list_by_feature(&conn, &fid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Create a task for a feature
pub async fn create_task(
    Path(fid): Path<String>,
    State(state): State<AppState>,
    Json(mut input): Json<CreateFeatureTask>,
) -> Result<(StatusCode, Json<FeatureTask>), StatusCode> {
    input.feature_id = fid;
    let conn = state.conn()?;
    FeatureTask::create(&conn, &input)
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Update a feature task
pub async fn update_task(
    Path((_fid, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(input): Json<UpdateFeatureTask>,
) -> Result<Json<FeatureTask>, StatusCode> {
    let conn = state.conn()?;
    FeatureTask::update(&conn, &id, &input)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Delete a feature task
pub async fn delete_task(
    Path((_fid, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    FeatureTask::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Implement a task: create an issue from the task and link them
pub async fn implement_task(
    Path((_fid, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let conn = state.conn()?;

    let task = FeatureTask::get_by_id(&conn, &id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let feature = Feature::get_by_id(&conn, &task.feature_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let issue = Issue::create(&conn, &CreateIssue {
        project_id: feature.project_id.clone(),
        title: task.title.clone(),
        description: Some(format!("From feature: {}\n\n{}", feature.title, feature.description)),
        issue_type: Some("task".into()),
        priority: Some(feature.priority),
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
        parent_id: None,
        needs_intake: Some(0),
        scope_mode: None,
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let updated_task = FeatureTask::implement(&conn, &id, &issue.id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({
        "task": updated_task,
        "issue": issue,
    }))))
}
