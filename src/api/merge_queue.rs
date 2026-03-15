use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::Serialize;
use crate::state::AppState;
use crate::models::merge_queue_entry::MergeQueueEntry;
use crate::models::project::Project;

#[derive(Serialize)]
pub struct MergeQueueDiff {
    pub branch: String,
    pub target: String,
    pub diff: String,
    pub conflict_files: Vec<String>,
}

pub async fn list_queue(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<Vec<MergeQueueEntry>>, StatusCode> {
    let conn = state.conn()?;
    let entries = MergeQueueEntry::list_by_project(&conn, &pid).unwrap_or_default();
    Ok(Json(entries))
}

pub async fn approve_merge(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<MergeQueueEntry>, StatusCode> {
    let conn = state.conn()?;
    MergeQueueEntry::update_status(&conn, &id, "pending", None, None, None)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

/// Manually trigger a resolver agent for a conflicted merge queue entry.
/// Returns 409 if already resolving, 404 if not found, 400 if not in conflicted/escalated state.
pub async fn resolve(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<MergeQueueEntry>, StatusCode> {
    let entry = {
        let conn = state.conn()?;
        MergeQueueEntry::get_by_id(&conn, &id)
            .map_err(|e| match e {
                crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            })?
    };

    // Only allow resolve on conflicted or escalated entries
    match entry.status.as_str() {
        "resolving" => return Err(StatusCode::CONFLICT),
        "conflicted" | "escalated" => {}
        _ => return Err(StatusCode::BAD_REQUEST),
    }

    // Set status to resolving — the orchestrator sweep will pick up
    // entries in "resolving" state without a resolver_agent_id and spawn an agent
    let conn = state.conn()?;
    MergeQueueEntry::update_status(&conn, &id, "resolving", None, None, None)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

/// Get the diff for a merge queue entry's branch vs the project's main branch.
pub async fn get_diff(
    State(state): State<AppState>,
    Path((pid, id)): Path<(String, String)>,
) -> Result<Json<MergeQueueDiff>, StatusCode> {
    let (entry, project) = {
        let conn = state.conn()?;
        let entry = MergeQueueEntry::get_by_id(&conn, &id)
            .map_err(|e| match e {
                crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            })?;
        let project = Project::get_by_id(&conn, &pid)
            .map_err(|e| match e {
                crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            })?;
        (entry, project)
    };

    let target = "main".to_string();
    let diff = std::process::Command::new("git")
        .args(["diff", &format!("{}...{}", target, entry.branch_name)])
        .current_dir(&project.directory)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let conflict_files: Vec<String> = serde_json::from_str(&entry.conflict_files)
        .unwrap_or_default();

    Ok(Json(MergeQueueDiff {
        branch: entry.branch_name,
        target,
        diff,
        conflict_files,
    }))
}

/// Reject a merge queue entry (human review action for escalated entries).
pub async fn reject(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<MergeQueueEntry>, StatusCode> {
    let conn = state.conn()?;
    MergeQueueEntry::update_status(&conn, &id, "rejected", None, None, Some("Rejected by human reviewer"))
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}
