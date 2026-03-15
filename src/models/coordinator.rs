use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

/// Persistent coordinator state per project.
/// The coordinator is a long-lived agent that manages team assembly and model routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorMemory {
    pub id: String,
    pub project_id: String,
    pub state: String,
    pub session_id: Option<String>,
    pub last_active_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct WakeCoordinator {
    pub session_id: String,
}

impl CoordinatorMemory {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            state: row.get("state")?,
            session_id: row.get("session_id")?,
            last_active_at: row.get("last_active_at")?,
            created_at: row.get("created_at")?,
        })
    }

    /// Get or create coordinator memory for a project
    pub fn get_or_create(conn: &Connection, project_id: &str) -> Result<Self> {
        match conn.query_row(
            "SELECT * FROM coordinator_memory WHERE project_id = ?1",
            params![project_id],
            Self::from_row,
        ) {
            Ok(cm) => Ok(cm),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let id = Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO coordinator_memory (id, project_id) VALUES (?1, ?2)",
                    params![id, project_id],
                )?;
                Self::get_by_id(conn, &id)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        let cm = conn.query_row(
            "SELECT * FROM coordinator_memory WHERE id = ?1",
            params![id],
            Self::from_row,
        )?;
        Ok(cm)
    }

    pub fn get_by_project(conn: &Connection, project_id: &str) -> Result<Self> {
        let cm = conn.query_row(
            "SELECT * FROM coordinator_memory WHERE project_id = ?1",
            params![project_id],
            Self::from_row,
        )?;
        Ok(cm)
    }

    /// Wake the coordinator — set state to active and record the session
    pub fn wake(conn: &Connection, project_id: &str, session_id: &str) -> Result<Self> {
        let cm = Self::get_or_create(conn, project_id)?;
        conn.execute(
            "UPDATE coordinator_memory SET state = 'active', session_id = ?1, last_active_at = datetime('now') WHERE id = ?2",
            params![session_id, cm.id],
        )?;
        Self::get_by_id(conn, &cm.id)
    }

    /// Put coordinator to sleep
    pub fn sleep(conn: &Connection, project_id: &str) -> Result<Self> {
        let cm = Self::get_or_create(conn, project_id)?;
        conn.execute(
            "UPDATE coordinator_memory SET state = 'dormant', session_id = NULL, last_active_at = datetime('now') WHERE id = ?1",
            params![cm.id],
        )?;
        Self::get_by_id(conn, &cm.id)
    }

    /// List all coordinator states
    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM coordinator_memory ORDER BY last_active_at DESC")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn is_active(&self) -> bool {
        self.state == "active"
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
    fn test_get_or_create() {
        let conn = setup_db();
        let cm = CoordinatorMemory::get_or_create(&conn, "p1").unwrap();
        assert_eq!(cm.project_id, "p1");
        assert_eq!(cm.state, "dormant");
        assert!(cm.session_id.is_none());

        // Second call returns same record
        let cm2 = CoordinatorMemory::get_or_create(&conn, "p1").unwrap();
        assert_eq!(cm.id, cm2.id);
    }

    #[test]
    fn test_wake_and_sleep() {
        let conn = setup_db();
        let cm = CoordinatorMemory::wake(&conn, "p1", "session-123").unwrap();
        assert_eq!(cm.state, "active");
        assert_eq!(cm.session_id, Some("session-123".to_string()));
        assert!(cm.is_active());

        let cm = CoordinatorMemory::sleep(&conn, "p1").unwrap();
        assert_eq!(cm.state, "dormant");
        assert!(cm.session_id.is_none());
        assert!(!cm.is_active());
    }

    #[test]
    fn test_list() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p2', 'Test2', '/tmp2', 'homelab')",
            [],
        ).unwrap();

        CoordinatorMemory::get_or_create(&conn, "p1").unwrap();
        CoordinatorMemory::get_or_create(&conn, "p2").unwrap();

        let all = CoordinatorMemory::list(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }
}
