# Intake Agent & Autonomous Task Decomposition — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable submitting any ticket (bug, tweak, feature) and having it automatically analysed, decomposed into role-tagged subtasks with dependencies, and built by the agent swarm autonomously.

**Architecture:** A new "intake" phase in the orchestrator sweep loop spawns a strong-model agent that reads the codebase, creates child issues via the API, and sets dependency chains. Agents work in git worktree isolation. Completed branches merge via a FIFO queue. Parents auto-close when all children close.

**Tech Stack:** Rust (Axum), SQLite (rusqlite), Svelte 5, portable-pty, git2

**Design doc:** `docs/plans/2026-03-13-intake-agent-design.md`

---

### Task 1: DB Migration — Add parent_id, needs_intake, scope_mode columns

**Files:**
- Modify: `src/db/migrations.rs:191-193` (add new ALTER TABLE statements)

**Step 1: Write the migration**

Add these incremental ALTER TABLE statements after line 192 in `src/db/migrations.rs`:

```rust
// Intake agent columns
let _ = conn.execute("ALTER TABLE issues ADD COLUMN parent_id TEXT REFERENCES issues(id)", []);
let _ = conn.execute("ALTER TABLE issues ADD COLUMN needs_intake INTEGER DEFAULT 1", []);
let _ = conn.execute("ALTER TABLE issues ADD COLUMN scope_mode TEXT DEFAULT 'auto'", []);
```

**Step 2: Add index for parent_id lookups**

Add after line 168 in `src/db/migrations.rs` (with the other CREATE INDEX statements):

```rust
// After the existing CREATE INDEX block (inside the execute_batch), this won't work
// because they're in the execute_batch. Instead, add after the ALTER TABLE block:
```

Actually, add as another idempotent statement after the ALTER TABLEs:

```rust
let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_parent ON issues(parent_id)", []);
let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_needs_intake ON issues(needs_intake)", []);
```

**Step 3: Write the failing test**

Add to `src/db/migrations.rs` tests module:

```rust
#[test]
fn test_issues_has_intake_columns() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    run_migrations(&conn).unwrap();

    // Insert a parent issue
    conn.execute(
        "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'proj', '/tmp', 'work')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO issues (id, project_id, title, parent_id, needs_intake, scope_mode) VALUES ('i1', 'p1', 'Parent', NULL, 1, 'auto')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO issues (id, project_id, title, parent_id, needs_intake, scope_mode) VALUES ('i2', 'p1', 'Child', 'i1', 0, 'auto')",
        [],
    ).unwrap();

    let parent_id: Option<String> = conn
        .query_row("SELECT parent_id FROM issues WHERE id = 'i2'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(parent_id, Some("i1".to_string()));

    let needs_intake: i64 = conn
        .query_row("SELECT needs_intake FROM issues WHERE id = 'i1'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(needs_intake, 1);
}
```

**Step 4: Run tests to verify**

Run: `cargo test -p ironweave db::migrations::tests`
Expected: All tests PASS including new test

**Step 5: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add parent_id, needs_intake, scope_mode columns for intake agent"
```

---

### Task 2: Issue Model — Add new fields to struct, CreateIssue, and from_row

**Files:**
- Modify: `src/models/issue.rs:7-25` (Issue struct)
- Modify: `src/models/issue.rs:27-38` (CreateIssue struct)
- Modify: `src/models/issue.rs:40-48` (UpdateIssue struct)
- Modify: `src/models/issue.rs:51-70` (from_row)
- Modify: `src/models/issue.rs:72-87` (create)

**Step 1: Add fields to Issue struct**

After line 22 (`pub role: Option<String>,`), add:

```rust
pub parent_id: Option<String>,
pub needs_intake: i64,
pub scope_mode: String,
```

**Step 2: Add fields to CreateIssue struct**

After line 37 (`pub role: Option<String>,`), add:

```rust
pub parent_id: Option<String>,
pub needs_intake: Option<i64>,
pub scope_mode: Option<String>,
```

**Step 3: Add fields to UpdateIssue struct**

After line 47 (`pub role: Option<String>,`), add:

```rust
pub needs_intake: Option<i64>,
pub scope_mode: Option<String>,
```

**Step 4: Update from_row**

After line 66 (`role: row.get("role")?,`), add:

```rust
parent_id: row.get("parent_id")?,
needs_intake: row.get("needs_intake")?,
scope_mode: row.get("scope_mode")?,
```

**Step 5: Update create()**

Update the INSERT statement at line 82-84 to include new columns:

```rust
conn.execute(
    "INSERT INTO issues (id, project_id, type, title, description, priority, depends_on, workflow_instance_id, stage_id, role, parent_id, needs_intake, scope_mode)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    params![
        id, input.project_id, issue_type, input.title, description, priority, depends_on,
        input.workflow_instance_id, input.stage_id, input.role,
        input.parent_id,
        input.needs_intake.unwrap_or(1),
        input.scope_mode.as_deref().unwrap_or("auto"),
    ],
)?;
```

**Step 6: Update update() to handle new fields**

In `update()` (line 229-273), add handling for `needs_intake` and `scope_mode` after the `role` handling:

```rust
if let Some(needs_intake) = input.needs_intake {
    sets.push("needs_intake = ?");
    values.push(Box::new(needs_intake));
}
if let Some(ref scope_mode) = input.scope_mode {
    sets.push("scope_mode = ?");
    values.push(Box::new(scope_mode.clone()));
}
```

**Step 7: Add get_children() method**

Add after `update()`:

```rust
/// Returns all child issues for a given parent issue.
pub fn get_children(conn: &Connection, parent_id: &str) -> Result<Vec<Self>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM issues WHERE parent_id = ?1 ORDER BY priority, created_at"
    )?;
    let rows = stmt.query_map(params![parent_id], Self::from_row)?;
    let mut children = Vec::new();
    for row in rows {
        children.push(row?);
    }
    Ok(children)
}

/// Returns issues that need intake processing.
/// These are issues with needs_intake = 1, no parent_id, and status = 'open'.
pub fn get_needs_intake(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM issues WHERE project_id = ?1 AND needs_intake = 1 AND parent_id IS NULL AND status = 'open' ORDER BY priority, created_at"
    )?;
    let rows = stmt.query_map(params![project_id], Self::from_row)?;
    let mut issues = Vec::new();
    for row in rows {
        issues.push(row?);
    }
    Ok(issues)
}

/// Check if all children of a parent are closed. Returns None if no children exist.
pub fn all_children_closed(conn: &Connection, parent_id: &str) -> Result<Option<bool>> {
    let children = Self::get_children(conn, parent_id)?;
    if children.is_empty() {
        return Ok(None);
    }
    Ok(Some(children.iter().all(|c| c.status == "closed")))
}
```

**Step 8: Update get_ready() to skip parent issues with children**

In `get_ready()` (line 144-174), update the SQL query at line 147 to exclude parent issues that have children and issues needing intake:

```rust
let mut stmt = conn.prepare(
    "SELECT * FROM issues WHERE project_id = ?1 AND status = 'open' AND claimed_by IS NULL AND needs_intake = 0 ORDER BY priority, created_at"
)?;
```

And add filtering in the loop to skip parent issues that have children:

```rust
// Filter out those with unresolved dependencies AND parent issues with children
let mut ready = Vec::new();
for issue in candidates {
    // Skip parent issues that have children (they're just trackers)
    let children = Self::get_children(conn, &issue.id)?;
    if !children.is_empty() {
        continue;
    }

    let deps: Vec<String> = serde_json::from_str(&issue.depends_on).unwrap_or_default();
    // ... rest unchanged
```

**Step 9: Apply same filter to get_ready_by_role()**

In `get_ready_by_role()` (line 177-227), update the SQL at line 188 to include `AND needs_intake = 0`, and add the same children-skip check in the loop.

**Step 10: Update existing tests to include new fields in CreateIssue**

All existing `CreateIssue` usages in tests need the new fields added. Add to each:

```rust
parent_id: None,
needs_intake: Some(0),  // test issues don't need intake
scope_mode: None,
```

**Step 11: Write new tests**

```rust
#[test]
fn test_create_issue_with_parent() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let parent = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: Some("feature".to_string()),
        title: "Parent feature".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
        parent_id: None,
        needs_intake: Some(0),
        scope_mode: None,
    }).unwrap();

    let child = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: Some("task".to_string()),
        title: "Child task".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: Some("senior_coder".to_string()),
        parent_id: Some(parent.id.clone()),
        needs_intake: Some(0),
        scope_mode: None,
    }).unwrap();

    assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
    let children = Issue::get_children(&conn, &parent.id).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].title, "Child task");
}

#[test]
fn test_get_needs_intake() {
    let conn = setup_db();
    let project = create_test_project(&conn);

    // Issue with needs_intake = 1 (default)
    Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "Needs intake".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
        parent_id: None,
        needs_intake: None,  // defaults to 1
        scope_mode: None,
    }).unwrap();

    // Issue with needs_intake = 0
    Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "Ready to go".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
        parent_id: None,
        needs_intake: Some(0),
        scope_mode: None,
    }).unwrap();

    let needs = Issue::get_needs_intake(&conn, &project.id).unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].title, "Needs intake");
}

#[test]
fn test_all_children_closed() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let parent = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "Parent".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
        parent_id: None,
        needs_intake: Some(0),
        scope_mode: None,
    }).unwrap();

    let c1 = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "Child 1".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
        parent_id: Some(parent.id.clone()),
        needs_intake: Some(0),
        scope_mode: None,
    }).unwrap();

    let c2 = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "Child 2".to_string(),
        description: None,
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
        role: None,
        parent_id: Some(parent.id.clone()),
        needs_intake: Some(0),
        scope_mode: None,
    }).unwrap();

    // Not all closed yet
    assert_eq!(Issue::all_children_closed(&conn, &parent.id).unwrap(), Some(false));

    // Close child 1
    conn.execute("UPDATE issues SET status = 'closed' WHERE id = ?1", params![c1.id]).unwrap();
    assert_eq!(Issue::all_children_closed(&conn, &parent.id).unwrap(), Some(false));

    // Close child 2
    conn.execute("UPDATE issues SET status = 'closed' WHERE id = ?1", params![c2.id]).unwrap();
    assert_eq!(Issue::all_children_closed(&conn, &parent.id).unwrap(), Some(true));
}
```

**Step 12: Run tests**

Run: `cargo test -p ironweave models::issue::tests`
Expected: All PASS

**Step 13: Commit**

```bash
git add src/models/issue.rs
git commit -m "feat: add parent_id, needs_intake, scope_mode to Issue model with get_children/get_needs_intake queries"
```

---

### Task 3: API — Add parent_id to CreateIssue, add children endpoint

**Files:**
- Modify: `src/api/issues.rs:6-16` (create handler)
- Modify: `src/api/issues.rs` (add children endpoint)
- Modify: `src/api/routes.rs` or wherever routes are registered

**Step 1: Add children endpoint**

Add to `src/api/issues.rs`:

```rust
pub async fn children(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
) -> Result<Json<Vec<Issue>>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::get_children(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

**Step 2: Register the route**

Find where issue routes are registered and add:

```rust
.route("/api/projects/:project_id/issues/:id/children", get(issues::children))
```

**Step 3: Run tests**

Run: `cargo test -p ironweave`
Expected: All PASS

**Step 4: Commit**

```bash
git add src/api/issues.rs src/api/routes.rs
git commit -m "feat: add children endpoint for parent/child issue hierarchy"
```

---

### Task 4: Frontend — Add parent_id, needs_intake, scope_mode to Issue type and CreateIssue

**Files:**
- Modify: `frontend/src/lib/api.ts:142-159` (Issue interface)
- Modify: `frontend/src/lib/api.ts:161-171` (CreateIssue interface)

**Step 1: Update Issue interface**

Add after `role: string | null;`:

```typescript
parent_id: string | null;
needs_intake: number;
scope_mode: string;
```

**Step 2: Update CreateIssue interface**

Add after `role?: string;`:

```typescript
parent_id?: string;
needs_intake?: number;
scope_mode?: string;
```

**Step 3: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add intake agent fields to frontend Issue types"
```

---

### Task 5: Intake Agent Trigger in Orchestrator Sweep Loop

**Files:**
- Modify: `src/orchestrator/runner.rs:92-100` (OrchestratorRunner struct — add intake tracking)
- Modify: `src/orchestrator/runner.rs:376-393` (sweep method — add intake before team dispatch)
- Add new method: `sweep_intake()` and `spawn_intake_agent()`

**Step 1: Add intake tracking to OrchestratorRunner**

Add a field to track active intake agents per project. In the struct at line 92-100:

```rust
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
}
```

Initialise in `new()`:

```rust
intake_agents: HashMap::new(),
```

**Step 2: Add intake sweep to main sweep**

In `sweep()` at line 376, add intake sweep **before** team dispatch:

```rust
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

    // Remove completed/failed workflows from active map
    self.active_workflows
        .retain(|_, ws| !ws.execution.is_complete());
}
```

**Step 3: Implement sweep_intake()**

```rust
/// Check existing intake agents for completion, then spawn new ones for issues needing intake.
async fn sweep_intake(&mut self) -> crate::error::Result<()> {
    // 1. Check existing intake agents for completion
    let project_ids: Vec<String> = self.intake_agents.keys().cloned().collect();
    let mut to_remove = Vec::new();

    for project_id in &project_ids {
        let (session_id, issue_id) = self.intake_agents.get(project_id).unwrap().clone();

        // Check if agent has exited
        let exit_status = self.process_manager.check_agent_exit(&session_id).await;

        if let Some(success) = exit_status {
            self.process_manager.remove_agent(&session_id).await;
            if success {
                // Intake agent completed successfully
                // The intake agent should have already set needs_intake = 0 via API
                tracing::info!(
                    project = %project_id, issue = %issue_id,
                    "Intake agent completed successfully"
                );
            } else {
                // Intake agent crashed — reset the issue for retry
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

        // Check if agent PTY still exists
        let agent_exists = self.process_manager.get_agent(&session_id).await.is_some();
        if !agent_exists {
            to_remove.push(project_id.clone());
            tracing::warn!(
                project = %project_id, issue = %issue_id,
                "Intake agent PTY disappeared"
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

    let project_ids: Vec<String> = active_teams.iter().map(|t| t.project_id.clone()).collect();
    let unique_projects: Vec<String> = project_ids.into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();

    for project_id in &unique_projects {
        // Skip if already running an intake agent for this project
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
```

**Step 4: Implement spawn_intake_agent()**

```rust
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
    let prompt = format!(
        r#"You are an Intake Agent for project {project_name}.

## Your Job
Analyse the submitted ticket below and break it into actionable subtasks that agents can pick up.

## Ticket
**Title:** {title}
**Type:** {issue_type}
**Description:**
{description}

## Scope Mode: {scope_mode}
{scope_instructions}

## Project Context
**Available roles:** {roles}
**Recent git log:**
```
{git_log}
```
**File tree:**
```
{file_tree}
```

## Instructions

1. Read relevant source files to understand the codebase
2. Analyse the ticket — determine type (bug fix, tweak, feature, performance)
3. Break into subtasks with:
   - Clear title and description with acceptance criteria
   - A role from the available list
   - Dependencies (which tasks must complete before this one can start)
4. Create child issues via the API
5. Update the parent issue when done

## API Commands

**Create a child issue:**
```bash
curl -X POST ${{IRONWEAVE_API}}/api/projects/{project_id}/issues \
  -H 'Content-Type: application/json' \
  -d '{{
    "title": "Task title",
    "description": "Detailed description with acceptance criteria",
    "issue_type": "task",
    "role": "senior_coder",
    "parent_id": "{parent_id}",
    "needs_intake": 0,
    "depends_on": ["id-of-dependency-1"]
  }}'
```

**Update parent issue (do this LAST after creating all children):**
```bash
curl -X PATCH ${{IRONWEAVE_API}}/api/projects/{project_id}/issues/{parent_id} \
  -H 'Content-Type: application/json' \
  -d '{{
    "needs_intake": 0,
    "summary": "Decomposed into N subtasks: [list them briefly]"
  }}'
```

## Guidelines
- Bug reports: 1-3 tasks. Investigate, identify root cause, plan fix.
- Tweaks: 1-2 tasks. Quick scan, small change.
- Features: 4-10 tasks with dependency chains. Full scope, architecture, phased breakdown.
- Performance: 2-4 tasks. Profile, identify bottleneck, plan optimisation.
- Simple changes that don't need decomposition: set needs_intake=0 on the parent directly and leave it for an agent to pick up (set a role on the parent too).
- Always set needs_intake=0 on child issues so agents can pick them up immediately when unblocked.
- Use depends_on to create execution waves — independent tasks unlock in parallel.
"#,
        project_name = project_name,
        title = issue.title,
        issue_type = issue.issue_type,
        description = issue.description,
        scope_mode = scope_mode,
        scope_instructions = if scope_mode == "conversational" {
            "This ticket needs scoping. Ask clarifying questions by updating the parent's summary field with your questions, set status to 'awaiting_input', then exit. The user will update the description with answers and intake will re-trigger."
        } else {
            "Analyse and decompose automatically. No user interaction needed."
        },
        roles = available_roles.join(", "),
        git_log = git_log.trim(),
        file_tree = file_tree.trim(),
        project_id = project_id,
        parent_id = issue.id,
    );

    let session_id = uuid::Uuid::new_v4().to_string();

    let config = AgentConfig {
        working_directory: std::path::PathBuf::from(&project_dir),
        prompt,
        environment: {
            let mut env = std::collections::HashMap::new();
            env.insert("IRONWEAVE_API".to_string(), api_url);
            Some(env)
        },
        allowed_tools: None,
        skills: None,
        extra_args: None,
        playwright_env: None,
        model: Some("sonnet".to_string()), // Strongest available model for intake
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
```

**Step 5: Run tests**

Run: `cargo test -p ironweave`
Expected: All PASS (compile check)

**Step 6: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: add intake agent trigger in orchestrator sweep loop"
```

---

### Task 6: Parent Auto-Close Logic

**Files:**
- Modify: `src/orchestrator/runner.rs` (add sweep_parent_autoclose method)

**Step 1: Implement sweep_parent_autoclose()**

Add to `OrchestratorRunner`:

```rust
/// Check if any parent issues should be auto-closed (all children closed).
async fn sweep_parent_autoclose(&mut self) -> crate::error::Result<()> {
    let conn = self.db.lock().unwrap();

    // Find all parent issues (have children) that are not closed
    let mut stmt = conn.prepare(
        "SELECT DISTINCT parent_id FROM issues WHERE parent_id IS NOT NULL"
    )?;
    let parent_ids: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    for parent_id in &parent_ids {
        let parent = match Issue::get_by_id(&conn, parent_id) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if parent.status == "closed" {
            continue;
        }

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
```

**Step 2: Run tests**

Run: `cargo test -p ironweave`
Expected: All PASS

**Step 3: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: add parent auto-close when all child issues complete"
```

---

### Task 7: Wire Worktree Isolation into spawn_team_agent

**Files:**
- Modify: `src/orchestrator/runner.rs:742-864` (spawn_team_agent)
- Modify: `src/orchestrator/runner.rs:92-100` (add WorktreeManager to OrchestratorRunner)

**Step 1: Add WorktreeManager to OrchestratorRunner**

Add import at top:

```rust
use crate::worktree::manager::WorktreeManager;
```

Add field to struct:

```rust
worktree_manager: WorktreeManager,
```

Update `new()` to accept project base dir and create the manager:

```rust
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
```

**Step 2: Update spawn_team_agent to use worktree**

In `spawn_team_agent()`, after claiming the issue and before building the prompt, add worktree creation:

```rust
// Create git worktree for isolation
let task_hash = &issue.id[..8]; // Short hash for branch name
let (worktree_path, branch_name) = match self.worktree_manager.create_worktree(
    std::path::Path::new(&project_dir),
    &slot.role,
    task_hash,
    "main",
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

// Use worktree path if available, otherwise main project dir
let working_dir = worktree_path
    .as_ref()
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_else(|| project_dir.clone());
```

Then update the `AgentConfig` to use `working_dir` instead of `project_dir`:

```rust
let config = AgentConfig {
    working_directory: std::path::PathBuf::from(&working_dir),
    // ... rest unchanged
};
```

And update the `AgentSession::create` call to include worktree info:

```rust
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
```

**Step 3: Update wherever OrchestratorRunner::new is called**

Find the callsite and add the `worktree_base` parameter. This will be something like:

```rust
let worktree_base = std::path::PathBuf::from("/home/paddy/ironweave/.worktrees");
let runner = OrchestratorRunner::new(rx, db, pm, registry, worktree_base);
```

**Step 4: Run tests**

Run: `cargo test -p ironweave`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: wire worktree isolation into team agent spawn"
```

---

### Task 8: Merge Queue Integration into Sweep Loop

**Files:**
- Modify: `src/orchestrator/runner.rs` (add merge queue processing to sweep)

**Step 1: Add merge queue sweep to main sweep**

In `sweep()`, add after parent auto-close:

```rust
// Merge queue processing
if let Err(e) = self.sweep_merge_queue().await {
    tracing::error!("Merge queue sweep error: {}", e);
}
```

**Step 2: Implement sweep_merge_queue()**

```rust
/// Process one pending merge per project per sweep cycle.
async fn sweep_merge_queue(&mut self) -> crate::error::Result<()> {
    use crate::worktree::merge_queue::{MergeQueueProcessor, MergeResult};

    let conn = self.db.lock().unwrap();

    // Get one queued entry per project (FIFO)
    let mut stmt = conn.prepare(
        "SELECT id, project_id, agent_session_id, branch, worktree_path, target_branch \
         FROM merge_queue_entries WHERE status = 'queued' \
         ORDER BY queued_at ASC"
    )?;

    let entries: Vec<(String, String, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(conn);

    // Process one entry per project
    let mut processed_projects = std::collections::HashSet::new();

    for (entry_id, project_id, _session_id, branch, worktree_path, target_branch) in &entries {
        if processed_projects.contains(project_id) {
            continue;
        }
        processed_projects.insert(project_id.clone());

        // Update status to merging
        {
            let conn = self.db.lock().unwrap();
            let _ = conn.execute(
                "UPDATE merge_queue_entries SET status = 'merging' WHERE id = ?1",
                rusqlite::params![entry_id],
            );
        }

        // Get project directory
        let project_dir = {
            let conn = self.db.lock().unwrap();
            Project::get_by_id(&conn, project_id)
                .map(|p| p.directory)
                .unwrap_or_default()
        };

        if project_dir.is_empty() {
            continue;
        }

        // Attempt merge
        let result = MergeQueueProcessor::try_merge(
            std::path::Path::new(&project_dir),
            branch,
            target_branch,
        );

        let conn = self.db.lock().unwrap();
        match result {
            Ok(MergeResult::Success) => {
                let _ = conn.execute(
                    "UPDATE merge_queue_entries SET status = 'merged', merged_at = datetime('now') WHERE id = ?1",
                    rusqlite::params![entry_id],
                );
                // Clean up worktree
                let _ = self.worktree_manager.remove_worktree(
                    std::path::Path::new(&project_dir),
                    &branch.replace('/', "-"),
                );
                tracing::info!(branch = %branch, "Merge successful");
            }
            Ok(MergeResult::Conflict { files }) => {
                let _ = conn.execute(
                    "UPDATE merge_queue_entries SET status = 'conflict', conflict_tier = 1 WHERE id = ?1",
                    rusqlite::params![entry_id],
                );
                tracing::warn!(
                    branch = %branch,
                    conflicts = ?files,
                    "Merge conflict — needs resolution"
                );
                // TODO: v1.1 — spawn resolver agent
            }
            Ok(MergeResult::Error(msg)) | Err(crate::error::IronweaveError::Internal(msg)) => {
                let _ = conn.execute(
                    "UPDATE merge_queue_entries SET status = 'failed' WHERE id = ?1",
                    rusqlite::params![entry_id],
                );
                tracing::error!(branch = %branch, "Merge failed: {}", msg);
            }
            Err(e) => {
                let _ = conn.execute(
                    "UPDATE merge_queue_entries SET status = 'failed' WHERE id = ?1",
                    rusqlite::params![entry_id],
                );
                tracing::error!(branch = %branch, "Merge error: {}", e);
            }
        }
    }

    Ok(())
}
```

**Step 3: Add merge queue enqueue on agent completion**

In `sweep_teams()`, when a team agent completes with exit code 0 (around line 597-611), add the merge queue enqueue after marking the issue as review:

```rust
if success {
    // Exit code 0 — agent completed successfully, move issue to review
    let _ = Issue::update(&conn, &ta.issue_id, &crate::models::issue::UpdateIssue {
        status: Some("review".to_string()),
        ..Default::default()
    });
    let _ = AgentSession::update_state(&conn, &ta.agent_session_id, "completed");

    // Enqueue for merge if agent was working in a worktree
    let session = AgentSession::get_by_id(&conn, &ta.agent_session_id).ok();
    if let Some(ref sess) = session {
        if let (Some(ref branch), Some(ref wt_path)) = (&sess.branch, &sess.worktree_path) {
            let merge_id = uuid::Uuid::new_v4().to_string();
            let project_id = {
                let team = Team::get_by_id(&conn, &ta.team_id).ok();
                team.map(|t| t.project_id).unwrap_or_default()
            };
            let _ = conn.execute(
                "INSERT INTO merge_queue_entries (id, project_id, agent_session_id, branch, worktree_path, target_branch) VALUES (?1, ?2, ?3, ?4, ?5, 'main')",
                rusqlite::params![merge_id, project_id, ta.agent_session_id, branch, wt_path],
            );
            tracing::info!(branch = %branch, "Enqueued branch for merge");
        }
    }
    // ...
}
```

**Step 4: Run tests**

Run: `cargo test -p ironweave`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: integrate merge queue into orchestrator sweep loop"
```

---

### Task 9: Frontend — Issue Detail Modal with Hierarchy Display

**Files:**
- Modify: `frontend/src/lib/components/IssueBoard.svelte`

**Step 1: Add state for selected issue and children**

Add to the script section:

```typescript
let selectedIssue: Issue | null = $state(null);
let childIssues: Issue[] = $state([]);
let loadingChildren: boolean = $state(false);

async function openIssueDetail(issue: Issue) {
    selectedIssue = issue;
    if (issue.parent_id === null) {
        // This might be a parent — fetch children
        loadingChildren = true;
        try {
            const res = await fetch(`/api/projects/${projectId}/issues/${issue.id}/children`);
            if (res.ok) {
                childIssues = await res.json();
            }
        } catch (e) {
            childIssues = [];
        } finally {
            loadingChildren = false;
        }
    } else {
        childIssues = [];
    }
}

function closeModal() {
    selectedIssue = null;
    childIssues = [];
}

function getParentTitle(parentId: string): string {
    const parent = issueList.find(i => i.id === parentId);
    return parent ? parent.title : parentId.slice(0, 8);
}

function childProgress(parentId: string): string {
    const children = issueList.filter(i => i.parent_id === parentId);
    if (children.length === 0) return '';
    const done = children.filter(c => c.status === 'closed').length;
    return `${done}/${children.length}`;
}
```

**Step 2: Add parent/child badges to issue cards**

In the issue card template, after the role badge, add:

```svelte
{#if issue.parent_id}
    <span class="text-[10px] text-gray-400 px-1.5 py-0.5 rounded bg-gray-700">
        ↳ {getParentTitle(issue.parent_id)}
    </span>
{/if}
{#if childProgress(issue.id)}
    <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-emerald-800 text-emerald-200">
        {childProgress(issue.id)} done
    </span>
{/if}
{#if issue.needs_intake === 1}
    <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-amber-800 text-amber-200">
        intake pending
    </span>
{/if}
```

**Step 3: Make issue cards clickable**

Wrap the card div with an onclick:

```svelte
onclick={() => openIssueDetail(issue)}
```

**Step 4: Add the modal**

After the kanban board div, add:

```svelte
{#if selectedIssue}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div
        class="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-4"
        onclick={closeModal}
    >
        <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
        <div
            class="bg-gray-900 border border-gray-700 rounded-2xl max-w-2xl w-full max-h-[80vh] overflow-y-auto p-6 space-y-4"
            onclick={(e) => e.stopPropagation()}
        >
            <div class="flex items-start justify-between">
                <h2 class="text-lg font-semibold text-gray-100">{selectedIssue.title}</h2>
                <button onclick={closeModal} class="text-gray-500 hover:text-gray-300 text-xl">&times;</button>
            </div>

            <div class="flex gap-2 flex-wrap">
                <span class="text-xs font-medium px-2 py-1 rounded {typeBadgeColor(selectedIssue.type)}">
                    {selectedIssue.type}
                </span>
                <span class="text-xs text-gray-400 px-2 py-1 rounded bg-gray-800">
                    {selectedIssue.status}
                </span>
                {#if selectedIssue.role}
                    <span class="text-xs font-medium px-2 py-1 rounded bg-purple-600 text-purple-100">
                        {selectedIssue.role}
                    </span>
                {/if}
                {#if selectedIssue.parent_id}
                    <span class="text-xs text-gray-400 px-2 py-1 rounded bg-gray-800">
                        ↳ {getParentTitle(selectedIssue.parent_id)}
                    </span>
                {/if}
            </div>

            {#if selectedIssue.description}
                <div class="text-sm text-gray-300 bg-gray-800 rounded-lg p-3 whitespace-pre-wrap">
                    {selectedIssue.description}
                </div>
            {/if}

            {#if selectedIssue.summary}
                <div>
                    <h3 class="text-xs font-semibold text-gray-400 mb-1">Summary</h3>
                    <div class="text-sm text-gray-300 bg-gray-800 rounded-lg p-3 whitespace-pre-wrap">
                        {selectedIssue.summary}
                    </div>
                </div>
            {/if}

            {#if childIssues.length > 0}
                <div>
                    <h3 class="text-xs font-semibold text-gray-400 mb-2">
                        Subtasks ({childIssues.filter(c => c.status === 'closed').length}/{childIssues.length} complete)
                    </h3>
                    <div class="space-y-1">
                        {#each childIssues as child}
                            <div class="flex items-center gap-2 text-sm px-3 py-2 rounded bg-gray-800">
                                <span class={child.status === 'closed' ? 'text-emerald-400' : 'text-gray-500'}>
                                    {child.status === 'closed' ? '✓' : '○'}
                                </span>
                                <span class="text-gray-200 flex-1">{child.title}</span>
                                {#if child.role}
                                    <span class="text-[10px] px-1.5 py-0.5 rounded bg-purple-600/50 text-purple-300">
                                        {child.role}
                                    </span>
                                {/if}
                                <span class="text-[10px] text-gray-500">{child.status}</span>
                            </div>
                        {/each}
                    </div>
                </div>
            {:else if loadingChildren}
                <div class="text-sm text-gray-500">Loading subtasks...</div>
            {/if}

            {#if selectedIssue.claimed_by}
                <div class="text-xs text-gray-500">
                    <span class="text-gray-400">Claimed by:</span>
                    <span class="font-mono ml-1">{selectedIssue.claimed_by}</span>
                </div>
            {/if}

            {#if selectedIssue.depends_on && selectedIssue.depends_on !== '[]'}
                <div class="text-xs text-gray-500">
                    <span class="text-gray-400">Depends on:</span>
                    <span class="font-mono ml-1">{selectedIssue.depends_on}</span>
                </div>
            {/if}
        </div>
    </div>
{/if}
```

**Step 5: Commit**

```bash
git add frontend/src/lib/components/IssueBoard.svelte
git commit -m "feat: add issue detail modal with parent/child hierarchy display"
```

---

### Task 10: Frontend — Scope Mode Toggle on Issue Creation

**Files:**
- Modify: `frontend/src/lib/components/IssueBoard.svelte`

**Step 1: Add scope_mode to create form state**

Add to the form fields:

```typescript
let newScopeMode: string = $state('auto');
```

**Step 2: Add toggle to create form UI**

After the role selector in the create form, add:

```svelte
<div>
    <label class="block text-xs text-gray-400 mb-1">Scope Mode</label>
    <div class="flex gap-2">
        <button
            type="button"
            onclick={() => newScopeMode = 'auto'}
            class="flex-1 px-2 py-1.5 text-xs rounded border transition-colors {newScopeMode === 'auto' ? 'border-purple-500 bg-purple-600/20 text-purple-300' : 'border-gray-700 bg-gray-900 text-gray-400'}"
        >
            Auto
        </button>
        <button
            type="button"
            onclick={() => newScopeMode = 'conversational'}
            class="flex-1 px-2 py-1.5 text-xs rounded border transition-colors {newScopeMode === 'conversational' ? 'border-purple-500 bg-purple-600/20 text-purple-300' : 'border-gray-700 bg-gray-900 text-gray-400'}"
        >
            Needs Scoping
        </button>
    </div>
</div>
```

**Step 3: Include scope_mode in handleCreate**

Update the `data` object in `handleCreate()`:

```typescript
const data: CreateIssue = {
    project_id: projectId,
    title: newTitle.trim(),
    description: newDescription.trim(),
    issue_type: newType,
    priority: newPriority,
    role: newRole || undefined,
    scope_mode: newScopeMode,
};
```

Reset in the success handler:

```typescript
newScopeMode = 'auto';
```

**Step 4: Commit**

```bash
git add frontend/src/lib/components/IssueBoard.svelte
git commit -m "feat: add scope mode toggle to issue creation form"
```

---

### Task 11: Add awaiting_input Status

**Files:**
- Modify: `src/db/migrations.rs` (update status CHECK constraint)
- Modify: `frontend/src/lib/components/IssueBoard.svelte` (add column)

**Step 1: Update the status constraint**

The existing `CREATE TABLE` has a CHECK constraint on status. Since we're using `CREATE TABLE IF NOT EXISTS`, the constraint is baked in. For the existing DB, we need to allow the new status without recreating the table.

SQLite doesn't support ALTER TABLE to change CHECK constraints. The simplest approach: the CHECK only validates on INSERT/UPDATE, and we can work around it by inserting via the API which uses the status field. Actually, the constraint is `CHECK(status IN ('open', 'in_progress', 'review', 'closed'))`.

Best approach: update the `CREATE TABLE IF NOT EXISTS` statement to include `'awaiting_input'` in the CHECK, so new deployments get it. For existing DBs, the orchestrator sets status via UPDATE which SQLite will reject.

**Alternative:** Don't use a new status. Instead, use a flag or the `scope_mode` field. The intake agent can set `summary` with questions and set `scope_mode = 'conversational'`. The sweep can check `scope_mode = 'conversational' AND needs_intake = 1` to know it's awaiting input.

**Decision:** Use the existing status field but update the CHECK. Modify the migration to recreate the table is too risky. Instead, add a migration that creates a new table, copies data, drops old, renames. But that's complex.

**Simpler approach:** Just update the `CREATE TABLE IF NOT EXISTS` for new installs, and for the production DB, run a one-off migration:

```rust
// Allow awaiting_input status for conversational intake
// SQLite doesn't support ALTER CHECK, so we recreate via a temp table approach
// For now, we'll disable the foreign key check temporarily
// Actually, the cleanest way is to just not add awaiting_input to the CHECK
// and instead track this state via scope_mode + needs_intake
```

**Final decision:** Skip `awaiting_input` status for v1. Track conversational state via `scope_mode = 'conversational' AND needs_intake = 1`. The intake agent will set the summary with questions and exit. On the next sweep, if the description has been updated (user answered), intake re-triggers.

**Step 2: Commit (if any changes made)**

Skip this task — use existing fields.

---

### Task 12: Build and Deploy

**Files:**
- No new files

**Step 1: Build locally to verify compilation**

Run: `cargo build --release 2>&1`
Expected: Build succeeds

**Step 2: Run full test suite**

Run: `cargo test -p ironweave`
Expected: All tests PASS

**Step 3: Build frontend**

Run: `cd frontend && npm run build`
Expected: Build succeeds

**Step 4: Deploy to server**

```bash
# Sync source to server
rsync -avz --exclude target --exclude node_modules --exclude .git . paddy@10.202.28.205:/home/paddy/ironweave/

# SSH to server and build
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave && cargo build --release'

# Restart service
ssh paddy@10.202.28.205 'echo "P0w3rPla72012@@" | sudo -S systemctl restart ironweave'
```

**Step 5: Verify deployment**

Run: `curl -k https://10.202.28.205/api/health`
Expected: 200 OK

**Step 6: Test intake by creating an issue**

```bash
curl -k -X POST https://10.202.28.205/api/projects/{PROJECT_ID}/issues \
  -H 'Content-Type: application/json' \
  -d '{"title": "Add health check endpoint", "issue_type": "feature", "description": "Add a /health endpoint that returns service status"}'
```

Wait 30s for sweep, then verify intake agent spawned and created children.

**Step 7: Commit any deployment fixes**

```bash
git add -A
git commit -m "fix: deployment adjustments for intake agent"
```

---

### Task 13: Integration Testing — End-to-End Flow

**Files:**
- No new files (manual testing)

**Step 1: Test bug report flow**

Submit a bug issue and verify:
- Intake agent spawns within 30s
- Intake creates 1-3 child tasks with roles and dependencies
- Parent gets `needs_intake = 0` and summary
- Children appear in the kanban board

**Step 2: Test feature flow**

Submit a feature issue and verify:
- Intake creates 4-10 tasks with dependency chains
- Tasks unlock progressively as dependencies close
- Agents pick up and work in worktrees
- Completed branches enter merge queue
- Merges succeed (or conflicts detected)
- Parent auto-closes when all children done

**Step 3: Test conversational flow**

Submit an issue with `scope_mode = 'conversational'` and verify:
- Intake asks questions in the summary field
- Intake exits
- User updates description with answers
- Next sweep re-triggers intake
- Intake creates children based on answers

**Step 4: Verify issue detail modal**

Click on a parent issue and verify:
- Modal shows description, summary, children list with progress
- Child issues show "↳ parent title" badge
- Parents show "N/N done" progress indicator
