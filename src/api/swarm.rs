use axum::{extract::{Path, State}, Json, http::StatusCode};
use rusqlite::params;
use serde::Serialize;
use crate::state::AppState;
use crate::models::team::Team;

#[derive(Serialize)]
pub struct SwarmAgent {
    pub session_id: String,
    pub role: String,
    pub runtime: String,
    pub state: String,
    pub issue_id: Option<String>,
    pub issue_title: Option<String>,
}

#[derive(Serialize)]
pub struct SwarmStatus {
    pub coordination_mode: String,
    pub active_agents: i64,
    pub idle_agents: i64,
    pub total_agents: i64,
    pub task_pool_depth: i64,
    pub throughput_issues_per_hour: f64,
    pub scaling_recommendation: String,
    pub agents: Vec<SwarmAgent>,
}

pub async fn get_status(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<SwarmStatus>, StatusCode> {
    let conn = state.conn()?;

    // Get primary team for this project
    let teams = Team::list_by_project(&conn, &pid).unwrap_or_default();
    let team = teams.into_iter().find(|t| t.is_active).or_else(|| {
        Team::list_by_project(&conn, &pid).unwrap_or_default().into_iter().next()
    });

    let coordination_mode = team.as_ref()
        .map(|t| t.coordination_mode.clone())
        .unwrap_or_else(|| "none".to_string());
    let max_agents = team.as_ref().map(|t| t.max_agents).unwrap_or(0) as i64;

    // Count agents by state across all teams for this project
    let active_agents: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agent_sessions AS a
         JOIN teams AS t ON a.team_id = t.id
         WHERE t.project_id = ?1 AND a.state = 'running'",
        params![pid],
        |row| row.get(0),
    ).unwrap_or(0);

    let idle_agents: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agent_sessions AS a
         JOIN teams AS t ON a.team_id = t.id
         WHERE t.project_id = ?1 AND a.state IN ('idle', 'ready')",
        params![pid],
        |row| row.get(0),
    ).unwrap_or(0);

    let total_agents = active_agents + idle_agents;

    // Task pool: open unclaimed issues
    let task_pool_depth: i64 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE project_id = ?1 AND status = 'open' AND claimed_by IS NULL AND needs_intake = 0",
        params![pid],
        |row| row.get(0),
    ).unwrap_or(0);

    // Throughput: issues closed in the last hour
    let throughput: f64 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE project_id = ?1 AND status = 'closed'
         AND updated_at >= datetime('now', '-1 hour')",
        params![pid],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) as f64;

    // Scaling recommendation
    let scaling_recommendation = if task_pool_depth > idle_agents && total_agents < max_agents {
        "scale_up".to_string()
    } else if task_pool_depth == 0 && idle_agents > 1 {
        "scale_down".to_string()
    } else {
        "no_change".to_string()
    };

    // Agent details
    let mut stmt = conn.prepare(
        "SELECT a.id, s.role, a.runtime, a.state,
                i.id, i.title
         FROM agent_sessions AS a
         JOIN teams AS t ON a.team_id = t.id
         JOIN team_agent_slots AS s ON a.slot_id = s.id
         LEFT JOIN issues AS i ON i.claimed_by = a.id
         WHERE t.project_id = ?1 AND a.state IN ('running', 'idle', 'ready', 'working')"
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let agents: Vec<SwarmAgent> = stmt.query_map(params![pid], |row| {
        Ok(SwarmAgent {
            session_id: row.get(0)?,
            role: row.get(1)?,
            runtime: row.get(2)?,
            state: row.get(3)?,
            issue_id: row.get(4)?,
            issue_title: row.get(5)?,
        })
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .filter_map(|r| r.ok())
    .collect();

    Ok(Json(SwarmStatus {
        coordination_mode,
        active_agents,
        idle_agents,
        total_agents,
        task_pool_depth,
        throughput_issues_per_hour: throughput,
        scaling_recommendation,
        agents,
    }))
}
