# Ironweave v1 Gap Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close all remaining v1 feature gaps: workflow execution loop, coordination mode enforcement, merge queue with auto-resolver, dashboard analytics, and frontend execution UIs.

**Architecture:** Five independent feature areas, each adding to existing modules. The orchestrator sweep loop (`runner.rs`) is the integration point for workflow execution, coordination modes, and merge queue processing. Dashboard gets new API endpoints and a `activity_log` table. Frontend adds D3 charts and execution controls to existing pages.

**Tech Stack:** Rust (Axum, rusqlite, git2, sysinfo), Svelte 5, D3.js, TypeScript

---

### Task 1: Activity log table and event recording

**Files:**
- Modify: `src/db/migrations.rs` — add `activity_log` table migration
- Create: `src/models/activity_log.rs` — model with create/query methods
- Modify: `src/models/mod.rs` — add `pub mod activity_log;`

**Step 1: Add the migration**

In `src/db/migrations.rs`, add a new migration to the migration list that creates the `activity_log` table:

```sql
CREATE TABLE IF NOT EXISTS activity_log (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    project_id TEXT,
    team_id TEXT,
    agent_id TEXT,
    issue_id TEXT,
    workflow_instance_id TEXT,
    message TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_activity_log_created ON activity_log(created_at);
CREATE INDEX IF NOT EXISTS idx_activity_log_project ON activity_log(project_id);
CREATE INDEX IF NOT EXISTS idx_activity_log_type ON activity_log(event_type);
```

**Step 2: Create the model**

Create `src/models/activity_log.rs`:

```rust
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLogEntry {
    pub id: String,
    pub event_type: String,
    pub project_id: Option<String>,
    pub team_id: Option<String>,
    pub agent_id: Option<String>,
    pub issue_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub message: String,
    pub metadata: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct LogEvent {
    pub event_type: String,
    pub project_id: Option<String>,
    pub team_id: Option<String>,
    pub agent_id: Option<String>,
    pub issue_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

impl ActivityLogEntry {
    pub fn log(conn: &Connection, event: &LogEvent) -> Result<String, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let metadata = event.metadata.as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string());
        conn.execute(
            "INSERT INTO activity_log (id, event_type, project_id, team_id, agent_id, issue_id, workflow_instance_id, message, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, event.event_type, event.project_id, event.team_id, event.agent_id,
                    event.issue_id, event.workflow_instance_id, event.message, metadata],
        )?;
        Ok(id)
    }

    pub fn list_recent(conn: &Connection, limit: usize, offset: usize) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, event_type, project_id, team_id, agent_id, issue_id, workflow_instance_id, message, metadata, created_at
             FROM activity_log ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(Self {
                id: row.get(0)?,
                event_type: row.get(1)?,
                project_id: row.get(2)?,
                team_id: row.get(3)?,
                agent_id: row.get(4)?,
                issue_id: row.get(5)?,
                workflow_instance_id: row.get(6)?,
                message: row.get(7)?,
                metadata: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, limit: usize) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, event_type, project_id, team_id, agent_id, issue_id, workflow_instance_id, message, metadata, created_at
             FROM activity_log WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![project_id, limit], |row| {
            Ok(Self {
                id: row.get(0)?,
                event_type: row.get(1)?,
                project_id: row.get(2)?,
                team_id: row.get(3)?,
                agent_id: row.get(4)?,
                issue_id: row.get(5)?,
                workflow_instance_id: row.get(6)?,
                message: row.get(7)?,
                metadata: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Get metrics: issues opened/closed per day, agent sessions per day
    pub fn daily_metrics(conn: &Connection, days: i64) -> Result<Vec<DailyMetric>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT date(created_at) as day, event_type, COUNT(*) as count
             FROM activity_log
             WHERE created_at >= datetime('now', ?1)
             GROUP BY day, event_type
             ORDER BY day"
        )?;
        let offset = format!("-{} days", days);
        let rows = stmt.query_map(params![offset], |row| {
            Ok(DailyMetric {
                day: row.get(0)?,
                event_type: row.get(1)?,
                count: row.get(2)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyMetric {
    pub day: String,
    pub event_type: String,
    pub count: i64,
}
```

**Step 3: Add module to `src/models/mod.rs`**

Add `pub mod activity_log;` after the existing module declarations.

**Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -5`

**Step 5: Commit**

```bash
git add src/db/migrations.rs src/models/activity_log.rs src/models/mod.rs
git commit -m "feat: add activity_log table and model for orchestrator event tracking"
```

---

### Task 2: Dashboard API endpoints (activity, metrics, system)

**Files:**
- Modify: `src/api/dashboard.rs` — add activity, metrics, and system endpoints
- Modify: `src/main.rs` — add new routes
- Modify: `Cargo.toml` — add `sysinfo` dependency

**Step 1: Add sysinfo dependency**

Add to `[dependencies]` in `Cargo.toml`:
```toml
sysinfo = "0.33"
```

**Step 2: Expand dashboard.rs**

Replace `src/api/dashboard.rs` with expanded version adding three new endpoints:

```rust
use axum::{extract::{State, Query}, Json};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::activity_log::{ActivityLogEntry, DailyMetric};

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub project_count: usize,
    pub active_agents: usize,
    pub open_issues: usize,
    pub running_workflows: usize,
}

pub async fn stats(State(state): State<AppState>) -> Json<DashboardStats> {
    let conn = state.db.lock().unwrap();
    let project_count: usize = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .unwrap_or(0);
    let open_issues: usize = conn
        .query_row("SELECT COUNT(*) FROM issues WHERE status != 'closed'", [], |row| row.get(0))
        .unwrap_or(0);
    let running_workflows: usize = conn
        .query_row("SELECT COUNT(*) FROM workflow_instances WHERE state = 'running'", [], |row| row.get(0))
        .unwrap_or(0);
    let active_agents = state.process_manager.list_sessions().len();

    Json(DashboardStats { project_count, active_agents, open_issues, running_workflows })
}

#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub project_id: Option<String>,
}

pub async fn activity(
    State(state): State<AppState>,
    Query(query): Query<ActivityQuery>,
) -> Json<Vec<ActivityLogEntry>> {
    let conn = state.db.lock().unwrap();
    let limit = query.limit.unwrap_or(50);
    let entries = if let Some(pid) = &query.project_id {
        ActivityLogEntry::list_by_project(&conn, pid, limit).unwrap_or_default()
    } else {
        let offset = query.offset.unwrap_or(0);
        ActivityLogEntry::list_recent(&conn, limit, offset).unwrap_or_default()
    };
    Json(entries)
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub daily: Vec<DailyMetric>,
    pub merge_stats: MergeStats,
    pub avg_resolution_hours: f64,
}

#[derive(Debug, Serialize)]
pub struct MergeStats {
    pub total: i64,
    pub clean: i64,
    pub conflicted: i64,
    pub escalated: i64,
}

pub async fn metrics(
    State(state): State<AppState>,
    Query(query): Query<MetricsQuery>,
) -> Json<MetricsResponse> {
    let conn = state.db.lock().unwrap();
    let days = query.days.unwrap_or(7);
    let daily = ActivityLogEntry::daily_metrics(&conn, days).unwrap_or_default();

    // Merge stats from activity log
    let offset = format!("-{} days", days);
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM activity_log WHERE event_type LIKE 'merge_%' AND created_at >= datetime('now', ?1)",
        [&offset], |row| row.get(0)
    ).unwrap_or(0);
    let clean: i64 = conn.query_row(
        "SELECT COUNT(*) FROM activity_log WHERE event_type = 'merge_success' AND created_at >= datetime('now', ?1)",
        [&offset], |row| row.get(0)
    ).unwrap_or(0);
    let conflicted: i64 = conn.query_row(
        "SELECT COUNT(*) FROM activity_log WHERE event_type = 'merge_conflict' AND created_at >= datetime('now', ?1)",
        [&offset], |row| row.get(0)
    ).unwrap_or(0);
    let escalated: i64 = conn.query_row(
        "SELECT COUNT(*) FROM activity_log WHERE event_type = 'merge_escalated' AND created_at >= datetime('now', ?1)",
        [&offset], |row| row.get(0)
    ).unwrap_or(0);

    // Average resolution time (hours between issue open and close)
    let avg_resolution_hours: f64 = conn.query_row(
        "SELECT COALESCE(AVG((julianday(updated_at) - julianday(created_at)) * 24), 0)
         FROM issues WHERE status = 'closed' AND created_at >= datetime('now', ?1)",
        [&offset], |row| row.get(0)
    ).unwrap_or(0.0);

    Json(MetricsResponse {
        daily,
        merge_stats: MergeStats { total, clean, conflicted, escalated },
        avg_resolution_hours,
    })
}

#[derive(Debug, Serialize)]
pub struct SystemHealth {
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub agent_process_count: usize,
}

pub async fn system(State(state): State<AppState>) -> Json<SystemHealth> {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_usage();
    let memory_used_mb = sys.used_memory() / 1024 / 1024;
    let memory_total_mb = sys.total_memory() / 1024 / 1024;

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let (disk_used_gb, disk_total_gb) = disks.list().first().map(|d| {
        let total = d.total_space() as f64 / 1_073_741_824.0;
        let available = d.available_space() as f64 / 1_073_741_824.0;
        (total - available, total)
    }).unwrap_or((0.0, 0.0));

    let agent_process_count = state.process_manager.list_sessions().len();

    Json(SystemHealth {
        cpu_usage_percent: cpu_usage,
        memory_used_mb,
        memory_total_mb,
        disk_used_gb,
        disk_total_gb,
        agent_process_count,
    })
}
```

**Step 3: Add routes to `src/main.rs`**

After the existing `.route("/api/dashboard", get(api::dashboard::stats))` line, add:

```rust
        .route("/api/dashboard/activity", get(api::dashboard::activity))
        .route("/api/dashboard/metrics", get(api::dashboard::metrics))
        .route("/api/dashboard/system", get(api::dashboard::system))
```

**Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -5`

**Step 5: Commit**

```bash
git add src/api/dashboard.rs src/main.rs Cargo.toml
git commit -m "feat: add dashboard activity feed, metrics, and system health endpoints"
```

---

### Task 3: Merge queue model and API

**Files:**
- Create: `src/models/merge_queue_entry.rs` — SQLite-persisted merge queue model
- Modify: `src/models/mod.rs` — add module
- Modify: `src/db/migrations.rs` — add merge_queue table
- Create: `src/api/merge_queue.rs` — API endpoints
- Modify: `src/api/mod.rs` — add module
- Modify: `src/main.rs` — add routes

**Step 1: Add migration**

Add to `src/db/migrations.rs`:

```sql
CREATE TABLE IF NOT EXISTS merge_queue (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_name TEXT NOT NULL,
    agent_session_id TEXT,
    issue_id TEXT,
    team_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    conflict_files TEXT DEFAULT '[]',
    resolver_agent_id TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
CREATE INDEX IF NOT EXISTS idx_merge_queue_project ON merge_queue(project_id);
CREATE INDEX IF NOT EXISTS idx_merge_queue_status ON merge_queue(status);
```

**Step 2: Create model**

Create `src/models/merge_queue_entry.rs`:

```rust
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeQueueEntry {
    pub id: String,
    pub project_id: String,
    pub branch_name: String,
    pub agent_session_id: Option<String>,
    pub issue_id: Option<String>,
    pub team_id: Option<String>,
    pub status: String, // pending, merging, conflicted, resolving, resolved, merged, failed
    pub conflict_files: String, // JSON array
    pub resolver_agent_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl MergeQueueEntry {
    pub fn create(conn: &Connection, project_id: &str, branch_name: &str,
                  agent_session_id: Option<&str>, issue_id: Option<&str>,
                  team_id: Option<&str>) -> Result<Self, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO merge_queue (id, project_id, branch_name, agent_session_id, issue_id, team_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, project_id, branch_name, agent_session_id, issue_id, team_id],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self, rusqlite::Error> {
        conn.query_row(
            "SELECT id, project_id, branch_name, agent_session_id, issue_id, team_id,
                    status, conflict_files, resolver_agent_id, error_message, created_at, updated_at
             FROM merge_queue WHERE id = ?1",
            params![id],
            |row| Ok(Self {
                id: row.get(0)?, project_id: row.get(1)?, branch_name: row.get(2)?,
                agent_session_id: row.get(3)?, issue_id: row.get(4)?, team_id: row.get(5)?,
                status: row.get(6)?, conflict_files: row.get(7)?, resolver_agent_id: row.get(8)?,
                error_message: row.get(9)?, created_at: row.get(10)?, updated_at: row.get(11)?,
            }),
        )
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, branch_name, agent_session_id, issue_id, team_id,
                    status, conflict_files, resolver_agent_id, error_message, created_at, updated_at
             FROM merge_queue WHERE project_id = ?1 ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(Self {
                id: row.get(0)?, project_id: row.get(1)?, branch_name: row.get(2)?,
                agent_session_id: row.get(3)?, issue_id: row.get(4)?, team_id: row.get(5)?,
                status: row.get(6)?, conflict_files: row.get(7)?, resolver_agent_id: row.get(8)?,
                error_message: row.get(9)?, created_at: row.get(10)?, updated_at: row.get(11)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn next_pending(conn: &Connection, project_id: &str) -> Result<Option<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, branch_name, agent_session_id, issue_id, team_id,
                    status, conflict_files, resolver_agent_id, error_message, created_at, updated_at
             FROM merge_queue WHERE project_id = ?1 AND status = 'pending'
             ORDER BY created_at ASC LIMIT 1"
        )?;
        let mut rows = stmt.query_map(params![project_id], |row| {
            Ok(Self {
                id: row.get(0)?, project_id: row.get(1)?, branch_name: row.get(2)?,
                agent_session_id: row.get(3)?, issue_id: row.get(4)?, team_id: row.get(5)?,
                status: row.get(6)?, conflict_files: row.get(7)?, resolver_agent_id: row.get(8)?,
                error_message: row.get(9)?, created_at: row.get(10)?, updated_at: row.get(11)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn update_status(conn: &Connection, id: &str, status: &str,
                         conflict_files: Option<&str>, resolver_agent_id: Option<&str>,
                         error_message: Option<&str>) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE merge_queue SET status = ?1, conflict_files = COALESCE(?2, conflict_files),
             resolver_agent_id = COALESCE(?3, resolver_agent_id),
             error_message = ?4, updated_at = datetime('now')
             WHERE id = ?5",
            params![status, conflict_files, resolver_agent_id, error_message, id],
        )?;
        Ok(())
    }
}
```

**Step 3: Create API**

Create `src/api/merge_queue.rs`:

```rust
use axum::{extract::{Path, State}, Json, http::StatusCode};
use crate::state::AppState;
use crate::models::merge_queue_entry::MergeQueueEntry;

pub async fn list_queue(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Json<Vec<MergeQueueEntry>> {
    let conn = state.db.lock().unwrap();
    Json(MergeQueueEntry::list_by_project(&conn, &project_id).unwrap_or_default())
}

pub async fn approve_merge(
    State(state): State<AppState>,
    Path((_project_id, id)): Path<(String, String)>,
) -> Result<Json<MergeQueueEntry>, (StatusCode, String)> {
    let conn = state.db.lock().unwrap();
    MergeQueueEntry::update_status(&conn, &id, "pending", None, None, None)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    MergeQueueEntry::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))
}
```

**Step 4: Add modules and routes**

Add `pub mod merge_queue;` to `src/api/mod.rs`.
Add `pub mod merge_queue_entry;` to `src/models/mod.rs`.

Add routes to `src/main.rs`:
```rust
        .route("/api/projects/{pid}/merge-queue", get(api::merge_queue::list_queue))
        .route("/api/projects/{pid}/merge-queue/{id}/approve", post(api::merge_queue::approve_merge))
```

**Step 5: Verify and commit**

Run: `cargo check 2>&1 | tail -5`

```bash
git add src/models/merge_queue_entry.rs src/api/merge_queue.rs src/models/mod.rs src/api/mod.rs src/main.rs src/db/migrations.rs
git commit -m "feat: add merge queue model and API endpoints"
```

---

### Task 4: Wire activity logging into the orchestrator

**Files:**
- Modify: `src/orchestrator/runner.rs` — add activity log calls at key orchestrator events

**Step 1: Add a helper method to OrchestratorRunner**

Add after the `new()` method (around line 124):

```rust
    fn log_activity(&self, event_type: &str, message: &str,
                    project_id: Option<&str>, team_id: Option<&str>,
                    agent_id: Option<&str>, issue_id: Option<&str>,
                    workflow_instance_id: Option<&str>) {
        use crate::models::activity_log::{ActivityLogEntry, LogEvent};
        let conn = self.db.lock().unwrap();
        let _ = ActivityLogEntry::log(&conn, &LogEvent {
            event_type: event_type.to_string(),
            project_id: project_id.map(|s| s.to_string()),
            team_id: team_id.map(|s| s.to_string()),
            agent_id: agent_id.map(|s| s.to_string()),
            issue_id: issue_id.map(|s| s.to_string()),
            workflow_instance_id: workflow_instance_id.map(|s| s.to_string()),
            message: message.to_string(),
            metadata: None,
        });
    }
```

**Step 2: Add log calls at key events in runner.rs**

Insert `self.log_activity(...)` calls at these locations:

1. **Agent spawned** — in `spawn_team_agent()` after successful PTY spawn (around line 870):
   ```rust
   self.log_activity("agent_spawned", &format!("Agent spawned for role '{}' on issue '{}'", slot.role, issue.title),
       Some(&team.project_id), Some(&team.id), Some(&session_id), Some(&issue.id), None);
   ```

2. **Issue claimed** — in `spawn_team_agent()` after `Issue::claim()` (around line 870):
   ```rust
   self.log_activity("issue_claimed", &format!("Issue '{}' claimed by agent", issue.title),
       Some(&team.project_id), Some(&team.id), Some(&session_id), Some(&issue.id), None);
   ```

3. **Agent completed** — in `sweep_teams()` when agent process exits successfully (around line 630):
   ```rust
   self.log_activity("agent_completed", &format!("Agent completed work on issue"),
       Some(&team_agent.team_id), Some(&team_agent.team_id), Some(&team_agent.agent_session_id),
       Some(&team_agent.issue_id), None);
   ```

4. **Merge enqueued** — in `sweep_teams()` when branch is added to merge queue (around line 641):
   ```rust
   self.log_activity("merge_enqueued", &format!("Branch enqueued for merge"),
       None, Some(&team_agent.team_id), Some(&team_agent.agent_session_id),
       Some(&team_agent.issue_id), None);
   ```

5. **Workflow started** — in `start_workflow()` (around line 245):
   ```rust
   self.log_activity("workflow_started", &format!("Workflow started"),
       Some(&project_id), None, None, None, Some(&instance_id));
   ```

6. **Intake decomposition** — in `sweep_intake()` when intake agent completes:
   ```rust
   self.log_activity("intake_completed", &format!("Intake decomposition completed"),
       None, None, None, Some(&issue_id), None);
   ```

**Step 3: Verify and commit**

Run: `cargo check 2>&1 | tail -5`

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: wire activity logging into orchestrator sweep events"
```

---

### Task 5: Merge queue processing in orchestrator sweep

**Files:**
- Modify: `src/orchestrator/runner.rs` — expand `sweep_merge_queue()` to process entries, attempt merges, and spawn resolver agents on conflict

**Step 1: Implement sweep_merge_queue**

The existing `sweep()` method calls `sweep_merge_queue()` but it's likely a no-op or stub. Replace/implement it (find the existing fn and replace):

```rust
    fn sweep_merge_queue(&mut self) {
        use crate::models::merge_queue_entry::MergeQueueEntry;
        use crate::worktree::merge_queue::MergeQueueProcessor;

        let conn = self.db.lock().unwrap();

        // Get all projects with active teams
        let project_ids: Vec<String> = conn.prepare(
            "SELECT DISTINCT project_id FROM teams WHERE is_active = 1"
        ).and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        }).unwrap_or_default();

        drop(conn);

        for project_id in project_ids {
            let conn = self.db.lock().unwrap();
            let entry = match MergeQueueEntry::next_pending(&conn, &project_id) {
                Ok(Some(e)) => e,
                _ => continue,
            };

            // Get project directory for merge
            let project_dir = match crate::models::project::Project::get_by_id(&conn, &project_id) {
                Ok(p) => p.directory,
                Err(_) => continue,
            };
            drop(conn);

            // Detect default branch
            let target_branch = self.worktree_manager.detect_default_branch(&project_dir)
                .unwrap_or_else(|| "main".to_string());

            // Attempt merge
            let result = MergeQueueProcessor::try_merge(
                &project_dir, &entry.branch_name, &target_branch
            );

            let conn = self.db.lock().unwrap();
            match result {
                crate::worktree::merge_queue::MergeResult::Success => {
                    let _ = MergeQueueEntry::update_status(&conn, &entry.id, "merged", None, None, None);
                    self.log_activity("merge_success",
                        &format!("Branch '{}' merged successfully", entry.branch_name),
                        Some(&project_id), entry.team_id.as_deref(),
                        entry.agent_session_id.as_deref(), entry.issue_id.as_deref(), None);

                    // Clean up worktree
                    let _ = self.worktree_manager.remove_worktree(&project_dir, &entry.branch_name);
                }
                crate::worktree::merge_queue::MergeResult::Conflict { files } => {
                    let files_json = serde_json::to_string(&files).unwrap_or_default();
                    let _ = MergeQueueEntry::update_status(
                        &conn, &entry.id, "conflicted", Some(&files_json), None, None
                    );
                    self.log_activity("merge_conflict",
                        &format!("Branch '{}' has conflicts in {} files", entry.branch_name, files.len()),
                        Some(&project_id), entry.team_id.as_deref(),
                        entry.agent_session_id.as_deref(), entry.issue_id.as_deref(), None);

                    // Auto-spawn resolver agent (T1)
                    // This will be picked up by the next sweep_teams cycle
                    // by creating a resolver issue
                    let _ = self.spawn_resolver_issue(&conn, &entry, &files, &project_id);
                }
                crate::worktree::merge_queue::MergeResult::Error(msg) => {
                    let _ = MergeQueueEntry::update_status(
                        &conn, &entry.id, "failed", None, None, Some(&msg)
                    );
                    self.log_activity("merge_failed",
                        &format!("Merge failed for branch '{}': {}", entry.branch_name, msg),
                        Some(&project_id), entry.team_id.as_deref(),
                        entry.agent_session_id.as_deref(), entry.issue_id.as_deref(), None);
                }
            }
        }
    }

    fn spawn_resolver_issue(&self, conn: &Connection, entry: &crate::models::merge_queue_entry::MergeQueueEntry,
                            conflict_files: &[String], project_id: &str) -> Result<(), String> {
        use crate::models::issue::{Issue, CreateIssue};
        let description = format!(
            "Resolve merge conflicts in branch '{}'. Conflicting files:\n{}",
            entry.branch_name,
            conflict_files.iter().map(|f| format!("- {}", f)).collect::<Vec<_>>().join("\n")
        );
        let create = CreateIssue {
            project_id: project_id.to_string(),
            issue_type: Some("merge_conflict".to_string()),
            title: format!("Resolve merge conflicts: {}", entry.branch_name),
            description: Some(description),
            priority: Some(10), // highest priority
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
            role: Some("senior_coder".to_string()),
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: Some("auto".to_string()),
        };
        let _ = Issue::create(conn, &create).map_err(|e| e.to_string())?;
        Ok(())
    }
```

**Step 2: Verify and commit**

Run: `cargo check 2>&1 | tail -5`

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: implement merge queue processing with auto-resolver in orchestrator sweep"
```

---

### Task 6: Coordination mode dispatch logic

**Files:**
- Modify: `src/orchestrator/runner.rs` — update `sweep_teams()` dispatch to branch on coordination_mode
- Modify: `src/models/team.rs` — add `is_lead` field to `TeamAgentSlot`
- Modify: `src/db/migrations.rs` — add `is_lead` column

**Step 1: Add is_lead migration**

Add migration:
```sql
ALTER TABLE team_agent_slots ADD COLUMN is_lead INTEGER NOT NULL DEFAULT 0;
```

**Step 2: Add is_lead to TeamAgentSlot struct**

In `src/models/team.rs`, add `pub is_lead: bool` to `TeamAgentSlot` struct (after `slot_order`), and update `from_row()` to read it. Also add it to `CreateTeamAgentSlot`.

**Step 3: Implement coordination mode dispatch**

In `src/orchestrator/runner.rs`, find the section in `sweep_teams()` where active teams dispatch agents (around lines 737-798). Wrap the dispatch logic in a match on `team.coordination_mode.as_str()`:

```rust
match team.coordination_mode.as_str() {
    "pipeline" => {
        // Pipeline: enforce slot ordering. Only dispatch to next role
        // when all issues for the previous role are closed.
        let slots = TeamAgentSlot::list_by_team(&conn, &team.id).unwrap_or_default();
        // Find the lowest slot_order that still has open issues
        let mut current_slot = None;
        for slot in &slots {
            let has_open = conn.query_row(
                "SELECT COUNT(*) FROM issues WHERE project_id = ?1 AND role = ?2 AND status != 'closed'",
                params![team.project_id, slot.role], |row| row.get::<_, i64>(0)
            ).unwrap_or(0);
            if has_open > 0 || current_slot.is_none() {
                current_slot = Some(slot.clone());
                if has_open > 0 { break; }
            }
        }
        // Only dispatch for the current pipeline stage's role
        if let Some(slot) = current_slot {
            // Dispatch agents only for this slot's role
            self.dispatch_for_slot(&conn, &team, &slot, &ready_issues);
        }
    }
    "swarm" => {
        // Swarm: existing behaviour — all slots can claim from the pool
        for slot in &slots {
            self.dispatch_for_slot(&conn, &team, &slot, &ready_issues);
        }
    }
    "collaborative" => {
        // Collaborative: assign the same issue to ALL slots simultaneously
        if let Some(issue) = ready_issues.first() {
            for slot in &slots {
                self.dispatch_for_slot_on_issue(&conn, &team, &slot, issue);
            }
        }
    }
    "hierarchical" => {
        // Hierarchical: lead slot gets issues first, non-leads only work children
        let lead_slot = slots.iter().find(|s| s.is_lead);
        let sub_slots: Vec<_> = slots.iter().filter(|s| !s.is_lead).collect();

        if let Some(lead) = lead_slot {
            // Lead works on top-level issues (no parent_id)
            let top_level: Vec<_> = ready_issues.iter()
                .filter(|i| i.parent_id.is_none())
                .collect();
            if let Some(issue) = top_level.first() {
                self.dispatch_for_slot_on_issue(&conn, &team, lead, issue);
            }
        }
        // Sub-agents work on child issues only
        for slot in sub_slots {
            let children: Vec<_> = ready_issues.iter()
                .filter(|i| i.parent_id.is_some())
                .collect();
            if let Some(issue) = children.first() {
                self.dispatch_for_slot_on_issue(&conn, &team, slot, issue);
            }
        }
    }
    _ => {
        // Default to swarm
        for slot in &slots {
            self.dispatch_for_slot(&conn, &team, &slot, &ready_issues);
        }
    }
}
```

Note: `dispatch_for_slot()` and `dispatch_for_slot_on_issue()` are refactored helper methods extracted from the existing spawn logic in `sweep_teams()`. They call `spawn_team_agent()` with the appropriate slot and issue.

**Step 4: Verify and commit**

Run: `cargo check 2>&1 | tail -5`

```bash
git add src/orchestrator/runner.rs src/models/team.rs src/db/migrations.rs
git commit -m "feat: implement pipeline, swarm, collaborative, and hierarchical coordination modes"
```

---

### Task 7: Workflow execution loop

**Files:**
- Modify: `src/orchestrator/runner.rs` — implement `sweep_workflow()` to advance DAG execution through tiers

**Step 1: Enhance sweep_workflow**

The existing `sweep_workflow()` (lines 435-591) already checks stage agent status and handles idle escalation. The gap is **advancing to the next tier** when all stages in the current tier complete, and **spawning agents for the next tier's stages**.

In `sweep_workflow()`, after the completion tracking section (around line 584), add tier advancement logic:

```rust
    // After checking all stage agents, advance to next tier if current is complete
    for (_instance_id, wf) in self.active_workflows.iter_mut() {
        let ready = wf.execution.ready_stages();
        for stage_id in ready {
            if wf.stage_agents.contains_key(&stage_id) {
                continue; // Already has an agent
            }
            // Get stage definition
            if let Some(stage) = wf.dag.stages.iter().find(|s| s.id == stage_id) {
                if stage.is_manual_gate {
                    wf.execution.update_stage(&stage_id, StageStatus::WaitingApproval);
                    continue;
                }
                // Spawn agent for this stage
                let project_id = wf.project_id.clone();
                let instance_id = wf.instance_id.clone();
                let stage_clone = stage.clone();
                // Queue for spawning (can't borrow self mutably in loop)
                stages_to_spawn.push((instance_id, project_id, stage_clone));
            }
        }

        // Check if workflow is complete
        if wf.execution.is_complete() {
            let _ = wf.state_machine.transition(WorkflowState::Completed);
            self.log_activity("workflow_completed", "Workflow completed successfully",
                Some(&wf.project_id), None, None, None, Some(&wf.instance_id));
        }
    }

    // Spawn queued stage agents outside the borrow
    for (instance_id, project_id, stage) in stages_to_spawn {
        self.spawn_stage_agent(&instance_id, &project_id, &stage);
    }
```

**Step 2: Add workflow gate approval endpoint**

Add to `src/api/workflows.rs`:

```rust
pub async fn approve_gate(
    State(state): State<AppState>,
    Path((wid, instance_id, stage_id)): Path<(String, String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Send event to orchestrator to approve the gate
    // The orchestrator will call execution.approve_gate(stage_id)
    Ok(StatusCode::OK)
}
```

Add route: `.route("/api/workflows/{wid}/instances/{iid}/stages/{sid}/approve", post(api::workflows::approve_gate))`

**Step 3: Verify and commit**

Run: `cargo check 2>&1 | tail -5`

```bash
git add src/orchestrator/runner.rs src/api/workflows.rs src/main.rs
git commit -m "feat: implement workflow DAG execution loop with tier advancement and manual gates"
```

---

### Task 8: Frontend — Dashboard with D3 charts

**Files:**
- Modify: `frontend/package.json` — add d3 dependency
- Modify: `frontend/src/lib/api.ts` — add new dashboard API functions
- Create: `frontend/src/lib/components/ActivityFeed.svelte` — scrolling event log
- Create: `frontend/src/lib/components/MetricsChart.svelte` — D3 line chart
- Create: `frontend/src/lib/components/SystemHealth.svelte` — gauge displays
- Modify: `frontend/src/routes/Dashboard.svelte` — integrate new components

**Step 1: Add D3**

```bash
cd frontend && npm install d3 @types/d3 && cd ..
```

**Step 2: Add API functions to api.ts**

Add to the `dashboard` export in `frontend/src/lib/api.ts`:

```typescript
export interface ActivityLogEntry {
  id: string;
  event_type: string;
  project_id: string | null;
  team_id: string | null;
  agent_id: string | null;
  issue_id: string | null;
  message: string;
  created_at: string;
}

export interface DailyMetric {
  day: string;
  event_type: string;
  count: number;
}

export interface MetricsResponse {
  daily: DailyMetric[];
  merge_stats: { total: number; clean: number; conflicted: number; escalated: number };
  avg_resolution_hours: number;
}

export interface SystemHealth {
  cpu_usage_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  disk_used_gb: number;
  disk_total_gb: number;
  agent_process_count: number;
}

export const dashboard = {
  stats: () => get<DashboardStats>('/api/dashboard'),
  activity: (limit = 50, offset = 0) => get<ActivityLogEntry[]>(`/api/dashboard/activity?limit=${limit}&offset=${offset}`),
  metrics: (days = 7) => get<MetricsResponse>(`/api/dashboard/metrics?days=${days}`),
  system: () => get<SystemHealth>('/api/dashboard/system'),
};
```

**Step 3: Create ActivityFeed.svelte**

Create `frontend/src/lib/components/ActivityFeed.svelte` — a scrolling log component that displays activity events with colour-coded type badges.

**Step 4: Create MetricsChart.svelte**

Create `frontend/src/lib/components/MetricsChart.svelte` — a D3 line chart component that takes daily metrics data and renders issues opened/closed over time with a 7d/30d toggle.

**Step 5: Create SystemHealth.svelte**

Create `frontend/src/lib/components/SystemHealth.svelte` — displays CPU, RAM, and disk usage as progress bars with percentage labels.

**Step 6: Update Dashboard.svelte**

Rewrite `frontend/src/routes/Dashboard.svelte` to include:
- Existing stats cards (top row)
- Activity feed (left column, 60%)
- Metrics charts (right column, 40%): issue throughput line chart, merge health doughnut
- System health panel (bottom row)
- Auto-refresh every 5 seconds

**Step 7: Build and verify**

```bash
cd frontend && npm run build && cd ..
```

**Step 8: Commit**

```bash
git add frontend/
git commit -m "feat: add D3 dashboard with activity feed, metrics charts, and system health"
```

---

### Task 9: Frontend — Merge queue panel

**Files:**
- Modify: `frontend/src/lib/api.ts` — add merge queue API functions
- Create: `frontend/src/lib/components/MergeQueue.svelte` — merge queue display
- Modify: `frontend/src/routes/ProjectDetail.svelte` — add merge queue tab/section

**Step 1: Add API functions**

Add to `frontend/src/lib/api.ts`:

```typescript
export interface MergeQueueEntry {
  id: string;
  project_id: string;
  branch_name: string;
  agent_session_id: string | null;
  issue_id: string | null;
  status: string;
  conflict_files: string;
  resolver_agent_id: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export const mergeQueue = {
  list: (projectId: string) => get<MergeQueueEntry[]>(`/api/projects/${projectId}/merge-queue`),
  approve: (projectId: string, id: string) => post<MergeQueueEntry>(`/api/projects/${projectId}/merge-queue/${id}/approve`, {}),
};
```

**Step 2: Create MergeQueue.svelte**

Create `frontend/src/lib/components/MergeQueue.svelte`:
- Lists merge queue entries with status badges (pending=blue, merging=yellow, conflicted=red, merged=green, failed=grey)
- For conflicted entries: show conflict file list, resolver agent status
- For T2 escalation: approve button
- Auto-refreshes every 10 seconds

**Step 3: Add to ProjectDetail**

Add a "merge-queue" tab to ProjectDetail.svelte and render the MergeQueue component when active.

**Step 4: Build and commit**

```bash
cd frontend && npm run build && cd ..
git add frontend/
git commit -m "feat: add merge queue panel to project detail view"
```

---

### Task 10: Frontend — Workflow runner controls and swarm status

**Files:**
- Modify: `frontend/src/lib/api.ts` — add workflow instance control APIs
- Modify: `frontend/src/routes/ProjectDetail.svelte` — add workflow runner controls and swarm status display
- Modify: `frontend/src/lib/components/DagGraph.svelte` — add stage status colouring

**Step 1: Add workflow control APIs**

Add to api.ts workflows section:

```typescript
export const workflows = {
  // ... existing
  instances: {
    list: (wid: string) => get<WorkflowInstance[]>(`/api/workflows/${wid}/instances`),
    create: (wid: string) => post<WorkflowInstance>(`/api/workflows/${wid}/instances`, {}),
    approveGate: (wid: string, iid: string, stageId: string) =>
      post(`/api/workflows/${wid}/instances/${iid}/stages/${stageId}/approve`, {}),
  },
};
```

**Step 2: Update DagGraph for stage colouring**

Modify `frontend/src/lib/components/DagGraph.svelte` to accept a `stageStatuses` prop (map of stage_id → status). Colour nodes:
- Pending: grey (#6b7280)
- Running: blue (#3b82f6)
- WaitingApproval: yellow (#f59e0b)
- Completed: green (#10b981)
- Failed: red (#ef4444)

**Step 3: Add workflow runner controls**

In the workflows tab of ProjectDetail.svelte, for each workflow instance:
- Show current state badge
- Start button (creates instance if none)
- Pause/Resume/Cancel buttons
- DagGraph with live stage status colouring
- Click a stage node to see the assigned agent's terminal

**Step 4: Add swarm status display**

In the teams tab of ProjectDetail.svelte, for each team:
- Coordination mode badge
- Agent counts: active / idle / total
- Task pool depth (ready issues count)
- Per-agent row: current issue title, runtime badge, uptime
- Scaling recommendation indicator

**Step 5: Build and commit**

```bash
cd frontend && npm run build && cd ..
git add frontend/
git commit -m "feat: add workflow runner controls and swarm status to project detail"
```

---

### Task 11: Build, deploy, and verify

**Files:** None (deploy only)

**Step 1: Run tests**

```bash
cargo test --lib 2>&1 | tail -20
```

**Step 2: Build frontend**

```bash
cd frontend && npm run build && cd ..
```

**Step 3: Build release**

```bash
cargo clean -p ironweave && cargo build --release 2>&1 | tail -5
```

**Step 4: Push to GitHub**

```bash
git push
```

**Step 5: Deploy to dev server**

```bash
ssh paddy@10.202.28.202 'cd /home/paddy/ironweave && git pull && ~/.cargo/bin/cargo clean -p ironweave && ~/.cargo/bin/cargo build --release && echo "P0w3rPla72012@@" | sudo -S systemctl restart ironweave'
```

**Step 6: Verify**

```bash
# Health check
curl -sk https://hl-ironweave-dev.techpad.uk/api/health

# Dashboard endpoints
curl -sk https://hl-ironweave-dev.techpad.uk/api/dashboard
curl -sk https://hl-ironweave-dev.techpad.uk/api/dashboard/activity
curl -sk https://hl-ironweave-dev.techpad.uk/api/dashboard/metrics
curl -sk https://hl-ironweave-dev.techpad.uk/api/dashboard/system
```

**Step 7: Deploy to prod**

```bash
rsync -avz --exclude '.git' --exclude 'target' --exclude 'node_modules' --exclude 'data/' --exclude 'certs/' --exclude 'ironweave.toml' --exclude 'ironweave.db' --exclude '.playwright-mcp/' --exclude 'mounts/' --exclude 'test-projects/' /Users/paddyharker/task2/ paddy@10.202.28.205:/home/paddy/ironweave/
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave && ~/.cargo/bin/cargo clean -p ironweave && ~/.cargo/bin/cargo build --release && echo "P0w3rPla72012@@" | sudo -S systemctl restart ironweave'
curl -sk https://hl-ironweave.techpad.uk/api/health
```
