use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeQueueEntry {
    pub id: String,
    pub project_id: String,
    pub branch_name: String,
    pub agent_session_id: Option<String>,
    pub issue_id: Option<String>,
    pub team_id: Option<String>,
    pub status: String,
    pub conflict_files: String,
    pub resolver_agent_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl MergeQueueEntry {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            branch_name: row.get("branch_name")?,
            agent_session_id: row.get("agent_session_id")?,
            issue_id: row.get("issue_id")?,
            team_id: row.get("team_id")?,
            status: row.get("status")?,
            conflict_files: row.get("conflict_files")?,
            resolver_agent_id: row.get("resolver_agent_id")?,
            error_message: row.get("error_message")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn create(
        conn: &Connection,
        project_id: &str,
        branch_name: &str,
        agent_session_id: Option<&str>,
        issue_id: Option<&str>,
        team_id: Option<&str>,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO merge_queue (id, project_id, branch_name, agent_session_id, issue_id, team_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, project_id, branch_name, agent_session_id, issue_id, team_id],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM merge_queue WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("merge_queue {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM merge_queue WHERE project_id = ?1 ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn next_pending(conn: &Connection, project_id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM merge_queue WHERE project_id = ?1 AND status = 'pending' ORDER BY created_at ASC LIMIT 1",
            params![project_id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound("no pending merge queue entries".to_string()),
            other => IronweaveError::Database(other),
        })
    }

    const VALID_STATUSES: &'static [&'static str] = &["pending", "merging", "merged", "conflict", "failed"];

    pub fn update_status(
        conn: &Connection,
        id: &str,
        status: &str,
        conflict_files: Option<&str>,
        resolver_agent_id: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<Self> {
        if !Self::VALID_STATUSES.contains(&status) {
            return Err(IronweaveError::Validation(format!(
                "Invalid merge queue status '{}'. Must be one of: {}",
                status, Self::VALID_STATUSES.join(", ")
            )));
        }
        let changes = conn.execute(
            "UPDATE merge_queue SET status = ?1, conflict_files = COALESCE(?2, conflict_files), resolver_agent_id = COALESCE(?3, resolver_agent_id), error_message = ?4, updated_at = datetime('now') WHERE id = ?5",
            params![status, conflict_files, resolver_agent_id, error_message, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("merge_queue {}", id)));
        }
        Self::get_by_id(conn, id)
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

    fn create_project(conn: &Connection) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES (?1, ?2, ?3, ?4)",
            params![id, "TestProj", "/tmp/test", "homelab"],
        ).unwrap();
        id
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let pid = create_project(&conn);
        let entry = MergeQueueEntry::create(&conn, &pid, "feature/login", Some("agent-1"), Some("issue-1"), Some("team-1")).unwrap();
        assert_eq!(entry.branch_name, "feature/login");
        assert_eq!(entry.status, "pending");
        assert_eq!(entry.agent_session_id.as_deref(), Some("agent-1"));

        let fetched = MergeQueueEntry::get_by_id(&conn, &entry.id).unwrap();
        assert_eq!(fetched.id, entry.id);
    }

    #[test]
    fn test_list_by_project() {
        let conn = setup_db();
        let pid = create_project(&conn);
        MergeQueueEntry::create(&conn, &pid, "feature/a", None, None, None).unwrap();
        MergeQueueEntry::create(&conn, &pid, "feature/b", None, None, None).unwrap();

        let entries = MergeQueueEntry::list_by_project(&conn, &pid).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_next_pending() {
        let conn = setup_db();
        let pid = create_project(&conn);
        let first = MergeQueueEntry::create(&conn, &pid, "feature/first", None, None, None).unwrap();
        MergeQueueEntry::create(&conn, &pid, "feature/second", None, None, None).unwrap();

        let next = MergeQueueEntry::next_pending(&conn, &pid).unwrap();
        assert_eq!(next.id, first.id);
    }

    #[test]
    fn test_update_status() {
        let conn = setup_db();
        let pid = create_project(&conn);
        let entry = MergeQueueEntry::create(&conn, &pid, "feature/test", None, None, None).unwrap();

        let updated = MergeQueueEntry::update_status(
            &conn, &entry.id, "conflict",
            Some("[\"src/main.rs\"]"), Some("resolver-1"), Some("merge conflict detected"),
        ).unwrap();
        assert_eq!(updated.status, "conflict");
        assert_eq!(updated.conflict_files, "[\"src/main.rs\"]");
        assert_eq!(updated.resolver_agent_id.as_deref(), Some("resolver-1"));
        assert_eq!(updated.error_message.as_deref(), Some("merge conflict detected"));
    }

    #[test]
    fn test_approve_resets_to_pending() {
        let conn = setup_db();
        let pid = create_project(&conn);
        let entry = MergeQueueEntry::create(&conn, &pid, "feature/fix", None, None, None).unwrap();
        MergeQueueEntry::update_status(&conn, &entry.id, "conflict", None, None, Some("conflict")).unwrap();

        let approved = MergeQueueEntry::update_status(&conn, &entry.id, "pending", None, None, None).unwrap();
        assert_eq!(approved.status, "pending");
        assert!(approved.error_message.is_none());
    }
}
