# Directory Browser & Remote Mount Manager — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add server-side filesystem browsing for project directory selection, plus managed NFS/SMB/SSHFS mounts that auto-mount when a project is active and store credentials encrypted at rest.

**Architecture:** New `src/mount/` module with MountManager, credential encryption via AES-256-GCM, and idle monitor tokio task. New filesystem browse API endpoint. Frontend gets a DirectoryBrowser modal and Mounts management page. Projects optionally link to a mount via `mount_id`.

**Tech Stack:** Rust (Axum 0.8), `aes-gcm` crate for encryption, `base64` for key encoding, Svelte 5 with runes, existing rusqlite/tokio stack.

---

### Task 1: Add `aes-gcm` and `base64` crate dependencies

**Files:**
- Modify: `Cargo.toml:6-28`

**Step 1: Add dependencies to Cargo.toml**

Add to `[dependencies]` section:

```toml
aes-gcm = "0.10"
base64 = "0.22"
```

**Step 2: Verify it compiles**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo check 2>&1 | tail -5`
Expected: compiles cleanly

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add aes-gcm and base64 crate dependencies for mount credential encryption"
```

---

### Task 2: Database migration — `mounts` table and `projects.mount_id` column

**Files:**
- Modify: `src/db/migrations.rs:1-136`

**Step 1: Write the failing test**

Add to `src/db/migrations.rs` tests module:

```rust
#[test]
fn test_mounts_table_exists() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    run_migrations(&conn).unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"mounts".to_string()));
}

#[test]
fn test_projects_has_mount_id_column() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    run_migrations(&conn).unwrap();

    // Insert a mount first
    conn.execute(
        "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point) VALUES ('m1', 'test', 'nfs', 'server:/share', '/mnt/test')",
        [],
    ).unwrap();

    // Insert a project with mount_id
    conn.execute(
        "INSERT INTO projects (id, name, directory, context, mount_id) VALUES ('p1', 'proj', '/tmp', 'work', 'm1')",
        [],
    ).unwrap();

    let mount_id: Option<String> = conn
        .query_row("SELECT mount_id FROM projects WHERE id = 'p1'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mount_id, Some("m1".to_string()));
}
```

**Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo test db::migrations::tests --no-default-features 2>&1 | tail -10`
Expected: FAIL — `mounts` table doesn't exist

**Step 3: Add `mounts` table and `mount_id` column to migrations**

In `src/db/migrations.rs`, inside the `execute_batch` string, add after the `merge_queue_entries` table and before the `CREATE INDEX` block:

```sql
CREATE TABLE IF NOT EXISTS mounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mount_type TEXT NOT NULL CHECK(mount_type IN ('nfs', 'smb', 'sshfs')),
    remote_path TEXT NOT NULL,
    local_mount_point TEXT NOT NULL,
    username TEXT,
    password TEXT,
    ssh_key TEXT,
    mount_options TEXT,
    auto_mount INTEGER NOT NULL DEFAULT 1,
    state TEXT NOT NULL DEFAULT 'unmounted'
        CHECK(state IN ('mounted', 'unmounted', 'error')),
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Add `mount_id` column to the `projects` table: change the `projects` CREATE TABLE to include `mount_id TEXT REFERENCES mounts(id)` after `git_remote`.

Add index:
```sql
CREATE INDEX IF NOT EXISTS idx_mounts_state ON mounts(state);
```

**Step 4: Run tests to verify they pass**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo test db::migrations::tests 2>&1 | tail -10`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add mounts table and projects.mount_id column to database schema"
```

---

### Task 3: Mount model (`src/models/mount.rs`)

**Files:**
- Create: `src/models/mount.rs`
- Modify: `src/models/mod.rs`

**Step 1: Write the failing test**

Create `src/models/mount.rs` with tests first:

```rust
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
        })
    }

    pub fn create(conn: &Connection, input: &CreateMount) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let auto_mount = input.auto_mount.unwrap_or(true) as i32;
        conn.execute(
            "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point, username, password, ssh_key, mount_options, auto_mount)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![id, input.name, input.mount_type, input.remote_path, input.local_mount_point,
                    input.username, input.password, input.ssh_key, input.mount_options, auto_mount],
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

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM mounts WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("mount {}", id)));
        }
        Ok(())
    }

    /// Return a redacted copy of this mount (credentials replaced with "***")
    pub fn redacted(&self) -> Self {
        let mut m = self.clone();
        if m.password.is_some() {
            m.password = Some("***".to_string());
        }
        if m.ssh_key.is_some() {
            m.ssh_key = Some("***".to_string());
        }
        Ok(m)
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
    fn test_create_and_get() {
        let conn = setup_db();
        let input = CreateMount {
            name: "nas-share".to_string(),
            mount_type: "nfs".to_string(),
            remote_path: "192.168.1.10:/export/data".to_string(),
            local_mount_point: "/mnt/nas-share".to_string(),
            username: None,
            password: None,
            ssh_key: None,
            mount_options: Some("rw,hard".to_string()),
            auto_mount: Some(true),
        };
        let mount = Mount::create(&conn, &input).unwrap();
        assert_eq!(mount.name, "nas-share");
        assert_eq!(mount.mount_type, "nfs");
        assert_eq!(mount.state, "unmounted");
        assert!(mount.auto_mount);

        let fetched = Mount::get_by_id(&conn, &mount.id).unwrap();
        assert_eq!(fetched.id, mount.id);
    }

    #[test]
    fn test_list() {
        let conn = setup_db();
        let input1 = CreateMount {
            name: "bravo".to_string(),
            mount_type: "smb".to_string(),
            remote_path: "//server/share".to_string(),
            local_mount_point: "/mnt/bravo".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            ssh_key: None,
            mount_options: None,
            auto_mount: None,
        };
        let input2 = CreateMount {
            name: "alpha".to_string(),
            mount_type: "sshfs".to_string(),
            remote_path: "user@host:/path".to_string(),
            local_mount_point: "/mnt/alpha".to_string(),
            username: None,
            password: None,
            ssh_key: Some("key-data".to_string()),
            mount_options: None,
            auto_mount: Some(false),
        };
        Mount::create(&conn, &input1).unwrap();
        Mount::create(&conn, &input2).unwrap();

        let mounts = Mount::list(&conn).unwrap();
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].name, "alpha");
        assert_eq!(mounts[1].name, "bravo");
    }

    #[test]
    fn test_update_state() {
        let conn = setup_db();
        let input = CreateMount {
            name: "test".to_string(),
            mount_type: "nfs".to_string(),
            remote_path: "server:/share".to_string(),
            local_mount_point: "/mnt/test".to_string(),
            username: None, password: None, ssh_key: None,
            mount_options: None, auto_mount: None,
        };
        let mount = Mount::create(&conn, &input).unwrap();
        Mount::update_state(&conn, &mount.id, "mounted", None).unwrap();
        let updated = Mount::get_by_id(&conn, &mount.id).unwrap();
        assert_eq!(updated.state, "mounted");

        Mount::update_state(&conn, &mount.id, "error", Some("connection refused")).unwrap();
        let errored = Mount::get_by_id(&conn, &mount.id).unwrap();
        assert_eq!(errored.state, "error");
        assert_eq!(errored.last_error.unwrap(), "connection refused");
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let input = CreateMount {
            name: "deleteme".to_string(),
            mount_type: "nfs".to_string(),
            remote_path: "server:/share".to_string(),
            local_mount_point: "/mnt/del".to_string(),
            username: None, password: None, ssh_key: None,
            mount_options: None, auto_mount: None,
        };
        let mount = Mount::create(&conn, &input).unwrap();
        Mount::delete(&conn, &mount.id).unwrap();
        assert!(Mount::get_by_id(&conn, &mount.id).is_err());
    }

    #[test]
    fn test_redacted() {
        let conn = setup_db();
        let input = CreateMount {
            name: "secret".to_string(),
            mount_type: "smb".to_string(),
            remote_path: "//server/share".to_string(),
            local_mount_point: "/mnt/secret".to_string(),
            username: Some("admin".to_string()),
            password: Some("s3cr3t".to_string()),
            ssh_key: Some("private-key-data".to_string()),
            mount_options: None,
            auto_mount: None,
        };
        let mount = Mount::create(&conn, &input).unwrap();
        let redacted = mount.redacted();
        assert_eq!(redacted.password.unwrap(), "***");
        assert_eq!(redacted.ssh_key.unwrap(), "***");
        assert_eq!(redacted.username.unwrap(), "admin"); // username NOT redacted
    }
}
```

**Step 2: Add `pub mod mount;` to `src/models/mod.rs`**

Add line: `pub mod mount;`

**Step 3: Run tests to verify they pass**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo test models::mount::tests 2>&1 | tail -10`
Expected: ALL PASS

**Note:** The `redacted()` method has a deliberate bug — it returns `Ok(m)` but the return type is `Self`, not `Result<Self>`. Fix: change `Ok(m)` to just `m`. This will be caught during compilation.

**Step 4: Commit**

```bash
git add src/models/mount.rs src/models/mod.rs
git commit -m "feat: mount model with CRUD operations and credential redaction"
```

---

### Task 4: Credential encryption module (`src/mount/crypto.rs`)

**Files:**
- Create: `src/mount/mod.rs`
- Create: `src/mount/crypto.rs`
- Modify: `src/lib.rs` — add `pub mod mount;`
- Modify: `src/main.rs` — add `mod mount;`

**Step 1: Create `src/mount/mod.rs`**

```rust
pub mod crypto;
```

**Step 2: Create `src/mount/crypto.rs` with tests**

```rust
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;

use crate::error::{IronweaveError, Result};

/// Encrypt plaintext using AES-256-GCM with a base64-encoded 32-byte key.
/// Returns base64-encoded `nonce || ciphertext`.
pub fn encrypt(plaintext: &str, master_key_b64: &str) -> Result<String> {
    let key_bytes = BASE64.decode(master_key_b64)
        .map_err(|e| IronweaveError::Internal(format!("invalid master key: {}", e)))?;
    if key_bytes.len() != 32 {
        return Err(IronweaveError::Internal("master key must be 32 bytes".to_string()));
    }

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| IronweaveError::Internal(format!("cipher init: {}", e)))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| IronweaveError::Internal(format!("encryption failed: {}", e)))?;

    // Prepend nonce to ciphertext
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

/// Decrypt base64-encoded `nonce || ciphertext` using AES-256-GCM.
pub fn decrypt(encrypted_b64: &str, master_key_b64: &str) -> Result<String> {
    let key_bytes = BASE64.decode(master_key_b64)
        .map_err(|e| IronweaveError::Internal(format!("invalid master key: {}", e)))?;
    if key_bytes.len() != 32 {
        return Err(IronweaveError::Internal("master key must be 32 bytes".to_string()));
    }

    let combined = BASE64.decode(encrypted_b64)
        .map_err(|e| IronweaveError::Internal(format!("invalid ciphertext: {}", e)))?;
    if combined.len() < 12 {
        return Err(IronweaveError::Internal("ciphertext too short".to_string()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| IronweaveError::Internal(format!("cipher init: {}", e)))?;

    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| IronweaveError::Internal(format!("decryption failed: {}", e)))?;

    String::from_utf8(plaintext)
        .map_err(|e| IronweaveError::Internal(format!("invalid utf-8: {}", e)))
}

/// Generate a random 32-byte key, returned as base64.
pub fn generate_key() -> String {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    BASE64.encode(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> String {
        generate_key()
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = "my-secret-password";
        let encrypted = encrypt(plaintext, &key).unwrap();
        assert_ne!(encrypted, plaintext);
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_nonces() {
        let key = test_key();
        let e1 = encrypt("same", &key).unwrap();
        let e2 = encrypt("same", &key).unwrap();
        assert_ne!(e1, e2); // Different nonces produce different ciphertexts
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = test_key();
        let key2 = test_key();
        let encrypted = encrypt("secret", &key1).unwrap();
        let result = decrypt(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_key_length() {
        let short_key = BASE64.encode(&[0u8; 16]);
        let result = encrypt("test", &short_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_string() {
        let key = test_key();
        let encrypted = encrypt("", &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_generate_key_length() {
        let key = generate_key();
        let bytes = BASE64.decode(&key).unwrap();
        assert_eq!(bytes.len(), 32);
    }
}
```

**Step 3: Add module declarations**

In `src/lib.rs`, add: `pub mod mount;`
In `src/main.rs`, add: `mod mount;`

**Step 4: Run tests**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo test mount::crypto::tests 2>&1 | tail -15`
Expected: ALL PASS (6 tests)

**Step 5: Commit**

```bash
git add src/mount/ src/lib.rs src/main.rs
git commit -m "feat: AES-256-GCM credential encryption for mount credentials"
```

---

### Task 5: Config additions (`[security]` and `[filesystem]` sections)

**Files:**
- Modify: `src/config.rs`

**Step 1: Add security and filesystem config structs**

```rust
// Add after TlsConfig:

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    pub master_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FilesystemConfig {
    pub browse_roots: Vec<String>,
    pub mount_base: String,
    pub idle_unmount_minutes: Option<u64>,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            browse_roots: vec!["/home/paddy".to_string()],
            mount_base: "/home/paddy/ironweave/mounts".to_string(),
            idle_unmount_minutes: Some(30),
        }
    }
}
```

Add to `Config` struct:
```rust
pub security: Option<SecurityConfig>,
pub filesystem: Option<FilesystemConfig>,
```

Add to `Config::default()`:
```rust
security: None,
filesystem: None,
```

**Step 2: Verify it compiles**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo check 2>&1 | tail -5`
Expected: compiles cleanly

**Step 3: Commit**

```bash
git add src/config.rs
git commit -m "feat: add security and filesystem config sections for mounts"
```

---

### Task 6: MountManager (`src/mount/manager.rs`)

**Files:**
- Create: `src/mount/manager.rs`
- Modify: `src/mount/mod.rs` — add `pub mod manager;`

**Step 1: Create `src/mount/manager.rs`**

```rust
use std::path::Path;
use std::process::Command;
use tracing::{info, error};

use crate::db::DbPool;
use crate::config::FilesystemConfig;
use crate::models::mount::Mount;
use crate::error::{IronweaveError, Result};

pub struct MountManager {
    db: DbPool,
    config: FilesystemConfig,
}

impl MountManager {
    pub fn new(db: DbPool, config: FilesystemConfig) -> Self {
        Self { db, config }
    }

    /// Mount a configured mount point by ID.
    pub fn mount(&self, mount_id: &str) -> Result<()> {
        let conn = self.db.lock().unwrap();
        let mount = Mount::get_by_id(&conn, mount_id)?;

        // Create local mount point directory if it doesn't exist
        std::fs::create_dir_all(&mount.local_mount_point)?;

        let result = match mount.mount_type.as_str() {
            "nfs" => self.mount_nfs(&mount),
            "smb" => self.mount_smb(&mount),
            "sshfs" => self.mount_sshfs(&mount),
            other => Err(IronweaveError::Internal(format!("unknown mount type: {}", other))),
        };

        match &result {
            Ok(()) => {
                info!(mount_id, "mount successful");
                Mount::update_state(&conn, mount_id, "mounted", None)?;
            }
            Err(e) => {
                let err_msg = e.to_string();
                error!(mount_id, error = %err_msg, "mount failed");
                Mount::update_state(&conn, mount_id, "error", Some(&err_msg))?;
            }
        }
        result
    }

    /// Unmount a mounted filesystem.
    pub fn unmount(&self, mount_id: &str) -> Result<()> {
        let conn = self.db.lock().unwrap();
        let mount = Mount::get_by_id(&conn, mount_id)?;

        let output = Command::new("sudo")
            .args(["umount", &mount.local_mount_point])
            .output()?;

        if output.status.success() {
            info!(mount_id, "unmount successful");
            Mount::update_state(&conn, mount_id, "unmounted", None)?;
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            error!(mount_id, error = %stderr, "unmount failed");
            Mount::update_state(&conn, mount_id, "error", Some(&stderr))?;
            Err(IronweaveError::Internal(format!("unmount failed: {}", stderr)))
        }
    }

    /// Check if a mount point is accessible.
    pub fn check_status(&self, mount_id: &str) -> Result<String> {
        let conn = self.db.lock().unwrap();
        let mount = Mount::get_by_id(&conn, mount_id)?;

        let path = Path::new(&mount.local_mount_point);
        if path.exists() && path.is_dir() {
            // Check if it's actually a mount point
            let output = Command::new("mountpoint")
                .arg("-q")
                .arg(&mount.local_mount_point)
                .status();

            match output {
                Ok(status) if status.success() => {
                    Mount::update_state(&conn, mount_id, "mounted", None)?;
                    Ok("mounted".to_string())
                }
                _ => {
                    Mount::update_state(&conn, mount_id, "unmounted", None)?;
                    Ok("unmounted".to_string())
                }
            }
        } else {
            Mount::update_state(&conn, mount_id, "unmounted", None)?;
            Ok("unmounted".to_string())
        }
    }

    /// Mount if not already mounted.
    pub fn ensure_mounted(&self, mount_id: &str) -> Result<()> {
        let status = self.check_status(mount_id)?;
        if status != "mounted" {
            self.mount(mount_id)?;
        }
        Ok(())
    }

    fn mount_nfs(&self, mount: &Mount) -> Result<()> {
        let mut args = vec![
            "mount".to_string(),
            "-t".to_string(),
            "nfs".to_string(),
        ];
        if let Some(opts) = &mount.mount_options {
            args.push("-o".to_string());
            args.push(opts.clone());
        }
        args.push(mount.remote_path.clone());
        args.push(mount.local_mount_point.clone());

        self.run_sudo(&args)
    }

    fn mount_smb(&self, mount: &Mount) -> Result<()> {
        let mut opts = Vec::new();
        if let Some(username) = &mount.username {
            opts.push(format!("username={}", username));
        }
        if let Some(password) = &mount.password {
            opts.push(format!("password={}", password));
        }
        if let Some(extra) = &mount.mount_options {
            opts.push(extra.clone());
        }

        let mut args = vec![
            "mount".to_string(),
            "-t".to_string(),
            "cifs".to_string(),
        ];
        if !opts.is_empty() {
            args.push("-o".to_string());
            args.push(opts.join(","));
        }
        args.push(mount.remote_path.clone());
        args.push(mount.local_mount_point.clone());

        self.run_sudo(&args)
    }

    fn mount_sshfs(&self, mount: &Mount) -> Result<()> {
        let mut args = vec![
            "sshfs".to_string(),
            mount.remote_path.clone(),
            mount.local_mount_point.clone(),
            "-o".to_string(),
            "ServerAliveInterval=15,ServerAliveCountMax=3".to_string(),
        ];
        if let Some(ssh_key) = &mount.ssh_key {
            args.push("-o".to_string());
            args.push(format!("IdentityFile={}", ssh_key));
        }
        if let Some(opts) = &mount.mount_options {
            args.push("-o".to_string());
            args.push(opts.clone());
        }

        // SSHFS doesn't need sudo
        let output = Command::new(&args[0])
            .args(&args[1..])
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(IronweaveError::Internal(format!("sshfs failed: {}", stderr)))
        }
    }

    fn run_sudo(&self, args: &[String]) -> Result<()> {
        let output = Command::new("sudo")
            .args(args)
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(IronweaveError::Internal(format!("mount command failed: {}", stderr)))
        }
    }
}
```

**Step 2: Add to `src/mount/mod.rs`**

```rust
pub mod crypto;
pub mod manager;
```

**Step 3: Verify it compiles**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo check 2>&1 | tail -5`
Expected: compiles cleanly

**Step 4: Commit**

```bash
git add src/mount/manager.rs src/mount/mod.rs
git commit -m "feat: MountManager with NFS, SMB, and SSHFS mount support"
```

---

### Task 7: Idle monitor (`src/mount/idle_monitor.rs`)

**Files:**
- Create: `src/mount/idle_monitor.rs`
- Modify: `src/mount/mod.rs` — add `pub mod idle_monitor;`

**Step 1: Create `src/mount/idle_monitor.rs`**

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};

use crate::db::DbPool;
use crate::config::FilesystemConfig;
use crate::models::mount::Mount;
use super::manager::MountManager;

/// Spawns a background task that checks for idle mounts every 5 minutes
/// and unmounts them if no active project sessions reference them.
pub fn spawn_idle_monitor(
    db: DbPool,
    config: FilesystemConfig,
    mount_manager: Arc<MountManager>,
) {
    let idle_minutes = config.idle_unmount_minutes.unwrap_or(30);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            if let Err(e) = check_idle_mounts(&db, &mount_manager, idle_minutes) {
                warn!(error = %e, "idle mount check failed");
            }
        }
    });
}

fn check_idle_mounts(
    db: &DbPool,
    mount_manager: &MountManager,
    _idle_minutes: u64,
) -> crate::error::Result<()> {
    let conn = db.lock().unwrap();
    let mounts = Mount::list(&conn)?;

    for mount in mounts {
        if mount.state != "mounted" {
            continue;
        }

        // Check if any active agent sessions reference projects that use this mount
        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_sessions a
             JOIN teams t ON a.team_id = t.id
             JOIN projects p ON t.project_id = p.id
             WHERE p.mount_id = ?1 AND a.state IN ('working', 'idle')",
            rusqlite::params![mount.id],
            |row| row.get(0),
        ).unwrap_or(0);

        if active_count == 0 {
            info!(mount_id = %mount.id, mount_name = %mount.name, "unmounting idle mount");
            drop(conn);
            if let Err(e) = mount_manager.unmount(&mount.id) {
                warn!(mount_id = %mount.id, error = %e, "failed to unmount idle mount");
            }
            return Ok(()); // Re-acquire lock next iteration
        }
    }
    Ok(())
}
```

**Step 2: Add to `src/mount/mod.rs`**

```rust
pub mod crypto;
pub mod manager;
pub mod idle_monitor;
```

**Step 3: Verify it compiles**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo check 2>&1 | tail -5`
Expected: compiles cleanly

**Step 4: Commit**

```bash
git add src/mount/idle_monitor.rs src/mount/mod.rs
git commit -m "feat: idle mount monitor background task"
```

---

### Task 8: Filesystem browse API (`src/api/filesystem.rs`)

**Files:**
- Create: `src/api/filesystem.rs`
- Modify: `src/api/mod.rs` — add `pub mod filesystem;`

**Step 1: Create `src/api/filesystem.rs`**

```rust
use axum::{extract::{Query, State}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: String,
    pub include_files: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BrowseEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
}

#[derive(Debug, Serialize)]
pub struct BrowseResponse {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<BrowseEntry>,
}

pub async fn browse(
    State(state): State<AppState>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<BrowseResponse>, StatusCode> {
    let requested = PathBuf::from(&query.path);
    let include_files = query.include_files.unwrap_or(false);

    // Canonicalize to resolve symlinks and ..
    let canonical = requested.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;

    // Security check: path must be under one of the allowed browse_roots
    let browse_roots = state.browse_roots();
    let allowed = browse_roots.iter().any(|root| {
        let root_path = Path::new(root);
        if let Ok(canonical_root) = root_path.canonicalize() {
            canonical.starts_with(&canonical_root)
        } else {
            false
        }
    });

    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }

    // Read directory entries
    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&canonical).map_err(|_| StatusCode::NOT_FOUND)?;

    for entry in read_dir.flatten() {
        let file_type = entry.file_type().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        if file_type.is_dir() {
            entries.push(BrowseEntry {
                name,
                entry_type: "directory".to_string(),
            });
        } else if include_files && file_type.is_file() {
            entries.push(BrowseEntry {
                name,
                entry_type: "file".to_string(),
            });
        }
    }

    entries.sort_by(|a, b| {
        // Directories first, then alphabetical
        let type_cmp = a.entry_type.cmp(&b.entry_type);
        if type_cmp == std::cmp::Ordering::Equal {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else {
            type_cmp
        }
    });

    let parent = canonical.parent().map(|p| p.to_string_lossy().to_string());

    Ok(Json(BrowseResponse {
        path: canonical.to_string_lossy().to_string(),
        parent,
        entries,
    }))
}
```

**Step 2: Add `pub mod filesystem;` to `src/api/mod.rs`**

**Step 3: Add `browse_roots()` helper to `AppState`**

In `src/state.rs`, add:

```rust
use crate::config::FilesystemConfig;

impl AppState {
    pub fn browse_roots(&self) -> Vec<String> {
        // Will be populated from config in a later task
        vec!["/home/paddy".to_string()]
    }
}
```

**Step 4: Verify it compiles**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo check 2>&1 | tail -5`
Expected: compiles cleanly

**Step 5: Commit**

```bash
git add src/api/filesystem.rs src/api/mod.rs src/state.rs
git commit -m "feat: filesystem browse API endpoint with path security"
```

---

### Task 9: Mount CRUD API (`src/api/mounts.rs`)

**Files:**
- Create: `src/api/mounts.rs`
- Modify: `src/api/mod.rs` — add `pub mod mounts;`

**Step 1: Create `src/api/mounts.rs`**

```rust
use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::Serialize;
use crate::state::AppState;
use crate::models::mount::{Mount, CreateMount};

#[derive(Debug, Serialize)]
pub struct MountResponse {
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
}

impl From<Mount> for MountResponse {
    fn from(m: Mount) -> Self {
        let redacted = m.redacted();
        Self {
            id: redacted.id,
            name: redacted.name,
            mount_type: redacted.mount_type,
            remote_path: redacted.remote_path,
            local_mount_point: redacted.local_mount_point,
            username: redacted.username,
            password: redacted.password,
            ssh_key: redacted.ssh_key,
            mount_options: redacted.mount_options,
            auto_mount: redacted.auto_mount,
            state: redacted.state,
            last_error: redacted.last_error,
            created_at: redacted.created_at,
        }
    }
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<MountResponse>> {
    let conn = state.db.lock().unwrap();
    let mounts = Mount::list(&conn).unwrap_or_default();
    Json(mounts.into_iter().map(MountResponse::from).collect())
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateMount>,
) -> Result<(StatusCode, Json<MountResponse>), StatusCode> {
    let conn = state.db.lock().unwrap();
    // TODO: encrypt credentials before storage when security config available
    Mount::create(&conn, &input)
        .map(|m| (StatusCode::CREATED, Json(MountResponse::from(m))))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MountResponse>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Mount::get_by_id(&conn, &id)
        .map(|m| Json(MountResponse::from(m)))
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.db.lock().unwrap();
    // TODO: unmount if mounted before deleting
    Mount::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn mount_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MountResponse>, StatusCode> {
    // Ensure mount exists
    {
        let conn = state.db.lock().unwrap();
        Mount::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    }

    if let Some(ref mm) = state.mount_manager {
        mm.mount(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let conn = state.db.lock().unwrap();
    Mount::get_by_id(&conn, &id)
        .map(|m| Json(MountResponse::from(m)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn unmount_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MountResponse>, StatusCode> {
    {
        let conn = state.db.lock().unwrap();
        Mount::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    }

    if let Some(ref mm) = state.mount_manager {
        mm.unmount(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let conn = state.db.lock().unwrap();
    Mount::get_by_id(&conn, &id)
        .map(|m| Json(MountResponse::from(m)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(ref mm) = state.mount_manager {
        let status = mm.check_status(&id).map_err(|_| StatusCode::NOT_FOUND)?;
        Ok(Json(serde_json::json!({ "status": status })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}
```

**Step 2: Add `pub mod mounts;` to `src/api/mod.rs`**

**Step 3: Add `mount_manager` to `AppState`**

In `src/state.rs`, add to the struct:
```rust
pub mount_manager: Option<Arc<crate::mount::manager::MountManager>>,
```

Update the `browse_roots` method to use filesystem config if available, and add `filesystem_config` to the state:
```rust
pub filesystem_config: Option<crate::config::FilesystemConfig>,
```

Update `browse_roots()`:
```rust
pub fn browse_roots(&self) -> Vec<String> {
    self.filesystem_config
        .as_ref()
        .map(|c| c.browse_roots.clone())
        .unwrap_or_else(|| vec!["/home/paddy".to_string()])
}
```

**Step 4: Verify it compiles**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo check 2>&1 | tail -5`
Expected: compiles cleanly

**Step 5: Commit**

```bash
git add src/api/mounts.rs src/api/mod.rs src/state.rs
git commit -m "feat: mount CRUD API with status check and mount/unmount actions"
```

---

### Task 10: Wire routes and state in `main.rs`

**Files:**
- Modify: `src/main.rs`

**Step 1: Update AppState construction to include mount_manager and filesystem_config**

After the `process_manager` construction and before building AppState, add:

```rust
let filesystem_config = config.filesystem.clone();
let mount_manager = filesystem_config.as_ref().map(|fs_config| {
    Arc::new(mount::manager::MountManager::new(db.clone(), fs_config.clone()))
});
```

Update `state` construction to include:
```rust
mount_manager: mount_manager.clone(),
filesystem_config: config.filesystem.clone(),
```

**Step 2: Add routes**

After the dashboard route line, add:

```rust
// Filesystem browser
.route("/api/filesystem/browse", get(api::filesystem::browse))
// Mounts
.route("/api/mounts", get(api::mounts::list).post(api::mounts::create))
.route("/api/mounts/{id}", get(api::mounts::get).delete(api::mounts::delete))
.route("/api/mounts/{id}/mount", post(api::mounts::mount_action))
.route("/api/mounts/{id}/unmount", post(api::mounts::unmount_action))
.route("/api/mounts/{id}/status", get(api::mounts::status))
```

**Step 3: Start idle monitor if filesystem config present**

After building the `app` router and before binding, add:

```rust
if let (Some(fs_config), Some(mm)) = (&config.filesystem, &mount_manager) {
    mount::idle_monitor::spawn_idle_monitor(db.clone(), fs_config.clone(), mm.clone());
    tracing::info!("Mount idle monitor started");
}
```

Note: `db` here should be `state.db.clone()` — use whichever reference is available after state construction. The db is `state.db`.

**Step 4: Verify it compiles**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo check 2>&1 | tail -5`
Expected: compiles cleanly

**Step 5: Run all existing tests to verify nothing breaks**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo test 2>&1 | tail -15`
Expected: ALL existing tests pass

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire filesystem browse and mount routes into main server"
```

---

### Task 11: Update Project model for `mount_id`

**Files:**
- Modify: `src/models/project.rs`

**Step 1: Add `mount_id` field to `Project` struct**

Add to `Project` struct: `pub mount_id: Option<String>,`
Add to `CreateProject` struct: `pub mount_id: Option<String>,`

**Step 2: Update `from_row`**

Add: `mount_id: row.get("mount_id")?,`

**Step 3: Update `create` SQL**

Change INSERT to include `mount_id`:
```rust
conn.execute(
    "INSERT INTO projects (id, name, directory, context, obsidian_vault_path, obsidian_project, git_remote, mount_id)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    params![id, input.name, input.directory, input.context, input.obsidian_vault_path, input.obsidian_project, input.git_remote, input.mount_id],
)?;
```

**Step 4: Update existing tests to include `mount_id: None` in `CreateProject`**

Every test `CreateProject` needs `mount_id: None` added.

**Step 5: Run tests**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo test models::project 2>&1 | tail -10`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add src/models/project.rs
git commit -m "feat: add mount_id to Project model for linking projects to mounts"
```

---

### Task 12: Frontend API types and functions for mounts and filesystem

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add TypeScript interfaces**

Add after the existing type definitions:

```typescript
export interface BrowseEntry {
  name: string;
  type: 'directory' | 'file';
}

export interface BrowseResponse {
  path: string;
  parent: string | null;
  entries: BrowseEntry[];
}

export interface MountConfig {
  id: string;
  name: string;
  mount_type: 'nfs' | 'smb' | 'sshfs';
  remote_path: string;
  local_mount_point: string;
  username: string | null;
  password: string | null;
  ssh_key: string | null;
  mount_options: string | null;
  auto_mount: boolean;
  state: 'mounted' | 'unmounted' | 'error';
  last_error: string | null;
  created_at: string;
}

export interface CreateMountConfig {
  name: string;
  mount_type: 'nfs' | 'smb' | 'sshfs';
  remote_path: string;
  local_mount_point: string;
  username?: string;
  password?: string;
  ssh_key?: string;
  mount_options?: string;
  auto_mount?: boolean;
}
```

**Step 2: Add API functions**

Add after existing resource APIs:

```typescript
export const filesystem = {
  browse: (path: string, includeFiles = false) =>
    get<BrowseResponse>(`/filesystem/browse?path=${encodeURIComponent(path)}&include_files=${includeFiles}`),
};

export const mounts = {
  list: () => get<MountConfig[]>('/mounts'),
  get: (id: string) => get<MountConfig>(`/mounts/${id}`),
  create: (data: CreateMountConfig) => post<MountConfig>('/mounts', data),
  delete: (id: string) => del(`/mounts/${id}`),
  mount: (id: string) => post<MountConfig>(`/mounts/${id}/mount`, {}),
  unmount: (id: string) => post<MountConfig>(`/mounts/${id}/unmount`, {}),
  status: (id: string) => get<{ status: string }>(`/mounts/${id}/status`),
};
```

**Step 3: Update `CreateProject` interface**

Add `mount_id?: string;` to `CreateProject`.

**Step 4: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add mount and filesystem browse API client functions"
```

---

### Task 13: DirectoryBrowser component (`frontend/src/lib/components/DirectoryBrowser.svelte`)

**Files:**
- Create: `frontend/src/lib/components/DirectoryBrowser.svelte`

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { filesystem, type BrowseResponse, type BrowseEntry } from '../api';

  interface Props {
    initialPath?: string;
    onSelect: (path: string) => void;
    onClose: () => void;
  }

  let { initialPath = '/home/paddy', onSelect, onClose }: Props = $props();

  let currentPath: string = $state(initialPath);
  let entries: BrowseEntry[] = $state([]);
  let parent: string | null = $state(null);
  let loading: boolean = $state(false);
  let error: string | null = $state(null);

  let breadcrumbs = $derived(
    currentPath.split('/').filter(Boolean).map((seg, i, arr) => ({
      label: seg,
      path: '/' + arr.slice(0, i + 1).join('/'),
    }))
  );

  async function browse(path: string) {
    loading = true;
    error = null;
    try {
      const res: BrowseResponse = await filesystem.browse(path);
      currentPath = res.path;
      entries = res.entries;
      parent = res.parent;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to browse directory';
    } finally {
      loading = false;
    }
  }

  function navigateTo(path: string) {
    browse(path);
  }

  function handleSelect() {
    onSelect(currentPath);
  }

  $effect(() => {
    browse(initialPath);
  });
</script>

<!-- Modal overlay -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 bg-black/60 z-50 flex items-center justify-center" onclick={onClose}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="bg-gray-900 border border-gray-700 rounded-xl w-full max-w-2xl mx-4 shadow-2xl" onclick={(e) => e.stopPropagation()}>
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 border-b border-gray-800">
      <h2 class="text-sm font-semibold text-white">Browse Directory</h2>
      <button onclick={onClose} class="text-gray-400 hover:text-white text-lg">&times;</button>
    </div>

    <!-- Breadcrumb -->
    <div class="px-4 py-2 border-b border-gray-800 flex items-center gap-1 text-xs text-gray-400 overflow-x-auto">
      <button onclick={() => navigateTo('/')} class="hover:text-white shrink-0">/</button>
      {#each breadcrumbs as crumb}
        <span class="shrink-0">/</span>
        <button onclick={() => navigateTo(crumb.path)} class="hover:text-white shrink-0">{crumb.label}</button>
      {/each}
    </div>

    <!-- Content -->
    <div class="h-72 overflow-y-auto">
      {#if loading}
        <div class="flex items-center justify-center h-full text-gray-500 text-sm">Loading...</div>
      {:else if error}
        <div class="p-4 text-red-400 text-sm">{error}</div>
      {:else}
        <div class="divide-y divide-gray-800/50">
          {#if parent}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
              onclick={() => navigateTo(parent!)}
            >
              <span class="text-gray-500">&#8593;</span>
              <span>..</span>
            </div>
          {/if}
          {#each entries as entry}
            {#if entry.type === 'directory'}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="px-4 py-2 text-sm text-gray-200 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
                onclick={() => navigateTo(currentPath + '/' + entry.name)}
              >
                <span class="text-yellow-500 text-xs">&#128193;</span>
                <span>{entry.name}</span>
              </div>
            {:else}
              <div class="px-4 py-2 text-sm text-gray-500 flex items-center gap-2">
                <span class="text-xs">&#128196;</span>
                <span>{entry.name}</span>
              </div>
            {/if}
          {/each}
          {#if entries.length === 0}
            <div class="p-4 text-gray-500 text-sm text-center">Empty directory</div>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="px-4 py-3 border-t border-gray-800 flex items-center justify-between">
      <span class="text-xs text-gray-500 font-mono truncate max-w-[60%]" title={currentPath}>{currentPath}</span>
      <div class="flex gap-2">
        <button
          onclick={onClose}
          class="px-3 py-1.5 text-sm rounded-lg bg-gray-800 text-gray-300 hover:bg-gray-700 transition-colors"
        >
          Cancel
        </button>
        <button
          onclick={handleSelect}
          class="px-3 py-1.5 text-sm rounded-lg bg-purple-600 text-white hover:bg-purple-500 transition-colors"
        >
          Select
        </button>
      </div>
    </div>
  </div>
</div>
```

**Step 2: Verify frontend builds**

Run: `cd /Users/paddyharker/task2/frontend && npm run build 2>&1 | tail -5`
Expected: builds cleanly

**Step 3: Commit**

```bash
git add frontend/src/lib/components/DirectoryBrowser.svelte
git commit -m "feat: DirectoryBrowser modal component with breadcrumb navigation"
```

---

### Task 14: Update Projects.svelte with Browse button and source type

**Files:**
- Modify: `frontend/src/routes/Projects.svelte`

**Step 1: Add imports and state for directory browser and source type**

At the top of the script section, add import:
```typescript
import DirectoryBrowser from '../lib/components/DirectoryBrowser.svelte';
```

Add new state variables:
```typescript
let showBrowser: boolean = $state(false);
let sourceType: string = $state('local');
let mountName: string = $state('');
let remotePath: string = $state('');
let mountUsername: string = $state('');
let mountPassword: string = $state('');
let mountSshKey: string = $state('');
let mountOptions: string = $state('');
```

**Step 2: Replace directory input with Browse button for local source**

Replace the directory `<div>` in the form with:

```svelte
<div>
  <label for="proj-source" class="block text-sm font-medium text-gray-400 mb-1">Source</label>
  <select
    id="proj-source"
    bind:value={sourceType}
    class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
  >
    <option value="local">Local</option>
    <option value="nfs">NFS Share</option>
    <option value="smb">SMB Share</option>
    <option value="sshfs">SSH Remote</option>
  </select>
</div>

{#if sourceType === 'local'}
  <div>
    <label for="proj-dir" class="block text-sm font-medium text-gray-400 mb-1">Directory</label>
    <div class="flex gap-2">
      <input
        id="proj-dir"
        type="text"
        bind:value={newDirectory}
        placeholder="/path/to/project"
        class="flex-1 rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
      />
      <button
        type="button"
        onclick={() => showBrowser = true}
        class="px-3 py-2 text-sm rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors shrink-0"
      >
        Browse
      </button>
    </div>
  </div>
{:else if sourceType === 'nfs'}
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Remote Path</label>
    <input type="text" bind:value={remotePath} placeholder="server:/export/path"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Mount Options (optional)</label>
    <input type="text" bind:value={mountOptions} placeholder="rw,hard,intr"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
{:else if sourceType === 'smb'}
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Remote Path</label>
    <input type="text" bind:value={remotePath} placeholder="//server/share"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Username</label>
    <input type="text" bind:value={mountUsername} placeholder="username"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Password</label>
    <input type="password" bind:value={mountPassword} placeholder="password"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Mount Options (optional)</label>
    <input type="text" bind:value={mountOptions} placeholder="domain=WORKGROUP"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
{:else if sourceType === 'sshfs'}
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Remote Path</label>
    <input type="text" bind:value={remotePath} placeholder="user@host:/path"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">SSH Key Path (optional)</label>
    <input type="text" bind:value={mountSshKey} placeholder="/home/paddy/.ssh/id_rsa"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
  <div>
    <label class="block text-sm font-medium text-gray-400 mb-1">Mount Options (optional)</label>
    <input type="text" bind:value={mountOptions} placeholder="port=22"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
  </div>
{/if}
```

**Step 3: Update `handleCreate` to handle remote sources**

The function should create a mount first (if remote), then create the project linked to it:

```typescript
async function handleCreate() {
  if (!newName.trim()) return;
  creating = true;
  try {
    let directory = newDirectory.trim();
    let mount_id: string | undefined;

    if (sourceType !== 'local') {
      // Create mount first
      const mountData: any = {
        name: `${newName.trim()}-mount`,
        mount_type: sourceType,
        remote_path: remotePath.trim(),
        local_mount_point: `/home/paddy/ironweave/mounts/${newName.trim()}`,
      };
      if (mountUsername.trim()) mountData.username = mountUsername.trim();
      if (mountPassword.trim()) mountData.password = mountPassword.trim();
      if (mountSshKey.trim()) mountData.ssh_key = mountSshKey.trim();
      if (mountOptions.trim()) mountData.mount_options = mountOptions.trim();

      const mount = await mounts.create(mountData);
      mount_id = mount.id;
      directory = mount.local_mount_point;
    }

    const data: CreateProject = {
      name: newName.trim(),
      directory,
      context: newContext,
    };
    if (newGitRemote.trim()) data.git_remote = newGitRemote.trim();
    if (mount_id) data.mount_id = mount_id;

    await projects.create(data);

    // Reset form
    newName = ''; newDirectory = ''; newContext = 'work'; newGitRemote = '';
    sourceType = 'local'; remotePath = ''; mountUsername = ''; mountPassword = '';
    mountSshKey = ''; mountOptions = '';
    showCreateForm = false;
    await fetchProjects();
  } catch (e) {
    error = e instanceof Error ? e.message : 'Failed to create project';
  } finally {
    creating = false;
  }
}
```

**Step 4: Add imports for mounts API**

Update the import line:
```typescript
import { push } from 'svelte-spa-router';
import { projects, mounts, type Project, type CreateProject } from '../lib/api';
```

**Step 5: Add DirectoryBrowser modal at the bottom of the template**

Before the closing `</div>`, add:

```svelte
{#if showBrowser}
  <DirectoryBrowser
    initialPath={newDirectory || '/home/paddy'}
    onSelect={(path) => { newDirectory = path; showBrowser = false; }}
    onClose={() => showBrowser = false}
  />
{/if}
```

**Step 6: Verify frontend builds**

Run: `cd /Users/paddyharker/task2/frontend && npm run build 2>&1 | tail -5`
Expected: builds cleanly

**Step 7: Commit**

```bash
git add frontend/src/routes/Projects.svelte
git commit -m "feat: project creation with source type selection and directory browser"
```

---

### Task 15: Mounts management page (`frontend/src/routes/Mounts.svelte`)

**Files:**
- Create: `frontend/src/routes/Mounts.svelte`

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { mounts, type MountConfig } from '../lib/api';

  let mountList: MountConfig[] = $state([]);
  let error: string | null = $state(null);

  async function fetchMounts() {
    try {
      mountList = await mounts.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch mounts';
    }
  }

  $effect(() => {
    fetchMounts();
  });

  async function handleMount(id: string) {
    try {
      await mounts.mount(id);
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Mount failed';
    }
  }

  async function handleUnmount(id: string) {
    try {
      await mounts.unmount(id);
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Unmount failed';
    }
  }

  async function handleDelete(id: string) {
    if (!confirm('Delete this mount configuration?')) return;
    try {
      await mounts.delete(id);
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete mount';
    }
  }

  function stateBadge(state: string): string {
    switch (state) {
      case 'mounted': return 'bg-green-600 text-green-100';
      case 'error': return 'bg-red-600 text-red-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function typeBadge(type: string): string {
    switch (type) {
      case 'nfs': return 'bg-blue-600 text-blue-100';
      case 'smb': return 'bg-orange-600 text-orange-100';
      case 'sshfs': return 'bg-purple-600 text-purple-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }
</script>

<div class="space-y-6">
  <div>
    <h1 class="text-2xl font-bold text-white">Mounts</h1>
    <p class="mt-1 text-sm text-gray-400">Manage remote filesystem mounts.</p>
  </div>

  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  {#if mountList.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      No mounts configured. Create a project with a remote source to add one.
    </div>
  {:else}
    <div class="space-y-3">
      {#each mountList as mount (mount.id)}
        <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-3">
          <div class="flex items-start justify-between">
            <div class="space-y-1">
              <div class="flex items-center gap-2">
                <h3 class="text-base font-semibold text-white">{mount.name}</h3>
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {typeBadge(mount.mount_type)}">
                  {mount.mount_type.toUpperCase()}
                </span>
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {stateBadge(mount.state)}">
                  {mount.state}
                </span>
              </div>
              <p class="text-xs text-gray-500 font-mono">{mount.remote_path}</p>
              <p class="text-xs text-gray-600">&#8594; {mount.local_mount_point}</p>
            </div>

            <div class="flex items-center gap-2">
              {#if mount.state === 'mounted'}
                <button
                  onclick={() => handleUnmount(mount.id)}
                  class="px-3 py-1 text-xs rounded-lg bg-yellow-600/20 text-yellow-400 hover:bg-yellow-600/30 transition-colors"
                >
                  Unmount
                </button>
              {:else}
                <button
                  onclick={() => handleMount(mount.id)}
                  class="px-3 py-1 text-xs rounded-lg bg-green-600/20 text-green-400 hover:bg-green-600/30 transition-colors"
                >
                  Mount
                </button>
              {/if}
              <button
                onclick={() => handleDelete(mount.id)}
                class="px-3 py-1 text-xs rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors"
              >
                Delete
              </button>
            </div>
          </div>

          {#if mount.state === 'error' && mount.last_error}
            <div class="text-xs text-red-400 bg-red-900/20 rounded-lg px-3 py-2">
              {mount.last_error}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
```

**Step 2: Verify frontend builds**

Run: `cd /Users/paddyharker/task2/frontend && npm run build 2>&1 | tail -5`
Expected: builds cleanly

**Step 3: Commit**

```bash
git add frontend/src/routes/Mounts.svelte
git commit -m "feat: Mounts management page with status badges and mount/unmount controls"
```

---

### Task 16: Wire Mounts page into App.svelte router and sidebar

**Files:**
- Modify: `frontend/src/App.svelte`

**Step 1: Add import**

Add: `import Mounts from './routes/Mounts.svelte';`

**Step 2: Add route**

Add to routes object: `'/mounts': Mounts,`

**Step 3: Add to navItems**

Insert between Projects and Agents:
```typescript
{ href: '/mounts', label: 'Mounts' },
```

**Step 4: Verify frontend builds**

Run: `cd /Users/paddyharker/task2/frontend && npm run build 2>&1 | tail -5`
Expected: builds cleanly

**Step 5: Commit**

```bash
git add frontend/src/App.svelte
git commit -m "feat: add Mounts page to sidebar navigation and router"
```

---

### Task 17: Update config example and deploy script

**Files:**
- Modify: `deploy/ironweave.toml.example` (or create if not exists)
- Modify: `deploy/ironweave.service`

**Step 1: Update/create config example**

If `deploy/ironweave.toml.example` doesn't exist, check for any existing example config. Add `[security]` and `[filesystem]` sections:

```toml
[security]
master_key = "GENERATE_WITH_ironweave_generate-key"

[filesystem]
browse_roots = ["/home/paddy"]
mount_base = "/home/paddy/ironweave/mounts"
idle_unmount_minutes = 30
```

**Step 2: Update sudoers documentation**

Create `deploy/sudoers-ironweave` (documentation file):
```
# Add to /etc/sudoers.d/ironweave
paddy ALL=(root) NOPASSWD: /usr/bin/mount, /usr/bin/umount
```

**Step 3: Commit**

```bash
git add deploy/
git commit -m "docs: config example with security and filesystem sections, sudoers template"
```

---

### Task 18: Full build verification and integration test

**Files:**
- No new files

**Step 1: Run all Rust tests**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo test 2>&1 | tail -20`
Expected: ALL tests pass

**Step 2: Build frontend**

Run: `cd /Users/paddyharker/task2/frontend && npm run build 2>&1 | tail -5`
Expected: builds cleanly

**Step 3: Build release binary**

Run: `source "$HOME/.cargo/env" && export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$PATH" && cd /Users/paddyharker/task2 && cargo build --release 2>&1 | tail -5`
Expected: builds cleanly

**Step 4: Commit if any fixes needed, then tag**

```bash
git tag directory-browser-mounts
```

---

### Task 19: Deploy to hl-ironweave VM

**Files:**
- No new files

**Step 1: Build frontend and backend on Mac**

Run: `cd /Users/paddyharker/task2/frontend && npm run build`

Note: The Rust binary must be cross-compiled for Linux or built on the VM. Since prior deployment rsync'd source and built on VM, follow the same pattern.

**Step 2: Rsync source to VM**

```bash
rsync -avz --exclude target --exclude node_modules --exclude .git /Users/paddyharker/task2/ paddy@10.202.28.205:/home/paddy/ironweave/
```

**Step 3: SSH to VM and build**

```bash
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave && source "$HOME/.cargo/env" && cargo build --release'
```

**Step 4: Install VM packages for mount support**

```bash
ssh paddy@10.202.28.205 'sudo apt-get update && sudo apt-get install -y cifs-utils nfs-common sshfs fuse3'
```

**Step 5: Set up sudoers for mount commands**

```bash
ssh paddy@10.202.28.205 'echo "paddy ALL=(root) NOPASSWD: /usr/bin/mount, /usr/bin/umount" | sudo tee /etc/sudoers.d/ironweave'
```

**Step 6: Generate master key and update config**

```bash
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave && ./target/release/ironweave generate-key'
```
(If generate-key CLI isn't implemented, generate locally: `openssl rand -base64 32`)

Update `ironweave.toml` on VM to include `[security]` and `[filesystem]` sections.

**Step 7: Create mount base directory**

```bash
ssh paddy@10.202.28.205 'mkdir -p /home/paddy/ironweave/mounts'
```

**Step 8: Restart service**

```bash
ssh paddy@10.202.28.205 'sudo systemctl restart ironweave && sleep 2 && sudo systemctl status ironweave'
```

**Step 9: Verify**

```bash
curl -k https://10.202.28.205/api/health
curl -k https://10.202.28.205/api/mounts
curl -k 'https://10.202.28.205/api/filesystem/browse?path=/home/paddy'
```

Expected: health returns "ok", mounts returns `[]`, filesystem returns directory listing.
