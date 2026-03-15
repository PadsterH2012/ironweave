use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub template_type: String,
    pub content: String,
    pub project_id: Option<String>,
    pub is_system: bool,
    pub coordination_mode: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePromptTemplate {
    pub name: String,
    pub template_type: Option<String>,
    pub content: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePromptTemplate {
    pub name: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplateAssignment {
    pub id: String,
    pub role: String,
    pub template_id: String,
    pub priority: i64,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAssignment {
    pub role: String,
    pub template_id: String,
    pub priority: Option<i64>,
}

impl PromptTemplate {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            template_type: row.get("template_type")?,
            content: row.get("content")?,
            project_id: row.get("project_id")?,
            is_system: row.get::<_, i32>("is_system").unwrap_or(0) != 0,
            coordination_mode: row.get("coordination_mode").ok().unwrap_or(None),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreatePromptTemplate) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let ttype = input.template_type.as_deref().unwrap_or("role");
        conn.execute(
            "INSERT INTO prompt_templates (id, name, template_type, content, project_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, input.name, ttype, input.content, input.project_id],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM prompt_templates WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("prompt_template {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection, project_id: Option<&str>) -> Result<Vec<Self>> {
        let mut entries = Vec::new();
        if let Some(pid) = project_id {
            // Return global + project-specific templates
            let mut stmt = conn.prepare(
                "SELECT * FROM prompt_templates WHERE project_id IS NULL OR project_id = ?1 ORDER BY template_type, name"
            )?;
            let rows = stmt.query_map(params![pid], Self::from_row)?;
            for row in rows { entries.push(row?); }
        } else {
            let mut stmt = conn.prepare(
                "SELECT * FROM prompt_templates ORDER BY template_type, name"
            )?;
            let rows = stmt.query_map([], Self::from_row)?;
            for row in rows { entries.push(row?); }
        }
        Ok(entries)
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdatePromptTemplate) -> Result<Self> {
        let existing = Self::get_by_id(conn, id)?;
        if existing.is_system {
            return Err(IronweaveError::Validation("Cannot modify system templates".into()));
        }
        let name = input.name.as_deref().unwrap_or(&existing.name);
        let content = input.content.as_deref().unwrap_or(&existing.content);
        conn.execute(
            "UPDATE prompt_templates SET name = ?1, content = ?2, updated_at = datetime('now') WHERE id = ?3",
            params![name, content, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let existing = Self::get_by_id(conn, id)?;
        if existing.is_system {
            return Err(IronweaveError::Validation("Cannot delete system templates".into()));
        }
        let affected = conn.execute("DELETE FROM prompt_templates WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(IronweaveError::NotFound(format!("prompt_template {}", id)));
        }
        Ok(())
    }

    /// Build the full prompt for a role: role template + system coordination skill + assigned skill templates.
    /// Checks project-specific first, falls back to global.
    pub fn build_prompt_for_role(conn: &Connection, role: &str, project_id: Option<&str>) -> Result<String> {
        Self::build_prompt_for_role_with_mode(conn, role, project_id, None)
    }

    /// Build prompt with coordination mode — injects the system skill for the team's coordination mode.
    pub fn build_prompt_for_role_with_mode(
        conn: &Connection,
        role: &str,
        project_id: Option<&str>,
        coordination_mode: Option<&str>,
    ) -> Result<String> {
        let mut parts: Vec<String> = Vec::new();

        // 1. Look for role template (project-specific first, then global)
        let role_template = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT * FROM prompt_templates WHERE template_type = 'role' AND name = ?1 AND project_id = ?2",
                params![role, pid],
                Self::from_row,
            ).ok()
        } else {
            None
        }.or_else(|| {
            conn.query_row(
                "SELECT * FROM prompt_templates WHERE template_type = 'role' AND name = ?1 AND project_id IS NULL",
                params![role],
                Self::from_row,
            ).ok()
        });

        if let Some(tmpl) = role_template {
            parts.push(tmpl.content);
        }

        // 2. Inject system coordination skill if coordination_mode is set
        if let Some(mode) = coordination_mode {
            if let Ok(sys_skill) = conn.query_row(
                "SELECT content FROM prompt_templates WHERE is_system = 1 AND coordination_mode = ?1",
                params![mode],
                |row| row.get::<_, String>(0),
            ) {
                parts.push(sys_skill);
            }
        }

        // 3. Get assigned skill templates in priority order
        let mut stmt = conn.prepare(
            "SELECT pt.content FROM prompt_template_assignments pta
             JOIN prompt_templates pt ON pta.template_id = pt.id
             WHERE pta.role = ?1 AND (pt.project_id IS NULL OR pt.project_id = ?2)
             ORDER BY pta.priority ASC"
        )?;
        let rows = stmt.query_map(params![role, project_id.unwrap_or("")], |row| {
            row.get::<_, String>(0)
        })?;
        for row in rows {
            parts.push(row?);
        }

        Ok(parts.join("\n\n---\n\n"))
    }
}

impl PromptTemplateAssignment {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            role: row.get("role")?,
            template_id: row.get("template_id")?,
            priority: row.get("priority")?,
            created_at: row.get("created_at")?,
            template_name: row.get("template_name").ok(),
            template_type: row.get("template_type").ok(),
        })
    }

    pub fn create(conn: &Connection, input: &CreateAssignment) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let priority = input.priority.unwrap_or(0);
        conn.execute(
            "INSERT INTO prompt_template_assignments (id, role, template_id, priority)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, input.role, input.template_id, priority],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT pta.*, pt.name as template_name, pt.template_type
             FROM prompt_template_assignments pta
             JOIN prompt_templates pt ON pta.template_id = pt.id
             WHERE pta.id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("assignment {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_role(conn: &Connection, role: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT pta.*, pt.name as template_name, pt.template_type
             FROM prompt_template_assignments pta
             JOIN prompt_templates pt ON pta.template_id = pt.id
             WHERE pta.role = ?1 ORDER BY pta.priority ASC"
        )?;
        let rows = stmt.query_map(params![role], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows { entries.push(row?); }
        Ok(entries)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let affected = conn.execute("DELETE FROM prompt_template_assignments WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(IronweaveError::NotFound(format!("assignment {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::project::{CreateProject, Project};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_create_role_template() {
        let conn = setup_db();
        let tmpl = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Senior Coder".to_string(),
            template_type: Some("role".to_string()),
            content: "You are a senior software engineer.".to_string(),
            project_id: None,
        }).unwrap();
        assert_eq!(tmpl.name, "Senior Coder");
        assert_eq!(tmpl.template_type, "role");
    }

    #[test]
    fn test_create_skill_template() {
        let conn = setup_db();
        let tmpl = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Rust Patterns".to_string(),
            template_type: Some("skill".to_string()),
            content: "Follow idiomatic Rust patterns. Use Result for errors.".to_string(),
            project_id: None,
        }).unwrap();
        assert_eq!(tmpl.template_type, "skill");
    }

    #[test]
    fn test_update_template() {
        let conn = setup_db();
        let tmpl = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Test".to_string(),
            template_type: None,
            content: "Original".to_string(),
            project_id: None,
        }).unwrap();

        let updated = PromptTemplate::update(&conn, &tmpl.id, &UpdatePromptTemplate {
            name: None,
            content: Some("Updated content".to_string()),
        }).unwrap();
        assert_eq!(updated.content, "Updated content");
        assert_eq!(updated.name, "Test");
    }

    #[test]
    fn test_delete_template() {
        let conn = setup_db();
        let tmpl = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Temp".to_string(),
            template_type: None,
            content: "Delete me".to_string(),
            project_id: None,
        }).unwrap();
        PromptTemplate::delete(&conn, &tmpl.id).unwrap();
        assert!(PromptTemplate::get_by_id(&conn, &tmpl.id).is_err());
    }

    #[test]
    fn test_assignment_crud() {
        let conn = setup_db();
        let skill = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Git Workflow".to_string(),
            template_type: Some("skill".to_string()),
            content: "Always use feature branches.".to_string(),
            project_id: None,
        }).unwrap();

        let assignment = PromptTemplateAssignment::create(&conn, &CreateAssignment {
            role: "Senior Coder".to_string(),
            template_id: skill.id.clone(),
            priority: Some(1),
        }).unwrap();
        assert_eq!(assignment.role, "Senior Coder");
        assert_eq!(assignment.template_name.as_deref(), Some("Git Workflow"));

        let list = PromptTemplateAssignment::list_by_role(&conn, "Senior Coder").unwrap();
        assert_eq!(list.len(), 1);

        PromptTemplateAssignment::delete(&conn, &assignment.id).unwrap();
        let list = PromptTemplateAssignment::list_by_role(&conn, "Senior Coder").unwrap();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_cascade_delete() {
        let conn = setup_db();
        let skill = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Temp Skill".to_string(),
            template_type: Some("skill".to_string()),
            content: "Temp".to_string(),
            project_id: None,
        }).unwrap();
        PromptTemplateAssignment::create(&conn, &CreateAssignment {
            role: "Tester".to_string(),
            template_id: skill.id.clone(),
            priority: None,
        }).unwrap();

        // Deleting template should cascade to assignments
        PromptTemplate::delete(&conn, &skill.id).unwrap();
        let list = PromptTemplateAssignment::list_by_role(&conn, "Tester").unwrap();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_build_prompt_for_role() {
        let conn = setup_db();

        // Create a role template
        PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Senior Coder".to_string(),
            template_type: Some("role".to_string()),
            content: "You are a senior coder.".to_string(),
            project_id: None,
        }).unwrap();

        // Create two skill templates and assign them
        let s1 = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Rust".to_string(),
            template_type: Some("skill".to_string()),
            content: "Use idiomatic Rust.".to_string(),
            project_id: None,
        }).unwrap();
        let s2 = PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Testing".to_string(),
            template_type: Some("skill".to_string()),
            content: "Write tests first.".to_string(),
            project_id: None,
        }).unwrap();

        PromptTemplateAssignment::create(&conn, &CreateAssignment {
            role: "Senior Coder".to_string(),
            template_id: s1.id,
            priority: Some(1),
        }).unwrap();
        PromptTemplateAssignment::create(&conn, &CreateAssignment {
            role: "Senior Coder".to_string(),
            template_id: s2.id,
            priority: Some(2),
        }).unwrap();

        let prompt = PromptTemplate::build_prompt_for_role(&conn, "Senior Coder", None).unwrap();
        assert!(prompt.contains("You are a senior coder."));
        assert!(prompt.contains("Use idiomatic Rust."));
        assert!(prompt.contains("Write tests first."));
        // Rust should come before Testing (priority order)
        let rust_pos = prompt.find("Use idiomatic Rust.").unwrap();
        let test_pos = prompt.find("Write tests first.").unwrap();
        assert!(rust_pos < test_pos);
    }

    #[test]
    fn test_coordination_mode_skill_injection() {
        let conn = setup_db();

        // Create a role template
        PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Senior Coder".to_string(),
            template_type: Some("role".to_string()),
            content: "You are a senior coder.".to_string(),
            project_id: None,
        }).unwrap();

        // Build without mode — should not include coordination skill
        let prompt = PromptTemplate::build_prompt_for_role_with_mode(&conn, "Senior Coder", None, None).unwrap();
        assert!(!prompt.contains("pipeline"));

        // Build with pipeline mode — should include system skill
        let prompt = PromptTemplate::build_prompt_for_role_with_mode(&conn, "Senior Coder", None, Some("pipeline")).unwrap();
        assert!(prompt.contains("pipeline"));
        assert!(prompt.contains("You are a senior coder."));

        // Build with swarm mode — should include swarm skill
        let prompt = PromptTemplate::build_prompt_for_role_with_mode(&conn, "Senior Coder", None, Some("swarm")).unwrap();
        assert!(prompt.contains("swarm"));
    }

    #[test]
    fn test_project_specific_overrides_global() {
        let conn = setup_db();
        let project = Project::create(&conn, &CreateProject {
            name: "Test Project".to_string(),
            directory: "/tmp/test".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        }).unwrap();

        // Global role template
        PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Senior Coder".to_string(),
            template_type: Some("role".to_string()),
            content: "Global senior coder prompt.".to_string(),
            project_id: None,
        }).unwrap();

        // Project-specific role template
        PromptTemplate::create(&conn, &CreatePromptTemplate {
            name: "Senior Coder".to_string(),
            template_type: Some("role".to_string()),
            content: "Project-specific senior coder prompt.".to_string(),
            project_id: Some(project.id.clone()),
        }).unwrap();

        let prompt = PromptTemplate::build_prompt_for_role(&conn, "Senior Coder", Some(&project.id)).unwrap();
        assert!(prompt.contains("Project-specific"));
        assert!(!prompt.contains("Global"));
    }
}
