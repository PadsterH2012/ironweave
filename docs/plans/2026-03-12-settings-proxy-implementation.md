# Settings Page & Proxy Tunnel Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Settings page with sub-routes for managing global app settings, SSH proxy chains, and API keys stored in the database, and enable SSHFS mounts to route through multi-hop proxy chains.

**Architecture:** Hybrid DB storage — key-value `settings` table for simple scalars, typed `proxy_configs` table for SSH proxy chains. TOML seeds DB on first run, DB takes precedence after. Frontend uses separate sub-routes under `/settings` with a sidebar nav. Mount manager builds `-o ProxyJump=...` from proxy hop chains for SSHFS mounts.

**Tech Stack:** Rust/Axum 0.8, SQLite (rusqlite), Svelte 5 (runes), TypeScript, Tailwind CSS v4, AES-256-GCM (existing `mount/crypto.rs`)

---

## Context for Implementers

- **Cargo path:** `/Users/paddyharker/.cargo/bin/cargo` (not in PATH)
- **Test command:** `/Users/paddyharker/.cargo/bin/cargo test --lib --bin ironweave`
- **Frontend build:** `cd frontend && npm run build` (required for rust-embed to pick up changes)
- **Existing patterns:** See `src/models/mount.rs` for model CRUD pattern, `src/api/mounts.rs` for API handler pattern, `frontend/src/routes/Mounts.svelte` for Svelte page pattern
- **Design doc:** `docs/plans/2026-03-12-settings-proxy-design.md`

---

### Task 1: Add `settings` and `proxy_configs` tables to DB schema

**Files:**
- Modify: `src/db/migrations.rs`

**Step 1: Add the new tables to the migration**

Add these two table definitions to the `execute_batch` in `run_migrations`, after the `merge_queue_entries` table and before the `CREATE INDEX` statements:

```rust
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'general',
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS proxy_configs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            hops TEXT NOT NULL DEFAULT '[]',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
```

Add an index after the existing indexes:

```rust
        CREATE INDEX IF NOT EXISTS idx_settings_category ON settings(category);
```

Add an incremental migration for the `mounts` table (after the existing `ALTER TABLE projects` migration):

```rust
    let _ = conn.execute("ALTER TABLE mounts ADD COLUMN proxy_config_id TEXT REFERENCES proxy_configs(id)", []);
```

**Step 2: Add tests**

Add to the existing test module:

```rust
    #[test]
    fn test_settings_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO settings (key, value, category) VALUES ('test_key', 'test_value', 'general')",
            [],
        ).unwrap();

        let val: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'test_key'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(val, "test_value");
    }

    #[test]
    fn test_proxy_configs_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO proxy_configs (id, name, hops) VALUES ('pc1', 'test-proxy', '[{\"host\":\"10.0.0.1\",\"port\":22}]')",
            [],
        ).unwrap();

        let name: String = conn
            .query_row("SELECT name FROM proxy_configs WHERE id = 'pc1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "test-proxy");
    }

    #[test]
    fn test_mounts_has_proxy_config_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO proxy_configs (id, name, hops) VALUES ('pc1', 'proxy', '[]')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point, proxy_config_id) VALUES ('m1', 'test', 'sshfs', 'user@host:/path', '/mnt/test', 'pc1')",
            [],
        ).unwrap();

        let pcid: Option<String> = conn
            .query_row("SELECT proxy_config_id FROM mounts WHERE id = 'm1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(pcid, Some("pc1".to_string()));
    }
```

**Step 3: Run tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib -- db::migrations`
Expected: All tests pass including the 3 new ones.

**Step 4: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add settings and proxy_configs tables to database schema"
```

---

### Task 2: Settings model with CRUD operations

**Files:**
- Create: `src/models/setting.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create the setting model**

Create `src/models/setting.rs`:

```rust
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
```

**Step 2: Register the module**

Add to `src/models/mod.rs`:

```rust
pub mod setting;
```

**Step 3: Run tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib -- models::setting`
Expected: All 5 tests pass.

**Step 4: Commit**

```bash
git add src/models/setting.rs src/models/mod.rs
git commit -m "feat: settings model with CRUD, redaction, and seeding"
```

---

### Task 3: ProxyConfig model with CRUD operations

**Files:**
- Create: `src/models/proxy_config.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create the proxy_config model**

Create `src/models/proxy_config.rs`:

```rust
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHop {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,  // "key" or "password"
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
        // Check if any mounts reference this proxy config
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

    /// Return a redacted copy (credentials replaced with "***")
    pub fn redacted(&self) -> Self {
        let mut c = self.clone();
        for hop in &mut c.hops {
            if hop.credential.is_some() {
                hop.credential = Some("***".to_string());
            }
        }
        c
    }

    /// Build a ProxyJump string for SSH/SSHFS: "user1@host1:port1,user2@host2:port2"
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
        assert_eq!(redacted.hops[0].credential, None); // key auth, no credential
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
```

**Step 2: Register the module**

Add to `src/models/mod.rs`:

```rust
pub mod proxy_config;
```

**Step 3: Run tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib -- models::proxy_config`
Expected: All 7 tests pass.

**Step 4: Commit**

```bash
git add src/models/proxy_config.rs src/models/mod.rs
git commit -m "feat: proxy config model with hop chain, CRUD, and cascade protection"
```

---

### Task 4: Settings API handlers

**Files:**
- Create: `src/api/settings.rs`
- Modify: `src/api/mod.rs`

**Step 1: Create the settings API**

Create `src/api/settings.rs`:

```rust
use axum::{extract::{Path, State}, Json, http::StatusCode};
use crate::state::AppState;
use crate::models::setting::{Setting, UpsertSetting};

pub async fn list(State(state): State<AppState>) -> Json<Vec<Setting>> {
    let conn = state.db.lock().unwrap();
    let settings = Setting::list(&conn).unwrap_or_default();
    Json(settings.into_iter().map(|s| s.redacted()).collect())
}

pub async fn get(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Setting>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Setting::get_by_key(&conn, &key)
        .map(|s| Json(s.redacted()))
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn upsert(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<UpsertSetting>,
) -> Result<Json<Setting>, StatusCode> {
    // Validate key: alphanumeric + underscores only
    if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(StatusCode::BAD_REQUEST);
    }
    if input.value.len() > 4096 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let conn = state.db.lock().unwrap();
    Setting::upsert(&conn, &key, &input)
        .map(|s| Json(s.redacted()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.db.lock().unwrap();
    Setting::delete(&conn, &key)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

**Step 2: Register in api/mod.rs**

Add to `src/api/mod.rs`:

```rust
pub mod settings;
```

**Step 3: Run compilation check**

Run: `/Users/paddyharker/.cargo/bin/cargo check`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src/api/settings.rs src/api/mod.rs
git commit -m "feat: settings API with list, get, upsert, and delete"
```

---

### Task 5: ProxyConfig API handlers

**Files:**
- Create: `src/api/proxy_configs.rs`
- Modify: `src/api/mod.rs`

**Step 1: Create the proxy_configs API**

Create `src/api/proxy_configs.rs`:

```rust
use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::Serialize;
use crate::state::AppState;
use crate::models::proxy_config::{ProxyConfig, CreateProxyConfig, UpdateProxyConfig};

#[derive(Debug, Serialize)]
pub struct ProxyConfigResponse {
    pub id: String,
    pub name: String,
    pub hops: Vec<crate::models::proxy_config::ProxyHop>,
    pub is_active: bool,
    pub created_at: String,
}

impl From<ProxyConfig> for ProxyConfigResponse {
    fn from(pc: ProxyConfig) -> Self {
        let redacted = pc.redacted();
        Self {
            id: redacted.id,
            name: redacted.name,
            hops: redacted.hops,
            is_active: redacted.is_active,
            created_at: redacted.created_at,
        }
    }
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<ProxyConfigResponse>> {
    let conn = state.db.lock().unwrap();
    let configs = ProxyConfig::list(&conn).unwrap_or_default();
    Json(configs.into_iter().map(ProxyConfigResponse::from).collect())
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateProxyConfig>,
) -> Result<(StatusCode, Json<ProxyConfigResponse>), StatusCode> {
    if input.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Validate hops
    for hop in &input.hops {
        if hop.host.trim().is_empty() || hop.username.trim().is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }
        if hop.port == 0 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let conn = state.db.lock().unwrap();
    ProxyConfig::create(&conn, &input)
        .map(|pc| (StatusCode::CREATED, Json(ProxyConfigResponse::from(pc))))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProxyConfigResponse>, StatusCode> {
    let conn = state.db.lock().unwrap();
    ProxyConfig::get_by_id(&conn, &id)
        .map(|pc| Json(ProxyConfigResponse::from(pc)))
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProxyConfig>,
) -> Result<Json<ProxyConfigResponse>, StatusCode> {
    let conn = state.db.lock().unwrap();
    ProxyConfig::update(&conn, &id, &input)
        .map(|pc| Json(ProxyConfigResponse::from(pc)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let conn = state.db.lock().unwrap();
    ProxyConfig::delete(&conn, &id).map(|_| StatusCode::NO_CONTENT).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("referenced") {
            (StatusCode::CONFLICT, msg)
        } else if msg.contains("not found") {
            (StatusCode::NOT_FOUND, msg)
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        }
    })
}

pub async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn = state.db.lock().unwrap();
    let pc = ProxyConfig::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    drop(conn);

    if pc.hops.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "No hops configured"
        })));
    }

    // Test first hop connectivity with a 10-second timeout
    let first = &pc.hops[0];
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new("ssh")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=5",
                "-o", "BatchMode=yes",
                "-p", &first.port.to_string(),
                &format!("{}@{}", first.username, first.host),
                "echo", "ok",
            ])
            .output()
    ).await;

    match output {
        Ok(Ok(out)) if out.status.success() => {
            Ok(Json(serde_json::json!({
                "success": true,
                "hops_tested": 1,
                "message": format!("First hop {}:{} reachable", first.host, first.port)
            })))
        }
        Ok(Ok(out)) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Ok(Json(serde_json::json!({
                "success": false,
                "failed_hop": 0,
                "error": format!("Hop 1 ({}:{}) failed: {}", first.host, first.port, stderr.trim())
            })))
        }
        Ok(Err(e)) => {
            Ok(Json(serde_json::json!({
                "success": false,
                "error": format!("SSH command failed: {}", e)
            })))
        }
        Err(_) => {
            Ok(Json(serde_json::json!({
                "success": false,
                "error": "Connection timed out (10s)"
            })))
        }
    }
}
```

**Step 2: Register in api/mod.rs**

Add to `src/api/mod.rs`:

```rust
pub mod proxy_configs;
```

**Step 3: Run compilation check**

Run: `/Users/paddyharker/.cargo/bin/cargo check`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src/api/proxy_configs.rs src/api/mod.rs
git commit -m "feat: proxy config API with CRUD, test connection, and cascade delete protection"
```

---

### Task 6: Wire routes into main.rs and add settings bootstrap

**Files:**
- Modify: `src/main.rs`

**Step 1: Add the new routes**

Add these route blocks to `main.rs` after the existing mount routes (line 106), before the auth middleware block:

```rust
        // Settings
        .route("/api/settings", get(api::settings::list))
        .route("/api/settings/{key}", get(api::settings::get).put(api::settings::upsert).delete(api::settings::delete))
        // Proxy Configs
        .route("/api/proxy-configs", get(api::proxy_configs::list).post(api::proxy_configs::create))
        .route("/api/proxy-configs/{id}", get(api::proxy_configs::get).put(api::proxy_configs::update).delete(api::proxy_configs::delete))
        .route("/api/proxy-configs/{id}/test", post(api::proxy_configs::test_connection));
```

Also add `put` to the routing import at the top of main.rs:

```rust
use axum::{Router, middleware, routing::{get, post, put, delete}};
```

**Step 2: Add settings bootstrap after DB init**

After the `auth::create_sessions_table` call (around line 47), add:

```rust
    // Seed default settings from config (only on first run)
    {
        let conn = db.lock().unwrap();
        if let Some(ref fs) = config.filesystem {
            let roots = serde_json::to_string(&fs.browse_roots).unwrap_or_else(|_| "[]".to_string());
            models::setting::Setting::seed(&conn, "browse_roots", &roots, "general").unwrap_or(());
            models::setting::Setting::seed(&conn, "mount_base", &fs.mount_base, "general").unwrap_or(());
            if let Some(mins) = fs.idle_unmount_minutes {
                models::setting::Setting::seed(&conn, "idle_unmount_minutes", &mins.to_string(), "general").unwrap_or(());
            }
        }
        if let Some(ref sec) = config.security {
            models::setting::Setting::seed(&conn, "master_key", &sec.master_key, "general").unwrap_or(());
        }
    }
```

Also add the `models` import if not already present:

```rust
use crate::models;
```

**Step 3: Run compilation check**

Run: `/Users/paddyharker/.cargo/bin/cargo check`
Expected: Compiles without errors.

**Step 4: Run all tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib --bin ironweave`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire settings and proxy config routes, add settings bootstrap from TOML"
```

---

### Task 7: Add proxy_config_id to Mount model and update SSHFS mount with ProxyJump

**Files:**
- Modify: `src/models/mount.rs`
- Modify: `src/mount/manager.rs`
- Modify: `src/api/mounts.rs`

**Step 1: Update Mount model**

In `src/models/mount.rs`, add `proxy_config_id` to the `Mount` struct:

```rust
pub struct Mount {
    // ... existing fields ...
    pub proxy_config_id: Option<String>,
}
```

Add to `CreateMount`:

```rust
pub struct CreateMount {
    // ... existing fields ...
    pub proxy_config_id: Option<String>,
}
```

Update `from_row` to read it:

```rust
    proxy_config_id: row.get("proxy_config_id")?,
```

Update the `create` method INSERT to include it:

```rust
        conn.execute(
            "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point, username, password, ssh_key, mount_options, auto_mount, proxy_config_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![id, input.name, input.mount_type, input.remote_path, input.local_mount_point,
                    input.username, input.password, input.ssh_key, input.mount_options, auto_mount, input.proxy_config_id],
        )?;
```

**Step 2: Update MountManager for ProxyJump**

In `src/mount/manager.rs`, update `mount_sshfs` to check for proxy config:

```rust
    fn mount_sshfs(&self, mount: &Mount) -> Result<()> {
        let mut args = vec![
            mount.remote_path.clone(),
            mount.local_mount_point.clone(),
            "-o".to_string(),
            "ServerAliveInterval=15,ServerAliveCountMax=3".to_string(),
        ];

        // Add ProxyJump if proxy config is set
        if let Some(ref proxy_id) = mount.proxy_config_id {
            let conn = self.db.lock().unwrap();
            if let Ok(pc) = crate::models::proxy_config::ProxyConfig::get_by_id(&conn, proxy_id) {
                drop(conn);
                if !pc.hops.is_empty() {
                    args.push("-o".to_string());
                    args.push(format!("ProxyJump={}", pc.proxy_jump_string()));
                }
            }
        }

        if let Some(ssh_key) = &mount.ssh_key {
            args.push("-o".to_string());
            args.push(format!("IdentityFile={}", ssh_key));
        }
        if let Some(opts) = &mount.mount_options {
            args.push("-o".to_string());
            args.push(opts.clone());
        }

        let output = Command::new("sshfs").args(&args).output()?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(IronweaveError::Internal(format!("sshfs failed: {}", stderr)))
        }
    }
```

**Step 3: Update MountResponse in api/mounts.rs**

Add `proxy_config_id` to `MountResponse` struct and its `From<Mount>` impl:

```rust
pub struct MountResponse {
    // ... existing fields ...
    pub proxy_config_id: Option<String>,
}

impl From<Mount> for MountResponse {
    fn from(m: Mount) -> Self {
        let redacted = m.redacted();
        Self {
            // ... existing fields ...
            proxy_config_id: redacted.proxy_config_id,
        }
    }
}
```

**Step 4: Update Mount tests**

Update `sample_input()` in mount model tests to include `proxy_config_id: None`.

**Step 5: Run tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib --bin ironweave`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/models/mount.rs src/mount/manager.rs src/api/mounts.rs
git commit -m "feat: add proxy_config_id to mounts and ProxyJump support in SSHFS"
```

---

### Task 8: Frontend API client — settings, proxy configs, mount updates

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add TypeScript interfaces**

Add after the existing `CreateMountConfig` interface:

```typescript
export interface Setting {
  key: string;
  value: string;
  category: string;
  updated_at: string;
}

export interface UpsertSetting {
  value: string;
  category?: string;
}

export interface ProxyHop {
  host: string;
  port: number;
  username: string;
  auth_type: 'key' | 'password';
  credential: string | null;
}

export interface ProxyConfigResponse {
  id: string;
  name: string;
  hops: ProxyHop[];
  is_active: boolean;
  created_at: string;
}

export interface CreateProxyConfig {
  name: string;
  hops: ProxyHop[];
}

export interface UpdateProxyConfig {
  name?: string;
  hops?: ProxyHop[];
  is_active?: boolean;
}

export interface TestConnectionResult {
  success: boolean;
  hops_tested?: number;
  failed_hop?: number;
  message?: string;
  error?: string;
}
```

**Step 2: Add `proxy_config_id` to `MountConfig` and `CreateMountConfig`**

```typescript
export interface MountConfig {
  // ... existing fields ...
  proxy_config_id: string | null;
}

export interface CreateMountConfig {
  // ... existing fields ...
  proxy_config_id?: string;
}
```

**Step 3: Add API objects**

Add after the existing `mounts` object:

```typescript
export const settings = {
  list: () => get<Setting[]>('/settings'),
  get: (key: string) => get<Setting>(`/settings/${key}`),
  upsert: (key: string, data: UpsertSetting) => put<Setting>(`/settings/${key}`, data),
  delete: (key: string) => del(`/settings/${key}`),
};

export const proxyConfigs = {
  list: () => get<ProxyConfigResponse[]>('/proxy-configs'),
  get: (id: string) => get<ProxyConfigResponse>(`/proxy-configs/${id}`),
  create: (data: CreateProxyConfig) => post<ProxyConfigResponse>('/proxy-configs', data),
  update: (id: string, data: UpdateProxyConfig) => put<ProxyConfigResponse>(`/proxy-configs/${id}`, data),
  delete: (id: string) => del(`/proxy-configs/${id}`),
  test: (id: string) => post<TestConnectionResult>(`/proxy-configs/${id}/test`, {}),
};
```

**Step 4: Add `put` helper function**

Add after the existing `patch` helper (around line 252):

```typescript
async function put<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json', ...authHeaders() },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    handle401(res);
    throw new Error(`PUT ${path} failed: ${res.status} ${res.statusText}`);
  }
  const text = await res.text();
  return text ? JSON.parse(text) : (undefined as unknown as T);
}
```

**Step 5: Build frontend**

Run: `cd /Users/paddyharker/task2/frontend && npm run build`
Expected: Build succeeds.

**Step 6: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add settings and proxy config API client functions"
```

---

### Task 9: Settings layout component with sidebar nav

**Files:**
- Create: `frontend/src/routes/Settings.svelte`

**Step 1: Create the Settings layout**

Create `frontend/src/routes/Settings.svelte`:

```svelte
<script lang="ts">
  import { push, location } from 'svelte-spa-router';

  const sections = [
    { key: 'general', label: 'General', href: '/settings/general' },
    { key: 'proxies', label: 'Proxies', href: '/settings/proxies' },
    { key: 'api-keys', label: 'API Keys', href: '/settings/api-keys' },
  ];

  let currentLocation = $state('');

  $effect(() => {
    const unsubscribe = location.subscribe((val) => {
      currentLocation = val ?? '';
    });
    return unsubscribe;
  });

  function isActive(href: string): boolean {
    return currentLocation === href;
  }
</script>

<div class="flex gap-6">
  <!-- Settings sidebar -->
  <nav class="w-48 shrink-0 space-y-1">
    <h1 class="text-2xl font-bold text-white mb-4">Settings</h1>
    {#each sections as section}
      <button
        onclick={() => push(section.href)}
        class="block w-full text-left px-3 py-2 rounded-lg text-sm transition-colors {isActive(section.href)
          ? 'bg-purple-600/20 text-purple-400 font-medium'
          : 'text-gray-400 hover:bg-gray-800 hover:text-gray-200'}"
      >
        {section.label}
      </button>
    {/each}
  </nav>

  <!-- Content slot -->
  <div class="flex-1 min-w-0">
    <slot />
  </div>
</div>
```

**Step 2: Commit**

```bash
git add frontend/src/routes/Settings.svelte
git commit -m "feat: settings layout component with sidebar navigation"
```

---

### Task 10: General settings page

**Files:**
- Create: `frontend/src/routes/SettingsGeneral.svelte`

**Step 1: Create the General settings page**

Create `frontend/src/routes/SettingsGeneral.svelte`:

```svelte
<script lang="ts">
  import { settings, type Setting } from '../lib/api';
  import SettingsLayout from './Settings.svelte';

  let settingsList: Setting[] = $state([]);
  let error: string | null = $state(null);
  let success: string | null = $state(null);
  let saving: boolean = $state(false);

  // Editable fields
  let browseRoots: string = $state('');
  let mountBase: string = $state('');
  let idleMinutes: string = $state('');

  async function fetchSettings() {
    try {
      settingsList = await settings.list();
      const general = settingsList.filter((s) => s.category === 'general');
      for (const s of general) {
        if (s.key === 'browse_roots') {
          try { browseRoots = JSON.parse(s.value).join(', '); } catch { browseRoots = s.value; }
        }
        if (s.key === 'mount_base') mountBase = s.value;
        if (s.key === 'idle_unmount_minutes') idleMinutes = s.value;
      }
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch settings';
    }
  }

  $effect(() => { fetchSettings(); });

  async function handleSave() {
    saving = true;
    success = null;
    try {
      const roots = browseRoots.split(',').map((r) => r.trim()).filter(Boolean);
      await settings.upsert('browse_roots', { value: JSON.stringify(roots), category: 'general' });
      if (mountBase.trim()) {
        await settings.upsert('mount_base', { value: mountBase.trim(), category: 'general' });
      }
      if (idleMinutes.trim()) {
        await settings.upsert('idle_unmount_minutes', { value: idleMinutes.trim(), category: 'general' });
      }
      success = 'Settings saved.';
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save settings';
    } finally {
      saving = false;
    }
  }
</script>

<SettingsLayout>
  <div class="space-y-6">
    <div>
      <h2 class="text-lg font-semibold text-white">General</h2>
      <p class="mt-1 text-sm text-gray-400">Filesystem and mount configuration.</p>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
    {/if}
    {#if success}
      <div class="rounded-lg bg-green-900/40 border border-green-700 px-4 py-3 text-green-300 text-sm">{success}</div>
    {/if}

    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <div>
        <label for="browse-roots" class="block text-sm font-medium text-gray-400 mb-1">Browse Roots</label>
        <input
          id="browse-roots"
          type="text"
          bind:value={browseRoots}
          placeholder="/home/paddy, /opt"
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
        />
        <p class="mt-1 text-xs text-gray-500">Comma-separated list of directories the file browser can access.</p>
      </div>

      <div>
        <label for="mount-base" class="block text-sm font-medium text-gray-400 mb-1">Mount Base Directory</label>
        <input
          id="mount-base"
          type="text"
          bind:value={mountBase}
          placeholder="/home/paddy/ironweave/mounts"
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
        />
      </div>

      <div>
        <label for="idle-minutes" class="block text-sm font-medium text-gray-400 mb-1">Idle Unmount (minutes)</label>
        <input
          id="idle-minutes"
          type="number"
          min="0"
          bind:value={idleMinutes}
          placeholder="30"
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
        />
        <p class="mt-1 text-xs text-gray-500">Automatically unmount after this many minutes of no active agent sessions. 0 to disable.</p>
      </div>

      <div class="flex justify-end">
        <button
          onclick={handleSave}
          disabled={saving}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
        >
          {saving ? 'Saving...' : 'Save'}
        </button>
      </div>
    </div>
  </div>
</SettingsLayout>
```

**Step 2: Build and commit**

```bash
cd /Users/paddyharker/task2/frontend && npm run build
git add frontend/src/routes/SettingsGeneral.svelte
git commit -m "feat: general settings page with browse roots, mount base, idle timeout"
```

---

### Task 11: Proxies settings page

**Files:**
- Create: `frontend/src/routes/SettingsProxies.svelte`

**Step 1: Create the Proxies settings page**

Create `frontend/src/routes/SettingsProxies.svelte`:

```svelte
<script lang="ts">
  import { proxyConfigs, type ProxyConfigResponse, type ProxyHop, type TestConnectionResult } from '../lib/api';
  import SettingsLayout from './Settings.svelte';

  let configList: ProxyConfigResponse[] = $state([]);
  let error: string | null = $state(null);
  let showForm: boolean = $state(false);
  let editing: string | null = $state(null);
  let saving: boolean = $state(false);
  let testing: string | null = $state(null);
  let testResult: TestConnectionResult | null = $state(null);

  // Form fields
  let formName: string = $state('');
  let formHops: ProxyHop[] = $state([]);

  async function fetchConfigs() {
    try {
      configList = await proxyConfigs.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch proxy configs';
    }
  }

  $effect(() => { fetchConfigs(); });

  function resetForm() {
    formName = '';
    formHops = [];
    editing = null;
    showForm = false;
  }

  function addHop() {
    formHops = [...formHops, { host: '', port: 22, username: '', auth_type: 'key', credential: null }];
  }

  function removeHop(index: number) {
    formHops = formHops.filter((_, i) => i !== index);
  }

  function startEdit(pc: ProxyConfigResponse) {
    formName = pc.name;
    formHops = pc.hops.map((h) => ({ ...h, credential: h.credential === '***' ? null : h.credential }));
    editing = pc.id;
    showForm = true;
  }

  async function handleSave() {
    if (!formName.trim()) return;
    saving = true;
    try {
      if (editing) {
        await proxyConfigs.update(editing, { name: formName.trim(), hops: formHops });
      } else {
        await proxyConfigs.create({ name: formName.trim(), hops: formHops });
      }
      resetForm();
      await fetchConfigs();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save proxy config';
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    if (!confirm('Delete this proxy configuration?')) return;
    try {
      await proxyConfigs.delete(id);
      await fetchConfigs();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete proxy config';
    }
  }

  async function handleTest(id: string) {
    testing = id;
    testResult = null;
    try {
      testResult = await proxyConfigs.test(id);
    } catch (e) {
      testResult = { success: false, error: e instanceof Error ? e.message : 'Test failed' };
    } finally {
      testing = null;
    }
  }

  async function handleToggle(pc: ProxyConfigResponse) {
    try {
      await proxyConfigs.update(pc.id, { is_active: !pc.is_active });
      await fetchConfigs();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to update proxy config';
    }
  }
</script>

<SettingsLayout>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-lg font-semibold text-white">Proxy Configurations</h2>
        <p class="mt-1 text-sm text-gray-400">SSH proxy chains for reaching remote hosts through tunnels.</p>
      </div>
      <button
        onclick={() => { if (showForm) resetForm(); else { showForm = true; addHop(); } }}
        class="px-3 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
      >
        {showForm ? 'Cancel' : 'Add Proxy'}
      </button>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
    {/if}

    {#if testResult}
      <div class="rounded-lg px-4 py-3 text-sm {testResult.success ? 'bg-green-900/40 border border-green-700 text-green-300' : 'bg-red-900/40 border border-red-700 text-red-300'}">
        {testResult.success ? testResult.message : testResult.error}
      </div>
    {/if}

    {#if showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
        <h3 class="text-sm font-semibold text-white">{editing ? 'Edit Proxy' : 'New Proxy'}</h3>

        <div>
          <label for="proxy-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
          <input
            id="proxy-name"
            type="text"
            bind:value={formName}
            placeholder="cuk-proxy-chain"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          />
        </div>

        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-sm font-medium text-gray-400">Hops</span>
            <button
              onclick={addHop}
              class="text-xs text-purple-400 hover:text-purple-300 transition-colors"
            >
              + Add Hop
            </button>
          </div>

          {#each formHops as hop, i}
            <div class="rounded-lg bg-gray-800 border border-gray-700 p-3 space-y-2">
              <div class="flex items-center justify-between">
                <span class="text-xs text-gray-500">Hop {i + 1}</span>
                <button onclick={() => removeHop(i)} class="text-xs text-red-400 hover:text-red-300">&times;</button>
              </div>
              <div class="grid grid-cols-2 md:grid-cols-4 gap-2">
                <input type="text" bind:value={hop.host} placeholder="10.0.0.1"
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
                <input type="number" bind:value={hop.port} min="1" max="65535"
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
                <input type="text" bind:value={hop.username} placeholder="username"
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
                <select bind:value={hop.auth_type}
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500">
                  <option value="key">SSH Key</option>
                  <option value="password">Password</option>
                </select>
              </div>
              {#if hop.auth_type === 'password'}
                <input type="password" bind:value={hop.credential} placeholder="Password"
                  class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
              {/if}
            </div>
          {/each}
        </div>

        <div class="flex justify-end">
          <button
            onclick={handleSave}
            disabled={saving || !formName.trim() || formHops.length === 0}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
          >
            {saving ? 'Saving...' : editing ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    {/if}

    {#if configList.length === 0 && !showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
        No proxy configurations yet.
      </div>
    {:else}
      <div class="space-y-3">
        {#each configList as pc (pc.id)}
          <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-3">
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2">
                <h3 class="text-sm font-semibold text-white">{pc.name}</h3>
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {pc.is_active ? 'bg-green-600 text-green-100' : 'bg-gray-600 text-gray-100'}">
                  {pc.is_active ? 'Active' : 'Inactive'}
                </span>
                <span class="text-xs text-gray-500">{pc.hops.length} hop{pc.hops.length !== 1 ? 's' : ''}</span>
              </div>
              <div class="flex items-center gap-2">
                <button
                  onclick={() => handleToggle(pc)}
                  class="px-2 py-1 text-xs rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
                >
                  {pc.is_active ? 'Disable' : 'Enable'}
                </button>
                <button
                  onclick={() => handleTest(pc.id)}
                  disabled={testing === pc.id}
                  class="px-2 py-1 text-xs rounded-lg bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 transition-colors"
                >
                  {testing === pc.id ? 'Testing...' : 'Test'}
                </button>
                <button
                  onclick={() => startEdit(pc)}
                  class="px-2 py-1 text-xs rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
                >
                  Edit
                </button>
                <button
                  onclick={() => handleDelete(pc.id)}
                  class="px-2 py-1 text-xs rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors"
                >
                  Delete
                </button>
              </div>
            </div>

            <div class="flex items-center gap-1 text-xs text-gray-500 font-mono">
              {#each pc.hops as hop, i}
                {#if i > 0}<span class="text-gray-600">&rarr;</span>{/if}
                <span>{hop.username}@{hop.host}:{hop.port}</span>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</SettingsLayout>
```

**Step 2: Build and commit**

```bash
cd /Users/paddyharker/task2/frontend && npm run build
git add frontend/src/routes/SettingsProxies.svelte
git commit -m "feat: proxy configurations settings page with hop builder and test connection"
```

---

### Task 12: API Keys settings page

**Files:**
- Create: `frontend/src/routes/SettingsApiKeys.svelte`

**Step 1: Create the API Keys page**

Create `frontend/src/routes/SettingsApiKeys.svelte`:

```svelte
<script lang="ts">
  import { settings, type Setting } from '../lib/api';
  import SettingsLayout from './Settings.svelte';

  let apiKeys: Setting[] = $state([]);
  let error: string | null = $state(null);
  let showForm: boolean = $state(false);
  let saving: boolean = $state(false);

  let newKeyName: string = $state('');
  let newKeyValue: string = $state('');

  async function fetchKeys() {
    try {
      const all = await settings.list();
      apiKeys = all.filter((s) => s.category === 'api_keys');
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch API keys';
    }
  }

  $effect(() => { fetchKeys(); });

  async function handleCreate() {
    if (!newKeyName.trim() || !newKeyValue.trim()) return;
    saving = true;
    try {
      const key = `apikey_${newKeyName.trim().toLowerCase().replace(/[^a-z0-9_]/g, '_')}`;
      await settings.upsert(key, { value: newKeyValue.trim(), category: 'api_keys' });
      newKeyName = '';
      newKeyValue = '';
      showForm = false;
      await fetchKeys();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save API key';
    } finally {
      saving = false;
    }
  }

  async function handleDelete(key: string) {
    if (!confirm('Delete this API key?')) return;
    try {
      await settings.delete(key);
      await fetchKeys();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete API key';
    }
  }

  function displayName(key: string): string {
    return key.replace(/^apikey_/, '').replace(/_/g, ' ');
  }
</script>

<SettingsLayout>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-lg font-semibold text-white">API Keys</h2>
        <p class="mt-1 text-sm text-gray-400">Store API keys for agent runtimes and integrations.</p>
      </div>
      <button
        onclick={() => showForm = !showForm}
        class="px-3 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
      >
        {showForm ? 'Cancel' : 'Add Key'}
      </button>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
    {/if}

    {#if showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label for="key-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
            <input
              id="key-name"
              type="text"
              bind:value={newKeyName}
              placeholder="anthropic_api"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
            />
          </div>
          <div>
            <label for="key-value" class="block text-sm font-medium text-gray-400 mb-1">Value</label>
            <input
              id="key-value"
              type="password"
              bind:value={newKeyValue}
              placeholder="sk-..."
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
            />
          </div>
        </div>
        <div class="flex justify-end">
          <button
            onclick={handleCreate}
            disabled={saving || !newKeyName.trim() || !newKeyValue.trim()}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
          >
            {saving ? 'Saving...' : 'Save'}
          </button>
        </div>
      </div>
    {/if}

    {#if apiKeys.length === 0 && !showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
        No API keys stored yet.
      </div>
    {:else}
      <div class="space-y-2">
        {#each apiKeys as key (key.key)}
          <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 flex items-center justify-between">
            <div>
              <span class="text-sm font-medium text-white">{displayName(key.key)}</span>
              <span class="ml-2 text-xs text-gray-500 font-mono">{key.value}</span>
            </div>
            <button
              onclick={() => handleDelete(key.key)}
              class="px-2 py-1 text-xs rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors"
            >
              Delete
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</SettingsLayout>
```

**Step 2: Build and commit**

```bash
cd /Users/paddyharker/task2/frontend && npm run build
git add frontend/src/routes/SettingsApiKeys.svelte
git commit -m "feat: API keys settings page with add, list, and delete"
```

---

### Task 13: Wire routes and nav into App.svelte

**Files:**
- Modify: `frontend/src/App.svelte`

**Step 1: Add imports**

Add these imports after the existing route imports:

```typescript
  import SettingsGeneral from './routes/SettingsGeneral.svelte';
  import SettingsProxies from './routes/SettingsProxies.svelte';
  import SettingsApiKeys from './routes/SettingsApiKeys.svelte';
```

**Step 2: Add routes**

Add to the `routes` object, after the `/agents` route:

```typescript
    '/settings': SettingsGeneral,
    '/settings/general': SettingsGeneral,
    '/settings/proxies': SettingsProxies,
    '/settings/api-keys': SettingsApiKeys,
```

**Step 3: Add Settings to nav**

Add Settings to the `navItems` array, after Agents:

```typescript
    { href: '/settings', label: 'Settings' },
```

**Step 4: Build frontend**

Run: `cd /Users/paddyharker/task2/frontend && npm run build`
Expected: Build succeeds.

**Step 5: Run all backend tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib --bin ironweave`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add frontend/src/App.svelte
git commit -m "feat: add Settings page with sub-routes to navigation and router"
```

---

### Task 14: Add proxy selector to SSHFS mount creation

**Files:**
- Modify: `frontend/src/routes/Projects.svelte`

**Step 1: Add proxy config imports and state**

Add to the script imports:

```typescript
  import { projects, mounts, proxyConfigs, type Project, type CreateProject, type ProxyConfigResponse } from '../lib/api';
```

Add state variables:

```typescript
  let proxyList: ProxyConfigResponse[] = $state([]);
  let selectedProxy: string = $state('');
```

Add fetch on mount:

```typescript
  async function fetchProxies() {
    try {
      proxyList = await proxyConfigs.list();
    } catch { /* ignore - proxies are optional */ }
  }

  // Add to existing $effect:
  $effect(() => {
    fetchProjects();
    fetchProxies();
  });
```

**Step 2: Add proxy dropdown to SSHFS section**

In the `{:else if sourceType === 'sshfs'}` block, add a proxy selector field after the existing mount options field:

```svelte
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Proxy (optional)</label>
            <select bind:value={selectedProxy}
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500">
              <option value="">Direct connection</option>
              {#each proxyList.filter(p => p.is_active) as pc}
                <option value={pc.id}>{pc.name} ({pc.hops.length} hop{pc.hops.length !== 1 ? 's' : ''})</option>
              {/each}
            </select>
          </div>
```

**Step 3: Pass proxy_config_id in handleCreate**

In the `handleCreate` function, when building `mountData` for non-local sources, add:

```typescript
        if (selectedProxy) mountData.proxy_config_id = selectedProxy;
```

And reset it after create:

```typescript
      selectedProxy = '';
```

**Step 4: Build frontend**

Run: `cd /Users/paddyharker/task2/frontend && npm run build`
Expected: Build succeeds.

**Step 5: Commit**

```bash
git add frontend/src/routes/Projects.svelte
git commit -m "feat: add proxy selector to SSHFS mount creation in project form"
```

---

### Task 15: Final compilation, tests, and verification

**Files:** None (verification only)

**Step 1: Run all backend tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib --bin ironweave`
Expected: All tests pass.

**Step 2: Build frontend**

Run: `cd /Users/paddyharker/task2/frontend && npm run build`
Expected: Build succeeds.

**Step 3: Full cargo build**

Run: `/Users/paddyharker/.cargo/bin/cargo build`
Expected: Compiles successfully.
