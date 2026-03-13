use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
    pub category: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpsertSetting {
    pub value: String,
    pub category: Option<String>,
}

impl Setting {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            key: row.get("key")?,
            value: row.get("value")?,
            category: row.get("category")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn upsert(conn: &Connection, key: &str, input: &UpsertSetting) -> Result<Self> {
        let category = input.category.as_deref().unwrap_or("general");
        conn.execute(
            "INSERT INTO settings (key, value, category, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = ?2, category = ?3, updated_at = datetime('now')",
            params![key, input.value, category],
        )?;
        Self::get_by_key(conn, key)
    }

    pub fn get_by_key(conn: &Connection, key: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM settings WHERE key = ?1",
            params![key],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("setting {}", key)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM settings ORDER BY category, key")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut settings = Vec::new();
        for row in rows {
            settings.push(row?);
        }
        Ok(settings)
    }

    pub fn list_by_category(conn: &Connection, category: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM settings WHERE category = ?1 ORDER BY key")?;
        let rows = stmt.query_map(params![category], Self::from_row)?;
        let mut settings = Vec::new();
        for row in rows {
            settings.push(row?);
        }
        Ok(settings)
    }

    pub fn delete(conn: &Connection, key: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("setting {}", key)));
        }
        Ok(())
    }

    /// Redact sensitive settings (master_key, anything in api_keys category)
    pub fn redacted(&self) -> Self {
        let mut s = self.clone();
        if s.key == "master_key" || s.category == "api_keys" {
            s.value = "***".to_string();
        }
        s
    }

    /// Seed a setting only if it doesn't already exist
    pub fn seed(conn: &Connection, key: &str, value: &str, category: &str) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value, category) VALUES (?1, ?2, ?3)",
            params![key, value, category],
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
        conn
    }

    #[test]
    fn test_upsert_and_get() {
        let conn = setup_db();
        let input = UpsertSetting { value: "val1".to_string(), category: Some("general".to_string()) };
        let setting = Setting::upsert(&conn, "test_key", &input).unwrap();
        assert_eq!(setting.key, "test_key");
        assert_eq!(setting.value, "val1");

        let input2 = UpsertSetting { value: "val2".to_string(), category: None };
        let updated = Setting::upsert(&conn, "test_key", &input2).unwrap();
        assert_eq!(updated.value, "val2");
    }

    #[test]
    fn test_list_and_category() {
        let conn = setup_db();
        Setting::seed(&conn, "k1", "v1", "general").unwrap();
        Setting::seed(&conn, "k2", "v2", "api_keys").unwrap();
        Setting::seed(&conn, "k3", "v3", "general").unwrap();

        let all = Setting::list(&conn).unwrap();
        assert_eq!(all.len(), 3);

        let api = Setting::list_by_category(&conn, "api_keys").unwrap();
        assert_eq!(api.len(), 1);
        assert_eq!(api[0].key, "k2");
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        Setting::seed(&conn, "k1", "v1", "general").unwrap();
        Setting::delete(&conn, "k1").unwrap();
        assert!(Setting::get_by_key(&conn, "k1").is_err());
    }

    #[test]
    fn test_redacted() {
        let conn = setup_db();
        Setting::seed(&conn, "master_key", "supersecret", "general").unwrap();
        Setting::seed(&conn, "api_token", "tok123", "api_keys").unwrap();

        let mk = Setting::get_by_key(&conn, "master_key").unwrap().redacted();
        assert_eq!(mk.value, "***");

        let api = Setting::get_by_key(&conn, "api_token").unwrap().redacted();
        assert_eq!(api.value, "***");
    }

    #[test]
    fn test_seed_no_overwrite() {
        let conn = setup_db();
        Setting::seed(&conn, "k1", "original", "general").unwrap();
        Setting::seed(&conn, "k1", "overwrite_attempt", "general").unwrap();
        let s = Setting::get_by_key(&conn, "k1").unwrap();
        assert_eq!(s.value, "original");
    }
}
