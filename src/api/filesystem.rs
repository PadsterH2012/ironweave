use axum::{extract::{Query, State}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: String,
    pub include_files: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BrowseEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
}

#[derive(Debug, Serialize)]
pub struct BrowseResponse {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<BrowseEntry>,
}

pub async fn browse(
    State(state): State<AppState>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<BrowseResponse>, StatusCode> {
    let requested = PathBuf::from(&query.path);
    let include_files = query.include_files.unwrap_or(false);

    let canonical = requested.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;

    // Security: path must be under allowed browse_roots
    let browse_roots = state.browse_roots();
    let allowed = browse_roots.iter().any(|root| {
        let root_path = std::path::Path::new(root);
        if let Ok(canonical_root) = root_path.canonicalize() {
            canonical.starts_with(&canonical_root)
        } else {
            false
        }
    });

    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&canonical).map_err(|_| StatusCode::NOT_FOUND)?;

    for entry in read_dir.flatten() {
        let file_type = entry.file_type().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        }

        if file_type.is_dir() {
            entries.push(BrowseEntry { name, entry_type: "directory".to_string() });
        } else if include_files && file_type.is_file() {
            entries.push(BrowseEntry { name, entry_type: "file".to_string() });
        }
    }

    entries.sort_by(|a, b| {
        let type_cmp = a.entry_type.cmp(&b.entry_type);
        if type_cmp == std::cmp::Ordering::Equal {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else {
            type_cmp
        }
    });

    let parent = canonical.parent().map(|p| p.to_string_lossy().to_string());

    Ok(Json(BrowseResponse {
        path: canonical.to_string_lossy().to_string(),
        parent,
        entries,
    }))
}
