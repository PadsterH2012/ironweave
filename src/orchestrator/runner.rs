use std::collections::HashMap;
use std::sync::Arc;

use portable_pty::PtySize;
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
    }

    // ── Startup recovery (Task 13) ──────────────────────────────────

    pub async fn restore_running_instances(&mut self) {
        // Reset orphaned in_progress issues (from agents killed by restart)
        {
            let conn = self.db.lock().unwrap();
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

        // Create issue (bead) for this stage
        let issue = {
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
            curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"status\": \"closed\", \"summary\": \"Brief description of what you accomplished\"}}'\n\n\
            You can also post progress updates at any time:\n\
            curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
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
                Some(env)
            },
            allowed_tools: None,
            skills: None,
            extra_args: Some(vec!["--print".to_string()]),
            playwright_env: None,
            model: stage.model.clone(),
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
        run_state.stage_agents.insert(
            stage_id.to_string(),
            StageAgent {
                agent_session_id: session_id,
                issue_id: issue.id,
                spawned_at: Instant::now(),
                last_activity: Instant::now(),
                nudge_count: 0,
                last_issue_updated_at: String::new(),
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

        // Team dispatch sweep
        if let Err(e) = self.sweep_teams().await {
            tracing::error!("Team sweep error: {}", e);
        }

        // Parent auto-close sweep
        if let Err(e) = self.sweep_parent_autoclose().await {
            tracing::error!("Parent auto-close sweep error: {}", e);
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

        // 3. Mark failed stages and unclaim issues
        for stage_id in &failed_stages {
            run_state.execution.update_stage(
                stage_id,
                StageStatus::Failed("agent died or timed out".into()),
            );
            if let Some(sa) = run_state.stage_agents.remove(stage_id) {
                self.process_manager.remove_agent(&sa.agent_session_id).await;
                let conn = self.db.lock().unwrap();
                let _ = Issue::unclaim(&conn, &sa.issue_id);
            }
            tracing::warn!(workflow = %instance_id, stage = %stage_id, "Stage failed");
        }

        // 4. Spawn agents for newly ready stages
        //    We need to split the borrow: extract process_manager and db refs
        //    before getting the mutable run_state reference for spawning.
        if !completed_stages.is_empty() {
            // Re-borrow run_state after the previous mutable borrow scope ended
            let run_state = self.active_workflows.get_mut(instance_id).unwrap();
            let ready = run_state.execution.ready_stages();
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

        // 5. Check workflow completion
        let run_state = match self.active_workflows.get_mut(instance_id) {
            Some(rs) => rs,
            None => return Ok(()),
        };

        if run_state.execution.is_complete() {
            let all_succeeded = run_state
                .execution
                .stage_statuses
                .values()
                .all(|s| matches!(s, StageStatus::Completed));
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

        // 6. Checkpoint
        let checkpoint = serde_json::to_value(&run_state.execution)?;
        run_state.state_machine.checkpoint(checkpoint)?;

        Ok(())
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
                        if let (Some(branch), Some(wt_path)) = (&session.branch, &session.worktree_path) {
                            let merge_id = uuid::Uuid::new_v4().to_string();
                            let project_id_for_merge = Team::get_by_id(&conn, &ta.team_id)
                                .map(|t| t.project_id)
                                .unwrap_or_default();
                            // Detect target branch from the project's repo
                            let target = if !project_id_for_merge.is_empty() {
                                Project::get_by_id(&conn, &project_id_for_merge)
                                    .ok()
                                    .and_then(|p| WorktreeManager::detect_default_branch(std::path::Path::new(&p.directory)))
                                    .unwrap_or_else(|| "main".to_string())
                            } else {
                                "main".to_string()
                            };
                            let _ = conn.execute(
                                "INSERT INTO merge_queue_entries (id, project_id, agent_session_id, branch, worktree_path, target_branch) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                rusqlite::params![merge_id, project_id_for_merge, ta.agent_session_id, branch, wt_path, target],
                            );
                            tracing::info!(branch = %branch, target = %target, "Enqueued branch for merge");
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
                to_remove.push(key.clone());
                continue;
            }

            // Check if agent process has exited
            let exit_status = self.process_manager.check_agent_exit(&ta.agent_session_id).await;
            if let Some(success) = exit_status {
                self.process_manager.remove_agent(&ta.agent_session_id).await;
                // If issue is still in_progress, agent exited without closing it —
                // unclaim so it can be retried, regardless of exit code
                if issue.status != "closed" {
                    let conn = self.db.lock().unwrap();
                    let _ = Issue::unclaim(&conn, &ta.issue_id);
                    let state = if success { "completed" } else { "failed" };
                    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, state);
                    tracing::warn!(
                        team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                        exit_success = success,
                        "Team agent exited without closing issue — unclaiming for retry"
                    );
                }
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
                {
                    let conn = self.db.lock().unwrap();
                    let _ = Issue::unclaim(&conn, &ta.issue_id);
                    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, "failed");
                }
                tracing::warn!(
                    team = %ta.team_id, role = %ta.role, issue = %ta.issue_id,
                    "Team agent PTY died — unclaiming issue"
                );
                to_remove.push(key.clone());
                continue;
            }

            // Idle escalation
            let idle_secs = ta.last_activity.elapsed().as_secs();
            if idle_secs >= KILL_THRESHOLD_SECS {
                tracing::warn!(team = %ta.team_id, role = %ta.role, "Killing idle team agent");
                let _ = self
                    .process_manager
                    .stop_agent(&ta.agent_session_id)
                    .await;
                {
                    let conn = self.db.lock().unwrap();
                    let _ = Issue::unclaim(&conn, &ta.issue_id);
                    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, "failed");
                }
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
            let slots = {
                let conn = self.db.lock().unwrap();
                TeamAgentSlot::list_by_team(&conn, &team.id)?
            };

            let pickup_types = team.get_auto_pickup_types();
            if pickup_types.is_empty() {
                continue;
            }
            let pickup_refs: Vec<&str> = pickup_types.iter().map(|s| s.as_str()).collect();

            // Group slots by role and count how many agents are already running per role
            let mut slots_by_role: HashMap<String, Vec<TeamAgentSlot>> = HashMap::new();
            for slot in slots {
                slots_by_role
                    .entry(slot.role.clone())
                    .or_default()
                    .push(slot);
            }

            for (role, role_slots) in &slots_by_role {
                // Count running agents for this team+role
                let running_count = self
                    .team_agents
                    .values()
                    .filter(|ta| ta.team_id == team.id && ta.role == *role)
                    .count();

                let max_for_role = role_slots.len();
                if running_count >= max_for_role {
                    continue;
                }

                // Find matching open issues
                let ready_issues = {
                    let conn = self.db.lock().unwrap();
                    Issue::get_ready_by_role(&conn, &team.project_id, role, &pickup_refs)?
                };

                // Spawn agents up to slot count
                let slots_available = max_for_role - running_count;
                let to_spawn = std::cmp::min(slots_available, ready_issues.len());

                for i in 0..to_spawn {
                    let issue = &ready_issues[i];
                    let slot = &role_slots[i % role_slots.len()];
                    if let Err(e) = self.spawn_team_agent(team, slot, issue).await {
                        tracing::error!(
                            team = %team.id, role = %role, issue = %issue.id,
                            "Failed to spawn team agent: {}", e
                        );
                    }
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

        // Create AgentSession record
        let session = {
            let conn = self.db.lock().unwrap();
            AgentSession::create(
                &conn,
                &CreateAgentSession {
                    team_id: team.id.clone(),
                    slot_id: slot.id.clone(),
                    runtime: slot.runtime.clone(),
                    workflow_instance_id: None,
                    pid: None,
                    worktree_path: worktree_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    branch: branch_name.clone(),
                },
            )?
        };

        // Claim the issue
        {
            let conn = self.db.lock().unwrap();
            Issue::claim(&conn, &issue.id, &session.id)?;
        }
        self.log_activity("issue_claimed", &format!("Issue '{}' claimed", issue.title), Some(&team.project_id), Some(&team.id), Some(&session.id), Some(&issue.id), None);

        // Build prompt with role, task details, and curl instructions
        let description = &issue.description;
        let mut prompt_parts = vec![
            format!("You are a {} agent working on project {}.", slot.role, project_name),
        ];

        if !claude_md.is_empty() {
            prompt_parts.push(format!("\n## Project Guidelines\n{}", claude_md));
        }

        if !file_tree.is_empty() {
            prompt_parts.push(format!("\n## Project Structure\n```\n{}\n```", file_tree));
        }

        prompt_parts.push(format!(
            "\n## Your Task\n**{}**\n\n{}\n\n\
            When you have completed your work, close your issue by running:\n\
            curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"status\": \"closed\", \"summary\": \"Brief description of what you accomplished\"}}'\n\n\
            You can also post progress updates at any time:\n\
            curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
            -H 'Content-Type: application/json' \\\n  \
            -d '{{\"summary\": \"Current progress update\"}}'",
            issue.title,
            description,
            team.project_id, issue.id,
            team.project_id, issue.id,
        ));

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
                Some(env)
            },
            allowed_tools: None,
            skills: None,
            extra_args: Some(vec!["--print".to_string()]),
            playwright_env: None,
            model: slot.model.clone(),
        };

        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        // Spawn PTY agent via process_manager
        if let Err(e) = self.process_manager
            .spawn_agent(&session.id, &slot.runtime, config, size)
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
                Ok(MergeResult::Conflict { files }) => {
                    let conflict_json = serde_json::to_string(&files).unwrap_or_else(|_| "[]".to_string());
                    let file_list = files.iter()
                        .map(|f| format!("- {}", f))
                        .collect::<Vec<_>>()
                        .join("\n");

                    {
                        let conn = self.db.lock().unwrap();
                        let _ = MergeQueueEntry::update_status(
                            &conn, &entry.id, "conflicted", Some(&conflict_json), None, None,
                        );
                        // Create a resolver issue for the conflict
                        let _ = Issue::create(&conn, &CreateIssue {
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
                        });
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
