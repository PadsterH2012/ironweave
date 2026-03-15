use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CrossProjectQuery {
    pub min_observations: Option<i64>,
    pub days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GlobalRoutingSuggestion {
    pub role: String,
    pub task_type: String,
    pub from_model: Option<String>,
    pub to_model: String,
    pub to_tier: i32,
    pub confidence: f64,
    pub observations: i64,
    pub projects_contributing: i64,
    pub reason: String,
}

/// Aggregate performance data across all projects with share_learning=1
/// and generate global routing suggestions
pub async fn global_suggestions(
    State(state): State<AppState>,
    Query(query): Query<CrossProjectQuery>,
) -> Result<Json<Vec<GlobalRoutingSuggestion>>, StatusCode> {
    let conn = state.conn()?;
    let min_obs = query.min_observations.unwrap_or(20);
    let days = query.days.unwrap_or(30);
    let offset = format!("-{} days", days);

    // Aggregate across opted-in projects
    let mut stmt = conn.prepare(
        "SELECT p.role, p.model, p.task_type, p.tier,
                COUNT(*) as total,
                SUM(CASE WHEN p.outcome = 'failure' THEN 1 ELSE 0 END) as failures,
                COUNT(DISTINCT pr.id) as project_count
         FROM model_performance_log p
         JOIN projects pr ON p.project_id = pr.id
         WHERE pr.share_learning = 1
           AND p.created_at >= datetime('now', ?1)
         GROUP BY p.role, p.model, p.task_type
         HAVING total >= ?2 AND CAST(failures AS REAL) / total > 0.5
         ORDER BY failures DESC"
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = stmt.query_map(rusqlite::params![offset, min_obs], |row| {
        let total: i64 = row.get(4)?;
        let failures: i64 = row.get(5)?;
        let tier: i32 = row.get(3)?;
        let failure_rate = failures as f64 / total as f64;
        let new_tier = (tier + 1).min(5);

        Ok(GlobalRoutingSuggestion {
            role: row.get(0)?,
            task_type: row.get(2)?,
            from_model: Some(row.get::<_, String>(1)?),
            to_model: format!("tier-{}-auto", new_tier),
            to_tier: new_tier,
            confidence: failure_rate.min(0.95),
            observations: total,
            projects_contributing: row.get(6)?,
            reason: format!(
                "{:.0}% failure rate across {} observations from {} projects",
                failure_rate * 100.0, total, row.get::<_, i64>(6)?
            ),
        })
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut suggestions = Vec::new();
    for row in rows {
        suggestions.push(row.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?);
    }

    Ok(Json(suggestions))
}

/// List projects that have opted in to cross-project learning
pub async fn list_opted_in(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let conn = state.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, name FROM projects WHERE share_learning = 1 ORDER BY name"
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
        }))
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut projects = Vec::new();
    for row in rows {
        projects.push(row.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?);
    }
    Ok(Json(projects))
}

/// Toggle share_learning for a project
pub async fn toggle_sharing(
    State(state): State<AppState>,
    axum::extract::Path(pid): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn = state.conn()?;
    let current: i32 = conn.query_row(
        "SELECT share_learning FROM projects WHERE id = ?1",
        rusqlite::params![pid],
        |row| row.get(0),
    ).map_err(|_| StatusCode::NOT_FOUND)?;

    let new_val = if current == 0 { 1 } else { 0 };
    conn.execute(
        "UPDATE projects SET share_learning = ?1 WHERE id = ?2",
        rusqlite::params![new_val, pid],
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "project_id": pid,
        "share_learning": new_val == 1,
    })))
}
