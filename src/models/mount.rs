use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    pub id: String,
    pub name: String,
    pub mount_type: String,
    pub remote_path: String,
    pub local_mount_point: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key: Option<String>,
    pub mount_options: Option<String>,
    pub auto_mount: bool,
    pub state: String,
    pub last_error: Option<String>,
    pub created_at: String,
    pub proxy_config_id: Option<String>,
    pub git_remote: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMount {
    pub name: String,
    pub mount_type: String,
    pub remote_path: String,
    pub local_mount_point: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key: Option<String>,
    pub mount_options: Option<String>,
    pub auto_mount: Option<bool>,
    pub proxy_config_id: Option<String>,
    pub git_remote: Option<String>,
}

impl Mount {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            mount_type: row.get("mount_type")?,
            remote_path: row.get("remote_path")?,
            local_mount_point: row.get("local_mount_point")?,
            username: row.get("username")?,
            password: row.get("password")?,
            ssh_key: row.get("ssh_key")?,
            mount_options: row.get("mount_options")?,
            auto_mount: row.get::<_, i32>("auto_mount").map(|v| v != 0)?,
            state: row.get("state")?,
            last_error: row.get("last_error")?,
            created_at: row.get("created_at")?,
            proxy_config_id: row.get("proxy_config_id")?,
            git_remote: row.get("git_remote")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateMount) -> Result<Self> {
        // Validate local_mount_point: must be absolute, no traversal, not a dangerous system path
        let mount_path = std::path::Path::new(&input.local_mount_point);
        if !mount_path.is_absolute() {
            return Err(IronweaveError::Validation("local_mount_point must be an absolute path".into()));
        }
        if input.local_mount_point.contains("..") {
            return Err(IronweaveError::Validation("local_mount_point must not contain path traversal".into()));
        }
        let dangerous_paths = ["/", "/etc", "/sys", "/proc", "/dev", "/boot", "/bin", "/sbin", "/usr", "/lib", "/var"];
        let normalized = input.local_mount_point.trim_end_matches('/');
        if dangerous_paths.contains(&normalized) {
            return Err(IronweaveError::Validation("local_mount_point must not be a system directory".into()));
        }
        let id = Uuid::new_v4().to_string();
        let auto_mount = input.auto_mount.unwrap_or(true) as i32;
        conn.execute(
            "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point, username, password, ssh_key, mount_options, auto_mount, proxy_config_id, git_remote)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![id, input.name, input.mount_type, input.remote_path, input.local_mount_point,
                    input.username, input.password, input.ssh_key, input.mount_options, auto_mount, input.proxy_config_id, input.git_remote],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM mounts WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("mount {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM mounts ORDER BY name")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut mounts = Vec::new();
        for row in rows {
            mounts.push(row?);
        }
        Ok(mounts)
    }

    pub fn update_state(conn: &Connection, id: &str, state: &str, error: Option<&str>) -> Result<()> {
        let changes = conn.execute(
            "UPDATE mounts SET state = ?1, last_error = ?2 WHERE id = ?3",
            params![state, error, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("mount {}", id)));
        }
        Ok(())
    }

    pub fn update(conn: &Connection, id: &str, input: &CreateMount) -> Result<Self> {
        // Same mount path validation as create
        let mount_path = std::path::Path::new(&input.local_mount_point);
        if !mount_path.is_absolute() {
            return Err(IronweaveError::Validation("local_mount_point must be an absolute path".into()));
        }
        if input.local_mount_point.contains("..") {
            return Err(IronweaveError::Validation("local_mount_point must not contain path traversal".into()));
        }
        let dangerous_paths = ["/", "/etc", "/sys", "/proc", "/dev", "/boot", "/bin", "/sbin", "/usr", "/lib", "/var"];
        let normalized = input.local_mount_point.trim_end_matches('/');
        if dangerous_paths.contains(&normalized) {
            return Err(IronweaveError::Validation("local_mount_point must not be a system directory".into()));
        }
        let auto_mount = input.auto_mount.unwrap_or(true) as i32;
        let changes = conn.execute(
            "UPDATE mounts SET name = ?1, mount_type = ?2, remote_path = ?3, local_mount_point = ?4,
             username = ?5, password = ?6, ssh_key = ?7, mount_options = ?8, auto_mount = ?9, proxy_config_id = ?10, git_remote = ?11
             WHERE id = ?12",
            params![input.name, input.mount_type, input.remote_path, input.local_mount_point,
                    input.username, input.password, input.ssh_key, input.mount_options, auto_mount, input.proxy_config_id, input.git_remote, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("mount {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM mounts WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("mount {}", id)));
        }
        Ok(())
    }

    /// Return a redacted copy (credentials replaced with "***")
    pub fn redacted(&self) -> Self {
        let mut m = self.clone();
        if m.password.is_some() {
            m.password = Some("***".to_string());
        }
        if m.ssh_key.is_some() {
            m.ssh_key = Some("***".to_string());
        }
        m
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

    fn sample_input() -> CreateMount {
        CreateMount {
            name: "NAS Share".to_string(),
            mount_type: "smb".to_string(),
            remote_path: "//nas/share".to_string(),
            local_mount_point: "/mnt/nas".to_string(),
            username: Some("admin".to_string()),
            password: Some("secret123".to_string()),
            ssh_key: Some("-----BEGIN KEY-----".to_string()),
            mount_options: Some("vers=3.0".to_string()),
            auto_mount: Some(true),
            proxy_config_id: None,
            git_remote: None,
        }
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let input = sample_input();
        let mount = Mount::create(&conn, &input).unwrap();
        assert_eq!(mount.name, "NAS Share");
        assert_eq!(mount.mount_type, "smb");
        assert_eq!(mount.remote_path, "//nas/share");
        assert_eq!(mount.local_mount_point, "/mnt/nas");
        assert_eq!(mount.username, Some("admin".to_string()));
        assert_eq!(mount.password, Some("secret123".to_string()));
        assert!(mount.auto_mount);
        assert_eq!(mount.state, "unmounted");

        let fetched = Mount::get_by_id(&conn, &mount.id).unwrap();
        assert_eq!(fetched.id, mount.id);
        assert_eq!(fetched.name, mount.name);
    }

    #[test]
    fn test_list_ordered_by_name() {
        let conn = setup_db();
        let mut input_b = sample_input();
        input_b.name = "Bravo".to_string();
        let mut input_a = sample_input();
        input_a.name = "Alpha".to_string();

        Mount::create(&conn, &input_b).unwrap();
        Mount::create(&conn, &input_a).unwrap();

        let mounts = Mount::list(&conn).unwrap();
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].name, "Alpha");
        assert_eq!(mounts[1].name, "Bravo");
    }

    #[test]
    fn test_update_state() {
        let conn = setup_db();
        let input = sample_input();
        let mount = Mount::create(&conn, &input).unwrap();
        assert_eq!(mount.state, "unmounted");

        Mount::update_state(&conn, &mount.id, "mounted", None).unwrap();
        let updated = Mount::get_by_id(&conn, &mount.id).unwrap();
        assert_eq!(updated.state, "mounted");
        assert_eq!(updated.last_error, None);

        Mount::update_state(&conn, &mount.id, "error", Some("connection refused")).unwrap();
        let errored = Mount::get_by_id(&conn, &mount.id).unwrap();
        assert_eq!(errored.state, "error");
        assert_eq!(errored.last_error, Some("connection refused".to_string()));
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let input = sample_input();
        let mount = Mount::create(&conn, &input).unwrap();
        Mount::delete(&conn, &mount.id).unwrap();

        let result = Mount::get_by_id(&conn, &mount.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_redacted() {
        let conn = setup_db();
        let input = sample_input();
        let mount = Mount::create(&conn, &input).unwrap();

        let redacted = mount.redacted();
        assert_eq!(redacted.password, Some("***".to_string()));
        assert_eq!(redacted.ssh_key, Some("***".to_string()));
        assert_eq!(redacted.username, Some("admin".to_string()));
        assert_eq!(redacted.name, "NAS Share");
    }
}
