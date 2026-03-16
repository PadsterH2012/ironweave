use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub context: String,
    pub description: Option<String>,
    pub obsidian_vault_path: Option<String>,
    pub obsidian_project: Option<String>,
    pub git_remote: Option<String>,
    pub mount_id: Option<String>,
    pub sync_path: Option<String>,
    pub last_synced_at: Option<String>,
    pub sync_state: String,
    pub created_at: String,
    pub app_url: Option<String>,
    pub is_paused: bool,
    pub paused_at: Option<String>,
    pub pause_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub directory: String,
    pub context: String,
    pub obsidian_vault_path: Option<String>,
    pub obsidian_project: Option<String>,
    pub git_remote: Option<String>,
    pub mount_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub directory: Option<String>,
    pub context: Option<String>,
    pub description: Option<String>,
    pub obsidian_vault_path: Option<String>,
    pub obsidian_project: Option<String>,
    pub git_remote: Option<String>,
    pub mount_id: Option<String>,
    pub app_url: Option<String>,
}

impl Project {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            directory: row.get("directory")?,
            context: row.get("context")?,
            description: row.get("description")?,
            obsidian_vault_path: row.get("obsidian_vault_path")?,
            obsidian_project: row.get("obsidian_project")?,
            git_remote: row.get("git_remote")?,
            mount_id: row.get("mount_id")?,
            sync_path: row.get("sync_path")?,
            last_synced_at: row.get("last_synced_at")?,
            sync_state: row.get::<_, Option<String>>("sync_state")?.unwrap_or_else(|| "idle".to_string()),
            created_at: row.get("created_at")?,
            app_url: row.get("app_url")?,
            is_paused: row.get::<_, i64>("is_paused").unwrap_or(0) != 0,
            paused_at: row.get("paused_at")?,
            pause_reason: row.get("pause_reason")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateProject) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context, obsidian_vault_path, obsidian_project, git_remote, mount_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, input.name, input.directory, input.context, input.obsidian_vault_path, input.obsidian_project, input.git_remote, input.mount_id],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM projects WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("project {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    pub fn list_by_mount(conn: &Connection, mount_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM projects WHERE mount_id = ?1")?;
        let rows = stmt.query_map(params![mount_id], Self::from_row)?;
        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    pub fn clear_sync_state(conn: &Connection, id: &str) -> Result<()> {
        conn.execute(
            "UPDATE projects SET sync_state = 'idle', sync_path = NULL, last_synced_at = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("project {}", id)));
        }
        Ok(())
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateProject) -> Result<Self> {
        let existing = Self::get_by_id(conn, id)?;
        let name = input.name.as_deref().unwrap_or(&existing.name);
        let directory = input.directory.as_deref().unwrap_or(&existing.directory);
        let context = input.context.as_deref().unwrap_or(&existing.context);
        let description = input.description.as_deref().or(existing.description.as_deref());
        let obsidian_vault_path = input.obsidian_vault_path.as_deref().or(existing.obsidian_vault_path.as_deref());
        let obsidian_project = input.obsidian_project.as_deref().or(existing.obsidian_project.as_deref());
        let git_remote = input.git_remote.as_deref().or(existing.git_remote.as_deref());
        let mount_id = input.mount_id.as_deref().or(existing.mount_id.as_deref());
        let app_url = input.app_url.as_deref().or(existing.app_url.as_deref());

        conn.execute(
            "UPDATE projects SET name = ?1, directory = ?2, context = ?3, description = ?4,
             obsidian_vault_path = ?5, obsidian_project = ?6, git_remote = ?7, mount_id = ?8,
             app_url = ?9
             WHERE id = ?10",
            params![name, directory, context, description, obsidian_vault_path, obsidian_project, git_remote, mount_id, app_url, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn update_sync_state(conn: &Connection, id: &str, state: &str, sync_path: Option<&str>, last_synced_at: Option<&str>) -> Result<()> {
        conn.execute(
            "UPDATE projects SET sync_state = ?1, sync_path = COALESCE(?2, sync_path), last_synced_at = COALESCE(?3, last_synced_at) WHERE id = ?4",
            params![state, sync_path, last_synced_at, id],
        )?;
        Ok(())
    }

    pub fn pause(conn: &Connection, id: &str, reason: Option<&str>) -> Result<Self> {
        conn.execute(
            "UPDATE projects SET is_paused = 1, paused_at = datetime('now'), pause_reason = ?1 WHERE id = ?2",
            params![reason, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn resume(conn: &Connection, id: &str) -> Result<Self> {
        conn.execute(
            "UPDATE projects SET is_paused = 0, paused_at = NULL, pause_reason = NULL WHERE id = ?1",
            params![id],
        )?;
        Self::get_by_id(conn, id)
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
    fn test_create_and_get_by_id() {
        let conn = setup_db();
        let input = CreateProject {
            name: "Test Project".to_string(),
            directory: "/tmp/test".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        };
        let project = Project::create(&conn, &input).unwrap();
        assert_eq!(project.name, "Test Project");
        assert_eq!(project.context, "homelab");

        let fetched = Project::get_by_id(&conn, &project.id).unwrap();
        assert_eq!(fetched.id, project.id);
        assert_eq!(fetched.name, project.name);
    }

    #[test]
    fn test_list() {
        let conn = setup_db();
        let input_b = CreateProject {
            name: "Bravo".to_string(),
            directory: "/tmp/b".to_string(),
            context: "work".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        };
        let input_a = CreateProject {
            name: "Alpha".to_string(),
            directory: "/tmp/a".to_string(),
            context: "work".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        };
        Project::create(&conn, &input_b).unwrap();
        Project::create(&conn, &input_a).unwrap();

        let projects = Project::list(&conn).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "Alpha");
        assert_eq!(projects[1].name, "Bravo");
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let input = CreateProject {
            name: "Deleteme".to_string(),
            directory: "/tmp/del".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        };
        let project = Project::create(&conn, &input).unwrap();
        Project::delete(&conn, &project.id).unwrap();

        let result = Project::get_by_id(&conn, &project.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_not_found() {
        let conn = setup_db();
        let result = Project::get_by_id(&conn, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_update_project() {
        let conn = setup_db();
        let input = CreateProject {
            name: "Original".to_string(),
            directory: "/tmp/orig".to_string(),
            context: "work".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        };
        let project = Project::create(&conn, &input).unwrap();

        let update = UpdateProject {
            name: Some("Updated".to_string()),
            directory: None,
            context: None,
            description: Some("My project description".to_string()),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: Some("https://github.com/test".to_string()),
            mount_id: None,
            app_url: None,
        };
        let updated = Project::update(&conn, &project.id, &update).unwrap();
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.directory, "/tmp/orig");
        assert_eq!(updated.description, Some("My project description".to_string()));
        assert_eq!(updated.git_remote, Some("https://github.com/test".to_string()));
    }

    #[test]
    fn test_pause_and_resume() {
        let conn = setup_db();
        let input = CreateProject {
            name: "PauseTest".to_string(),
            directory: "/tmp/pause".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        };
        let project = Project::create(&conn, &input).unwrap();
        assert!(!project.is_paused);

        let paused = Project::pause(&conn, &project.id, Some("going home")).unwrap();
        assert!(paused.is_paused);
        assert!(paused.paused_at.is_some());
        assert_eq!(paused.pause_reason, Some("going home".to_string()));

        let resumed = Project::resume(&conn, &project.id).unwrap();
        assert!(!resumed.is_paused);
        assert!(resumed.paused_at.is_none());
        assert!(resumed.pause_reason.is_none());
    }

    #[test]
    fn test_update_sync_state() {
        let conn = setup_db();
        let input = CreateProject {
            name: "SyncTest".to_string(),
            directory: "/tmp/sync".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        };
        let project = Project::create(&conn, &input).unwrap();

        Project::update_sync_state(&conn, &project.id, "syncing", Some("/sync/test"), None).unwrap();
        let fetched = Project::get_by_id(&conn, &project.id).unwrap();
        assert_eq!(fetched.sync_state, "syncing");
        assert_eq!(fetched.sync_path, Some("/sync/test".to_string()));
    }
}
