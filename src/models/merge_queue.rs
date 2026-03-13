use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeQueueEntry {
    pub id: String,
    pub project_id: String,
    pub agent_session_id: String,
    pub branch: String,
    pub worktree_path: String,
    pub target_branch: String,
    pub status: String,
    pub conflict_tier: Option<i64>,
    pub queued_at: String,
    pub merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMergeQueueEntry {
    pub project_id: String,
    pub agent_session_id: String,
    pub branch: String,
    pub worktree_path: String,
    pub target_branch: Option<String>,
}

impl MergeQueueEntry {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            agent_session_id: row.get("agent_session_id")?,
            branch: row.get("branch")?,
            worktree_path: row.get("worktree_path")?,
            target_branch: row.get("target_branch")?,
            status: row.get("status")?,
            conflict_tier: row.get("conflict_tier")?,
            queued_at: row.get("queued_at")?,
            merged_at: row.get("merged_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateMergeQueueEntry) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let target_branch = input.target_branch.as_deref().unwrap_or("main");
        conn.execute(
            "INSERT INTO merge_queue_entries (id, project_id, agent_session_id, branch, worktree_path, target_branch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, input.project_id, input.agent_session_id, input.branch, input.worktree_path, target_branch],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM merge_queue_entries WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("merge_queue_entry {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM merge_queue_entries WHERE project_id = ?1 ORDER BY queued_at")?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn get_next(conn: &Connection, project_id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM merge_queue_entries WHERE project_id = ?1 AND status = 'queued' ORDER BY queued_at ASC LIMIT 1",
            params![project_id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound("no queued merge entries".to_string()),
            other => IronweaveError::Database(other),
        })
    }

    pub fn update_status(conn: &Connection, id: &str, new_status: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE merge_queue_entries SET status = ?1 WHERE id = ?2",
            params![new_status, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("merge_queue_entry {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM merge_queue_entries WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("merge_queue_entry {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::project::{CreateProject, Project};
    use crate::models::team::{CreateTeam, Team, CreateTeamAgentSlot, TeamAgentSlot};
    use crate::models::agent::{AgentSession, CreateAgentSession};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn create_prereqs(conn: &Connection) -> (Project, AgentSession) {
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
        let session = AgentSession::create(conn, &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: None,
            worktree_path: None,
            branch: None,
        }).unwrap();
        (project, session)
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let (project, session) = create_prereqs(&conn);
        let entry = MergeQueueEntry::create(&conn, &CreateMergeQueueEntry {
            project_id: project.id.clone(),
            agent_session_id: session.id.clone(),
            branch: "feature/login".to_string(),
            worktree_path: "/tmp/wt".to_string(),
            target_branch: None,
        }).unwrap();
        assert_eq!(entry.branch, "feature/login");
        assert_eq!(entry.status, "queued");
        assert_eq!(entry.target_branch, "main");

        let fetched = MergeQueueEntry::get_by_id(&conn, &entry.id).unwrap();
        assert_eq!(fetched.id, entry.id);
    }

    #[test]
    fn test_list_by_project() {
        let conn = setup_db();
        let (project, session) = create_prereqs(&conn);
        MergeQueueEntry::create(&conn, &CreateMergeQueueEntry {
            project_id: project.id.clone(),
            agent_session_id: session.id.clone(),
            branch: "feature/a".to_string(),
            worktree_path: "/tmp/a".to_string(),
            target_branch: None,
        }).unwrap();
        MergeQueueEntry::create(&conn, &CreateMergeQueueEntry {
            project_id: project.id.clone(),
            agent_session_id: session.id.clone(),
            branch: "feature/b".to_string(),
            worktree_path: "/tmp/b".to_string(),
            target_branch: None,
        }).unwrap();

        let entries = MergeQueueEntry::list_by_project(&conn, &project.id).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_get_next() {
        let conn = setup_db();
        let (project, session) = create_prereqs(&conn);
        let first = MergeQueueEntry::create(&conn, &CreateMergeQueueEntry {
            project_id: project.id.clone(),
            agent_session_id: session.id.clone(),
            branch: "feature/first".to_string(),
            worktree_path: "/tmp/first".to_string(),
            target_branch: None,
        }).unwrap();
        MergeQueueEntry::create(&conn, &CreateMergeQueueEntry {
            project_id: project.id.clone(),
            agent_session_id: session.id.clone(),
            branch: "feature/second".to_string(),
            worktree_path: "/tmp/second".to_string(),
            target_branch: None,
        }).unwrap();

        let next = MergeQueueEntry::get_next(&conn, &project.id).unwrap();
        assert_eq!(next.id, first.id);
    }

    #[test]
    fn test_update_status() {
        let conn = setup_db();
        let (project, session) = create_prereqs(&conn);
        let entry = MergeQueueEntry::create(&conn, &CreateMergeQueueEntry {
            project_id: project.id.clone(),
            agent_session_id: session.id.clone(),
            branch: "feature/test".to_string(),
            worktree_path: "/tmp/test".to_string(),
            target_branch: None,
        }).unwrap();

        let updated = MergeQueueEntry::update_status(&conn, &entry.id, "merging").unwrap();
        assert_eq!(updated.status, "merging");

        // After updating to non-queued, get_next should not return it
        let next_result = MergeQueueEntry::get_next(&conn, &project.id);
        assert!(next_result.is_err());
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let (project, session) = create_prereqs(&conn);
        let entry = MergeQueueEntry::create(&conn, &CreateMergeQueueEntry {
            project_id: project.id.clone(),
            agent_session_id: session.id.clone(),
            branch: "feature/del".to_string(),
            worktree_path: "/tmp/del".to_string(),
            target_branch: None,
        }).unwrap();
        MergeQueueEntry::delete(&conn, &entry.id).unwrap();
        assert!(MergeQueueEntry::get_by_id(&conn, &entry.id).is_err());
    }
}
