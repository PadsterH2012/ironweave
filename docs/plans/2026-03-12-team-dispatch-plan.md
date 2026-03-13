# Team Dispatch — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let the orchestrator spawn agents from team slots to work on project issues, matching issues to agents by role, with configurable auto-pickup by issue type.

**Architecture:** Add `role` to issues and `auto_pickup_types`/`is_active` to teams. Extend the orchestrator's 30s sweep loop with a `sweep_teams()` method that matches open issues to available slots by role, spawns agents on-demand up to slot count, and creates AgentSession records. Build UI for team activation, auto-pickup config, and role assignment on issues.

**Tech Stack:** Rust (Axum), SQLite (rusqlite), Svelte 5, TypeScript, TDD

---

### Task 1: Schema migration — add role to issues, auto_pickup_types and is_active to teams

**Files:**
- Modify: `src/db/migrations.rs:188` (add ALTER TABLE statements before `Ok(())`)

**Step 1: Add the migration statements**

Add after line 187 (`ALTER TABLE team_agent_slots ADD COLUMN model TEXT`):

```rust
// Team dispatch
let _ = conn.execute("ALTER TABLE issues ADD COLUMN role TEXT", []);
let _ = conn.execute("ALTER TABLE teams ADD COLUMN auto_pickup_types TEXT DEFAULT '[\"task\",\"bug\",\"feature\"]'", []);
let _ = conn.execute("ALTER TABLE teams ADD COLUMN is_active INTEGER DEFAULT 0", []);
```

**Step 2: Run migration test**

Run: `cargo test --lib db::migrations::tests -- --nocapture`
Expected: PASS (idempotent ALTER TABLE)

**Step 3: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add role to issues, auto_pickup_types and is_active to teams"
```

---

### Task 2: Add role field to Issue model — test

**Files:**
- Modify: `src/models/issue.rs:487` (add test before closing `}` of tests module)

**Step 1: Write the failing test**

Add at the end of the `tests` module in `src/models/issue.rs`:

```rust
#[test]
fn test_create_issue_with_role() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let issue = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: Some("task".to_string()),
        title: "Implement auth".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: Some("architect".to_string()),
    }).unwrap();

    assert_eq!(issue.role.as_deref(), Some("architect"));

    // Null role works too
    let issue2 = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "No role".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
    }).unwrap();
    assert!(issue2.role.is_none());
}

#[test]
fn test_update_issue_role() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let issue = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "Update role".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
    }).unwrap();
    assert!(issue.role.is_none());

    let updated = Issue::update(&conn, &issue.id, &UpdateIssue {
        status: None,
        title: None,
        description: None,
        summary: None,
        priority: None,
        role: Some("senior_coder".to_string()),
    }).unwrap();
    assert_eq!(updated.role.as_deref(), Some("senior_coder"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib models::issue::tests::test_create_issue_with_role -- --nocapture`
Expected: FAIL — `role` field doesn't exist on structs

---

### Task 3: Add role field to Issue model — implement

**Files:**
- Modify: `src/models/issue.rs`

**Step 1: Add `role` to `Issue` struct (after line 21, `stage_id`)**

```rust
pub role: Option<String>,
```

**Step 2: Add `role` to `CreateIssue` struct (after line 35, `stage_id`)**

```rust
pub role: Option<String>,
```

**Step 3: Add `role` to `UpdateIssue` struct (after line 44, `priority`)**

```rust
pub role: Option<String>,
```

**Step 4: Update `from_row()` — add after `stage_id` line (line 62)**

```rust
role: row.get("role")?,
```

**Step 5: Update `create()` — change INSERT to include role**

Change the INSERT SQL and params to:
```rust
conn.execute(
    "INSERT INTO issues (id, project_id, type, title, description, priority, depends_on, workflow_instance_id, stage_id, role)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    params![id, input.project_id, issue_type, input.title, description, priority, depends_on, input.workflow_instance_id, input.stage_id, input.role],
)?;
```

**Step 6: Update `update()` — add role handling after the priority block**

```rust
if let Some(ref role) = input.role {
    sets.push("role = ?");
    values.push(Box::new(role.clone()));
}
```

**Step 7: Run tests**

Run: `cargo test --lib models::issue::tests -- --nocapture`
Expected: ALL PASS

**Step 8: Commit**

```bash
git add src/models/issue.rs
git commit -m "feat: add role field to Issue model"
```

---

### Task 4: Add auto_pickup_types and is_active to Team model — test

**Files:**
- Modify: `src/models/team.rs:519` (add test before closing `}` of tests module)

**Step 1: Write failing tests**

Add at the end of the `tests` module:

```rust
#[test]
fn test_team_activate_deactivate() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let team = Team::create(&conn, &CreateTeam {
        name: "Active".to_string(),
        project_id: project.id.clone(),
        coordination_mode: None,
        max_agents: None,
        token_budget: None,
        cost_budget_daily: None,
        is_template: None,
    }).unwrap();

    assert!(!team.is_active);

    let activated = Team::set_active(&conn, &team.id, true).unwrap();
    assert!(activated.is_active);

    let deactivated = Team::set_active(&conn, &team.id, false).unwrap();
    assert!(!deactivated.is_active);
}

#[test]
fn test_team_auto_pickup_types() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let team = Team::create(&conn, &CreateTeam {
        name: "Pickup".to_string(),
        project_id: project.id.clone(),
        coordination_mode: None,
        max_agents: None,
        token_budget: None,
        cost_budget_daily: None,
        is_template: None,
    }).unwrap();

    // Default includes all three types
    let types = team.get_auto_pickup_types();
    assert!(types.contains(&"task".to_string()));
    assert!(types.contains(&"bug".to_string()));
    assert!(types.contains(&"feature".to_string()));

    // Update to only bugs
    let updated = Team::update_auto_pickup_types(&conn, &team.id, &["bug"]).unwrap();
    let types = updated.get_auto_pickup_types();
    assert_eq!(types, vec!["bug".to_string()]);
    assert!(!types.contains(&"task".to_string()));
}

#[test]
fn test_list_active_teams() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    Team::create(&conn, &CreateTeam {
        name: "Inactive".to_string(),
        project_id: project.id.clone(),
        coordination_mode: None,
        max_agents: None,
        token_budget: None,
        cost_budget_daily: None,
        is_template: None,
    }).unwrap();
    let active_team = Team::create(&conn, &CreateTeam {
        name: "Active".to_string(),
        project_id: project.id.clone(),
        coordination_mode: None,
        max_agents: None,
        token_budget: None,
        cost_budget_daily: None,
        is_template: None,
    }).unwrap();
    Team::set_active(&conn, &active_team.id, true).unwrap();

    let active = Team::list_active(&conn).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "Active");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib models::team::tests::test_team_activate_deactivate -- --nocapture`
Expected: FAIL — `is_active` field and methods don't exist

---

### Task 5: Add auto_pickup_types and is_active to Team model — implement

**Files:**
- Modify: `src/models/team.rs`

**Step 1: Add fields to `Team` struct (after line 15, `is_template`)**

```rust
pub auto_pickup_types: String,
pub is_active: bool,
```

**Step 2: Update `from_row()` — add after `is_template` line (line 69)**

```rust
auto_pickup_types: row.get("auto_pickup_types")?,
is_active: row.get::<_, i64>("is_active")? != 0,
```

**Step 3: Add methods to `impl Team` (before `list_templates`, around line 116)**

```rust
pub fn set_active(conn: &Connection, id: &str, active: bool) -> Result<Self> {
    let val: i64 = if active { 1 } else { 0 };
    let changes = conn.execute(
        "UPDATE teams SET is_active = ?1 WHERE id = ?2",
        params![val, id],
    )?;
    if changes == 0 {
        return Err(IronweaveError::NotFound(format!("team {}", id)));
    }
    Self::get_by_id(conn, id)
}

pub fn get_auto_pickup_types(&self) -> Vec<String> {
    serde_json::from_str(&self.auto_pickup_types).unwrap_or_default()
}

pub fn update_auto_pickup_types(conn: &Connection, id: &str, types: &[&str]) -> Result<Self> {
    let json = serde_json::to_string(types)
        .map_err(|e| IronweaveError::Internal(e.to_string()))?;
    let changes = conn.execute(
        "UPDATE teams SET auto_pickup_types = ?1 WHERE id = ?2",
        params![json, id],
    )?;
    if changes == 0 {
        return Err(IronweaveError::NotFound(format!("team {}", id)));
    }
    Self::get_by_id(conn, id)
}

pub fn list_active(conn: &Connection) -> Result<Vec<Self>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM teams WHERE is_active = 1 AND is_template = 0 ORDER BY name"
    )?;
    let rows = stmt.query_map([], Self::from_row)?;
    let mut teams = Vec::new();
    for row in rows {
        teams.push(row?);
    }
    Ok(teams)
}
```

**Step 4: Run tests**

Run: `cargo test --lib models::team::tests -- --nocapture`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/models/team.rs
git commit -m "feat: add auto_pickup_types and is_active to Team model"
```

---

### Task 6: Add team activation API endpoints

**Files:**
- Modify: `src/api/teams.rs:113` (add handlers after `clone_template`)
- Modify: `src/main.rs` (wire new routes)

**Step 1: Add handlers to `src/api/teams.rs`**

Add after `clone_template` (after line 112):

```rust
pub async fn activate(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::set_active(&conn, &id, true)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn deactivate(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::set_active(&conn, &id, false)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

#[derive(serde::Deserialize)]
pub struct UpdateAutoPickup {
    pub types: Vec<String>,
}

pub async fn update_config(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
    Json(input): Json<UpdateAutoPickup>,
) -> Result<Json<Team>, StatusCode> {
    let conn = state.db.lock().unwrap();
    let type_refs: Vec<&str> = input.types.iter().map(|s| s.as_str()).collect();
    Team::update_auto_pickup_types(&conn, &id, &type_refs)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn team_status(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn = state.db.lock().unwrap();
    let team = Team::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    let slots = TeamAgentSlot::list_by_team(&conn, &id).unwrap_or_default();

    // Count running agents per role
    let mut role_status: Vec<serde_json::Value> = Vec::new();
    let mut seen_roles = std::collections::HashSet::new();
    for slot in &slots {
        if !seen_roles.insert(slot.role.clone()) {
            continue;
        }
        let slot_count = slots.iter().filter(|s| s.role == slot.role).count();
        let running: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE team_id = ?1 AND state = 'running'
             AND slot_id IN (SELECT id FROM team_agent_slots WHERE team_id = ?1 AND role = ?2)",
            params![id, slot.role],
            |row| row.get(0),
        ).unwrap_or(0);
        role_status.push(serde_json::json!({
            "role": slot.role,
            "slot_count": slot_count,
            "running": running,
            "runtime": slot.runtime,
            "model": slot.model,
        }));
    }

    Ok(Json(serde_json::json!({
        "team_id": team.id,
        "is_active": team.is_active,
        "auto_pickup_types": team.get_auto_pickup_types(),
        "roles": role_status,
    })))
}
```

**Step 2: Wire routes in `src/main.rs`**

Add after the existing team routes (after the `/api/projects/{pid}/teams/{id}` route):

```rust
.route("/api/projects/{pid}/teams/{id}/activate", put(api::teams::activate))
.route("/api/projects/{pid}/teams/{id}/deactivate", put(api::teams::deactivate))
.route("/api/projects/{pid}/teams/{id}/config", put(api::teams::update_config))
.route("/api/projects/{pid}/teams/{id}/status", get(api::teams::team_status))
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add src/api/teams.rs src/main.rs
git commit -m "feat: add team activation, config, and status API endpoints"
```

---

### Task 7: Add Issue::get_ready_by_role query

**Files:**
- Modify: `src/models/issue.rs`

**Step 1: Write failing test**

Add to tests module:

```rust
#[test]
fn test_get_ready_by_role() {
    let conn = setup_db();
    let project = create_test_project(&conn);

    // Create issues with different roles
    Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: Some("task".to_string()),
        title: "Architect work".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: Some("architect".to_string()),
    }).unwrap();

    Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: Some("bug".to_string()),
        title: "Coder work".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: Some("senior_coder".to_string()),
    }).unwrap();

    Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: Some("feature".to_string()),
        title: "No role work".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
    }).unwrap();

    // Filter by role and types
    let ready = Issue::get_ready_by_role(
        &conn,
        &project.id,
        "architect",
        &["task", "bug", "feature"],
    ).unwrap();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].title, "Architect work");

    // Filter by types
    let ready = Issue::get_ready_by_role(
        &conn,
        &project.id,
        "senior_coder",
        &["task"],  // only tasks, not bugs
    ).unwrap();
    assert_eq!(ready.len(), 0);  // the coder issue is a bug, not a task
}
```

**Step 2: Implement `get_ready_by_role`**

Add to `impl Issue` (before the `update` method):

```rust
/// Returns unclaimed, unblocked issues for a project matching a specific role and issue types.
pub fn get_ready_by_role(
    conn: &Connection,
    project_id: &str,
    role: &str,
    issue_types: &[&str],
) -> Result<Vec<Self>> {
    if issue_types.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders: Vec<String> = (0..issue_types.len()).map(|i| format!("?{}", i + 3)).collect();
    let sql = format!(
        "SELECT * FROM issues WHERE project_id = ?1 AND role = ?2 AND status = 'open' \
         AND claimed_by IS NULL AND type IN ({}) ORDER BY priority, created_at",
        placeholders.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    params.push(Box::new(project_id.to_string()));
    params.push(Box::new(role.to_string()));
    for t in issue_types {
        params.push(Box::new(t.to_string()));
    }
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), Self::from_row)?;
    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }

    // Filter out unresolved dependencies
    let mut ready = Vec::new();
    for issue in candidates {
        let deps: Vec<String> = serde_json::from_str(&issue.depends_on).unwrap_or_default();
        if deps.is_empty() {
            ready.push(issue);
        } else {
            let all_closed = deps.iter().all(|dep_id| {
                match Self::get_by_id(conn, dep_id) {
                    Ok(dep) => dep.status == "closed",
                    Err(_) => false,
                }
            });
            if all_closed {
                ready.push(issue);
            }
        }
    }
    Ok(ready)
}
```

**Step 3: Run tests**

Run: `cargo test --lib models::issue::tests -- --nocapture`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/models/issue.rs
git commit -m "feat: add Issue::get_ready_by_role query for team dispatch"
```

---

### Task 8: Implement sweep_teams in orchestrator — test

**Files:**
- Modify: `src/orchestrator/runner.rs`

This is the core task. The orchestrator needs a `sweep_teams()` method that:
1. Queries active teams
2. For each team, gets slots grouped by role
3. Counts running agents per role (from agent_sessions)
4. Finds matching open issues via `Issue::get_ready_by_role`
5. Spawns agents up to slot count per role

**Step 1: Add `sweep_teams` call to `sweep()` method**

In `src/orchestrator/runner.rs`, in the `sweep()` method (line 359), add after the existing workflow sweep and before the retain:

```rust
async fn sweep(&mut self) {
    let instance_ids: Vec<String> = self.active_workflows.keys().cloned().collect();

    for instance_id in instance_ids {
        if let Err(e) = self.sweep_workflow(&instance_id).await {
            tracing::error!(workflow = %instance_id, "Sweep error: {}", e);
        }
    }

    // Team dispatch sweep
    if let Err(e) = self.sweep_teams().await {
        tracing::error!("Team sweep error: {}", e);
    }

    // Remove completed/failed workflows from active map
    self.active_workflows
        .retain(|_, ws| !ws.execution.is_complete());
}
```

**Step 2: Add team agent tracking struct**

Add after the `StageAgent` struct (after line 64):

```rust
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
```

**Step 3: Add team_agents tracking to OrchestratorRunner**

Add to the `OrchestratorRunner` struct (after line 84):

```rust
team_agents: HashMap<String, TeamDispatchedAgent>,  // keyed by agent_session_id
```

And initialize in `new()`:

```rust
team_agents: HashMap::new(),
```

**Step 4: Implement `sweep_teams()`**

Add to `impl OrchestratorRunner` (after `sweep_workflow`):

```rust
async fn sweep_teams(&mut self) -> crate::error::Result<()> {
    // 1. Check existing team-dispatched agents for completion
    let agent_ids: Vec<String> = self.team_agents.keys().cloned().collect();
    let mut completed = Vec::new();
    let mut failed = Vec::new();

    for agent_id in &agent_ids {
        let ta = self.team_agents.get_mut(agent_id).unwrap();
        let issue = {
            let conn = self.db.lock().unwrap();
            Issue::get_by_id(&conn, &ta.issue_id)?
        };

        // Reset idle timer if issue was updated
        if issue.updated_at != ta.last_issue_updated_at {
            ta.last_activity = Instant::now();
            ta.last_issue_updated_at = issue.updated_at.clone();
        }

        if issue.status == "closed" {
            completed.push(agent_id.clone());
            continue;
        }

        // Check if PTY still alive
        let agent_exists = self.process_manager.get_agent(agent_id).await.is_some();
        if !agent_exists {
            failed.push(agent_id.clone());
            continue;
        }

        // Idle escalation
        let idle_secs = ta.last_activity.elapsed().as_secs();
        if idle_secs >= KILL_THRESHOLD_SECS {
            tracing::warn!(agent = %agent_id, "Killing idle team agent");
            let _ = self.process_manager.stop_agent(agent_id).await;
            failed.push(agent_id.clone());
        } else if idle_secs >= NUDGE_WARNING_SECS && ta.nudge_count >= 1 {
            let msg = "\n\nNo status update received. Please respond or your session will be terminated.\n";
            let _ = self.process_manager.write_to_agent(agent_id, msg.as_bytes()).await;
            ta.nudge_count = 2;
        } else if idle_secs >= NUDGE_THRESHOLD_SECS && ta.nudge_count == 0 {
            let msg = "\n\nPlease update your issue status with your current progress.\n";
            let _ = self.process_manager.write_to_agent(agent_id, msg.as_bytes()).await;
            ta.nudge_count = 1;
        }
    }

    // Clean up completed agents
    for agent_id in &completed {
        if let Some(ta) = self.team_agents.remove(agent_id) {
            let conn = self.db.lock().unwrap();
            let _ = conn.execute(
                "UPDATE agent_sessions SET state = 'completed' WHERE id = ?1",
                rusqlite::params![ta.agent_session_id],
            );
            tracing::info!(agent = %agent_id, role = %ta.role, "Team agent completed");
        }
    }

    // Clean up failed agents
    for agent_id in &failed {
        if let Some(ta) = self.team_agents.remove(agent_id) {
            let conn = self.db.lock().unwrap();
            let _ = Issue::unclaim(&conn, &ta.issue_id);
            let _ = conn.execute(
                "UPDATE agent_sessions SET state = 'failed' WHERE id = ?1",
                rusqlite::params![ta.agent_session_id],
            );
            tracing::warn!(agent = %agent_id, role = %ta.role, "Team agent failed");
        }
    }

    // 2. Dispatch new agents for active teams
    let active_teams: Vec<(crate::models::team::Team, Vec<crate::models::team::TeamAgentSlot>)> = {
        let conn = self.db.lock().unwrap();
        let teams = crate::models::team::Team::list_active(&conn)?;
        teams.into_iter().map(|t| {
            let slots = crate::models::team::TeamAgentSlot::list_by_team(&conn, &t.id).unwrap_or_default();
            (t, slots)
        }).collect()
    };

    for (team, slots) in active_teams {
        let pickup_types = team.get_auto_pickup_types();
        let type_refs: Vec<&str> = pickup_types.iter().map(|s| s.as_str()).collect();

        // Group slots by role
        let mut roles: std::collections::HashMap<String, Vec<&crate::models::team::TeamAgentSlot>> = std::collections::HashMap::new();
        for slot in &slots {
            roles.entry(slot.role.clone()).or_default().push(slot);
        }

        for (role, role_slots) in &roles {
            let slot_count = role_slots.len();

            // Count running agents for this role
            let running_count = self.team_agents.values()
                .filter(|ta| ta.team_id == team.id && ta.role == *role)
                .count();

            if running_count >= slot_count {
                continue;
            }

            // Find matching issues
            let ready_issues = {
                let conn = self.db.lock().unwrap();
                Issue::get_ready_by_role(&conn, &team.project_id, role, &type_refs)
                    .unwrap_or_default()
            };

            let available_slots = slot_count - running_count;
            for issue in ready_issues.into_iter().take(available_slots) {
                // Pick the first available slot for this role
                let slot = role_slots[running_count % role_slots.len()];

                if let Err(e) = self.spawn_team_agent(&team, slot, &issue).await {
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
    team: &crate::models::team::Team,
    slot: &crate::models::team::TeamAgentSlot,
    issue: &Issue,
) -> crate::error::Result<()> {
    // Look up project directory
    let project_dir = {
        let conn = self.db.lock().unwrap();
        crate::models::project::Project::get_by_id(&conn, &team.project_id)
            .map(|p| p.directory)
            .unwrap_or_else(|_| "/home/paddy".to_string())
    };

    // Get project name for prompt
    let project_name = {
        let conn = self.db.lock().unwrap();
        crate::models::project::Project::get_by_id(&conn, &team.project_id)
            .map(|p| p.name)
            .unwrap_or_else(|_| "Unknown".to_string())
    };

    // Build prompt
    let slot_config = if slot.config != "{}" { format!("\n\n{}", slot.config) } else { String::new() };
    let prompt = format!(
        "You are a {} working on project {}.\n\n\
        Your current task:\n\
        Title: {}\n\
        Description: {}\n\
        {}\n\n\
        When you have completed your work, close your issue by running:\n\
        curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
        -H 'Content-Type: application/json' \\\n  \
        -d '{{\"status\": \"closed\", \"summary\": \"Brief description of what you accomplished\"}}'\n\n\
        You can also post progress updates at any time:\n\
        curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{}/issues/{} \\\n  \
        -H 'Content-Type: application/json' \\\n  \
        -d '{{\"summary\": \"Current progress update\"}}'",
        slot.role, project_name,
        issue.title, issue.description,
        slot_config,
        team.project_id, issue.id,
        team.project_id, issue.id,
    );

    // Create AgentSession
    let session_id = {
        let conn = self.db.lock().unwrap();
        let session = crate::models::agent::AgentSession::create(
            &conn,
            &crate::models::agent::CreateAgentSession {
                team_id: team.id.clone(),
                slot_id: slot.id.clone(),
                runtime: slot.runtime.clone(),
                workflow_instance_id: None,
                pid: None,
                worktree_path: None,
                branch: None,
            },
        )?;
        // Claim the issue
        Issue::claim(&conn, &issue.id, &session.id)?;
        session.id
    };

    // Build agent config
    let config = crate::runtime::adapter::AgentConfig {
        working_directory: std::path::PathBuf::from(&project_dir),
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
            Some(env)
        },
        allowed_tools: None,
        skills: None,
        extra_args: None,
        playwright_env: None,
        model: slot.model.clone(),
    };

    let size = portable_pty::PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };

    // Spawn
    self.process_manager
        .spawn_agent(&session_id, &slot.runtime, config, size)
        .await?;

    // Track
    self.team_agents.insert(
        session_id.clone(),
        TeamDispatchedAgent {
            agent_session_id: session_id.clone(),
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

    tracing::info!(
        team = %team.id, role = %slot.role, issue = %issue.id,
        agent = %session_id, "Spawned team agent"
    );
    Ok(())
}
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 6: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: add sweep_teams and spawn_team_agent to orchestrator"
```

---

### Task 9: Update frontend API types for team dispatch

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add `role` to Issue interfaces**

Add `role: string | null;` to the `Issue` interface (after `stage_id`).
Add `role?: string;` to `CreateIssue` (after `stage_id`).
Add `role?: string;` to `UpdateIssue` (after `priority`).

**Step 2: Add `auto_pickup_types` and `is_active` to Team interface**

Add after `is_template`:
```typescript
auto_pickup_types: string;
is_active: boolean;
```

**Step 3: Add team dispatch endpoints to teams API**

Add to the `teams` export object:

```typescript
activate: (projectId: string, id: string) => put<Team>(`/projects/${projectId}/teams/${id}/activate`, {}),
deactivate: (projectId: string, id: string) => put<Team>(`/projects/${projectId}/teams/${id}/deactivate`, {}),
updateConfig: (projectId: string, id: string, types: string[]) => put<Team>(`/projects/${projectId}/teams/${id}/config`, { types }),
status: (projectId: string, id: string) => get<TeamStatus>(`/projects/${projectId}/teams/${id}/status`),
```

**Step 4: Add TeamStatus interface**

```typescript
export interface TeamStatus {
  team_id: string;
  is_active: boolean;
  auto_pickup_types: string[];
  roles: {
    role: string;
    slot_count: number;
    running: number;
    runtime: string;
    model: string | null;
  }[];
}
```

**Step 5: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Success

**Step 6: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add team dispatch types and endpoints to frontend API"
```

---

### Task 10: Add team activation UI

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte`

**Step 1: Add activate/deactivate toggle to team cards**

When a team is expanded, show:
- An "Activate" / "Deactivate" button that calls `teams.activate()` or `teams.deactivate()`
- When active: green "Active" badge, live status showing agents per role
- Auto-pickup config: checkboxes for task, bug, feature — calls `teams.updateConfig()` on change

**Step 2: Add role dropdown to issue creation**

In the IssueBoard component (or wherever issues are created), add a role dropdown populated from the active team's slot roles (deduplicated). If no team is active, show a free-text input.

**Step 3: Add role badge to issue cards**

Show the role as a small badge on each issue card in the board.

**Step 4: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Success

**Step 5: Commit**

```bash
git add frontend/src/routes/ProjectDetail.svelte
git commit -m "feat: add team activation UI with role assignment and status display"
```

---

### Task 11: Build, deploy, and E2E verify

**Files:**
- No new files — build and deploy only

**Step 1: Rsync source and build on server**

```bash
rsync -az --exclude 'target/' --exclude 'node_modules/' --exclude 'frontend/dist/' --exclude '.git/' . paddy@10.202.28.205:/home/paddy/ironweave/
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave/frontend && npm run build'
ssh paddy@10.202.28.205 'source ~/.cargo/env; cd /home/paddy/ironweave && cargo build --release'
```

**Step 2: Restart service**

```bash
ssh paddy@10.202.28.205 'sudo -n /usr/bin/systemctl restart ironweave'
```

**Step 3: E2E verify**

1. Activate a team:
```bash
curl -sk -X PUT https://10.202.28.205/api/projects/{pid}/teams/{tid}/activate | python3 -m json.tool
```

2. Create an issue with a role:
```bash
curl -sk -X POST https://10.202.28.205/api/projects/{pid}/issues \
  -H 'Content-Type: application/json' \
  -d '{"title": "Test dispatch", "issue_type": "task", "role": "Architect"}'
```

3. Check team status (wait 30s for sweep):
```bash
curl -sk https://10.202.28.205/api/projects/{pid}/teams/{tid}/status | python3 -m json.tool
```

4. Verify an agent was spawned and claimed the issue.

5. Open UI, verify team activation toggle and role dropdown work.

**Step 4: Commit any deploy fixes**

```bash
git add -A
git commit -m "fix: deploy adjustments for team dispatch"
```
