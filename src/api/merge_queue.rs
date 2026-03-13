use axum::{extract::{Path, State}, Json, http::StatusCode};
use crate::state::AppState;
use crate::models::merge_queue_entry::MergeQueueEntry;

pub async fn list_queue(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Json<Vec<MergeQueueEntry>> {
    let conn = state.db.lock().unwrap();
    let entries = MergeQueueEntry::list_by_project(&conn, &pid).unwrap_or_default();
    Json(entries)
}

pub async fn approve_merge(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<MergeQueueEntry>, StatusCode> {
    let conn = state.db.lock().unwrap();
    MergeQueueEntry::update_status(&conn, &id, "pending", None, None, None)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}
