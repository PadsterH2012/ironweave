use axum::{extract::{Path, State}, Json, http::StatusCode};
use rusqlite::params;
use crate::state::AppState;
use crate::models::team::{Team, CreateTeam, TeamAgentSlot, CreateTeamAgentSlot, UpdateTeamAgentSlot};

pub async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(mut input): Json<CreateTeam>,
) -> Result<(StatusCode, Json<Team>), StatusCode> {
    input.project_id = project_id;
    let conn = state.db.lock().unwrap();
    Team::create(&conn, &input)
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Json<Vec<Team>> {
    let conn = state.db.lock().unwrap();
    Json(Team::list_by_project(&conn, &project_id).unwrap_or_default())
}

pub async fn get(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn create_slot(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
    Json(mut input): Json<CreateTeamAgentSlot>,
) -> Result<(StatusCode, Json<TeamAgentSlot>), StatusCode> {
    input.team_id = team_id;
    let conn = state.db.lock().unwrap();
    TeamAgentSlot::create(&conn, &input)
        .map(|s| (StatusCode::CREATED, Json(s)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list_slots(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
) -> Json<Vec<TeamAgentSlot>> {
    let conn = state.db.lock().unwrap();
    Json(TeamAgentSlot::list_by_team(&conn, &team_id).unwrap_or_default())
}

pub async fn update_slot(
    State(state): State<AppState>,
    Path((_tid, id)): Path<(String, String)>,
    Json(input): Json<UpdateTeamAgentSlot>,
) -> Result<Json<TeamAgentSlot>, StatusCode> {
    let conn = state.db.lock().unwrap();
    TeamAgentSlot::update(&conn, &id, &input)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

pub async fn delete_slot(
    State(state): State<AppState>,
    Path((_tid, id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.db.lock().unwrap();
    TeamAgentSlot::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn list_templates(
    State(state): State<AppState>,
) -> Json<Vec<Team>> {
    let conn = state.db.lock().unwrap();
    Json(Team::list_templates(&conn, None).unwrap_or_default())
}

pub async fn list_project_templates(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Json<Vec<Team>> {
    let conn = state.db.lock().unwrap();
    Json(Team::list_templates(&conn, Some(&project_id)).unwrap_or_default())
}

pub async fn clone_template(
    State(state): State<AppState>,
    Path((project_id, template_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Team>), StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::clone_into_project(&conn, &template_id, &project_id)
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn activate(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::set_active(&conn, &id, true)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn deactivate(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::set_active(&conn, &id, false)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

#[derive(serde::Deserialize)]
pub struct UpdateAutoPickup {
    pub types: Vec<String>,
}

pub async fn update_config(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
    Json(input): Json<UpdateAutoPickup>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.db.lock().unwrap();
    let type_refs: Vec<&str> = input.types.iter().map(|s| s.as_str()).collect();
    Team::update_auto_pickup_types(&conn, &id, &type_refs)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

pub async fn team_status(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn = state.db.lock().unwrap();
    let team = Team::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    let slots = TeamAgentSlot::list_by_team(&conn, &id).unwrap_or_default();

    // Count running agents per role
    let mut role_status: Vec<serde_json::Value> = Vec::new();
    let mut seen_roles = std::collections::HashSet::new();
    for slot in &slots {
        if !seen_roles.insert(slot.role.clone()) {
            continue;
        }
        let slot_count = slots.iter().filter(|s| s.role == slot.role).count();
        let running: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE team_id = ?1 AND state = 'running'
             AND slot_id IN (SELECT id FROM team_agent_slots WHERE team_id = ?1 AND role = ?2)",
            params![id, slot.role],
            |row| row.get(0),
        ).unwrap_or(0);
        role_status.push(serde_json::json!({
            "role": slot.role,
            "slot_count": slot_count,
            "running": running,
            "runtime": slot.runtime,
            "model": slot.model,
        }));
    }

    Ok(Json(serde_json::json!({
        "team_id": team.id,
        "is_active": team.is_active,
        "auto_pickup_types": team.get_auto_pickup_types(),
        "roles": role_status,
    })))
}
