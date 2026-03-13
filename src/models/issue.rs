use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub project_id: String,
    #[serde(rename = "type")]
    pub issue_type: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i64,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<String>,
    pub depends_on: String,
    pub summary: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub stage_id: Option<String>,
    pub role: Option<String>,
    pub parent_id: Option<String>,
    pub needs_intake: i64,
    pub scope_mode: String,
    pub retry_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateIssue {
    pub project_id: String,
    pub issue_type: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i64>,
    pub depends_on: Option<Vec<String>>,
    pub workflow_instance_id: Option<String>,
    pub stage_id: Option<String>,
    pub role: Option<String>,
    pub parent_id: Option<String>,
    pub needs_intake: Option<i64>,
    pub scope_mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateIssue {
    pub status: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub priority: Option<i64>,
    pub role: Option<String>,
    pub needs_intake: Option<i64>,
    pub scope_mode: Option<String>,
}

impl Issue {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            issue_type: row.get("type")?,
            title: row.get("title")?,
            description: row.get("description")?,
            status: row.get("status")?,
            priority: row.get("priority")?,
            claimed_by: row.get("claimed_by")?,
            claimed_at: row.get("claimed_at")?,
            depends_on: row.get("depends_on")?,
            summary: row.get("summary")?,
            workflow_instance_id: row.get("workflow_instance_id")?,
            stage_id: row.get("stage_id")?,
            role: row.get("role")?,
            parent_id: row.get("parent_id")?,
            needs_intake: row.get("needs_intake")?,
            scope_mode: row.get("scope_mode")?,
            retry_count: row.get("retry_count").unwrap_or(0),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateIssue) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let issue_type = input.issue_type.as_deref().unwrap_or("task");
        let description = input.description.as_deref().unwrap_or("");
        let priority = input.priority.unwrap_or(5);
        let depends_on = match &input.depends_on {
            Some(deps) => serde_json::to_string(deps)?,
            None => "[]".to_string(),
        };
        conn.execute(
            "INSERT INTO issues (id, project_id, type, title, description, priority, depends_on, workflow_instance_id, stage_id, role, parent_id, needs_intake, scope_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id, input.project_id, issue_type, input.title, description, priority, depends_on,
                input.workflow_instance_id, input.stage_id, input.role,
                input.parent_id,
                input.needs_intake.unwrap_or(1),
                input.scope_mode.as_deref().unwrap_or("auto"),
            ],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM issues WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("issue {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM issues ORDER BY priority, created_at")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut issues = Vec::new();
        for row in rows {
            issues.push(row?);
        }
        Ok(issues)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM issues WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("issue {}", id)));
        }
        Ok(())
    }

    pub fn claim(conn: &Connection, issue_id: &str, agent_session_id: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE issues SET claimed_by = ?1, claimed_at = datetime('now'), status = 'in_progress', updated_at = datetime('now')
             WHERE id = ?2",
            params![agent_session_id, issue_id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("issue {}", issue_id)));
        }
        Self::get_by_id(conn, issue_id)
    }

    pub fn unclaim(conn: &Connection, issue_id: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE issues SET claimed_by = NULL, claimed_at = NULL, status = 'open',
             retry_count = COALESCE(retry_count, 0) + 1, updated_at = datetime('now')
             WHERE id = ?1",
            params![issue_id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("issue {}", issue_id)));
        }
        Self::get_by_id(conn, issue_id)
    }

    /// Returns unclaimed, unblocked issues for a project.
    /// An issue is unblocked if all issues in its depends_on list have status 'closed'.
    pub fn get_ready(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        // Get all open, unclaimed issues for the project that have been through intake
        let mut stmt = conn.prepare(
            "SELECT * FROM issues WHERE project_id = ?1 AND status = 'open' AND claimed_by IS NULL AND needs_intake = 0 ORDER BY priority, created_at"
        )?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut candidates = Vec::new();
        for row in rows {
            candidates.push(row?);
        }

        // Filter out those with unresolved dependencies or that are parent issues with children
        let mut ready = Vec::new();
        for issue in candidates {
            // Skip parent issues that have children
            let children = Self::get_children(conn, &issue.id)?;
            if !children.is_empty() {
                continue;
            }

            let deps: Vec<String> = serde_json::from_str(&issue.depends_on).unwrap_or_default();
            if deps.is_empty() {
                ready.push(issue);
            } else {
                let all_closed = deps.iter().all(|dep_id| {
                    match Self::get_by_id(conn, dep_id) {
                        Ok(dep) => dep.status == "closed",
                        Err(_) => false,
                    }
                });
                if all_closed {
                    ready.push(issue);
                }
            }
        }
        Ok(ready)
    }

    /// Returns unclaimed, unblocked issues for a project matching a specific role and issue types.
    pub fn get_ready_by_role(
        conn: &Connection,
        project_id: &str,
        role: &str,
        issue_types: &[&str],
    ) -> Result<Vec<Self>> {
        if issue_types.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = (0..issue_types.len()).map(|i| format!("?{}", i + 3)).collect();
        let sql = format!(
            "SELECT * FROM issues WHERE project_id = ?1 AND REPLACE(LOWER(role), '_', ' ') = REPLACE(LOWER(?2), '_', ' ') AND status = 'open' \
             AND claimed_by IS NULL AND needs_intake = 0 AND type IN ({}) ORDER BY priority, created_at",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        params.push(Box::new(project_id.to_string()));
        params.push(Box::new(role.to_string()));
        for t in issue_types {
            params.push(Box::new(t.to_string()));
        }
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), Self::from_row)?;
        let mut candidates = Vec::new();
        for row in rows {
            candidates.push(row?);
        }

        // Filter out unresolved dependencies or parent issues with children
        let mut ready = Vec::new();
        for issue in candidates {
            // Skip parent issues that have children
            let children = Self::get_children(conn, &issue.id)?;
            if !children.is_empty() {
                continue;
            }

            let deps: Vec<String> = serde_json::from_str(&issue.depends_on).unwrap_or_default();
            if deps.is_empty() {
                ready.push(issue);
            } else {
                let all_closed = deps.iter().all(|dep_id| {
                    match Self::get_by_id(conn, dep_id) {
                        Ok(dep) => dep.status == "closed",
                        Err(_) => false,
                    }
                });
                if all_closed {
                    ready.push(issue);
                }
            }
        }
        Ok(ready)
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateIssue) -> Result<Self> {
        let mut sets = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref status) = input.status {
            sets.push("status = ?");
            values.push(Box::new(status.clone()));
        }
        if let Some(ref title) = input.title {
            sets.push("title = ?");
            values.push(Box::new(title.clone()));
        }
        if let Some(ref description) = input.description {
            sets.push("description = ?");
            values.push(Box::new(description.clone()));
        }
        if let Some(ref summary) = input.summary {
            sets.push("summary = ?");
            values.push(Box::new(summary.clone()));
        }
        if let Some(priority) = input.priority {
            sets.push("priority = ?");
            values.push(Box::new(priority));
        }
        if let Some(ref role) = input.role {
            sets.push("role = ?");
            values.push(Box::new(role.clone()));
        }
        if let Some(needs_intake) = input.needs_intake {
            sets.push("needs_intake = ?");
            values.push(Box::new(needs_intake));
        }
        if let Some(ref scope_mode) = input.scope_mode {
            sets.push("scope_mode = ?");
            values.push(Box::new(scope_mode.clone()));
        }

        if sets.is_empty() {
            return Self::get_by_id(conn, id);
        }

        sets.push("updated_at = datetime('now')");

        let sql = format!("UPDATE issues SET {} WHERE id = ?", sets.join(", "));
        values.push(Box::new(id.to_string()));

        let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        let changes = conn.execute(&sql, params.as_slice())?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("issue {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn get_children(conn: &Connection, parent_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM issues WHERE parent_id = ?1 ORDER BY priority, created_at"
        )?;
        let rows = stmt.query_map(params![parent_id], Self::from_row)?;
        let mut children = Vec::new();
        for row in rows {
            children.push(row?);
        }
        Ok(children)
    }

    pub fn get_needs_intake(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM issues WHERE project_id = ?1 AND needs_intake = 1 AND parent_id IS NULL AND status = 'open' ORDER BY priority, created_at"
        )?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut issues = Vec::new();
        for row in rows {
            issues.push(row?);
        }
        Ok(issues)
    }

    pub fn all_children_closed(conn: &Connection, parent_id: &str) -> Result<Option<bool>> {
        let children = Self::get_children(conn, parent_id)?;
        if children.is_empty() {
            return Ok(None);
        }
        Ok(Some(children.iter().all(|c| c.status == "closed")))
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

    fn create_test_project(conn: &Connection) -> Project {
        Project::create(conn, &CreateProject {
            name: "Test".to_string(),
            directory: "/tmp/test".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        }).unwrap()
    }

    fn create_test_agent_session(conn: &Connection, project: &Project) -> AgentSession {
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
        AgentSession::create(conn, &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: None,
            worktree_path: None,
            branch: None,
        }).unwrap()
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let issue = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: Some("bug".to_string()),
            title: "Fix login".to_string(),
            description: Some("Login is broken".to_string()),
            priority: Some(1),
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        assert_eq!(issue.title, "Fix login");
        assert_eq!(issue.issue_type, "bug");
        assert_eq!(issue.priority, 1);
        assert_eq!(issue.status, "open");

        let fetched = Issue::get_by_id(&conn, &issue.id).unwrap();
        assert_eq!(fetched.id, issue.id);
    }

    #[test]
    fn test_list() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Task 1".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Task 2".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        let issues = Issue::list(&conn).unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let issue = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Delete me".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        Issue::delete(&conn, &issue.id).unwrap();
        assert!(Issue::get_by_id(&conn, &issue.id).is_err());
    }

    #[test]
    fn test_claim_and_unclaim() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let agent = create_test_agent_session(&conn, &project);
        let issue = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Claim me".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        let claimed = Issue::claim(&conn, &issue.id, &agent.id).unwrap();
        assert_eq!(claimed.status, "in_progress");
        assert_eq!(claimed.claimed_by.as_deref(), Some(agent.id.as_str()));
        assert!(claimed.claimed_at.is_some());

        let unclaimed = Issue::unclaim(&conn, &issue.id).unwrap();
        assert_eq!(unclaimed.status, "open");
        assert!(unclaimed.claimed_by.is_none());
        assert!(unclaimed.claimed_at.is_none());
    }

    #[test]
    fn test_get_ready() {
        let conn = setup_db();
        let project = create_test_project(&conn);

        // Create a dependency issue and close it
        let dep = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Dependency".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        conn.execute("UPDATE issues SET status = 'closed' WHERE id = ?1", params![dep.id]).unwrap();

        // Create an issue that depends on the closed one (should be ready)
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Ready issue".to_string(),
            description: None,
            priority: None,
            depends_on: Some(vec![dep.id.clone()]),
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        // Create an issue with an unresolved dependency (should NOT be ready)
        let open_dep = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Open dep".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Blocked issue".to_string(),
            description: None,
            priority: None,
            depends_on: Some(vec![open_dep.id.clone()]),
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        // Create a simple issue with no deps (should be ready)
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Simple issue".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        let ready = Issue::get_ready(&conn, &project.id).unwrap();
        let titles: Vec<&str> = ready.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"Ready issue"));
        assert!(titles.contains(&"Simple issue"));
        assert!(titles.contains(&"Open dep"));
        assert!(!titles.contains(&"Blocked issue"));
    }

    #[test]
    fn test_update_issue() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let issue = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Original title".to_string(),
            description: Some("Original desc".to_string()),
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        // Force a different updated_at by backdating the original record
        conn.execute(
            "UPDATE issues SET updated_at = datetime('now', '-1 minute') WHERE id = ?1",
            params![issue.id],
        ).unwrap();
        let issue = Issue::get_by_id(&conn, &issue.id).unwrap();

        let updated = Issue::update(&conn, &issue.id, &UpdateIssue {
            status: Some("closed".to_string()),
            title: None,
            description: None,
            summary: Some("Work complete".to_string()),
            priority: None,
            role: None,
            needs_intake: None,
            scope_mode: None,
        }).unwrap();

        assert_eq!(updated.status, "closed");
        assert_eq!(updated.summary.as_deref(), Some("Work complete"));
        assert_eq!(updated.title, "Original title");
        assert!(updated.updated_at != issue.updated_at);
    }

    #[test]
    fn test_update_issue_not_found() {
        let conn = setup_db();
        let result = Issue::update(&conn, "nonexistent", &UpdateIssue {
            status: Some("closed".to_string()),
            title: None,
            description: None,
            summary: None,
            priority: None,
            role: None,
            needs_intake: None,
            scope_mode: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_create_issue_with_role() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let issue = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: Some("task".to_string()),
            title: "Implement auth".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: Some("architect".to_string()),
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        assert_eq!(issue.role.as_deref(), Some("architect"));

        let issue2 = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "No role".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        assert!(issue2.role.is_none());
    }

    #[test]
    fn test_update_issue_role() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let issue = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Update role".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();
        assert!(issue.role.is_none());

        let updated = Issue::update(&conn, &issue.id, &UpdateIssue {
            status: None,
            title: None,
            description: None,
            summary: None,
            priority: None,
            role: Some("senior_coder".to_string()),
            needs_intake: None,
            scope_mode: None,
        }).unwrap();
        assert_eq!(updated.role.as_deref(), Some("senior_coder"));
    }

    #[test]
    fn test_get_ready_by_role() {
        let conn = setup_db();
        let project = create_test_project(&conn);

        // Create issues with different roles
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: Some("task".to_string()),
            title: "Architect work".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: Some("architect".to_string()),
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: Some("bug".to_string()),
            title: "Coder work".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: Some("senior_coder".to_string()),
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: Some("feature".to_string()),
            title: "No role work".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        // Filter by role and types
        let ready = Issue::get_ready_by_role(
            &conn,
            &project.id,
            "architect",
            &["task", "bug", "feature"],
        ).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].title, "Architect work");

        // Filter by types
        let ready = Issue::get_ready_by_role(
            &conn,
            &project.id,
            "senior_coder",
            &["task"],  // only tasks, not bugs
        ).unwrap();
        assert_eq!(ready.len(), 0);  // the coder issue is a bug, not a task
    }

    #[test]
    fn test_get_children() {
        let conn = setup_db();
        let project = create_test_project(&conn);

        let parent = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Parent".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Child 1".to_string(),
            description: None,
            priority: Some(1),
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: Some(parent.id.clone()),
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Child 2".to_string(),
            description: None,
            priority: Some(2),
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: Some(parent.id.clone()),
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        let children = Issue::get_children(&conn, &parent.id).unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].title, "Child 1");
        assert_eq!(children[1].title, "Child 2");

        // Parent issues should not appear in get_ready
        let ready = Issue::get_ready(&conn, &project.id).unwrap();
        let titles: Vec<&str> = ready.iter().map(|i| i.title.as_str()).collect();
        assert!(!titles.contains(&"Parent"));
        assert!(titles.contains(&"Child 1"));
        assert!(titles.contains(&"Child 2"));
    }

    #[test]
    fn test_get_needs_intake() {
        let conn = setup_db();
        let project = create_test_project(&conn);

        // Issue that needs intake (default)
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Needs intake".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: None,  // defaults to 1
            scope_mode: None,
        }).unwrap();

        // Issue that does NOT need intake
        Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Already processed".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        let needs = Issue::get_needs_intake(&conn, &project.id).unwrap();
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0].title, "Needs intake");
    }

    #[test]
    fn test_all_children_closed() {
        let conn = setup_db();
        let project = create_test_project(&conn);

        let parent = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Parent".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        // No children => None
        assert_eq!(Issue::all_children_closed(&conn, &parent.id).unwrap(), None);

        let child1 = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Child 1".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: Some(parent.id.clone()),
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        let child2 = Issue::create(&conn, &CreateIssue {
            project_id: project.id.clone(),
            issue_type: None,
            title: "Child 2".to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: None,
            parent_id: Some(parent.id.clone()),
            needs_intake: Some(0),
            scope_mode: None,
        }).unwrap();

        // Not all closed
        assert_eq!(Issue::all_children_closed(&conn, &parent.id).unwrap(), Some(false));

        // Close both
        conn.execute("UPDATE issues SET status = 'closed' WHERE id = ?1", params![child1.id]).unwrap();
        conn.execute("UPDATE issues SET status = 'closed' WHERE id = ?1", params![child2.id]).unwrap();

        assert_eq!(Issue::all_children_closed(&conn, &parent.id).unwrap(), Some(true));
    }
}
