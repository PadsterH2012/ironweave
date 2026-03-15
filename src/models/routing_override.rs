use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

/// A routing override suggestion or accepted rule from the Coordinator's learning system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingOverride {
    pub id: String,
    pub project_id: String,
    pub role: String,
    pub task_type: String,
    pub from_model: Option<String>,
    pub to_model: String,
    pub to_tier: i32,
    pub reason: String,
    pub confidence: f64,
    pub status: String,
    pub evidence: String,
    pub observations: i64,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoutingOverride {
    pub project_id: String,
    pub role: String,
    pub task_type: Option<String>,
    pub from_model: Option<String>,
    pub to_model: String,
    pub to_tier: i32,
    pub reason: String,
    pub confidence: Option<f64>,
    pub evidence: Option<String>,
    pub observations: Option<i64>,
}

impl RoutingOverride {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            role: row.get("role")?,
            task_type: row.get("task_type")?,
            from_model: row.get("from_model")?,
            to_model: row.get("to_model")?,
            to_tier: row.get("to_tier")?,
            reason: row.get("reason")?,
            confidence: row.get("confidence")?,
            status: row.get("status")?,
            evidence: row.get("evidence")?,
            observations: row.get("observations")?,
            created_at: row.get("created_at")?,
            resolved_at: row.get("resolved_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateRoutingOverride) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO model_routing_overrides (id, project_id, role, task_type, from_model, to_model, to_tier, reason, confidence, evidence, observations)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                input.project_id,
                input.role,
                input.task_type.as_deref().unwrap_or("task"),
                input.from_model,
                input.to_model,
                input.to_tier,
                input.reason,
                input.confidence.unwrap_or(0.5),
                input.evidence.as_deref().unwrap_or("{}"),
                input.observations.unwrap_or(0),
            ],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        let entry = conn.query_row(
            "SELECT * FROM model_routing_overrides WHERE id = ?1",
            params![id],
            Self::from_row,
        )?;
        Ok(entry)
    }

    /// List overrides for a project, optionally filtered by status
    pub fn list_by_project(conn: &Connection, project_id: &str, status: Option<&str>) -> Result<Vec<Self>> {
        let (sql, param_values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match status {
            Some(s) => (
                "SELECT * FROM model_routing_overrides WHERE project_id = ?1 AND status = ?2 ORDER BY created_at DESC".to_string(),
                vec![Box::new(project_id.to_string()), Box::new(s.to_string())],
            ),
            None => (
                "SELECT * FROM model_routing_overrides WHERE project_id = ?1 ORDER BY created_at DESC".to_string(),
                vec![Box::new(project_id.to_string())],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Get accepted overrides for a specific role+task_type (used during routing decisions)
    pub fn get_accepted_for_role(conn: &Connection, project_id: &str, role: &str, task_type: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM model_routing_overrides
             WHERE project_id = ?1 AND role = ?2 AND task_type = ?3 AND status = 'accepted'
             ORDER BY confidence DESC"
        )?;
        let rows = stmt.query_map(params![project_id, role, task_type], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Accept a suggested override
    pub fn accept(conn: &Connection, id: &str) -> Result<Self> {
        conn.execute(
            "UPDATE model_routing_overrides SET status = 'accepted', resolved_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Self::get_by_id(conn, id)
    }

    /// Reject a suggested override
    pub fn reject(conn: &Connection, id: &str) -> Result<Self> {
        conn.execute(
            "UPDATE model_routing_overrides SET status = 'rejected', resolved_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Self::get_by_id(conn, id)
    }

    /// Detect failure patterns from the performance log and generate suggestions.
    /// Looks for role+model+task_type combos with >50% failure rate over min_observations.
    pub fn detect_patterns(conn: &Connection, project_id: &str, min_observations: i64, days: i64) -> Result<Vec<Self>> {
        let offset = format!("-{} days", days);
        let mut stmt = conn.prepare(
            "SELECT role, model, task_type, tier,
                    COUNT(*) as total,
                    SUM(CASE WHEN outcome = 'failure' THEN 1 ELSE 0 END) as failures
             FROM model_performance_log
             WHERE project_id = ?1 AND created_at >= datetime('now', ?2)
             GROUP BY role, model, task_type
             HAVING total >= ?3 AND CAST(failures AS REAL) / total > 0.5
             ORDER BY failures DESC"
        )?;

        let rows = stmt.query_map(params![project_id, offset, min_observations], |row| {
            Ok((
                row.get::<_, String>(0)?,  // role
                row.get::<_, String>(1)?,  // model
                row.get::<_, String>(2)?,  // task_type
                row.get::<_, i32>(3)?,     // tier
                row.get::<_, i64>(4)?,     // total
                row.get::<_, i64>(5)?,     // failures
            ))
        })?;

        let mut suggestions = Vec::new();
        for row in rows {
            let (role, model, task_type, tier, total, failures) = row?;
            let failure_rate = failures as f64 / total as f64;

            // Check if we already have a pending suggestion for this combo
            let existing: i64 = conn.query_row(
                "SELECT COUNT(*) FROM model_routing_overrides
                 WHERE project_id = ?1 AND role = ?2 AND task_type = ?3 AND from_model = ?4 AND status = 'suggested'",
                params![project_id, role, task_type, model],
                |row| row.get(0),
            )?;
            if existing > 0 {
                continue;
            }

            // Suggest moving up one tier
            let new_tier = (tier + 1).min(5);
            let evidence = serde_json::json!({
                "total_observations": total,
                "failures": failures,
                "failure_rate": failure_rate,
                "period_days": days,
            });

            let suggestion = Self::create(conn, &CreateRoutingOverride {
                project_id: project_id.to_string(),
                role: role.clone(),
                task_type: Some(task_type),
                from_model: Some(model),
                to_model: format!("tier-{}-auto", new_tier),
                to_tier: new_tier,
                reason: format!("{:.0}% failure rate over {} observations — suggest escalating to tier {}", failure_rate * 100.0, total, new_tier),
                confidence: Some(failure_rate.min(0.95)),
                evidence: Some(evidence.to_string()),
                observations: Some(total),
            })?;
            suggestions.push(suggestion);
        }

        Ok(suggestions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'Test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_create_and_list() {
        let conn = setup_db();
        let override_ = RoutingOverride::create(&conn, &CreateRoutingOverride {
            project_id: "p1".to_string(),
            role: "Senior Coder".to_string(),
            task_type: Some("feature".to_string()),
            from_model: Some("claude-haiku-4-5".to_string()),
            to_model: "claude-sonnet-4-6".to_string(),
            to_tier: 4,
            reason: "High failure rate on Haiku".to_string(),
            confidence: Some(0.8),
            evidence: None,
            observations: Some(25),
        }).unwrap();

        assert_eq!(override_.status, "suggested");
        assert_eq!(override_.role, "Senior Coder");

        let all = RoutingOverride::list_by_project(&conn, "p1", None).unwrap();
        assert_eq!(all.len(), 1);

        let suggested = RoutingOverride::list_by_project(&conn, "p1", Some("suggested")).unwrap();
        assert_eq!(suggested.len(), 1);
    }

    #[test]
    fn test_accept_reject() {
        let conn = setup_db();
        let o1 = RoutingOverride::create(&conn, &CreateRoutingOverride {
            project_id: "p1".to_string(),
            role: "Tester".to_string(),
            task_type: None,
            from_model: None,
            to_model: "claude-sonnet-4-6".to_string(),
            to_tier: 4,
            reason: "test".to_string(),
            confidence: None,
            evidence: None,
            observations: None,
        }).unwrap();

        let accepted = RoutingOverride::accept(&conn, &o1.id).unwrap();
        assert_eq!(accepted.status, "accepted");
        assert!(accepted.resolved_at.is_some());

        let o2 = RoutingOverride::create(&conn, &CreateRoutingOverride {
            project_id: "p1".to_string(),
            role: "Documentor".to_string(),
            task_type: None,
            from_model: None,
            to_model: "deepseek-v3".to_string(),
            to_tier: 3,
            reason: "test".to_string(),
            confidence: None,
            evidence: None,
            observations: None,
        }).unwrap();

        let rejected = RoutingOverride::reject(&conn, &o2.id).unwrap();
        assert_eq!(rejected.status, "rejected");
    }

    #[test]
    fn test_get_accepted_for_role() {
        let conn = setup_db();
        let o = RoutingOverride::create(&conn, &CreateRoutingOverride {
            project_id: "p1".to_string(),
            role: "Senior Coder".to_string(),
            task_type: Some("feature".to_string()),
            from_model: None,
            to_model: "claude-sonnet-4-6".to_string(),
            to_tier: 4,
            reason: "test".to_string(),
            confidence: Some(0.9),
            evidence: None,
            observations: None,
        }).unwrap();

        // Not accepted yet — should return empty
        let accepted = RoutingOverride::get_accepted_for_role(&conn, "p1", "Senior Coder", "feature").unwrap();
        assert_eq!(accepted.len(), 0);

        RoutingOverride::accept(&conn, &o.id).unwrap();

        let accepted = RoutingOverride::get_accepted_for_role(&conn, "p1", "Senior Coder", "feature").unwrap();
        assert_eq!(accepted.len(), 1);
    }

    #[test]
    fn test_detect_patterns() {
        let conn = setup_db();
        use crate::models::performance_log::{PerformanceLogEntry, CreatePerformanceLog};

        // Create 10 entries: 7 failures, 3 successes for Senior Coder on haiku
        for i in 0..10 {
            let outcome = if i < 7 { "failure" } else { "success" };
            PerformanceLogEntry::create(&conn, &CreatePerformanceLog {
                project_id: "p1".to_string(),
                role: "Senior Coder".to_string(),
                runtime: "claude".to_string(),
                provider: None,
                model: "claude-haiku-4-5".to_string(),
                tier: Some(3),
                task_type: Some("feature".to_string()),
                task_complexity: None,
                outcome: outcome.to_string(),
                failure_reason: None,
                tokens_used: Some(5000),
                cost_usd: Some(0.05),
                duration_seconds: Some(60),
                retries: None,
                escalated_from: None,
                complexity_score: None,
                files_touched: None,
            }).unwrap();
        }

        let suggestions = RoutingOverride::detect_patterns(&conn, "p1", 5, 30).unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].role, "Senior Coder");
        assert_eq!(suggestions[0].to_tier, 4); // escalated from tier 3
        assert!(suggestions[0].reason.contains("70%"));

        // Running again should not create duplicates
        let suggestions2 = RoutingOverride::detect_patterns(&conn, "p1", 5, 30).unwrap();
        assert_eq!(suggestions2.len(), 0);
    }
}
