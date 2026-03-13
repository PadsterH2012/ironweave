use axum::{extract::State, Json};
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
) -> Json<Vec<serde_json::Value>> {
    Json(state.runtime_registry.list_with_capabilities())
}
