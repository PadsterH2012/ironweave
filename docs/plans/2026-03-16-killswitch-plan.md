# Killswitch Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add global and per-project dispatch killswitch with cron-style scheduling so agents don't run unattended.

**Architecture:** New `is_paused` column on projects table, global pause via settings keys, new `dispatch_schedules` table for cron rules. Sweep loop checks pause state before dispatch. Existing idle escalation (5/7/9 min) handles drain. New API handler module + frontend UI components.

**Tech Stack:** Rust/Axum backend, rusqlite, `cron` crate for expression parsing, Svelte 5 frontend.

---

### Task 1: Database Migration — Project Pause Columns

**Files:**
- Modify: `src/db/migrations.rs` (append to `run_migrations`)

**Step 1: Add migration statements**

Add to the end of `run_migrations()` in `src/db/migrations.rs`, using the existing `migrate_alter` pattern:

```rust
// Killswitch: project-level pause
migrate_alter(conn, "ALTER TABLE projects ADD COLUMN is_paused INTEGER NOT NULL DEFAULT 0");
migrate_alter(conn, "ALTER TABLE projects ADD COLUMN paused_at TEXT");
migrate_alter(conn, "ALTER TABLE projects ADD COLUMN pause_reason TEXT");
```

**Step 2: Run tests to verify migration doesn't break existing code**

Run: `cargo test --lib db::migrations -- --nocapture 2>&1 | tail -20`
Expected: existing tests pass (or no migration-specific tests — verify the project model tests still work)

Run: `cargo test --lib models::project -- --nocapture 2>&1 | tail -20`
Expected: PASS (existing project tests unaffected since from_row uses named columns)

**Step 3: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat(killswitch): add is_paused, paused_at, pause_reason columns to projects"
```

---

### Task 2: Database Migration — dispatch_schedules Table

**Files:**
- Modify: `src/db/migrations.rs` (append to `run_migrations`)

**Step 1: Add CREATE TABLE for dispatch_schedules**

Append to `run_migrations()`:

```rust
// Killswitch: dispatch schedules table
conn.execute_batch("
    CREATE TABLE IF NOT EXISTS dispatch_schedules (
        id TEXT PRIMARY KEY,
        scope TEXT NOT NULL CHECK(scope IN ('global', 'project')),
        project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
        cron_expression TEXT NOT NULL,
        action TEXT NOT NULL CHECK(action IN ('resume', 'pause')),
        timezone TEXT NOT NULL DEFAULT 'Europe/London',
        is_enabled INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        description TEXT
    );
")?;
```

**Step 2: Run tests**

Run: `cargo test --lib models::project -- --nocapture 2>&1 | tail -20`
Expected: PASS

**Step 3: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat(killswitch): add dispatch_schedules table"
```

---

### Task 3: Project Model — Add Pause Fields + Methods

**Files:**
- Modify: `src/models/project.rs`

**Step 1: Add fields to Project struct**

Add after `app_url` field:

```rust
pub is_paused: bool,
pub paused_at: Option<String>,
pub pause_reason: Option<String>,
```

**Step 2: Update `from_row` to read new columns**

Add after `app_url` line in `from_row`:

```rust
is_paused: row.get::<_, i64>("is_paused").unwrap_or(0) != 0,
paused_at: row.get("paused_at")?,
pause_reason: row.get("pause_reason")?,
```

**Step 3: Add pause/resume methods**

Add to `impl Project`:

```rust
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
```

**Step 4: Write tests for pause/resume**

Add to the `tests` module in `project.rs`:

```rust
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
```

**Step 5: Run tests**

Run: `cargo test --lib models::project -- --nocapture 2>&1 | tail -30`
Expected: ALL PASS including new test

**Step 6: Commit**

```bash
git add src/models/project.rs
git commit -m "feat(killswitch): add pause/resume to Project model"
```

---

### Task 4: DispatchSchedule Model

**Files:**
- Create: `src/models/dispatch_schedule.rs`
- Modify: `src/models/mod.rs` (add `pub mod dispatch_schedule;`)

**Step 1: Write the model**

Create `src/models/dispatch_schedule.rs`:

```rust
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchSchedule {
    pub id: String,
    pub scope: String,
    pub project_id: Option<String>,
    pub cron_expression: String,
    pub action: String,
    pub timezone: String,
    pub is_enabled: bool,
    pub created_at: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDispatchSchedule {
    pub scope: String,
    pub project_id: Option<String>,
    pub cron_expression: String,
    pub action: String,
    pub timezone: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDispatchSchedule {
    pub cron_expression: Option<String>,
    pub action: Option<String>,
    pub timezone: Option<String>,
    pub is_enabled: Option<bool>,
    pub description: Option<String>,
}

impl DispatchSchedule {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            scope: row.get("scope")?,
            project_id: row.get("project_id")?,
            cron_expression: row.get("cron_expression")?,
            action: row.get("action")?,
            timezone: row.get("timezone")?,
            is_enabled: row.get::<_, i64>("is_enabled")? != 0,
            created_at: row.get("created_at")?,
            description: row.get("description")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateDispatchSchedule) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let tz = input.timezone.as_deref().unwrap_or("Europe/London");
        conn.execute(
            "INSERT INTO dispatch_schedules (id, scope, project_id, cron_expression, action, timezone, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, input.scope, input.project_id, input.cron_expression, input.action, tz, input.description],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM dispatch_schedules WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("schedule {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM dispatch_schedules ORDER BY scope, created_at")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut schedules = Vec::new();
        for row in rows {
            schedules.push(row?);
        }
        Ok(schedules)
    }

    pub fn list_enabled(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM dispatch_schedules WHERE is_enabled = 1 ORDER BY scope, created_at")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut schedules = Vec::new();
        for row in rows {
            schedules.push(row?);
        }
        Ok(schedules)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM dispatch_schedules WHERE project_id = ?1 ORDER BY created_at")?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut schedules = Vec::new();
        for row in rows {
            schedules.push(row?);
        }
        Ok(schedules)
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateDispatchSchedule) -> Result<Self> {
        let existing = Self::get_by_id(conn, id)?;
        let cron_expression = input.cron_expression.as_deref().unwrap_or(&existing.cron_expression);
        let action = input.action.as_deref().unwrap_or(&existing.action);
        let timezone = input.timezone.as_deref().unwrap_or(&existing.timezone);
        let is_enabled = input.is_enabled.unwrap_or(existing.is_enabled);
        let description = input.description.as_deref().or(existing.description.as_deref());

        conn.execute(
            "UPDATE dispatch_schedules SET cron_expression = ?1, action = ?2, timezone = ?3, is_enabled = ?4, description = ?5 WHERE id = ?6",
            params![cron_expression, action, timezone, is_enabled as i64, description, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM dispatch_schedules WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("schedule {}", id)));
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
    fn test_create_global_schedule() {
        let conn = setup_db();
        let input = CreateDispatchSchedule {
            scope: "global".to_string(),
            project_id: None,
            cron_expression: "0 9 * * 1-5".to_string(),
            action: "resume".to_string(),
            timezone: Some("Europe/London".to_string()),
            description: Some("Weekday start".to_string()),
        };
        let schedule = DispatchSchedule::create(&conn, &input).unwrap();
        assert_eq!(schedule.scope, "global");
        assert_eq!(schedule.action, "resume");
        assert!(schedule.is_enabled);
    }

    #[test]
    fn test_create_project_schedule() {
        let conn = setup_db();
        // Create a project first
        let proj = crate::models::project::Project::create(&conn, &crate::models::project::CreateProject {
            name: "Test".to_string(),
            directory: "/tmp/test".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        }).unwrap();

        let input = CreateDispatchSchedule {
            scope: "project".to_string(),
            project_id: Some(proj.id.clone()),
            cron_expression: "0 18 * * 1-5".to_string(),
            action: "pause".to_string(),
            timezone: None,
            description: None,
        };
        let schedule = DispatchSchedule::create(&conn, &input).unwrap();
        assert_eq!(schedule.scope, "project");
        assert_eq!(schedule.project_id, Some(proj.id));
    }

    #[test]
    fn test_list_enabled() {
        let conn = setup_db();
        let input1 = CreateDispatchSchedule {
            scope: "global".to_string(),
            project_id: None,
            cron_expression: "0 9 * * *".to_string(),
            action: "resume".to_string(),
            timezone: None,
            description: None,
        };
        let input2 = CreateDispatchSchedule {
            scope: "global".to_string(),
            project_id: None,
            cron_expression: "0 18 * * *".to_string(),
            action: "pause".to_string(),
            timezone: None,
            description: None,
        };
        let s1 = DispatchSchedule::create(&conn, &input1).unwrap();
        DispatchSchedule::create(&conn, &input2).unwrap();

        // Disable one
        DispatchSchedule::update(&conn, &s1.id, &UpdateDispatchSchedule {
            cron_expression: None, action: None, timezone: None,
            is_enabled: Some(false), description: None,
        }).unwrap();

        let enabled = DispatchSchedule::list_enabled(&conn).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].action, "pause");
    }

    #[test]
    fn test_delete_schedule() {
        let conn = setup_db();
        let input = CreateDispatchSchedule {
            scope: "global".to_string(),
            project_id: None,
            cron_expression: "0 9 * * *".to_string(),
            action: "resume".to_string(),
            timezone: None,
            description: None,
        };
        let schedule = DispatchSchedule::create(&conn, &input).unwrap();
        DispatchSchedule::delete(&conn, &schedule.id).unwrap();
        assert!(DispatchSchedule::get_by_id(&conn, &schedule.id).is_err());
    }
}
```

**Step 2: Register the module**

Add `pub mod dispatch_schedule;` to `src/models/mod.rs`.

**Step 3: Run tests**

Run: `cargo test --lib models::dispatch_schedule -- --nocapture 2>&1 | tail -30`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/models/dispatch_schedule.rs src/models/mod.rs
git commit -m "feat(killswitch): add DispatchSchedule model with CRUD + tests"
```

---

### Task 5: Add `cron` Crate Dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add cron crate**

Add to `[dependencies]` in `Cargo.toml`:

```toml
cron = "0.15"
chrono-tz = "0.10"
```

The `cron` crate parses cron expressions. `chrono-tz` handles timezone-aware scheduling. Check the latest versions on crates.io if these don't resolve.

**Step 2: Verify it compiles**

Run: `cargo check 2>&1 | tail -10`
Expected: no errors

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(killswitch): add cron and chrono-tz dependencies"
```

---

### Task 6: Sweep Loop — Global + Project Pause Checks

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Add global pause check at top of `sweep()`**

In `sweep()` (around line 548), add a check before `sweep_teams()`:

```rust
// Killswitch: check global dispatch pause
let global_paused = {
    let conn = self.db.lock().unwrap();
    Setting::get_by_key(&conn, "global_dispatch_paused")
        .map(|s| s.value == "true")
        .unwrap_or(false)
};
```

Then wrap the `sweep_teams()` call:

```rust
if global_paused {
    tracing::debug!("Global dispatch paused — skipping team dispatch");
} else {
    if let Err(e) = self.sweep_teams().await {
        tracing::error!("Team sweep error: {}", e);
    }
}
```

**Step 2: Add per-project pause check in `sweep_teams()`**

In `sweep_teams()`, after fetching `active_teams` (line 1181), add a project pause check inside the `for team in &active_teams` loop, before the budget check:

```rust
// Killswitch: check project-level pause
{
    let conn = self.db.lock().unwrap();
    if let Ok(project) = Project::get_by_id(&conn, &team.project_id) {
        if project.is_paused {
            tracing::debug!(project = %team.project_id, "Project paused — skipping team {}", team.id);
            continue;
        }
    }
}
```

Add the import at the top of the file if not already present:
```rust
use crate::models::project::Project;
use crate::models::setting::Setting;
```

**Step 3: Run the full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat(killswitch): add global + project pause checks to sweep loop"
```

---

### Task 7: Sweep Loop — Schedule Evaluation

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Add schedule evaluation helper method**

Add a method to `OrchestratorRunner`:

```rust
/// Evaluate dispatch schedules and auto-pause/resume as needed.
/// Called at the start of each sweep cycle.
fn evaluate_schedules(&self) {
    let conn = self.db.lock().unwrap();
    let schedules = match DispatchSchedule::list_enabled(&conn) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to load dispatch schedules: {}", e);
            return;
        }
    };

    let now_utc = chrono::Utc::now();

    for schedule in &schedules {
        // Parse timezone
        let tz: chrono_tz::Tz = match schedule.timezone.parse() {
            Ok(tz) => tz,
            Err(_) => {
                tracing::warn!(schedule_id = %schedule.id, tz = %schedule.timezone, "Invalid timezone, skipping");
                continue;
            }
        };

        let now_local = now_utc.with_timezone(&tz);

        // Parse cron expression
        let cron_schedule = match cron::Schedule::from_str(&schedule.cron_expression) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(schedule_id = %schedule.id, expr = %schedule.cron_expression, "Invalid cron: {}", e);
                continue;
            }
        };

        // Check if the cron expression matches the current minute (within the 30s sweep window)
        // We check if there's a scheduled time in the last 60 seconds
        let window_start = now_local - chrono::Duration::seconds(60);
        let has_match = cron_schedule
            .after(&window_start)
            .take(1)
            .any(|t| t <= now_local);

        if !has_match {
            continue;
        }

        tracing::info!(
            schedule_id = %schedule.id, action = %schedule.action, scope = %schedule.scope,
            "Dispatch schedule triggered"
        );

        match (schedule.scope.as_str(), schedule.action.as_str()) {
            ("global", "pause") => {
                let _ = Setting::upsert(&conn, "global_dispatch_paused", &UpsertSetting {
                    value: "true".to_string(), category: Some("killswitch".to_string()),
                });
                let _ = Setting::upsert(&conn, "global_paused_at", &UpsertSetting {
                    value: chrono::Utc::now().to_rfc3339(), category: Some("killswitch".to_string()),
                });
                let _ = Setting::upsert(&conn, "global_pause_reason", &UpsertSetting {
                    value: schedule.description.clone().unwrap_or_else(|| "Scheduled pause".to_string()),
                    category: Some("killswitch".to_string()),
                });
            }
            ("global", "resume") => {
                let _ = Setting::upsert(&conn, "global_dispatch_paused", &UpsertSetting {
                    value: "false".to_string(), category: Some("killswitch".to_string()),
                });
            }
            ("project", "pause") => {
                if let Some(ref pid) = schedule.project_id {
                    let reason = schedule.description.as_deref().unwrap_or("Scheduled pause");
                    let _ = Project::pause(&conn, pid, Some(reason));
                }
            }
            ("project", "resume") => {
                if let Some(ref pid) = schedule.project_id {
                    let _ = Project::resume(&conn, pid);
                }
            }
            _ => {}
        }
    }
}
```

**Step 2: Call evaluate_schedules at the start of sweep()**

In `sweep()`, before the global pause check added in Task 6:

```rust
// Killswitch: evaluate cron schedules
self.evaluate_schedules();
```

Add necessary imports at top of file:

```rust
use crate::models::dispatch_schedule::DispatchSchedule;
use crate::models::setting::UpsertSetting;
use std::str::FromStr;
```

**Step 3: Run tests**

Run: `cargo test 2>&1 | tail -20`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat(killswitch): add cron schedule evaluation to sweep loop"
```

---

### Task 8: API Handler — Global Dispatch Control

**Files:**
- Create: `src/api/dispatch.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`

**Step 1: Create the dispatch API handler**

Create `src/api/dispatch.rs`:

```rust
use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::setting::{Setting, UpsertSetting};
use crate::models::project::Project;
use crate::models::dispatch_schedule::{DispatchSchedule, CreateDispatchSchedule, UpdateDispatchSchedule};

#[derive(Debug, Deserialize)]
pub struct PauseRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DispatchStatus {
    pub paused: bool,
    pub paused_at: Option<String>,
    pub reason: Option<String>,
    pub active_schedules: Vec<DispatchSchedule>,
}

// ── Global dispatch ──────────────────────────────────────────

pub async fn global_pause(
    State(state): State<AppState>,
    Json(input): Json<PauseRequest>,
) -> Result<Json<DispatchStatus>, StatusCode> {
    let conn = state.conn()?;
    Setting::upsert(&conn, "global_dispatch_paused", &UpsertSetting {
        value: "true".to_string(), category: Some("killswitch".to_string()),
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Setting::upsert(&conn, "global_paused_at", &UpsertSetting {
        value: chrono::Utc::now().to_rfc3339(), category: Some("killswitch".to_string()),
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(reason) = &input.reason {
        Setting::upsert(&conn, "global_pause_reason", &UpsertSetting {
            value: reason.clone(), category: Some("killswitch".to_string()),
        }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    drop(conn);
    global_status(State(state)).await
}

pub async fn global_resume(
    State(state): State<AppState>,
) -> Result<Json<DispatchStatus>, StatusCode> {
    let conn = state.conn()?;
    Setting::upsert(&conn, "global_dispatch_paused", &UpsertSetting {
        value: "false".to_string(), category: Some("killswitch".to_string()),
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = Setting::delete(&conn, "global_paused_at");
    let _ = Setting::delete(&conn, "global_pause_reason");
    drop(conn);
    global_status(State(state)).await
}

pub async fn global_status(
    State(state): State<AppState>,
) -> Result<Json<DispatchStatus>, StatusCode> {
    let conn = state.conn()?;
    let paused = Setting::get_by_key(&conn, "global_dispatch_paused")
        .map(|s| s.value == "true").unwrap_or(false);
    let paused_at = Setting::get_by_key(&conn, "global_paused_at")
        .map(|s| s.value).ok();
    let reason = Setting::get_by_key(&conn, "global_pause_reason")
        .map(|s| s.value).ok();
    let active_schedules = DispatchSchedule::list(&conn)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.scope == "global")
        .collect();
    Ok(Json(DispatchStatus { paused, paused_at, reason, active_schedules }))
}

// ── Per-project dispatch ─────────────────────────────────────

pub async fn project_pause(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Json(input): Json<PauseRequest>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.conn()?;
    Project::pause(&conn, &pid, input.reason.as_deref())
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn project_resume(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.conn()?;
    Project::resume(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

#[derive(Debug, Serialize)]
pub struct ProjectDispatchStatus {
    pub paused: bool,
    pub paused_at: Option<String>,
    pub reason: Option<String>,
    pub global_override: bool,
    pub schedules: Vec<DispatchSchedule>,
}

pub async fn project_status(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<Json<ProjectDispatchStatus>, StatusCode> {
    let conn = state.conn()?;
    let project = Project::get_by_id(&conn, &pid).map_err(|_| StatusCode::NOT_FOUND)?;
    let global_override = Setting::get_by_key(&conn, "global_dispatch_paused")
        .map(|s| s.value == "true").unwrap_or(false);
    let schedules = DispatchSchedule::list_by_project(&conn, &pid).unwrap_or_default();
    Ok(Json(ProjectDispatchStatus {
        paused: project.is_paused,
        paused_at: project.paused_at,
        reason: project.pause_reason,
        global_override,
        schedules,
    }))
}

// ── Schedules CRUD ───────────────────────────────────────────

pub async fn list_schedules(
    State(state): State<AppState>,
) -> Result<Json<Vec<DispatchSchedule>>, StatusCode> {
    let conn = state.conn()?;
    DispatchSchedule::list(&conn)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn create_schedule(
    State(state): State<AppState>,
    Json(input): Json<CreateDispatchSchedule>,
) -> Result<(StatusCode, Json<DispatchSchedule>), StatusCode> {
    // Validate cron expression
    use std::str::FromStr;
    if cron::Schedule::from_str(&input.cron_expression).is_err() {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Validate timezone if provided
    if let Some(ref tz) = input.timezone {
        if tz.parse::<chrono_tz::Tz>().is_err() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let conn = state.conn()?;
    DispatchSchedule::create(&conn, &input)
        .map(|s| (StatusCode::CREATED, Json(s)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DispatchSchedule>, StatusCode> {
    let conn = state.conn()?;
    DispatchSchedule::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn update_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateDispatchSchedule>,
) -> Result<Json<DispatchSchedule>, StatusCode> {
    if let Some(ref expr) = input.cron_expression {
        use std::str::FromStr;
        if cron::Schedule::from_str(expr).is_err() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref tz) = input.timezone {
        if tz.parse::<chrono_tz::Tz>().is_err() {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let conn = state.conn()?;
    DispatchSchedule::update(&conn, &id, &input)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    DispatchSchedule::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

**Step 2: Register in api/mod.rs**

Add `pub mod dispatch;` to `src/api/mod.rs`.

**Step 3: Register routes in main.rs**

Add after the settings routes (around line 198):

```rust
// Dispatch killswitch
.route("/api/dispatch/pause", post(api::dispatch::global_pause))
.route("/api/dispatch/resume", post(api::dispatch::global_resume))
.route("/api/dispatch/status", get(api::dispatch::global_status))
.route("/api/projects/{pid}/dispatch/pause", post(api::dispatch::project_pause))
.route("/api/projects/{pid}/dispatch/resume", post(api::dispatch::project_resume))
.route("/api/projects/{pid}/dispatch/status", get(api::dispatch::project_status))
.route("/api/dispatch/schedules", get(api::dispatch::list_schedules).post(api::dispatch::create_schedule))
.route("/api/dispatch/schedules/{id}", get(api::dispatch::get_schedule).put(api::dispatch::update_schedule).delete(api::dispatch::delete_schedule))
```

**Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -20`
Expected: no errors

**Step 5: Commit**

```bash
git add src/api/dispatch.rs src/api/mod.rs src/main.rs
git commit -m "feat(killswitch): add dispatch API endpoints for pause/resume/schedules"
```

---

### Task 9: Seed Default Settings

**Files:**
- Modify: `src/main.rs` (in the settings seed block)

**Step 1: Seed global_dispatch_paused default**

In the settings seed block in `main()` (around line 52-66), add:

```rust
// Seed killswitch defaults
Setting::seed(&conn, "global_dispatch_paused", "false", "killswitch").unwrap_or(());
```

**Step 2: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: no errors

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(killswitch): seed global_dispatch_paused default"
```

---

### Task 10: Frontend — API Client Functions

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add TypeScript types**

Add near the other type definitions:

```typescript
export interface DispatchStatus {
  paused: boolean;
  paused_at: string | null;
  reason: string | null;
  active_schedules: DispatchSchedule[];
}

export interface ProjectDispatchStatus {
  paused: boolean;
  paused_at: string | null;
  reason: string | null;
  global_override: boolean;
  schedules: DispatchSchedule[];
}

export interface DispatchSchedule {
  id: string;
  scope: 'global' | 'project';
  project_id: string | null;
  cron_expression: string;
  action: 'resume' | 'pause';
  timezone: string;
  is_enabled: boolean;
  created_at: string;
  description: string | null;
}

export interface CreateDispatchSchedule {
  scope: 'global' | 'project';
  project_id?: string;
  cron_expression: string;
  action: 'resume' | 'pause';
  timezone?: string;
  description?: string;
}
```

**Step 2: Add API functions**

```typescript
// Dispatch killswitch
export async function getGlobalDispatchStatus(): Promise<DispatchStatus> {
  const res = await fetch(`${API}/dispatch/status`);
  return res.json();
}

export async function pauseGlobalDispatch(reason?: string): Promise<DispatchStatus> {
  const res = await fetch(`${API}/dispatch/pause`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ reason }),
  });
  return res.json();
}

export async function resumeGlobalDispatch(): Promise<DispatchStatus> {
  const res = await fetch(`${API}/dispatch/resume`, { method: 'POST' });
  return res.json();
}

export async function getProjectDispatchStatus(pid: string): Promise<ProjectDispatchStatus> {
  const res = await fetch(`${API}/projects/${pid}/dispatch/status`);
  return res.json();
}

export async function pauseProjectDispatch(pid: string, reason?: string): Promise<any> {
  const res = await fetch(`${API}/projects/${pid}/dispatch/pause`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ reason }),
  });
  return res.json();
}

export async function resumeProjectDispatch(pid: string): Promise<any> {
  const res = await fetch(`${API}/projects/${pid}/dispatch/resume`, { method: 'POST' });
  return res.json();
}

export async function listDispatchSchedules(): Promise<DispatchSchedule[]> {
  const res = await fetch(`${API}/dispatch/schedules`);
  return res.json();
}

export async function createDispatchSchedule(input: CreateDispatchSchedule): Promise<DispatchSchedule> {
  const res = await fetch(`${API}/dispatch/schedules`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });
  return res.json();
}

export async function updateDispatchSchedule(id: string, input: Partial<DispatchSchedule>): Promise<DispatchSchedule> {
  const res = await fetch(`${API}/dispatch/schedules/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });
  return res.json();
}

export async function deleteDispatchSchedule(id: string): Promise<void> {
  await fetch(`${API}/dispatch/schedules/${id}`, { method: 'DELETE' });
}
```

**Step 3: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat(killswitch): add dispatch API client types and functions"
```

---

### Task 11: Frontend — Global Killswitch Component

**Files:**
- Create: `frontend/src/lib/components/KillSwitch.svelte`

**Step 1: Create the KillSwitch component**

This is a self-contained toggle component that shows global dispatch status with pause/resume and schedule management. It will be embedded in the Dashboard header.

```svelte
<script lang="ts">
  import {
    getGlobalDispatchStatus, pauseGlobalDispatch, resumeGlobalDispatch,
    listDispatchSchedules, createDispatchSchedule, deleteDispatchSchedule,
    updateDispatchSchedule,
    type DispatchStatus, type DispatchSchedule
  } from '$lib/api';
  import { onMount } from 'svelte';

  let status: DispatchStatus | null = $state(null);
  let schedules: DispatchSchedule[] = $state([]);
  let showSchedules = $state(false);
  let loading = $state(false);
  let pauseReason = $state('');

  // New schedule form
  let newCron = $state('');
  let newAction: 'pause' | 'resume' = $state('pause');
  let newTz = $state('Europe/London');
  let newDesc = $state('');

  async function refresh() {
    status = await getGlobalDispatchStatus();
    schedules = await listDispatchSchedules();
  }

  onMount(() => { refresh(); const iv = setInterval(refresh, 15000); return () => clearInterval(iv); });

  async function togglePause() {
    loading = true;
    if (status?.paused) {
      status = await resumeGlobalDispatch();
    } else {
      status = await pauseGlobalDispatch(pauseReason || undefined);
      pauseReason = '';
    }
    loading = false;
  }

  async function addSchedule() {
    if (!newCron) return;
    await createDispatchSchedule({
      scope: 'global', cron_expression: newCron, action: newAction,
      timezone: newTz, description: newDesc || undefined,
    });
    newCron = ''; newDesc = '';
    await refresh();
  }

  async function removeSchedule(id: string) {
    await deleteDispatchSchedule(id);
    await refresh();
  }

  async function toggleSchedule(s: DispatchSchedule) {
    await updateDispatchSchedule(s.id, { is_enabled: !s.is_enabled });
    await refresh();
  }
</script>

<div class="killswitch">
  {#if status}
    <div class="ks-status" class:paused={status.paused} class:active={!status.paused}>
      <div class="ks-indicator">
        <span class="ks-dot"></span>
        <span class="ks-label">
          {status.paused ? 'Dispatch Paused' : 'Dispatch Active'}
        </span>
        {#if status.paused && status.reason}
          <span class="ks-reason">({status.reason})</span>
        {/if}
      </div>

      <div class="ks-controls">
        {#if !status.paused}
          <input type="text" bind:value={pauseReason} placeholder="Reason (optional)" class="ks-reason-input" />
        {/if}
        <button onclick={togglePause} disabled={loading} class="ks-toggle"
          class:btn-danger={!status.paused} class:btn-success={status.paused}>
          {status.paused ? 'Resume' : 'Pause All'}
        </button>
        <button onclick={() => showSchedules = !showSchedules} class="ks-schedule-btn">
          Schedules ({schedules.filter(s => s.scope === 'global').length})
        </button>
      </div>
    </div>

    {#if showSchedules}
      <div class="ks-schedules">
        <h4>Dispatch Schedules</h4>
        {#each schedules.filter(s => s.scope === 'global') as s}
          <div class="ks-schedule-row">
            <span class="ks-cron">{s.cron_expression}</span>
            <span class="ks-action" class:pause={s.action === 'pause'} class:resume={s.action === 'resume'}>{s.action}</span>
            <span class="ks-tz">{s.timezone}</span>
            {#if s.description}<span class="ks-desc">{s.description}</span>{/if}
            <button onclick={() => toggleSchedule(s)} class="ks-sm-btn">
              {s.is_enabled ? 'Disable' : 'Enable'}
            </button>
            <button onclick={() => removeSchedule(s.id)} class="ks-sm-btn danger">Delete</button>
          </div>
        {/each}

        <div class="ks-add-schedule">
          <input bind:value={newCron} placeholder="Cron (e.g. 0 9 * * 1-5)" />
          <select bind:value={newAction}>
            <option value="pause">Pause</option>
            <option value="resume">Resume</option>
          </select>
          <input bind:value={newTz} placeholder="Timezone" />
          <input bind:value={newDesc} placeholder="Description" />
          <button onclick={addSchedule} class="ks-sm-btn">Add</button>
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .killswitch { margin-bottom: 1rem; }
  .ks-status { display: flex; justify-content: space-between; align-items: center; padding: 0.75rem 1rem; border-radius: 8px; }
  .ks-status.paused { background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.3); }
  .ks-status.active { background: rgba(34, 197, 94, 0.1); border: 1px solid rgba(34, 197, 94, 0.3); }
  .ks-indicator { display: flex; align-items: center; gap: 0.5rem; }
  .ks-dot { width: 10px; height: 10px; border-radius: 50%; }
  .paused .ks-dot { background: #ef4444; }
  .active .ks-dot { background: #22c55e; }
  .ks-label { font-weight: 600; font-size: 0.9rem; }
  .ks-reason { font-size: 0.8rem; opacity: 0.7; }
  .ks-controls { display: flex; gap: 0.5rem; align-items: center; }
  .ks-reason-input { padding: 0.35rem 0.5rem; border-radius: 4px; border: 1px solid #555; background: #1a1a2e; color: #e0e0e0; font-size: 0.8rem; width: 160px; }
  .ks-toggle { padding: 0.4rem 1rem; border-radius: 6px; border: none; cursor: pointer; font-weight: 600; font-size: 0.85rem; }
  .btn-danger { background: #ef4444; color: white; }
  .btn-danger:hover { background: #dc2626; }
  .btn-success { background: #22c55e; color: white; }
  .btn-success:hover { background: #16a34a; }
  .ks-schedule-btn { padding: 0.4rem 0.75rem; border-radius: 6px; border: 1px solid #555; background: transparent; color: #e0e0e0; cursor: pointer; font-size: 0.8rem; }
  .ks-schedules { margin-top: 0.75rem; padding: 1rem; background: #1a1a2e; border-radius: 8px; border: 1px solid #333; }
  .ks-schedules h4 { margin: 0 0 0.75rem 0; font-size: 0.9rem; }
  .ks-schedule-row { display: flex; align-items: center; gap: 0.75rem; padding: 0.5rem 0; border-bottom: 1px solid #2a2a3e; font-size: 0.85rem; }
  .ks-cron { font-family: monospace; background: #2a2a3e; padding: 0.2rem 0.5rem; border-radius: 4px; }
  .ks-action.pause { color: #ef4444; }
  .ks-action.resume { color: #22c55e; }
  .ks-tz { opacity: 0.6; font-size: 0.8rem; }
  .ks-desc { opacity: 0.7; }
  .ks-sm-btn { padding: 0.2rem 0.5rem; border-radius: 4px; border: 1px solid #555; background: transparent; color: #e0e0e0; cursor: pointer; font-size: 0.75rem; }
  .ks-sm-btn.danger { border-color: #ef4444; color: #ef4444; }
  .ks-add-schedule { display: flex; gap: 0.5rem; margin-top: 0.75rem; padding-top: 0.75rem; border-top: 1px solid #333; }
  .ks-add-schedule input, .ks-add-schedule select { padding: 0.35rem 0.5rem; border-radius: 4px; border: 1px solid #555; background: #1a1a2e; color: #e0e0e0; font-size: 0.8rem; }
  .ks-add-schedule input:first-child { width: 160px; }
</style>
```

**Step 2: Commit**

```bash
git add frontend/src/lib/components/KillSwitch.svelte
git commit -m "feat(killswitch): add KillSwitch global toggle component"
```

---

### Task 12: Frontend — Embed KillSwitch in Dashboard

**Files:**
- Modify: `frontend/src/routes/Dashboard.svelte`

**Step 1: Import and add KillSwitch component**

Add import at the top of the script:

```typescript
import KillSwitch from '$lib/components/KillSwitch.svelte';
```

Add `<KillSwitch />` near the top of the template, before the main dashboard content (after the page title/header area).

**Step 2: Commit**

```bash
git add frontend/src/routes/Dashboard.svelte
git commit -m "feat(killswitch): embed global KillSwitch in dashboard"
```

---

### Task 13: Frontend — Project-Level Pause in ProjectDetail

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte`

**Step 1: Add project dispatch status and controls**

Add to the script section:

```typescript
import {
  getProjectDispatchStatus, pauseProjectDispatch, resumeProjectDispatch,
  type ProjectDispatchStatus
} from '$lib/api';

let dispatchStatus: ProjectDispatchStatus | null = $state(null);

// In the onMount or data-loading section, add:
// dispatchStatus = await getProjectDispatchStatus(projectId);
```

Add a pause/resume toggle in the project header area (near the existing team activate/deactivate buttons):

```svelte
{#if dispatchStatus}
  <div class="project-dispatch" class:paused={dispatchStatus.paused || dispatchStatus.global_override}>
    {#if dispatchStatus.global_override}
      <span class="dispatch-badge override">Global Pause Active</span>
    {:else if dispatchStatus.paused}
      <span class="dispatch-badge paused">Paused</span>
      <button onclick={async () => { await resumeProjectDispatch(projectId); dispatchStatus = await getProjectDispatchStatus(projectId); }}>
        Resume
      </button>
    {:else}
      <span class="dispatch-badge active">Active</span>
      <button onclick={async () => { await pauseProjectDispatch(projectId); dispatchStatus = await getProjectDispatchStatus(projectId); }}>
        Pause
      </button>
    {/if}
  </div>
{/if}
```

**Step 2: Commit**

```bash
git add frontend/src/routes/ProjectDetail.svelte
git commit -m "feat(killswitch): add per-project pause/resume to ProjectDetail"
```

---

### Task 14: Integration Testing — Deploy and Verify

**Files:** None (testing only)

**Step 1: Build locally and run tests**

Run: `cargo test 2>&1 | tail -30`
Expected: ALL PASS

**Step 2: Test API endpoints with curl (after deploy)**

```bash
# Check global status
curl -sk https://localhost/api/dispatch/status | python3 -m json.tool

# Pause globally
curl -sk -X POST https://localhost/api/dispatch/pause -H 'Content-Type: application/json' -d '{"reason":"testing killswitch"}'

# Check status shows paused
curl -sk https://localhost/api/dispatch/status | python3 -m json.tool

# Resume
curl -sk -X POST https://localhost/api/dispatch/resume

# Add a schedule
curl -sk -X POST https://localhost/api/dispatch/schedules -H 'Content-Type: application/json' -d '{"scope":"global","cron_expression":"0 0 9 * * Mon-Fri *","action":"resume","timezone":"Europe/London","description":"Weekday start"}'

# List schedules
curl -sk https://localhost/api/dispatch/schedules | python3 -m json.tool
```

**Step 3: Commit (no files to commit — this is verification)**

---

### Task 15: Deploy to Production

Follow the standard deploy procedure from CLAUDE.md:

```bash
# 1. Rsync
rsync -az --exclude target --exclude .git --exclude node_modules \
  --exclude data/ --exclude certs/ --exclude ironweave.toml \
  --exclude ironweave.db --exclude mounts/ --exclude test-projects/ \
  /Users/paddyharker/task2/ paddy@10.202.28.205:/home/paddy/ironweave/

# 2. Frontend build
ssh paddy@10.202.28.205 "cd /home/paddy/ironweave/frontend && npm run build 2>&1"

# 3. Cargo build
ssh paddy@10.202.28.205 "source ~/.cargo/env && cd /home/paddy/ironweave && cargo clean -p ironweave && cargo build --release 2>&1"

# 4. Restart
ssh paddy@10.202.28.205 "sudo systemctl restart ironweave && sleep 2 && systemctl status ironweave | head -10"

# 5. Verify
ssh paddy@10.202.28.205 "curl -sk https://localhost/api/dispatch/status"
```
