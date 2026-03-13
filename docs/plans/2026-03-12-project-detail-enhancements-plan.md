# Project Detail Enhancements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add rsync-based file sync with jj version control, a read-only file browser, and editable project details to the ProjectDetail page.

**Architecture:** New SyncManager backend module runs rsync and jj via `Command` (same pattern as MountManager). Frontend gets three new tabs (Files, History, Settings) on the ProjectDetail page. File browsing reads from local synced copy, falling back to SSHFS mount if no sync exists yet.

**Tech Stack:** Rust/Axum 0.8, rusqlite, Svelte 5 (runes), rsync, jj (Jujutsu), existing MountManager

---

### Task 1: DB migration — add new project columns

**Files:**
- Modify: `src/db/migrations.rs:171-174`

**Step 1: Add incremental ALTER TABLE statements**

Add these lines after the existing `ALTER TABLE` statements at line 174 in `src/db/migrations.rs`:

```rust
let _ = conn.execute("ALTER TABLE projects ADD COLUMN description TEXT", []);
let _ = conn.execute("ALTER TABLE projects ADD COLUMN sync_path TEXT", []);
let _ = conn.execute("ALTER TABLE projects ADD COLUMN last_synced_at TEXT", []);
let _ = conn.execute("ALTER TABLE projects ADD COLUMN sync_state TEXT NOT NULL DEFAULT 'idle'", []);
```

**Step 2: Add migration test**

Add a test at the end of the `mod tests` block in `src/db/migrations.rs`:

```rust
#[test]
fn test_projects_has_sync_columns() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO projects (id, name, directory, context, description, sync_path, sync_state) VALUES ('p1', 'proj', '/tmp', 'work', 'A test project', '/sync/p1', 'idle')",
        [],
    ).unwrap();

    let desc: Option<String> = conn
        .query_row("SELECT description FROM projects WHERE id = 'p1'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(desc, Some("A test project".to_string()));

    let sync_state: String = conn
        .query_row("SELECT sync_state FROM projects WHERE id = 'p1'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sync_state, "idle");
}
```

**Step 3: Run tests**

Run: `cargo test db::migrations::tests -- --nocapture`
Expected: All migration tests pass, including the new one.

**Step 4: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add description, sync_path, last_synced_at, sync_state columns to projects"
```

---

### Task 2: Update Project model — new fields + UpdateProject + update method

**Files:**
- Modify: `src/models/project.rs`

**Step 1: Add new fields to the Project struct**

Update the `Project` struct (lines 7-17) to include the new fields. The full struct should be:

```rust
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
}
```

**Step 2: Update `from_row` to read new fields**

Update the `from_row` method (lines 31-43):

```rust
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
    })
}
```

**Step 3: Add UpdateProject struct and update method**

Add after the `CreateProject` struct (after line 28):

```rust
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
}
```

Add the `update` method inside the `impl Project` block, after the `delete` method:

```rust
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

    conn.execute(
        "UPDATE projects SET name = ?1, directory = ?2, context = ?3, description = ?4,
         obsidian_vault_path = ?5, obsidian_project = ?6, git_remote = ?7, mount_id = ?8
         WHERE id = ?9",
        params![name, directory, context, description, obsidian_vault_path, obsidian_project, git_remote, mount_id, id],
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
```

**Step 4: Add tests for update**

Add to the `mod tests` block:

```rust
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
    };
    let updated = Project::update(&conn, &project.id, &update).unwrap();
    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.directory, "/tmp/orig");
    assert_eq!(updated.description, Some("My project description".to_string()));
    assert_eq!(updated.git_remote, Some("https://github.com/test".to_string()));
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
```

**Step 5: Run tests**

Run: `cargo test models::project::tests -- --nocapture`
Expected: All project model tests pass.

**Step 6: Commit**

```bash
git add src/models/project.rs
git commit -m "feat: add UpdateProject, description, sync fields to Project model"
```

---

### Task 3: Project update API endpoint

**Files:**
- Modify: `src/api/projects.rs`
- Modify: `src/main.rs:92`

**Step 1: Add update handler to api/projects.rs**

Add to `src/api/projects.rs` after the existing imports (line 2), update the import to include `UpdateProject`:

```rust
use crate::models::project::{Project, CreateProject, UpdateProject};
```

Add the update handler after the `delete` function:

```rust
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProject>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Project::update(&conn, &id, &input)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

**Step 2: Wire the route in main.rs**

Change line 92 in `src/main.rs` from:

```rust
.route("/api/projects/{id}", get(api::projects::get).delete(api::projects::delete))
```

to:

```rust
.route("/api/projects/{id}", get(api::projects::get).put(api::projects::update).delete(api::projects::delete))
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add src/api/projects.rs src/main.rs
git commit -m "feat: add PUT /api/projects/{id} endpoint for project updates"
```

---

### Task 4: SyncManager — core sync logic

**Files:**
- Create: `src/sync/mod.rs`
- Create: `src/sync/manager.rs`
- Modify: `src/main.rs:1-12` (add `mod sync;`)

**Step 1: Create sync module files**

Create `src/sync/mod.rs`:

```rust
pub mod manager;
```

Create `src/sync/manager.rs`:

```rust
use std::path::Path;
use std::process::Command;
use tracing::{info, warn, error};
use chrono::Utc;

use crate::db::DbPool;
use crate::models::project::Project;
use crate::models::mount::Mount;
use crate::error::{IronweaveError, Result};

pub struct SyncManager {
    db: DbPool,
    sync_base: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncSnapshot {
    pub change_id: String,
    pub description: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncStatus {
    pub sync_state: String,
    pub last_synced_at: Option<String>,
    pub sync_path: Option<String>,
    pub source: String,
}

impl SyncManager {
    pub fn new(db: DbPool, sync_base: String) -> Self {
        Self { db, sync_base }
    }

    /// Get the sync directory path for a project
    fn sync_path(&self, project_id: &str) -> String {
        format!("{}/{}", self.sync_base, project_id)
    }

    /// Get the source path for rsync — either the mount point or the project directory
    fn source_path(&self, project: &Project) -> Result<String> {
        if let Some(ref mount_id) = project.mount_id {
            let conn = self.db.lock().unwrap();
            let mount = Mount::get_by_id(&conn, mount_id)?;
            Ok(mount.local_mount_point.clone())
        } else {
            Ok(project.directory.clone())
        }
    }

    /// Initialise a jj repo in the sync directory (first-time setup)
    fn init_jj_repo(&self, sync_path: &str) -> Result<()> {
        std::fs::create_dir_all(sync_path)?;

        let output = Command::new("jj")
            .args(["git", "init"])
            .current_dir(sync_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Already initialised is fine
            if !stderr.contains("already") {
                return Err(IronweaveError::Internal(format!("jj init failed: {}", stderr)));
            }
        }
        info!(sync_path, "jj repo initialised");
        Ok(())
    }

    /// Run rsync from source to sync directory
    fn run_rsync(&self, source: &str, dest: &str) -> Result<bool> {
        // Ensure trailing slash on source to copy contents, not the directory itself
        let source_with_slash = if source.ends_with('/') {
            source.to_string()
        } else {
            format!("{}/", source)
        };

        let output = Command::new("rsync")
            .args([
                "-az", "--delete",
                "--exclude", ".jj",
                "--itemize-changes",
                &source_with_slash,
                dest,
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("rsync failed: {}", stderr)));
        }

        // Check if any files were transferred (itemize-changes outputs a line per change)
        let stdout = String::from_utf8_lossy(&output.stdout);
        let has_changes = stdout.lines().any(|l| !l.trim().is_empty());
        Ok(has_changes)
    }

    /// Commit current state in jj
    fn jj_commit(&self, sync_path: &str, message: &str) -> Result<()> {
        // jj automatically tracks all changes, just need to describe and create new change
        let output = Command::new("jj")
            .args(["describe", "-m", message])
            .current_dir(sync_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(sync_path, error = %stderr, "jj describe failed");
        }

        // Create a new empty change on top
        let output = Command::new("jj")
            .arg("new")
            .current_dir(sync_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(sync_path, error = %stderr, "jj new failed");
        }

        Ok(())
    }

    /// Main sync operation
    pub fn sync_project(&self, project_id: &str) -> Result<SyncStatus> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let sync_path = self.sync_path(project_id);
        let source = self.source_path(&project)?;

        // Ensure mount is active if project has one
        if project.mount_id.is_some() {
            // Check if source directory exists/is accessible
            if !Path::new(&source).exists() {
                return Err(IronweaveError::Internal(
                    "Mount point not accessible. Ensure the mount is active.".to_string()
                ));
            }
        }

        // Update state to syncing
        let conn = self.db.lock().unwrap();
        Project::update_sync_state(&conn, project_id, "syncing", Some(&sync_path), None)?;
        drop(conn);

        // Init jj repo if needed
        if !Path::new(&sync_path).join(".jj").exists() {
            self.init_jj_repo(&sync_path)?;
        }

        // Run rsync
        match self.run_rsync(&source, &sync_path) {
            Ok(has_changes) => {
                let now = Utc::now().to_rfc3339();

                if has_changes {
                    let msg = format!("sync: {}", now);
                    self.jj_commit(&sync_path, &msg)?;
                    info!(project_id, "sync completed with changes");
                } else {
                    info!(project_id, "sync completed, no changes");
                }

                let conn = self.db.lock().unwrap();
                Project::update_sync_state(&conn, project_id, "idle", Some(&sync_path), Some(&now))?;

                Ok(SyncStatus {
                    sync_state: "idle".to_string(),
                    last_synced_at: Some(now),
                    sync_path: Some(sync_path),
                    source: "local".to_string(),
                })
            }
            Err(e) => {
                let conn = self.db.lock().unwrap();
                Project::update_sync_state(&conn, project_id, "error", Some(&sync_path), None)?;
                error!(project_id, error = %e, "sync failed");
                Err(e)
            }
        }
    }

    /// Get sync status for a project
    pub fn get_status(&self, project_id: &str) -> Result<SyncStatus> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;

        let source = if project.sync_path.is_some() && Path::new(project.sync_path.as_deref().unwrap_or("")).join(".jj").exists() {
            "local".to_string()
        } else if project.mount_id.is_some() {
            "sshfs".to_string()
        } else {
            "none".to_string()
        };

        Ok(SyncStatus {
            sync_state: project.sync_state,
            last_synced_at: project.last_synced_at,
            sync_path: project.sync_path,
            source,
        })
    }

    /// Get jj history (recent snapshots)
    pub fn get_history(&self, project_id: &str, limit: usize) -> Result<Vec<SyncSnapshot>> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let sync_path = project.sync_path.ok_or_else(|| {
            IronweaveError::Internal("No sync path configured".to_string())
        })?;

        if !Path::new(&sync_path).join(".jj").exists() {
            return Ok(vec![]);
        }

        let output = Command::new("jj")
            .args([
                "log",
                "--no-graph",
                "-r", &format!("ancestors(@, {})", limit),
                "-T", r#"change_id.short(12) ++ "\t" ++ description.first_line() ++ "\t" ++ committer.timestamp() ++ "\n""#,
            ])
            .current_dir(&sync_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("jj log failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let snapshots: Vec<SyncSnapshot> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(3, '\t').collect();
                if parts.len() >= 3 {
                    Some(SyncSnapshot {
                        change_id: parts[0].to_string(),
                        description: parts[1].to_string(),
                        timestamp: parts[2].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(snapshots)
    }

    /// Get diff for a specific jj change
    pub fn get_diff(&self, project_id: &str, change_id: &str) -> Result<String> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let sync_path = project.sync_path.ok_or_else(|| {
            IronweaveError::Internal("No sync path configured".to_string())
        })?;

        // Validate change_id is alphanumeric (prevent command injection)
        if !change_id.chars().all(|c| c.is_alphanumeric()) {
            return Err(IronweaveError::Internal("Invalid change ID".to_string()));
        }

        let output = Command::new("jj")
            .args(["diff", "-r", change_id])
            .current_dir(&sync_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("jj diff failed: {}", stderr)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Restore to a specific jj change
    pub fn restore(&self, project_id: &str, change_id: &str) -> Result<()> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let sync_path = project.sync_path.ok_or_else(|| {
            IronweaveError::Internal("No sync path configured".to_string())
        })?;

        // Validate change_id
        if !change_id.chars().all(|c| c.is_alphanumeric()) {
            return Err(IronweaveError::Internal("Invalid change ID".to_string()));
        }

        let output = Command::new("jj")
            .args(["restore", "--from", change_id])
            .current_dir(&sync_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("jj restore failed: {}", stderr)));
        }

        // Describe the restore
        let msg = format!("restored from {}", change_id);
        self.jj_commit(&sync_path, &msg)?;

        Ok(())
    }

    /// Browse files in the sync directory (or mount fallback)
    pub fn browse_files(&self, project_id: &str, relative_path: &str) -> Result<Vec<crate::api::filesystem::BrowseEntry>> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        // Determine base path: sync_path if available, otherwise mount point
        let base = if let Some(ref sp) = project.sync_path {
            if Path::new(sp).exists() {
                sp.clone()
            } else if let Some(ref mount_id) = project.mount_id {
                let conn = self.db.lock().unwrap();
                Mount::get_by_id(&conn, mount_id)?.local_mount_point
            } else {
                project.directory.clone()
            }
        } else if let Some(ref mount_id) = project.mount_id {
            let conn = self.db.lock().unwrap();
            Mount::get_by_id(&conn, mount_id)?.local_mount_point
        } else {
            project.directory.clone()
        };

        let full_path = if relative_path.is_empty() || relative_path == "/" {
            base.clone()
        } else {
            format!("{}/{}", base, relative_path.trim_start_matches('/'))
        };

        // Security: ensure resolved path is under base
        let canonical_base = Path::new(&base).canonicalize()
            .map_err(|_| IronweaveError::Internal("Base path not accessible".to_string()))?;
        let canonical_full = Path::new(&full_path).canonicalize()
            .map_err(|_| IronweaveError::NotFound("Path not found".to_string()))?;

        if !canonical_full.starts_with(&canonical_base) {
            return Err(IronweaveError::Internal("Path traversal not allowed".to_string()));
        }

        let mut entries = Vec::new();
        let read_dir = std::fs::read_dir(&canonical_full)
            .map_err(|_| IronweaveError::NotFound("Directory not found".to_string()))?;

        for entry in read_dir.flatten() {
            let file_type = entry.file_type().map_err(|e| IronweaveError::Internal(e.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files and .jj directory
            if name.starts_with('.') {
                continue;
            }

            if file_type.is_dir() {
                entries.push(crate::api::filesystem::BrowseEntry {
                    name,
                    entry_type: "directory".to_string(),
                });
            } else if file_type.is_file() {
                entries.push(crate::api::filesystem::BrowseEntry {
                    name,
                    entry_type: "file".to_string(),
                });
            }
        }

        entries.sort_by(|a, b| {
            let type_cmp = a.entry_type.cmp(&b.entry_type);
            if type_cmp == std::cmp::Ordering::Equal {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else {
                type_cmp
            }
        });

        Ok(entries)
    }

    /// Read file content from sync directory (or mount fallback)
    pub fn read_file(&self, project_id: &str, relative_path: &str) -> Result<String> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let base = if let Some(ref sp) = project.sync_path {
            if Path::new(sp).exists() { sp.clone() } else { project.directory.clone() }
        } else if let Some(ref mount_id) = project.mount_id {
            let conn = self.db.lock().unwrap();
            Mount::get_by_id(&conn, mount_id)?.local_mount_point
        } else {
            project.directory.clone()
        };

        let full_path = format!("{}/{}", base, relative_path.trim_start_matches('/'));

        // Security: path traversal check
        let canonical_base = Path::new(&base).canonicalize()
            .map_err(|_| IronweaveError::Internal("Base path not accessible".to_string()))?;
        let canonical_full = Path::new(&full_path).canonicalize()
            .map_err(|_| IronweaveError::NotFound("File not found".to_string()))?;

        if !canonical_full.starts_with(&canonical_base) {
            return Err(IronweaveError::Internal("Path traversal not allowed".to_string()));
        }

        let metadata = std::fs::metadata(&canonical_full)
            .map_err(|_| IronweaveError::NotFound("File not found".to_string()))?;

        // Cap at 1MB
        if metadata.len() > 1_048_576 {
            return Err(IronweaveError::Internal("File too large (max 1MB)".to_string()));
        }

        std::fs::read_to_string(&canonical_full)
            .map_err(|e| IronweaveError::Internal(format!("Failed to read file: {}", e)))
    }
}
```

**Step 2: Register the sync module in main.rs**

Add `mod sync;` after `mod mount;` (line 12) in `src/main.rs`:

```rust
mod sync;
```

**Step 3: Add `chrono` dependency**

Run: `cargo add chrono --features serde`

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no errors.

**Step 5: Commit**

```bash
git add src/sync/ src/main.rs Cargo.toml Cargo.lock
git commit -m "feat: add SyncManager with rsync + jj integration"
```

---

### Task 5: Sync API endpoints

**Files:**
- Create: `src/api/sync.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs` (routes + state)
- Modify: `src/state.rs`

**Step 1: Add sync_manager to AppState**

In `src/state.rs`, add to the struct:

```rust
pub sync_manager: Option<Arc<crate::sync::manager::SyncManager>>,
```

Don't forget to add `use std::sync::Arc;` if not already present (it is — line 1).

**Step 2: Initialise SyncManager in main.rs**

In `src/main.rs`, after the `mount_manager` initialisation (around line 71-73), add:

```rust
let sync_manager = config.filesystem.as_ref().map(|fs_config| {
    let sync_base = format!("{}/sync", fs_config.mount_base.trim_end_matches('/'));
    Arc::new(sync::manager::SyncManager::new(db.clone(), sync_base))
});
```

Add `sync_manager` to the `AppState` construction (around line 75-82):

```rust
let state = AppState {
    db: db.clone(),
    process_manager,
    runtime_registry: registry,
    auth_config: auth_config.clone(),
    mount_manager: mount_manager.clone(),
    filesystem_config: config.filesystem.clone(),
    sync_manager,
};
```

**Step 3: Create api/sync.rs**

```rust
use axum::{extract::{Path, Query, State}, Json, http::StatusCode};
use serde::Deserialize;
use crate::state::AppState;
use crate::sync::manager::{SyncSnapshot, SyncStatus};
use crate::api::filesystem::BrowseEntry;

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RestoreRequest {
    pub change_id: String,
}

pub async fn trigger_sync(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SyncStatus>, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;

    // Ensure mount is active first if project has one
    if let Some(ref mm) = state.mount_manager {
        let conn = state.db.lock().unwrap();
        if let Ok(project) = crate::models::project::Project::get_by_id(&conn, &id) {
            drop(conn);
            if let Some(ref mount_id) = project.mount_id {
                let _ = mm.ensure_mounted(mount_id);
            }
        }
    }

    sm.sync_project(&id)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn get_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SyncStatus>, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;

    sm.get_status(&id)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn get_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<SyncSnapshot>>, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;

    sm.get_history(&id, 50)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn get_diff(
    State(state): State<AppState>,
    Path((id, change_id)): Path<(String, String)>,
) -> Result<String, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;

    sm.get_diff(&id, &change_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn restore(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<RestoreRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;

    sm.restore(&id, &input.change_id)
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn browse_files(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<Vec<BrowseEntry>>, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;

    let path = query.path.unwrap_or_default();
    sm.browse_files(&id, &path)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn read_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<String, (StatusCode, String)> {
    let sm = state.sync_manager.as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Sync not configured".to_string()))?;

    let path = query.path.unwrap_or_default();
    sm.read_file(&id, &path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
```

**Step 4: Register sync in api/mod.rs**

Add to `src/api/mod.rs`:

```rust
pub mod sync;
```

**Step 5: Wire routes in main.rs**

Add after the project routes (after line 92 in `src/main.rs`):

```rust
// Project sync
.route("/api/projects/{id}/sync", post(api::sync::trigger_sync))
.route("/api/projects/{id}/sync/status", get(api::sync::get_status))
.route("/api/projects/{id}/sync/history", get(api::sync::get_history))
.route("/api/projects/{id}/sync/diff/{change_id}", get(api::sync::get_diff))
.route("/api/projects/{id}/sync/restore", post(api::sync::restore))
.route("/api/projects/{id}/files", get(api::sync::browse_files))
.route("/api/projects/{id}/files/content", get(api::sync::read_file))
```

**Step 6: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no errors.

**Step 7: Commit**

```bash
git add src/api/sync.rs src/api/mod.rs src/state.rs src/main.rs
git commit -m "feat: add sync API endpoints — trigger, status, history, diff, restore, browse, read"
```

---

### Task 6: Frontend — update API client with new types and endpoints

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add new types**

Add after the `TestConnectionResult` interface (around line 260):

```typescript
export interface UpdateProject {
  name?: string;
  directory?: string;
  context?: string;
  description?: string;
  obsidian_vault_path?: string;
  obsidian_project?: string;
  git_remote?: string;
  mount_id?: string;
}

export interface SyncStatus {
  sync_state: string;
  last_synced_at: string | null;
  sync_path: string | null;
  source: string;
}

export interface SyncSnapshot {
  change_id: string;
  description: string;
  timestamp: string;
}
```

**Step 2: Update the Project interface**

Update the existing `Project` interface (lines 34-43) to include the new fields:

```typescript
export interface Project {
  id: string;
  name: string;
  directory: string;
  context: string;
  description: string | null;
  obsidian_vault_path: string | null;
  obsidian_project: string | null;
  git_remote: string | null;
  mount_id: string | null;
  sync_path: string | null;
  last_synced_at: string | null;
  sync_state: string;
  created_at: string;
}
```

**Step 3: Add update method to projects API and new sync API**

Update the `projects` object (lines 363-368):

```typescript
export const projects = {
  list: () => get<Project[]>('/projects'),
  get: (id: string) => get<Project>(`/projects/${id}`),
  create: (data: CreateProject) => post<Project>('/projects', data),
  update: (id: string, data: UpdateProject) => put<Project>(`/projects/${id}`, data),
  delete: (id: string) => del(`/projects/${id}`),
};
```

Add a new `sync` API object after the `proxyConfigs` object:

```typescript
export const sync = {
  trigger: (projectId: string) => post<SyncStatus>(`/projects/${projectId}/sync`, {}),
  status: (projectId: string) => get<SyncStatus>(`/projects/${projectId}/sync/status`),
  history: (projectId: string) => get<SyncSnapshot[]>(`/projects/${projectId}/sync/history`),
  diff: (projectId: string, changeId: string) => get<string>(`/projects/${projectId}/sync/diff/${changeId}`),
  restore: (projectId: string, changeId: string) => post<void>(`/projects/${projectId}/sync/restore`, { change_id: changeId }),
  browseFiles: (projectId: string, path?: string) =>
    get<BrowseEntry[]>(`/projects/${projectId}/files${path ? `?path=${encodeURIComponent(path)}` : ''}`),
  readFile: (projectId: string, path: string) =>
    get<string>(`/projects/${projectId}/files/content?path=${encodeURIComponent(path)}`),
};
```

**Step 4: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add sync, project update, and file browse types to frontend API client"
```

---

### Task 7: Frontend — Files tab component

**Files:**
- Create: `frontend/src/lib/components/ProjectFiles.svelte`

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { sync, type BrowseEntry, type SyncStatus } from '../api';

  interface Props {
    projectId: string;
  }
  let { projectId }: Props = $props();

  let entries: BrowseEntry[] = $state([]);
  let currentPath: string = $state('');
  let pathStack: string[] = $state([]);
  let fileContent: string | null = $state(null);
  let selectedFile: string | null = $state(null);
  let syncStatus: SyncStatus | null = $state(null);
  let loading: boolean = $state(false);
  let syncing: boolean = $state(false);
  let error: string | null = $state(null);

  async function fetchStatus() {
    try {
      syncStatus = await sync.status(projectId);
    } catch { /* ignore */ }
  }

  async function browse(path: string) {
    loading = true;
    error = null;
    fileContent = null;
    selectedFile = null;
    try {
      entries = await sync.browseFiles(projectId, path);
      currentPath = path;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to browse files';
      entries = [];
    } finally {
      loading = false;
    }
  }

  async function openFile(name: string) {
    const filePath = currentPath ? `${currentPath}/${name}` : name;
    selectedFile = name;
    try {
      const res = await fetch(`/api/projects/${projectId}/files/content?path=${encodeURIComponent(filePath)}`, {
        headers: {},
      });
      if (!res.ok) throw new Error(`Failed to read file: ${res.status}`);
      fileContent = await res.text();
    } catch (e) {
      fileContent = e instanceof Error ? e.message : 'Failed to read file';
    }
  }

  function navigateInto(name: string) {
    pathStack = [...pathStack, currentPath];
    browse(currentPath ? `${currentPath}/${name}` : name);
  }

  function navigateUp() {
    const prev = pathStack.pop() ?? '';
    pathStack = pathStack;
    browse(prev);
  }

  async function triggerSync() {
    syncing = true;
    error = null;
    try {
      syncStatus = await sync.trigger(projectId);
      await browse(currentPath);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Sync failed';
    } finally {
      syncing = false;
    }
  }

  $effect(() => {
    fetchStatus();
    browse('');
  });
</script>

<div class="space-y-4">
  <!-- Sync status bar -->
  <div class="flex items-center justify-between rounded-lg bg-gray-800 border border-gray-700 px-4 py-2">
    <div class="flex items-center gap-3 text-sm">
      {#if syncStatus}
        <span class="text-gray-400">Source:</span>
        <span class="font-medium {syncStatus.source === 'local' ? 'text-green-400' : syncStatus.source === 'sshfs' ? 'text-yellow-400' : 'text-gray-500'}">
          {syncStatus.source === 'local' ? 'Local copy' : syncStatus.source === 'sshfs' ? 'Live (SSHFS)' : 'No source'}
        </span>
        {#if syncStatus.last_synced_at}
          <span class="text-gray-600">|</span>
          <span class="text-gray-400">Last sync:</span>
          <span class="text-gray-300">{new Date(syncStatus.last_synced_at).toLocaleString('en-GB')}</span>
        {/if}
        {#if syncStatus.sync_state === 'error'}
          <span class="text-red-400 text-xs">Sync error</span>
        {/if}
      {:else}
        <span class="text-gray-500">Loading status...</span>
      {/if}
    </div>
    <button
      onclick={triggerSync}
      disabled={syncing}
      class="px-3 py-1.5 text-xs font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
    >
      {syncing ? 'Syncing...' : 'Sync Now'}
    </button>
  </div>

  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
  {/if}

  <!-- File browser -->
  <div class="grid grid-cols-1 lg:grid-cols-3 gap-4" style="min-height: 400px;">
    <!-- Tree panel -->
    <div class="rounded-xl bg-gray-900 border border-gray-800 overflow-hidden">
      <!-- Breadcrumb -->
      <div class="px-3 py-2 border-b border-gray-800 text-xs text-gray-400 font-mono truncate">
        /{currentPath}
      </div>

      <div class="overflow-y-auto" style="max-height: 500px;">
        {#if loading}
          <div class="p-4 text-gray-500 text-sm text-center">Loading...</div>
        {:else}
          {#if currentPath}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="px-3 py-2 text-sm text-gray-300 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
              onclick={navigateUp}
            >
              <span class="text-gray-500">..</span>
            </div>
          {/if}
          {#each entries as entry}
            {#if entry.type === 'directory'}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
                onclick={() => navigateInto(entry.name)}
              >
                <span class="text-yellow-500 text-xs shrink-0">&#128193;</span>
                <span class="truncate">{entry.name}</span>
              </div>
            {:else}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="px-3 py-1.5 text-sm hover:bg-gray-800 cursor-pointer flex items-center gap-2 {selectedFile === entry.name ? 'bg-gray-800 text-purple-400' : 'text-gray-400'}"
                onclick={() => openFile(entry.name)}
              >
                <span class="text-xs shrink-0">&#128196;</span>
                <span class="truncate">{entry.name}</span>
              </div>
            {/if}
          {/each}
          {#if entries.length === 0 && !currentPath}
            <div class="p-4 text-gray-500 text-sm text-center">
              No files available. Click "Sync Now" to pull files.
            </div>
          {/if}
        {/if}
      </div>
    </div>

    <!-- File viewer -->
    <div class="lg:col-span-2 rounded-xl bg-gray-900 border border-gray-800 overflow-hidden">
      {#if selectedFile}
        <div class="px-4 py-2 border-b border-gray-800 text-xs text-gray-400 font-mono">
          {currentPath ? `${currentPath}/` : ''}{selectedFile}
        </div>
        <pre class="p-4 text-sm text-gray-300 font-mono overflow-auto whitespace-pre" style="max-height: 500px;">{fileContent ?? 'Loading...'}</pre>
      {:else}
        <div class="flex items-center justify-center h-full text-gray-500 text-sm p-8">
          Select a file to view its contents.
        </div>
      {/if}
    </div>
  </div>
</div>
```

**Step 2: Commit**

```bash
git add frontend/src/lib/components/ProjectFiles.svelte
git commit -m "feat: add ProjectFiles component with sync status bar and file browser"
```

---

### Task 8: Frontend — History tab component

**Files:**
- Create: `frontend/src/lib/components/ProjectHistory.svelte`

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { sync, type SyncSnapshot } from '../api';

  interface Props {
    projectId: string;
  }
  let { projectId }: Props = $props();

  let snapshots: SyncSnapshot[] = $state([]);
  let selectedSnapshot: SyncSnapshot | null = $state(null);
  let diffContent: string | null = $state(null);
  let loading: boolean = $state(true);
  let restoring: boolean = $state(false);
  let error: string | null = $state(null);

  async function fetchHistory() {
    loading = true;
    try {
      snapshots = await sync.history(projectId);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch history';
    } finally {
      loading = false;
    }
  }

  async function viewDiff(snapshot: SyncSnapshot) {
    selectedSnapshot = snapshot;
    diffContent = null;
    try {
      const res = await fetch(`/api/projects/${projectId}/sync/diff/${snapshot.change_id}`);
      if (!res.ok) throw new Error(`Failed: ${res.status}`);
      diffContent = await res.text();
    } catch (e) {
      diffContent = e instanceof Error ? e.message : 'Failed to fetch diff';
    }
  }

  async function handleRestore(changeId: string) {
    if (!confirm('Restore all files to this snapshot? This will overwrite the current state.')) return;
    restoring = true;
    try {
      await sync.restore(projectId, changeId);
      await fetchHistory();
      selectedSnapshot = null;
      diffContent = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Restore failed';
    } finally {
      restoring = false;
    }
  }

  function formatTimestamp(ts: string): string {
    try {
      return new Date(ts).toLocaleString('en-GB', {
        day: 'numeric', month: 'short', year: 'numeric',
        hour: '2-digit', minute: '2-digit',
      });
    } catch {
      return ts;
    }
  }

  $effect(() => { fetchHistory(); });
</script>

<div class="space-y-4">
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
  {/if}

  {#if loading}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">Loading history...</div>
  {:else if snapshots.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      No sync history yet. Open the Files tab to trigger your first sync.
    </div>
  {:else}
    <div class="grid grid-cols-1 lg:grid-cols-3 gap-4" style="min-height: 400px;">
      <!-- Snapshot list -->
      <div class="rounded-xl bg-gray-900 border border-gray-800 overflow-hidden">
        <div class="px-4 py-2 border-b border-gray-800 text-xs text-gray-400 font-medium">
          Snapshots ({snapshots.length})
        </div>
        <div class="overflow-y-auto" style="max-height: 500px;">
          {#each snapshots as snapshot}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="px-4 py-3 border-b border-gray-800/50 cursor-pointer hover:bg-gray-800 transition-colors {selectedSnapshot?.change_id === snapshot.change_id ? 'bg-gray-800' : ''}"
              onclick={() => viewDiff(snapshot)}
            >
              <div class="flex items-center justify-between">
                <span class="text-xs font-mono text-purple-400">{snapshot.change_id}</span>
                <button
                  onclick={(e) => { e.stopPropagation(); handleRestore(snapshot.change_id); }}
                  disabled={restoring}
                  class="text-[10px] px-2 py-0.5 rounded bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
                >
                  Restore
                </button>
              </div>
              <p class="text-sm text-gray-300 mt-1 truncate">{snapshot.description || '(no description)'}</p>
              <p class="text-xs text-gray-500 mt-0.5">{formatTimestamp(snapshot.timestamp)}</p>
            </div>
          {/each}
        </div>
      </div>

      <!-- Diff viewer -->
      <div class="lg:col-span-2 rounded-xl bg-gray-900 border border-gray-800 overflow-hidden">
        {#if selectedSnapshot}
          <div class="px-4 py-2 border-b border-gray-800 text-xs text-gray-400 font-mono">
            Diff for {selectedSnapshot.change_id}
          </div>
          <pre class="p-4 text-xs font-mono overflow-auto whitespace-pre" style="max-height: 500px;">{#if diffContent}{#each diffContent.split('\n') as line}{#if line.startsWith('+')}<span class="text-green-400">{line}</span>
{:else if line.startsWith('-')}<span class="text-red-400">{line}</span>
{:else}<span class="text-gray-400">{line}</span>
{/if}{/each}{:else}<span class="text-gray-500">Loading diff...</span>{/if}</pre>
        {:else}
          <div class="flex items-center justify-center h-full text-gray-500 text-sm p-8">
            Select a snapshot to view its diff.
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
```

**Step 2: Commit**

```bash
git add frontend/src/lib/components/ProjectHistory.svelte
git commit -m "feat: add ProjectHistory component with snapshot timeline and diff viewer"
```

---

### Task 9: Frontend — Settings tab component

**Files:**
- Create: `frontend/src/lib/components/ProjectSettings.svelte`

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { projects, mounts, type Project, type MountConfig, type UpdateProject } from '../api';

  interface Props {
    project: Project;
    onUpdate: () => void;
  }
  let { project, onUpdate }: Props = $props();

  let name: string = $state(project.name);
  let description: string = $state(project.description ?? '');
  let context: string = $state(project.context);
  let directory: string = $state(project.directory);
  let gitRemote: string = $state(project.git_remote ?? '');
  let mountId: string = $state(project.mount_id ?? '');
  let saving: boolean = $state(false);
  let error: string | null = $state(null);
  let success: string | null = $state(null);
  let mountList: MountConfig[] = $state([]);

  async function fetchMounts() {
    try {
      mountList = await mounts.list();
    } catch { /* optional */ }
  }

  $effect(() => { fetchMounts(); });

  // Reset form when project changes
  $effect(() => {
    name = project.name;
    description = project.description ?? '';
    context = project.context;
    directory = project.directory;
    gitRemote = project.git_remote ?? '';
    mountId = project.mount_id ?? '';
  });

  async function handleSave() {
    saving = true;
    error = null;
    success = null;
    try {
      const data: UpdateProject = {};
      if (name !== project.name) data.name = name;
      if (description !== (project.description ?? '')) data.description = description;
      if (context !== project.context) data.context = context;
      if (directory !== project.directory) data.directory = directory;
      if (gitRemote !== (project.git_remote ?? '')) data.git_remote = gitRemote;
      if (mountId !== (project.mount_id ?? '')) data.mount_id = mountId;

      if (Object.keys(data).length === 0) {
        success = 'No changes to save.';
        return;
      }

      await projects.update(project.id, data);
      success = 'Project updated.';
      onUpdate();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to update project';
    } finally {
      saving = false;
    }
  }

  function mountStateColor(state: string): string {
    switch (state) {
      case 'mounted': return 'text-green-400';
      case 'error': return 'text-red-400';
      default: return 'text-gray-500';
    }
  }
</script>

<div class="space-y-4 max-w-2xl">
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
  {/if}
  {#if success}
    <div class="rounded-lg bg-green-900/40 border border-green-700 px-4 py-3 text-green-300 text-sm">{success}</div>
  {/if}

  <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
    <h3 class="text-sm font-semibold text-white">Project Details</h3>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <div>
        <label for="proj-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
        <input id="proj-name" type="text" bind:value={name}
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
      </div>

      <div>
        <label for="proj-context" class="block text-sm font-medium text-gray-400 mb-1">Context</label>
        <select id="proj-context" bind:value={context}
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500">
          <option value="work">Work</option>
          <option value="homelab">Homelab</option>
        </select>
      </div>
    </div>

    <div>
      <label for="proj-desc" class="block text-sm font-medium text-gray-400 mb-1">Description</label>
      <textarea id="proj-desc" bind:value={description} rows="3" placeholder="Project description..."
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500 resize-y"></textarea>
    </div>

    <div>
      <label for="proj-dir" class="block text-sm font-medium text-gray-400 mb-1">Directory</label>
      <input id="proj-dir" type="text" bind:value={directory}
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
    </div>

    <div>
      <label for="proj-git" class="block text-sm font-medium text-gray-400 mb-1">Git Remote</label>
      <input id="proj-git" type="text" bind:value={gitRemote} placeholder="https://github.com/..."
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
    </div>

    <div>
      <label for="proj-mount" class="block text-sm font-medium text-gray-400 mb-1">Mount</label>
      <select id="proj-mount" bind:value={mountId}
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500">
        <option value="">No mount (local project)</option>
        {#each mountList as m}
          <option value={m.id}>
            {m.name} — {m.remote_path}
          </option>
        {/each}
      </select>
      {#if mountId}
        {@const selectedMount = mountList.find(m => m.id === mountId)}
        {#if selectedMount}
          <p class="mt-1 text-xs {mountStateColor(selectedMount.state)}">
            State: {selectedMount.state}
            {#if selectedMount.last_error} — {selectedMount.last_error}{/if}
          </p>
        {/if}
      {/if}
    </div>

    <div class="flex justify-end">
      <button
        onclick={handleSave}
        disabled={saving}
        class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
      >
        {saving ? 'Saving...' : 'Save Changes'}
      </button>
    </div>
  </div>
</div>
```

**Step 2: Commit**

```bash
git add frontend/src/lib/components/ProjectSettings.svelte
git commit -m "feat: add ProjectSettings component for editing project details"
```

---

### Task 10: Frontend — wire new tabs into ProjectDetail

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte`

**Step 1: Add imports for new components**

Add after the existing imports (line 12):

```svelte
import ProjectFiles from '../lib/components/ProjectFiles.svelte';
import ProjectHistory from '../lib/components/ProjectHistory.svelte';
import ProjectSettings from '../lib/components/ProjectSettings.svelte';
```

**Step 2: Update the tabs array**

Replace the tabs definition (lines 32-36) with:

```typescript
const tabs = $derived([
  { key: 'teams', label: 'Teams' },
  { key: 'issues', label: 'Issues' },
  { key: 'workflows', label: 'Workflows' },
  { key: 'files', label: 'Files' },
  ...(project?.mount_id ? [{ key: 'history', label: 'History' }] : []),
  { key: 'settings', label: 'Settings' },
]);
```

**Step 3: Update the project header to show description**

Replace the project header section (lines 141-149) with:

```svelte
<div class="flex items-center gap-4">
  <div>
    <h1 class="text-2xl font-bold text-white">{project.name}</h1>
    <p class="mt-1 text-sm text-gray-400 font-mono">{project.directory}</p>
    {#if project.description}
      <p class="mt-1 text-sm text-gray-400 line-clamp-2">{project.description}</p>
    {/if}
  </div>
  <span class="text-xs font-medium px-2.5 py-1 rounded-full {contextBadge(project.context)}">
    {project.context}
  </span>
</div>
```

**Step 4: Add auto-sync trigger on project load**

Add to the script section, after `fetchWorkflows` function:

```typescript
async function autoSync() {
  if (project?.mount_id) {
    try {
      const { sync: syncApi } = await import('../lib/api');
      await syncApi.trigger(params.id);
    } catch { /* non-blocking */ }
  }
}
```

In the `$effect` block (lines 63-70), add `autoSync()` call after `fetchProject()`:

```typescript
$effect(() => {
  const pid = params.id;
  if (pid) {
    fetchProject().then(() => autoSync());
    fetchTeams();
    fetchWorkflows();
  }
});
```

**Step 5: Add new tab content sections**

After the workflows tab content section (before the closing `{:else if !error}` around line 294), add the new tab content:

```svelte
{:else if activeTab === 'files'}
  <ProjectFiles projectId={params.id} />
{:else if activeTab === 'history'}
  <ProjectHistory projectId={params.id} />
{:else if activeTab === 'settings'}
  <ProjectSettings {project} onUpdate={fetchProject} />
```

**Step 6: Build frontend**

Run: `cd frontend && npm run build`
Expected: Build succeeds.

**Step 7: Commit**

```bash
git add frontend/src/routes/ProjectDetail.svelte
git commit -m "feat: wire Files, History, Settings tabs into ProjectDetail page"
```

---

### Task 11: Backend build + deploy to hl-ironweave

**Step 1: Build frontend**

Run: `cd frontend && npm run build && cd ..`

**Step 2: Verify backend compiles**

Run: `cargo build`

**Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass.

**Step 4: Deploy to hl-ironweave**

```bash
rsync -avz --exclude 'target/' --exclude 'node_modules/' --exclude '.git/' --exclude 'data/' --exclude 'ironweave.toml' . paddy@10.202.28.205:/home/paddy/ironweave/
ssh paddy@10.202.28.205 "cd /home/paddy/ironweave && cargo build --release && sudo systemctl restart ironweave"
```

**Step 5: Verify jj and rsync are installed on hl-ironweave**

```bash
ssh paddy@10.202.28.205 "which jj && which rsync"
```

If jj is not installed:
```bash
ssh paddy@10.202.28.205 "curl -sSf https://jj-vcs.github.io/jj/install.sh | sh"
```

**Step 6: Commit (if any deploy fixes needed)**

Only commit if code changes were required.

---

### Task 12: End-to-end verification with Playwright

**Step 1: Navigate to the app and open a project**

Browse to `http://10.202.28.205:3000/#/projects` and click on an existing project (or create one with an SSHFS mount).

**Step 2: Verify new tabs appear**

Check that Files, History (if project has mount), and Settings tabs are visible.

**Step 3: Test Files tab**

Click Files tab. Verify:
- Sync status bar appears
- Click "Sync Now" — status should update
- Directory tree should populate after sync
- Click a file — content should appear in the viewer

**Step 4: Test History tab**

Click History tab. Verify:
- At least one snapshot appears after sync
- Click a snapshot — diff panel shows content
- Restore button is visible

**Step 5: Test Settings tab**

Click Settings tab. Verify:
- All fields pre-populated
- Add a description, click Save
- Navigate away and back — description persists and shows in header

**Step 6: Commit any fixes**

If any issues found during verification, fix and redeploy.
