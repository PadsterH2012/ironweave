# Agent Stage Completion — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let orchestrator-spawned agents close their issues via a PATCH API endpoint, so stages complete instead of always failing.

**Architecture:** Add `UpdateIssue` struct + `Issue::update()` to the model, a PATCH handler in the issues API, wire the route in main.rs, pass `IRONWEAVE_API` env var + curl instructions in agent prompts, and add `issues.update()` to the frontend API client.

**Tech Stack:** Rust (Axum), SQLite (rusqlite), Svelte 5 (TypeScript), TDD

---

### Task 1: Add `UpdateIssue` struct and `Issue::update()` — test

**Files:**
- Modify: `src/models/issue.rs:162` (add test before closing `}` of tests module)

**Step 1: Write the failing test**

Add this test at the end of the `tests` module in `src/models/issue.rs`:

```rust
#[test]
fn test_update_issue() {
    let conn = setup_db();
    let project = create_test_project(&conn);
    let issue = Issue::create(&conn, &CreateIssue {
        project_id: project.id.clone(),
        issue_type: None,
        title: "Original title".to_string(),
        description: Some("Original desc".to_string()),
        priority: None,
        depends_on: None,
        workflow_instance_id: None,
        stage_id: None,
    }).unwrap();

    // Update status and summary
    let updated = Issue::update(&conn, &issue.id, &UpdateIssue {
        status: Some("closed".to_string()),
        title: None,
        description: None,
        summary: Some("Work complete".to_string()),
        priority: None,
    }).unwrap();

    assert_eq!(updated.status, "closed");
    assert_eq!(updated.summary.as_deref(), Some("Work complete"));
    assert_eq!(updated.title, "Original title"); // unchanged
    assert!(updated.updated_at != issue.updated_at); // timestamp bumped
}

#[test]
fn test_update_issue_not_found() {
    let conn = setup_db();
    let result = Issue::update(&conn, "nonexistent", &UpdateIssue {
        status: Some("closed".to_string()),
        title: None,
        description: None,
        summary: None,
        priority: None,
    });
    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib models::issue::tests::test_update_issue -- --nocapture`
Expected: FAIL — `UpdateIssue` and `Issue::update()` do not exist yet

---

### Task 2: Add `UpdateIssue` struct and `Issue::update()` — implement

**Files:**
- Modify: `src/models/issue.rs:36` (add `UpdateIssue` struct after `CreateIssue`)
- Modify: `src/models/issue.rs:161` (add `update()` method before closing `}` of impl block)

**Step 1: Add UpdateIssue struct**

Add after the `CreateIssue` struct (after line 36):

```rust
#[derive(Debug, Deserialize)]
pub struct UpdateIssue {
    pub status: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub priority: Option<i64>,
}
```

**Step 2: Add Issue::update() method**

Add before the closing `}` of the `impl Issue` block (before the `#[cfg(test)]` line):

```rust
pub fn update(conn: &Connection, id: &str, input: &UpdateIssue) -> Result<Self> {
    // Build SET clauses dynamically for provided fields
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref status) = input.status {
        sets.push("status = ?");
        values.push(Box::new(status.clone()));
    }
    if let Some(ref title) = input.title {
        sets.push("title = ?");
        values.push(Box::new(title.clone()));
    }
    if let Some(ref description) = input.description {
        sets.push("description = ?");
        values.push(Box::new(description.clone()));
    }
    if let Some(ref summary) = input.summary {
        sets.push("summary = ?");
        values.push(Box::new(summary.clone()));
    }
    if let Some(priority) = input.priority {
        sets.push("priority = ?");
        values.push(Box::new(priority));
    }

    if sets.is_empty() {
        return Self::get_by_id(conn, id);
    }

    // Always bump updated_at
    sets.push("updated_at = datetime('now')");

    let sql = format!("UPDATE issues SET {} WHERE id = ?", sets.join(", "));
    values.push(Box::new(id.to_string()));

    let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let changes = conn.execute(&sql, params.as_slice())?;
    if changes == 0 {
        return Err(IronweaveError::NotFound(format!("issue {}", id)));
    }
    Self::get_by_id(conn, id)
}
```

**Step 3: Run tests to verify they pass**

Run: `cargo test --lib models::issue::tests -- --nocapture`
Expected: ALL PASS including `test_update_issue` and `test_update_issue_not_found`

**Step 4: Commit**

```bash
git add src/models/issue.rs
git commit -m "feat: add UpdateIssue struct and Issue::update() method"
```

---

### Task 3: Add PATCH handler to issues API

**Files:**
- Modify: `src/api/issues.rs` (add `update` handler and import `UpdateIssue`)

**Step 1: Update the import**

Change line 4 of `src/api/issues.rs` from:
```rust
use crate::models::issue::{Issue, CreateIssue};
```
to:
```rust
use crate::models::issue::{Issue, CreateIssue, UpdateIssue};
```

**Step 2: Add the update handler**

Add after the `delete` function (after line 47):

```rust
pub async fn update(
    State(state): State<AppState>,
    Path((_pid, id)): Path<(String, String)>,
    Json(input): Json<UpdateIssue>,
) -> Result<Json<Issue>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Issue::update(&conn, &id, &input)
        .map(Json)
        .map_err(|e| match e {
            crate::error::IronweaveError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add src/api/issues.rs
git commit -m "feat: add PATCH handler for issue updates"
```

---

### Task 4: Wire PATCH route in main.rs

**Files:**
- Modify: `src/main.rs:125` (add `.patch()` to the issues/{id} route)

**Step 1: Add PATCH to the route**

Change line 125 of `src/main.rs` from:
```rust
.route("/api/projects/{pid}/issues/{id}", get(api::issues::get).delete(api::issues::delete))
```
to:
```rust
.route("/api/projects/{pid}/issues/{id}", get(api::issues::get).patch(api::issues::update).delete(api::issues::delete))
```

Also add `patch` to the routing import on line 17 — change:
```rust
use axum::{Router, middleware, routing::{get, post, put, delete}};
```
to:
```rust
use axum::{Router, middleware, routing::{get, post, put, patch, delete}};
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire PATCH route for /api/projects/{pid}/issues/{id}"
```

---

### Task 5: Update orchestrator prompt with curl instructions and IRONWEAVE_API env var

**Files:**
- Modify: `src/orchestrator/runner.rs:278-293` (update prompt and config in `spawn_stage_agent`)

**Step 1: Update the prompt template**

Replace the prompt building block (lines 279-282) with:

```rust
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
```

**Step 2: Add IRONWEAVE_API env var to agent config**

Replace the `environment: None,` line in the AgentConfig builder with:

```rust
environment: {
    let api_url = {
        let conn = db.lock().unwrap();
        crate::models::setting::Setting::get_by_key(&conn, "api_url")
            .map(|s| s.value)
            .unwrap_or_else(|_| "https://localhost:443".to_string())
    };
    let mut env = std::collections::HashMap::new();
    env.insert("IRONWEAVE_API".to_string(), api_url);
    Some(env)
},
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: pass IRONWEAVE_API env var and curl instructions to agents"
```

---

### Task 6: Add `issues.update()` to frontend API client

**Files:**
- Modify: `frontend/src/lib/api.ts` (add `UpdateIssue` interface and `update` method)

**Step 1: Add UpdateIssue interface**

Add after the `CreateIssue` interface (after line 109):

```typescript
export interface UpdateIssue {
  status?: string;
  title?: string;
  description?: string;
  summary?: string;
  priority?: number;
}
```

**Step 2: Add update method to issues object**

Add to the `issues` object (after the `create` line, line 402):

```typescript
update: (projectId: string, id: string, data: UpdateIssue) => patch<Issue>(`/projects/${projectId}/issues/${id}`, data),
```

**Step 3: Verify frontend builds**

Run: `cd frontend && npm run build`
Expected: Success

**Step 4: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add issues.update() to frontend API client"
```

---

### Task 7: Build, deploy, and E2E verify

**Files:**
- No new files — build and deploy only

**Step 1: Build frontend**

```bash
cd frontend && npm run build && cd ..
```

**Step 2: Clean cargo fingerprints and build release**

```bash
cargo clean -p ironweave && cargo build --release
```

**Step 3: Deploy to hl-ironweave**

```bash
rsync -az target/release/ironweave paddy@10.202.28.205:/home/paddy/ironweave/ironweave-new
ssh paddy@10.202.28.205 'sudo /usr/bin/systemctl stop ironweave && cp /home/paddy/ironweave/ironweave-new /home/paddy/ironweave/ironweave && sudo /usr/bin/systemctl start ironweave'
```

**Step 4: E2E verify the PATCH endpoint**

First, create a test issue to get an issue ID, then PATCH it:

```bash
# Test PATCH endpoint directly
curl -X PATCH https://10.202.28.205/api/projects/<project_id>/issues/<issue_id> \
  -H 'Content-Type: application/json' \
  -d '{"status": "closed", "summary": "Test completion"}'
```

Expected: 200 OK with updated issue JSON showing `status: "closed"` and `summary: "Test completion"`

**Step 5: Verify updated_at changed**

The returned JSON should show `updated_at` has been bumped from its original value.

**Step 6: Commit any deploy fixes if needed**

```bash
git add -A
git commit -m "fix: deploy adjustments for agent completion"
```
