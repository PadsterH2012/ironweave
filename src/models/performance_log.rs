use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceLogEntry {
    pub id: String,
    pub project_id: String,
    pub role: String,
    pub runtime: String,
    pub provider: String,
    pub model: String,
    pub tier: i32,
    pub task_type: String,
    pub task_complexity: i32,
    pub outcome: String,
    pub failure_reason: Option<String>,
    pub tokens_used: i64,
    pub cost_usd: f64,
    pub duration_seconds: i64,
    pub retries: i32,
    pub escalated_from: Option<String>,
    pub complexity_score: Option<i32>,
    pub files_touched: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePerformanceLog {
    pub project_id: String,
    pub role: String,
    pub runtime: String,
    pub provider: Option<String>,
    pub model: String,
    pub tier: Option<i32>,
    pub task_type: Option<String>,
    pub task_complexity: Option<i32>,
    pub outcome: String,
    pub failure_reason: Option<String>,
    pub tokens_used: Option<i64>,
    pub cost_usd: Option<f64>,
    pub duration_seconds: Option<i64>,
    pub retries: Option<i32>,
    pub escalated_from: Option<String>,
    pub complexity_score: Option<i32>,
    pub files_touched: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceQuery {
    pub role: Option<String>,
    pub model: Option<String>,
    pub outcome: Option<String>,
    pub days: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ModelStats {
    pub model: String,
    pub role: String,
    pub runtime: String,
    pub total: i64,
    pub successes: i64,
    pub failures: i64,
    pub avg_tokens: f64,
    pub avg_cost: f64,
    pub avg_duration: f64,
    pub success_rate: f64,
}

impl PerformanceLogEntry {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            role: row.get("role")?,
            runtime: row.get("runtime")?,
            provider: row.get("provider")?,
            model: row.get("model")?,
            tier: row.get("tier")?,
            task_type: row.get("task_type")?,
            task_complexity: row.get("task_complexity")?,
            outcome: row.get("outcome")?,
            failure_reason: row.get("failure_reason")?,
            tokens_used: row.get("tokens_used")?,
            cost_usd: row.get("cost_usd")?,
            duration_seconds: row.get("duration_seconds")?,
            retries: row.get("retries")?,
            escalated_from: row.get("escalated_from")?,
            complexity_score: row.get("complexity_score")?,
            files_touched: row.get("files_touched")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreatePerformanceLog) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO model_performance_log (id, project_id, role, runtime, provider, model, tier,
             task_type, task_complexity, outcome, failure_reason, tokens_used, cost_usd,
             duration_seconds, retries, escalated_from, complexity_score, files_touched)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                id,
                input.project_id,
                input.role,
                input.runtime,
                input.provider.as_deref().unwrap_or("anthropic"),
                input.model,
                input.tier.unwrap_or(3),
                input.task_type.as_deref().unwrap_or("task"),
                input.task_complexity.unwrap_or(3),
                input.outcome,
                input.failure_reason,
                input.tokens_used.unwrap_or(0),
                input.cost_usd.unwrap_or(0.0),
                input.duration_seconds.unwrap_or(0),
                input.retries.unwrap_or(0),
                input.escalated_from,
                input.complexity_score,
                input.files_touched,
            ],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        let entry = conn.query_row(
            "SELECT * FROM model_performance_log WHERE id = ?1",
            params![id],
            Self::from_row,
        )?;
        Ok(entry)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, query: &PerformanceQuery) -> Result<Vec<Self>> {
        let mut sql = "SELECT * FROM model_performance_log WHERE project_id = ?1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(project_id.to_string())];
        let mut idx = 2;

        if let Some(ref role) = query.role {
            sql.push_str(&format!(" AND role = ?{}", idx));
            param_values.push(Box::new(role.clone()));
            idx += 1;
        }
        if let Some(ref model) = query.model {
            sql.push_str(&format!(" AND model = ?{}", idx));
            param_values.push(Box::new(model.clone()));
            idx += 1;
        }
        if let Some(ref outcome) = query.outcome {
            sql.push_str(&format!(" AND outcome = ?{}", idx));
            param_values.push(Box::new(outcome.clone()));
            idx += 1;
        }
        if let Some(days) = query.days {
            let offset = format!("-{} days", days);
            sql.push_str(&format!(" AND created_at >= datetime('now', ?{})", idx));
            param_values.push(Box::new(offset));
            idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        let limit = query.limit.unwrap_or(100);
        sql.push_str(&format!(" LIMIT ?{}", idx));
        param_values.push(Box::new(limit));

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Get aggregated stats per model+role for a project
    pub fn model_stats(conn: &Connection, project_id: &str, days: i64) -> Result<Vec<ModelStats>> {
        let offset = format!("-{} days", days);
        let mut stmt = conn.prepare(
            "SELECT model, role, runtime,
                    COUNT(*) as total,
                    SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) as successes,
                    SUM(CASE WHEN outcome = 'failure' THEN 1 ELSE 0 END) as failures,
                    AVG(tokens_used) as avg_tokens,
                    AVG(cost_usd) as avg_cost,
                    AVG(duration_seconds) as avg_duration
             FROM model_performance_log
             WHERE project_id = ?1 AND created_at >= datetime('now', ?2)
             GROUP BY model, role, runtime
             ORDER BY total DESC"
        )?;
        let rows = stmt.query_map(params![project_id, offset], |row| {
            let total: i64 = row.get("total")?;
            let successes: i64 = row.get("successes")?;
            Ok(ModelStats {
                model: row.get("model")?,
                role: row.get("role")?,
                runtime: row.get("runtime")?,
                total,
                successes,
                failures: row.get("failures")?,
                avg_tokens: row.get("avg_tokens")?,
                avg_cost: row.get("avg_cost")?,
                avg_duration: row.get("avg_duration")?,
                success_rate: if total > 0 { successes as f64 / total as f64 } else { 0.0 },
            })
        })?;
        let mut stats = Vec::new();
        for row in rows {
            stats.push(row?);
        }
        Ok(stats)
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
    fn test_create_and_get() {
        let conn = setup_db();
        let entry = PerformanceLogEntry::create(&conn, &CreatePerformanceLog {
            project_id: "p1".to_string(),
            role: "Senior Coder".to_string(),
            runtime: "claude".to_string(),
            provider: Some("anthropic".to_string()),
            model: "claude-sonnet-4-6".to_string(),
            tier: Some(4),
            task_type: Some("feature".to_string()),
            task_complexity: Some(5),
            outcome: "success".to_string(),
            failure_reason: None,
            tokens_used: Some(15000),
            cost_usd: Some(0.45),
            duration_seconds: Some(120),
            retries: Some(0),
            escalated_from: None,
            complexity_score: None,
            files_touched: Some("src/main.rs,src/lib.rs".to_string()),
        }).unwrap();

        assert_eq!(entry.role, "Senior Coder");
        assert_eq!(entry.outcome, "success");
        assert_eq!(entry.tokens_used, 15000);
        assert_eq!(entry.tier, 4);
    }

    #[test]
    fn test_list_with_filters() {
        let conn = setup_db();

        // Create two entries with different outcomes
        PerformanceLogEntry::create(&conn, &CreatePerformanceLog {
            project_id: "p1".to_string(),
            role: "Senior Coder".to_string(),
            runtime: "claude".to_string(),
            provider: None,
            model: "claude-sonnet-4-6".to_string(),
            tier: None,
            task_type: None,
            task_complexity: None,
            outcome: "success".to_string(),
            failure_reason: None,
            tokens_used: Some(10000),
            cost_usd: Some(0.30),
            duration_seconds: Some(60),
            retries: None,
            escalated_from: None,
            complexity_score: None,
            files_touched: None,
        }).unwrap();

        PerformanceLogEntry::create(&conn, &CreatePerformanceLog {
            project_id: "p1".to_string(),
            role: "Senior Tester".to_string(),
            runtime: "claude".to_string(),
            provider: None,
            model: "claude-sonnet-4-6".to_string(),
            tier: None,
            task_type: None,
            task_complexity: None,
            outcome: "failure".to_string(),
            failure_reason: Some("timeout".to_string()),
            tokens_used: Some(5000),
            cost_usd: Some(0.15),
            duration_seconds: Some(300),
            retries: Some(2),
            escalated_from: None,
            complexity_score: None,
            files_touched: None,
        }).unwrap();

        // All entries
        let all = PerformanceLogEntry::list_by_project(&conn, "p1", &PerformanceQuery {
            role: None, model: None, outcome: None, days: None, limit: None,
        }).unwrap();
        assert_eq!(all.len(), 2);

        // Filter by role
        let coders = PerformanceLogEntry::list_by_project(&conn, "p1", &PerformanceQuery {
            role: Some("Senior Coder".to_string()), model: None, outcome: None, days: None, limit: None,
        }).unwrap();
        assert_eq!(coders.len(), 1);

        // Filter by outcome
        let failures = PerformanceLogEntry::list_by_project(&conn, "p1", &PerformanceQuery {
            role: None, model: None, outcome: Some("failure".to_string()), days: None, limit: None,
        }).unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].failure_reason, Some("timeout".to_string()));
    }

    #[test]
    fn test_model_stats() {
        let conn = setup_db();

        for i in 0..5 {
            let outcome = if i < 4 { "success" } else { "failure" };
            PerformanceLogEntry::create(&conn, &CreatePerformanceLog {
                project_id: "p1".to_string(),
                role: "Senior Coder".to_string(),
                runtime: "claude".to_string(),
                provider: None,
                model: "claude-sonnet-4-6".to_string(),
                tier: None,
                task_type: None,
                task_complexity: None,
                outcome: outcome.to_string(),
                failure_reason: None,
                tokens_used: Some(10000),
                cost_usd: Some(0.30),
                duration_seconds: Some(60),
                retries: None,
                escalated_from: None,
                complexity_score: None,
                files_touched: None,
            }).unwrap();
        }

        let stats = PerformanceLogEntry::model_stats(&conn, "p1", 30).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].total, 5);
        assert_eq!(stats[0].successes, 4);
        assert_eq!(stats[0].failures, 1);
        assert!((stats[0].success_rate - 0.8).abs() < 0.001);
    }
}
