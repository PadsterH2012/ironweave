use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::setting::{Setting, UpsertSetting};
use crate::models::project::Project;
use crate::models::dispatch_schedule::{DispatchSchedule, CreateDispatchSchedule, UpdateDispatchSchedule};

#[derive(Debug, Deserialize)]
pub struct PauseRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DispatchStatus {
    pub paused: bool,
    pub paused_at: Option<String>,
    pub reason: Option<String>,
    pub active_schedules: Vec<DispatchSchedule>,
}

pub async fn global_pause(
    State(state): State<AppState>,
    Json(input): Json<PauseRequest>,
) -> Result<Json<DispatchStatus>, StatusCode> {
    {
        let conn = state.conn()?;
        Setting::upsert(&conn, "global_dispatch_paused", &UpsertSetting {
            value: "true".to_string(), category: Some("killswitch".to_string()),
        }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Setting::upsert(&conn, "global_paused_at", &UpsertSetting {
            value: chrono::Utc::now().to_rfc3339(), category: Some("killswitch".to_string()),
        }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if let Some(reason) = &input.reason {
            Setting::upsert(&conn, "global_pause_reason", &UpsertSetting {
                value: reason.clone(), category: Some("killswitch".to_string()),
            }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    get_global_status(&state)
}

pub async fn global_resume(
    State(state): State<AppState>,
) -> Result<Json<DispatchStatus>, StatusCode> {
    {
        let conn = state.conn()?;
        Setting::upsert(&conn, "global_dispatch_paused", &UpsertSetting {
            value: "false".to_string(), category: Some("killswitch".to_string()),
        }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let _ = Setting::delete(&conn, "global_paused_at");
        let _ = Setting::delete(&conn, "global_pause_reason");
    }
    get_global_status(&state)
}

fn get_global_status(state: &AppState) -> Result<Json<DispatchStatus>, StatusCode> {
    let conn = state.conn()?;
    let paused = Setting::get_by_key(&conn, "global_dispatch_paused")
        .map(|s| s.value == "true").unwrap_or(false);
    let paused_at = Setting::get_by_key(&conn, "global_paused_at")
        .map(|s| s.value).ok();
    let reason = Setting::get_by_key(&conn, "global_pause_reason")
        .map(|s| s.value).ok();
    let active_schedules = DispatchSchedule::list(&conn)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.scope == "global")
        .collect();
    Ok(Json(DispatchStatus { paused, paused_at, reason, active_schedules }))
}

pub async fn global_status(
    State(state): State<AppState>,
) -> Result<Json<DispatchStatus>, StatusCode> {
    get_global_status(&state)
}

pub async fn project_pause(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(input): Json<PauseRequest>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.conn()?;
    Project::pause(&conn, &pid, input.reason.as_deref())
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn project_resume(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.conn()?;
    Project::resume(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

#[derive(Debug, Serialize)]
pub struct ProjectDispatchStatus {
    pub paused: bool,
    pub paused_at: Option<String>,
    pub reason: Option<String>,
    pub global_override: bool,
    pub schedules: Vec<DispatchSchedule>,
}

pub async fn project_status(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<ProjectDispatchStatus>, StatusCode> {
    let conn = state.conn()?;
    let project = Project::get_by_id(&conn, &pid).map_err(|_| StatusCode::NOT_FOUND)?;
    let global_override = Setting::get_by_key(&conn, "global_dispatch_paused")
        .map(|s| s.value == "true").unwrap_or(false);
    let schedules = DispatchSchedule::list_by_project(&conn, &pid).unwrap_or_default();
    Ok(Json(ProjectDispatchStatus {
        paused: project.is_paused,
        paused_at: project.paused_at,
        reason: project.pause_reason,
        global_override,
        schedules,
    }))
}

pub async fn list_schedules(
    State(state): State<AppState>,
) -> Result<Json<Vec<DispatchSchedule>>, StatusCode> {
    let conn = state.conn()?;
    DispatchSchedule::list(&conn)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn create_schedule(
    State(state): State<AppState>,
    Json(input): Json<CreateDispatchSchedule>,
) -> Result<(StatusCode, Json<DispatchSchedule>), StatusCode> {
    use std::str::FromStr;
    if cron::Schedule::from_str(&input.cron_expression).is_err() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref tz) = input.timezone {
        if tz.parse::<chrono_tz::Tz>().is_err() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let conn = state.conn()?;
    DispatchSchedule::create(&conn, &input)
        .map(|s| (StatusCode::CREATED, Json(s)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DispatchSchedule>, StatusCode> {
    let conn = state.conn()?;
    DispatchSchedule::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn update_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateDispatchSchedule>,
) -> Result<Json<DispatchSchedule>, StatusCode> {
    if let Some(ref expr) = input.cron_expression {
        use std::str::FromStr;
        if cron::Schedule::from_str(expr).is_err() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref tz) = input.timezone {
        if tz.parse::<chrono_tz::Tz>().is_err() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let conn = state.conn()?;
    DispatchSchedule::update(&conn, &id, &input)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    DispatchSchedule::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}
