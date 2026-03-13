use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHop {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub credential: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub id: String,
    pub name: String,
    pub hops: Vec<ProxyHop>,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProxyConfig {
    pub name: String,
    pub hops: Vec<ProxyHop>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProxyConfig {
    pub name: Option<String>,
    pub hops: Option<Vec<ProxyHop>>,
    pub is_active: Option<bool>,
}

impl ProxyConfig {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let hops_json: String = row.get("hops")?;
        let hops: Vec<ProxyHop> = serde_json::from_str(&hops_json).unwrap_or_default();
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            hops,
            is_active: row.get::<_, i32>("is_active").map(|v| v != 0)?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateProxyConfig) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let hops_json = serde_json::to_string(&input.hops)?;
        conn.execute(
            "INSERT INTO proxy_configs (id, name, hops) VALUES (?1, ?2, ?3)",
            params![id, input.name, hops_json],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM proxy_configs WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("proxy_config {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM proxy_configs ORDER BY name")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut configs = Vec::new();
        for row in rows {
            configs.push(row?);
        }
        Ok(configs)
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateProxyConfig) -> Result<Self> {
        let existing = Self::get_by_id(conn, id)?;
        let name = input.name.as_deref().unwrap_or(&existing.name);
        let hops_json = match &input.hops {
            Some(hops) => serde_json::to_string(hops)?,
            None => serde_json::to_string(&existing.hops)?,
        };
        let is_active = input.is_active.unwrap_or(existing.is_active) as i32;

        conn.execute(
            "UPDATE proxy_configs SET name = ?1, hops = ?2, is_active = ?3 WHERE id = ?4",
            params![name, hops_json, is_active, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let mount_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM mounts WHERE proxy_config_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        if mount_count > 0 {
            return Err(IronweaveError::Conflict(
                format!("proxy config {} is referenced by {} mount(s)", id, mount_count)
            ));
        }

        let changes = conn.execute("DELETE FROM proxy_configs WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("proxy_config {}", id)));
        }
        Ok(())
    }

    pub fn redacted(&self) -> Self {
        let mut c = self.clone();
        for hop in &mut c.hops {
            if hop.credential.is_some() {
                hop.credential = Some("***".to_string());
            }
        }
        c
    }

    pub fn proxy_jump_string(&self) -> String {
        self.hops
            .iter()
            .map(|h| format!("{}@{}:{}", h.username, h.host, h.port))
            .collect::<Vec<_>>()
            .join(",")
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

    fn sample_hops() -> Vec<ProxyHop> {
        vec![
            ProxyHop {
                host: "10.202.28.230".to_string(),
                port: 22,
                username: "paddy".to_string(),
                auth_type: "key".to_string(),
                credential: None,
            },
            ProxyHop {
                host: "localhost".to_string(),
                port: 2222,
                username: "paddy".to_string(),
                auth_type: "password".to_string(),
                credential: Some("secret".to_string()),
            },
        ]
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let input = CreateProxyConfig {
            name: "cuk-proxy".to_string(),
            hops: sample_hops(),
        };
        let pc = ProxyConfig::create(&conn, &input).unwrap();
        assert_eq!(pc.name, "cuk-proxy");
        assert_eq!(pc.hops.len(), 2);
        assert!(pc.is_active);

        let fetched = ProxyConfig::get_by_id(&conn, &pc.id).unwrap();
        assert_eq!(fetched.hops[0].host, "10.202.28.230");
        assert_eq!(fetched.hops[1].port, 2222);
    }

    #[test]
    fn test_list() {
        let conn = setup_db();
        ProxyConfig::create(&conn, &CreateProxyConfig {
            name: "bravo".to_string(), hops: vec![],
        }).unwrap();
        ProxyConfig::create(&conn, &CreateProxyConfig {
            name: "alpha".to_string(), hops: vec![],
        }).unwrap();

        let list = ProxyConfig::list(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha");
    }

    #[test]
    fn test_update() {
        let conn = setup_db();
        let pc = ProxyConfig::create(&conn, &CreateProxyConfig {
            name: "orig".to_string(), hops: sample_hops(),
        }).unwrap();

        let updated = ProxyConfig::update(&conn, &pc.id, &UpdateProxyConfig {
            name: Some("renamed".to_string()),
            hops: None,
            is_active: Some(false),
        }).unwrap();
        assert_eq!(updated.name, "renamed");
        assert!(!updated.is_active);
        assert_eq!(updated.hops.len(), 2);
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let pc = ProxyConfig::create(&conn, &CreateProxyConfig {
            name: "del".to_string(), hops: vec![],
        }).unwrap();
        ProxyConfig::delete(&conn, &pc.id).unwrap();
        assert!(ProxyConfig::get_by_id(&conn, &pc.id).is_err());
    }

    #[test]
    fn test_delete_blocked_by_mount() {
        let conn = setup_db();
        let pc = ProxyConfig::create(&conn, &CreateProxyConfig {
            name: "used".to_string(), hops: vec![],
        }).unwrap();

        conn.execute(
            "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point, proxy_config_id) VALUES ('m1', 'test', 'sshfs', 'u@h:/p', '/mnt/t', ?1)",
            params![pc.id],
        ).unwrap();

        let result = ProxyConfig::delete(&conn, &pc.id);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("referenced"));
    }

    #[test]
    fn test_redacted() {
        let conn = setup_db();
        let pc = ProxyConfig::create(&conn, &CreateProxyConfig {
            name: "test".to_string(), hops: sample_hops(),
        }).unwrap();
        let redacted = pc.redacted();
        assert_eq!(redacted.hops[0].credential, None);
        assert_eq!(redacted.hops[1].credential, Some("***".to_string()));
    }

    #[test]
    fn test_proxy_jump_string() {
        let pc = ProxyConfig {
            id: "x".to_string(),
            name: "test".to_string(),
            hops: sample_hops(),
            is_active: true,
            created_at: "".to_string(),
        };
        assert_eq!(pc.proxy_jump_string(), "paddy@10.202.28.230:22,paddy@localhost:2222");
    }
}
