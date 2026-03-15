use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::Serialize;

use crate::app_runner::detect::detect_app;
use crate::models::project::Project;
use crate::models::project_app::ProjectApp;
use crate::state::AppState;

#[derive(Serialize)]
pub struct AppStatus {
    pub id: Option<String>,
    pub state: String,
    pub port: Option<i32>,
    pub url: Option<String>,
    pub run_command: Option<String>,
    pub last_error: Option<String>,
    pub started_at: Option<String>,
}

impl AppStatus {
    fn from_app(app: &ProjectApp) -> Self {
        let url = app.port.map(|p| format!("http://10.202.28.205:{}", p));
        Self {
            id: Some(app.id.clone()),
            state: app.state.clone(),
            port: app.port,
            url,
            run_command: Some(app.run_command.clone()),
            last_error: app.last_error.clone(),
            started_at: app.started_at.clone(),
        }
    }

    fn stopped() -> Self {
        Self {
            id: None,
            state: "stopped".into(),
            port: None,
            url: None,
            run_command: None,
            last_error: None,
            started_at: None,
        }
    }
}

pub async fn start(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AppStatus>, StatusCode> {
    // Get project from DB
    let project = {
        let conn = state.conn()?;
        Project::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?
    };

    // Detect app in project directory
    let detected = detect_app(std::path::Path::new(&project.directory))
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Build run_command string for storage
    let run_command = format!("{} {}", detected.command, detected.args.join(" "));

    // Upsert the project app record
    let app = {
        let conn = state.conn()?;
        ProjectApp::upsert(&conn, &id, &run_command)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    // Check if already running
    if app.state == "running" && state.app_runner.check_running(&app.id).await {
        return Ok(Json(AppStatus::from_app(&app)));
    }

    // Start the app
    match state.app_runner.start_app(&app.id, &project.directory, &detected).await {
        Ok((port, pid)) => {
            let conn = state.conn()?;
            ProjectApp::update_state(&conn, &app.id, "running", Some(pid as i64), Some(port), None)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let updated = ProjectApp::get_by_project(&conn, &id)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(AppStatus::from_app(&updated)))
        }
        Err(e) => {
            let conn = state.conn()?;
            let _ = ProjectApp::update_state(&conn, &app.id, "error", None, None, Some(&e));
            let updated = ProjectApp::get_by_project(&conn, &id)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(AppStatus::from_app(&updated)))
        }
    }
}

pub async fn stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let app = {
        let conn = state.conn()?;
        ProjectApp::get_by_project(&conn, &id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?
    };

    state.app_runner.stop_app(&app.id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let conn = state.conn()?;
    ProjectApp::update_state(&conn, &app.id, "stopped", None, None, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AppStatus>, StatusCode> {
    let app = {
        let conn = state.conn()?;
        ProjectApp::get_by_project(&conn, &id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    match app {
        Some(app) => {
            if app.state == "running" && !state.app_runner.check_running(&app.id).await {
                // Process died — update to stopped
                let conn = state.conn()?;
                let _ = ProjectApp::update_state(&conn, &app.id, "stopped", None, None, None);
                let updated = ProjectApp::get_by_project(&conn, &id)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
                Ok(Json(AppStatus::from_app(&updated)))
            } else {
                Ok(Json(AppStatus::from_app(&app)))
            }
        }
        None => Ok(Json(AppStatus::stopped())),
    }
}
