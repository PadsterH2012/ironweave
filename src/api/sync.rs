use axum::{extract::{Path, Query, State}, Json, http::StatusCode};
use serde::Deserialize;
use crate::state::AppState;
use crate::sync::manager::{SyncSnapshot, SyncStatus};
use crate::api::filesystem::BrowseEntry;
use crate::error::IronweaveError;

fn map_err(e: IronweaveError) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RestoreRequest {
    pub change_id: String,
}

pub async fn trigger_sync(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SyncStatus>, (StatusCode, String)> {
    let sm = state.sync_manager.clone()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;
    let mm = state.mount_manager.clone();
    let db = state.db.clone();
    let project_id = id.clone();

    tokio::task::spawn_blocking(move || {
        if let Some(ref mm) = mm {
            let conn = db.lock().unwrap();
            if let Ok(project) = crate::models::project::Project::get_by_id(&conn, &project_id) {
                drop(conn);
                if let Some(ref mount_id) = project.mount_id {
                    mm.ensure_mounted(mount_id)?;
                }
            }
        }
        sm.sync_project(&project_id)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("task join error: {}", e)))?
    .map(Json)
    .map_err(map_err)
}

pub async fn get_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SyncStatus>, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;
    sm.get_status(&id)
        .map(Json)
        .map_err(map_err)
}

pub async fn get_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<SyncSnapshot>>, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;
    sm.get_history(&id, 50)
        .map(Json)
        .map_err(map_err)
}

pub async fn get_diff(
    State(state): State<AppState>,
    Path((id, change_id)): Path<(String, String)>,
) -> Result<String, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;
    sm.get_diff(&id, &change_id)
        .map_err(map_err)
}

pub async fn restore(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<RestoreRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;
    sm.restore(&id, &input.change_id)
        .map(|_| StatusCode::OK)
        .map_err(map_err)
}

pub async fn browse_files(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<Vec<BrowseEntry>>, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;
    let path = query.path.unwrap_or_default();
    sm.browse_files(&id, &path)
        .map(Json)
        .map_err(map_err)
}

pub async fn read_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<String, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;
    let path = query.path.unwrap_or_default();
    sm.read_file(&id, &path)
        .map_err(map_err)
}
