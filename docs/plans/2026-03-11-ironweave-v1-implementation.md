# Ironweave v1 (MVP) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the core Ironweave platform — spawn and manage mixed AI CLI agent teams, execute workflows as DAGs, track issues, isolate work in git worktrees, and merge results back via a conflict-resolving merge queue. All accessible through a Svelte 5 web dashboard over HTTPS.

**Architecture:** Rust (Axum) backend serving a Svelte 5 SPA. SQLite for all state. WebSockets for real-time agent output streaming. CLI agents (Claude Code, OpenCode, Gemini) spawned as subprocesses via a pluggable runtime adapter layer. Git worktrees for agent isolation, FIFO merge queue for integration.

**Tech Stack:** Rust (Axum, tokio, rusqlite, serde, git2, tokio-tungstenite, rustls), Svelte 5, Vite, xterm.js, Cytoscape.js, Tailwind CSS, SQLite.

**Design doc:** `docs/plans/2026-03-11-ironweave-design.md`

---

## Phase 0: Project Scaffolding

### Task 1: Initialise Rust project

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `.gitignore`

**Step 1: Create the Rust project**

```bash
cd /Users/paddyharker/task2
cargo init --name ironweave
```

**Step 2: Add core dependencies to Cargo.toml**

```toml
[package]
name = "ironweave"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = { version = "0.8", features = ["ws", "macros"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.32", features = ["bundled"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tower-http = { version = "0.6", features = ["cors", "fs", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tokio-tungstenite = "0.24"
axum-extra = { version = "0.10", features = ["typed-header"] }
thiserror = "2"
git2 = "0.20"
toml = "0.8"
rust-embed = "8"

[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
```

**Step 3: Create minimal src/main.rs**

```rust
use std::net::SocketAddr;
use axum::{Router, routing::get};
use tracing_subscriber::EnvFilter;

mod lib;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Ironweave listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 4: Verify it compiles and runs**

Run: `cargo build`
Expected: Compiles successfully

Run: `cargo run &` then `curl http://localhost:3000/api/health`
Expected: `ok`

**Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/ .gitignore
git commit -m "feat: initialise Ironweave Rust project with Axum health endpoint"
```

---

### Task 2: Initialise Svelte 5 frontend

**Files:**
- Create: `frontend/` (Svelte project via create-svelte or Vite)
- Create: `frontend/package.json`
- Create: `frontend/src/App.svelte`
- Create: `frontend/vite.config.ts`

**Step 1: Scaffold Svelte 5 project**

```bash
cd /Users/paddyharker/task2
npm create vite@latest frontend -- --template svelte-ts
cd frontend
npm install
npm install -D tailwindcss @tailwindcss/vite
```

**Step 2: Configure Vite proxy to Rust backend**

Modify `frontend/vite.config.ts`:

```typescript
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
      '/ws': {
        target: 'ws://localhost:3000',
        ws: true,
      },
    },
  },
});
```

**Step 3: Create minimal App.svelte with health check**

```svelte
<script lang="ts">
  let status = $state('checking...');

  async function checkHealth() {
    try {
      const res = await fetch('/api/health');
      status = await res.text();
    } catch {
      status = 'unreachable';
    }
  }

  $effect(() => { checkHealth(); });
</script>

<main class="min-h-screen bg-gray-950 text-gray-100 p-8">
  <h1 class="text-3xl font-bold">Ironweave</h1>
  <p class="mt-2 text-gray-400">Backend: {status}</p>
</main>
```

**Step 4: Add Tailwind to app.css**

```css
@import "tailwindcss";
```

**Step 5: Verify frontend runs**

Run: `cd frontend && npm run dev`
Expected: Svelte app loads at localhost:5173, shows "Ironweave" and "Backend: ok" (with Rust backend running)

**Step 6: Commit**

```bash
git add frontend/
git commit -m "feat: initialise Svelte 5 frontend with Tailwind and backend proxy"
```

---

### Task 3: Module structure

**Files:**
- Create: `src/db/mod.rs`
- Create: `src/api/mod.rs`
- Create: `src/models/mod.rs`
- Create: `src/runtime/mod.rs`
- Create: `src/orchestrator/mod.rs`
- Create: `src/worktree/mod.rs`
- Create: `src/config.rs`
- Create: `src/error.rs`
- Modify: `src/main.rs`

**Step 1: Create module skeleton**

Create each module file with a placeholder:

`src/error.rs`:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IronweaveError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("git error: {0}")]
    Git(#[from] git2::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, IronweaveError>;
```

`src/config.rs`:
```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_path: PathBuf,
    pub data_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            database_path: PathBuf::from("data/ironweave.db"),
            data_dir: PathBuf::from("data"),
        }
    }
}
```

`src/db/mod.rs`:
```rust
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &std::path::Path) -> crate::error::Result<DbPool> {
    std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")))?;
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(Arc::new(Mutex::new(conn)))
}
```

`src/models/mod.rs`:
```rust
pub mod project;
pub mod team;
pub mod issue;
pub mod agent;
pub mod workflow;
pub mod loom;
pub mod merge_queue;
```

Create empty sub-modules: `src/models/project.rs`, `src/models/team.rs`, `src/models/issue.rs`, `src/models/agent.rs`, `src/models/workflow.rs`, `src/models/loom.rs`, `src/models/merge_queue.rs`.

`src/api/mod.rs`:
```rust
pub mod projects;
pub mod teams;
pub mod issues;
pub mod agents;
pub mod workflows;
pub mod dashboard;
```

Create empty sub-modules for each.

`src/runtime/mod.rs`:
```rust
pub mod adapter;
pub mod claude;
pub mod opencode;
pub mod gemini;
```

Create empty sub-modules for each.

`src/orchestrator/mod.rs`:
```rust
pub mod engine;
pub mod state_machine;
```

Create empty sub-modules for each.

`src/worktree/mod.rs`:
```rust
pub mod manager;
pub mod merge_queue;
```

Create empty sub-modules for each.

**Step 2: Update src/main.rs to reference modules**

```rust
mod api;
mod config;
mod db;
mod error;
mod models;
mod orchestrator;
mod runtime;
mod worktree;
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles with no errors (warnings for unused modules are fine)

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: establish module structure for all v1 components"
```

---

## Phase 1: Database & Data Models

### Task 4: Database migrations

**Files:**
- Create: `src/db/migrations.rs`
- Modify: `src/db/mod.rs`

**Step 1: Write the migration**

`src/db/migrations.rs`:
```rust
use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            directory TEXT NOT NULL,
            context TEXT NOT NULL CHECK(context IN ('work', 'homelab')),
            obsidian_vault_path TEXT,
            obsidian_project TEXT,
            git_remote TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS teams (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id),
            coordination_mode TEXT NOT NULL DEFAULT 'pipeline'
                CHECK(coordination_mode IN ('pipeline', 'swarm', 'collaborative', 'hierarchical')),
            max_agents INTEGER NOT NULL DEFAULT 5,
            token_budget INTEGER,
            cost_budget_daily REAL,
            is_template INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(name, project_id)
        );

        CREATE TABLE IF NOT EXISTS team_agent_slots (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            runtime TEXT NOT NULL CHECK(runtime IN ('claude', 'opencode', 'gemini')),
            config TEXT NOT NULL DEFAULT '{}',
            slot_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS agent_sessions (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL REFERENCES teams(id),
            slot_id TEXT NOT NULL REFERENCES team_agent_slots(id),
            workflow_instance_id TEXT,
            runtime TEXT NOT NULL,
            pid INTEGER,
            worktree_path TEXT,
            branch TEXT,
            state TEXT NOT NULL DEFAULT 'idle'
                CHECK(state IN ('idle', 'working', 'blocked', 'crashed')),
            claimed_task_id TEXT,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            cost REAL NOT NULL DEFAULT 0.0,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_heartbeat TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS issues (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            type TEXT NOT NULL DEFAULT 'task'
                CHECK(type IN ('bug', 'feature', 'task')),
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'open'
                CHECK(status IN ('open', 'in_progress', 'review', 'closed')),
            priority INTEGER NOT NULL DEFAULT 5,
            claimed_by TEXT REFERENCES agent_sessions(id),
            claimed_at TEXT,
            depends_on TEXT NOT NULL DEFAULT '[]',
            summary TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS workflow_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id),
            team_id TEXT NOT NULL REFERENCES teams(id),
            dag TEXT NOT NULL DEFAULT '{}',
            version INTEGER NOT NULL DEFAULT 1,
            git_sha TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS workflow_instances (
            id TEXT PRIMARY KEY,
            definition_id TEXT NOT NULL REFERENCES workflow_definitions(id),
            state TEXT NOT NULL DEFAULT 'pending'
                CHECK(state IN ('pending', 'running', 'paused', 'failed', 'completed')),
            current_stage TEXT,
            checkpoint TEXT NOT NULL DEFAULT '{}',
            started_at TEXT,
            completed_at TEXT,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost REAL NOT NULL DEFAULT 0.0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS loom_entries (
            id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            agent_id TEXT REFERENCES agent_sessions(id),
            team_id TEXT NOT NULL REFERENCES teams(id),
            project_id TEXT NOT NULL REFERENCES projects(id),
            workflow_instance_id TEXT,
            entry_type TEXT NOT NULL
                CHECK(entry_type IN ('status', 'finding', 'warning', 'delegation', 'escalation', 'completion')),
            content TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS merge_queue_entries (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id),
            branch TEXT NOT NULL,
            worktree_path TEXT NOT NULL,
            target_branch TEXT NOT NULL DEFAULT 'main',
            status TEXT NOT NULL DEFAULT 'queued'
                CHECK(status IN ('queued', 'validating', 'merging', 'conflict', 'merged', 'failed')),
            conflict_tier INTEGER,
            queued_at TEXT NOT NULL DEFAULT (datetime('now')),
            merged_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_issues_project ON issues(project_id);
        CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
        CREATE INDEX IF NOT EXISTS idx_agent_sessions_team ON agent_sessions(team_id);
        CREATE INDEX IF NOT EXISTS idx_agent_sessions_state ON agent_sessions(state);
        CREATE INDEX IF NOT EXISTS idx_loom_entries_project ON loom_entries(project_id);
        CREATE INDEX IF NOT EXISTS idx_loom_entries_team ON loom_entries(team_id);
        CREATE INDEX IF NOT EXISTS idx_merge_queue_status ON merge_queue_entries(status);
        CREATE INDEX IF NOT EXISTS idx_workflow_instances_state ON workflow_instances(state);
    ")?;
    Ok(())
}
```

**Step 2: Wire migrations into db init**

Update `src/db/mod.rs`:
```rust
pub mod migrations;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &std::path::Path) -> crate::error::Result<DbPool> {
    std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")))?;
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrations::run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}
```

**Step 3: Write a test for migrations**

Add to `src/db/migrations.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_migrations_run_cleanly() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"projects".to_string()));
        assert!(tables.contains(&"teams".to_string()));
        assert!(tables.contains(&"issues".to_string()));
        assert!(tables.contains(&"agent_sessions".to_string()));
        assert!(tables.contains(&"workflow_definitions".to_string()));
        assert!(tables.contains(&"workflow_instances".to_string()));
        assert!(tables.contains(&"loom_entries".to_string()));
        assert!(tables.contains(&"merge_queue_entries".to_string()));
    }

    #[test]
    fn test_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap(); // should not error
    }
}
```

**Step 4: Run tests**

Run: `cargo test db::migrations`
Expected: 2 tests pass

**Step 5: Commit**

```bash
git add src/db/
git commit -m "feat: database schema and migrations for all v1 entities"
```

---

### Task 5: Data models (structs + CRUD)

**Files:**
- Modify: `src/models/project.rs`
- Modify: `src/models/team.rs`
- Modify: `src/models/issue.rs`
- Modify: `src/models/agent.rs`
- Modify: `src/models/workflow.rs`
- Modify: `src/models/loom.rs`
- Modify: `src/models/merge_queue.rs`

**Step 1: Implement Project model with CRUD**

`src/models/project.rs`:
```rust
use chrono::NaiveDateTime;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub context: String,
    pub obsidian_vault_path: Option<String>,
    pub obsidian_project: Option<String>,
    pub git_remote: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub directory: String,
    pub context: String,
    pub obsidian_vault_path: Option<String>,
    pub obsidian_project: Option<String>,
    pub git_remote: Option<String>,
}

impl Project {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            directory: row.get("directory")?,
            context: row.get("context")?,
            obsidian_vault_path: row.get("obsidian_vault_path")?,
            obsidian_project: row.get("obsidian_project")?,
            git_remote: row.get("git_remote")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateProject) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context, obsidian_vault_path, obsidian_project, git_remote)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, input.name, input.directory, input.context,
                    input.obsidian_vault_path, input.obsidian_project, input.git_remote],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row("SELECT * FROM projects WHERE id = ?1", params![id], Self::from_row)
            .map_err(|_| IronweaveError::NotFound(format!("project {}", id)))
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], Self::from_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let affected = conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(IronweaveError::NotFound(format!("project {}", id)));
        }
        Ok(())
    }
}
```

**Step 2: Implement remaining models**

Follow the same pattern for Team, TeamAgentSlot, Issue, AgentSession, WorkflowDefinition, WorkflowInstance, LoomEntry, and MergeQueueEntry. Each model gets:
- Struct with Serialize/Deserialize
- CreateX input struct
- `from_row`, `create`, `get_by_id`, `list` (filtered by parent), `delete`
- Issue additionally gets: `claim`, `unclaim`, `get_ready` (unblocked + unclaimed)
- AgentSession gets: `update_state`, `update_heartbeat`
- MergeQueueEntry gets: `get_next` (oldest queued entry)

**Step 3: Write tests for each model**

Test CRUD operations using in-memory SQLite for each model. Ensure foreign key constraints work (e.g. can't create a team without a valid project).

**Step 4: Run tests**

Run: `cargo test models`
Expected: All model tests pass

**Step 5: Commit**

```bash
git add src/models/
git commit -m "feat: data models with CRUD for projects, teams, issues, agents, workflows, loom, merge queue"
```

---

## Phase 2: Runtime Adapters & Process Manager

### Task 6: Runtime adapter trait

**Files:**
- Modify: `src/runtime/adapter.rs`

**Step 1: Define the adapter trait**

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::Child;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub working_directory: PathBuf,
    pub prompt: String,
    pub allowed_tools: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub environment: Option<std::collections::HashMap<String, String>>,
    pub extra_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentOutput {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub stream: OutputStream,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

pub struct SpawnedAgent {
    pub child: Child,
    pub stdout_rx: mpsc::Receiver<AgentOutput>,
}

#[async_trait]
pub trait RuntimeAdapter: Send + Sync {
    /// Human-readable name (e.g. "Claude Code")
    fn name(&self) -> &str;

    /// CLI binary name (e.g. "claude")
    fn binary(&self) -> &str;

    /// Check if the CLI tool is installed and accessible
    async fn check_available(&self) -> bool;

    /// Spawn an agent process with the given config
    async fn spawn(&self, config: &AgentConfig) -> crate::error::Result<SpawnedAgent>;

    /// Build the command-line arguments from config
    fn build_args(&self, config: &AgentConfig) -> Vec<String>;
}
```

Note: Add `async-trait = "0.1"` to Cargo.toml dependencies.

**Step 2: Commit**

```bash
git add src/runtime/adapter.rs Cargo.toml
git commit -m "feat: define RuntimeAdapter trait for pluggable AI CLI support"
```

---

### Task 7: Claude Code adapter

**Files:**
- Modify: `src/runtime/claude.rs`

**Step 1: Implement ClaudeAdapter**

```rust
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::adapter::*;

pub struct ClaudeAdapter;

#[async_trait]
impl RuntimeAdapter for ClaudeAdapter {
    fn name(&self) -> &str { "Claude Code" }
    fn binary(&self) -> &str { "claude" }

    async fn check_available(&self) -> bool {
        Command::new(self.binary())
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn build_args(&self, config: &AgentConfig) -> Vec<String> {
        let mut args = vec![
            "--print".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];

        if let Some(ref tools) = config.allowed_tools {
            args.push("--allowedTools".to_string());
            args.push(tools.join(" "));
        }

        args.push(config.prompt.clone());
        args
    }

    async fn spawn(&self, config: &AgentConfig) -> crate::error::Result<SpawnedAgent> {
        let args = self.build_args(config);

        let mut child = Command::new(self.binary())
            .args(&args)
            .current_dir(&config.working_directory)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(config.environment.clone().unwrap_or_default())
            .spawn()?;

        let (tx, rx) = mpsc::channel(1000);

        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let tx_out = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.send(AgentOutput {
                    timestamp: chrono::Utc::now(),
                    stream: OutputStream::Stdout,
                    data: line,
                }).await;
            }
        });

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx.send(AgentOutput {
                    timestamp: chrono::Utc::now(),
                    stream: OutputStream::Stderr,
                    data: line,
                }).await;
            }
        });

        Ok(SpawnedAgent { child, stdout_rx: rx })
    }
}
```

**Step 2: Implement stub adapters for OpenCode and Gemini**

`src/runtime/opencode.rs` and `src/runtime/gemini.rs` — same structure as Claude but with `binary()` returning `"opencode"` and `"gemini"` respectively, and appropriate `build_args` for each CLI's argument format.

**Step 3: Create adapter registry**

Add to `src/runtime/mod.rs`:
```rust
pub mod adapter;
pub mod claude;
pub mod opencode;
pub mod gemini;

use adapter::RuntimeAdapter;
use std::collections::HashMap;
use std::sync::Arc;

pub struct RuntimeRegistry {
    adapters: HashMap<String, Arc<dyn RuntimeAdapter>>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        let mut adapters: HashMap<String, Arc<dyn RuntimeAdapter>> = HashMap::new();
        adapters.insert("claude".to_string(), Arc::new(claude::ClaudeAdapter));
        adapters.insert("opencode".to_string(), Arc::new(opencode::OpenCodeAdapter));
        adapters.insert("gemini".to_string(), Arc::new(gemini::GeminiAdapter));
        Self { adapters }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn RuntimeAdapter>> {
        self.adapters.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }
}
```

**Step 4: Commit**

```bash
git add src/runtime/
git commit -m "feat: runtime adapters for Claude Code, OpenCode, and Gemini CLI"
```

---

### Task 8: Process manager

**Files:**
- Create: `src/process/mod.rs`
- Create: `src/process/manager.rs`

**Step 1: Implement the process manager**

The process manager tracks running agent processes, handles lifecycle (start, stop, crash detection), and routes output to WebSocket subscribers.

```rust
// src/process/manager.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

use crate::runtime::adapter::{AgentConfig, AgentOutput, SpawnedAgent};
use crate::runtime::RuntimeRegistry;

pub struct ManagedAgent {
    pub session_id: String,
    pub runtime: String,
    pub config: AgentConfig,
    pub output_tx: broadcast::Sender<AgentOutput>,
}

pub struct ProcessManager {
    registry: Arc<RuntimeRegistry>,
    agents: Arc<RwLock<HashMap<String, ManagedAgent>>>,
}

impl ProcessManager {
    pub fn new(registry: Arc<RuntimeRegistry>) -> Self {
        Self {
            registry,
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn spawn_agent(
        &self,
        session_id: &str,
        runtime_name: &str,
        config: AgentConfig,
    ) -> crate::error::Result<broadcast::Receiver<AgentOutput>> {
        let adapter = self.registry.get(runtime_name)
            .ok_or_else(|| crate::error::IronweaveError::NotFound(
                format!("runtime adapter: {}", runtime_name)
            ))?;

        let spawned = adapter.spawn(&config).await?;
        let (output_tx, output_rx) = broadcast::channel(1000);

        let managed = ManagedAgent {
            session_id: session_id.to_string(),
            runtime: runtime_name.to_string(),
            config,
            output_tx: output_tx.clone(),
        };

        self.agents.write().await.insert(session_id.to_string(), managed);

        // Forward spawned agent output to broadcast channel
        let agents = self.agents.clone();
        let sid = session_id.to_string();
        tokio::spawn(async move {
            let mut rx = spawned.stdout_rx;
            while let Some(output) = rx.recv().await {
                let _ = output_tx.send(output);
            }
            // Agent process ended — mark as crashed or completed
            agents.write().await.remove(&sid);
        });

        Ok(output_rx)
    }

    pub async fn stop_agent(&self, session_id: &str) -> crate::error::Result<()> {
        let agents = self.agents.read().await;
        if !agents.contains_key(session_id) {
            return Err(crate::error::IronweaveError::NotFound(
                format!("agent session: {}", session_id)
            ));
        }
        // Kill handled via child process — needs pid tracking
        // For now, remove from map
        drop(agents);
        self.agents.write().await.remove(session_id);
        Ok(())
    }

    pub async fn subscribe(&self, session_id: &str) -> Option<broadcast::Receiver<AgentOutput>> {
        self.agents.read().await
            .get(session_id)
            .map(|a| a.output_tx.subscribe())
    }

    pub async fn list_active(&self) -> Vec<String> {
        self.agents.read().await.keys().cloned().collect()
    }
}
```

**Step 2: Commit**

```bash
git add src/process/
git commit -m "feat: process manager for spawning and monitoring CLI agent processes"
```

---

## Phase 3: REST API

### Task 9: Application state and router

**Files:**
- Modify: `src/main.rs`
- Create: `src/state.rs`

**Step 1: Create shared application state**

`src/state.rs`:
```rust
use std::sync::Arc;
use crate::db::DbPool;
use crate::process::manager::ProcessManager;
use crate::runtime::RuntimeRegistry;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub process_manager: Arc<ProcessManager>,
    pub runtime_registry: Arc<RuntimeRegistry>,
}
```

**Step 2: Wire up main.rs with state and all routers**

Update `src/main.rs` to create state, init DB, and mount API routes for:
- `POST/GET/DELETE /api/projects`
- `POST/GET/DELETE /api/projects/:id/teams`
- `POST/GET/PATCH /api/projects/:id/issues`
- `POST/GET /api/agents` (spawn, list, stop)
- `GET /api/dashboard` (aggregate stats)
- `GET /ws/agent/:id` (WebSocket for agent output)

**Step 3: Implement each API handler module**

Each handler follows the pattern:
- Extract `State<AppState>` and path/body params
- Lock DB, call model methods
- Return JSON response

Example for projects (`src/api/projects.rs`):
```rust
use axum::{extract::{Path, State}, Json, http::StatusCode};
use crate::state::AppState;
use crate::models::project::{Project, CreateProject};

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), StatusCode> {
    let conn = state.db.lock().unwrap();
    Project::create(&conn, &input)
        .map(|p| (StatusCode::CREATED, Json(p)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<Project>> {
    let conn = state.db.lock().unwrap();
    Json(Project::list(&conn).unwrap_or_default())
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Project>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Project::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.db.lock().unwrap();
    Project::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

Follow same pattern for teams, issues, agents, workflows.

**Step 4: Add WebSocket handler for agent output streaming**

```rust
// src/api/agents.rs (WebSocket handler)
pub async fn ws_agent_output(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_agent_ws(socket, state, session_id))
}

async fn handle_agent_ws(mut socket: WebSocket, state: AppState, session_id: String) {
    if let Some(mut rx) = state.process_manager.subscribe(&session_id).await {
        while let Ok(output) = rx.recv().await {
            let msg = serde_json::to_string(&output).unwrap();
            if socket.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    }
}
```

**Step 5: Write integration tests**

Test project CRUD via HTTP using `reqwest` against a running server with an in-memory or temp-file DB.

**Step 6: Run tests**

Run: `cargo test api`
Expected: All API tests pass

**Step 7: Commit**

```bash
git add src/
git commit -m "feat: REST API and WebSocket endpoints for projects, teams, issues, agents"
```

---

## Phase 4: Git Worktrees & Merge Queue

### Task 10: Worktree manager

**Files:**
- Modify: `src/worktree/manager.rs`

**Step 1: Implement worktree creation and cleanup**

```rust
use git2::Repository;
use std::path::{Path, PathBuf};

pub struct WorktreeManager {
    base_dir: PathBuf, // where worktrees are stored
}

impl WorktreeManager {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).ok();
        Self { base_dir }
    }

    /// Create a worktree for an agent's task
    pub fn create_worktree(
        &self,
        repo_path: &Path,
        agent_id: &str,
        task_hash: &str,
        base_branch: &str,
    ) -> crate::error::Result<(PathBuf, String)> {
        let repo = Repository::open(repo_path)?;
        let branch_name = format!("ironweave/{}/{}", agent_id, task_hash);
        let worktree_path = self.base_dir.join(&branch_name.replace('/', "-"));

        // Create branch from base
        let base = repo.find_branch(base_branch, git2::BranchType::Local)?;
        let commit = base.get().peel_to_commit()?;
        repo.branch(&branch_name, &commit, false)?;

        // Create worktree
        repo.worktree(
            &branch_name.replace('/', "-"),
            &worktree_path,
            Some(&mut git2::WorktreeAddOptions::new()
                .reference(Some(&format!("refs/heads/{}", branch_name)))),
        )?;

        Ok((worktree_path, branch_name))
    }

    /// Remove a worktree after merge or abandonment
    pub fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_name: &str,
    ) -> crate::error::Result<()> {
        let repo = Repository::open(repo_path)?;
        let wt = repo.find_worktree(worktree_name)?;
        if wt.validate().is_ok() {
            wt.prune(None)?;
        }
        let worktree_path = self.base_dir.join(worktree_name);
        if worktree_path.exists() {
            std::fs::remove_dir_all(&worktree_path)?;
        }
        Ok(())
    }

    /// List all active worktrees
    pub fn list_worktrees(&self, repo_path: &Path) -> crate::error::Result<Vec<String>> {
        let repo = Repository::open(repo_path)?;
        Ok(repo.worktrees()?.iter().filter_map(|s| s.map(String::from)).collect())
    }
}
```

**Step 2: Write tests**

Test worktree creation and cleanup using a temporary git repo.

**Step 3: Commit**

```bash
git add src/worktree/
git commit -m "feat: git worktree manager for agent isolation"
```

---

### Task 11: Merge queue

**Files:**
- Modify: `src/worktree/merge_queue.rs`

**Step 1: Implement FIFO merge queue processor**

```rust
use git2::{Repository, MergeOptions, CheckoutBuilder};
use std::path::Path;

pub enum MergeResult {
    Success,
    Conflict { files: Vec<String> },
    Error(String),
}

pub struct MergeQueueProcessor;

impl MergeQueueProcessor {
    /// Attempt to merge a branch into the target branch
    pub fn try_merge(
        repo_path: &Path,
        source_branch: &str,
        target_branch: &str,
    ) -> crate::error::Result<MergeResult> {
        let repo = Repository::open(repo_path)?;

        let source = repo.find_branch(source_branch, git2::BranchType::Local)?;
        let source_commit = source.get().peel_to_commit()?;
        let source_annotated = repo.find_annotated_commit(source_commit.id())?;

        let target = repo.find_branch(target_branch, git2::BranchType::Local)?;
        let target_commit = target.get().peel_to_commit()?;

        // Check merge analysis
        let (analysis, _) = repo.merge_analysis(&[&source_annotated])?;

        if analysis.is_up_to_date() {
            return Ok(MergeResult::Success);
        }

        if analysis.is_fast_forward() {
            // Fast-forward merge
            let mut target_ref = repo.find_reference(
                &format!("refs/heads/{}", target_branch)
            )?;
            target_ref.set_target(source_commit.id(), "ironweave: fast-forward merge")?;
            return Ok(MergeResult::Success);
        }

        // Normal merge — check for conflicts
        let mut index = repo.merge_commits(&target_commit, &source_commit, Some(&MergeOptions::new()))?;

        if index.has_conflicts() {
            let conflicts: Vec<String> = index.conflicts()?
                .filter_map(|c| c.ok())
                .filter_map(|c| c.our.map(|e| String::from_utf8_lossy(&e.path).to_string()))
                .collect();
            return Ok(MergeResult::Conflict { files: conflicts });
        }

        // No conflicts — create merge commit
        let oid = index.write_tree_to(&repo)?;
        let tree = repo.find_tree(oid)?;
        let sig = repo.signature()?;
        repo.commit(
            Some(&format!("refs/heads/{}", target_branch)),
            &sig, &sig,
            &format!("ironweave: merge {} into {}", source_branch, target_branch),
            &tree,
            &[&target_commit, &source_commit],
        )?;

        Ok(MergeResult::Success)
    }
}
```

**Step 2: Write tests**

Test fast-forward merge, normal merge, and conflict detection using temporary repos.

**Step 3: Commit**

```bash
git add src/worktree/
git commit -m "feat: FIFO merge queue with conflict detection"
```

---

## Phase 5: Workflow Orchestrator & State Machine

### Task 12: Workflow state machine

**Files:**
- Modify: `src/orchestrator/state_machine.rs`

**Step 1: Implement state machine with checkpointing**

Define state transitions (pending → running → paused/failed/completed), checkpoint serialisation to SQLite, and crash-resume logic. The state machine persists its state after every transition so a server restart resumes from the last checkpoint.

**Step 2: Write tests**

Test valid and invalid transitions, checkpoint save/restore, and crash-resume.

**Step 3: Commit**

```bash
git add src/orchestrator/
git commit -m "feat: workflow state machine with checkpointing and crash-resume"
```

---

### Task 13: Workflow engine (DAG execution)

**Files:**
- Modify: `src/orchestrator/engine.rs`

**Step 1: Implement DAG parser and executor**

- Parse workflow DAG definition (JSON) into stages with edges
- Topological sort to determine execution order
- Execute stages: spawn agents via ProcessManager, wait for completion, evaluate conditions
- Handle parallel stages (independent nodes in the DAG run concurrently)
- Handle manual gates (pause workflow, wait for user approval via API)
- On stage completion, update state machine and checkpoint

**Step 2: Write tests**

Test linear pipeline, parallel fan-out, conditional branching, and manual gate pause/resume.

**Step 3: Commit**

```bash
git add src/orchestrator/
git commit -m "feat: workflow DAG engine with parallel execution and manual gates"
```

---

### Task 14: Team & swarm coordinator

**Files:**
- Create: `src/orchestrator/swarm.rs`

**Step 1: Implement swarm coordination**

- **Task pool:** shared queue of issues/tasks for a team
- **Claiming:** atomic claim via SQLite transaction (SELECT + UPDATE in one tx)
- **Agent loop:** idle agent checks pool → claims next unblocked task → spawns via ProcessManager → on completion, releases claim and checks pool again
- **Dynamic scaling:** monitor pool depth, spawn additional agents up to `max_agents`, drain agents when pool empties
- **Heartbeat:** agents send periodic heartbeats; missed heartbeats → mark crashed, unclaim task

**Step 2: Write tests**

Test claiming, double-claim prevention, heartbeat timeout, and dynamic scaling.

**Step 3: Commit**

```bash
git add src/orchestrator/
git commit -m "feat: swarm coordinator with task pool, atomic claiming, and dynamic scaling"
```

---

## Phase 6: Frontend

### Task 15: Frontend routing and layout

**Files:**
- Create: `frontend/src/lib/api.ts` — API client
- Create: `frontend/src/routes/` — page components
- Modify: `frontend/src/App.svelte` — router and layout shell

**Step 1: Set up routing**

Install `svelte-spa-router` or use Svelte 5's built-in routing. Create pages:
- `/` — Dashboard (F9)
- `/projects` — Project list
- `/projects/:id` — Project detail with teams, issues, workflows
- `/projects/:id/workflows/:wid` — Workflow visualiser
- `/agents` — Active agents view with terminal output

**Step 2: Create layout shell**

Sidebar navigation, header with project switcher, main content area.

**Step 3: Create API client**

`frontend/src/lib/api.ts` — typed fetch wrappers for all REST endpoints, WebSocket connection manager for agent output.

**Step 4: Commit**

```bash
git add frontend/
git commit -m "feat: frontend routing, layout shell, and API client"
```

---

### Task 16: Dashboard page

**Files:**
- Create: `frontend/src/routes/Dashboard.svelte`

**Step 1: Build dashboard**

- Active agents grid: cards showing agent name, runtime, state, current task, output preview
- Team overview: team name, coordination mode, agents count, task pool depth
- Project summary: issues open/closed, active workflows
- System stats: total agents, CPU, memory (via `/api/dashboard` endpoint)

**Step 2: Add auto-refresh**

Poll `/api/dashboard` every 5 seconds or use WebSocket for live updates.

**Step 3: Commit**

```bash
git add frontend/
git commit -m "feat: dashboard page with agent, team, and project overviews"
```

---

### Task 17: Agent terminal view

**Files:**
- Create: `frontend/src/routes/AgentTerminal.svelte`
- Create: `frontend/src/lib/components/Terminal.svelte`

**Step 1: Integrate xterm.js**

```bash
cd frontend && npm install xterm @xterm/addon-fit
```

Create a Terminal component that connects to `/ws/agent/:id` and renders output in an xterm.js instance.

**Step 2: Build agent terminal page**

Grid of terminal panels — one per active agent. Resizable. Click to focus/expand.

**Step 3: Commit**

```bash
git add frontend/
git commit -m "feat: agent terminal view with xterm.js and WebSocket streaming"
```

---

### Task 18: Project, team, and issue management pages

**Files:**
- Create: `frontend/src/routes/Projects.svelte`
- Create: `frontend/src/routes/ProjectDetail.svelte`
- Create: `frontend/src/routes/IssueBoard.svelte`

**Step 1: Project list and create form**

CRUD for projects with form validation.

**Step 2: Project detail page**

Tabs: Teams, Issues, Workflows. Each tab shows the relevant data with create/edit/delete actions.

**Step 3: Issue board**

Kanban-style board with columns: Open, In Progress, Review, Closed. Drag-and-drop to change status. Show dependency links. Show claimed-by agent.

**Step 4: Commit**

```bash
git add frontend/
git commit -m "feat: project management, team config, and issue board pages"
```

---

### Task 19: Workflow visualiser

**Files:**
- Create: `frontend/src/routes/WorkflowView.svelte`
- Create: `frontend/src/lib/components/DagGraph.svelte`

**Step 1: Install Cytoscape.js**

```bash
cd frontend && npm install cytoscape
```

**Step 2: Build DAG visualiser**

Render workflow DAG as an interactive graph. Nodes = stages (colour-coded by state: pending/running/completed/failed). Edges = handoffs. Click a node to see agent output for that stage.

**Step 3: Commit**

```bash
git add frontend/
git commit -m "feat: workflow DAG visualiser with Cytoscape.js"
```

---

## Phase 7: TLS, Auth & Production Readiness

### Task 20: TLS configuration

**Files:**
- Modify: `src/main.rs`
- Modify: `src/config.rs`

**Step 1: Add TLS support with rustls**

Add `axum-server` with `tls-rustls` feature. Load homelab CA cert and key from config paths.

```toml
# Cargo.toml addition
axum-server = { version = "0.7", features = ["tls-rustls"] }
```

**Step 2: Update config**

```rust
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}
```

**Step 3: Commit**

```bash
git add src/ Cargo.toml
git commit -m "feat: HTTPS support via rustls with homelab CA certificates"
```

---

### Task 21: Authentication

**Files:**
- Create: `src/auth/mod.rs`

**Step 1: Implement session-based auth**

Simple session-based auth with username/password stored in config (hashed with argon2). Sessions stored in SQLite. Axum middleware checks session cookie on all `/api/` routes.

**Step 2: Add login page to frontend**

`frontend/src/routes/Login.svelte` — login form, stores session cookie, redirects to dashboard.

**Step 3: Commit**

```bash
git add src/auth/ frontend/
git commit -m "feat: session-based authentication with login page"
```

---

### Task 22: Systemd service and build script

**Files:**
- Create: `deploy/ironweave.service`
- Create: `deploy/build.sh`
- Create: `ironweave.toml.example`

**Step 1: Create systemd unit file**

```ini
[Unit]
Description=Ironweave Agent Orchestrator
After=network.target

[Service]
Type=simple
User=ironweave
WorkingDirectory=/opt/ironweave
ExecStart=/opt/ironweave/ironweave
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

**Step 2: Create build script**

```bash
#!/bin/bash
set -e
echo "Building Ironweave..."
cd frontend && npm ci && npm run build && cd ..
cargo build --release
echo "Binary: target/release/ironweave"
echo "Frontend: frontend/dist/"
```

**Step 3: Create example config**

```toml
host = "0.0.0.0"
port = 443
database_path = "/opt/ironweave/data/ironweave.db"
data_dir = "/opt/ironweave/data"

[tls]
cert_path = "/etc/ironweave/cert.pem"
key_path = "/etc/ironweave/key.pem"

[auth]
# Generate hash with: ironweave hash-password
users = [
    { username = "admin", password_hash = "..." }
]
```

**Step 4: Commit**

```bash
git add deploy/ ironweave.toml.example
git commit -m "feat: systemd service, build script, and example config"
```

---

## Phase 8: Integration Testing & Polish

### Task 23: End-to-end integration tests

**Files:**
- Create: `tests/integration/`

**Step 1: Write E2E tests**

- Create project via API → create team → add agent slots → spawn agent → verify output streams via WebSocket
- Create issues → verify dependency DAG → claim issue → verify atomic claiming
- Create workflow → run it → verify state machine transitions → verify checkpoint/resume
- Create worktree → make changes → queue merge → verify merge succeeds
- Full pipeline: create project + team + workflow + issues → run workflow → agents claim issues → worktrees created → work merged

**Step 2: Run full test suite**

Run: `cargo test`
Expected: All unit and integration tests pass

**Step 3: Commit**

```bash
git add tests/
git commit -m "test: end-to-end integration tests for full agent orchestration flow"
```

---

### Task 24: Frontend build embedding

**Files:**
- Modify: `src/main.rs`

**Step 1: Embed frontend with rust-embed**

Serve the built Svelte SPA from the Rust binary using `rust-embed` so the deployment is a single binary.

```rust
#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

// Fallback handler serves index.html for SPA routing
```

**Step 2: Verify single-binary deployment**

Build with `./deploy/build.sh`, run the binary, verify the SPA loads at `/` and API works at `/api/`.

**Step 3: Final commit**

```bash
git add src/
git commit -m "feat: embed Svelte frontend in Rust binary for single-artifact deployment"
```

---

### Task 25: Playwright E2E tests for Ironweave UI

**Files:**
- Create: `tests/e2e/playwright.config.ts`
- Create: `tests/e2e/package.json`
- Create: `tests/e2e/tests/`

**Step 1: Set up Playwright project**

```bash
mkdir -p tests/e2e && cd tests/e2e
npm init -y
npm install -D @playwright/test
npx playwright install chromium --with-deps
```

**Step 2: Create Playwright config**

```typescript
// tests/e2e/playwright.config.ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  use: {
    baseURL: 'https://localhost:443',
    ignoreHTTPSErrors: true, // homelab CA
    screenshot: 'on-first-failure',
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium' } },
  ],
});
```

**Step 3: Write E2E tests**

- Login flow: navigate to `/`, redirected to login, enter credentials, land on dashboard
- Project CRUD: create project → verify it appears → delete → verify removed
- Agent lifecycle: create project + team → spawn agent → verify terminal output streams in browser → stop agent
- Issue board: create issues → drag between columns → verify status updates
- Workflow visualiser: create workflow → run it → verify DAG nodes change colour as stages progress

**Step 4: Run tests**

Run: `cd tests/e2e && npx playwright test`
Expected: All tests pass headless

**Step 5: Commit**

```bash
git add tests/e2e/
git commit -m "test: Playwright E2E tests for Ironweave web UI"
```

---

### Task 26: Playwright as agent tooling

**Files:**
- Modify: `src/runtime/adapter.rs`
- Modify: `deploy/build.sh`

**Step 1: Ensure Playwright is available to agents**

Agents need access to Playwright for testing projects they work on. This requires:

- Playwright and browser binaries installed system-wide on the VM (not per-project)
- Environment variables set so agents can find Playwright browsers

Add to `deploy/build.sh`:
```bash
# Install Playwright browsers system-wide
echo "Installing Playwright browsers..."
npx playwright install chromium firefox --with-deps
```

**Step 2: Add Playwright environment to agent config**

Update `AgentConfig` to optionally inject Playwright-related environment variables:

```rust
// In AgentConfig or runtime adapter
// Agents that need browser testing get these env vars:
// PLAYWRIGHT_BROWSERS_PATH=/opt/ironweave/browsers
// PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
```

**Step 3: Add Playwright MCP server as optional agent tool**

Agents using Claude Code can access Playwright via the MCP server (`@anthropic-ai/mcp-playwright`). Add to the runtime adapter config schema so teams can enable Playwright MCP per agent slot.

Update `team_agent_slots` config to support:
```json
{
  "mcp_servers": {
    "playwright": {
      "enabled": true
    }
  }
}
```

**Step 4: Commit**

```bash
git add src/ deploy/
git commit -m "feat: Playwright available as agent tooling for project testing"
```

---

## Summary

| Phase | Tasks | What it delivers |
|-------|-------|-----------------|
| **0: Scaffolding** | 1-3 | Rust + Svelte project with module structure |
| **1: Database** | 4-5 | Schema, migrations, model CRUD |
| **2: Runtime** | 6-8 | Adapter trait, Claude/OpenCode/Gemini adapters, process manager |
| **3: API** | 9 | REST + WebSocket endpoints, app state |
| **4: Git** | 10-11 | Worktree isolation, merge queue with conflict detection |
| **5: Orchestrator** | 12-14 | State machine, DAG engine, swarm coordinator |
| **6: Frontend** | 15-19 | Dashboard, terminal, project/issue management, workflow visualiser |
| **7: Production** | 20-22 | TLS, auth, systemd, build script |
| **8: Testing** | 23-26 | E2E tests, Playwright UI tests, Playwright agent tooling, single-binary deployment |

**Total: 26 tasks across 9 phases.**
