use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub team_id: String,
    pub slot_id: String,
    pub workflow_instance_id: Option<String>,
    pub runtime: String,
    pub pid: Option<i64>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub state: String,
    pub claimed_task_id: Option<String>,
    pub tokens_used: i64,
    pub cost: f64,
    pub started_at: String,
    pub last_heartbeat: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentSession {
    pub team_id: String,
    pub slot_id: String,
    pub runtime: String,
    pub workflow_instance_id: Option<String>,
    pub pid: Option<i64>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
}

impl AgentSession {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            team_id: row.get("team_id")?,
            slot_id: row.get("slot_id")?,
            workflow_instance_id: row.get("workflow_instance_id")?,
            runtime: row.get("runtime")?,
            pid: row.get("pid")?,
            worktree_path: row.get("worktree_path")?,
            branch: row.get("branch")?,
            state: row.get("state")?,
            claimed_task_id: row.get("claimed_task_id")?,
            tokens_used: row.get("tokens_used")?,
            cost: row.get("cost")?,
            started_at: row.get("started_at")?,
            last_heartbeat: row.get("last_heartbeat")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateAgentSession) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO agent_sessions (id, team_id, slot_id, workflow_instance_id, runtime, pid, worktree_path, branch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, input.team_id, input.slot_id, input.workflow_instance_id, input.runtime, input.pid, input.worktree_path, input.branch],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM agent_sessions WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("agent_session {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_team(conn: &Connection, team_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM agent_sessions WHERE team_id = ?1 ORDER BY started_at")?;
        let rows = stmt.query_map(params![team_id], Self::from_row)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    pub fn update_state(conn: &Connection, id: &str, new_state: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE agent_sessions SET state = ?1 WHERE id = ?2",
            params![new_state, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("agent_session {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn update_heartbeat(conn: &Connection, id: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE agent_sessions SET last_heartbeat = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("agent_session {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM agent_sessions WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("agent_session {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::project::{CreateProject, Project};
    use crate::models::team::{CreateTeam, Team, CreateTeamAgentSlot, TeamAgentSlot};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn create_prereqs(conn: &Connection) -> (Project, Team, TeamAgentSlot) {
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
        let slot = TeamAgentSlot::create(conn, &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "coder".to_string(),
            runtime: "claude".to_string(),
            model: None,
            config: None,
            slot_order: None,
            is_lead: None,
        }).unwrap();
        (project, team, slot)
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let (_project, team, slot) = create_prereqs(&conn);
        let session = AgentSession::create(&conn, &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: Some(1234),
            worktree_path: None,
            branch: None,
        }).unwrap();
        assert_eq!(session.runtime, "claude");
        assert_eq!(session.state, "idle");
        assert_eq!(session.pid, Some(1234));

        let fetched = AgentSession::get_by_id(&conn, &session.id).unwrap();
        assert_eq!(fetched.id, session.id);
    }

    #[test]
    fn test_list_by_team() {
        let conn = setup_db();
        let (_project, team, slot) = create_prereqs(&conn);
        AgentSession::create(&conn, &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: None,
            worktree_path: None,
            branch: None,
        }).unwrap();

        let sessions = AgentSession::list_by_team(&conn, &team.id).unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_update_state() {
        let conn = setup_db();
        let (_project, team, slot) = create_prereqs(&conn);
        let session = AgentSession::create(&conn, &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: None,
            worktree_path: None,
            branch: None,
        }).unwrap();

        let updated = AgentSession::update_state(&conn, &session.id, "working").unwrap();
        assert_eq!(updated.state, "working");
    }

    #[test]
    fn test_update_heartbeat() {
        let conn = setup_db();
        let (_project, team, slot) = create_prereqs(&conn);
        let session = AgentSession::create(&conn, &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: None,
            worktree_path: None,
            branch: None,
        }).unwrap();

        let updated = AgentSession::update_heartbeat(&conn, &session.id).unwrap();
        assert!(!updated.last_heartbeat.is_empty());
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let (_project, team, slot) = create_prereqs(&conn);
        let session = AgentSession::create(&conn, &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: None,
            worktree_path: None,
            branch: None,
        }).unwrap();
        AgentSession::delete(&conn, &session.id).unwrap();
        assert!(AgentSession::get_by_id(&conn, &session.id).is_err());
    }
}
