use axum::{
    extract::{Path, State, Multipart},
    response::IntoResponse,
    Json, http::StatusCode,
};
use crate::state::AppState;
use crate::models::attachment::{Attachment, CreateAttachment};

pub async fn upload(
    State(state): State<AppState>,
    Path((_pid, issue_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Attachment>), StatusCode> {
    let field = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "upload".to_string());

    let mime_type = field
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let data = field
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let size_bytes = data.len() as i64;

    // Validate issue_id is a UUID to prevent path traversal
    if uuid::Uuid::parse_str(&issue_id).is_err() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file_id = uuid::Uuid::new_v4().to_string();
    let safe_filename = filename.replace(['/', '\\', '\0'], "_");
    let dir = state.data_dir.join("attachments").join(&issue_id);
    let file_path = dir.join(format!("{}_{}", &file_id[..8], safe_filename));

    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create attachment dir: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tokio::fs::write(&file_path, &data)
        .await
        .map_err(|e| {
            tracing::error!("Failed to write attachment: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let input = CreateAttachment {
        issue_id,
        filename: safe_filename,
        mime_type,
        size_bytes,
        stored_path: file_path.to_string_lossy().to_string(),
    };

    let conn = state.conn()?;
    Attachment::create(&conn, &input)
        .map(|a| (StatusCode::CREATED, Json(a)))
        .map_err(|e| {
            tracing::error!("Failed to save attachment record: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn list(
    State(state): State<AppState>,
    Path((_pid, issue_id)): Path<(String, String)>,
) -> Result<Json<Vec<Attachment>>, StatusCode> {
    let conn = state.conn()?;
    Attachment::list_by_issue(&conn, &issue_id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let attachment = {
        let conn = state.conn()?;
        Attachment::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?
    };

    let data = tokio::fs::read(&attachment.stored_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let headers = [
        (axum::http::header::CONTENT_TYPE, attachment.mime_type),
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", attachment.filename.replace('"', "_")),
        ),
    ];

    Ok((headers, data))
}
