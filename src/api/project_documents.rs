use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use crate::models::project_document::{ProjectDocument, detect_removals};
use crate::state::AppState;

// --- Request/Response types ---

#[derive(Deserialize)]
pub struct UpdateDocumentRequest {
    pub content: String,
    pub updated_by: Option<String>,
}

#[derive(Serialize)]
pub struct DocumentUpdateResponse {
    pub document: ProjectDocument,
    pub removals: Vec<String>,
}

#[derive(Serialize)]
pub struct GapAnalysis {
    pub missing: Vec<String>,
    pub undocumented: Vec<String>,
}

// --- Handlers ---

/// Get (or create) a project document by type
pub async fn get_document(
    Path((pid, doc_type)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<ProjectDocument>, StatusCode> {
    let conn = state.conn()?;
    ProjectDocument::get_or_create(&conn, &pid, &doc_type)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Update a project document and detect removals
pub async fn update_document(
    Path((pid, doc_type)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(body): Json<UpdateDocumentRequest>,
) -> Result<Json<DocumentUpdateResponse>, StatusCode> {
    let conn = state.conn()?;

    // Get the old content before updating
    let old_doc = ProjectDocument::get_or_create(&conn, &pid, &doc_type)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let old_content = old_doc.content.clone();

    let updated_by = body.updated_by.as_deref().unwrap_or("user");
    let document = ProjectDocument::update_content(&conn, &pid, &doc_type, &body.content, updated_by)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let removals = detect_removals(&old_content, &body.content);

    Ok(Json(DocumentUpdateResponse { document, removals }))
}

/// Get document history (current + previous content)
pub async fn get_history(
    Path((pid, doc_type)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<ProjectDocument>, StatusCode> {
    let conn = state.conn()?;
    ProjectDocument::get_history(&conn, &pid, &doc_type)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Trigger a document scan (placeholder)
pub async fn trigger_scan(
    Path(_pid): Path<String>,
    State(_state): State<AppState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "scan_triggered"}))
}

/// Analyse gaps between intent and reality documents
pub async fn get_gaps(
    Path(pid): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<GapAnalysis>, StatusCode> {
    let conn = state.conn()?;

    let intent = ProjectDocument::get_or_create(&conn, &pid, "intent")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let reality = ProjectDocument::get_or_create(&conn, &pid, "reality")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let extract_keywords = |text: &str| -> HashSet<String> {
        text.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .filter(|w| w.len() > 2)
            .collect()
    };

    let intent_kw = extract_keywords(&intent.content);
    let reality_kw = extract_keywords(&reality.content);

    let missing: Vec<String> = intent_kw.difference(&reality_kw).cloned().collect();
    let undocumented: Vec<String> = reality_kw.difference(&intent_kw).cloned().collect();

    Ok(Json(GapAnalysis { missing, undocumented }))
}
