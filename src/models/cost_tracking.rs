use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTrackingEntry {
    pub id: String,
    pub project_id: String,
    pub period: String,
    pub period_start: String,
    pub total_tokens: i64,
    pub total_cost_usd: f64,
    pub by_tier: String,
    pub by_role: String,
    pub by_model: String,
    pub task_count: i64,
    pub failure_count: i64,
    pub escalation_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CostSummary {
    pub total_tokens: i64,
    pub total_cost_usd: f64,
    pub task_count: i64,
    pub failure_count: i64,
    pub by_role: serde_json::Value,
    pub by_model: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct DailySpend {
    pub date: String,
    pub cost_usd: f64,
    pub tokens: i64,
}

impl CostTrackingEntry {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            period: row.get("period")?,
            period_start: row.get("period_start")?,
            total_tokens: row.get("total_tokens")?,
            total_cost_usd: row.get("total_cost_usd")?,
            by_tier: row.get("by_tier")?,
            by_role: row.get("by_role")?,
            by_model: row.get("by_model")?,
            task_count: row.get("task_count")?,
            failure_count: row.get("failure_count")?,
            escalation_count: row.get("escalation_count")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, period: Option<&str>) -> Result<Vec<Self>> {
        let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match period {
            Some(p) => (
                "SELECT * FROM cost_tracking WHERE project_id = ?1 AND period = ?2 ORDER BY period_start DESC".to_string(),
                vec![Box::new(project_id.to_string()), Box::new(p.to_string())],
            ),
            None => (
                "SELECT * FROM cost_tracking WHERE project_id = ?1 ORDER BY period_start DESC".to_string(),
                vec![Box::new(project_id.to_string())],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Get today's aggregated cost for a project from agent_sessions
    pub fn daily_cost_from_sessions(conn: &Connection, project_id: &str) -> Result<f64> {
        let cost: f64 = conn.query_row(
            "SELECT COALESCE(SUM(s.cost), 0.0)
             FROM agent_sessions s
             JOIN teams t ON s.team_id = t.id
             WHERE t.project_id = ?1
               AND date(s.started_at) = date('now')",
            params![project_id],
            |row| row.get(0),
        )?;
        Ok(cost)
    }

    /// Get today's aggregated tokens for a team from agent_sessions
    pub fn team_tokens_today(conn: &Connection, team_id: &str) -> Result<i64> {
        let tokens: i64 = conn.query_row(
            "SELECT COALESCE(SUM(tokens_used), 0)
             FROM agent_sessions
             WHERE team_id = ?1
               AND date(started_at) = date('now')",
            params![team_id],
            |row| row.get(0),
        )?;
        Ok(tokens)
    }

    /// Get today's aggregated cost for a team from agent_sessions
    pub fn team_cost_today(conn: &Connection, team_id: &str) -> Result<f64> {
        let cost: f64 = conn.query_row(
            "SELECT COALESCE(SUM(cost), 0.0)
             FROM agent_sessions
             WHERE team_id = ?1
               AND date(started_at) = date('now')",
            params![team_id],
            |row| row.get(0),
        )?;
        Ok(cost)
    }

    /// Aggregate cost data from agent_sessions into cost_tracking table
    pub fn aggregate_daily(conn: &Connection, project_id: &str) -> Result<()> {
        let today = conn.query_row("SELECT date('now')", [], |row| row.get::<_, String>(0))?;

        // Gather aggregated data from sessions
        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(SUM(s.tokens_used), 0) AS total_tokens,
                COALESCE(SUM(s.cost), 0.0) AS total_cost,
                COUNT(DISTINCT s.claimed_task_id) AS task_count
             FROM agent_sessions s
             JOIN teams t ON s.team_id = t.id
             WHERE t.project_id = ?1
               AND date(s.started_at) = ?2"
        )?;
        let (total_tokens, total_cost, task_count): (i64, f64, i64) = stmt.query_row(
            params![project_id, today],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // By-role breakdown
        let mut role_stmt = conn.prepare(
            "SELECT ts.role, COALESCE(SUM(s.cost), 0.0)
             FROM agent_sessions s
             JOIN team_agent_slots ts ON s.slot_id = ts.id
             JOIN teams t ON s.team_id = t.id
             WHERE t.project_id = ?1 AND date(s.started_at) = ?2
             GROUP BY ts.role"
        )?;
        let role_rows = role_stmt.query_map(params![project_id, today], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        let mut by_role = serde_json::Map::new();
        for row in role_rows {
            let (role, cost) = row?;
            by_role.insert(role, serde_json::json!(cost));
        }

        // By-model breakdown
        let mut model_stmt = conn.prepare(
            "SELECT COALESCE(s.model, 'unknown'), COALESCE(SUM(s.cost), 0.0)
             FROM agent_sessions s
             JOIN teams t ON s.team_id = t.id
             WHERE t.project_id = ?1 AND date(s.started_at) = ?2
             GROUP BY s.model"
        )?;
        let model_rows = model_stmt.query_map(params![project_id, today], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        let mut by_model = serde_json::Map::new();
        for row in model_rows {
            let (model, cost) = row?;
            by_model.insert(model, serde_json::json!(cost));
        }

        let id = Uuid::new_v4().to_string();
        let by_role_json = serde_json::to_string(&by_role)?;
        let by_model_json = serde_json::to_string(&by_model)?;

        conn.execute(
            "INSERT INTO cost_tracking (id, project_id, period, period_start, total_tokens, total_cost_usd, by_role, by_model, task_count, updated_at)
             VALUES (?1, ?2, 'daily', ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
             ON CONFLICT(project_id, period, period_start)
             DO UPDATE SET total_tokens = ?4, total_cost_usd = ?5, by_role = ?6, by_model = ?7, task_count = ?8, updated_at = datetime('now')",
            params![id, project_id, today, total_tokens, total_cost, by_role_json, by_model_json, task_count],
        )?;

        Ok(())
    }

    /// Summary for a project over a date range
    pub fn project_summary(conn: &Connection, project_id: &str, days: i64) -> Result<CostSummary> {
        let offset = format!("-{} days", days);
        let (total_tokens, total_cost, task_count, failure_count): (i64, f64, i64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(total_tokens), 0), COALESCE(SUM(total_cost_usd), 0.0),
                    COALESCE(SUM(task_count), 0), COALESCE(SUM(failure_count), 0)
             FROM cost_tracking
             WHERE project_id = ?1 AND period = 'daily' AND period_start >= date('now', ?2)",
            params![project_id, offset],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        // Aggregate by_role and by_model across days
        let entries = Self::list_by_project(conn, project_id, Some("daily"))?;
        let mut agg_role: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        let mut agg_model: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

        for entry in &entries {
            if let Ok(role_map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&entry.by_role) {
                for (k, v) in role_map {
                    let val = v.as_f64().unwrap_or(0.0);
                    let existing = agg_role.get(&k).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    agg_role.insert(k, serde_json::json!(existing + val));
                }
            }
            if let Ok(model_map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&entry.by_model) {
                for (k, v) in model_map {
                    let val = v.as_f64().unwrap_or(0.0);
                    let existing = agg_model.get(&k).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    agg_model.insert(k, serde_json::json!(existing + val));
                }
            }
        }

        Ok(CostSummary {
            total_tokens,
            total_cost_usd: total_cost,
            task_count,
            failure_count,
            by_role: serde_json::Value::Object(agg_role),
            by_model: serde_json::Value::Object(agg_model),
        })
    }

    /// Daily spend for charting
    pub fn daily_spend(conn: &Connection, project_id: &str, days: i64) -> Result<Vec<DailySpend>> {
        let offset = format!("-{} days", days);
        let mut stmt = conn.prepare(
            "SELECT period_start, total_cost_usd, total_tokens
             FROM cost_tracking
             WHERE project_id = ?1 AND period = 'daily' AND period_start >= date('now', ?2)
             ORDER BY period_start"
        )?;
        let rows = stmt.query_map(params![project_id, offset], |row| {
            Ok(DailySpend {
                date: row.get(0)?,
                cost_usd: row.get(1)?,
                tokens: row.get(2)?,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();

        // Seed a project and team
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'Test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO teams (id, name, project_id, cost_budget_daily, token_budget) VALUES ('t1', 'Dev', 'p1', 5.0, 100000)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO team_agent_slots (id, team_id, role, runtime) VALUES ('s1', 't1', 'Senior Coder', 'claude')",
            [],
        ).unwrap();

        conn
    }

    #[test]
    fn test_daily_cost_from_sessions() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO agent_sessions (id, team_id, slot_id, runtime, tokens_used, cost) VALUES ('a1', 't1', 's1', 'claude', 5000, 0.50)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO agent_sessions (id, team_id, slot_id, runtime, tokens_used, cost) VALUES ('a2', 't1', 's1', 'claude', 3000, 0.30)",
            [],
        ).unwrap();

        let cost = CostTrackingEntry::daily_cost_from_sessions(&conn, "p1").unwrap();
        assert!((cost - 0.80).abs() < 0.001);
    }

    #[test]
    fn test_team_cost_today() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO agent_sessions (id, team_id, slot_id, runtime, tokens_used, cost) VALUES ('a1', 't1', 's1', 'claude', 5000, 0.50)",
            [],
        ).unwrap();

        let cost = CostTrackingEntry::team_cost_today(&conn, "t1").unwrap();
        assert!((cost - 0.50).abs() < 0.001);

        let tokens = CostTrackingEntry::team_tokens_today(&conn, "t1").unwrap();
        assert_eq!(tokens, 5000);
    }

    #[test]
    fn test_aggregate_daily() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO agent_sessions (id, team_id, slot_id, runtime, model, tokens_used, cost) VALUES ('a1', 't1', 's1', 'claude', 'claude-sonnet-4-6', 5000, 0.50)",
            [],
        ).unwrap();

        CostTrackingEntry::aggregate_daily(&conn, "p1").unwrap();

        let entries = CostTrackingEntry::list_by_project(&conn, "p1", Some("daily")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].total_tokens, 5000);
        assert!((entries[0].total_cost_usd - 0.50).abs() < 0.001);
    }

    #[test]
    fn test_aggregate_daily_idempotent() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO agent_sessions (id, team_id, slot_id, runtime, tokens_used, cost) VALUES ('a1', 't1', 's1', 'claude', 5000, 0.50)",
            [],
        ).unwrap();

        CostTrackingEntry::aggregate_daily(&conn, "p1").unwrap();
        CostTrackingEntry::aggregate_daily(&conn, "p1").unwrap();

        let entries = CostTrackingEntry::list_by_project(&conn, "p1", Some("daily")).unwrap();
        assert_eq!(entries.len(), 1);
    }
}
