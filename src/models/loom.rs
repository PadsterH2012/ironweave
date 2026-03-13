use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoomEntry {
    pub id: String,
    pub timestamp: String,
    pub agent_id: Option<String>,
    pub team_id: String,
    pub project_id: String,
    pub workflow_instance_id: Option<String>,
    pub entry_type: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateLoomEntry {
    pub agent_id: Option<String>,
    pub team_id: String,
    pub project_id: String,
    pub workflow_instance_id: Option<String>,
    pub entry_type: String,
    pub content: String,
}

impl LoomEntry {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            timestamp: row.get("timestamp")?,
            agent_id: row.get("agent_id")?,
            team_id: row.get("team_id")?,
            project_id: row.get("project_id")?,
            workflow_instance_id: row.get("workflow_instance_id")?,
            entry_type: row.get("entry_type")?,
            content: row.get("content")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateLoomEntry) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO loom_entries (id, agent_id, team_id, project_id, workflow_instance_id, entry_type, content)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, input.agent_id, input.team_id, input.project_id, input.workflow_instance_id, input.entry_type, input.content],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM loom_entries WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("loom_entry {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, limit: Option<i64>) -> Result<Vec<Self>> {
        let limit_val = limit.unwrap_or(100);
        let mut stmt = conn.prepare(
            "SELECT * FROM loom_entries WHERE project_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![project_id, limit_val], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn list_by_team(conn: &Connection, team_id: &str, limit: Option<i64>) -> Result<Vec<Self>> {
        let limit_val = limit.unwrap_or(100);
        let mut stmt = conn.prepare(
            "SELECT * FROM loom_entries WHERE team_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![team_id, limit_val], Self::from_row)?;
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
    use crate::models::project::{CreateProject, Project};
    use crate::models::team::{CreateTeam, Team};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn create_prereqs(conn: &Connection) -> (Project, Team) {
        let project = Project::create(conn, &CreateProject {
            name: "Test".to_string(),
            directory: "/tmp/test".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        }).unwrap();
        let team = Team::create(conn, &CreateTeam {
            name: "Dev".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();
        (project, team)
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let entry = LoomEntry::create(&conn, &CreateLoomEntry {
            agent_id: None,
            team_id: team.id.clone(),
            project_id: project.id.clone(),
            workflow_instance_id: None,
            entry_type: "status".to_string(),
            content: "Build started".to_string(),
        }).unwrap();
        assert_eq!(entry.content, "Build started");
        assert_eq!(entry.entry_type, "status");

        let fetched = LoomEntry::get_by_id(&conn, &entry.id).unwrap();
        assert_eq!(fetched.id, entry.id);
    }

    #[test]
    fn test_list_by_project() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        for i in 0..5 {
            LoomEntry::create(&conn, &CreateLoomEntry {
                agent_id: None,
                team_id: team.id.clone(),
                project_id: project.id.clone(),
                workflow_instance_id: None,
                entry_type: "status".to_string(),
                content: format!("Entry {}", i),
            }).unwrap();
        }

        let entries = LoomEntry::list_by_project(&conn, &project.id, Some(3)).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_list_by_team() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        LoomEntry::create(&conn, &CreateLoomEntry {
            agent_id: None,
            team_id: team.id.clone(),
            project_id: project.id.clone(),
            workflow_instance_id: None,
            entry_type: "finding".to_string(),
            content: "Found a bug".to_string(),
        }).unwrap();

        let entries = LoomEntry::list_by_team(&conn, &team.id, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_type, "finding");
    }

    #[test]
    fn test_no_delete() {
        // LoomEntry is append-only, no delete method exists
        // This test verifies the struct works without delete
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let entry = LoomEntry::create(&conn, &CreateLoomEntry {
            agent_id: None,
            team_id: team.id.clone(),
            project_id: project.id.clone(),
            workflow_instance_id: None,
            entry_type: "status".to_string(),
            content: "Append only".to_string(),
        }).unwrap();

        // Entry should always be retrievable
        let fetched = LoomEntry::get_by_id(&conn, &entry.id).unwrap();
        assert_eq!(fetched.content, "Append only");
    }
}
