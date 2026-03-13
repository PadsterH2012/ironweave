use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub team_id: String,
    pub dag: String,
    pub version: i64,
    pub git_sha: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkflowDefinition {
    pub name: String,
    pub project_id: String,
    pub team_id: String,
    pub dag: Option<String>,
    pub version: Option<i64>,
    pub git_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    pub id: String,
    pub definition_id: String,
    pub state: String,
    pub current_stage: Option<String>,
    pub checkpoint: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkflowInstance {
    pub definition_id: String,
    pub current_stage: Option<String>,
}

impl WorkflowDefinition {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            project_id: row.get("project_id")?,
            team_id: row.get("team_id")?,
            dag: row.get("dag")?,
            version: row.get("version")?,
            git_sha: row.get("git_sha")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateWorkflowDefinition) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let dag = input.dag.as_deref().unwrap_or("{}");
        let version = input.version.unwrap_or(1);
        conn.execute(
            "INSERT INTO workflow_definitions (id, name, project_id, team_id, dag, version, git_sha)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, input.name, input.project_id, input.team_id, dag, version, input.git_sha],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM workflow_definitions WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("workflow_definition {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM workflow_definitions WHERE project_id = ?1 ORDER BY name")?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut defs = Vec::new();
        for row in rows {
            defs.push(row?);
        }
        Ok(defs)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM workflow_definitions WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("workflow_definition {}", id)));
        }
        Ok(())
    }
}

impl WorkflowInstance {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            definition_id: row.get("definition_id")?,
            state: row.get("state")?,
            current_stage: row.get("current_stage")?,
            checkpoint: row.get("checkpoint")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            total_tokens: row.get("total_tokens")?,
            total_cost: row.get("total_cost")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateWorkflowInstance) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO workflow_instances (id, definition_id, current_stage)
             VALUES (?1, ?2, ?3)",
            params![id, input.definition_id, input.current_stage],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM workflow_instances WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("workflow_instance {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_definition(conn: &Connection, definition_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM workflow_instances WHERE definition_id = ?1 ORDER BY created_at")?;
        let rows = stmt.query_map(params![definition_id], Self::from_row)?;
        let mut instances = Vec::new();
        for row in rows {
            instances.push(row?);
        }
        Ok(instances)
    }

    pub fn update_state(conn: &Connection, id: &str, new_state: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE workflow_instances SET state = ?1 WHERE id = ?2",
            params![new_state, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("workflow_instance {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM workflow_instances WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("workflow_instance {}", id)));
        }
        Ok(())
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
    fn test_definition_create_and_get() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let def = WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Build pipeline".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();
        assert_eq!(def.name, "Build pipeline");
        assert_eq!(def.version, 1);

        let fetched = WorkflowDefinition::get_by_id(&conn, &def.id).unwrap();
        assert_eq!(fetched.id, def.id);
    }

    #[test]
    fn test_definition_list_by_project() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Alpha".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();
        WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Beta".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();

        let defs = WorkflowDefinition::list_by_project(&conn, &project.id).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "Alpha");
    }

    #[test]
    fn test_definition_delete() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let def = WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Temp".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();
        WorkflowDefinition::delete(&conn, &def.id).unwrap();
        assert!(WorkflowDefinition::get_by_id(&conn, &def.id).is_err());
    }

    #[test]
    fn test_instance_create_and_get() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let def = WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Pipeline".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();
        let instance = WorkflowInstance::create(&conn, &CreateWorkflowInstance {
            definition_id: def.id.clone(),
            current_stage: Some("build".to_string()),
        }).unwrap();
        assert_eq!(instance.state, "pending");
        assert_eq!(instance.current_stage.as_deref(), Some("build"));

        let fetched = WorkflowInstance::get_by_id(&conn, &instance.id).unwrap();
        assert_eq!(fetched.id, instance.id);
    }

    #[test]
    fn test_instance_list_by_definition() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let def = WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Pipeline".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();
        WorkflowInstance::create(&conn, &CreateWorkflowInstance {
            definition_id: def.id.clone(),
            current_stage: None,
        }).unwrap();

        let instances = WorkflowInstance::list_by_definition(&conn, &def.id).unwrap();
        assert_eq!(instances.len(), 1);
    }

    #[test]
    fn test_instance_update_state() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let def = WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Pipeline".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();
        let instance = WorkflowInstance::create(&conn, &CreateWorkflowInstance {
            definition_id: def.id.clone(),
            current_stage: None,
        }).unwrap();

        let updated = WorkflowInstance::update_state(&conn, &instance.id, "running").unwrap();
        assert_eq!(updated.state, "running");
    }

    #[test]
    fn test_instance_delete() {
        let conn = setup_db();
        let (project, team) = create_prereqs(&conn);
        let def = WorkflowDefinition::create(&conn, &CreateWorkflowDefinition {
            name: "Pipeline".to_string(),
            project_id: project.id.clone(),
            team_id: team.id.clone(),
            dag: None,
            version: None,
            git_sha: None,
        }).unwrap();
        let instance = WorkflowInstance::create(&conn, &CreateWorkflowInstance {
            definition_id: def.id.clone(),
            current_stage: None,
        }).unwrap();
        WorkflowInstance::delete(&conn, &instance.id).unwrap();
        assert!(WorkflowInstance::get_by_id(&conn, &instance.id).is_err());
    }
}
