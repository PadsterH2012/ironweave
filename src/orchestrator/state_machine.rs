use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowState {
    Pending,
    Running,
    Paused,
    Failed,
    Completed,
}

impl WorkflowState {
    /// Returns true if transitioning from `self` to `next` is permitted.
    pub fn can_transition_to(&self, next: WorkflowState) -> bool {
        matches!(
            (self, next),
            (WorkflowState::Pending, WorkflowState::Running)
                | (WorkflowState::Running, WorkflowState::Paused)
                | (WorkflowState::Running, WorkflowState::Failed)
                | (WorkflowState::Running, WorkflowState::Completed)
                | (WorkflowState::Paused, WorkflowState::Running)
                | (WorkflowState::Paused, WorkflowState::Failed)
                | (WorkflowState::Failed, WorkflowState::Pending) // retry
        )
    }
}

impl fmt::Display for WorkflowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            WorkflowState::Pending => "pending",
            WorkflowState::Running => "running",
            WorkflowState::Paused => "paused",
            WorkflowState::Failed => "failed",
            WorkflowState::Completed => "completed",
        };
        write!(f, "{s}")
    }
}

impl FromStr for WorkflowState {
    type Err = IronweaveError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(WorkflowState::Pending),
            "running" => Ok(WorkflowState::Running),
            "paused" => Ok(WorkflowState::Paused),
            "failed" => Ok(WorkflowState::Failed),
            "completed" => Ok(WorkflowState::Completed),
            other => Err(IronweaveError::Internal(format!(
                "unknown workflow state: {other}"
            ))),
        }
    }
}

/// Manages state transitions for a single workflow instance, persisting every
/// change to SQLite for crash-resume safety.
pub struct StateMachine {
    instance_id: String,
    state: WorkflowState,
    checkpoint_data: Option<serde_json::Value>,
    db: Arc<Mutex<Connection>>,
}

impl StateMachine {
    /// Create a new state machine in the `Pending` state.  The initial state
    /// is **not** persisted here — the caller is expected to have already
    /// inserted the `workflow_instances` row when creating the instance.
    pub fn new(instance_id: String, db: Arc<Mutex<Connection>>) -> Self {
        Self {
            instance_id,
            state: WorkflowState::Pending,
            checkpoint_data: None,
            db,
        }
    }

    /// Current state of the workflow.
    pub fn state(&self) -> WorkflowState {
        self.state
    }

    /// The instance id this machine is tracking.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Attempt a state transition.  Returns an error if the transition is
    /// invalid.  On success the new state (and optional finished_at timestamp)
    /// are persisted to SQLite immediately.
    pub fn transition(&mut self, new_state: WorkflowState) -> Result<()> {
        if !self.state.can_transition_to(new_state) {
            return Err(IronweaveError::Conflict(format!(
                "invalid transition: {} -> {}",
                self.state, new_state
            )));
        }

        let conn = self.db.lock().map_err(|e| {
            IronweaveError::Internal(format!("db lock poisoned: {e}"))
        })?;

        let finished_at: Option<String> = if new_state == WorkflowState::Completed
            || new_state == WorkflowState::Failed
        {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };

        conn.execute(
            "UPDATE workflow_instances SET state = ?1, completed_at = COALESCE(?2, completed_at) WHERE id = ?3",
            rusqlite::params![new_state.to_string(), finished_at, self.instance_id],
        )?;

        self.state = new_state;
        Ok(())
    }

    /// Persist arbitrary JSON checkpoint data so the workflow can be resumed
    /// from this point after a crash.
    pub fn checkpoint(&mut self, data: serde_json::Value) -> Result<()> {
        let serialised = serde_json::to_string(&data)?;

        let conn = self.db.lock().map_err(|e| {
            IronweaveError::Internal(format!("db lock poisoned: {e}"))
        })?;

        conn.execute(
            "UPDATE workflow_instances SET checkpoint = ?1 WHERE id = ?2",
            rusqlite::params![serialised, self.instance_id],
        )?;

        self.checkpoint_data = Some(data);
        Ok(())
    }

    /// Restore a state machine from SQLite.  Returns an error if the instance
    /// does not exist.
    pub fn restore(db: Arc<Mutex<Connection>>, instance_id: &str) -> Result<Self> {
        let conn = db.lock().map_err(|e| {
            IronweaveError::Internal(format!("db lock poisoned: {e}"))
        })?;

        let (state_str, checkpoint_raw): (String, Option<String>) = conn
            .query_row(
                "SELECT state, checkpoint FROM workflow_instances WHERE id = ?1",
                rusqlite::params![instance_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    IronweaveError::NotFound(format!(
                        "workflow instance not found: {instance_id}"
                    ))
                }
                other => IronweaveError::Database(other),
            })?;

        let state = WorkflowState::from_str(&state_str)?;
        let checkpoint_data = checkpoint_raw
            .map(|raw| serde_json::from_str(&raw))
            .transpose()?;

        drop(conn); // release lock before constructing Self which holds the Arc

        Ok(Self {
            instance_id: instance_id.to_string(),
            state,
            checkpoint_data,
            db,
        })
    }

    /// Access the last saved checkpoint data, if any.
    pub fn checkpoint_data(&self) -> Option<&serde_json::Value> {
        self.checkpoint_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: create an in-memory DB with the workflow_instances table and
    /// return it wrapped in Arc<Mutex<_>>.
    fn test_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE workflow_instances (
                id TEXT PRIMARY KEY,
                definition_id TEXT,
                project_id TEXT,
                state TEXT CHECK(state IN ('pending','running','paused','failed','completed')),
                current_stage TEXT,
                checkpoint TEXT,
                started_at TEXT,
                completed_at TEXT
            );",
        )
        .expect("create table");
        Arc::new(Mutex::new(conn))
    }

    /// Insert a minimal workflow_instances row so UPDATE statements have
    /// something to target.
    fn insert_instance(db: &Arc<Mutex<Connection>>, id: &str, state: &str) {
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO workflow_instances (id, state) VALUES (?1, ?2)",
            rusqlite::params![id, state],
        )
        .expect("insert instance");
    }

    // ── Display / FromStr round-trip ──────────────────────────────────

    #[test]
    fn state_display_and_from_str() {
        for state in [
            WorkflowState::Pending,
            WorkflowState::Running,
            WorkflowState::Paused,
            WorkflowState::Failed,
            WorkflowState::Completed,
        ] {
            let s = state.to_string();
            let parsed: WorkflowState = s.parse().unwrap();
            assert_eq!(parsed, state);
        }
    }

    #[test]
    fn from_str_invalid() {
        let result = WorkflowState::from_str("exploded");
        assert!(result.is_err());
    }

    // ── Valid transitions ─────────────────────────────────────────────

    #[test]
    fn transition_pending_to_running() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        assert!(sm.transition(WorkflowState::Running).is_ok());
        assert_eq!(sm.state(), WorkflowState::Running);
    }

    #[test]
    fn transition_running_to_paused() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();
        assert!(sm.transition(WorkflowState::Paused).is_ok());
        assert_eq!(sm.state(), WorkflowState::Paused);
    }

    #[test]
    fn transition_running_to_failed() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();
        assert!(sm.transition(WorkflowState::Failed).is_ok());
    }

    #[test]
    fn transition_running_to_completed() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();
        assert!(sm.transition(WorkflowState::Completed).is_ok());
    }

    #[test]
    fn transition_paused_to_running() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();
        sm.transition(WorkflowState::Paused).unwrap();
        assert!(sm.transition(WorkflowState::Running).is_ok());
    }

    #[test]
    fn transition_paused_to_failed() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();
        sm.transition(WorkflowState::Paused).unwrap();
        assert!(sm.transition(WorkflowState::Failed).is_ok());
    }

    #[test]
    fn transition_failed_to_pending_retry() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();
        sm.transition(WorkflowState::Failed).unwrap();
        assert!(sm.transition(WorkflowState::Pending).is_ok());
    }

    // ── Invalid transitions ──────────────────────────────────────────

    #[test]
    fn transition_pending_to_completed_invalid() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        let err = sm.transition(WorkflowState::Completed);
        assert!(err.is_err());
        // State should remain unchanged
        assert_eq!(sm.state(), WorkflowState::Pending);
    }

    #[test]
    fn transition_completed_to_running_invalid() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();
        sm.transition(WorkflowState::Completed).unwrap();
        let err = sm.transition(WorkflowState::Running);
        assert!(err.is_err());
        assert_eq!(sm.state(), WorkflowState::Completed);
    }

    #[test]
    fn transition_pending_to_paused_invalid() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        assert!(sm.transition(WorkflowState::Paused).is_err());
    }

    // ── Checkpoint save and restore ──────────────────────────────────

    #[test]
    fn checkpoint_save_and_read() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");
        let mut sm = StateMachine::new("w1".into(), db);
        sm.transition(WorkflowState::Running).unwrap();

        let data = json!({"stage": "build", "progress": 42});
        sm.checkpoint(data.clone()).unwrap();

        assert_eq!(sm.checkpoint_data(), Some(&data));
    }

    #[test]
    fn checkpoint_restore_from_db() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");

        // Transition and checkpoint
        let mut sm = StateMachine::new("w1".into(), Arc::clone(&db));
        sm.transition(WorkflowState::Running).unwrap();
        let data = json!({"stage": "test", "items": [1, 2, 3]});
        sm.checkpoint(data.clone()).unwrap();

        // Restore from DB
        let restored = StateMachine::restore(Arc::clone(&db), "w1").unwrap();
        assert_eq!(restored.state(), WorkflowState::Running);
        assert_eq!(restored.checkpoint_data(), Some(&data));
    }

    // ── Crash-resume ─────────────────────────────────────────────────

    #[test]
    fn crash_resume_preserves_state() {
        let db = test_db();
        insert_instance(&db, "w1", "pending");

        // Simulate a workflow that transitions, checkpoints, then "crashes"
        {
            let mut sm = StateMachine::new("w1".into(), Arc::clone(&db));
            sm.transition(WorkflowState::Running).unwrap();
            sm.checkpoint(json!({"step": 3})).unwrap();
            sm.transition(WorkflowState::Paused).unwrap();
            // sm is dropped here — simulating crash
        }

        // After "restart", restore from DB
        let restored = StateMachine::restore(Arc::clone(&db), "w1").unwrap();
        assert_eq!(restored.state(), WorkflowState::Paused);
        assert_eq!(restored.checkpoint_data(), Some(&json!({"step": 3})));

        // Can resume from restored state
        let mut sm = restored;
        assert!(sm.transition(WorkflowState::Running).is_ok());
        assert_eq!(sm.state(), WorkflowState::Running);
    }

    #[test]
    fn restore_not_found() {
        let db = test_db();
        let result = StateMachine::restore(db, "nonexistent");
        assert!(result.is_err());
    }

    // ── Serde round-trip ─────────────────────────────────────────────

    #[test]
    fn serde_json_round_trip() {
        let state = WorkflowState::Running;
        let json_str = serde_json::to_string(&state).unwrap();
        assert_eq!(json_str, "\"running\"");
        let parsed: WorkflowState = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed, state);
    }
}
