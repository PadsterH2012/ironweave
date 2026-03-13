# Project App Preview — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users start/stop local web apps and link to remote project URLs from the project detail page.

**Architecture:** New `project_apps` table + `app_url` column on projects. Backend spawns app processes as child subprocesses with auto-detected run commands and auto-assigned ports. Frontend adds start/stop button and URL link to the project header.

**Tech Stack:** Rust/Axum backend, SQLite (rusqlite), Svelte 5 frontend, TypeScript API client

---

### Task 1: Database Migration — Add project_apps table and app_url column

**Files:**
- Modify: `src/db/migrations.rs`

**Step 1: Add the migration code**

At the end of the `run_migrations` function (before the final `Ok(())`), add:

```rust
// ── Project App Preview ──────────────────────────────────────────
conn.execute_batch("
    CREATE TABLE IF NOT EXISTS project_apps (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        pid INTEGER,
        port INTEGER,
        run_command TEXT NOT NULL,
        state TEXT CHECK(state IN ('stopped', 'starting', 'running', 'error')) DEFAULT 'stopped',
        last_error TEXT,
        started_at TEXT,
        created_at TEXT DEFAULT (datetime('now'))
    );
")?;

// Add app_url to projects
let _ = conn.execute("ALTER TABLE projects ADD COLUMN app_url TEXT", []);
```

**Step 2: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: Build succeeds (warnings OK)

**Step 3: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add project_apps table and app_url column migration"
```

---

### Task 2: ProjectApp Model

**Files:**
- Create: `src/models/project_app.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create the model file**

Create `src/models/project_app.rs`:

```rust
use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectApp {
    pub id: String,
    pub project_id: String,
    pub pid: Option<i64>,
    pub port: Option<i32>,
    pub run_command: String,
    pub state: String,
    pub last_error: Option<String>,
    pub started_at: Option<String>,
    pub created_at: String,
}

impl ProjectApp {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            pid: row.get("pid")?,
            port: row.get("port")?,
            run_command: row.get("run_command")?,
            state: row.get("state")?,
            last_error: row.get("last_error")?,
            started_at: row.get("started_at")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn get_by_project(conn: &Connection, project_id: &str) -> Result<Option<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM project_apps WHERE project_id = ?1"
        )?;
        let result = stmt.query_row(params![project_id], Self::from_row).ok();
        Ok(result)
    }

    pub fn upsert(conn: &Connection, project_id: &str, run_command: &str) -> Result<Self> {
        // If one exists, update it; otherwise create
        if let Some(existing) = Self::get_by_project(conn, project_id)? {
            conn.execute(
                "UPDATE project_apps SET run_command = ?1 WHERE id = ?2",
                params![run_command, existing.id],
            )?;
            return Self::get_by_project(conn, project_id)?
                .ok_or_else(|| crate::error::IronweaveError::NotFound("project_app".into()));
        }

        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO project_apps (id, project_id, run_command, state) VALUES (?1, ?2, ?3, 'stopped')",
            params![id, project_id, run_command],
        )?;
        Self::get_by_project(conn, project_id)?
            .ok_or_else(|| crate::error::IronweaveError::NotFound("project_app".into()))
    }

    pub fn update_state(conn: &Connection, id: &str, state: &str, pid: Option<i64>, port: Option<i32>, error: Option<&str>) -> Result<()> {
        let started_at = if state == "running" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        conn.execute(
            "UPDATE project_apps SET state = ?1, pid = ?2, port = ?3, last_error = ?4, started_at = ?5 WHERE id = ?6",
            params![state, pid, port, error, started_at, id],
        )?;
        Ok(())
    }
}
```

**Step 2: Register the module**

Add to `src/models/mod.rs`:

```rust
pub mod project_app;
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/models/project_app.rs src/models/mod.rs
git commit -m "feat: add ProjectApp model for app preview tracking"
```

---

### Task 3: Add app_url to Project model

**Files:**
- Modify: `src/models/project.rs`
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add app_url to Project struct**

In `src/models/project.rs`, add to the `Project` struct after `sync_state`:

```rust
pub app_url: Option<String>,
```

**Step 2: Update from_row**

Add to the `from_row` method:

```rust
app_url: row.get("app_url")?,
```

**Step 3: Add to UpdateProject**

Add to the `UpdateProject` struct:

```rust
pub app_url: Option<String>,
```

**Step 4: Update the update method**

Add to the `update` method, following the existing pattern:

```rust
let app_url = input.app_url.as_deref().or(existing.app_url.as_deref());
```

And add `app_url` to the SQL UPDATE statement and params.

**Step 5: Update frontend types**

In `frontend/src/lib/api.ts`, add to the `Project` interface:

```typescript
app_url: string | null;
```

Add to the `UpdateProject` interface:

```typescript
app_url?: string;
```

**Step 6: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: Build succeeds

**Step 7: Commit**

```bash
git add src/models/project.rs frontend/src/lib/api.ts
git commit -m "feat: add app_url field to Project model and frontend types"
```

---

### Task 4: Auto-Detection Logic

**Files:**
- Create: `src/app_runner/mod.rs`
- Create: `src/app_runner/detect.rs`

**Step 1: Create the module structure**

Create `src/app_runner/mod.rs`:

```rust
pub mod detect;
```

Create `src/app_runner/detect.rs`:

```rust
use std::path::Path;

pub struct DetectedApp {
    pub command: String,
    pub args: Vec<String>,
    pub port_via_env: bool,
}

pub fn detect_app(project_dir: &Path) -> Option<DetectedApp> {
    // Flask app
    if project_dir.join("app.py").exists() {
        if file_contains(project_dir, "app.py", "Flask") {
            return Some(DetectedApp {
                command: "python".into(),
                args: vec!["app.py".into()],
                port_via_env: true,
            });
        }
    }

    if project_dir.join("main.py").exists() {
        if file_contains(project_dir, "main.py", "Flask") {
            return Some(DetectedApp {
                command: "python".into(),
                args: vec!["main.py".into()],
                port_via_env: true,
            });
        }
    }

    // Django
    if project_dir.join("manage.py").exists() {
        return Some(DetectedApp {
            command: "python".into(),
            args: vec!["manage.py".into(), "runserver".into()], // port appended at spawn time
            port_via_env: false,
        });
    }

    // Node.js
    if project_dir.join("package.json").exists() {
        if file_contains(project_dir, "package.json", "\"start\"") {
            return Some(DetectedApp {
                command: "npm".into(),
                args: vec!["start".into()],
                port_via_env: true,
            });
        }
    }

    // Rust
    if project_dir.join("Cargo.toml").exists() {
        return Some(DetectedApp {
            command: "cargo".into(),
            args: vec!["run".into()],
            port_via_env: true,
        });
    }

    // Go
    if project_dir.join("go.mod").exists() {
        return Some(DetectedApp {
            command: "go".into(),
            args: vec!["run".into(), ".".into()],
            port_via_env: true,
        });
    }

    // Static site
    if project_dir.join("index.html").exists() {
        return Some(DetectedApp {
            command: "python".into(),
            args: vec!["-m".into(), "http.server".into()], // port appended at spawn time
            port_via_env: false,
        });
    }

    None
}

fn file_contains(dir: &Path, filename: &str, needle: &str) -> bool {
    std::fs::read_to_string(dir.join(filename))
        .map(|content| content.contains(needle))
        .unwrap_or(false)
}
```

**Step 2: Register the module in main**

Add to the top of `src/main.rs`:

```rust
mod app_runner;
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/app_runner/mod.rs src/app_runner/detect.rs src/main.rs
git commit -m "feat: add auto-detection logic for project web apps"
```

---

### Task 5: App Runner — Port Assignment and Process Spawning

**Files:**
- Create: `src/app_runner/runner.rs`
- Modify: `src/app_runner/mod.rs`

**Step 1: Create the runner**

Create `src/app_runner/runner.rs`:

```rust
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Child};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::DbPool;
use crate::models::project_app::ProjectApp;

const PORT_RANGE_START: i32 = 8100;
const PORT_RANGE_END: i32 = 8199;

pub struct AppRunner {
    db: DbPool,
    children: Arc<Mutex<HashMap<String, Child>>>,
}

impl AppRunner {
    pub fn new(db: DbPool) -> Self {
        Self {
            db,
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn find_free_port(&self) -> Option<i32> {
        let conn = self.db.lock().unwrap();
        let used_ports: Vec<i32> = conn
            .prepare("SELECT port FROM project_apps WHERE state = 'running' AND port IS NOT NULL")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get(0))?
                    .collect::<std::result::Result<Vec<i32>, _>>()
            })
            .unwrap_or_default();

        (PORT_RANGE_START..=PORT_RANGE_END).find(|p| !used_ports.contains(p))
    }

    pub async fn start_app(
        &self,
        app_id: &str,
        project_dir: &str,
        detected: &super::detect::DetectedApp,
    ) -> Result<(i32, u32), String> {
        let port = self.find_free_port().ok_or("No free ports in range 8100-8199")?;

        let mut cmd = Command::new(&detected.command);
        cmd.current_dir(project_dir);

        for arg in &detected.args {
            cmd.arg(arg);
        }

        // Pass port
        if detected.port_via_env {
            cmd.env("PORT", port.to_string());
            // Flask-specific: also set FLASK_RUN_PORT and bind to 0.0.0.0
            if detected.command == "python" {
                cmd.env("FLASK_RUN_HOST", "0.0.0.0");
                cmd.env("FLASK_RUN_PORT", port.to_string());
            }
        } else {
            // Append port as argument (Django manage.py runserver, python -m http.server)
            if detected.args.contains(&"runserver".to_string()) {
                cmd.arg(format!("0.0.0.0:{}", port));
            } else {
                cmd.arg(port.to_string());
                cmd.arg("--bind");
                cmd.arg("0.0.0.0");
            }
        }

        // Redirect stdout/stderr to /dev/null to avoid blocking
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;
        let pid = child.id();

        // Store the child process handle
        self.children.lock().await.insert(app_id.to_string(), child);

        Ok((port, pid))
    }

    pub async fn stop_app(&self, app_id: &str) -> Result<(), String> {
        let mut children = self.children.lock().await;
        if let Some(mut child) = children.remove(app_id) {
            let _ = child.kill();
            let _ = child.wait();
            Ok(())
        } else {
            // Try killing by PID from DB
            let pid = {
                let conn = self.db.lock().unwrap();
                ProjectApp::get_by_project(&conn, "")
                    .ok()
                    .flatten()
                    .and_then(|a| a.pid)
            };
            if let Some(pid) = pid {
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }
            Ok(())
        }
    }

    pub async fn check_running(&self, app_id: &str) -> bool {
        let mut children = self.children.lock().await;
        if let Some(child) = children.get_mut(app_id) {
            match child.try_wait() {
                Ok(Some(_)) => false, // exited
                Ok(None) => true,     // still running
                Err(_) => false,
            }
        } else {
            false
        }
    }
}
```

**Step 2: Update mod.rs**

Update `src/app_runner/mod.rs`:

```rust
pub mod detect;
pub mod runner;
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`

If `libc` is not a dependency, add it to `Cargo.toml`:

```bash
cargo add libc
```

Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/app_runner/runner.rs src/app_runner/mod.rs Cargo.toml Cargo.lock
git commit -m "feat: add AppRunner for port assignment and process spawning"
```

---

### Task 6: Add AppRunner to AppState

**Files:**
- Modify: `src/state.rs` (or wherever `AppState` is defined)
- Modify: `src/main.rs`

**Step 1: Find and update AppState**

Find the `AppState` struct. Add:

```rust
pub app_runner: Arc<crate::app_runner::runner::AppRunner>,
```

**Step 2: Initialise in main.rs**

Where `AppState` is constructed, add:

```rust
let app_runner = Arc::new(crate::app_runner::runner::AppRunner::new(db.clone()));
```

And include it in the `AppState` struct construction.

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/state.rs src/main.rs
git commit -m "feat: add AppRunner to AppState"
```

---

### Task 7: API Endpoints — start, stop, status

**Files:**
- Create: `src/api/project_apps.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`

**Step 1: Create the API module**

Create `src/api/project_apps.rs`:

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::app_runner::detect::detect_app;
use crate::models::project::Project;
use crate::models::project_app::ProjectApp;
use crate::state::AppState;

#[derive(Serialize)]
pub struct AppStatus {
    pub id: Option<String>,
    pub state: String,
    pub port: Option<i32>,
    pub url: Option<String>,
    pub run_command: Option<String>,
    pub last_error: Option<String>,
    pub started_at: Option<String>,
}

pub async fn start(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<AppStatus>, (StatusCode, String)> {
    // Get project
    let project = {
        let conn = state.db.lock().unwrap();
        Project::get_by_id(&conn, &project_id)
            .map_err(|_| (StatusCode::NOT_FOUND, "Project not found".into()))?
    };

    // Auto-detect app
    let project_dir = std::path::Path::new(&project.directory);
    let detected = detect_app(project_dir)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "No runnable app detected in project directory".into()))?;

    let run_command = format!("{} {}", detected.command, detected.args.join(" "));

    // Upsert project_app record
    let app = {
        let conn = state.db.lock().unwrap();
        ProjectApp::upsert(&conn, &project_id, &run_command)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?
    };

    // Check if already running
    if app.state == "running" && state.app_runner.check_running(&app.id).await {
        return Ok(Json(AppStatus {
            id: Some(app.id),
            state: "running".into(),
            port: app.port,
            url: app.port.map(|p| format!("http://10.202.28.205:{}", p)),
            run_command: Some(app.run_command),
            last_error: None,
            started_at: app.started_at,
        }));
    }

    // Start the app
    match state.app_runner.start_app(&app.id, &project.directory, &detected).await {
        Ok((port, pid)) => {
            let conn = state.db.lock().unwrap();
            let _ = ProjectApp::update_state(&conn, &app.id, "running", Some(pid as i64), Some(port), None);

            Ok(Json(AppStatus {
                id: Some(app.id),
                state: "running".into(),
                port: Some(port),
                url: Some(format!("http://10.202.28.205:{}", port)),
                run_command: Some(run_command),
                last_error: None,
                started_at: Some(chrono::Utc::now().to_rfc3339()),
            }))
        }
        Err(e) => {
            let conn = state.db.lock().unwrap();
            let _ = ProjectApp::update_state(&conn, &app.id, "error", None, None, Some(&e));

            Err((StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

pub async fn stop(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let app = {
        let conn = state.db.lock().unwrap();
        ProjectApp::get_by_project(&conn, &project_id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?
            .ok_or_else(|| (StatusCode::NOT_FOUND, "No app found for project".into()))?
    };

    let _ = state.app_runner.stop_app(&app.id).await;

    let conn = state.db.lock().unwrap();
    let _ = ProjectApp::update_state(&conn, &app.id, "stopped", None, None, None);

    Ok(StatusCode::NO_CONTENT)
}

pub async fn status(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Json<AppStatus> {
    let app = {
        let conn = state.db.lock().unwrap();
        ProjectApp::get_by_project(&conn, &project_id)
            .ok()
            .flatten()
    };

    match app {
        Some(app) => {
            // Verify process is actually still running
            let actually_running = state.app_runner.check_running(&app.id).await;
            let real_state = if app.state == "running" && !actually_running {
                // Process died — update DB
                let conn = state.db.lock().unwrap();
                let _ = ProjectApp::update_state(&conn, &app.id, "stopped", None, None, Some("Process exited unexpectedly"));
                "stopped"
            } else {
                &app.state
            };

            Json(AppStatus {
                id: Some(app.id),
                state: real_state.to_string(),
                port: if real_state == "running" { app.port } else { None },
                url: if real_state == "running" { app.port.map(|p| format!("http://10.202.28.205:{}", p)) } else { None },
                run_command: Some(app.run_command),
                last_error: app.last_error,
                started_at: app.started_at,
            })
        }
        None => Json(AppStatus {
            id: None,
            state: "stopped".into(),
            port: None,
            url: None,
            run_command: None,
            last_error: None,
            started_at: None,
        }),
    }
}
```

**Step 2: Register in api/mod.rs**

Add to `src/api/mod.rs`:

```rust
pub mod project_apps;
```

**Step 3: Register routes in main.rs**

Add after the existing project routes:

```rust
// Project app preview
.route("/api/projects/{id}/app/start", post(api::project_apps::start))
.route("/api/projects/{id}/app/stop", post(api::project_apps::stop))
.route("/api/projects/{id}/app/status", get(api::project_apps::status))
```

**Step 4: Verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add src/api/project_apps.rs src/api/mod.rs src/main.rs
git commit -m "feat: add project app start/stop/status API endpoints"
```

---

### Task 8: Frontend — API Client and UI

**Files:**
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/routes/ProjectDetail.svelte`

**Step 1: Add API client methods**

In `frontend/src/lib/api.ts`, add the `AppStatus` interface near the other types:

```typescript
export interface AppStatus {
  id: string | null;
  state: string;
  port: number | null;
  url: string | null;
  run_command: string | null;
  last_error: string | null;
  started_at: string | null;
}
```

Add the `projectApps` API client after the `projects` export:

```typescript
export const projectApps = {
  start: (projectId: string) => post<AppStatus>(`/projects/${projectId}/app/start`, {}),
  stop: (projectId: string) => post<void>(`/projects/${projectId}/app/stop`, {}),
  status: (projectId: string) => get<AppStatus>(`/projects/${projectId}/app/status`),
};
```

**Step 2: Add state and logic to ProjectDetail.svelte**

In the `<script>` section, add the import:

```typescript
import { ..., projectApps, type AppStatus } from '../lib/api';
```

Add state variables (after the mount state variables):

```typescript
// App preview state
let appStatus: AppStatus | null = $state(null);
let togglingApp: boolean = $state(false);
```

Add a function to fetch app status (call it from the existing `onMount` or data-loading block):

```typescript
async function loadAppStatus() {
  if (project) {
    try {
      appStatus = await projectApps.status(project.id);
    } catch {
      appStatus = null;
    }
  }
}
```

Add toggle function:

```typescript
async function handleToggleApp() {
  if (!project || togglingApp) return;
  togglingApp = true;
  try {
    if (appStatus?.state === 'running') {
      await projectApps.stop(project.id);
    } else {
      await projectApps.start(project.id);
    }
    await loadAppStatus();
  } catch (e: any) {
    error = e.message;
  } finally {
    togglingApp = false;
  }
}
```

Call `loadAppStatus()` after the project data is loaded (in the same place `loadMount` is called).

**Step 3: Add UI to template**

In the project header area, after the existing mount toggle button, add:

```svelte
<!-- App preview controls -->
{#if project.app_url}
  <!-- Remote project: just show link -->
  <a
    href={project.app_url}
    target="_blank"
    rel="noopener noreferrer"
    class="flex items-center gap-2 px-3 py-1.5 text-xs font-medium rounded-full bg-blue-600/20 border border-blue-600 text-blue-400 hover:bg-blue-600/30 transition-colors"
  >
    <span class="w-2 h-2 rounded-full bg-blue-400"></span>
    Open App ↗
  </a>
{:else if !project.mount_id}
  <!-- Local project: start/stop + link -->
  <button
    onclick={handleToggleApp}
    disabled={togglingApp}
    class="flex items-center gap-2 px-3 py-1.5 text-xs font-medium rounded-full transition-colors {appStatus?.state === 'running'
      ? 'bg-green-600/20 border border-green-600 text-green-400 hover:bg-red-600/20 hover:border-red-600 hover:text-red-400'
      : appStatus?.state === 'error'
        ? 'bg-red-600/20 border border-red-600 text-red-400 hover:bg-green-600/20 hover:border-green-600 hover:text-green-400'
        : 'bg-gray-800 border border-gray-700 text-gray-400 hover:bg-green-600/20 hover:border-green-600 hover:text-green-400'}"
    title={appStatus?.last_error || ''}
  >
    <span class="w-2 h-2 rounded-full {appStatus?.state === 'running' ? 'bg-green-400' : appStatus?.state === 'error' ? 'bg-red-400' : 'bg-gray-500'}"></span>
    {#if togglingApp}
      ...
    {:else if appStatus?.state === 'running'}
      Stop App
    {:else if appStatus?.state === 'error'}
      Retry
    {:else}
      Start App
    {/if}
  </button>
  {#if appStatus?.state === 'running' && appStatus?.url}
    <a
      href={appStatus.url}
      target="_blank"
      rel="noopener noreferrer"
      class="flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded-full bg-blue-600/20 border border-blue-600 text-blue-400 hover:bg-blue-600/30 transition-colors"
    >
      Open ↗
    </a>
  {/if}
{/if}
```

**Step 4: Add app_url field to Settings tab**

In the `ProjectSettings` component (or inline settings section), add an `app_url` input field following the existing pattern for optional fields like `obsidian_project`.

**Step 5: Build frontend**

Run: `cd frontend && npm run build`
Expected: Build succeeds

**Step 6: Commit**

```bash
git add frontend/src/lib/api.ts frontend/src/routes/ProjectDetail.svelte
git commit -m "feat: add app preview start/stop button and URL link to project header"
```

---

### Task 9: Deploy and Test

**Step 1: Rsync to server**

```bash
rsync -az --exclude target --exclude .git --exclude node_modules --exclude .worktrees -e "ssh -o StrictHostKeyChecking=no" ./ paddy@10.202.28.205:/home/paddy/ironweave/
```

**Step 2: Build on server**

```bash
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave && source ~/.cargo/env && cargo build --release 2>&1 | tail -5'
```

**Step 3: Restart service**

```bash
ssh -t paddy@10.202.28.205 'echo "P0w3rPla72012@@" | sudo -S systemctl restart ironweave 2>/dev/null'
```

**Step 4: Test the feature**

1. Open the Simple Task Manager project page in browser
2. Click "Start App" — should show the app starting
3. Click "Open ↗" link — should open `http://10.202.28.205:<port>` showing the Flask login page
4. Click "Stop App" — should stop the process
5. Verify the status badge updates correctly

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: deployment adjustments for app preview feature"
```
