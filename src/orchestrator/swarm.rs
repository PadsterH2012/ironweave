use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::error::{IronweaveError, Result};
use crate::models::issue::Issue;

/// Represents an agent's status in the swarm
#[derive(Debug, Clone)]
pub struct SwarmAgent {
    pub session_id: String,
    pub claimed_task_id: Option<String>,
    pub last_heartbeat: DateTime<Utc>,
    pub state: AgentSwarmState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSwarmState {
    Idle,
    Working,
    Crashed,
}

/// Scaling recommendation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalingAction {
    SpawnMore(usize),
    DrainExcess(usize),
    NoChange,
}

pub struct SwarmCoordinator {
    db: Arc<Mutex<Connection>>,
    project_id: String,
    max_agents: usize,
    pub heartbeat_timeout_secs: i64,
    pub agents: HashMap<String, SwarmAgent>,
}

impl SwarmCoordinator {
    pub fn new(db: Arc<Mutex<Connection>>, project_id: String, max_agents: usize) -> Self {
        Self {
            db,
            project_id,
            max_agents,
            heartbeat_timeout_secs: 60,
            agents: HashMap::new(),
        }
    }

    /// Try to claim the next available task for an agent.
    /// Uses atomic SELECT + UPDATE in a single SQLite transaction.
    pub fn claim_next_task(&mut self, session_id: &str) -> Result<Option<String>> {
        let agent = self
            .agents
            .get(session_id)
            .ok_or_else(|| IronweaveError::NotFound(format!("agent {}", session_id)))?;

        if agent.claimed_task_id.is_some() {
            return Err(IronweaveError::Conflict(format!(
                "agent {} already has a claimed task",
                session_id
            )));
        }

        let conn = self
            .db
            .lock()
            .map_err(|e| IronweaveError::Internal(format!("lock poisoned: {}", e)))?;

        // Atomic claim: find a ready task and claim it in one transaction
        let task_id: Option<String> = {
            let tx = conn.unchecked_transaction()?;

            // Find ready issues (open, unclaimed, unblocked)
            let ready = Issue::get_ready(&tx, &self.project_id)?;
            if let Some(issue) = ready.first() {
                let task_id = issue.id.clone();
                // Atomically claim it — the WHERE clause ensures no race
                let changes = tx.execute(
                    "UPDATE issues SET claimed_by = ?1, claimed_at = datetime('now'), \
                     status = 'in_progress', updated_at = datetime('now') \
                     WHERE id = ?2 AND claimed_by IS NULL AND status = 'open'",
                    rusqlite::params![session_id, task_id],
                )?;
                tx.commit()?;
                if changes > 0 {
                    Some(task_id)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(ref tid) = task_id {
            if let Some(agent) = self.agents.get_mut(session_id) {
                agent.claimed_task_id = Some(tid.clone());
                agent.state = AgentSwarmState::Working;
            }
        }

        Ok(task_id)
    }

    /// Release a claimed task (on completion or failure).
    pub fn release_task(&mut self, session_id: &str, new_status: &str) -> Result<()> {
        let agent = self
            .agents
            .get(session_id)
            .ok_or_else(|| IronweaveError::NotFound(format!("agent {}", session_id)))?;

        let task_id = agent
            .claimed_task_id
            .clone()
            .ok_or_else(|| IronweaveError::Conflict(format!("agent {} has no claimed task", session_id)))?;

        let conn = self
            .db
            .lock()
            .map_err(|e| IronweaveError::Internal(format!("lock poisoned: {}", e)))?;

        conn.execute(
            "UPDATE issues SET claimed_by = NULL, claimed_at = NULL, \
             status = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![new_status, task_id],
        )?;

        if let Some(agent) = self.agents.get_mut(session_id) {
            agent.claimed_task_id = None;
            agent.state = AgentSwarmState::Idle;
        }

        Ok(())
    }

    /// Record a heartbeat from an agent.
    pub fn heartbeat(&mut self, session_id: &str) {
        if let Some(agent) = self.agents.get_mut(session_id) {
            agent.last_heartbeat = Utc::now();
            if agent.state == AgentSwarmState::Crashed {
                // Revive if it was marked crashed but is now reporting in
                agent.state = if agent.claimed_task_id.is_some() {
                    AgentSwarmState::Working
                } else {
                    AgentSwarmState::Idle
                };
            }
        }
    }

    /// Check for crashed agents (missed heartbeats) and unclaim their tasks.
    /// Returns the session IDs of agents that were marked as crashed.
    pub fn check_heartbeats(&mut self) -> Result<Vec<String>> {
        let now = Utc::now();
        let mut crashed_ids = Vec::new();
        let mut tasks_to_unclaim = Vec::new();

        for (sid, agent) in &self.agents {
            if agent.state == AgentSwarmState::Crashed {
                continue;
            }
            let elapsed = now
                .signed_duration_since(agent.last_heartbeat)
                .num_seconds();
            if elapsed > self.heartbeat_timeout_secs {
                crashed_ids.push(sid.clone());
                if let Some(ref task_id) = agent.claimed_task_id {
                    tasks_to_unclaim.push(task_id.clone());
                }
            }
        }

        // Unclaim tasks from crashed agents
        if !tasks_to_unclaim.is_empty() {
            let conn = self
                .db
                .lock()
                .map_err(|e| IronweaveError::Internal(format!("lock poisoned: {}", e)))?;
            for task_id in &tasks_to_unclaim {
                conn.execute(
                    "UPDATE issues SET claimed_by = NULL, claimed_at = NULL, \
                     status = 'open', updated_at = datetime('now') WHERE id = ?1",
                    rusqlite::params![task_id],
                )?;
            }
        }

        // Update agent state
        for sid in &crashed_ids {
            if let Some(agent) = self.agents.get_mut(sid) {
                agent.state = AgentSwarmState::Crashed;
                agent.claimed_task_id = None;
            }
        }

        Ok(crashed_ids)
    }

    /// Get scaling recommendation based on pool depth vs active agents.
    pub fn scaling_recommendation(&self) -> Result<ScalingAction> {
        let conn = self
            .db
            .lock()
            .map_err(|e| IronweaveError::Internal(format!("lock poisoned: {}", e)))?;

        let ready = Issue::get_ready(&conn, &self.project_id)?;
        let pool_depth = ready.len();

        let idle_count = self
            .agents
            .values()
            .filter(|a| a.state == AgentSwarmState::Idle)
            .count();
        let total_agents = self
            .agents
            .values()
            .filter(|a| a.state != AgentSwarmState::Crashed)
            .count();

        if pool_depth > idle_count && total_agents < self.max_agents {
            let needed = pool_depth - idle_count;
            let can_spawn = self.max_agents - total_agents;
            Ok(ScalingAction::SpawnMore(needed.min(can_spawn)))
        } else if pool_depth == 0 && idle_count > 1 {
            Ok(ScalingAction::DrainExcess(idle_count - 1))
        } else {
            Ok(ScalingAction::NoChange)
        }
    }

    /// Register an agent in the swarm.
    pub fn register_agent(&mut self, session_id: String) {
        self.agents.insert(
            session_id.clone(),
            SwarmAgent {
                session_id,
                claimed_task_id: None,
                last_heartbeat: Utc::now(),
                state: AgentSwarmState::Idle,
            },
        );
    }

    /// Remove an agent from the swarm.
    pub fn remove_agent(&mut self, session_id: &str) {
        self.agents.remove(session_id);
    }

    /// Get all agents and their states.
    pub fn agents(&self) -> &HashMap<String, SwarmAgent> {
        &self.agents
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent::{AgentSession, CreateAgentSession};
    use crate::models::issue::CreateIssue;
    use crate::models::project::{CreateProject, Project};
    use crate::models::team::{CreateTeam, CreateTeamAgentSlot, Team, TeamAgentSlot};
    use rusqlite::Connection;

    fn setup_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        Arc::new(Mutex::new(conn))
    }

    fn create_project(db: &Arc<Mutex<Connection>>) -> Project {
        let conn = db.lock().unwrap();
        Project::create(
            &conn,
            &CreateProject {
                name: "SwarmTest".to_string(),
                directory: "/tmp/swarm-test".to_string(),
                context: "homelab".to_string(),
                obsidian_vault_path: None,
                obsidian_project: None,
                git_remote: None,
                mount_id: None,
            },
        )
        .unwrap()
    }

    /// Create a real agent session in the DB and return its id.
    /// The swarm coordinator uses agent session ids as claimed_by values,
    /// which must satisfy the FK constraint on the issues table.
    fn create_agent_session(db: &Arc<Mutex<Connection>>, project: &Project) -> String {
        let conn = db.lock().unwrap();
        let team = Team::create(
            &conn,
            &CreateTeam {
                name: "SwarmTeam".to_string(),
                project_id: project.id.clone(),
                coordination_mode: None,
                max_agents: None,
                token_budget: None,
                cost_budget_daily: None,
                is_template: None,
            },
        )
        .unwrap();
        let slot = TeamAgentSlot::create(
            &conn,
            &CreateTeamAgentSlot {
                team_id: team.id.clone(),
                role: "coder".to_string(),
                runtime: "claude".to_string(),
                model: None,
                config: None,
                slot_order: None,
                is_lead: None,
            },
        )
        .unwrap();
        let session = AgentSession::create(
            &conn,
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
        .unwrap();
        session.id
    }

    /// Create two agent sessions from the same team (avoids UNIQUE constraint on team name).
    fn create_two_agent_sessions(
        db: &Arc<Mutex<Connection>>,
        project: &Project,
    ) -> (String, String) {
        let conn = db.lock().unwrap();
        let team = Team::create(
            &conn,
            &CreateTeam {
                name: "SwarmTeam".to_string(),
                project_id: project.id.clone(),
                coordination_mode: None,
                max_agents: None,
                token_budget: None,
                cost_budget_daily: None,
                is_template: None,
            },
        )
        .unwrap();
        let slot = TeamAgentSlot::create(
            &conn,
            &CreateTeamAgentSlot {
                team_id: team.id.clone(),
                role: "coder".to_string(),
                runtime: "claude".to_string(),
                model: None,
                config: None,
                slot_order: None,
                is_lead: None,
            },
        )
        .unwrap();
        let s1 = AgentSession::create(
            &conn,
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
        .unwrap();
        let s2 = AgentSession::create(
            &conn,
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
        .unwrap();
        (s1.id, s2.id)
    }

    fn create_issue(db: &Arc<Mutex<Connection>>, project_id: &str, title: &str) -> String {
        let conn = db.lock().unwrap();
        let issue = Issue::create(
            &conn,
            &CreateIssue {
                project_id: project_id.to_string(),
                issue_type: None,
                title: title.to_string(),
                description: None,
                priority: None,
                depends_on: None,
                workflow_instance_id: None,
                stage_id: None,
                role: None,
                parent_id: None,
                needs_intake: Some(0),
                scope_mode: None,
            },
        )
        .unwrap();
        issue.id
    }

    #[test]
    fn test_claim_task() {
        let db = setup_db();
        let project = create_project(&db);
        let agent_id = create_agent_session(&db, &project);
        let task_id = create_issue(&db, &project.id, "Task A");

        let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
        coord.register_agent(agent_id.clone());

        let claimed = coord.claim_next_task(&agent_id).unwrap();
        assert_eq!(claimed, Some(task_id.clone()));

        // Verify the agent state was updated
        let agent = coord.agents().get(&agent_id).unwrap();
        assert_eq!(agent.claimed_task_id.as_deref(), Some(task_id.as_str()));
        assert_eq!(agent.state, AgentSwarmState::Working);

        // Verify DB was updated
        let conn = db.lock().unwrap();
        let issue = Issue::get_by_id(&conn, &task_id).unwrap();
        assert_eq!(issue.status, "in_progress");
        assert_eq!(issue.claimed_by.as_deref(), Some(agent_id.as_str()));
    }

    #[test]
    fn test_double_claim_prevention() {
        let db = setup_db();
        let project = create_project(&db);
        let (agent1, agent2) = create_two_agent_sessions(&db, &project);
        create_issue(&db, &project.id, "Only One");

        let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
        coord.register_agent(agent1.clone());
        coord.register_agent(agent2.clone());

        let first = coord.claim_next_task(&agent1).unwrap();
        assert!(first.is_some());

        let second = coord.claim_next_task(&agent2).unwrap();
        assert!(second.is_none());
    }

    #[test]
    fn test_release_task() {
        let db = setup_db();
        let project = create_project(&db);
        let agent_id = create_agent_session(&db, &project);
        let task_id = create_issue(&db, &project.id, "Release Me");

        let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
        coord.register_agent(agent_id.clone());

        coord.claim_next_task(&agent_id).unwrap();
        coord.release_task(&agent_id, "open").unwrap();

        // Agent should be idle with no task
        let agent = coord.agents().get(&agent_id).unwrap();
        assert!(agent.claimed_task_id.is_none());
        assert_eq!(agent.state, AgentSwarmState::Idle);

        // Task should be available again
        let conn = db.lock().unwrap();
        let issue = Issue::get_by_id(&conn, &task_id).unwrap();
        assert_eq!(issue.status, "open");
        assert!(issue.claimed_by.is_none());
    }

    #[test]
    fn test_heartbeat_timeout() {
        let db = setup_db();
        let project = create_project(&db);
        let agent_id = create_agent_session(&db, &project);
        let task_id = create_issue(&db, &project.id, "Orphan Task");

        let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
        coord.heartbeat_timeout_secs = 5;
        coord.register_agent(agent_id.clone());

        coord.claim_next_task(&agent_id).unwrap();

        // Simulate an old heartbeat
        if let Some(agent) = coord.agents.get_mut(&agent_id) {
            agent.last_heartbeat = Utc::now() - chrono::Duration::seconds(10);
        }

        let crashed = coord.check_heartbeats().unwrap();
        assert_eq!(crashed, vec![agent_id.clone()]);

        // Agent should be crashed with no task
        let agent = coord.agents().get(&agent_id).unwrap();
        assert_eq!(agent.state, AgentSwarmState::Crashed);
        assert!(agent.claimed_task_id.is_none());

        // Task should be unclaimed in DB
        let conn = db.lock().unwrap();
        let issue = Issue::get_by_id(&conn, &task_id).unwrap();
        assert_eq!(issue.status, "open");
        assert!(issue.claimed_by.is_none());
    }

    #[test]
    fn test_scaling_spawn_more() {
        let db = setup_db();
        let project = create_project(&db);
        // Create 3 tasks
        create_issue(&db, &project.id, "Task 1");
        create_issue(&db, &project.id, "Task 2");
        create_issue(&db, &project.id, "Task 3");

        let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
        coord.register_agent("agent-1".to_string());

        // 3 tasks in pool, 1 idle agent => need 2 more
        let action = coord.scaling_recommendation().unwrap();
        assert_eq!(action, ScalingAction::SpawnMore(2));
    }

    #[test]
    fn test_scaling_drain_excess() {
        let db = setup_db();
        let project = create_project(&db);
        // No tasks in pool

        let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
        coord.register_agent("agent-1".to_string());
        coord.register_agent("agent-2".to_string());
        coord.register_agent("agent-3".to_string());

        // 0 tasks, 3 idle agents => drain 2
        let action = coord.scaling_recommendation().unwrap();
        assert_eq!(action, ScalingAction::DrainExcess(2));
    }

    #[test]
    fn test_scaling_no_change() {
        let db = setup_db();
        let project = create_project(&db);
        create_issue(&db, &project.id, "Task 1");

        let mut coord = SwarmCoordinator::new(db.clone(), project.id.clone(), 5);
        coord.register_agent("agent-1".to_string());

        // 1 task, 1 idle agent => balanced
        let action = coord.scaling_recommendation().unwrap();
        assert_eq!(action, ScalingAction::NoChange);
    }
}
