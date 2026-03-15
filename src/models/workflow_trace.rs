use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTrace {
    pub id: String,
    pub project_id: String,
    pub agent_session_id: String,
    pub performance_log_id: Option<String>,
    pub issue_id: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub total_steps: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub id: String,
    pub trace_id: String,
    pub step_number: i64,
    pub action: String,
    pub detail: String,
    pub files_touched: Option<String>,
    pub tokens_used: i64,
    pub duration_ms: i64,
    pub outcome: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTraceStep {
    pub action: String,
    pub detail: Option<String>,
    pub files_touched: Option<String>,
    pub tokens_used: Option<i64>,
    pub duration_ms: Option<i64>,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chokepoint {
    pub id: String,
    pub project_id: String,
    pub action: String,
    pub role: Option<String>,
    pub failure_count: i64,
    pub total_count: i64,
    pub avg_duration_ms: i64,
    pub last_seen_at: String,
}

#[derive(Debug, Serialize)]
pub struct ChokepointSummary {
    pub action: String,
    pub role: Option<String>,
    pub failure_rate: f64,
    pub total_count: i64,
    pub avg_duration_ms: i64,
}

impl WorkflowTrace {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            agent_session_id: row.get("agent_session_id")?,
            performance_log_id: row.get("performance_log_id")?,
            issue_id: row.get("issue_id")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            total_steps: row.get("total_steps")?,
            status: row.get("status")?,
        })
    }

    /// Start a new trace for an agent session
    pub fn start(conn: &Connection, project_id: &str, agent_session_id: &str, issue_id: Option<&str>) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO workflow_traces (id, project_id, agent_session_id, issue_id) VALUES (?1, ?2, ?3, ?4)",
            params![id, project_id, agent_session_id, issue_id],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        let t = conn.query_row(
            "SELECT * FROM workflow_traces WHERE id = ?1",
            params![id],
            Self::from_row,
        )?;
        Ok(t)
    }

    pub fn get_by_session(conn: &Connection, agent_session_id: &str) -> Result<Option<Self>> {
        match conn.query_row(
            "SELECT * FROM workflow_traces WHERE agent_session_id = ?1 ORDER BY started_at DESC LIMIT 1",
            params![agent_session_id],
            Self::from_row,
        ) {
            Ok(t) => Ok(Some(t)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Complete a trace and link to performance log
    pub fn complete(conn: &Connection, id: &str, performance_log_id: Option<&str>, success: bool) -> Result<Self> {
        let status = if success { "completed" } else { "failed" };
        conn.execute(
            "UPDATE workflow_traces SET status = ?1, completed_at = datetime('now'), performance_log_id = ?2 WHERE id = ?3",
            params![status, performance_log_id, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, limit: i64) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM workflow_traces WHERE project_id = ?1 ORDER BY started_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![project_id, limit], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Add a step to this trace
    pub fn add_step(conn: &Connection, trace_id: &str, input: &CreateTraceStep) -> Result<TraceStep> {
        let id = Uuid::new_v4().to_string();
        // Get next step number
        let step_number: i64 = conn.query_row(
            "SELECT COALESCE(MAX(step_number), 0) + 1 FROM workflow_trace_steps WHERE trace_id = ?1",
            params![trace_id],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO workflow_trace_steps (id, trace_id, step_number, action, detail, files_touched, tokens_used, duration_ms, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                trace_id,
                step_number,
                input.action,
                input.detail.as_deref().unwrap_or(""),
                input.files_touched,
                input.tokens_used.unwrap_or(0),
                input.duration_ms.unwrap_or(0),
                input.outcome,
            ],
        )?;

        // Update step count on trace
        conn.execute(
            "UPDATE workflow_traces SET total_steps = ?1 WHERE id = ?2",
            params![step_number, trace_id],
        )?;

        TraceStep::get_by_id(conn, &id)
    }
}

impl TraceStep {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            trace_id: row.get("trace_id")?,
            step_number: row.get("step_number")?,
            action: row.get("action")?,
            detail: row.get("detail")?,
            files_touched: row.get("files_touched")?,
            tokens_used: row.get("tokens_used")?,
            duration_ms: row.get("duration_ms")?,
            outcome: row.get("outcome")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        let s = conn.query_row(
            "SELECT * FROM workflow_trace_steps WHERE id = ?1",
            params![id],
            Self::from_row,
        )?;
        Ok(s)
    }

    pub fn list_by_trace(conn: &Connection, trace_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM workflow_trace_steps WHERE trace_id = ?1 ORDER BY step_number"
        )?;
        let rows = stmt.query_map(params![trace_id], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }
}

impl Chokepoint {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            action: row.get("action")?,
            role: row.get("role")?,
            failure_count: row.get("failure_count")?,
            total_count: row.get("total_count")?,
            avg_duration_ms: row.get("avg_duration_ms")?,
            last_seen_at: row.get("last_seen_at")?,
        })
    }

    /// Detect chokepoints by aggregating trace step outcomes
    pub fn detect(conn: &Connection, project_id: &str) -> Result<Vec<ChokepointSummary>> {
        let mut stmt = conn.prepare(
            "SELECT s.action, NULL as role,
                    SUM(CASE WHEN s.outcome = 'failure' THEN 1 ELSE 0 END) as failures,
                    COUNT(*) as total,
                    AVG(s.duration_ms) as avg_dur
             FROM workflow_trace_steps s
             JOIN workflow_traces t ON s.trace_id = t.id
             WHERE t.project_id = ?1 AND s.outcome IS NOT NULL
             GROUP BY s.action
             HAVING total >= 5 AND CAST(failures AS REAL) / total > 0.3
             ORDER BY CAST(failures AS REAL) / total DESC"
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let failures: i64 = row.get(2)?;
            let total: i64 = row.get(3)?;
            Ok(ChokepointSummary {
                action: row.get(0)?,
                role: row.get(1)?,
                failure_rate: if total > 0 { failures as f64 / total as f64 } else { 0.0 },
                total_count: total,
                avg_duration_ms: row.get::<_, f64>(4)? as i64,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// List stored chokepoints
    pub fn list(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM workflow_chokepoints WHERE project_id = ?1 ORDER BY failure_count DESC"
        )?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
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
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'Test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_trace_lifecycle() {
        let conn = setup_db();

        let trace = WorkflowTrace::start(&conn, "p1", "session-1", Some("issue-1")).unwrap();
        assert_eq!(trace.status, "recording");
        assert_eq!(trace.total_steps, 0);

        // Add steps
        WorkflowTrace::add_step(&conn, &trace.id, &CreateTraceStep {
            action: "read_file".to_string(),
            detail: Some("src/main.rs".to_string()),
            files_touched: Some("src/main.rs".to_string()),
            tokens_used: Some(1000),
            duration_ms: Some(500),
            outcome: Some("success".to_string()),
        }).unwrap();

        WorkflowTrace::add_step(&conn, &trace.id, &CreateTraceStep {
            action: "edit_file".to_string(),
            detail: Some("Added function".to_string()),
            files_touched: Some("src/main.rs".to_string()),
            tokens_used: Some(2000),
            duration_ms: Some(1500),
            outcome: Some("success".to_string()),
        }).unwrap();

        let steps = TraceStep::list_by_trace(&conn, &trace.id).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].step_number, 1);
        assert_eq!(steps[1].step_number, 2);

        // Complete
        let completed = WorkflowTrace::complete(&conn, &trace.id, Some("perf-log-1"), true).unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.total_steps, 2);
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_get_by_session() {
        let conn = setup_db();
        WorkflowTrace::start(&conn, "p1", "session-1", None).unwrap();

        let found = WorkflowTrace::get_by_session(&conn, "session-1").unwrap();
        assert!(found.is_some());

        let not_found = WorkflowTrace::get_by_session(&conn, "session-999").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_list_by_project() {
        let conn = setup_db();
        WorkflowTrace::start(&conn, "p1", "s1", None).unwrap();
        WorkflowTrace::start(&conn, "p1", "s2", None).unwrap();

        let traces = WorkflowTrace::list_by_project(&conn, "p1", 10).unwrap();
        assert_eq!(traces.len(), 2);
    }
}
