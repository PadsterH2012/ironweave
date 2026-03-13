# Teams & Model Selection — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add per-slot model selection to teams, wire model into runtime adapters, seed preset team templates, and build the team/slot management UI.

**Architecture:** Add `model` column to slots table, make teams.project_id nullable for global templates, wire `--model` flag into all three runtime adapters, seed 6 preset templates on startup, build template picker and slot management UI in ProjectDetail.svelte.

**Tech Stack:** Rust (Axum), SQLite (rusqlite), Svelte 5, TypeScript, TDD

---

### Task 1: Schema migration — add model column and nullable project_id

**Files:**
- Modify: `src/db/migrations.rs:185` (add ALTER TABLE statements)

**Step 1: Add the migration statements**

Add after line 184 (`ALTER TABLE issues ADD COLUMN stage_id`):

```rust
// Teams & model selection
let _ = conn.execute("ALTER TABLE team_agent_slots ADD COLUMN model TEXT", []);
// Make project_id nullable for global templates (SQLite doesn't support ALTER COLUMN,
// but NULL values work even with NOT NULL if inserted via direct SQL — for new rows
// we'll handle it in code. For existing NOT NULL constraint, we work around it.)
```

Note: SQLite doesn't support `ALTER COLUMN` to drop `NOT NULL`. However, since the `NOT NULL` constraint on `project_id` only applies to INSERT, and we control all inserts via our Rust code, we can insert empty string `""` for global templates and query with `WHERE project_id = '' AND is_template = 1`. This avoids a full table rebuild migration.

**Alternative**: Use `project_id = '__global__'` as the sentinel value for templates.

**Step 2: Run migration test**

Run: `cargo test --lib db::migrations::tests -- --nocapture`
Expected: PASS (idempotent ALTER TABLE)

**Step 3: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add model column to team_agent_slots"
```

---

### Task 2: Update TeamAgentSlot model with model field

**Files:**
- Modify: `src/models/team.rs`

**Step 1: Write the failing test**

Add to the tests module in `src/models/team.rs`:

```rust
#[test]
fn test_slot_with_model() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let team = Team::create(&conn, &CreateTeam {
        name: "Dev".to_string(),
        project_id: project.id.clone(),
        coordination_mode: None,
        max_agents: None,
        token_budget: None,
        cost_budget_daily: None,
        is_template: None,
    }).unwrap();

    let slot = TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
        team_id: team.id.clone(),
        role: "coder".to_string(),
        runtime: "claude".to_string(),
        model: Some("claude-sonnet-4-6".to_string()),
        config: None,
        slot_order: Some(1),
    }).unwrap();

    assert_eq!(slot.model.as_deref(), Some("claude-sonnet-4-6"));

    // Null model works too
    let slot2 = TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
        team_id: team.id.clone(),
        role: "reviewer".to_string(),
        runtime: "claude".to_string(),
        model: None,
        config: None,
        slot_order: Some(2),
    }).unwrap();
    assert!(slot2.model.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib models::team::tests::test_slot_with_model -- --nocapture`
Expected: FAIL — `model` field doesn't exist on structs

**Step 3: Update the structs and methods**

Add `model` field to `TeamAgentSlot`:

```rust
pub struct TeamAgentSlot {
    pub id: String,
    pub team_id: String,
    pub role: String,
    pub runtime: String,
    pub model: Option<String>,  // NEW
    pub config: String,
    pub slot_order: i64,
}
```

Add `model` to `CreateTeamAgentSlot`:

```rust
pub struct CreateTeamAgentSlot {
    pub team_id: String,
    pub role: String,
    pub runtime: String,
    pub model: Option<String>,  // NEW
    pub config: Option<String>,
    pub slot_order: Option<i64>,
}
```

Update `TeamAgentSlot::from_row()` — add:
```rust
model: row.get("model")?,
```

Update `TeamAgentSlot::create()` — change INSERT to include model:
```rust
conn.execute(
    "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, config, slot_order)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![id, input.team_id, input.role, input.runtime, input.model, config, slot_order],
)?;
```

**Step 4: Run tests**

Run: `cargo test --lib models::team::tests -- --nocapture`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/models/team.rs
git commit -m "feat: add model field to TeamAgentSlot"
```

---

### Task 3: Wire --model flag into runtime adapters

**Files:**
- Modify: `src/runtime/claude.rs:37` (before `cmd.arg(&config.prompt)`)
- Modify: `src/runtime/opencode.rs:31` (before `cmd.arg(&config.prompt)`)
- Modify: `src/runtime/gemini.rs:30` (before `cmd.arg(&config.prompt)`)

**Step 1: Add model flag to ClaudeAdapter**

In `src/runtime/claude.rs`, in `build_command()`, add before `cmd.arg(&config.prompt)`:

```rust
if let Some(ref model) = config.model {
    cmd.arg("--model");
    cmd.arg(model);
}
```

**Step 2: Add model flag to OpenCodeAdapter**

In `src/runtime/opencode.rs`, in `build_command()`, add before `cmd.arg(&config.prompt)`:

```rust
if let Some(ref model) = config.model {
    cmd.arg("--model");
    cmd.arg(model);
}
```

**Step 3: Add model flag to GeminiAdapter**

In `src/runtime/gemini.rs`, in `build_command()`, add before `cmd.arg(&config.prompt)`:

```rust
if let Some(ref model) = config.model {
    cmd.arg("--model");
    cmd.arg(model);
}
```

**Step 4: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 5: Commit**

```bash
git add src/runtime/claude.rs src/runtime/opencode.rs src/runtime/gemini.rs
git commit -m "feat: wire --model flag into all runtime adapters"
```

---

### Task 4: Add slot update method and API endpoint

**Files:**
- Modify: `src/models/team.rs` (add `UpdateTeamAgentSlot` struct and `update()` method)
- Modify: `src/api/teams.rs` (add `update_slot` handler)
- Modify: `src/main.rs:121` (add PUT to slots route)

**Step 1: Write failing test**

Add to tests module in `src/models/team.rs`:

```rust
#[test]
fn test_slot_update() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let team = Team::create(&conn, &CreateTeam {
        name: "Dev".to_string(),
        project_id: project.id.clone(),
        coordination_mode: None,
        max_agents: None,
        token_budget: None,
        cost_budget_daily: None,
        is_template: None,
    }).unwrap();

    let slot = TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
        team_id: team.id.clone(),
        role: "coder".to_string(),
        runtime: "claude".to_string(),
        model: Some("claude-sonnet-4-6".to_string()),
        config: None,
        slot_order: Some(1),
    }).unwrap();

    let updated = TeamAgentSlot::update(&conn, &slot.id, &UpdateTeamAgentSlot {
        role: Some("architect".to_string()),
        runtime: None,
        model: Some(Some("claude-opus-4-6".to_string())),
        slot_order: None,
    }).unwrap();

    assert_eq!(updated.role, "architect");
    assert_eq!(updated.model.as_deref(), Some("claude-opus-4-6"));
    assert_eq!(updated.runtime, "claude"); // unchanged
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib models::team::tests::test_slot_update -- --nocapture`
Expected: FAIL — `UpdateTeamAgentSlot` doesn't exist

**Step 3: Add UpdateTeamAgentSlot struct and update method**

Add after `CreateTeamAgentSlot`:

```rust
#[derive(Debug, Deserialize)]
pub struct UpdateTeamAgentSlot {
    pub role: Option<String>,
    pub runtime: Option<String>,
    pub model: Option<Option<String>>,  // None = don't change, Some(None) = clear, Some(Some(x)) = set
    pub slot_order: Option<i64>,
}
```

Add to `impl TeamAgentSlot`:

```rust
pub fn update(conn: &Connection, id: &str, input: &UpdateTeamAgentSlot) -> Result<Self> {
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref role) = input.role {
        sets.push("role = ?");
        values.push(Box::new(role.clone()));
    }
    if let Some(ref runtime) = input.runtime {
        sets.push("runtime = ?");
        values.push(Box::new(runtime.clone()));
    }
    if let Some(ref model) = input.model {
        sets.push("model = ?");
        values.push(Box::new(model.clone()));
    }
    if let Some(slot_order) = input.slot_order {
        sets.push("slot_order = ?");
        values.push(Box::new(slot_order));
    }

    if sets.is_empty() {
        return Self::get_by_id(conn, id);
    }

    let sql = format!("UPDATE team_agent_slots SET {} WHERE id = ?", sets.join(", "));
    values.push(Box::new(id.to_string()));

    let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let changes = conn.execute(&sql, params.as_slice())?;
    if changes == 0 {
        return Err(IronweaveError::NotFound(format!("team_agent_slot {}", id)));
    }
    Self::get_by_id(conn, id)
}
```

**Step 4: Add API handler**

In `src/api/teams.rs`, add import for `UpdateTeamAgentSlot` and handler:

```rust
use crate::models::team::{Team, CreateTeam, TeamAgentSlot, CreateTeamAgentSlot, UpdateTeamAgentSlot};

pub async fn update_slot(
    State(state): State<AppState>,
    Path((_tid, id)): Path<(String, String)>,
    Json(input): Json<UpdateTeamAgentSlot>,
) -> Result<Json<TeamAgentSlot>, StatusCode> {
    let conn = state.db.lock().unwrap();
    TeamAgentSlot::update(&conn, &id, &input)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}
```

**Step 5: Wire route in main.rs**

Change line 121:
```rust
.route("/api/teams/{tid}/slots/{id}", delete(api::teams::delete_slot))
```
to:
```rust
.route("/api/teams/{tid}/slots/{id}", put(api::teams::update_slot).delete(api::teams::delete_slot))
```

**Step 6: Run tests and verify compilation**

Run: `cargo test --lib models::team::tests -- --nocapture && cargo check`
Expected: ALL PASS

**Step 7: Commit**

```bash
git add src/models/team.rs src/api/teams.rs src/main.rs
git commit -m "feat: add slot update endpoint with model support"
```

---

### Task 5: Seed preset team templates

**Files:**
- Create: `src/db/seeds.rs`
- Modify: `src/db/mod.rs` (add `pub mod seeds;`)
- Modify: `src/main.rs` (call seed function)

**Step 1: Create seeds.rs**

Create `src/db/seeds.rs`:

```rust
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Seed preset team templates. Idempotent — skips if template names already exist.
pub fn seed_team_templates(conn: &Connection) -> Result<(), rusqlite::Error> {
    let templates = vec![
        // (name, coordination_mode, slots: Vec<(role, runtime, model)>)
        ("Dev Team", "pipeline", vec![
            ("Architect", "claude", Some("claude-opus-4-6")),
            ("Coder", "claude", Some("claude-sonnet-4-6")),
            ("Reviewer", "claude", Some("claude-sonnet-4-6")),
        ]),
        ("Fix Team", "pipeline", vec![
            ("Investigator", "claude", Some("claude-sonnet-4-6")),
            ("Fixer", "claude", Some("claude-sonnet-4-6")),
            ("Tester", "claude", Some("claude-haiku-4-5-20251001")),
        ]),
        ("Research Team", "collaborative", vec![
            ("Researcher", "claude", Some("claude-opus-4-6")),
            ("Writer", "claude", Some("claude-sonnet-4-6")),
        ]),
        ("Docs Team", "pipeline", vec![
            ("Analyst", "claude", Some("claude-opus-4-6")),
            ("Documenter", "claude", Some("claude-sonnet-4-6")),
            ("Gap Reviewer", "claude", Some("claude-sonnet-4-6")),
        ]),
        ("Mixed Fleet", "swarm", vec![
            ("Claude Agent", "claude", Some("claude-sonnet-4-6")),
            ("OpenCode Agent", "opencode", None),
            ("Gemini Agent", "gemini", None),
        ]),
        ("Budget Squad", "swarm", vec![
            ("Worker 1", "claude", Some("claude-haiku-4-5-20251001")),
            ("Worker 2", "claude", Some("claude-haiku-4-5-20251001")),
            ("Worker 3", "claude", Some("claude-haiku-4-5-20251001")),
        ]),
    ];

    for (name, mode, slots) in templates {
        // Check if template already exists
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM teams WHERE name = ?1 AND project_id = '__global__' AND is_template = 1",
            params![name],
            |row| row.get(0),
        ).unwrap_or(false);

        if exists {
            continue;
        }

        let team_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO teams (id, name, project_id, coordination_mode, max_agents, is_template)
             VALUES (?1, ?2, '__global__', ?3, ?4, 1)",
            params![team_id, name, mode, slots.len() as i64],
        )?;

        for (order, (role, runtime, model)) in slots.into_iter().enumerate() {
            let slot_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, slot_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![slot_id, team_id, role, runtime, model, order as i64],
            )?;
        }

        tracing::info!("Seeded team template: {}", name);
    }

    Ok(())
}
```

**Step 2: Register module**

In `src/db/mod.rs`, add:
```rust
pub mod seeds;
```

**Step 3: Call from main.rs**

In `src/main.rs`, add after the settings seeding block (after line 65):

```rust
// Seed team templates
{
    let conn = db.lock().unwrap();
    crate::db::seeds::seed_team_templates(&conn).unwrap_or_else(|e| {
        tracing::warn!("Failed to seed team templates: {}", e);
    });
}
```

**Step 4: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 5: Commit**

```bash
git add src/db/seeds.rs src/db/mod.rs src/main.rs
git commit -m "feat: seed 6 preset team templates on startup"
```

---

### Task 6: Add template list and clone API endpoints

**Files:**
- Modify: `src/models/team.rs` (add `list_templates()` and `clone_into_project()`)
- Modify: `src/api/teams.rs` (add handlers)
- Modify: `src/main.rs` (add routes)

**Step 1: Write failing tests**

Add to tests module in `src/models/team.rs`:

```rust
#[test]
fn test_list_templates() {
    let conn = setup_db();
    // Create a global template
    conn.execute(
        "INSERT INTO teams (id, name, project_id, coordination_mode, is_template) VALUES ('t1', 'Template', '__global__', 'pipeline', 1)",
        [],
    ).unwrap();

    let templates = Team::list_templates(&conn, None).unwrap();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "Template");
}

#[test]
fn test_clone_template() {
    let conn = setup_db();
    let project = create_test_project(&conn);

    // Create a global template with slots
    conn.execute(
        "INSERT INTO teams (id, name, project_id, coordination_mode, is_template, max_agents) VALUES ('t1', 'Template', '__global__', 'pipeline', 1, 3)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, slot_order) VALUES ('s1', 't1', 'coder', 'claude', 'claude-sonnet-4-6', 0)",
        [],
    ).unwrap();

    let cloned = Team::clone_into_project(&conn, "t1", &project.id).unwrap();
    assert_eq!(cloned.name, "Template");
    assert_eq!(cloned.project_id, project.id);
    assert!(!cloned.is_template);

    let slots = TeamAgentSlot::list_by_team(&conn, &cloned.id).unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].role, "coder");
    assert_eq!(slots[0].model.as_deref(), Some("claude-sonnet-4-6"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib models::team::tests::test_list_templates -- --nocapture`
Expected: FAIL

**Step 3: Implement list_templates and clone_into_project**

Add to `impl Team`:

```rust
pub fn list_templates(conn: &Connection, project_id: Option<&str>) -> Result<Vec<Self>> {
    let mut templates = Vec::new();
    // Global templates
    let mut stmt = conn.prepare(
        "SELECT * FROM teams WHERE project_id = '__global__' AND is_template = 1 ORDER BY name"
    )?;
    let rows = stmt.query_map([], Self::from_row)?;
    for row in rows {
        templates.push(row?);
    }
    // Project-specific templates
    if let Some(pid) = project_id {
        let mut stmt = conn.prepare(
            "SELECT * FROM teams WHERE project_id = ?1 AND is_template = 1 ORDER BY name"
        )?;
        let rows = stmt.query_map(params![pid], Self::from_row)?;
        for row in rows {
            templates.push(row?);
        }
    }
    Ok(templates)
}

pub fn clone_into_project(conn: &Connection, template_id: &str, project_id: &str) -> Result<Self> {
    let template = Self::get_by_id(conn, template_id)?;
    let new_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO teams (id, name, project_id, coordination_mode, max_agents, token_budget, cost_budget_daily, is_template)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
        params![new_id, template.name, project_id, template.coordination_mode, template.max_agents, template.token_budget, template.cost_budget_daily],
    )?;

    // Clone slots
    let slots = TeamAgentSlot::list_by_team(conn, template_id)?;
    for slot in slots {
        let slot_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, config, slot_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![slot_id, new_id, slot.role, slot.runtime, slot.model, slot.config, slot.slot_order],
        )?;
    }

    Self::get_by_id(conn, &new_id)
}
```

**Step 4: Add API handlers**

In `src/api/teams.rs`:

```rust
pub async fn list_templates(
    State(state): State<AppState>,
) -> Json<Vec<Team>> {
    let conn = state.db.lock().unwrap();
    Json(Team::list_templates(&conn, None).unwrap_or_default())
}

pub async fn list_project_templates(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Json<Vec<Team>> {
    let conn = state.db.lock().unwrap();
    Json(Team::list_templates(&conn, Some(&project_id)).unwrap_or_default())
}

pub async fn clone_template(
    State(state): State<AppState>,
    Path((project_id, template_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Team>), StatusCode> {
    let conn = state.db.lock().unwrap();
    Team::clone_into_project(&conn, &template_id, &project_id)
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

**Step 5: Wire routes in main.rs**

Add these routes in the teams section:

```rust
.route("/api/teams/templates", get(api::teams::list_templates))
.route("/api/projects/{pid}/teams/templates", get(api::teams::list_project_templates))
.route("/api/projects/{pid}/teams/from-template/{tid}", post(api::teams::clone_template))
```

**Step 6: Run tests and verify compilation**

Run: `cargo test --lib models::team::tests -- --nocapture && cargo check`
Expected: ALL PASS

**Step 7: Commit**

```bash
git add src/models/team.rs src/api/teams.rs src/main.rs
git commit -m "feat: add template list and clone endpoints"
```

---

### Task 7: Update frontend API types and methods

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add model to TeamAgentSlot types**

Update the existing interfaces — there's no `TeamAgentSlot` interface yet, so add it. Also add `UpdateTeamAgentSlot` and model constants.

Add after the `CreateTeam` interface:

```typescript
export interface TeamAgentSlot {
  id: string;
  team_id: string;
  role: string;
  runtime: string;
  model: string | null;
  config: string;
  slot_order: number;
}

export interface CreateTeamAgentSlot {
  role: string;
  runtime: string;
  model?: string;
  config?: string;
  slot_order?: number;
}

export interface UpdateTeamAgentSlot {
  role?: string;
  runtime?: string;
  model?: string | null;
  slot_order?: number;
}

export const RUNTIME_MODELS: Record<string, string[]> = {
  claude: ['claude-sonnet-4-6', 'claude-opus-4-6', 'claude-haiku-4-5-20251001'],
  opencode: [],
  gemini: ['gemini-2.5-pro', 'gemini-2.5-flash'],
};
```

**Step 2: Update teams API object**

Replace the existing `teams` export with:

```typescript
export const teams = {
  list: (projectId: string) => get<Team[]>(`/projects/${projectId}/teams`),
  get: (projectId: string, id: string) => get<Team>(`/projects/${projectId}/teams/${id}`),
  create: (projectId: string, data: CreateTeam) => post<Team>(`/projects/${projectId}/teams`, data),
  delete: (projectId: string, id: string) => del(`/projects/${projectId}/teams/${id}`),
  templates: () => get<Team[]>('/teams/templates'),
  projectTemplates: (projectId: string) => get<Team[]>(`/projects/${projectId}/teams/templates`),
  cloneTemplate: (projectId: string, templateId: string) => post<Team>(`/projects/${projectId}/teams/from-template/${templateId}`, {}),
  slots: {
    list: (teamId: string) => get<TeamAgentSlot[]>(`/teams/${teamId}/slots`),
    create: (teamId: string, data: CreateTeamAgentSlot) => post<TeamAgentSlot>(`/teams/${teamId}/slots`, data),
    update: (teamId: string, id: string, data: UpdateTeamAgentSlot) => put<TeamAgentSlot>(`/teams/${teamId}/slots/${id}`, data),
    delete: (teamId: string, id: string) => del(`/teams/${teamId}/slots/${id}`),
  },
};
```

**Step 3: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Success

**Step 4: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add team slot types, templates, and model constants to frontend API"
```

---

### Task 8: Build team template picker UI

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte` (teams tab section)

**Step 1: Add template picker**

Replace the existing team creation form in the teams tab with a template picker that:

1. Loads templates via `teams.projectTemplates(projectId)` on mount
2. Shows a grid of template cards (name, mode badge, slot count)
3. Has a "Custom Team" card that opens the existing creation form
4. Clicking a template calls `teams.cloneTemplate(projectId, templateId)` and refreshes the team list

**Step 2: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Success

**Step 3: Commit**

```bash
git add frontend/src/routes/ProjectDetail.svelte
git commit -m "feat: add team template picker UI"
```

---

### Task 9: Build slot management UI

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte` (teams tab section)

**Step 1: Add slot management within team detail**

When a team is expanded/selected, show:

1. List of slots with role, runtime badge, model badge
2. "Add Slot" button that opens a form with:
   - Role (text input)
   - Runtime (dropdown: claude / opencode / gemini)
   - Model (dropdown populated from `RUNTIME_MODELS[selectedRuntime]` + custom text entry)
3. Each slot has edit (pencil) and delete (trash) buttons
4. Edit mode uses `teams.slots.update()` to save changes
5. Model dropdown dynamically changes when runtime changes

**Step 2: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Success

**Step 3: Commit**

```bash
git add frontend/src/routes/ProjectDetail.svelte
git commit -m "feat: add slot management UI with model selection"
```

---

### Task 10: Build, deploy, and E2E verify

**Files:**
- No new files — build and deploy only

**Step 1: Build frontend on server**

```bash
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave/frontend && npm run build'
```

**Step 2: Rsync source and build on server**

```bash
rsync -az --exclude 'target/' --exclude 'node_modules/' --exclude 'frontend/dist/' . paddy@10.202.28.205:/home/paddy/ironweave/
ssh paddy@10.202.28.205 'cd /home/paddy/ironweave/frontend && npm run build'
ssh paddy@10.202.28.205 'source ~/.cargo/env; cd /home/paddy/ironweave && cargo clean -p ironweave && cargo build --release'
```

**Step 3: Restart service**

```bash
ssh paddy@10.202.28.205 'sudo -n /usr/bin/systemctl restart ironweave'
```

**Step 4: E2E verify**

1. Check templates seeded: `curl -sk https://10.202.28.205/api/teams/templates | python3 -m json.tool`
2. Clone a template: `curl -sk -X POST https://10.202.28.205/api/projects/{pid}/teams/from-template/{tid}`
3. List slots: `curl -sk https://10.202.28.205/api/teams/{new_team_id}/slots`
4. Update a slot model: `curl -sk -X PUT https://10.202.28.205/api/teams/{tid}/slots/{sid} -H 'Content-Type: application/json' -d '{"model": "claude-opus-4-6"}'`
5. Open the UI and verify template picker + slot management work

**Step 5: Commit any deploy fixes**

```bash
git add -A
git commit -m "fix: deploy adjustments for teams and model selection"
```
