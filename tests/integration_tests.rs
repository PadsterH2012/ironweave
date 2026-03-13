//! End-to-end integration tests for Ironweave domain logic.
//!
//! These tests exercise the core modules (db, models, orchestrator, worktree)
//! without needing a running HTTP server.  Every test uses an in-memory SQLite
//! database and (where git is involved) temporary directories.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde_json::json;
use tempfile::tempdir;

use ironweave::db::migrations::run_migrations;
use ironweave::models::agent::{AgentSession, CreateAgentSession};
use ironweave::models::issue::{CreateIssue, Issue};
use ironweave::models::project::{CreateProject, Project};
use ironweave::models::team::{CreateTeam, CreateTeamAgentSlot, Team, TeamAgentSlot};
use ironweave::orchestrator::engine::{
    DagDefinition, DagExecutionState, Stage, StageStatus,
};
use ironweave::orchestrator::state_machine::{StateMachine, WorkflowState};
use ironweave::orchestrator::swarm::{AgentSwarmState, SwarmCoordinator};
use ironweave::worktree::manager::WorktreeManager;
use ironweave::worktree::merge_queue::{MergeQueueProcessor, MergeResult};

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Create an in-memory database with all migrations applied.
fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    run_migrations(&conn).unwrap();
    conn
}

/// Wrapped version for modules that need `Arc<Mutex<Connection>>`.
fn setup_db_arc() -> Arc<Mutex<Connection>> {
    Arc::new(Mutex::new(setup_db()))
}

/// Create a project with sensible defaults.
fn make_project(conn: &Connection) -> Project {
    Project::create(
        conn,
        &CreateProject {
            name: format!("project-{}", uuid::Uuid::new_v4()),
            directory: "/tmp/test-project".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        },
    )
    .unwrap()
}

/// Create a team belonging to `project`.
fn make_team(conn: &Connection, project: &Project) -> Team {
    Team::create(
        conn,
        &CreateTeam {
            name: format!("team-{}", uuid::Uuid::new_v4()),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        },
    )
    .unwrap()
}

/// Create an agent session (requires project, team, slot in the DB).
fn make_agent_session(conn: &Connection, project: &Project) -> AgentSession {
    let team = make_team(conn, project);
    let slot = TeamAgentSlot::create(
        conn,
        &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "coder".to_string(),
            runtime: "claude".to_string(),
            config: None,
            slot_order: None,
        },
    )
    .unwrap();
    AgentSession::create(
        conn,
        &CreateAgentSession {
            team_id: team.id.clone(),
            slot_id: slot.id.clone(),
            runtime: "claude".to_string(),
            workflow_instance_id: None,
            pid: None,
            worktree_path: None,
            branch: None,
        },
    )
    .unwrap()
}

/// Create a simple issue.
fn make_issue(conn: &Connection, project_id: &str, title: &str) -> Issue {
    Issue::create(
        conn,
        &CreateIssue {
            project_id: project_id.to_string(),
            issue_type: None,
            title: title.to_string(),
            description: None,
            priority: None,
            depends_on: None,
            workflow_instance_id: None,
            stage_id: None,
        },
    )
    .unwrap()
}

/// Create an issue that depends on the given issue IDs.
fn make_issue_with_deps(
    conn: &Connection,
    project_id: &str,
    title: &str,
    deps: Vec<String>,
) -> Issue {
    Issue::create(
        conn,
        &CreateIssue {
            project_id: project_id.to_string(),
            issue_type: None,
            title: title.to_string(),
            description: None,
            priority: None,
            depends_on: Some(deps),
            workflow_instance_id: None,
            stage_id: None,
        },
    )
    .unwrap()
}

/// Helper to create a DAG stage.
fn make_stage(id: &str, deps: Vec<&str>, gate: bool) -> Stage {
    Stage {
        id: id.to_string(),
        name: id.to_string(),
        runtime: "claude".to_string(),
        prompt: format!("do {}", id),
        depends_on: deps.into_iter().map(String::from).collect(),
        is_manual_gate: gate,
        model: None,
    }
}

/// Insert a workflow_instances row through the full FK chain so state machine
/// UPDATEs work.  Returns the instance id.
fn insert_workflow_instance(db: &Arc<Mutex<Connection>>, id: &str, state: &str) {
    let conn = db.lock().unwrap();
    // Need a project -> team -> workflow_definition -> workflow_instance chain
    let project = Project::create(
        &conn,
        &CreateProject {
            name: format!("wf-proj-{}", id),
            directory: "/tmp/wf".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        },
    )
    .unwrap();
    let team = Team::create(
        &conn,
        &CreateTeam {
            name: format!("wf-team-{}", id),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        },
    )
    .unwrap();
    let def_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO workflow_definitions (id, name, project_id, team_id) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![def_id, format!("def-{}", id), project.id, team.id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO workflow_instances (id, definition_id, state) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, def_id, state],
    )
    .unwrap();
}

/// Initialise a bare git repo with one commit on "main".
fn init_repo_with_commit(path: &std::path::Path) -> git2::Repository {
    let repo = git2::Repository::init(path).expect("init repo");
    let mut config = repo.config().expect("repo config");
    config.set_str("user.name", "Test User").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();

    std::fs::write(path.join("README.md"), "# Init\n").unwrap();
    {
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("README.md"))
            .unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap();
    }
    // Ensure "main" branch exists
    {
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("main", &head_commit, true).unwrap();
    }
    repo
}

/// Commit a file on a given branch and return the commit OID.
fn commit_file_on_branch(
    repo: &git2::Repository,
    branch_name: &str,
    file_name: &str,
    content: &str,
    message: &str,
) -> git2::Oid {
    let sig = git2::Signature::now("test", "test@test.com").unwrap();
    let parent_commit = repo
        .find_branch(branch_name, git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap();

    let blob_oid = repo.blob(content.as_bytes()).unwrap();
    let mut builder = repo
        .treebuilder(Some(&parent_commit.tree().unwrap()))
        .unwrap();
    builder.insert(file_name, blob_oid, 0o100644).unwrap();
    let tree_oid = builder.write().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();

    repo.commit(
        Some(&format!("refs/heads/{}", branch_name)),
        &sig,
        &sig,
        message,
        &tree,
        &[&parent_commit],
    )
    .unwrap()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Full project lifecycle
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn full_project_lifecycle() {
    let conn = setup_db();

    // Create project
    let project = make_project(&conn);
    assert!(!project.id.is_empty());

    // Create team
    let team = make_team(&conn, &project);
    assert_eq!(team.project_id, project.id);
    assert_eq!(team.coordination_mode, "pipeline");

    // Create issue
    let issue = make_issue(&conn, &project.id, "Implement auth");
    assert_eq!(issue.status, "open");
    assert!(issue.claimed_by.is_none());

    // Claim issue
    let agent = make_agent_session(&conn, &project);
    let claimed = Issue::claim(&conn, &issue.id, &agent.id).unwrap();
    assert_eq!(claimed.status, "in_progress");
    assert_eq!(claimed.claimed_by.as_deref(), Some(agent.id.as_str()));
    assert!(claimed.claimed_at.is_some());

    // Unclaim issue
    let unclaimed = Issue::unclaim(&conn, &issue.id).unwrap();
    assert_eq!(unclaimed.status, "open");
    assert!(unclaimed.claimed_by.is_none());
    assert!(unclaimed.claimed_at.is_none());

    // Delete issue
    Issue::delete(&conn, &issue.id).unwrap();
    assert!(Issue::get_by_id(&conn, &issue.id).is_err());

    // Clean up agent session and its team/slot (FK constraints require this order)
    // The agent was created by make_agent_session which creates its own team+slot.
    // We need to delete the agent session first, then its team.
    let agent_team_id = agent.team_id.clone();
    ironweave::models::agent::AgentSession::delete(&conn, &agent.id).unwrap();
    // Delete the slot (cascaded with team via ON DELETE CASCADE) and then the team
    Team::delete(&conn, &agent_team_id).unwrap();

    // Delete the original team
    Team::delete(&conn, &team.id).unwrap();
    assert!(Team::get_by_id(&conn, &team.id).is_err());

    // Delete project
    Project::delete(&conn, &project.id).unwrap();
    assert!(Project::get_by_id(&conn, &project.id).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Issue dependency DAG
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn issue_dependency_dag() {
    let conn = setup_db();
    let project = make_project(&conn);

    // Create blocking issue (no deps)
    let blocker = make_issue(&conn, &project.id, "Setup database");

    // Create dependent issue (blocked by blocker)
    let dependent = make_issue_with_deps(
        &conn,
        &project.id,
        "Build API layer",
        vec![blocker.id.clone()],
    );

    // Create independent issue (no deps)
    let independent = make_issue(&conn, &project.id, "Write docs");

    // get_ready should return blocker and independent, but NOT dependent
    let ready = Issue::get_ready(&conn, &project.id).unwrap();
    let ready_ids: Vec<&str> = ready.iter().map(|i| i.id.as_str()).collect();
    assert!(ready_ids.contains(&blocker.id.as_str()));
    assert!(ready_ids.contains(&independent.id.as_str()));
    assert!(
        !ready_ids.contains(&dependent.id.as_str()),
        "dependent issue should be blocked"
    );

    // Close the blocker
    conn.execute(
        "UPDATE issues SET status = 'closed' WHERE id = ?1",
        rusqlite::params![blocker.id],
    )
    .unwrap();

    // Now the dependent should become ready
    let ready_after = Issue::get_ready(&conn, &project.id).unwrap();
    let ready_ids_after: Vec<&str> = ready_after.iter().map(|i| i.id.as_str()).collect();
    assert!(
        ready_ids_after.contains(&dependent.id.as_str()),
        "dependent issue should now be ready after blocker is closed"
    );
    assert!(ready_ids_after.contains(&independent.id.as_str()));
    // Blocker is now closed, so it shouldn't appear in ready list
    assert!(!ready_ids_after.contains(&blocker.id.as_str()));
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Workflow state machine flow
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn workflow_state_machine_flow() {
    let db = setup_db_arc();
    insert_workflow_instance(&db, "wf-1", "pending");

    // Create a new state machine in Pending state
    let mut sm = StateMachine::new("wf-1".to_string(), Arc::clone(&db));
    assert_eq!(sm.state(), WorkflowState::Pending);

    // Transition: Pending -> Running
    sm.transition(WorkflowState::Running).unwrap();
    assert_eq!(sm.state(), WorkflowState::Running);

    // Save a checkpoint
    let checkpoint_data = json!({"stage": "build", "progress": 50});
    sm.checkpoint(checkpoint_data.clone()).unwrap();
    assert_eq!(sm.checkpoint_data(), Some(&checkpoint_data));

    // Transition: Running -> Completed
    sm.transition(WorkflowState::Completed).unwrap();
    assert_eq!(sm.state(), WorkflowState::Completed);

    // Verify checkpoint persisted by restoring from DB
    let restored = StateMachine::restore(Arc::clone(&db), "wf-1").unwrap();
    assert_eq!(restored.state(), WorkflowState::Completed);
    assert_eq!(restored.checkpoint_data(), Some(&checkpoint_data));
}

#[test]
fn workflow_state_machine_invalid_transition_rejected() {
    let db = setup_db_arc();
    insert_workflow_instance(&db, "wf-2", "pending");

    let mut sm = StateMachine::new("wf-2".to_string(), db);
    // Cannot go directly from Pending to Completed
    let result = sm.transition(WorkflowState::Completed);
    assert!(result.is_err());
    assert_eq!(sm.state(), WorkflowState::Pending);
}

#[test]
fn workflow_state_machine_crash_resume() {
    let db = setup_db_arc();
    insert_workflow_instance(&db, "wf-crash", "pending");

    // Simulate a workflow that runs, checkpoints, then "crashes"
    {
        let mut sm = StateMachine::new("wf-crash".to_string(), Arc::clone(&db));
        sm.transition(WorkflowState::Running).unwrap();
        sm.checkpoint(json!({"step": 7, "partial_results": [1, 2, 3]}))
            .unwrap();
        sm.transition(WorkflowState::Paused).unwrap();
        // sm drops here — simulating crash
    }

    // After restart, restore from DB
    let mut restored = StateMachine::restore(Arc::clone(&db), "wf-crash").unwrap();
    assert_eq!(restored.state(), WorkflowState::Paused);
    assert_eq!(
        restored.checkpoint_data(),
        Some(&json!({"step": 7, "partial_results": [1, 2, 3]}))
    );

    // Can resume
    restored.transition(WorkflowState::Running).unwrap();
    assert_eq!(restored.state(), WorkflowState::Running);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. DAG engine flow
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dag_engine_parallel_stages() {
    // Build → (Test + Lint in parallel) → Deploy
    let dag = DagDefinition {
        stages: vec![
            make_stage("build", vec![], false),
            make_stage("test", vec!["build"], false),
            make_stage("lint", vec!["build"], false),
            make_stage("deploy", vec!["test", "lint"], false),
        ],
    };

    // Validate and topological sort
    dag.validate().unwrap();
    let tiers = dag.topological_sort().unwrap();
    assert_eq!(tiers.len(), 3);
    assert_eq!(tiers[0], vec!["build"]);
    let mut tier1 = tiers[1].clone();
    tier1.sort();
    assert_eq!(tier1, vec!["lint", "test"]);
    assert_eq!(tiers[2], vec!["deploy"]);

    // Track execution state
    let mut state = DagExecutionState::new(&dag).unwrap();

    // Initially only build is ready
    assert_eq!(state.ready_stages(), vec!["build"]);
    assert!(!state.is_complete());

    // Start and complete build
    state.update_stage("build", StageStatus::Running);
    assert!(state.ready_stages().is_empty());
    state.update_stage("build", StageStatus::Completed);

    // Now test and lint are ready
    let mut ready = state.ready_stages();
    ready.sort();
    assert_eq!(ready, vec!["lint", "test"]);

    // Complete test, lint still running
    state.update_stage("test", StageStatus::Completed);
    state.update_stage("lint", StageStatus::Running);
    assert!(state.ready_stages().is_empty()); // deploy not ready yet

    // Complete lint
    state.update_stage("lint", StageStatus::Completed);
    assert_eq!(state.ready_stages(), vec!["deploy"]);

    // Complete deploy
    state.update_stage("deploy", StageStatus::Completed);
    assert!(state.is_complete());
}

#[test]
fn dag_engine_cycle_detection() {
    let dag = DagDefinition {
        stages: vec![
            make_stage("A", vec!["C"], false),
            make_stage("B", vec!["A"], false),
            make_stage("C", vec!["B"], false),
        ],
    };
    let result = dag.validate();
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("cycle"), "expected cycle error: {}", err_msg);
}

#[test]
fn dag_engine_completion_with_failures() {
    let dag = DagDefinition {
        stages: vec![
            make_stage("A", vec![], false),
            make_stage("B", vec!["A"], false),
        ],
    };
    let mut state = DagExecutionState::new(&dag).unwrap();

    state.update_stage("A", StageStatus::Failed("build error".into()));
    state.update_stage("B", StageStatus::Skipped);
    assert!(state.is_complete());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Swarm coordinator
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn swarm_register_and_claim() {
    let db = setup_db_arc();
    let project = {
        let conn = db.lock().unwrap();
        make_project(&conn)
    };

    let agent_id = {
        let conn = db.lock().unwrap();
        let agent = make_agent_session(&conn, &project);
        agent.id
    };

    // Create a task
    {
        let conn = db.lock().unwrap();
        make_issue(&conn, &project.id, "Swarm task A");
    }

    let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
    coord.register_agent(agent_id.clone());

    // Claim the task
    let claimed = coord.claim_next_task(&agent_id).unwrap();
    assert!(claimed.is_some());

    // Agent should be in Working state
    let agent = coord.agents().get(&agent_id).unwrap();
    assert_eq!(agent.state, AgentSwarmState::Working);
    assert!(agent.claimed_task_id.is_some());
}

#[test]
fn swarm_double_claim_prevention() {
    let db = setup_db_arc();
    let project = {
        let conn = db.lock().unwrap();
        make_project(&conn)
    };

    // Create two agents and one task
    let (agent1_id, agent2_id) = {
        let conn = db.lock().unwrap();
        let a1 = make_agent_session(&conn, &project);
        let a2 = make_agent_session(&conn, &project);
        (a1.id, a2.id)
    };
    {
        let conn = db.lock().unwrap();
        make_issue(&conn, &project.id, "Single task");
    }

    let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
    coord.register_agent(agent1_id.clone());
    coord.register_agent(agent2_id.clone());

    // First agent claims
    let first = coord.claim_next_task(&agent1_id).unwrap();
    assert!(first.is_some());

    // Second agent gets nothing (only one task available)
    let second = coord.claim_next_task(&agent2_id).unwrap();
    assert!(second.is_none());
}

#[test]
fn swarm_agent_already_has_task_error() {
    let db = setup_db_arc();
    let project = {
        let conn = db.lock().unwrap();
        make_project(&conn)
    };

    let agent_id = {
        let conn = db.lock().unwrap();
        let agent = make_agent_session(&conn, &project);
        agent.id
    };
    {
        let conn = db.lock().unwrap();
        make_issue(&conn, &project.id, "Task 1");
        make_issue(&conn, &project.id, "Task 2");
    }

    let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
    coord.register_agent(agent_id.clone());

    // Claim first task
    coord.claim_next_task(&agent_id).unwrap();

    // Trying to claim again should fail (agent already has a task)
    let result = coord.claim_next_task(&agent_id);
    assert!(result.is_err());
}

#[test]
fn swarm_heartbeat_timeout_and_crash_recovery() {
    let db = setup_db_arc();
    let project = {
        let conn = db.lock().unwrap();
        make_project(&conn)
    };

    let agent_id = {
        let conn = db.lock().unwrap();
        let agent = make_agent_session(&conn, &project);
        agent.id
    };
    let task_id = {
        let conn = db.lock().unwrap();
        let issue = make_issue(&conn, &project.id, "Orphan task");
        issue.id
    };

    let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
    coord.heartbeat_timeout_secs = 2;
    coord.register_agent(agent_id.clone());

    // Claim task
    coord.claim_next_task(&agent_id).unwrap();

    // Simulate stale heartbeat
    if let Some(agent) = coord.agents.get_mut(&agent_id) {
        agent.last_heartbeat = chrono::Utc::now() - chrono::Duration::seconds(10);
    }

    // Check heartbeats — agent should be marked crashed, task unclaimed
    let crashed = coord.check_heartbeats().unwrap();
    assert_eq!(crashed, vec![agent_id.clone()]);

    let agent = coord.agents().get(&agent_id).unwrap();
    assert_eq!(agent.state, AgentSwarmState::Crashed);
    assert!(agent.claimed_task_id.is_none());

    // Task should be unclaimed in DB
    {
        let conn = db.lock().unwrap();
        let issue = Issue::get_by_id(&conn, &task_id).unwrap();
        assert_eq!(issue.status, "open");
        assert!(issue.claimed_by.is_none());
    }

    // Heartbeat revives the crashed agent
    coord.heartbeat(&agent_id);
    let agent = coord.agents().get(&agent_id).unwrap();
    assert_eq!(agent.state, AgentSwarmState::Idle);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Worktree lifecycle
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn worktree_create_list_remove() {
    let repo_dir = tempdir().unwrap();
    let wt_dir = tempdir().unwrap();

    let _repo = init_repo_with_commit(repo_dir.path());
    let mgr = WorktreeManager::new(wt_dir.path().to_path_buf());

    // Create worktree
    let (wt_path, branch) = mgr
        .create_worktree(repo_dir.path(), "agent-42", "task-abc", "main")
        .unwrap();
    assert_eq!(branch, "ironweave/agent-42/task-abc");
    assert!(wt_path.exists());

    // Verify the worktree is a valid git repo
    let wt_repo = git2::Repository::open(&wt_path).unwrap();
    assert!(wt_repo.head().is_ok());

    // List worktrees
    let worktrees = mgr.list_worktrees(repo_dir.path()).unwrap();
    assert!(worktrees.contains(&"ironweave-agent-42-task-abc".to_string()));

    // Create a second worktree
    let (wt_path2, _) = mgr
        .create_worktree(repo_dir.path(), "agent-99", "task-xyz", "main")
        .unwrap();
    assert!(wt_path2.exists());
    let worktrees = mgr.list_worktrees(repo_dir.path()).unwrap();
    assert_eq!(
        worktrees
            .iter()
            .filter(|w| w.starts_with("ironweave-"))
            .count(),
        2
    );

    // Remove first worktree
    mgr.remove_worktree(repo_dir.path(), "ironweave-agent-42-task-abc")
        .unwrap();
    assert!(!wt_path.exists());

    // Verify it's gone from the list
    let worktrees_after = mgr.list_worktrees(repo_dir.path()).unwrap();
    assert!(!worktrees_after.contains(&"ironweave-agent-42-task-abc".to_string()));
    assert!(worktrees_after.contains(&"ironweave-agent-99-task-xyz".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Merge queue
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn merge_queue_fast_forward() {
    let dir = tempdir().unwrap();
    let repo = init_repo_with_commit(dir.path());

    // Create feature branch from main
    let main_commit = repo
        .find_branch("main", git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap();
    repo.branch("feature-ff", &main_commit, false).unwrap();

    // Add a commit only on feature (main stays behind)
    let feature_oid = commit_file_on_branch(
        &repo,
        "feature-ff",
        "new_file.txt",
        "hello\n",
        "add new file",
    );

    // Merge should fast-forward
    let result = MergeQueueProcessor::try_merge(dir.path(), "feature-ff", "main").unwrap();
    assert!(matches!(result, MergeResult::Success));

    // Verify main tip matches feature
    let main_tip = repo
        .find_branch("main", git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap()
        .id();
    assert_eq!(main_tip, feature_oid);
}

#[test]
fn merge_queue_normal_merge() {
    let dir = tempdir().unwrap();
    let repo = init_repo_with_commit(dir.path());

    let main_commit = repo
        .find_branch("main", git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap();
    repo.branch("feature-merge", &main_commit, false).unwrap();

    // Diverge: different files on each branch
    commit_file_on_branch(&repo, "main", "main_only.txt", "from main\n", "main work");
    commit_file_on_branch(
        &repo,
        "feature-merge",
        "feature_only.txt",
        "from feature\n",
        "feature work",
    );

    let result =
        MergeQueueProcessor::try_merge(dir.path(), "feature-merge", "main").unwrap();
    assert!(matches!(result, MergeResult::Success));

    // Verify merge commit has both files
    let main_commit = repo
        .find_branch("main", git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap();
    let tree = main_commit.tree().unwrap();
    assert!(tree.get_name("main_only.txt").is_some());
    assert!(tree.get_name("feature_only.txt").is_some());
    assert_eq!(main_commit.parent_count(), 2);
}

#[test]
fn merge_queue_conflict_detection() {
    let dir = tempdir().unwrap();
    let repo = init_repo_with_commit(dir.path());

    let main_commit = repo
        .find_branch("main", git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap();
    repo.branch("feature-conflict", &main_commit, false)
        .unwrap();

    // Both branches modify the same file
    commit_file_on_branch(
        &repo,
        "main",
        "shared.txt",
        "main version\n",
        "main edits shared",
    );
    commit_file_on_branch(
        &repo,
        "feature-conflict",
        "shared.txt",
        "feature version\n",
        "feature edits shared",
    );

    let result =
        MergeQueueProcessor::try_merge(dir.path(), "feature-conflict", "main").unwrap();
    match result {
        MergeResult::Conflict { files } => {
            assert!(!files.is_empty());
            assert!(files.contains(&"shared.txt".to_string()));
        }
        _ => panic!("Expected merge conflict"),
    }
}
