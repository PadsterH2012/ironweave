use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub category: String,
    pub default_runtime: String,
    pub default_provider: String,
    pub default_model: Option<String>,
    pub default_skills: String,
    pub min_model_tier: i32,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRole {
    pub name: String,
    pub category: Option<String>,
    pub default_runtime: Option<String>,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_skills: Option<Vec<String>>,
    pub min_model_tier: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRole {
    pub category: Option<String>,
    pub default_runtime: Option<String>,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_skills: Option<Vec<String>>,
    pub min_model_tier: Option<i32>,
    pub description: Option<String>,
}

impl Role {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            name: row.get("name")?,
            category: row.get("category")?,
            default_runtime: row.get("default_runtime")?,
            default_provider: row.get("default_provider")?,
            default_model: row.get("default_model")?,
            default_skills: row.get("default_skills")?,
            min_model_tier: row.get("min_model_tier")?,
            description: row.get("description")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM roles ORDER BY category, name")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut roles = Vec::new();
        for row in rows {
            roles.push(row?);
        }
        Ok(roles)
    }

    pub fn get_by_name(conn: &Connection, name: &str) -> Result<Self> {
        let role = conn.query_row(
            "SELECT * FROM roles WHERE name = ?1",
            params![name],
            Self::from_row,
        )?;
        Ok(role)
    }

    pub fn create(conn: &Connection, input: &CreateRole) -> Result<Self> {
        let skills_json = match &input.default_skills {
            Some(skills) => serde_json::to_string(skills)?,
            None => "[]".to_string(),
        };
        conn.execute(
            "INSERT INTO roles (name, category, default_runtime, default_provider, default_model, default_skills, min_model_tier, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                input.name,
                input.category.as_deref().unwrap_or("General"),
                input.default_runtime.as_deref().unwrap_or("claude"),
                input.default_provider.as_deref().unwrap_or("anthropic"),
                input.default_model,
                skills_json,
                input.min_model_tier.unwrap_or(1),
                input.description.as_deref().unwrap_or(""),
            ],
        )?;
        Self::get_by_name(conn, &input.name)
    }

    pub fn update(conn: &Connection, name: &str, input: &UpdateRole) -> Result<Self> {
        let existing = Self::get_by_name(conn, name)?;

        let category = input.category.as_deref().unwrap_or(&existing.category);
        let runtime = input.default_runtime.as_deref().unwrap_or(&existing.default_runtime);
        let provider = input.default_provider.as_deref().unwrap_or(&existing.default_provider);
        let model = input.default_model.as_deref().or(existing.default_model.as_deref());
        let skills = match &input.default_skills {
            Some(s) => serde_json::to_string(s)?,
            None => existing.default_skills.clone(),
        };
        let tier = input.min_model_tier.unwrap_or(existing.min_model_tier);
        let desc = input.description.as_deref().unwrap_or(&existing.description);

        conn.execute(
            "UPDATE roles SET category = ?1, default_runtime = ?2, default_provider = ?3,
             default_model = ?4, default_skills = ?5, min_model_tier = ?6, description = ?7,
             updated_at = datetime('now')
             WHERE name = ?8",
            params![category, runtime, provider, model, skills, tier, desc, name],
        )?;
        Self::get_by_name(conn, name)
    }

    pub fn delete(conn: &Connection, name: &str) -> Result<()> {
        conn.execute("DELETE FROM roles WHERE name = ?1", params![name])?;
        Ok(())
    }

    /// Seed the 17 predefined roles from the v2 design
    pub fn seed_defaults(conn: &Connection) -> Result<()> {
        let defaults = vec![
            ("Coordinator", "Orchestration", "claude", "anthropic", Some("claude-opus-4-6"), 4, "Persistent agent — decomposes tasks, builds teams, routes models"),
            ("Architect", "Engineering", "claude", "anthropic", Some("claude-opus-4-6"), 3, "System design, interfaces, structural decisions"),
            ("Senior Coder", "Engineering", "claude", "anthropic", Some("claude-sonnet-4-6"), 2, "Feature implementation, bug fixes, production code"),
            ("Code Reviewer", "Engineering", "claude", "anthropic", Some("claude-sonnet-4-6"), 3, "Code review for correctness, security, quality"),
            ("DB Senior Engineer", "Engineering", "claude", "anthropic", Some("claude-sonnet-4-6"), 2, "Schema design, query optimisation, migrations"),
            ("UI/UX Senior Coder", "Engineering", "claude", "anthropic", Some("claude-sonnet-4-6"), 2, "Frontend implementation, component development"),
            ("Senior UX/UI Designer", "Design", "claude", "anthropic", Some("claude-sonnet-4-6"), 2, "User flows, wireframes, visual design"),
            ("Brand Designer", "Design", "opencode", "openrouter", Some("mistral-large"), 2, "Visual identity, brand guidelines"),
            ("Senior Tester", "Quality", "claude", "anthropic", Some("claude-sonnet-4-6"), 2, "Test design and implementation, QA"),
            ("Security Engineer", "Security", "claude", "anthropic", Some("claude-sonnet-4-6"), 3, "Vulnerability audits, hardening, secure coding"),
            ("DevOps Engineer", "Operations", "opencode", "openrouter", Some("deepseek-v3"), 2, "Deployments, CI/CD, system reliability"),
            ("Infrastructure Engineer", "Operations", "opencode", "openrouter", Some("deepseek-v3"), 2, "Server provisioning, networking, monitoring"),
            ("Researcher", "Research", "opencode", "openrouter", Some("claude-opus-4-6"), 2, "Technology evaluation, requirements gathering"),
            ("Documentor", "Documentation", "opencode", "openrouter", Some("deepseek-v3"), 1, "Technical writing, API docs, guides"),
            ("Marketing Manager", "Content", "opencode", "openrouter", Some("mistral-large"), 1, "Product messaging, campaigns, copy"),
            ("Newsletter Writer", "Content", "opencode", "openrouter", Some("mistral-large"), 1, "Newsletter content, subscriber engagement"),
            ("Office Monkey", "General", "opencode", "ollama", Some("llama-3.1-8b"), 1, "Ad-hoc tasks, filing, cleanup, simple transforms"),
        ];

        for (name, cat, runtime, provider, model, tier, desc) in defaults {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO roles (name, category, default_runtime, default_provider, default_model, min_model_tier, description)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![name, cat, runtime, provider, model, tier, desc],
            );
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
    fn test_seed_defaults() {
        let conn = setup_db();
        Role::seed_defaults(&conn).unwrap();

        let roles = Role::list(&conn).unwrap();
        assert_eq!(roles.len(), 17);

        let coordinator = Role::get_by_name(&conn, "Coordinator").unwrap();
        assert_eq!(coordinator.category, "Orchestration");
        assert_eq!(coordinator.min_model_tier, 4);
        assert_eq!(coordinator.default_model, Some("claude-opus-4-6".to_string()));
    }

    #[test]
    fn test_seed_idempotent() {
        let conn = setup_db();
        Role::seed_defaults(&conn).unwrap();
        Role::seed_defaults(&conn).unwrap();
        let roles = Role::list(&conn).unwrap();
        assert_eq!(roles.len(), 17);
    }

    #[test]
    fn test_create_custom_role() {
        let conn = setup_db();
        let role = Role::create(&conn, &CreateRole {
            name: "Custom Role".to_string(),
            category: Some("Custom".to_string()),
            default_runtime: None,
            default_provider: None,
            default_model: None,
            default_skills: None,
            min_model_tier: Some(3),
            description: Some("A custom role".to_string()),
        }).unwrap();

        assert_eq!(role.name, "Custom Role");
        assert_eq!(role.category, "Custom");
        assert_eq!(role.min_model_tier, 3);
    }

    #[test]
    fn test_update_role() {
        let conn = setup_db();
        Role::seed_defaults(&conn).unwrap();

        let updated = Role::update(&conn, "Documentor", &UpdateRole {
            category: None,
            default_runtime: Some("claude".to_string()),
            default_provider: Some("anthropic".to_string()),
            default_model: Some("claude-haiku-4-5-20251001".to_string()),
            default_skills: None,
            min_model_tier: Some(2),
            description: None,
        }).unwrap();

        assert_eq!(updated.default_runtime, "claude");
        assert_eq!(updated.min_model_tier, 2);
        assert_eq!(updated.default_model, Some("claude-haiku-4-5-20251001".to_string()));
    }

    #[test]
    fn test_delete_role() {
        let conn = setup_db();
        Role::create(&conn, &CreateRole {
            name: "Temp".to_string(),
            category: None,
            default_runtime: None,
            default_provider: None,
            default_model: None,
            default_skills: None,
            min_model_tier: None,
            description: None,
        }).unwrap();

        Role::delete(&conn, "Temp").unwrap();
        assert!(Role::get_by_name(&conn, "Temp").is_err());
    }
}
