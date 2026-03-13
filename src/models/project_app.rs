use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectApp {
    pub id: String,
    pub project_id: String,
    pub pid: Option<i64>,
    pub port: Option<i32>,
    pub run_command: String,
    pub state: String,
    pub last_error: Option<String>,
    pub started_at: Option<String>,
    pub created_at: String,
}

impl ProjectApp {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            pid: row.get("pid")?,
            port: row.get("port")?,
            run_command: row.get("run_command")?,
            state: row.get("state")?,
            last_error: row.get("last_error")?,
            started_at: row.get("started_at")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn get_by_project(conn: &Connection, project_id: &str) -> Result<Option<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM project_apps WHERE project_id = ?1"
        )?;
        let result = stmt.query_row(params![project_id], Self::from_row).ok();
        Ok(result)
    }

    pub fn upsert(conn: &Connection, project_id: &str, run_command: &str) -> Result<Self> {
        if let Some(existing) = Self::get_by_project(conn, project_id)? {
            conn.execute(
                "UPDATE project_apps SET run_command = ?1 WHERE id = ?2",
                params![run_command, existing.id],
            )?;
            return Self::get_by_project(conn, project_id)?
                .ok_or_else(|| crate::error::IronweaveError::NotFound("project_app".into()));
        }

        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO project_apps (id, project_id, run_command, state) VALUES (?1, ?2, ?3, 'stopped')",
            params![id, project_id, run_command],
        )?;
        Self::get_by_project(conn, project_id)?
            .ok_or_else(|| crate::error::IronweaveError::NotFound("project_app".into()))
    }

    pub fn update_state(conn: &Connection, id: &str, state: &str, pid: Option<i64>, port: Option<i32>, error: Option<&str>) -> Result<()> {
        let started_at = if state == "running" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        conn.execute(
            "UPDATE project_apps SET state = ?1, pid = ?2, port = ?3, last_error = ?4, started_at = ?5 WHERE id = ?6",
            params![state, pid, port, error, started_at, id],
        )?;
        Ok(())
    }
}
