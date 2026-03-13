# Orchestrator Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire the orchestrator loop that connects workflow instances, DAG execution, issue tracking, and agent spawning into a single background task that executes workflows end-to-end.

**Architecture:** A background tokio task (`OrchestratorRunner`) listens for events (new workflow instances) and sweeps on a 30s timer. On each tick it polls issue statuses for running stages, advances the DAG, spawns agents for newly ready stages, and escalates idle agents via PTY nudges. Each stage maps to an issue (bead) that the agent updates.

**Tech Stack:** Rust (tokio channels, timers, select!), existing crate modules (orchestrator/engine, orchestrator/state_machine, process/manager, models/issue, models/workflow)

**Design doc:** `docs/plans/2026-03-12-orchestrator-loop-design.md`

---

### Task 1: Add `model` field to Stage and SpawnRequest

The DAG `Stage` struct needs an optional `model` field for future model selection. `SpawnRequest` and `AgentConfig` also need it.

**Files:**
- Modify: `src/orchestrator/engine.rs:8-16`
- Modify: `src/runtime/adapter.rs` (AgentConfig struct)
- Modify: `src/api/agents.rs:15-21` (SpawnRequest struct)

**Step 1: Add `model` to Stage**

In `src/orchestrator/engine.rs`, add to the `Stage` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub id: String,
    pub name: String,
    pub runtime: String,
    pub prompt: String,
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub is_manual_gate: bool,
    #[serde(default)]
    pub model: Option<String>,
}
```

**Step 2: Add `model` to AgentConfig**

In `src/runtime/adapter.rs`, add to the `AgentConfig` struct:

```rust
pub model: Option<String>,
```

**Step 3: Add `model` to SpawnRequest**

In `src/api/agents.rs`, add to the `SpawnRequest` struct:

```rust
pub model: Option<String>,
```

And pass it through to `AgentConfig` in the `spawn` handler.

**Step 4: Run tests**

Run: `cargo test` on the server (or locally if cargo available)
Expected: All existing tests pass — `model` is `Option` with `serde(default)`.

**Step 5: Commit**

```bash
git add src/orchestrator/engine.rs src/runtime/adapter.rs src/api/agents.rs
git commit -m "feat: add optional model field to Stage, AgentConfig, and SpawnRequest"
```

---

### Task 2: Add `workflow_instance_id` and `stage_id` to issues table

The orchestrator needs to link issues to workflow stages.

**Files:**
- Modify: `src/db/migrations.rs` (add ALTER TABLE or new migration)
- Modify: `src/models/issue.rs` (add fields to struct and from_row)

**Step 1: Add migration**

In `src/db/migrations.rs`, after existing migrations, add:

```rust
// Add workflow linkage columns to issues
conn.execute_batch("
    ALTER TABLE issues ADD COLUMN workflow_instance_id TEXT REFERENCES workflow_instances(id);
    ALTER TABLE issues ADD COLUMN stage_id TEXT;
").ok(); // .ok() because ALTER TABLE IF NOT EXISTS isn't supported — idempotent via ignoring errors
```

**Step 2: Update Issue struct**

In `src/models/issue.rs`, add to the `Issue` struct:

```rust
pub workflow_instance_id: Option<String>,
pub stage_id: Option<String>,
```

Update `from_row` to include these fields. Update `CreateIssue` to include them as optional fields.

**Step 3: Run tests**

Run: `cargo test -- --test-threads=1`
Expected: All existing tests pass.

**Step 4: Commit**

```bash
git add src/db/migrations.rs src/models/issue.rs
git commit -m "feat: add workflow_instance_id and stage_id columns to issues"
```

---

### Task 3: Fix `#[serde(skip)]` on DagExecutionState.dag

The `dag` field is skipped during serialization, which breaks checkpoint restore. Store the DAG JSON alongside the execution state in the checkpoint.

**Files:**
- Modify: `src/orchestrator/engine.rs:36-42`

**Step 1: Add `set_dag` method and `dag_json` field**

Replace `#[serde(skip)]` approach — store the raw DAG JSON in the execution state so it survives serialization:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagExecutionState {
    pub stage_statuses: HashMap<String, StageStatus>,
    pub execution_order: Vec<Vec<String>>,
    pub dag_json: String,
    #[serde(skip)]
    dag: Option<DagDefinition>,
}
```

Update `DagExecutionState::new()` to store `dag_json`. Add a `restore_dag()` method that re-parses `dag_json` into the `dag` field. Call `restore_dag()` after deserialization.

**Step 2: Update existing tests**

Ensure all existing tests in `engine.rs` still pass.

**Step 3: Add restore test**

```rust
#[test]
fn dag_survives_serialization() {
    let dag = make_dag(vec![
        make_stage("A", vec![], false),
        make_stage("B", vec!["A"], false),
    ]);
    let state = DagExecutionState::new(&dag).unwrap();
    let json = serde_json::to_string(&state).unwrap();
    let mut restored: DagExecutionState = serde_json::from_str(&json).unwrap();
    restored.restore_dag().unwrap();
    assert_eq!(restored.ready_stages(), vec!["A"]);
}
```

**Step 4: Run tests**

Run: `cargo test orchestrator::engine`
Expected: All pass including new test.

**Step 5: Commit**

```bash
git add src/orchestrator/engine.rs
git commit -m "fix: preserve DAG definition through serialization for checkpoint restore"
```

---

### Task 4: Add `write_to_agent` to ProcessManager

The orchestrator needs to write nudge messages to agent PTYs.

**Files:**
- Modify: `src/process/manager.rs`

**Step 1: Add method**

Add to `ProcessManager`:

```rust
pub async fn write_to_agent(&self, session_id: &str, data: &[u8]) -> crate::error::Result<()> {
    let agent = self.agents.read().await.get(session_id).cloned().ok_or_else(|| {
        crate::error::IronweaveError::NotFound(format!("agent session: {}", session_id))
    })?;
    let mut locked = agent.lock().await;
    locked.master.write_all(data).map_err(|e| {
        crate::error::IronweaveError::Internal(format!("failed to write to agent PTY: {}", e))
    })?;
    Ok(())
}
```

Note: requires `use std::io::Write as IoWrite;` at the top of the file (or in the method).

**Step 2: Commit**

```bash
git add src/process/manager.rs
git commit -m "feat: add write_to_agent for PTY nudge messages"
```

---

### Task 5: Create OrchestratorHandle and event types

Define the channel types and handle struct that the API uses to notify the orchestrator.

**Files:**
- Create: `src/orchestrator/runner.rs`
- Modify: `src/orchestrator/mod.rs`

**Step 1: Create runner.rs with types**

```rust
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum OrchestratorEvent {
    InstanceCreated { instance_id: String, definition_id: String },
}

#[derive(Clone)]
pub struct OrchestratorHandle {
    tx: mpsc::Sender<OrchestratorEvent>,
}

impl OrchestratorHandle {
    pub fn new(tx: mpsc::Sender<OrchestratorEvent>) -> Self {
        Self { tx }
    }

    pub async fn notify_instance_created(&self, instance_id: String, definition_id: String) {
        let _ = self.tx.send(OrchestratorEvent::InstanceCreated {
            instance_id,
            definition_id,
        }).await;
    }
}
```

**Step 2: Add to mod.rs**

```rust
pub mod engine;
pub mod runner;
pub mod state_machine;
pub mod swarm;
```

**Step 3: Commit**

```bash
git add src/orchestrator/runner.rs src/orchestrator/mod.rs
git commit -m "feat: add OrchestratorHandle and event types"
```

---

### Task 6: Create WorkflowRunState and StageAgent structs

Add the in-memory state tracking structs to runner.rs.

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Add structs**

```rust
use std::collections::HashMap;
use tokio::time::Instant;

use super::engine::{DagDefinition, DagExecutionState, StageStatus};
use super::state_machine::StateMachine;

pub struct StageAgent {
    pub agent_session_id: String,
    pub issue_id: String,
    pub spawned_at: Instant,
    pub last_activity: Instant,
    pub nudge_count: u32,
}

pub struct WorkflowRunState {
    pub instance_id: String,
    pub definition_id: String,
    pub project_id: String,
    pub dag: DagDefinition,
    pub execution: DagExecutionState,
    pub stage_agents: HashMap<String, StageAgent>,
    pub state_machine: StateMachine,
}
```

**Step 2: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: add WorkflowRunState and StageAgent structs"
```

---

### Task 7: Implement OrchestratorRunner core loop

The main orchestrator background task with event handling and sweep timer.

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Implement the runner**

```rust
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

use crate::db::DbPool;
use crate::process::manager::ProcessManager;
use crate::runtime::RuntimeRegistry;

const SWEEP_INTERVAL_SECS: u64 = 30;
const NUDGE_THRESHOLD_SECS: u64 = 300;  // 5 minutes
const NUDGE_WARNING_SECS: u64 = 420;    // 7 minutes
const KILL_THRESHOLD_SECS: u64 = 540;   // 9 minutes

pub struct OrchestratorRunner {
    rx: mpsc::Receiver<OrchestratorEvent>,
    db: DbPool,
    process_manager: Arc<ProcessManager>,
    runtime_registry: Arc<RuntimeRegistry>,
    active_workflows: HashMap<String, WorkflowRunState>,
}

impl OrchestratorRunner {
    pub fn new(
        rx: mpsc::Receiver<OrchestratorEvent>,
        db: DbPool,
        process_manager: Arc<ProcessManager>,
        runtime_registry: Arc<RuntimeRegistry>,
    ) -> Self {
        Self {
            rx,
            db,
            process_manager,
            runtime_registry,
            active_workflows: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        tracing::info!("Orchestrator started");
        let mut tick = interval(Duration::from_secs(SWEEP_INTERVAL_SECS));

        loop {
            tokio::select! {
                Some(event) = self.rx.recv() => {
                    self.handle_event(event).await;
                }
                _ = tick.tick() => {
                    self.sweep().await;
                }
            }
        }
    }

    async fn handle_event(&mut self, event: OrchestratorEvent) {
        match event {
            OrchestratorEvent::InstanceCreated { instance_id, definition_id } => {
                if self.active_workflows.contains_key(&instance_id) {
                    return; // idempotent
                }
                if let Err(e) = self.start_workflow(instance_id, definition_id).await {
                    tracing::error!("Failed to start workflow: {}", e);
                }
            }
        }
    }
}
```

**Step 2: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: implement OrchestratorRunner core loop with select!"
```

---

### Task 8: Implement `start_workflow`

Load the definition, parse the DAG, transition to Running, spawn agents for ready stages.

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Implement start_workflow**

```rust
impl OrchestratorRunner {
    async fn start_workflow(
        &mut self,
        instance_id: String,
        definition_id: String,
    ) -> crate::error::Result<()> {
        // Load definition from DB
        let (dag_json, project_id, team_id) = {
            let conn = self.db.lock().unwrap();
            let def = crate::models::workflow::WorkflowDefinition::get_by_id(&conn, &definition_id)?;
            (def.dag, def.project_id, def.team_id)
        };

        // Parse DAG
        let dag = DagDefinition::from_json(&dag_json)?;
        let execution = DagExecutionState::new(&dag)?;

        // Create state machine and transition to Running
        let state_machine = StateMachine::new(instance_id.clone(), self.db.clone());
        let mut run_state = WorkflowRunState {
            instance_id: instance_id.clone(),
            definition_id,
            project_id,
            dag: dag.clone(),
            execution,
            stage_agents: HashMap::new(),
            state_machine,
        };
        run_state.state_machine.transition(
            crate::orchestrator::state_machine::WorkflowState::Running
        )?;

        // Spawn agents for ready stages
        let ready = run_state.execution.ready_stages();
        for stage_id in ready {
            self.spawn_stage_agent(&mut run_state, &stage_id).await?;
        }

        // Checkpoint
        let checkpoint = serde_json::to_value(&run_state.execution)?;
        run_state.state_machine.checkpoint(checkpoint)?;

        self.active_workflows.insert(instance_id, run_state);
        Ok(())
    }
}
```

**Step 2: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: implement start_workflow — load DAG, transition, spawn initial stages"
```

---

### Task 9: Implement `spawn_stage_agent`

Create an issue (bead) for the stage, spawn a PTY agent, link them.

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Implement spawn_stage_agent**

```rust
use portable_pty::PtySize;
use crate::runtime::adapter::AgentConfig;
use crate::models::issue::{Issue, CreateIssue};

impl OrchestratorRunner {
    async fn spawn_stage_agent(
        &self,
        run_state: &mut WorkflowRunState,
        stage_id: &str,
    ) -> crate::error::Result<()> {
        let stage = run_state.dag.stages.iter()
            .find(|s| s.id == stage_id)
            .ok_or_else(|| crate::error::IronweaveError::NotFound(
                format!("stage {}", stage_id)
            ))?
            .clone();

        // Create issue (bead) for this stage
        let issue = {
            let conn = self.db.lock().unwrap();
            Issue::create(&conn, &CreateIssue {
                project_id: run_state.project_id.clone(),
                issue_type: Some("task".to_string()),
                title: stage.name.clone(),
                description: Some(stage.prompt.clone()),
                priority: None,
                depends_on: None,
                workflow_instance_id: Some(run_state.instance_id.clone()),
                stage_id: Some(stage_id.to_string()),
            })?
        };

        // Build agent config
        let session_id = uuid::Uuid::new_v4().to_string();
        let prompt = format!(
            "{}\n\nYou are working on issue {}. When you are done, update the issue status to 'closed' via the API: POST /api/projects/{}/issues/{}/claim with your session then update status.",
            stage.prompt, issue.id, run_state.project_id, issue.id
        );

        let config = AgentConfig {
            working_directory: std::path::PathBuf::from("/home/paddy"),
            prompt,
            environment: None,
            allowed_tools: None,
            skills: None,
            extra_args: None,
            playwright_env: None,
            model: stage.model.clone(),
        };

        let size = PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 };

        // Spawn the agent
        self.process_manager
            .spawn_agent(&session_id, &stage.runtime, config, size)
            .await?;

        // Claim the issue for this agent
        {
            let conn = self.db.lock().unwrap();
            Issue::claim(&conn, &issue.id, &session_id)?;
        }

        // Track in-memory
        run_state.execution.update_stage(stage_id, StageStatus::Running);
        run_state.stage_agents.insert(stage_id.to_string(), StageAgent {
            agent_session_id: session_id,
            issue_id: issue.id,
            spawned_at: Instant::now(),
            last_activity: Instant::now(),
            nudge_count: 0,
        });

        tracing::info!(
            workflow = %run_state.instance_id,
            stage = %stage_id,
            "Spawned agent for stage"
        );
        Ok(())
    }
}
```

**Step 2: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: implement spawn_stage_agent — creates bead, spawns PTY, claims issue"
```

---

### Task 10: Implement `sweep` — poll issues, advance DAG, handle nudges

The periodic housekeeping loop.

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Implement sweep**

```rust
impl OrchestratorRunner {
    async fn sweep(&mut self) {
        let instance_ids: Vec<String> = self.active_workflows.keys().cloned().collect();

        for instance_id in instance_ids {
            if let Err(e) = self.sweep_workflow(&instance_id).await {
                tracing::error!(workflow = %instance_id, "Sweep error: {}", e);
            }
        }

        // Remove completed/failed workflows from active map
        self.active_workflows.retain(|_, ws| {
            !ws.execution.is_complete()
        });
    }

    async fn sweep_workflow(&mut self, instance_id: &str) -> crate::error::Result<()> {
        let run_state = match self.active_workflows.get_mut(instance_id) {
            Some(rs) => rs,
            None => return Ok(()),
        };

        // 1. Check issue statuses for running stages
        let stage_ids: Vec<String> = run_state.stage_agents.keys().cloned().collect();
        let mut completed_stages = Vec::new();
        let mut failed_stages = Vec::new();

        for stage_id in &stage_ids {
            let sa = &run_state.stage_agents[stage_id];
            let issue = {
                let conn = self.db.lock().unwrap();
                Issue::get_by_id(&conn, &sa.issue_id)?
            };

            if issue.status == "closed" {
                completed_stages.push(stage_id.clone());
                continue;
            }

            // Check if agent PTY is still alive
            let agent_exists = self.process_manager.get_agent(&sa.agent_session_id).await.is_some();
            if !agent_exists && issue.status != "closed" {
                failed_stages.push(stage_id.clone());
                continue;
            }

            // Check for idle agent — nudge/kill escalation
            let idle_secs = sa.last_activity.elapsed().as_secs();

            // Update last_activity from issue.updated_at
            if issue.updated_at != "" {
                // If issue was updated recently, reset activity timer
                // (simplified: just check if updated_at changed)
                let sa_mut = run_state.stage_agents.get_mut(stage_id).unwrap();
                // Use updated_at as proxy for activity
                sa_mut.last_activity = Instant::now(); // Reset on each sweep if issue changed
                // TODO: track previous updated_at to detect actual changes
            }

            if idle_secs >= KILL_THRESHOLD_SECS {
                tracing::warn!(workflow = %instance_id, stage = %stage_id, "Killing idle agent");
                let _ = self.process_manager.stop_agent(&sa.agent_session_id).await;
                failed_stages.push(stage_id.clone());
            } else if idle_secs >= NUDGE_WARNING_SECS && sa.nudge_count >= 1 {
                let msg = "\n\nNo status update received. Please respond or your session will be terminated.\n";
                let _ = self.process_manager.write_to_agent(&sa.agent_session_id, msg.as_bytes()).await;
                run_state.stage_agents.get_mut(stage_id).unwrap().nudge_count = 2;
            } else if idle_secs >= NUDGE_THRESHOLD_SECS && sa.nudge_count == 0 {
                let msg = "\n\nPlease update your issue status with your current progress.\n";
                let _ = self.process_manager.write_to_agent(&sa.agent_session_id, msg.as_bytes()).await;
                run_state.stage_agents.get_mut(stage_id).unwrap().nudge_count = 1;
            }
        }

        // 2. Mark completed stages
        for stage_id in &completed_stages {
            run_state.execution.update_stage(stage_id, StageStatus::Completed);
            run_state.stage_agents.remove(stage_id);
            tracing::info!(workflow = %instance_id, stage = %stage_id, "Stage completed");
        }

        // 3. Mark failed stages and unclaim issues
        for stage_id in &failed_stages {
            run_state.execution.update_stage(stage_id, StageStatus::Failed("agent died or timed out".into()));
            if let Some(sa) = run_state.stage_agents.remove(stage_id) {
                let conn = self.db.lock().unwrap();
                let _ = Issue::unclaim(&conn, &sa.issue_id);
            }
            tracing::warn!(workflow = %instance_id, stage = %stage_id, "Stage failed");
        }

        // 4. Spawn agents for newly ready stages
        if !completed_stages.is_empty() {
            let ready = run_state.execution.ready_stages();
            for stage_id in ready {
                if let Err(e) = self.spawn_stage_agent(run_state, &stage_id).await {
                    tracing::error!(workflow = %instance_id, stage = %stage_id, "Failed to spawn: {}", e);
                    run_state.execution.update_stage(&stage_id, StageStatus::Failed(e.to_string()));
                }
            }
        }

        // 5. Check workflow completion
        if run_state.execution.is_complete() {
            let all_succeeded = run_state.execution.stage_statuses.values().all(|s| {
                matches!(s, StageStatus::Completed)
            });
            let new_state = if all_succeeded {
                crate::orchestrator::state_machine::WorkflowState::Completed
            } else {
                crate::orchestrator::state_machine::WorkflowState::Failed
            };
            if let Err(e) = run_state.state_machine.transition(new_state) {
                tracing::error!(workflow = %instance_id, "Failed to transition: {}", e);
            }
            tracing::info!(workflow = %instance_id, state = %new_state, "Workflow finished");
        }

        // 6. Checkpoint
        let checkpoint = serde_json::to_value(&run_state.execution)?;
        run_state.state_machine.checkpoint(checkpoint)?;

        Ok(())
    }
}
```

**Step 2: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: implement sweep — poll issues, advance DAG, nudge/kill idle agents"
```

---

### Task 11: Wire orchestrator into AppState and main.rs

Start the orchestrator on server boot and make the handle available to API handlers.

**Files:**
- Modify: `src/state.rs:1-16`
- Modify: `src/main.rs:67-89`

**Step 1: Add OrchestratorHandle to AppState**

In `src/state.rs`:

```rust
use crate::orchestrator::runner::OrchestratorHandle;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub process_manager: Arc<ProcessManager>,
    pub runtime_registry: Arc<RuntimeRegistry>,
    pub auth_config: Option<AuthConfig>,
    pub mount_manager: Option<Arc<crate::mount::manager::MountManager>>,
    pub filesystem_config: Option<crate::config::FilesystemConfig>,
    pub sync_manager: Option<Arc<crate::sync::manager::SyncManager>>,
    pub orchestrator: OrchestratorHandle,
}
```

**Step 2: Create and spawn orchestrator in main.rs**

After `let process_manager = ...` in main.rs, add:

```rust
// Orchestrator
let (orch_tx, orch_rx) = tokio::sync::mpsc::channel(64);
let orchestrator_handle = orchestrator::runner::OrchestratorHandle::new(orch_tx);

let orch_runner = orchestrator::runner::OrchestratorRunner::new(
    orch_rx,
    db.clone(),
    process_manager.clone(),
    registry.clone(),
);
tokio::spawn(orch_runner.run());
```

Add `orchestrator: orchestrator_handle` to the `AppState` construction.

**Step 3: Commit**

```bash
git add src/state.rs src/main.rs
git commit -m "feat: wire orchestrator into AppState and spawn on startup"
```

---

### Task 12: Notify orchestrator on workflow instance creation

Send an event when a new workflow instance is created via the API.

**Files:**
- Modify: `src/api/workflows.rs:38-48`

**Step 1: Send event after instance creation**

```rust
pub async fn create_instance(
    State(state): State<AppState>,
    Path(wid): Path<String>,
    Json(mut input): Json<CreateWorkflowInstance>,
) -> Result<(StatusCode, Json<WorkflowInstance>), StatusCode> {
    input.definition_id = wid.clone();
    let instance = {
        let conn = state.db.lock().unwrap();
        WorkflowInstance::create(&conn, &input)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    // Notify orchestrator
    state.orchestrator.notify_instance_created(
        instance.id.clone(),
        wid,
    ).await;

    Ok((StatusCode::CREATED, Json(instance)))
}
```

**Step 2: Commit**

```bash
git add src/api/workflows.rs
git commit -m "feat: notify orchestrator when workflow instance is created"
```

---

### Task 13: Add startup recovery for running instances

On server restart, scan for workflow instances in `running` state and re-register them.

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Add restore method**

```rust
impl OrchestratorRunner {
    pub async fn restore_running_instances(&mut self) {
        let instances: Vec<(String, String)> = {
            let conn = self.db.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT wi.id, wi.definition_id FROM workflow_instances wi WHERE wi.state = 'running'"
            ).unwrap();
            stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }).unwrap().filter_map(|r| r.ok()).collect()
        };

        for (instance_id, definition_id) in instances {
            tracing::info!(workflow = %instance_id, "Restoring running workflow");
            // Mark all stages with missing agents as failed
            if let Err(e) = self.start_workflow(instance_id.clone(), definition_id).await {
                tracing::error!(workflow = %instance_id, "Failed to restore: {}", e);
                // Mark as failed in DB
                let conn = self.db.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE workflow_instances SET state = 'failed' WHERE id = ?1",
                    rusqlite::params![instance_id],
                );
            }
        }
    }
}
```

**Step 2: Call restore before entering the loop**

In the `run` method, call `self.restore_running_instances().await;` before entering the `loop`.

**Step 3: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: restore running workflow instances on server startup"
```

---

### Task 14: Build, deploy, and verify E2E

Build the full stack on hl-ironweave and test with a real workflow.

**Files:**
- No code changes — build and deploy

**Step 1: Rsync source to server**

```bash
rsync -az --delete --exclude='target' --exclude='node_modules' --exclude='.git' --exclude='frontend/dist' ./ paddy@10.202.28.205:/home/paddy/ironweave/src/
```

**Step 2: Build frontend**

```bash
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave/src/frontend && npm install && npx vite build'
```

**Step 3: Build backend (clean fingerprints)**

```bash
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave/src && source $HOME/.cargo/env && rm -f target/release/deps/ironweave-* && rm -rf target/release/.fingerprint/ironweave-* && cargo build --release'
```

**Step 4: Deploy and restart**

```bash
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave && cp src/target/release/ironweave target/release/ironweave.new && mv -f target/release/ironweave.new target/release/ironweave && sudo systemctl restart ironweave'
```

**Step 5: Verify via API**

Create a project, create a workflow definition with a 2-stage DAG, create an instance, and verify:
- Orchestrator picks it up (check logs)
- Issues are created for stages
- Agents are spawned
- DAG advances when stages complete

**Step 6: Commit any fixes**

```bash
git commit -m "fix: deployment adjustments for orchestrator loop"
```
