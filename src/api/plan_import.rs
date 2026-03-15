use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::issue::{Issue, CreateIssue};
use crate::models::project::Project;
use crate::orchestrator::plan_parser;

#[derive(Debug, Deserialize)]
pub struct ImportPlanRequest {
    pub plan_path: String,
    pub default_role: Option<String>,
    pub default_priority: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ImportedIssue {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportPlanResponse {
    pub imported: usize,
    pub issues: Vec<ImportedIssue>,
}

pub async fn import_plan(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(input): Json<ImportPlanRequest>,
) -> Result<(StatusCode, Json<ImportPlanResponse>), (StatusCode, String)> {
    if input.plan_path.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "plan_path is required".to_string()));
    }
    let default_role = input.default_role.unwrap_or_else(|| "senior_coder".to_string());
    let default_priority = input.default_priority.unwrap_or(5);

    // Get project directory
    let project_dir = {
        let conn = state.conn().map_err(|s| (s, "database unavailable".into()))?;
        let project = Project::get_by_id(&conn, &project_id)
            .map_err(|_| (StatusCode::NOT_FOUND, "Project not found".to_string()))?;
        project.directory
    };

    // Validate and read plan file (prevent path traversal)
    let plan_file = std::path::Path::new(&project_dir).join(&input.plan_path);
    let canonical = plan_file.canonicalize()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Cannot resolve plan file: {}", e)))?;
    let canonical_dir = std::path::Path::new(&project_dir).canonicalize()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Project directory not found".to_string()))?;
    if !canonical.starts_with(&canonical_dir) {
        return Err((StatusCode::BAD_REQUEST, "Plan path must be within project directory".to_string()));
    }
    let content = std::fs::read_to_string(&canonical)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Cannot read plan file: {}", e)))?;

    // Parse tasks
    let parsed = plan_parser::parse_plan(&content);
    if parsed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No tasks found in plan file".to_string()));
    }

    // Create issues atomically, mapping task numbers -> UUIDs for dependency wiring
    let mut task_to_uuid: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let mut imported_issues = Vec::new();

    let conn = state.conn().map_err(|s| (s, "database unavailable".into()))?;
    conn.execute_batch("BEGIN").map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Transaction start failed: {}", e)))?;

    for task in &parsed {
        // Resolve dependencies: map task numbers to already-created UUIDs
        let depends_on: Vec<String> = task
            .depends_on_task_numbers
            .iter()
            .filter_map(|n| task_to_uuid.get(n).cloned())
            .collect();

        let role = task.role.clone().unwrap_or_else(|| default_role.clone());

        let create_input = CreateIssue {
            project_id: project_id.clone(),
            issue_type: Some("task".to_string()),
            title: task.title.clone(),
            description: Some(task.description.clone()),
            priority: Some(default_priority),
            depends_on: if depends_on.is_empty() { None } else { Some(depends_on.clone()) },
            workflow_instance_id: None,
            stage_id: None,
            role: Some(role),
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: Some("auto".to_string()),
        };

        let issue = match Issue::create(&conn, &create_input) {
            Ok(i) => i,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create issue: {}", e)));
            }
        };

        task_to_uuid.insert(task.task_number, issue.id.clone());

        imported_issues.push(ImportedIssue {
            id: issue.id,
            title: task.title.clone(),
            depends_on,
        });
    }

    conn.execute_batch("COMMIT").map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Transaction commit failed: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(ImportPlanResponse {
            imported: imported_issues.len(),
            issues: imported_issues,
        }),
    ))
}
