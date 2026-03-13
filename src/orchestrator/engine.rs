use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::{IronweaveError, Result};

/// A stage in the workflow DAG.
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

/// A complete workflow DAG definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagDefinition {
    pub stages: Vec<Stage>,
}

/// Status of a stage during execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageStatus {
    Pending,
    Running,
    WaitingApproval,
    Completed,
    Failed(String),
    Skipped,
}

/// Tracks the execution state of a DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagExecutionState {
    pub stage_statuses: HashMap<String, StageStatus>,
    pub execution_order: Vec<Vec<String>>,
    pub dag_json: String,
    #[serde(skip)]
    dag: Option<DagDefinition>,
}

impl DagDefinition {
    /// Parse from a JSON string.
    pub fn from_json(json: &str) -> Result<Self> {
        let def: DagDefinition = serde_json::from_str(json)?;
        def.validate()?;
        Ok(def)
    }

    /// Validate the DAG: all dependency references exist and there are no cycles.
    pub fn validate(&self) -> Result<()> {
        let ids: HashSet<&str> = self.stages.iter().map(|s| s.id.as_str()).collect();

        // Check for duplicate IDs.
        if ids.len() != self.stages.len() {
            return Err(IronweaveError::Internal(
                "duplicate stage IDs in DAG definition".into(),
            ));
        }

        // Check that every dependency references an existing stage.
        for stage in &self.stages {
            for dep in &stage.depends_on {
                if !ids.contains(dep.as_str()) {
                    return Err(IronweaveError::Internal(format!(
                        "stage '{}' depends on unknown stage '{}'",
                        stage.id, dep
                    )));
                }
            }
        }

        // Cycle detection via topological sort attempt.
        self.topological_sort()?;

        Ok(())
    }

    /// Topological sort into parallel tiers.
    ///
    /// Returns a `Vec` of tiers where each tier is a `Vec` of stage IDs that
    /// can execute in parallel. Stages within a tier have all their dependencies
    /// satisfied by earlier tiers.
    pub fn topological_sort(&self) -> Result<Vec<Vec<String>>> {
        // Build adjacency info: in-degree counts and reverse-dep map.
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for stage in &self.stages {
            in_degree.entry(stage.id.as_str()).or_insert(0);
            for dep in &stage.depends_on {
                *in_degree.entry(stage.id.as_str()).or_insert(0) += 1;
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(stage.id.as_str());
            }
        }

        let mut tiers: Vec<Vec<String>> = Vec::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        let mut processed = 0usize;

        // Seed with zero-in-degree nodes.
        for (id, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(id);
            }
        }

        while !queue.is_empty() {
            // Drain the current frontier into one tier.
            let tier: Vec<String> = queue.drain(..).map(|s| s.to_string()).collect();
            processed += tier.len();

            // Reduce in-degrees for dependents; collect next frontier.
            for id in &tier {
                if let Some(deps) = dependents.get(id.as_str()) {
                    for &dep_id in deps {
                        let deg = in_degree.get_mut(dep_id).unwrap();
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep_id);
                        }
                    }
                }
            }

            tiers.push(tier);
        }

        if processed != self.stages.len() {
            return Err(IronweaveError::Internal(
                "cycle detected in DAG definition".into(),
            ));
        }

        Ok(tiers)
    }
}

impl DagExecutionState {
    /// Create a new execution state from a DAG definition.
    pub fn new(dag: &DagDefinition) -> Result<Self> {
        let execution_order = dag.topological_sort()?;
        let dag_json = serde_json::to_string(dag)
            .map_err(|e| IronweaveError::Internal(format!("failed to serialize DAG: {}", e)))?;
        let mut stage_statuses = HashMap::new();
        for stage in &dag.stages {
            stage_statuses.insert(stage.id.clone(), StageStatus::Pending);
        }
        Ok(Self {
            stage_statuses,
            execution_order,
            dag_json,
            dag: Some(dag.clone()),
        })
    }

    /// Restore the in-memory DAG from the serialized `dag_json` field.
    /// Call this after deserializing a `DagExecutionState` from a checkpoint.
    pub fn restore_dag(&mut self) -> Result<()> {
        let dag: DagDefinition = serde_json::from_str(&self.dag_json)
            .map_err(|e| IronweaveError::Internal(format!("failed to parse dag_json: {}", e)))?;
        self.dag = Some(dag);
        Ok(())
    }

    /// Get stage IDs that are ready to run: all dependencies completed, stage
    /// is still `Pending`, and if it is a manual gate, it moves to
    /// `WaitingApproval` instead.
    pub fn ready_stages(&self) -> Vec<String> {
        let dag = match &self.dag {
            Some(d) => d,
            None => return Vec::new(),
        };

        let stage_map: HashMap<&str, &Stage> =
            dag.stages.iter().map(|s| (s.id.as_str(), s)).collect();

        let mut ready = Vec::new();
        for stage in &dag.stages {
            if self.stage_statuses.get(&stage.id) != Some(&StageStatus::Pending) {
                continue;
            }
            let deps_met = stage.depends_on.iter().all(|dep| {
                self.stage_statuses.get(dep.as_str()) == Some(&StageStatus::Completed)
            });
            if deps_met {
                ready.push(stage.id.clone());
            }
        }
        let _ = stage_map; // suppress unused warning
        ready
    }

    /// Mark a stage with the given status.
    pub fn update_stage(&mut self, stage_id: &str, status: StageStatus) {
        self.stage_statuses.insert(stage_id.to_string(), status);
    }

    /// Check if all stages have reached a terminal state.
    pub fn is_complete(&self) -> bool {
        self.stage_statuses.values().all(|s| {
            matches!(
                s,
                StageStatus::Completed | StageStatus::Failed(_) | StageStatus::Skipped
            )
        })
    }

    /// Return IDs of stages currently waiting for manual approval.
    pub fn has_pending_approvals(&self) -> Vec<String> {
        self.stage_statuses
            .iter()
            .filter(|(_, s)| **s == StageStatus::WaitingApproval)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Approve a manual gate, moving it from `WaitingApproval` to `Pending` so
    /// it becomes eligible for execution on the next `ready_stages` call.
    pub fn approve_gate(&mut self, stage_id: &str) -> Result<()> {
        match self.stage_statuses.get(stage_id) {
            Some(StageStatus::WaitingApproval) => {
                self.stage_statuses
                    .insert(stage_id.to_string(), StageStatus::Running);
                Ok(())
            }
            Some(other) => Err(IronweaveError::Conflict(format!(
                "stage '{}' is {:?}, not WaitingApproval",
                stage_id, other
            ))),
            None => Err(IronweaveError::NotFound(format!(
                "unknown stage '{}'",
                stage_id
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    fn make_dag(stages: Vec<Stage>) -> DagDefinition {
        DagDefinition { stages }
    }

    // ── Topological sort ────────────────────────────────────────────

    #[test]
    fn linear_pipeline() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("B", vec!["A"], false),
            make_stage("C", vec!["B"], false),
        ]);
        let tiers = dag.topological_sort().unwrap();
        assert_eq!(tiers.len(), 3);
        assert_eq!(tiers[0], vec!["A"]);
        assert_eq!(tiers[1], vec!["B"]);
        assert_eq!(tiers[2], vec!["C"]);
    }

    #[test]
    fn parallel_fan_out() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("B", vec!["A"], false),
            make_stage("C", vec!["A"], false),
        ]);
        let tiers = dag.topological_sort().unwrap();
        assert_eq!(tiers.len(), 2);
        assert_eq!(tiers[0], vec!["A"]);
        let mut tier1 = tiers[1].clone();
        tier1.sort();
        assert_eq!(tier1, vec!["B", "C"]);
    }

    #[test]
    fn diamond_dag() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("B", vec!["A"], false),
            make_stage("C", vec!["A"], false),
            make_stage("D", vec!["B", "C"], false),
        ]);
        let tiers = dag.topological_sort().unwrap();
        assert_eq!(tiers.len(), 3);
        assert_eq!(tiers[0], vec!["A"]);
        let mut tier1 = tiers[1].clone();
        tier1.sort();
        assert_eq!(tier1, vec!["B", "C"]);
        assert_eq!(tiers[2], vec!["D"]);
    }

    #[test]
    fn cycle_detection() {
        let dag = make_dag(vec![
            make_stage("A", vec!["C"], false),
            make_stage("B", vec!["A"], false),
            make_stage("C", vec!["B"], false),
        ]);
        let result = dag.topological_sort();
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("cycle"), "expected cycle error, got: {}", msg);
    }

    // ── Ready stages tracking ───────────────────────────────────────

    #[test]
    fn ready_stages_tracking() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("B", vec!["A"], false),
            make_stage("C", vec!["A"], false),
        ]);
        let mut state = DagExecutionState::new(&dag).unwrap();

        // Initially only A is ready.
        let ready = state.ready_stages();
        assert_eq!(ready, vec!["A"]);

        // Mark A as running — nothing new should be ready.
        state.update_stage("A", StageStatus::Running);
        assert!(state.ready_stages().is_empty());

        // Mark A as completed — B and C become ready.
        state.update_stage("A", StageStatus::Completed);
        let mut ready = state.ready_stages();
        ready.sort();
        assert_eq!(ready, vec!["B", "C"]);
    }

    // ── Manual gate ─────────────────────────────────────────────────

    #[test]
    fn manual_gate_flow() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("gate", vec!["A"], true),
            make_stage("B", vec!["gate"], false),
        ]);
        let mut state = DagExecutionState::new(&dag).unwrap();

        // Complete A.
        state.update_stage("A", StageStatus::Completed);
        assert_eq!(state.ready_stages(), vec!["gate"]);

        // The executor would check is_manual_gate and set WaitingApproval.
        state.update_stage("gate", StageStatus::WaitingApproval);
        assert_eq!(state.has_pending_approvals(), vec!["gate"]);
        assert!(state.ready_stages().is_empty());

        // Approve the gate — moves to Running.
        state.approve_gate("gate").unwrap();
        assert!(state.has_pending_approvals().is_empty());

        // After gate runs, mark complete.
        state.update_stage("gate", StageStatus::Completed);
        assert_eq!(state.ready_stages(), vec!["B"]);
    }

    #[test]
    fn approve_gate_wrong_state() {
        let dag = make_dag(vec![make_stage("A", vec![], false)]);
        let state = DagExecutionState::new(&dag).unwrap();
        let err = state.clone().approve_gate("A");
        assert!(err.is_err());
    }

    #[test]
    fn approve_gate_unknown_stage() {
        let dag = make_dag(vec![make_stage("A", vec![], false)]);
        let state = DagExecutionState::new(&dag).unwrap();
        let err = state.clone().approve_gate("Z");
        assert!(err.is_err());
    }

    // ── Completion detection ────────────────────────────────────────

    #[test]
    fn completion_detection() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("B", vec!["A"], false),
        ]);
        let mut state = DagExecutionState::new(&dag).unwrap();
        assert!(!state.is_complete());

        state.update_stage("A", StageStatus::Completed);
        assert!(!state.is_complete());

        state.update_stage("B", StageStatus::Completed);
        assert!(state.is_complete());
    }

    #[test]
    fn completion_with_failed_and_skipped() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("B", vec!["A"], false),
        ]);
        let mut state = DagExecutionState::new(&dag).unwrap();

        state.update_stage("A", StageStatus::Failed("oops".into()));
        state.update_stage("B", StageStatus::Skipped);
        assert!(state.is_complete());
    }

    // ── JSON parsing ────────────────────────────────────────────────

    #[test]
    fn parse_from_json() {
        let json_str = serde_json::to_string(&json!({
            "stages": [
                {
                    "id": "build",
                    "name": "Build",
                    "runtime": "claude",
                    "prompt": "run build",
                    "depends_on": [],
                    "is_manual_gate": false
                },
                {
                    "id": "test",
                    "name": "Test",
                    "runtime": "claude",
                    "prompt": "run tests",
                    "depends_on": ["build"],
                    "is_manual_gate": false
                }
            ]
        }))
        .unwrap();

        let dag = DagDefinition::from_json(&json_str).unwrap();
        assert_eq!(dag.stages.len(), 2);
        assert_eq!(dag.stages[1].depends_on, vec!["build"]);
    }

    #[test]
    fn invalid_dependency_reference() {
        let dag = make_dag(vec![make_stage("A", vec!["nonexistent"], false)]);
        let result = dag.validate();
        assert!(result.is_err());
    }

    #[test]
    fn duplicate_stage_ids() {
        let dag = make_dag(vec![
            make_stage("A", vec![], false),
            make_stage("A", vec![], false),
        ]);
        let result = dag.validate();
        assert!(result.is_err());
    }

    // ── Serialization round-trip ───────────────────────────────────

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
}
