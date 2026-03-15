use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

/// Per-team override for a role's runtime/provider/model.
/// Resolution order: slot config → team_role_override → project tier range → global role default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRoleOverride {
    pub id: String,
    pub team_id: String,
    pub role: String,
    pub runtime: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub is_user_set: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SetTeamRoleOverride {
    pub role: String,
    pub runtime: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

impl TeamRoleOverride {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            team_id: row.get("team_id")?,
            role: row.get("role")?,
            runtime: row.get("runtime")?,
            provider: row.get("provider")?,
            model: row.get("model")?,
            is_user_set: row.get::<_, i32>("is_user_set")? != 0,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// List all overrides for a team
    pub fn list_by_team(conn: &Connection, team_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM team_role_overrides WHERE team_id = ?1 ORDER BY role"
        )?;
        let rows = stmt.query_map(params![team_id], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Get override for a specific role in a team
    pub fn get_for_role(conn: &Connection, team_id: &str, role: &str) -> Result<Option<Self>> {
        match conn.query_row(
            "SELECT * FROM team_role_overrides WHERE team_id = ?1 AND role = ?2",
            params![team_id, role],
            Self::from_row,
        ) {
            Ok(o) => Ok(Some(o)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set or update a role override for a team (user-initiated, so is_user_set = true)
    pub fn set(conn: &Connection, team_id: &str, input: &SetTeamRoleOverride) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO team_role_overrides (id, team_id, role, runtime, provider, model, is_user_set)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
             ON CONFLICT(team_id, role)
             DO UPDATE SET runtime = ?4, provider = ?5, model = ?6, is_user_set = 1, updated_at = datetime('now')",
            params![id, team_id, input.role, input.runtime, input.provider, input.model],
        )?;
        // Return the actual record (might be the existing one on conflict)
        Ok(Self::get_for_role(conn, team_id, &input.role)?.unwrap())
    }

    /// Set a coordinator-initiated override (is_user_set = false, can be overwritten by mode changes)
    pub fn set_coordinator(conn: &Connection, team_id: &str, role: &str, model: &str, runtime: Option<&str>, provider: Option<&str>) -> Result<Self> {
        // Don't overwrite user-set overrides
        if let Some(existing) = Self::get_for_role(conn, team_id, role)? {
            if existing.is_user_set {
                return Ok(existing);
            }
        }
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO team_role_overrides (id, team_id, role, runtime, provider, model, is_user_set)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
             ON CONFLICT(team_id, role)
             DO UPDATE SET runtime = ?4, provider = ?5, model = ?6, updated_at = datetime('now')
             WHERE is_user_set = 0",
            params![id, team_id, role, runtime, provider, model],
        )?;
        Ok(Self::get_for_role(conn, team_id, role)?.unwrap())
    }

    /// Clear a role override for a team
    pub fn clear(conn: &Connection, team_id: &str, role: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM team_role_overrides WHERE team_id = ?1 AND role = ?2",
            params![team_id, role],
        )?;
        Ok(())
    }

    /// Clear all non-user-set overrides for a team (used during coordination mode transitions)
    pub fn clear_coordinator_overrides(conn: &Connection, team_id: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM team_role_overrides WHERE team_id = ?1 AND is_user_set = 0",
            params![team_id],
        )?;
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
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'Test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO teams (id, name, project_id) VALUES ('t1', 'Dev', 'p1')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_set_and_list() {
        let conn = setup_db();
        TeamRoleOverride::set(&conn, "t1", &SetTeamRoleOverride {
            role: "Senior Coder".to_string(),
            runtime: Some("claude".to_string()),
            provider: Some("anthropic".to_string()),
            model: Some("claude-sonnet-4-6".to_string()),
        }).unwrap();

        let overrides = TeamRoleOverride::list_by_team(&conn, "t1").unwrap();
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].role, "Senior Coder");
        assert!(overrides[0].is_user_set);
    }

    #[test]
    fn test_upsert() {
        let conn = setup_db();
        TeamRoleOverride::set(&conn, "t1", &SetTeamRoleOverride {
            role: "Senior Coder".to_string(),
            runtime: None,
            provider: None,
            model: Some("claude-haiku-4-5".to_string()),
        }).unwrap();

        // Update the same role
        let updated = TeamRoleOverride::set(&conn, "t1", &SetTeamRoleOverride {
            role: "Senior Coder".to_string(),
            runtime: None,
            provider: None,
            model: Some("claude-sonnet-4-6".to_string()),
        }).unwrap();

        assert_eq!(updated.model, Some("claude-sonnet-4-6".to_string()));
        let all = TeamRoleOverride::list_by_team(&conn, "t1").unwrap();
        assert_eq!(all.len(), 1); // No duplicates
    }

    #[test]
    fn test_coordinator_respects_user_set() {
        let conn = setup_db();

        // User sets an override
        TeamRoleOverride::set(&conn, "t1", &SetTeamRoleOverride {
            role: "Researcher".to_string(),
            runtime: None,
            provider: None,
            model: Some("deepseek-v3".to_string()),
        }).unwrap();

        // Coordinator tries to override — should be ignored
        let result = TeamRoleOverride::set_coordinator(&conn, "t1", "Researcher", "llama-3.1-70b", None, None).unwrap();
        assert_eq!(result.model, Some("deepseek-v3".to_string())); // User's choice preserved
        assert!(result.is_user_set);
    }

    #[test]
    fn test_clear_coordinator_overrides() {
        let conn = setup_db();

        // Coordinator sets two overrides
        TeamRoleOverride::set_coordinator(&conn, "t1", "Senior Coder", "claude-sonnet-4-6", None, None).unwrap();
        TeamRoleOverride::set_coordinator(&conn, "t1", "Tester", "claude-haiku-4-5", None, None).unwrap();

        // User sets one override
        TeamRoleOverride::set(&conn, "t1", &SetTeamRoleOverride {
            role: "Researcher".to_string(),
            runtime: None,
            provider: None,
            model: Some("deepseek-v3".to_string()),
        }).unwrap();

        assert_eq!(TeamRoleOverride::list_by_team(&conn, "t1").unwrap().len(), 3);

        // Clear coordinator overrides — user's should survive
        TeamRoleOverride::clear_coordinator_overrides(&conn, "t1").unwrap();
        let remaining = TeamRoleOverride::list_by_team(&conn, "t1").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].role, "Researcher");
        assert!(remaining[0].is_user_set);
    }

    #[test]
    fn test_clear_specific() {
        let conn = setup_db();
        TeamRoleOverride::set(&conn, "t1", &SetTeamRoleOverride {
            role: "Senior Coder".to_string(),
            runtime: None,
            provider: None,
            model: Some("test".to_string()),
        }).unwrap();

        TeamRoleOverride::clear(&conn, "t1", "Senior Coder").unwrap();
        assert!(TeamRoleOverride::get_for_role(&conn, "t1", "Senior Coder").unwrap().is_none());
    }
}
