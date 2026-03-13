use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLogEntry {
    pub id: String,
    pub event_type: String,
    pub project_id: Option<String>,
    pub team_id: Option<String>,
    pub agent_id: Option<String>,
    pub issue_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub message: String,
    pub metadata: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct LogEvent {
    pub event_type: String,
    pub project_id: Option<String>,
    pub team_id: Option<String>,
    pub agent_id: Option<String>,
    pub issue_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct DailyMetric {
    pub day: String,
    pub event_type: String,
    pub count: i64,
}

impl ActivityLogEntry {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            event_type: row.get("event_type")?,
            project_id: row.get("project_id")?,
            team_id: row.get("team_id")?,
            agent_id: row.get("agent_id")?,
            issue_id: row.get("issue_id")?,
            workflow_instance_id: row.get("workflow_instance_id")?,
            message: row.get("message")?,
            metadata: row.get("metadata")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn log(conn: &Connection, event: &LogEvent) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let metadata = match &event.metadata {
            Some(v) => serde_json::to_string(v)?,
            None => "{}".to_string(),
        };
        conn.execute(
            "INSERT INTO activity_log (id, event_type, project_id, team_id, agent_id, issue_id, workflow_instance_id, message, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                event.event_type,
                event.project_id,
                event.team_id,
                event.agent_id,
                event.issue_id,
                event.workflow_instance_id,
                event.message,
                metadata,
            ],
        )?;
        Ok(id)
    }

    pub fn list_recent(conn: &Connection, limit: i64, offset: i64) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM activity_log ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
        )?;
        let rows = stmt.query_map(params![limit, offset], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, limit: i64) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM activity_log WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![project_id, limit], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn daily_metrics(conn: &Connection, days: i64) -> Result<Vec<DailyMetric>> {
        let mut stmt = conn.prepare(
            "SELECT date(created_at) AS day, event_type, COUNT(*) AS count
             FROM activity_log
             WHERE created_at >= datetime('now', ?1)
             GROUP BY day, event_type
             ORDER BY day, event_type"
        )?;
        let offset = format!("-{} days", days);
        let rows = stmt.query_map(params![offset], |row| {
            Ok(DailyMetric {
                day: row.get("day")?,
                event_type: row.get("event_type")?,
                count: row.get("count")?,
            })
        })?;
        let mut metrics = Vec::new();
        for row in rows {
            metrics.push(row?);
        }
        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_log_and_list_recent() {
        let conn = setup_db();
        let id = ActivityLogEntry::log(&conn, &LogEvent {
            event_type: "issue.created".to_string(),
            project_id: Some("p1".to_string()),
            team_id: None,
            agent_id: None,
            issue_id: Some("i1".to_string()),
            workflow_instance_id: None,
            message: "Issue created".to_string(),
            metadata: None,
        }).unwrap();
        assert!(!id.is_empty());

        let entries = ActivityLogEntry::list_recent(&conn, 10, 0).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "issue.created");
        assert_eq!(entries[0].message, "Issue created");
        assert_eq!(entries[0].metadata, "{}");
    }

    #[test]
    fn test_log_with_metadata() {
        let conn = setup_db();
        let meta = serde_json::json!({"key": "value"});
        ActivityLogEntry::log(&conn, &LogEvent {
            event_type: "agent.started".to_string(),
            project_id: None,
            team_id: Some("t1".to_string()),
            agent_id: Some("a1".to_string()),
            issue_id: None,
            workflow_instance_id: None,
            message: "Agent started".to_string(),
            metadata: Some(meta),
        }).unwrap();

        let entries = ActivityLogEntry::list_recent(&conn, 10, 0).unwrap();
        assert_eq!(entries.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&entries[0].metadata).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_list_by_project() {
        let conn = setup_db();
        ActivityLogEntry::log(&conn, &LogEvent {
            event_type: "test".to_string(),
            project_id: Some("p1".to_string()),
            team_id: None,
            agent_id: None,
            issue_id: None,
            workflow_instance_id: None,
            message: "For p1".to_string(),
            metadata: None,
        }).unwrap();
        ActivityLogEntry::log(&conn, &LogEvent {
            event_type: "test".to_string(),
            project_id: Some("p2".to_string()),
            team_id: None,
            agent_id: None,
            issue_id: None,
            workflow_instance_id: None,
            message: "For p2".to_string(),
            metadata: None,
        }).unwrap();

        let entries = ActivityLogEntry::list_by_project(&conn, "p1", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "For p1");
    }

    #[test]
    fn test_daily_metrics() {
        let conn = setup_db();
        for _ in 0..3 {
            ActivityLogEntry::log(&conn, &LogEvent {
                event_type: "issue.created".to_string(),
                project_id: None,
                team_id: None,
                agent_id: None,
                issue_id: None,
                workflow_instance_id: None,
                message: "test".to_string(),
                metadata: None,
            }).unwrap();
        }
        ActivityLogEntry::log(&conn, &LogEvent {
            event_type: "agent.started".to_string(),
            project_id: None,
            team_id: None,
            agent_id: None,
            issue_id: None,
            workflow_instance_id: None,
            message: "test".to_string(),
            metadata: None,
        }).unwrap();

        let metrics = ActivityLogEntry::daily_metrics(&conn, 7).unwrap();
        assert_eq!(metrics.len(), 2);
        let issue_metric = metrics.iter().find(|m| m.event_type == "issue.created").unwrap();
        assert_eq!(issue_metric.count, 3);
        let agent_metric = metrics.iter().find(|m| m.event_type == "agent.started").unwrap();
        assert_eq!(agent_metric.count, 1);
    }

    #[test]
    fn test_list_recent_with_offset() {
        let conn = setup_db();
        for i in 0..5 {
            ActivityLogEntry::log(&conn, &LogEvent {
                event_type: "test".to_string(),
                project_id: None,
                team_id: None,
                agent_id: None,
                issue_id: None,
                workflow_instance_id: None,
                message: format!("Entry {}", i),
                metadata: None,
            }).unwrap();
        }

        let page1 = ActivityLogEntry::list_recent(&conn, 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = ActivityLogEntry::list_recent(&conn, 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        let page3 = ActivityLogEntry::list_recent(&conn, 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }
}
