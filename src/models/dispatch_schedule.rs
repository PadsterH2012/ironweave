use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchSchedule {
    pub id: String,
    pub scope: String,
    pub project_id: Option<String>,
    pub cron_expression: String,
    pub action: String,
    pub timezone: String,
    pub is_enabled: bool,
    pub created_at: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDispatchSchedule {
    pub scope: String,
    pub project_id: Option<String>,
    pub cron_expression: String,
    pub action: String,
    pub timezone: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDispatchSchedule {
    pub cron_expression: Option<String>,
    pub action: Option<String>,
    pub timezone: Option<String>,
    pub is_enabled: Option<bool>,
    pub description: Option<String>,
}

impl DispatchSchedule {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            scope: row.get("scope")?,
            project_id: row.get("project_id")?,
            cron_expression: row.get("cron_expression")?,
            action: row.get("action")?,
            timezone: row.get("timezone")?,
            is_enabled: row.get::<_, i64>("is_enabled")? != 0,
            created_at: row.get("created_at")?,
            description: row.get("description")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateDispatchSchedule) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let tz = input.timezone.as_deref().unwrap_or("Europe/London");
        conn.execute(
            "INSERT INTO dispatch_schedules (id, scope, project_id, cron_expression, action, timezone, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, input.scope, input.project_id, input.cron_expression, input.action, tz, input.description],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM dispatch_schedules WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("schedule {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM dispatch_schedules ORDER BY scope, created_at")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut schedules = Vec::new();
        for row in rows { schedules.push(row?); }
        Ok(schedules)
    }

    pub fn list_enabled(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM dispatch_schedules WHERE is_enabled = 1 ORDER BY scope, created_at")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut schedules = Vec::new();
        for row in rows { schedules.push(row?); }
        Ok(schedules)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM dispatch_schedules WHERE project_id = ?1 ORDER BY created_at")?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut schedules = Vec::new();
        for row in rows { schedules.push(row?); }
        Ok(schedules)
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateDispatchSchedule) -> Result<Self> {
        let existing = Self::get_by_id(conn, id)?;
        let cron_expression = input.cron_expression.as_deref().unwrap_or(&existing.cron_expression);
        let action = input.action.as_deref().unwrap_or(&existing.action);
        let timezone = input.timezone.as_deref().unwrap_or(&existing.timezone);
        let is_enabled = input.is_enabled.unwrap_or(existing.is_enabled);
        let description = input.description.as_deref().or(existing.description.as_deref());

        conn.execute(
            "UPDATE dispatch_schedules SET cron_expression = ?1, action = ?2, timezone = ?3, is_enabled = ?4, description = ?5 WHERE id = ?6",
            params![cron_expression, action, timezone, is_enabled as i64, description, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM dispatch_schedules WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("schedule {}", id)));
        }
        Ok(())
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
    fn test_create_global_schedule() {
        let conn = setup_db();
        let input = CreateDispatchSchedule {
            scope: "global".to_string(),
            project_id: None,
            cron_expression: "0 9 * * 1-5".to_string(),
            action: "resume".to_string(),
            timezone: Some("Europe/London".to_string()),
            description: Some("Weekday start".to_string()),
        };
        let schedule = DispatchSchedule::create(&conn, &input).unwrap();
        assert_eq!(schedule.scope, "global");
        assert_eq!(schedule.action, "resume");
        assert!(schedule.is_enabled);
    }

    #[test]
    fn test_create_project_schedule() {
        let conn = setup_db();
        let proj = crate::models::project::Project::create(&conn, &crate::models::project::CreateProject {
            name: "Test".to_string(),
            directory: "/tmp/test".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        }).unwrap();

        let input = CreateDispatchSchedule {
            scope: "project".to_string(),
            project_id: Some(proj.id.clone()),
            cron_expression: "0 18 * * 1-5".to_string(),
            action: "pause".to_string(),
            timezone: None,
            description: None,
        };
        let schedule = DispatchSchedule::create(&conn, &input).unwrap();
        assert_eq!(schedule.scope, "project");
        assert_eq!(schedule.project_id, Some(proj.id));
    }

    #[test]
    fn test_list_enabled() {
        let conn = setup_db();
        let input1 = CreateDispatchSchedule {
            scope: "global".to_string(), project_id: None,
            cron_expression: "0 9 * * *".to_string(), action: "resume".to_string(),
            timezone: None, description: None,
        };
        let input2 = CreateDispatchSchedule {
            scope: "global".to_string(), project_id: None,
            cron_expression: "0 18 * * *".to_string(), action: "pause".to_string(),
            timezone: None, description: None,
        };
        let s1 = DispatchSchedule::create(&conn, &input1).unwrap();
        DispatchSchedule::create(&conn, &input2).unwrap();

        DispatchSchedule::update(&conn, &s1.id, &UpdateDispatchSchedule {
            cron_expression: None, action: None, timezone: None,
            is_enabled: Some(false), description: None,
        }).unwrap();

        let enabled = DispatchSchedule::list_enabled(&conn).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].action, "pause");
    }

    #[test]
    fn test_delete_schedule() {
        let conn = setup_db();
        let input = CreateDispatchSchedule {
            scope: "global".to_string(), project_id: None,
            cron_expression: "0 9 * * *".to_string(), action: "resume".to_string(),
            timezone: None, description: None,
        };
        let schedule = DispatchSchedule::create(&conn, &input).unwrap();
        DispatchSchedule::delete(&conn, &schedule.id).unwrap();
        assert!(DispatchSchedule::get_by_id(&conn, &schedule.id).is_err());
    }
}
