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
    let conn = state.conn()?;
    Team::create(&conn, &input)
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<Team>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(Team::list_by_project(&conn, &project_id).unwrap_or_default()))
}

pub async fn get(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.conn()?;
    Team::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
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
    let conn = state.conn()?;
    TeamAgentSlot::create(&conn, &input)
        .map(|s| (StatusCode::CREATED, Json(s)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list_slots(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<TeamAgentSlot>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(TeamAgentSlot::list_by_team(&conn, &team_id).unwrap_or_default()))
}

pub async fn update_slot(
    State(state): State<AppState>,
    Path((_tid, id)): Path<(String, String)>,
    Json(input): Json<UpdateTeamAgentSlot>,
) -> Result<Json<TeamAgentSlot>, StatusCode> {
    let conn = state.conn()?;
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
    let conn = state.conn()?;
    TeamAgentSlot::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn list_templates(
    State(state): State<AppState>,
) -> Result<Json<Vec<Team>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(Team::list_templates(&conn, None).unwrap_or_default()))
}

pub async fn list_project_templates(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<Team>>, StatusCode> {
    let conn = state.conn()?;
    Ok(Json(Team::list_templates(&conn, Some(&project_id)).unwrap_or_default()))
}

pub async fn clone_template(
    State(state): State<AppState>,
    Path((project_id, template_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Team>), StatusCode> {
    let conn = state.conn()?;
    Team::clone_into_project(&conn, &template_id, &project_id)
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn activate(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.conn()?;
    Team::set_active(&conn, &id, true)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn deactivate(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.conn()?;
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
    let conn = state.conn()?;
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
    let conn = state.conn()?;
    let team = Team::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    let slots = TeamAgentSlot::list_by_team(&conn, &id).unwrap_or_default();

    // Batch query: running agents per role (single query instead of N)
    let mut running_by_role: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT tas.role, COUNT(*) FROM agent_sessions a \
             JOIN team_agent_slots tas ON a.slot_id = tas.id \
             WHERE a.team_id = ?1 AND a.state = 'running' \
             GROUP BY tas.role"
        ).unwrap();
        let rows = stmt.query_map(params![id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }).unwrap();
        for row in rows {
            if let Ok((role, count)) = row {
                running_by_role.insert(role, count);
            }
        }
    }

    let mut role_status: Vec<serde_json::Value> = Vec::new();
    let mut seen_roles = std::collections::HashSet::new();
    for slot in &slots {
        if !seen_roles.insert(slot.role.clone()) {
            continue;
        }
        let slot_count = slots.iter().filter(|s| s.role == slot.role).count();
        let running = *running_by_role.get(&slot.role).unwrap_or(&0);
        role_status.push(serde_json::json!({
            "role": slot.role,
            "slot_count": slot_count,
            "running": running,
            "runtime": slot.runtime,
            "model": slot.model,
        }));
    }

    // Scaling recommendation — single query for running + idle counts
    let (total_running, total_idle): (i64, i64) = conn.query_row(
        "SELECT \
            COALESCE(SUM(CASE WHEN state = 'running' THEN 1 ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN state IN ('idle', 'ready') THEN 1 ELSE 0 END), 0) \
         FROM agent_sessions WHERE team_id = ?1",
        params![id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0, 0));
    let pool_depth: i64 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE project_id = ?1 AND status = 'open' AND claimed_by IS NULL AND needs_intake = 0",
        params![team.project_id],
        |row| row.get(0),
    ).unwrap_or(0);

    let max_agents = team.max_agents as usize;
    let idle = total_idle as usize;
    let active = total_running as usize;
    let total_healthy = idle + active;
    let pool = pool_depth as usize;

    let scaling = if pool > idle && total_healthy < max_agents {
        let needed = pool - idle;
        let can_spawn = max_agents - total_healthy;
        serde_json::json!({
            "action": "SpawnMore",
            "count": needed.min(can_spawn),
            "reason": format!("{} tasks waiting, {} idle agents, room for {} more", pool, idle, can_spawn)
        })
    } else if pool == 0 && idle > 1 {
        serde_json::json!({
            "action": "DrainExcess",
            "count": idle - 1,
            "reason": format!("No tasks waiting, {} idle agents — recommend draining {}", idle, idle - 1)
        })
    } else {
        serde_json::json!({
            "action": "NoChange",
            "count": 0,
            "reason": "Pool balanced"
        })
    };

    Ok(Json(serde_json::json!({
        "team_id": team.id,
        "is_active": team.is_active,
        "auto_pickup_types": team.get_auto_pickup_types(),
        "roles": role_status,
        "scaling": {
            "recommendation": scaling,
            "pool_depth": pool,
            "idle_agents": idle,
            "active_agents": active,
            "max_agents": max_agents,
        },
    })))
}
