use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use portable_pty::PtySize;
use rusqlite::params;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, Instant};

use crate::db::DbPool;
use crate::models::agent::{AgentSession, CreateAgentSession};
use crate::models::issue::{CreateIssue, Issue};
use crate::models::project::Project;
use crate::models::team::{Team, TeamAgentSlot};
use crate::models::workflow::WorkflowDefinition;
use crate::process::manager::ProcessManager;
use crate::runtime::adapter::AgentConfig;
use crate::runtime::RuntimeRegistry;
use crate::worktree::manager::WorktreeManager;

use super::engine::{DagDefinition, DagExecutionState, StageStatus};
use super::state_machine::{StateMachine, WorkflowState};

const SWEEP_INTERVAL_SECS: u64 = 30;
const NUDGE_THRESHOLD_SECS: u64 = 300; // 5 minutes
const NUDGE_WARNING_SECS: u64 = 420; // 7 minutes
const KILL_THRESHOLD_SECS: u64 = 540; // 9 minutes

// ── Event & Handle ─────────────────────────────────────────────────────

#[derive(Debug)]
pub enum OrchestratorEvent {
    InstanceCreated {
        instance_id: String,
        definition_id: String,
    },
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
        let _ = self
            .tx
            .send(OrchestratorEvent::InstanceCreated {
                instance_id,
                definition_id,
            })
            .await;
    }
}

// ── In-memory tracking structs ─────────────────────────────────────────

pub struct StageAgent {
    pub agent_session_id: String,
    pub issue_id: String,
    pub spawned_at: Instant,
    pub last_activity: Instant,
    pub nudge_count: u32,
    pub last_issue_updated_at: String,
    pub retry_count: u8,
}

pub struct TeamDispatchedAgent {
    pub agent_session_id: String,
    pub team_id: String,
    pub slot_id: String,
    pub issue_id: String,
    pub role: String,
    pub spawned_at: Instant,
    pub last_activity: Instant,
    pub nudge_count: u32,
    pub last_issue_updated_at: String,
}

pub struct WorkflowRunState {
    pub instance_id: String,
    pub definition_id: String,
    pub project_id: String,
    pub dag: DagDefinition,
    pub execution: DagExecutionState,
    pub stage_agents: HashMap<String, StageAgent>,
    pub state_machine: StateMachine,
    /// Tracks retry counts per stage_id (persists across agent re-spawns)
    pub stage_retry_counts: HashMap<String, u8>,
    /// Stages pending retry — maps stage_id to issue_id for reuse
    pub stages_pending_retry: HashMap<String, String>,
}

// ── OrchestratorRunner ─────────────────────────────────────────────────

pub struct OrchestratorRunner {
    rx: mpsc::Receiver<OrchestratorEvent>,
    db: DbPool,
    process_manager: Arc<ProcessManager>,
    #[allow(dead_code)]
    runtime_registry: Arc<RuntimeRegistry>,
    active_workflows: HashMap<String, WorkflowRunState>,
    team_agents: HashMap<String, TeamDispatchedAgent>,
    /// Tracks active intake agents: project_id -> (session_id, issue_id)
    intake_agents: HashMap<String, (String, String)>,
    worktree_manager: WorktreeManager,
    build_server: Option<crate::config::BuildServerConfig>,
}

impl OrchestratorRunner {
    pub fn new(
        rx: mpsc::Receiver<OrchestratorEvent>,
        db: DbPool,
        process_manager: Arc<ProcessManager>,
        runtime_registry: Arc<RuntimeRegistry>,
        worktree_base: std::path::PathBuf,
    ) -> Self {
        Self {
            rx,
            db,
            process_manager,
            runtime_registry,
            active_workflows: HashMap::new(),
            team_agents: HashMap::new(),
            intake_agents: HashMap::new(),
            worktree_manager: WorktreeManager::new(worktree_base),
            build_server: None,
        }
    }

    pub fn with_build_server(mut self, config: Option<crate::config::BuildServerConfig>) -> Self {
        self.build_server = config;
        self
    }

    // ── Worktree cleanup helper ──────────────────────────────────────

    fn cleanup_agent_worktree(&self, agent_session_id: &str) {
        let (worktree_path, branch, project_dir) = {
            let conn = self.db.lock().unwrap();
            match AgentSession::get_by_id(&conn, agent_session_id) {
                Ok(session) => {
                    let dir = Team::get_by_id(&conn, &session.team_id)
                        .ok()
                        .and_then(|t| Project::get_by_id(&conn, &t.project_id).ok())
                        .map(|p| p.directory);
                    (session.worktree_path, session.branch, dir)
                }
                Err(_) => return,
            }
        };
        // Remove the worktree directory
        if let Some(wt_path) = &worktree_path {
            let path = std::path::Path::new(wt_path);
            if path.exists() {
                if let Err(e) = std::fs::remove_dir_all(path) {
                    tracing::warn!(session = %agent_session_id, path = %wt_path, "Failed to remove worktree dir: {}", e);
                } else {
                    tracing::info!(session = %agent_session_id, path = %wt_path, "Cleaned up worktree directory");
                }
            }
        }
        // Prune the git worktree reference
        if let (Some(branch), Some(dir)) = (branch, project_dir) {
            let worktree_name = branch.replace('/', "-");
            let _ = self.worktree_manager.remove_worktree(
                std::path::Path::new(&dir),
                &worktree_name,
            );
        }
    }

    // ── Activity logging helper ──────────────────────────────────────

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

        // Also write to loom if we have team + project context
        if let (Some(tid), Some(pid)) = (team_id, project_id) {
            use crate::models::loom::{LoomEntry, CreateLoomEntry};
            let loom_type = match event_type {
                "agent_spawned" => "status",
                "agent_completed" => "completion",
                "issue_claimed" => "status",
                "issue_retry" | "issue_max_retries" => "warning",
                _ => "status",
            };
            let _ = LoomEntry::create(&conn, &CreateLoomEntry {
                agent_id: agent_id.map(|s| s.to_string()),
                team_id: tid.to_string(),
                project_id: pid.to_string(),
                workflow_instance_id: workflow_instance_id.map(|s| s.to_string()),
                entry_type: loom_type.to_string(),
                content: message.to_string(),
            });
        }
    }

    // ── Startup recovery (Task 13) ──────────────────────────────────

    pub async fn restore_running_instances(&mut self) {
        // Reset orphaned sessions and issues from agents killed by restart
        {
            let conn = self.db.lock().unwrap();

            // Mark stale agent sessions as failed
            let stale_sessions = conn.execute(
                "UPDATE agent_sessions SET state = 'failed', updated_at = datetime('now') \
                 WHERE state IN ('running', 'idle', 'ready', 'working')",
                [],
            ).unwrap_or(0);
            if stale_sessions > 0 {
                tracing::info!("Cleaned up {} stale agent sessions on startup", stale_sessions);
            }

            // Reset orphaned in_progress issues
            let reset_count = conn.execute(
                "UPDATE issues SET status = 'open', claimed_by = NULL, claimed_at = NULL, \
                 updated_at = datetime('now') \
                 WHERE status = 'in_progress' AND claimed_by IS NOT NULL",
                [],
            ).unwrap_or(0);
            if reset_count > 0 {
                tracing::info!("Reset {} orphaned in_progress issues on startup", reset_count);
            }
        }

        let instances: Vec<(String, String)> = {
            let conn = self.db.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, definition_id FROM workflow_instances WHERE state = 'running'"
            ).unwrap();
            stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }).unwrap().filter_map(|r| r.ok()).collect()
        };

        for (instance_id, definition_id) in instances {
            tracing::info!(workflow = %instance_id, "Restoring running workflow");
            if let Err(e) = self.start_workflow(instance_id.clone(), definition_id).await {
                tracing::error!(workflow = %instance_id, "Failed to restore: {}", e);
                let conn = self.db.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE workflow_instances SET state = 'failed' WHERE id = ?1",
                    rusqlite::params![instance_id],
                );
            }
        }
    }

    // ── Main loop (Task 7) ─────────────────────────────────────────

    pub async fn run(mut self) {
        tracing::info!("Orchestrator started");
        let mut tick = interval(Duration::from_secs(SWEEP_INTERVAL_SECS));

        loop {
            tokio::select! {
                event = self.rx.recv() => {
                    match event {
                        Some(e) => self.handle_event(e).await,
                        None => {
                            tracing::info!("Orchestrator channel closed, shutting down");
                            break;
                        }
                    }
                }
                _ = tick.tick() => {
                    self.sweep().await;
                }
            }
        }
    }

    async fn handle_event(&mut self, event: OrchestratorEvent) {
        match event {
            OrchestratorEvent::InstanceCreated {
                instance_id,
                definition_id,
            } => {
                if self.active_workflows.contains_key(&instance_id) {
                    return; // idempotent
                }
                if let Err(e) = self.start_workflow(instance_id, definition_id).await {
                    tracing::error!("Failed to start workflow: {}", e);
                }
            }
        }
    }

    // ── start_workflow (Task 8) ────────────────────────────────────

    async fn start_workflow(
        &mut self,
        instance_id: String,
        definition_id: String,
    ) -> crate::error::Result<()> {
        // Load definition from DB
        let (dag_json, project_id) = {
            let conn = self.db.lock().unwrap();
            let def = WorkflowDefinition::get_by_id(&conn, &definition_id)?;
            (def.dag, def.project_id)
        };

        // Parse DAG
        let dag = DagDefinition::from_json(&dag_json)?;
        let execution = DagExecutionState::new(&dag)?;

        // Create state machine and transition to Running
        let mut state_machine = StateMachine::new(instance_id.clone(), self.db.clone());
        state_machine.transition(WorkflowState::Running)?;

        let mut run_state = WorkflowRunState {
            instance_id: instance_id.clone(),
            definition_id,
            project_id,
            dag,
            execution,
            stage_agents: HashMap::new(),
            state_machine,
            stage_retry_counts: HashMap::new(),
            stages_pending_retry: HashMap::new(),
        };

        // Spawn agents for ready stages
        let ready = run_state.execution.ready_stages();
        for stage_id in ready {
            Self::spawn_stage_agent(&self.db, &self.process_manager, &mut run_state, &stage_id)
                .await?;
        }

        // Checkpoint
        let checkpoint = serde_json::to_value(&run_state.execution)?;
        run_state.state_machine.checkpoint(checkpoint)?;

        self.log_activity("workflow_started", "Workflow started", Some(&run_state.project_id), None, None, None, Some(&instance_id));
        self.active_workflows.insert(instance_id, run_state);
        Ok(())
    }

    // ── spawn_stage_agent (Task 9) ─────────────────────────────────

    /// Spawn an agent for a stage. This is a static-ish method that takes
    /// references to the shared resources explicitly, avoiding double-borrow
    /// issues when called from sweep_workflow (which holds &mut self via
    /// active_workflows).
    async fn spawn_stage_agent(
        db: &DbPool,
        process_manager: &ProcessManager,
        run_state: &mut WorkflowRunState,
        stage_id: &str,
    ) -> crate::error::Result<()> {
        let stage = run_state
            .dag
            .stages
            .iter()
            .find(|s| s.id == stage_id)
            .ok_or_else(|| {
                crate::error::IronweaveError::NotFound(format!("stage {}", stage_id))
            })?
            .clone();

        // Manual gates wait for approval instead of spawning an agent
        if stage.is_manual_gate {
            run_state
                .execution
                .update_stage(stage_id, StageStatus::WaitingApproval);
            tracing::info!(
                workflow = %run_state.instance_id,
                stage = %stage_id,
                "Stage is manual gate — waiting for approval"
            );
            return Ok(());
        }

        // Reuse existing issue on retry, or create a new one
        let issue = if let Some(retry_issue_id) = run_state.stages_pending_retry.remove(stage_id) {
            // Retry case — reopen the existing issue
            let conn = db.lock().unwrap();
            let _ = Issue::update(
                &conn,
                &retry_issue_id,
                &crate::models::issue::UpdateIssue {
                    status: Some("open".to_string()),
                    ..Default::default()
                },
            );
            Issue::get_by_id(&conn, &retry_issue_id)?
        } else {
            let conn = db.lock().unwrap();
            Issue::create(
                &conn,
                &CreateIssue {
                    project_id: run_state.project_id.clone(),
                    issue_type: Some("task".to_string()),
                    title: stage.name.clone(),
                    description: Some(stage.prompt.clone()),
                    priority: None,
                    depends_on: None,
                    workflow_instance_id: Some(run_state.instance_id.clone()),
                    stage_id: Some(stage_id.to_string()),
                    role: None,
                    parent_id: None,
                    needs_intake: Some(0),
                    scope_mode: None,
                },
            )?
        };

        // Look up project directory
        let project_dir = {
            let conn = db.lock().unwrap();
            Project::get_by_id(&conn, &run_state.project_id)
                .map(|p| p.directory)
                .unwrap_or_else(|_| "/home/paddy".to_string())
        };

        // Build agent config
        let session_id = uuid::Uuid::new_v4().to_string();
        let prompt = format!(
            "{}\n\n\
            You are working on issue {} in project {}.\n\n\
            When you have completed your work, close your issue by running:\n\
            curl -sk -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"status\": \"closed\", \"summary\": \"Brief description of what you accomplished\"}}'\n\n\
            You can also post progress updates at any time:\n\
            curl -sk -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"summary\": \"Current progress update\"}}'",
            stage.prompt,
            issue.id, run_state.project_id,
            run_state.project_id, issue.id,
            run_state.project_id, issue.id,
        );

        let config = AgentConfig {
            working_directory: std::path::PathBuf::from(project_dir),
            prompt,
            environment: {
                let api_url = {
                    let conn = db.lock().unwrap();
                    crate::models::setting::Setting::get_by_key(&conn, "api_url")
                        .map(|s| s.value)
                        .unwrap_or_else(|_| "https://localhost:443".to_string())
                };
                let mut env = std::collections::HashMap::new();
                env.insert("IRONWEAVE_API".to_string(), api_url);
                env.insert("TERM".to_string(), "xterm-256color".to_string());
                // Inject API keys from settings for non-Claude runtimes
                {
                    let conn = db.lock().unwrap();
                    let key_mappings = [
                        ("gemini_api_key", "GEMINI_API_KEY"),
                        ("google_api_key", "GOOGLE_API_KEY"),
                        ("apikey_openrouter_api_key", "OPENROUTER_API_KEY"),
                        ("anthropic_api_key", "ANTHROPIC_API_KEY"),
                        ("ollama_host", "OLLAMA_HOST"),
                    ];
                    for (setting_key, env_var) in &key_mappings {
                        if let Ok(s) = crate::models::setting::Setting::get_by_key(&conn, setting_key) {
                            if !s.value.is_empty() {
                                env.insert(env_var.to_string(), s.value);
                            }
                        }
                    }
                }
                // Ensure PATH includes runtime binary locations
                let path = std::env::var("PATH").unwrap_or_default();
                env.insert("PATH".to_string(), format!(
                    "/home/paddy/.opencode/bin:/home/paddy/.npm-global/bin:/home/paddy/.cargo/bin:{}",
                    path
                ));
                Some(env)
            },
            allowed_tools: None,
            skills: None,
            extra_args: Some(vec!["--print".to_string()]),
            playwright_env: None,
            model: None,
        };

        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        // Spawn the agent
        process_manager
            .spawn_agent(&session_id, &stage.runtime, config, size)
            .await?;

        // Note: We do NOT call Issue::claim() here because claimed_by has a FK
        // to agent_sessions, which is populated by the team system (F4). The
        // orchestrator tracks the agent-issue link in memory via StageAgent.

        // Track in-memory
        run_state
            .execution
            .update_stage(stage_id, StageStatus::Running);
        let retry_count = run_state
            .stage_retry_counts
            .get(stage_id)
            .copied()
            .unwrap_or(0);
        run_state.stage_agents.insert(
            stage_id.to_string(),
            StageAgent {
                agent_session_id: session_id,
                issue_id: issue.id,
                spawned_at: Instant::now(),
                last_activity: Instant::now(),
                nudge_count: 0,
                last_issue_updated_at: String::new(),
                retry_count,
            },
        );

        tracing::info!(
            workflow = %run_state.instance_id,
            stage = %stage_id,
            "Spawned agent for stage"
        );
        Ok(())
    }

    // ── sweep (Task 10) ────────────────────────────────────────────

    async fn sweep(&mut self) {
        let instance_ids: Vec<String> = self.active_workflows.keys().cloned().collect();

        for instance_id in instance_ids {
            if let Err(e) = self.sweep_workflow(&instance_id).await {
                tracing::error!(workflow = %instance_id, "Sweep error: {}", e);
            }
        }

        // Intake agent sweep — before team dispatch so new children are available next cycle
        if let Err(e) = self.sweep_intake().await {
            tracing::error!("Intake sweep error: {}", e);
        }

        // Killswitch: evaluate cron schedules
        self.evaluate_schedules();

        // Killswitch: check global dispatch pause
        let global_paused = {
            let conn = self.db.lock().unwrap();
            crate::models::setting::Setting::get_by_key(&conn, "global_dispatch_paused")
                .map(|s| s.value == "true")
                .unwrap_or(false)
        };

        if global_paused {
            tracing::debug!("Global dispatch paused — skipping team dispatch");
        } else {
            if let Err(e) = self.sweep_teams().await {
                tracing::error!("Team sweep error: {}", e);
            }
        }

        // Parent auto-close sweep
        if let Err(e) = self.sweep_parent_autoclose().await {
            tracing::error!("Parent auto-close sweep error: {}", e);
        }

        // Dead session reaper — mark stale/orphaned sessions as dead, unclaim their issues
        {
            let conn = self.db.lock().unwrap();
            match AgentSession::reap_dead_sessions(&conn, 1800) {
                Ok(reaped) if !reaped.is_empty() => {
                    tracing::info!("Reaped {} dead sessions: {:?}", reaped.len(), reaped);
                }
                Err(e) => tracing::error!("Dead session reaper error: {}", e),
                _ => {}
            }
        }

        // Merge queue processing
        if let Err(e) = self.sweep_merge_queue().await {
            tracing::error!("Merge queue sweep error: {}", e);
        }

        // Remove completed/failed workflows from active map
        self.active_workflows
            .retain(|_, ws| !ws.execution.is_complete());
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
            // Fetch issue and update activity tracking in a scoped mutable borrow
            let (issue_status, agent_session_id, idle_secs, nudge_count) = {
                let sa = run_state.stage_agents.get_mut(stage_id).unwrap();
                let issue = {
                    let conn = self.db.lock().unwrap();
                    Issue::get_by_id(&conn, &sa.issue_id)?
                };

                // Reset idle timer if the issue was updated since last sweep
                if issue.updated_at != sa.last_issue_updated_at {
                    sa.last_activity = Instant::now();
                    sa.last_issue_updated_at = issue.updated_at.clone();
                }

                (
                    issue.status.clone(),
                    sa.agent_session_id.clone(),
                    sa.last_activity.elapsed().as_secs(),
                    sa.nudge_count,
                )
            };

            if issue_status == "closed" {
                completed_stages.push(stage_id.clone());
                continue;
            }

            // Check if agent PTY is still alive
            let agent_exists = self
                .process_manager
                .get_agent(&agent_session_id)
                .await
                .is_some();
            if !agent_exists && issue_status != "closed" {
                failed_stages.push(stage_id.clone());
                continue;
            }

            // Check for idle agent -- nudge/kill escalation
            if idle_secs >= KILL_THRESHOLD_SECS {
                tracing::warn!(workflow = %instance_id, stage = %stage_id, "Killing idle agent");
                let _ = self
                    .process_manager
                    .stop_agent(&agent_session_id)
                    .await;
                failed_stages.push(stage_id.clone());
            } else if idle_secs >= NUDGE_WARNING_SECS && nudge_count >= 1 {
                let msg = "\n\nNo status update received. Please respond or your session will be terminated.\n";
                let _ = self
                    .process_manager
                    .write_to_agent(&agent_session_id, msg.as_bytes())
                    .await;
                run_state
                    .stage_agents
                    .get_mut(stage_id)
                    .unwrap()
                    .nudge_count = 2;
            } else if idle_secs >= NUDGE_THRESHOLD_SECS && nudge_count == 0 {
                let msg =
                    "\n\nPlease update your issue status with your current progress.\n";
                let _ = self
                    .process_manager
                    .write_to_agent(&agent_session_id, msg.as_bytes())
                    .await;
                run_state
                    .stage_agents
                    .get_mut(stage_id)
                    .unwrap()
                    .nudge_count = 1;
            }
        }

        // 2. Mark completed stages
        for stage_id in &completed_stages {
            run_state
                .execution
                .update_stage(stage_id, StageStatus::Completed);
            if let Some(sa) = run_state.stage_agents.remove(stage_id) {
                self.process_manager.remove_agent(&sa.agent_session_id).await;
            }
            tracing::info!(workflow = %instance_id, stage = %stage_id, "Stage completed");
        }

        // 3. Handle failed stages: retry once, then permanently fail
        for stage_id in &failed_stages {
            if let Some(sa) = run_state.stage_agents.remove(stage_id) {
                self.process_manager.remove_agent(&sa.agent_session_id).await;
                let conn = self.db.lock().unwrap();
                let _ = Issue::unclaim(&conn, &sa.issue_id);

                let retries = run_state.stage_retry_counts.get(stage_id).copied().unwrap_or(0);
                if retries < 1 {
                    // First failure — reset to Pending for re-spawn on next sweep tick
                    run_state
                        .execution
                        .update_stage(stage_id, StageStatus::Pending);
                    run_state
                        .stage_retry_counts
                        .insert(stage_id.clone(), retries + 1);
                    // Remember the issue_id so spawn_stage_agent can reuse it
                    run_state
                        .stages_pending_retry
                        .insert(stage_id.clone(), sa.issue_id);
                    tracing::warn!(
                        workflow = %instance_id, stage = %stage_id,
                        "Stage failed — scheduling retry (attempt 1/1)"
                    );
                } else {
                    // Second failure — permanently fail the stage
                    let reason = "agent died or timed out after retry".to_string();
                    run_state
                        .execution
                        .update_stage(stage_id, StageStatus::Failed(reason));
                    tracing::error!(
                        workflow = %instance_id, stage = %stage_id,
                        "Stage permanently failed after retry"
                    );
                }
            }
        }

        // 4. Check for gate approvals from the database
        {
            let run_state = self.active_workflows.get_mut(instance_id).unwrap();
            let pending_gates = run_state.execution.has_pending_approvals();
            for gate_id in &pending_gates {
                let approved = {
                    let conn = self.db.lock().unwrap();
                    let exists: bool = conn.query_row(
                        "SELECT COUNT(*) > 0 FROM workflow_gate_approvals WHERE instance_id = ?1 AND stage_id = ?2",
                        rusqlite::params![instance_id, gate_id],
                        |row| row.get(0),
                    ).unwrap_or(false);
                    if exists {
                        // Delete the approval row so it's not processed again
                        let _ = conn.execute(
                            "DELETE FROM workflow_gate_approvals WHERE instance_id = ?1 AND stage_id = ?2",
                            rusqlite::params![instance_id, gate_id],
                        );
                    }
                    exists
                };
                if approved {
                    if let Err(e) = run_state.execution.approve_gate(gate_id) {
                        tracing::error!(workflow = %instance_id, stage = %gate_id, "Failed to approve gate: {}", e);
                    } else {
                        // Gate approval moves to Running; mark as Completed since gates
                        // don't need agent execution — they're just approval checkpoints.
                        run_state.execution.update_stage(gate_id, StageStatus::Completed);
                        tracing::info!(workflow = %instance_id, stage = %gate_id, "Gate approved and completed");
                    }
                }
            }
        }

        // 5. Spawn agents for newly ready stages
        //    Collect ready stages and project_id first to avoid borrow conflicts.
        let ready_info: Option<(Vec<String>, String)> = {
            let rs = self.active_workflows.get(instance_id).unwrap();
            let ready = rs.execution.ready_stages();
            if ready.is_empty() {
                None
            } else {
                Some((ready, rs.project_id.clone()))
            }
        };

        if let Some((ready, project_id)) = ready_info {
            // Log stage starts before mutable borrow
            for stage_id in &ready {
                self.log_activity(
                    "stage_started",
                    &format!("Stage '{}' started", stage_id),
                    Some(&project_id),
                    None, None, None,
                    Some(instance_id),
                );
            }

            let run_state = self.active_workflows.get_mut(instance_id).unwrap();
            let db = &self.db;
            let pm = &self.process_manager;
            for stage_id in ready {
                if let Err(e) = Self::spawn_stage_agent(db, pm, run_state, &stage_id).await {
                    tracing::error!(workflow = %instance_id, stage = %stage_id, "Failed to spawn: {}", e);
                    run_state
                        .execution
                        .update_stage(&stage_id, StageStatus::Failed(e.to_string()));
                }
            }
        }

        // 6. Check workflow completion
        let completion_info: Option<(bool, String)> = {
            let rs = match self.active_workflows.get(instance_id) {
                Some(rs) => rs,
                None => return Ok(()),
            };
            if rs.execution.is_complete() {
                let all_succeeded = rs
                    .execution
                    .stage_statuses
                    .values()
                    .all(|s| matches!(s, StageStatus::Completed));
                Some((all_succeeded, rs.project_id.clone()))
            } else {
                None
            }
        };

        if let Some((all_succeeded, project_id)) = completion_info {
            let state_label = if all_succeeded { "completed" } else { "failed" };
            self.log_activity(
                &format!("workflow_{}", state_label),
                &format!("Workflow {}", state_label),
                Some(&project_id),
                None, None, None,
                Some(instance_id),
            );
            let run_state = self.active_workflows.get_mut(instance_id).unwrap();
            let new_state = if all_succeeded {
                WorkflowState::Completed
            } else {
                WorkflowState::Failed
            };
            if let Err(e) = run_state.state_machine.transition(new_state) {
                tracing::error!(workflow = %instance_id, "Failed to transition: {}", e);
            }
            tracing::info!(workflow = %instance_id, "Workflow finished");
        }

        // 7. Checkpoint
        let run_state = match self.active_workflows.get_mut(instance_id) {
            Some(rs) => rs,
            None => return Ok(()),
        };
        let checkpoint = serde_json::to_value(&run_state.execution)?;
        run_state.state_machine.checkpoint(checkpoint)?;

        Ok(())
    }

    // ── evaluate_schedules (killswitch cron) ──────────────────────

    /// Evaluate dispatch schedules and auto-pause/resume as needed.
    fn evaluate_schedules(&self) {
        let conn = self.db.lock().unwrap();
        let schedules = match crate::models::dispatch_schedule::DispatchSchedule::list_enabled(&conn) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to load dispatch schedules: {}", e);
                return;
            }
        };

        let now_utc = chrono::Utc::now();

        for schedule in &schedules {
            let tz: chrono_tz::Tz = match schedule.timezone.parse() {
                Ok(tz) => tz,
                Err(_) => {
                    tracing::warn!(schedule_id = %schedule.id, tz = %schedule.timezone, "Invalid timezone, skipping");
                    continue;
                }
            };

            let _now_local = now_utc.with_timezone(&tz);

            let cron_schedule = match cron::Schedule::from_str(&schedule.cron_expression) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(schedule_id = %schedule.id, expr = %schedule.cron_expression, "Invalid cron: {}", e);
                    continue;
                }
            };

            // Check if cron matches within the last 60 seconds (sweep runs every 30s)
            // The cron crate works with UTC DateTime, so convert window boundaries to UTC
            let window_start = now_utc - chrono::Duration::seconds(60);
            let has_match = cron_schedule
                .after(&window_start)
                .take(1)
                .any(|t| t <= now_utc);

            if !has_match {
                continue;
            }

            tracing::info!(
                schedule_id = %schedule.id, action = %schedule.action, scope = %schedule.scope,
                "Dispatch schedule triggered"
            );

            match (schedule.scope.as_str(), schedule.action.as_str()) {
                ("global", "pause") => {
                    let _ = crate::models::setting::Setting::upsert(&conn, "global_dispatch_paused", &crate::models::setting::UpsertSetting {
                        value: "true".to_string(), category: Some("killswitch".to_string()),
                    });
                    let _ = crate::models::setting::Setting::upsert(&conn, "global_paused_at", &crate::models::setting::UpsertSetting {
                        value: chrono::Utc::now().to_rfc3339(), category: Some("killswitch".to_string()),
                    });
                    let _ = crate::models::setting::Setting::upsert(&conn, "global_pause_reason", &crate::models::setting::UpsertSetting {
                        value: schedule.description.clone().unwrap_or_else(|| "Scheduled pause".to_string()),
                        category: Some("killswitch".to_string()),
                    });
                }
                ("global", "resume") => {
                    let _ = crate::models::setting::Setting::upsert(&conn, "global_dispatch_paused", &crate::models::setting::UpsertSetting {
                        value: "false".to_string(), category: Some("killswitch".to_string()),
                    });
                }
                ("project", "pause") => {
                    if let Some(ref pid) = schedule.project_id {
                        let reason = schedule.description.as_deref().unwrap_or("Scheduled pause");
                        let _ = crate::models::project::Project::pause(&conn, pid, Some(reason));
                    }
                }
                ("project", "resume") => {
                    if let Some(ref pid) = schedule.project_id {
                        let _ = crate::models::project::Project::resume(&conn, pid);
                    }
                }
                _ => {}
            }
        }
    }

    // ── sweep_teams (Task 8 – team dispatch) ──────────────────────

    async fn sweep_teams(&mut self) -> crate::error::Result<()> {
        // 1. Check existing team-dispatched agents for completion or death
        let agent_keys: Vec<String> = self.team_agents.keys().cloned().collect();
        let mut to_remove = Vec::new();

        for key in &agent_keys {
            let ta = self.team_agents.get_mut(key).unwrap();

            // Check if issue is closed → agent is done
            let issue = {
                let conn = self.db.lock().unwrap();
                Issue::get_by_id(&conn, &ta.issue_id)?
            };

            if issue.status == "closed" {
                // Mark session as completed
                {
                    let conn = self.db.lock().unwrap();
                    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, "completed");
                }

                // Enqueue for merge if agent was working in a worktree
                {
                    let conn = self.db.lock().unwrap();
                    if let Ok(session) = AgentSession::get_by_id(&conn, &ta.agent_session_id) {
                        if let (Some(branch), Some(_wt_path)) = (&session.branch, &session.worktree_path) {
                            let project_id_for_merge = Team::get_by_id(&conn, &ta.team_id)
                                .map(|t| t.project_id)
                                .unwrap_or_default();
                            // Check if this branch already has a non-terminal entry (pending/merging/verifying/reviewing/resolving)
                            let already_queued: bool = conn.query_row(
                                "SELECT COUNT(*) FROM merge_queue WHERE branch_name = ?1 AND project_id = ?2 AND status IN ('pending', 'merging', 'verifying', 'reviewing', 'resolving')",
                                rusqlite::params![branch, project_id_for_merge],
                                |row| row.get::<_, i64>(0),
                            ).unwrap_or(0) > 0;
                            if already_queued {
                                tracing::info!(branch = %branch, "Branch already in merge queue, skipping duplicate enqueue");
                            } else {
                                let merge_id = uuid::Uuid::new_v4().to_string();
                                let _ = conn.execute(
                                    "INSERT INTO merge_queue (id, project_id, branch_name, agent_session_id, issue_id, team_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                    rusqlite::params![merge_id, project_id_for_merge, branch, ta.agent_session_id, ta.issue_id, ta.team_id],
                                );
                                tracing::info!(branch = %branch, "Enqueued branch for merge");
                            }
                        }
                    }
                }

                self.process_manager.remove_agent(&ta.agent_session_id).await;
                tracing::info!(
                    team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                    "Team agent completed issue"
                );
                {
                    use crate::models::activity_log::{ActivityLogEntry, LogEvent};
                    let conn = self.db.lock().unwrap();
                    let _ = ActivityLogEntry::log(&conn, &LogEvent {
                        event_type: "agent_completed".to_string(),
                        project_id: None,
                        team_id: Some(ta.team_id.clone()),
                        agent_id: Some(ta.agent_session_id.clone()),
                        issue_id: Some(ta.issue_id.clone()),
                        workflow_instance_id: None,
                        message: "Agent completed work".to_string(),
                        metadata: None,
                    });
                }
                // ── Auto-log performance (v2 Phase 2.2) ────────────────────
                {
                    let conn = self.db.lock().unwrap();
                    if let Ok(session) = AgentSession::get_by_id(&conn, &ta.agent_session_id) {
                        let project_id = Team::get_by_id(&conn, &ta.team_id)
                            .map(|t| t.project_id).unwrap_or_default();
                        let duration = ta.spawned_at.elapsed().as_secs() as i64;
                        let _ = crate::models::performance_log::PerformanceLogEntry::create(&conn,
                            &crate::models::performance_log::CreatePerformanceLog {
                                project_id,
                                role: ta.role.clone(),
                                runtime: session.runtime.clone(),
                                provider: None,
                                model: session.model.clone().unwrap_or_else(|| "unknown".to_string()),
                                tier: None,
                                task_type: Some(issue.issue_type.clone()),
                                task_complexity: None,
                                outcome: "success".to_string(),
                                failure_reason: None,
                                tokens_used: Some(session.tokens_used),
                                cost_usd: Some(session.cost),
                                duration_seconds: Some(duration),
                                retries: Some(issue.retry_count as i32),
                                escalated_from: None,
                                complexity_score: None,
                                files_touched: None,
                            },
                        );
                    }
                }

                // Note: worktree cleanup happens after merge in sweep_merge_queue
                to_remove.push(key.clone());
                continue;
            }

            // Check if agent process has exited
            let exit_status = self.process_manager.check_agent_exit(&ta.agent_session_id).await;
            if let Some(success) = exit_status {
                let session_id_for_cleanup = ta.agent_session_id.clone();
                self.process_manager.remove_agent(&ta.agent_session_id).await;
                // If issue is still in_progress, agent exited without closing it —
                // unclaim so it can be retried, regardless of exit code
                if issue.status != "closed" {
                    let conn = self.db.lock().unwrap();
                    let state = if success { "completed" } else { "failed" };
                    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, state);

                    const MAX_RETRIES: i64 = 3;

                    // If this was a merge_conflict resolver agent, escalate the merge queue entry
                    if issue.issue_type == "merge_conflict" {
                        use crate::models::merge_queue_entry::MergeQueueEntry;
                        // Find the merge queue entry this resolver was working on
                        let mut stmt = conn.prepare(
                            "SELECT id FROM merge_queue WHERE resolver_agent_id = ?1 AND status = 'resolving'"
                        ).ok();
                        if let Some(ref mut stmt) = stmt {
                            let entry_ids: Vec<String> = stmt.query_map(
                                params![&ta.agent_session_id],
                                |row| row.get(0),
                            ).ok().map(|rows| rows.filter_map(|r| r.ok()).collect()).unwrap_or_default();
                            for eid in &entry_ids {
                                let _ = MergeQueueEntry::update_status(
                                    &conn, eid, "escalated", None, None,
                                    Some("Resolver agent failed — escalated to human review"),
                                );
                                tracing::warn!(entry_id = %eid, "Resolver failed — escalated merge queue entry");
                            }
                        }
                    }

                    // ── v2 Phase 3.2: Tier-aware failure handling ────────────
                    // Check if we can escalate to a higher tier within bounds
                    if !success {
                        if AgentSession::get_by_id(&conn, &ta.agent_session_id).is_ok() {
                            let current_tier: i32 = conn.query_row(
                                "SELECT COALESCE(
                                    (SELECT tier FROM model_performance_log WHERE id = (
                                        SELECT id FROM model_performance_log WHERE project_id = (
                                            SELECT project_id FROM teams WHERE id = ?1
                                        ) ORDER BY created_at DESC LIMIT 1
                                    )),
                                    3
                                )",
                                params![&ta.team_id],
                                |row| row.get(0),
                            ).unwrap_or(3);

                            let tier_ceiling: i32 = conn.query_row(
                                "SELECT COALESCE(t.tier_ceiling, p.tier_ceiling)
                                 FROM teams t JOIN projects p ON t.project_id = p.id
                                 WHERE t.id = ?1",
                                params![&ta.team_id],
                                |row| row.get(0),
                            ).unwrap_or(5);

                            if current_tier < tier_ceiling {
                                tracing::info!(
                                    team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                                    current_tier = current_tier, ceiling = tier_ceiling,
                                    "Agent failed — eligible for tier escalation on retry"
                                );
                                // The performance log entry already records the failure with
                                // escalated_from context; the next dispatch will use performance
                                // data to select a higher-tier model
                            } else {
                                // At ceiling — post loom entry alerting user
                                let project_id = Team::get_by_id(&conn, &ta.team_id)
                                    .map(|t| t.project_id).unwrap_or_default();
                                let _ = conn.execute(
                                    "INSERT INTO loom (id, project_id, team_id, agent_id, entry_type, role, content)
                                     VALUES (?1, ?2, ?3, ?4, 'escalation', ?5, ?6)",
                                    params![
                                        uuid::Uuid::new_v4().to_string(),
                                        project_id,
                                        ta.team_id,
                                        ta.agent_session_id,
                                        ta.role,
                                        format!("Task failed at tier ceiling ({}). Consider raising the quality ceiling for this team or project.", tier_ceiling),
                                    ],
                                );
                                tracing::warn!(
                                    team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                                    ceiling = tier_ceiling,
                                    "Agent failed at tier ceiling — posted loom escalation"
                                );
                            }
                        }
                    }

                    if issue.retry_count >= MAX_RETRIES {
                        // Max retries reached — mark as needs_investigation instead of retrying
                        let _ = conn.execute(
                            "UPDATE issues SET status = 'open', claimed_by = NULL, claimed_at = NULL, \
                             needs_intake = 1, updated_at = datetime('now') WHERE id = ?1",
                            params![&ta.issue_id],
                        );
                        tracing::error!(
                            team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                            retries = issue.retry_count,
                            "Issue hit max retries ({}) — flagging for investigation",
                            MAX_RETRIES,
                        );
                    } else {
                        let _ = Issue::unclaim(&conn, &ta.issue_id);
                        tracing::warn!(
                            team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                            exit_success = success, retry = issue.retry_count + 1,
                            "Team agent exited without closing issue — unclaiming for retry ({}/{})",
                            issue.retry_count + 1, MAX_RETRIES,
                        );
                    }
                }
                // ── Auto-log performance on exit (v2 Phase 2.2) ────────────
                {
                    let conn = self.db.lock().unwrap();
                    if let Ok(session) = AgentSession::get_by_id(&conn, &session_id_for_cleanup) {
                        let project_id = Team::get_by_id(&conn, &ta.team_id)
                            .map(|t| t.project_id).unwrap_or_default();
                        let duration = ta.spawned_at.elapsed().as_secs() as i64;
                        let outcome = if success && issue.status == "closed" { "success" } else if success { "partial" } else { "failure" };
                        let _ = crate::models::performance_log::PerformanceLogEntry::create(&conn,
                            &crate::models::performance_log::CreatePerformanceLog {
                                project_id,
                                role: ta.role.clone(),
                                runtime: session.runtime.clone(),
                                provider: None,
                                model: session.model.clone().unwrap_or_else(|| "unknown".to_string()),
                                tier: None,
                                task_type: Some(issue.issue_type.clone()),
                                task_complexity: None,
                                outcome: outcome.to_string(),
                                failure_reason: if !success { Some("process_exit".to_string()) } else { None },
                                tokens_used: Some(session.tokens_used),
                                cost_usd: Some(session.cost),
                                duration_seconds: Some(duration),
                                retries: Some(issue.retry_count as i32),
                                escalated_from: None,
                                complexity_score: None,
                                files_touched: None,
                            },
                        );
                    }
                }

                // Clean up worktree after handling the exit
                self.cleanup_agent_worktree(&session_id_for_cleanup);
                to_remove.push(key.clone());
                continue;
            }

            // Reset idle timer if issue was updated since last sweep
            if issue.updated_at != ta.last_issue_updated_at {
                ta.last_activity = Instant::now();
                ta.last_issue_updated_at = issue.updated_at.clone();
            }

            // Check if PTY is still alive
            let agent_exists = self
                .process_manager
                .get_agent(&ta.agent_session_id)
                .await
                .is_some();
            if !agent_exists {
                // Agent removed (e.g. by WS handler) — unclaim issue and mark session failed
                let session_id_for_wt = ta.agent_session_id.clone();
                {
                    let conn = self.db.lock().unwrap();
                    let _ = Issue::unclaim(&conn, &ta.issue_id);
                    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, "failed");
                }
                tracing::warn!(
                    team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                    "Team agent PTY died — unclaiming issue"
                );
                self.cleanup_agent_worktree(&session_id_for_wt);
                to_remove.push(key.clone());
                continue;
            }

            // Idle escalation
            let idle_secs = ta.last_activity.elapsed().as_secs();
            if idle_secs >= KILL_THRESHOLD_SECS {
                tracing::warn!(team = %ta.team_id, role = %ta.role, "Killing idle team agent");
                let idle_session_id = ta.agent_session_id.clone();
                let _ = self
                    .process_manager
                    .stop_agent(&ta.agent_session_id)
                    .await;
                {
                    let conn = self.db.lock().unwrap();
                    let _ = Issue::unclaim(&conn, &ta.issue_id);
                    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, "failed");
                }
                self.cleanup_agent_worktree(&idle_session_id);
                to_remove.push(key.clone());
            } else if idle_secs >= NUDGE_WARNING_SECS && ta.nudge_count >= 1 {
                let msg = "\n\nNo status update received. Please respond or your session will be terminated.\n";
                let _ = self
                    .process_manager
                    .write_to_agent(&ta.agent_session_id, msg.as_bytes())
                    .await;
                ta.nudge_count = 2;
            } else if idle_secs >= NUDGE_THRESHOLD_SECS && ta.nudge_count == 0 {
                let msg =
                    "\n\nPlease update your issue status with your current progress.\n";
                let _ = self
                    .process_manager
                    .write_to_agent(&ta.agent_session_id, msg.as_bytes())
                    .await;
                ta.nudge_count = 1;
            }
        }

        // Remove completed/failed team agents
        for key in &to_remove {
            self.team_agents.remove(key);
        }

        // 2. Query active teams and dispatch new agents
        let active_teams = {
            let conn = self.db.lock().unwrap();
            Team::list_active(&conn)?
        };

        for team in &active_teams {
            // Killswitch: check project-level pause
            {
                let conn = self.db.lock().unwrap();
                if let Ok(project) = crate::models::project::Project::get_by_id(&conn, &team.project_id) {
                    if project.is_paused {
                        tracing::debug!(project = %team.project_id, "Project paused — skipping team {}", team.id);
                        continue;
                    }
                }
            }

            // ── Budget enforcement (v2 Phase 1.2) ────────────────────────
            // Skip this team if it has exceeded its daily cost or token budget
            if team.cost_budget_daily.is_some() || team.token_budget.is_some() {
                let conn = self.db.lock().unwrap();
                if let Some(budget) = team.cost_budget_daily {
                    if budget > 0.0 {
                        let spent = crate::models::cost_tracking::CostTrackingEntry::team_cost_today(&conn, &team.id).unwrap_or(0.0);
                        if spent >= budget {
                            tracing::info!(team = %team.id, spent = spent, budget = budget,
                                "Team daily cost budget exceeded, skipping dispatch");
                            continue;
                        }
                    }
                }
                if let Some(budget) = team.token_budget {
                    if budget > 0 {
                        let used = crate::models::cost_tracking::CostTrackingEntry::team_tokens_today(&conn, &team.id).unwrap_or(0);
                        if used >= budget as i64 {
                            tracing::info!(team = %team.id, used = used, budget = budget,
                                "Team token budget exceeded, skipping dispatch");
                            continue;
                        }
                    }
                }
            }

            let slots = {
                let conn = self.db.lock().unwrap();
                TeamAgentSlot::list_by_team(&conn, &team.id)?
            };

            let pickup_types = team.get_auto_pickup_types();
            if pickup_types.is_empty() {
                continue;
            }
            let pickup_refs: Vec<&str> = pickup_types.iter().map(|s| s.as_str()).collect();

            tracing::debug!(
                team_id = %team.id, team_name = %team.name, mode = %team.coordination_mode,
                slots = slots.len(), pickup_types = ?pickup_refs,
                "Dispatching for team"
            );

            match team.coordination_mode.as_str() {
                "pipeline" => {
                    self.dispatch_pipeline(team, &slots, &pickup_refs).await?;
                }
                "collaborative" => {
                    self.dispatch_collaborative(team, &slots, &pickup_refs).await?;
                }
                "hierarchical" => {
                    self.dispatch_hierarchical(team, &slots, &pickup_refs).await?;
                }
                _ => {
                    // "swarm" and any unknown mode: all slots claim freely from the ready pool
                    self.dispatch_swarm(team, &slots, &pickup_refs).await?;
                }
            }
        }

        Ok(())
    }

    /// Swarm dispatch: all slots can claim from the ready pool freely (original behaviour).
    async fn dispatch_swarm(
        &mut self,
        team: &Team,
        slots: &[TeamAgentSlot],
        pickup_refs: &[&str],
    ) -> crate::error::Result<()> {
        let mut slots_by_role: HashMap<String, Vec<&TeamAgentSlot>> = HashMap::new();
        for slot in slots {
            slots_by_role.entry(slot.role.clone()).or_default().push(slot);
        }

        for (role, role_slots) in &slots_by_role {
            let running_count = self.team_agents.values()
                .filter(|ta| ta.team_id == team.id && ta.role == *role)
                .count();
            let max_for_role = role_slots.len();
            if running_count >= max_for_role {
                continue;
            }

            let ready_issues = {
                let conn = self.db.lock().unwrap();
                Issue::get_ready_by_role(&conn, &team.project_id, role, pickup_refs)?
            };

            let slots_available = max_for_role - running_count;
            let to_spawn = std::cmp::min(slots_available, ready_issues.len());
            for i in 0..to_spawn {
                let issue = &ready_issues[i];
                let slot = role_slots[i % role_slots.len()];
                if let Err(e) = self.spawn_team_agent(team, slot, issue).await {
                    tracing::error!(team = %team.id, role = %role, issue = %issue.id,
                        "Failed to spawn team agent: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Pipeline dispatch: issues are worked sequentially by slot_order.
    /// Find the lowest slot_order that has ANY role with open/in_progress issues,
    /// then dispatch agents for ALL roles at that stage independently.
    async fn dispatch_pipeline(
        &mut self,
        team: &Team,
        slots: &[TeamAgentSlot],
        pickup_refs: &[&str],
    ) -> crate::error::Result<()> {
        // Slots are already sorted by slot_order from list_by_team.
        // Group slots by slot_order, find the lowest order with work.
        let mut current_stage_order: Option<i64> = None;

        // Collect unique slot_orders in ascending order
        let mut seen_orders = std::collections::BTreeSet::new();
        for slot in slots {
            seen_orders.insert(slot.slot_order);
        }

        for &order in &seen_orders {
            // Check if ANY role at this order has open issues
            let order_slots: Vec<&TeamAgentSlot> = slots.iter()
                .filter(|s| s.slot_order == order)
                .collect();

            let has_work = {
                let conn = self.db.lock().unwrap();
                let mut found = false;
                // Check each unique role at this order
                let mut checked_roles = std::collections::HashSet::new();
                for slot in &order_slots {
                    if !checked_roles.insert(slot.role.to_lowercase()) {
                        continue;
                    }
                    let count: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM issues WHERE project_id = ?1 \
                         AND REPLACE(LOWER(role), '_', ' ') = REPLACE(LOWER(?2), '_', ' ') \
                         AND status IN ('open', 'in_progress') AND needs_intake = 0",
                        rusqlite::params![team.project_id, slot.role],
                        |row| row.get(0),
                    ).unwrap_or(0);
                    if count > 0 {
                        found = true;
                        break;
                    }
                }
                found
            };

            if has_work {
                current_stage_order = Some(order);
                break;
            }
        }

        let stage_order = match current_stage_order {
            Some(o) => o,
            None => return Ok(()),
        };

        // Collect all slots at the current stage order
        let stage_slots: Vec<&TeamAgentSlot> = slots.iter()
            .filter(|s| s.slot_order == stage_order)
            .collect();

        // Group stage slots by normalised role name and dispatch each role independently
        let mut role_slots: std::collections::HashMap<String, Vec<&TeamAgentSlot>> = std::collections::HashMap::new();
        for slot in &stage_slots {
            let normalised = slot.role.to_lowercase().replace('_', " ");
            role_slots.entry(normalised).or_default().push(slot);
        }

        for (normalised_role, slots_for_role) in &role_slots {
            // Count running agents for this role
            let running_count = self.team_agents.values()
                .filter(|ta| {
                    ta.team_id == team.id
                        && ta.role.to_lowercase().replace('_', " ") == *normalised_role
                })
                .count();
            let max_for_role = slots_for_role.len();
            if running_count >= max_for_role {
                continue;
            }

            // Use the original role name from the first slot for issue lookup
            let role_name = &slots_for_role[0].role;
            let ready_issues = {
                let conn = self.db.lock().unwrap();
                Issue::get_ready_by_role(&conn, &team.project_id, role_name, pickup_refs)?
            };

            let slots_available = max_for_role - running_count;
            let to_spawn = std::cmp::min(slots_available, ready_issues.len());
            tracing::debug!(
                team = %team.id, role = %role_name, stage_order = stage_order,
                running = running_count, max = max_for_role, ready = ready_issues.len(),
                spawning = to_spawn,
                "Pipeline dispatch for role"
            );
            for i in 0..to_spawn {
                let issue = &ready_issues[i];
                let slot = slots_for_role[i % slots_for_role.len()];
                if let Err(e) = self.spawn_team_agent(team, slot, issue).await {
                    tracing::error!(team = %team.id, role = %role_name, issue = %issue.id,
                        "Failed to spawn pipeline agent: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Collaborative dispatch: multiple agents work on the SAME issue simultaneously.
    /// Picks the first ready issue, then dispatches one agent per idle slot for that issue.
    /// Each agent gets a separate clone issue so they can independently track status.
    async fn dispatch_collaborative(
        &mut self,
        team: &Team,
        slots: &[TeamAgentSlot],
        pickup_refs: &[&str],
    ) -> crate::error::Result<()> {
        // Find slots that don't already have a running agent
        let idle_slots: Vec<&TeamAgentSlot> = slots.iter()
            .filter(|slot| {
                !self.team_agents.values().any(|ta| ta.slot_id == slot.id)
            })
            .collect();

        if idle_slots.is_empty() {
            return Ok(());
        }

        // Pick the first ready issue (any role, since collaborative teams share work)
        let first_ready = {
            let conn = self.db.lock().unwrap();
            let mut found: Option<Issue> = None;
            // Try each slot's role to find a ready issue
            for slot in &idle_slots {
                let ready = Issue::get_ready_by_role(&conn, &team.project_id, &slot.role, pickup_refs)?;
                if let Some(issue) = ready.into_iter().next() {
                    found = Some(issue);
                    break;
                }
            }
            found
        };

        let source_issue = match first_ready {
            Some(issue) => issue,
            None => return Ok(()),
        };

        // Dispatch the first idle slot against the original issue
        let first_slot = idle_slots[0];
        if let Err(e) = self.spawn_team_agent(team, first_slot, &source_issue).await {
            tracing::error!(team = %team.id, issue = %source_issue.id,
                "Failed to spawn collaborative agent: {}", e);
            return Ok(());
        }

        // For remaining idle slots, create clone child issues so each agent has its own tracker
        for slot in &idle_slots[1..] {
            let clone_issue = {
                let conn = self.db.lock().unwrap();
                Issue::create(&conn, &CreateIssue {
                    project_id: team.project_id.clone(),
                    issue_type: Some(source_issue.issue_type.clone()),
                    title: format!("{} [collaborative: {}]", source_issue.title, slot.role),
                    description: Some(source_issue.description.clone()),
                    priority: Some(source_issue.priority),
                    depends_on: None,
                    workflow_instance_id: source_issue.workflow_instance_id.clone(),
                    stage_id: source_issue.stage_id.clone(),
                    role: Some(slot.role.clone()),
                    parent_id: Some(source_issue.id.clone()),
                    needs_intake: Some(0),
                    scope_mode: Some(source_issue.scope_mode.clone()),
                })?
            };
            if let Err(e) = self.spawn_team_agent(team, slot, &clone_issue).await {
                tracing::error!(team = %team.id, issue = %clone_issue.id,
                    "Failed to spawn collaborative agent: {}", e);
            }
        }
        Ok(())
    }

    /// Hierarchical dispatch: lead slots get top-level issues (parent_id IS NULL),
    /// non-lead slots only get child issues (parent_id IS NOT NULL).
    /// The lead agent decomposes top-level issues into children via the intake mechanism.
    async fn dispatch_hierarchical(
        &mut self,
        team: &Team,
        slots: &[TeamAgentSlot],
        _pickup_refs: &[&str],
    ) -> crate::error::Result<()> {
        let lead_slots: Vec<&TeamAgentSlot> = slots.iter().filter(|s| s.is_lead).collect();
        let worker_slots: Vec<&TeamAgentSlot> = slots.iter().filter(|s| !s.is_lead).collect();

        // Dispatch lead slots against top-level issues (parent_id IS NULL)
        for slot in &lead_slots {
            let already_running = self.team_agents.values()
                .any(|ta| ta.slot_id == slot.id);
            if already_running {
                continue;
            }

            let top_level_issue = {
                let conn = self.db.lock().unwrap();
                let mut stmt = conn.prepare(
                    "SELECT * FROM issues WHERE project_id = ?1 \
                     AND REPLACE(LOWER(role), '_', ' ') = REPLACE(LOWER(?2), '_', ' ') \
                     AND status = 'open' AND claimed_by IS NULL \
                     AND parent_id IS NULL AND needs_intake = 0 \
                     ORDER BY priority, created_at LIMIT 1"
                )?;
                let mut rows = stmt.query_map(
                    rusqlite::params![team.project_id, slot.role],
                    Issue::from_row,
                )?;
                match rows.next() {
                    Some(Ok(issue)) => Some(issue),
                    _ => None,
                }
            };

            if let Some(issue) = top_level_issue {
                if let Err(e) = self.spawn_team_agent(team, slot, &issue).await {
                    tracing::error!(team = %team.id, role = %slot.role, issue = %issue.id,
                        "Failed to spawn hierarchical lead agent: {}", e);
                }
            }
        }

        // Dispatch worker slots against child issues (parent_id IS NOT NULL)
        for slot in &worker_slots {
            let already_running = self.team_agents.values()
                .any(|ta| ta.slot_id == slot.id);
            if already_running {
                continue;
            }

            let child_issue = {
                let conn = self.db.lock().unwrap();
                let mut stmt = conn.prepare(
                    "SELECT * FROM issues WHERE project_id = ?1 \
                     AND REPLACE(LOWER(role), '_', ' ') = REPLACE(LOWER(?2), '_', ' ') \
                     AND status = 'open' AND claimed_by IS NULL \
                     AND parent_id IS NOT NULL AND needs_intake = 0 \
                     ORDER BY priority, created_at LIMIT 1"
                )?;
                let mut rows = stmt.query_map(
                    rusqlite::params![team.project_id, slot.role],
                    Issue::from_row,
                )?;
                match rows.next() {
                    Some(Ok(issue)) => Some(issue),
                    _ => None,
                }
            };

            if let Some(issue) = child_issue {
                if let Err(e) = self.spawn_team_agent(team, slot, &issue).await {
                    tracing::error!(team = %team.id, role = %slot.role, issue = %issue.id,
                        "Failed to spawn hierarchical worker agent: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn spawn_team_agent(
        &mut self,
        team: &Team,
        slot: &TeamAgentSlot,
        issue: &Issue,
    ) -> crate::error::Result<()> {
        // Look up project directory and name
        let (project_dir, project_name) = {
            let conn = self.db.lock().unwrap();
            let project = Project::get_by_id(&conn, &team.project_id)?;
            (project.directory, project.name)
        };

        // Create git worktree for isolation
        let task_hash = &issue.id[..8];
        let default_branch = WorktreeManager::detect_default_branch(std::path::Path::new(&project_dir))
            .unwrap_or_else(|| "main".to_string());
        // Sanitize role for use in git branch names (no spaces or special chars)
        let safe_role = slot.role.to_lowercase().replace(' ', "-").replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "");
        let (worktree_path, branch_name) = match self.worktree_manager.create_worktree(
            std::path::Path::new(&project_dir),
            &safe_role,
            task_hash,
            &default_branch,
        ) {
            Ok((path, branch)) => (Some(path), Some(branch)),
            Err(e) => {
                tracing::warn!(
                    issue = %issue.id,
                    "Failed to create worktree, falling back to main dir: {}", e
                );
                (None, None)
            }
        };

        let working_dir = worktree_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| project_dir.clone());

        // Read project context for the agent prompt
        let working_path = std::path::Path::new(&working_dir);
        let claude_md = crate::orchestrator::context::read_claude_md(working_path)
            .unwrap_or_default();
        let file_tree = crate::orchestrator::context::generate_file_tree(working_path);

        // Check for accepted routing overrides that apply to this slot's model
        let (effective_model, effective_runtime) = {
            let conn = self.db.lock().unwrap();
            let overrides = crate::models::routing_override::RoutingOverride::get_accepted_for_role(
                &conn, &team.project_id, &slot.role, &issue.issue_type,
            ).unwrap_or_default();

            // Check if any accepted override targets the current slot's model
            let matching_override = overrides.iter().find(|o| {
                o.from_model.as_deref() == slot.model.as_deref()
            });

            if let Some(ov) = matching_override {
                // Look up the role's default model/runtime at the higher tier
                let role_defaults = crate::models::role::Role::get_by_name(&conn, &slot.role).ok();
                let new_model = role_defaults.as_ref().and_then(|r| r.default_model.clone());
                let new_runtime = role_defaults.as_ref().map(|r| r.default_runtime.clone())
                    .unwrap_or_else(|| slot.runtime.clone());

                tracing::info!(
                    role = %slot.role,
                    from_model = ?slot.model,
                    to_model = ?new_model,
                    to_runtime = %new_runtime,
                    to_tier = ov.to_tier,
                    override_id = %ov.id,
                    "Applying accepted routing override — escalating model"
                );
                (new_model, new_runtime)
            } else {
                (slot.model.clone(), slot.runtime.clone())
            }
        };

        // Create AgentSession record
        let session = {
            let conn = self.db.lock().unwrap();
            AgentSession::create(
                &conn,
                &CreateAgentSession {
                    team_id: team.id.clone(),
                    slot_id: slot.id.clone(),
                    runtime: effective_runtime.clone(),
                    model: effective_model.clone(),
                    workflow_instance_id: None,
                    pid: None,
                    worktree_path: worktree_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    branch: branch_name.clone(),
                },
            )?
        };

        // Claim the issue and link session to task
        {
            let conn = self.db.lock().unwrap();
            Issue::claim(&conn, &issue.id, &session.id)?;
            conn.execute(
                "UPDATE agent_sessions SET claimed_task_id = ?1, state = 'working' WHERE id = ?2",
                rusqlite::params![issue.id, session.id],
            )?;
        }
        self.log_activity("issue_claimed", &format!("Issue '{}' claimed", issue.title), Some(&team.project_id), Some(&team.id), Some(&session.id), Some(&issue.id), None);

        // Build prompt with role template, task details, and curl instructions
        let description = &issue.description;
        let role_prompt = {
            let conn = self.db.lock().unwrap();
            crate::models::prompt_template::PromptTemplate::build_prompt_for_role(
                &conn, &slot.role, Some(&team.project_id)
            ).unwrap_or_default()
        };

        let mut prompt_parts = Vec::new();
        if !role_prompt.is_empty() {
            prompt_parts.push(role_prompt);
            prompt_parts.push(format!("You are working on project {}.", project_name));
        } else {
            prompt_parts.push(format!("You are a {} agent working on project {}.", slot.role, project_name));
        }

        if !claude_md.is_empty() {
            prompt_parts.push(format!("\n## Project Guidelines\n{}", claude_md));
        }

        if !file_tree.is_empty() {
            prompt_parts.push(format!("\n## Project Structure\n```\n{}\n```", file_tree));
        }

        prompt_parts.push(format!(
            "\n## Your Task\n**{}**\n\n{}\n\n\
            When you have completed your work, close your issue by running:\n\
            curl -sk -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"status\": \"closed\", \"summary\": \"Brief description of what you accomplished\"}}'\n\n\
            You can also post progress updates at any time:\n\
            curl -sk -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"summary\": \"Current progress update\"}}'",
            issue.title,
            description,
            team.project_id, issue.id,
            team.project_id, issue.id,
        ));

        // Loom progress reporting instructions
        prompt_parts.push(format!(
            "\n## Progress Reporting\n\
            Post status updates as you work so the team dashboard shows your progress.\n\
            Use these curl commands at natural transition points (3-5 updates per task is ideal):\n\n\
            When starting a phase of work:\n\
            curl -sk -X POST ${{IRONWEAVE_API}}/api/loom \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"team_id\": \"{team_id}\", \"project_id\": \"{project_id}\", \"agent_id\": \"{agent_id}\", \
            \"entry_type\": \"status\", \"content\": \"Starting: <what you are about to do>\"}}'\n\n\
            When blocked or hitting an issue, use entry_type \"warning\".\n\
            When you discover something notable, use entry_type \"finding\".\n\
            When done (before closing the issue), use entry_type \"completion\".\n\n\
            Only change the entry_type and content fields — keep team_id, project_id, and agent_id as shown.",
            team_id = team.id,
            project_id = team.project_id,
            agent_id = session.id,
        ));

        // Build restrictions — compilation happens on the build server via merge queue
        prompt_parts.push(
            "\n## Build Rules\n\
            IMPORTANT: Do NOT run `cargo build`, `cargo check`, or `cargo test` yourself.\n\
            Build verification is handled automatically by the merge queue on a dedicated build server.\n\
            Your job is to edit code and commit your changes. When you close the issue, \
            the merge queue will merge your branch and verify it compiles.\n\
            If you need to check syntax, read the code carefully instead of compiling."
            .to_string()
        );

        let prompt = prompt_parts.join("\n");

        // Build AgentConfig
        let config = AgentConfig {
            working_directory: std::path::PathBuf::from(&working_dir),
            prompt,
            environment: {
                let api_url = {
                    let conn = self.db.lock().unwrap();
                    crate::models::setting::Setting::get_by_key(&conn, "api_url")
                        .map(|s| s.value)
                        .unwrap_or_else(|_| "https://localhost:443".to_string())
                };
                let mut env = std::collections::HashMap::new();
                env.insert("IRONWEAVE_API".to_string(), api_url);
                env.insert("TERM".to_string(), "xterm-256color".to_string());
                // Inject API keys from settings for non-Claude runtimes
                {
                    let conn = self.db.lock().unwrap();
                    let key_mappings = [
                        ("gemini_api_key", "GEMINI_API_KEY"),
                        ("google_api_key", "GOOGLE_API_KEY"),
                        ("apikey_openrouter_api_key", "OPENROUTER_API_KEY"),
                        ("anthropic_api_key", "ANTHROPIC_API_KEY"),
                        ("ollama_host", "OLLAMA_HOST"),
                    ];
                    for (setting_key, env_var) in &key_mappings {
                        if let Ok(s) = crate::models::setting::Setting::get_by_key(&conn, setting_key) {
                            if !s.value.is_empty() {
                                env.insert(env_var.to_string(), s.value);
                            }
                        }
                    }
                }
                // Ensure PATH includes runtime binary locations
                let path = std::env::var("PATH").unwrap_or_default();
                env.insert("PATH".to_string(), format!(
                    "/home/paddy/.opencode/bin:/home/paddy/.npm-global/bin:/home/paddy/.cargo/bin:{}",
                    path
                ));
                Some(env)
            },
            allowed_tools: None,
            skills: None,
            extra_args: Some(vec!["--print".to_string()]),
            playwright_env: None,
            model: effective_model.clone(),
        };

        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        // Spawn PTY agent via process_manager
        if let Err(e) = self.process_manager
            .spawn_agent(&session.id, &effective_runtime, config, size)
            .await
        {
            // Clean up claim and session on spawn failure
            let conn = self.db.lock().unwrap();
            let _ = Issue::unclaim(&conn, &issue.id);
            let _ = AgentSession::update_state(&conn, &session.id, "failed");
            return Err(e);
        }

        // Track in team_agents HashMap
        self.log_activity("agent_spawned", &format!("Agent spawned for role '{}'", slot.role), Some(&team.project_id), Some(&team.id), Some(&session.id), Some(&issue.id), None);
        tracing::info!(
            team = %team.id, role = %slot.role, issue = %issue.id,
            session = %session.id, "Spawned team agent"
        );

        self.team_agents.insert(
            session.id.clone(),
            TeamDispatchedAgent {
                agent_session_id: session.id,
                team_id: team.id.clone(),
                slot_id: slot.id.clone(),
                issue_id: issue.id.clone(),
                role: slot.role.clone(),
                spawned_at: Instant::now(),
                last_activity: Instant::now(),
                nudge_count: 0,
                last_issue_updated_at: String::new(),
            },
        );

        Ok(())
    }

    // ── sweep_intake (Task 5 – intake agent lifecycle) ───────────

    async fn sweep_intake(&mut self) -> crate::error::Result<()> {
        // 1. Check existing intake agents for completion
        let project_ids: Vec<String> = self.intake_agents.keys().cloned().collect();
        let mut to_remove = Vec::new();

        for project_id in &project_ids {
            let (session_id, issue_id) = self.intake_agents.get(project_id).unwrap().clone();

            let exit_status = self.process_manager.check_agent_exit(&session_id).await;

            if let Some(success) = exit_status {
                self.process_manager.remove_agent(&session_id).await;
                if success {
                    tracing::info!(
                        project = %project_id, issue = %issue_id,
                        "Intake agent completed successfully"
                    );
                } else {
                    let conn = self.db.lock().unwrap();
                    let _ = Issue::update(&conn, &issue_id, &crate::models::issue::UpdateIssue {
                        status: Some("open".to_string()),
                        ..Default::default()
                    });
                    tracing::warn!(
                        project = %project_id, issue = %issue_id,
                        "Intake agent crashed — issue available for retry"
                    );
                }
                to_remove.push(project_id.clone());
                continue;
            }

            let agent_exists = self.process_manager.get_agent(&session_id).await.is_some();
            if !agent_exists {
                // Reset issue to open so it can be retried
                {
                    let conn = self.db.lock().unwrap();
                    let _ = Issue::update(&conn, &issue_id, &crate::models::issue::UpdateIssue {
                        status: Some("open".to_string()),
                        ..Default::default()
                    });
                }
                to_remove.push(project_id.clone());
                tracing::warn!(
                    project = %project_id, issue = %issue_id,
                    "Intake agent PTY disappeared — issue reset to open"
                );
            }
        }

        for key in &to_remove {
            self.intake_agents.remove(key);
        }

        // 2. Find projects with issues needing intake
        let active_teams = {
            let conn = self.db.lock().unwrap();
            Team::list_active(&conn)?
        };

        // Deduplicate project IDs
        let mut unique_projects = std::collections::HashSet::new();
        for team in &active_teams {
            unique_projects.insert(team.project_id.clone());
        }

        for project_id in &unique_projects {
            if self.intake_agents.contains_key(project_id) {
                continue;
            }

            let needs_intake = {
                let conn = self.db.lock().unwrap();
                Issue::get_needs_intake(&conn, project_id)?
            };

            if let Some(issue) = needs_intake.first() {
                if let Err(e) = self.spawn_intake_agent(project_id, issue).await {
                    tracing::error!(
                        project = %project_id, issue = %issue.id,
                        "Failed to spawn intake agent: {}", e
                    );
                }
            }
        }

        Ok(())
    }

    // ── spawn_intake_agent (Task 5 – intake agent spawning) ──────

    async fn spawn_intake_agent(
        &mut self,
        project_id: &str,
        issue: &Issue,
    ) -> crate::error::Result<()> {
        let (project_dir, project_name) = {
            let conn = self.db.lock().unwrap();
            let project = Project::get_by_id(&conn, project_id)?;
            (project.directory, project.name)
        };

        // Get available roles from team slots
        let available_roles = {
            let conn = self.db.lock().unwrap();
            let teams = Team::list_active(&conn)?;
            let mut roles = Vec::new();
            for team in &teams {
                if team.project_id == project_id {
                    let slots = TeamAgentSlot::list_by_team(&conn, &team.id)?;
                    for slot in slots {
                        if !roles.contains(&slot.role) {
                            roles.push(slot.role.clone());
                        }
                    }
                }
            }
            roles
        };

        // Get recent git log
        let git_log = tokio::process::Command::new("git")
            .args(["log", "--oneline", "-20"])
            .current_dir(&project_dir)
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        // Get file tree (top 2 levels)
        let file_tree = tokio::process::Command::new("find")
            .args([".", "-maxdepth", "2", "-not", "-path", "./.git/*", "-not", "-path", "./target/*", "-not", "-path", "./node_modules/*"])
            .current_dir(&project_dir)
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let api_url = {
            let conn = self.db.lock().unwrap();
            crate::models::setting::Setting::get_by_key(&conn, "api_url")
                .map(|s| s.value)
                .unwrap_or_else(|_| "https://localhost:443".to_string())
        };

        let scope_mode = &issue.scope_mode;
        let scope_instructions = if scope_mode == "conversational" {
            "This ticket needs scoping. Ask clarifying questions by updating the parent's \
             summary field with your questions, set status to 'awaiting_input', then exit. \
             The user will update the description with answers and intake will re-trigger."
        } else {
            "Analyse and decompose automatically. No user interaction needed."
        };

        let prompt = format!(
            "You are an Intake Agent for project {project_name}.\n\n\
            ## Your Job\n\
            Analyse the submitted ticket below and break it into actionable subtasks \
            that agents can pick up.\n\n\
            ## Ticket\n\
            **Title:** {title}\n\
            **Type:** {issue_type}\n\
            **Description:**\n\
            {description}\n\n\
            ## Scope Mode: {scope_mode}\n\
            {scope_instructions}\n\n\
            ## Project Context\n\
            **Available roles:** {roles}\n\
            **Recent git log:**\n```\n{git_log}\n```\n\
            **File tree:**\n```\n{file_tree}\n```\n\n\
            ## Instructions\n\n\
            1. Read relevant source files to understand the codebase\n\
            2. Analyse the ticket — determine type (bug fix, tweak, feature, performance)\n\
            3. Break into subtasks with clear title, description with acceptance criteria, \
               a role, and dependencies\n\
            4. Create child issues via the API\n\
            5. Update the parent issue when done\n\n\
            ## API Commands\n\n\
            **Create a child issue:**\n\
            ```bash\n\
            curl -X POST ${{IRONWEAVE_API}}/api/projects/{project_id}/issues \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"title\": \"Task title\", \"description\": \"Detailed description\", \
            \"issue_type\": \"task\", \"role\": \"senior_coder\", \
            \"parent_id\": \"{parent_id}\", \"needs_intake\": 0, \
            \"depends_on\": [\"dep-id\"]}}'\n\
            ```\n\n\
            **Update parent issue (do this LAST):**\n\
            ```bash\n\
            curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{project_id}/issues/{parent_id} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"needs_intake\": 0, \"summary\": \"Decomposed into N subtasks\"}}'\n\
            ```\n\n\
            ## Guidelines\n\
            - Bug reports: 1-3 tasks\n\
            - Tweaks: 1-2 tasks\n\
            - Features: 4-10 tasks with dependency chains\n\
            - Performance: 2-4 tasks\n\
            - Simple changes: set needs_intake=0 on parent directly and leave for pickup\n\
            - Always set needs_intake=0 on children\n\
            - Use depends_on for execution waves",
            project_name = project_name,
            title = issue.title,
            issue_type = issue.issue_type,
            description = issue.description,
            scope_mode = scope_mode,
            scope_instructions = scope_instructions,
            roles = available_roles.join(", "),
            git_log = git_log.trim(),
            file_tree = file_tree.trim(),
            project_id = project_id,
            parent_id = issue.id,
        );

        // Append attachment info to prompt (fetch list with short lock, then read files without lock)
        let attachments = {
            let conn = self.db.lock().unwrap();
            crate::models::attachment::Attachment::list_by_issue(&conn, &issue.id)
                .unwrap_or_default()
        };

        let attachments_section = if attachments.is_empty() {
            String::new()
        } else {
            let mut section = String::from("\n\n## Attached Files\n\n");
            for att in &attachments {
                let is_text = matches!(
                    att.mime_type.as_str(),
                    "text/plain" | "text/markdown" | "text/csv"
                        | "application/json" | "application/xml"
                        | "text/x-log" | "text/x-rust" | "text/x-python"
                ) || att.filename.ends_with(".log")
                  || att.filename.ends_with(".txt")
                  || att.filename.ends_with(".md")
                  || att.filename.ends_with(".json")
                  || att.filename.ends_with(".csv");

                if is_text && att.size_bytes <= 50_000 {
                    if let Ok(content) = std::fs::read_to_string(&att.stored_path) {
                        section.push_str(&format!(
                            "### {} ({}, {} bytes)\n```\n{}\n```\n\n",
                            att.filename, att.mime_type, att.size_bytes, content
                        ));
                    } else {
                        section.push_str(&format!(
                            "- {} ({}, {} bytes) — file could not be read\n",
                            att.filename, att.mime_type, att.size_bytes
                        ));
                    }
                } else {
                    section.push_str(&format!(
                        "- {} ({}, {} bytes) — binary file, available at /api/attachments/{}/download\n",
                        att.filename, att.mime_type, att.size_bytes, att.id
                    ));
                }
            }
            section
        };

        let prompt = format!("{}{}", prompt, attachments_section);

        let session_id = uuid::Uuid::new_v4().to_string();

        let config = AgentConfig {
            working_directory: std::path::PathBuf::from(&project_dir),
            prompt,
            environment: {
                let mut env = std::collections::HashMap::new();
                env.insert("IRONWEAVE_API".to_string(), api_url);
                env.insert("TERM".to_string(), "xterm-256color".to_string());
                // Inject API keys from settings for non-Claude runtimes
                {
                    let conn = self.db.lock().unwrap();
                    let key_mappings = [
                        ("gemini_api_key", "GEMINI_API_KEY"),
                        ("google_api_key", "GOOGLE_API_KEY"),
                        ("apikey_openrouter_api_key", "OPENROUTER_API_KEY"),
                        ("anthropic_api_key", "ANTHROPIC_API_KEY"),
                        ("ollama_host", "OLLAMA_HOST"),
                    ];
                    for (setting_key, env_var) in &key_mappings {
                        if let Ok(s) = crate::models::setting::Setting::get_by_key(&conn, setting_key) {
                            if !s.value.is_empty() {
                                env.insert(env_var.to_string(), s.value);
                            }
                        }
                    }
                }
                // Ensure PATH includes runtime binary locations
                let path = std::env::var("PATH").unwrap_or_default();
                env.insert("PATH".to_string(), format!(
                    "/home/paddy/.opencode/bin:/home/paddy/.npm-global/bin:/home/paddy/.cargo/bin:{}",
                    path
                ));
                Some(env)
            },
            allowed_tools: None,
            skills: None,
            extra_args: Some(vec!["--print".to_string()]),
            playwright_env: None,
            model: Some("sonnet".to_string()),
        };

        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        // Mark issue as in_progress
        {
            let conn = self.db.lock().unwrap();
            Issue::update(&conn, &issue.id, &crate::models::issue::UpdateIssue {
                status: Some("in_progress".to_string()),
                ..Default::default()
            })?;
        }

        self.process_manager
            .spawn_agent(&session_id, "claude", config, size)
            .await?;

        tracing::info!(
            project = %project_id, issue = %issue.id,
            "Spawned intake agent"
        );

        self.intake_agents.insert(
            project_id.to_string(),
            (session_id, issue.id.clone()),
        );

        Ok(())
    }

    // ── sweep_parent_autoclose ─────────────────────────────────────

    /// Check if any parent issues should be auto-closed (all children closed).
    async fn sweep_parent_autoclose(&mut self) -> crate::error::Result<()> {
        // Collect parent IDs in a short-lived lock scope
        let parent_ids: Vec<String> = {
            let conn = self.db.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT DISTINCT parent_id FROM issues WHERE parent_id IS NOT NULL"
            )?;
            stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect()
        };

        for parent_id in &parent_ids {
            // Acquire lock per parent to avoid holding it across the entire loop
            let conn = self.db.lock().unwrap();

            let parent = match Issue::get_by_id(&conn, parent_id) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Skip already closed parents
            if parent.status == "closed" {
                continue;
            }

            // Check if all children are closed
            if let Ok(Some(true)) = Issue::all_children_closed(&conn, parent_id) {
                // Aggregate child summaries
                let children = Issue::get_children(&conn, parent_id)?;
                let child_summaries: Vec<String> = children
                    .iter()
                    .map(|c| {
                        let summary = c.summary.as_deref().unwrap_or("completed");
                        format!("- {}: {}", c.title, summary)
                    })
                    .collect();

                let summary = format!(
                    "All {} subtasks completed:\n{}",
                    children.len(),
                    child_summaries.join("\n")
                );

                Issue::update(&conn, parent_id, &crate::models::issue::UpdateIssue {
                    status: Some("closed".to_string()),
                    summary: Some(summary),
                    ..Default::default()
                })?;

                tracing::info!(parent = %parent_id, "Auto-closed parent issue — all children done");
            }
        }

        Ok(())
    }

    // ── sweep_merge_queue (merge queue processing) ─────────────────

    async fn sweep_merge_queue(&mut self) -> crate::error::Result<()> {
        use crate::models::merge_queue_entry::MergeQueueEntry;
        use crate::worktree::merge_queue::{MergeQueueProcessor, MergeResult};

        // Step 1: Find distinct project_ids with pending entries (lock briefly)
        let project_ids: Vec<String> = {
            let conn = self.db.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT DISTINCT project_id FROM merge_queue WHERE status = 'pending'"
            )?;
            stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect()
        };

        for project_id in &project_ids {
            // Step 2: Get next pending entry and project directory (lock briefly)
            let (entry, project_dir) = {
                let conn = self.db.lock().unwrap();
                let entry = match MergeQueueEntry::next_pending(&conn, project_id) {
                    Ok(e) => e,
                    Err(_) => continue, // No pending entries (race condition)
                };
                let project_dir = match Project::get_by_id(&conn, project_id) {
                    Ok(p) => p.directory,
                    Err(_) => continue,
                };
                (entry, project_dir)
            };
            // DB lock is dropped here before file I/O

            if project_dir.is_empty() {
                continue;
            }

            let repo_path = std::path::Path::new(&project_dir);

            // Step 4: Detect the default branch (target)
            let target_branch = WorktreeManager::detect_default_branch(repo_path)
                .unwrap_or_else(|| "main".to_string());

            // Step 5: Attempt the merge (file I/O — no DB lock held)
            let result = MergeQueueProcessor::try_merge(
                repo_path,
                &entry.branch_name,
                &target_branch,
            );

            // Step 6: Handle result (re-acquire DB lock)
            match result {
                Ok(MergeResult::Success) => {
                    // Build verification: if a build server is configured, verify
                    // the merged code compiles before accepting the merge.
                    let build_passed = if let Some(ref bs) = self.build_server {
                        let source_dir = bs.local_source_dir.as_deref()
                            .unwrap_or(&project_dir);
                        let source_path = std::path::Path::new(source_dir);

                        {
                            let conn = self.db.lock().unwrap();
                            let _ = MergeQueueEntry::update_status(
                                &conn, &entry.id, "verifying", None, None, None,
                            );
                        }
                        self.log_activity(
                            "build_verify_start",
                            &format!("Verifying build for branch '{}' on build server", entry.branch_name),
                            Some(project_id), entry.team_id.as_deref(),
                            entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                            None,
                        );

                        match MergeQueueProcessor::verify_build(
                            source_path,
                            &bs.ssh_target,
                            &bs.remote_source_dir,
                        ) {
                            crate::worktree::merge_queue::BuildVerifyResult::Pass => true,
                            crate::worktree::merge_queue::BuildVerifyResult::Fail(err) => {
                                tracing::error!(
                                    branch = %entry.branch_name,
                                    "Build verification failed, reverting merge: {}", err
                                );
                                // Revert the merge
                                if let Err(revert_err) = MergeQueueProcessor::revert_merge(
                                    repo_path, &target_branch,
                                ) {
                                    tracing::error!("Failed to revert merge: {}", revert_err);
                                }
                                // Mark as build_failed
                                {
                                    let conn = self.db.lock().unwrap();
                                    let _ = MergeQueueEntry::update_status(
                                        &conn, &entry.id, "build_failed", None, None, Some(&err),
                                    );
                                }
                                self.log_activity(
                                    "build_verify_failed",
                                    &format!("Build failed for branch '{}': {}", entry.branch_name, &err[..err.len().min(200)]),
                                    Some(project_id), entry.team_id.as_deref(),
                                    entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                                    None,
                                );
                                false
                            }
                        }
                    } else {
                        true // No build server configured — skip verification
                    };

                    if build_passed {
                        // Code review gate: run Claude Sonnet review on the diff
                        {
                            let conn = self.db.lock().unwrap();
                            let _ = MergeQueueEntry::update_status(
                                &conn, &entry.id, "reviewing", None, None, None,
                            );
                        }
                        self.log_activity(
                            "code_review_start",
                            &format!("Starting code review for branch '{}'", entry.branch_name),
                            Some(project_id), entry.team_id.as_deref(),
                            entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                            None,
                        );

                        let review_passed = match MergeQueueProcessor::review_code(
                            repo_path,
                            &entry.branch_name,
                        ) {
                            crate::worktree::merge_queue::CodeReviewResult::Pass => {
                                tracing::info!(branch = %entry.branch_name, "Code review passed");
                                true
                            }
                            crate::worktree::merge_queue::CodeReviewResult::Fail(feedback) => {
                                tracing::warn!(
                                    branch = %entry.branch_name,
                                    "Code review rejected: {}", feedback
                                );
                                // Revert the merge
                                if let Err(revert_err) = MergeQueueProcessor::revert_merge(
                                    repo_path, &target_branch,
                                ) {
                                    tracing::error!("Failed to revert merge after review rejection: {}", revert_err);
                                }
                                {
                                    let conn = self.db.lock().unwrap();
                                    let _ = MergeQueueEntry::update_status(
                                        &conn, &entry.id, "review_failed", None, None,
                                        Some(&feedback[..feedback.len().min(1000)]),
                                    );
                                }
                                self.log_activity(
                                    "code_review_failed",
                                    &format!("Code review rejected for branch '{}': {}", entry.branch_name, &feedback[..feedback.len().min(200)]),
                                    Some(project_id), entry.team_id.as_deref(),
                                    entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                                    None,
                                );
                                false
                            }
                            crate::worktree::merge_queue::CodeReviewResult::Error(err) => {
                                tracing::error!(
                                    branch = %entry.branch_name,
                                    "Code review error (proceeding with merge): {}", err
                                );
                                self.log_activity(
                                    "code_review_error",
                                    &format!("Code review error for branch '{}': {} — proceeding with merge", entry.branch_name, &err[..err.len().min(200)]),
                                    Some(project_id), entry.team_id.as_deref(),
                                    entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                                    None,
                                );
                                true // Don't block merges if the review tool itself fails
                            }
                        };

                        if review_passed {
                            {
                                let conn = self.db.lock().unwrap();
                                let _ = MergeQueueEntry::update_status(
                                    &conn, &entry.id, "merged", None, None, None,
                                );
                            }
                            self.log_activity(
                                "merge_success",
                                &format!("Merged branch '{}' into '{}'", entry.branch_name, target_branch),
                                Some(project_id), entry.team_id.as_deref(),
                                entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                                None,
                            );
                            let _ = self.worktree_manager.remove_worktree(
                                repo_path,
                                &entry.branch_name.replace('/', "-"),
                            );
                            tracing::info!(branch = %entry.branch_name, "Merge successful");
                        }
                    }
                }
                Ok(MergeResult::Conflict { files }) => {
                    let conflict_json = serde_json::to_string(&files).unwrap_or_else(|_| "[]".to_string());
                    let file_list = files.iter()
                        .map(|f| format!("- {}", f))
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Create resolver issue and attempt auto-spawn
                    let resolver_issue = {
                        let conn = self.db.lock().unwrap();
                        let _ = MergeQueueEntry::update_status(
                            &conn, &entry.id, "conflicted", Some(&conflict_json), None, None,
                        );
                        Issue::create(&conn, &CreateIssue {
                            project_id: project_id.clone(),
                            issue_type: Some("merge_conflict".to_string()),
                            title: format!("Resolve merge conflicts: {}", entry.branch_name),
                            description: Some(format!(
                                "Resolve merge conflicts in branch '{}'. Conflicting files:\n{}",
                                entry.branch_name, file_list,
                            )),
                            priority: Some(10),
                            depends_on: None,
                            workflow_instance_id: None,
                            stage_id: None,
                            role: Some("senior_coder".to_string()),
                            parent_id: None,
                            needs_intake: Some(0),
                            scope_mode: None,
                        }).ok()
                    };

                    // Auto-spawn resolver agent (T1)
                    if let Some(ref resolver_issue) = resolver_issue {
                        if let Some(ref team_id) = entry.team_id {
                            let spawn_result = {
                                let team_and_slot = {
                                    let conn = self.db.lock().unwrap();
                                    Team::get_by_id(&conn, team_id).ok().and_then(|team| {
                                        let slots = TeamAgentSlot::list_by_team(&conn, team_id).ok()?;
                                        // Find a coder slot to use as resolver
                                        let slot = slots.into_iter()
                                            .find(|s| s.role.contains("coder"))
                                            .or_else(|| {
                                                let slots = TeamAgentSlot::list_by_team(&conn, team_id).ok()?;
                                                slots.into_iter().next()
                                            });
                                        slot.map(|s| (team, s))
                                    })
                                };
                                if let Some((team, slot)) = team_and_slot {
                                    self.spawn_team_agent(&team, &slot, resolver_issue).await
                                } else {
                                    Err(crate::error::IronweaveError::Internal("No team/slot found for resolver".into()))
                                }
                            };

                            match spawn_result {
                                Ok(()) => {
                                    // Find the agent session that was just spawned for this issue
                                    let resolver_session_id = self.team_agents.values()
                                        .find(|ta| ta.issue_id == resolver_issue.id)
                                        .map(|ta| ta.agent_session_id.clone());
                                    if let Some(ref sid) = resolver_session_id {
                                        let conn = self.db.lock().unwrap();
                                        let _ = MergeQueueEntry::update_status(
                                            &conn, &entry.id, "resolving", None, Some(sid), None,
                                        );
                                    }
                                    self.log_activity(
                                        "resolver_spawned",
                                        &format!("Auto-spawned resolver agent for branch '{}'", entry.branch_name),
                                        Some(project_id), entry.team_id.as_deref(),
                                        resolver_session_id.as_deref(), entry.issue_id.as_deref(),
                                        None,
                                    );
                                    tracing::info!(branch = %entry.branch_name, "Resolver agent spawned");
                                }
                                Err(e) => {
                                    tracing::error!(branch = %entry.branch_name, "Failed to spawn resolver: {}", e);
                                    self.log_activity(
                                        "resolver_spawn_failed",
                                        &format!("Failed to auto-spawn resolver for '{}': {}", entry.branch_name, e),
                                        Some(project_id), entry.team_id.as_deref(),
                                        entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                                        None,
                                    );
                                }
                            }
                        }
                    }

                    self.log_activity(
                        "merge_conflict",
                        &format!("Merge conflict on branch '{}': {} file(s)", entry.branch_name, files.len()),
                        Some(project_id), entry.team_id.as_deref(),
                        entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                        None,
                    );
                    tracing::warn!(branch = %entry.branch_name, conflicts = ?files, "Merge conflict");
                }
                Ok(MergeResult::Error(msg)) => {
                    {
                        let conn = self.db.lock().unwrap();
                        let _ = MergeQueueEntry::update_status(
                            &conn, &entry.id, "failed", None, None, Some(&msg),
                        );
                    }
                    self.log_activity(
                        "merge_failed",
                        &format!("Merge failed for branch '{}': {}", entry.branch_name, msg),
                        Some(project_id), entry.team_id.as_deref(),
                        entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                        None,
                    );
                    tracing::error!(branch = %entry.branch_name, "Merge failed: {}", msg);
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    {
                        let conn = self.db.lock().unwrap();
                        let _ = MergeQueueEntry::update_status(
                            &conn, &entry.id, "failed", None, None, Some(&err_msg),
                        );
                    }
                    self.log_activity(
                        "merge_failed",
                        &format!("Merge error for branch '{}': {}", entry.branch_name, err_msg),
                        Some(project_id), entry.team_id.as_deref(),
                        entry.agent_session_id.as_deref(), entry.issue_id.as_deref(),
                        None,
                    );
                    tracing::error!(branch = %entry.branch_name, "Merge error: {}", e);
                }
            }
        }

        Ok(())
    }
}
