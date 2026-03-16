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

// --- Gap Analysis ---

#[derive(Serialize)]
pub struct FeatureGapResult {
    pub task_title: String,
    pub status: String,       // found, partial, not_found
    pub evidence: String,     // what was found in the codebase (or why not)
}

#[derive(Serialize)]
pub struct FeatureGapAnalysis {
    pub feature_id: String,
    pub feature_title: String,
    pub found: i64,
    pub partial: i64,
    pub not_found: i64,
    pub total: i64,
    pub results: Vec<FeatureGapResult>,
}

fn scan_dir_recursive(dir: &str, keywords: &[String], found_in: &mut Vec<String>, depth: u32) {
    if depth > 5 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return; };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        // Skip hidden, node_modules, target, .git
        if name.starts_with('.') || name == "node_modules" || name == "target" { continue; }

        if path.is_dir() {
            scan_dir_recursive(&path.to_string_lossy(), keywords, found_in, depth + 1);
        } else if path.is_file() {
            let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
            if !["rs", "ts", "js", "svelte", "md", "toml", "json", "py", "sh", "yml", "yaml", "css", "html"].contains(&ext.as_str()) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                let lower = content.to_lowercase();
                for keyword in keywords {
                    if lower.contains(&keyword.to_lowercase()) {
                        let relative = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().to_string();
                        if !found_in.contains(&relative) {
                            found_in.push(relative);
                        }
                        break;
                    }
                }
            }
        }
    }
}

fn scan_project_for_keywords(project_dir: &str, keywords: &[String]) -> (String, String) {
    let mut found_in: Vec<String> = Vec::new();

    if std::fs::read_dir(project_dir).is_ok() {
        scan_dir_recursive(project_dir, keywords, &mut found_in, 0);
    }

    let match_ratio = if keywords.is_empty() { 0.0 } else {
        let matched = keywords.iter().filter(|k| {
            found_in.iter().any(|f| f.to_lowercase().contains(&k.to_lowercase()))
        }).count();
        matched as f64 / keywords.len() as f64
    };

    if match_ratio > 0.5 {
        ("found".into(), format!("Keywords found in: {}", found_in.join(", ")))
    } else if match_ratio > 0.0 {
        ("partial".into(), format!("Some keywords found in: {}", found_in.join(", ")))
    } else {
        ("not_found".into(), "No matching keywords found in codebase".into())
    }
}

/// Analyze gaps between feature tasks and codebase
pub async fn analyze_gaps(
    Path((pid, fid)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<FeatureGapAnalysis>, StatusCode> {
    let (feature, tasks, project_dir) = {
        let conn = state.conn()?;
        let feature = Feature::get_by_id(&conn, &fid).map_err(|_| StatusCode::NOT_FOUND)?;
        let tasks = FeatureTask::list_by_feature(&conn, &fid).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let project = crate::models::project::Project::get_by_id(&conn, &pid).map_err(|_| StatusCode::NOT_FOUND)?;
        (feature, tasks, project.directory)
    };

    let mut results = Vec::new();
    let mut found = 0i64;
    let mut partial = 0i64;
    let mut not_found = 0i64;

    for task in &tasks {
        let keywords = crate::models::knowledge_pattern::extract_keywords(&task.title);
        let (status, evidence) = scan_project_for_keywords(&project_dir, &keywords);
        match status.as_str() {
            "found" => found += 1,
            "partial" => partial += 1,
            _ => not_found += 1,
        }
        results.push(FeatureGapResult {
            task_title: task.title.clone(),
            status,
            evidence,
        });
    }

    Ok(Json(FeatureGapAnalysis {
        feature_id: fid,
        feature_title: feature.title,
        found,
        partial,
        not_found,
        total: tasks.len() as i64,
        results,
    }))
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
