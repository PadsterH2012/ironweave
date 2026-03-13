# Agent Context Injection + Plan Importer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Give team agents CLAUDE.md + file tree context in their prompts, and add an API endpoint to bulk-import plan tasks as issues.

**Architecture:** Two independent features sharing no code. Feature 1 adds a context helper module called from `spawn_team_agent()`. Feature 2 adds a plan parser module and API endpoint. Both are additive — no existing behaviour changes.

**Tech Stack:** Rust (std::fs, regex), Axum handlers, rusqlite, existing Issue model

---

### Task 1: Create the context helper module

**Files:**
- Create: `src/orchestrator/context.rs`
- Modify: `src/orchestrator/mod.rs:1-4`

**Step 1: Create `src/orchestrator/context.rs` with both helper functions**

```rust
use std::path::Path;

const CLAUDE_MD_MAX_BYTES: usize = 8192;
const FILE_TREE_MAX_LINES: usize = 200;

const EXCLUDED_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "dist", ".next",
    "__pycache__", ".venv", "venv", ".svelte-kit",
];

/// Read CLAUDE.md from a directory, truncated to 8KB
pub fn read_claude_md(dir: &Path) -> Option<String> {
    let path = dir.join("CLAUDE.md");
    let content = std::fs::read_to_string(&path).ok()?;
    if content.len() > CLAUDE_MD_MAX_BYTES {
        Some(content[..CLAUDE_MD_MAX_BYTES].to_string() + "\n\n[...truncated at 8KB]")
    } else {
        Some(content)
    }
}

/// Generate an indented file tree listing, capped at 200 lines
pub fn generate_file_tree(dir: &Path) -> String {
    let mut lines = Vec::new();
    walk_dir(dir, dir, 0, &mut lines);
    if lines.len() > FILE_TREE_MAX_LINES {
        lines.truncate(FILE_TREE_MAX_LINES);
        lines.push("[...truncated at 200 lines]".to_string());
    }
    lines.join("\n")
}

fn walk_dir(base: &Path, dir: &Path, depth: usize, lines: &mut Vec<String>) {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip excluded directories
        if EXCLUDED_DIRS.contains(&name.as_str()) {
            continue;
        }

        if lines.len() >= FILE_TREE_MAX_LINES + 10 {
            return; // Early exit to avoid unnecessary work
        }

        let indent = "  ".repeat(depth);
        let file_type = entry.file_type().unwrap_or_else(|_| {
            // Fallback — treat as file
            std::fs::metadata(entry.path())
                .map(|m| m.file_type())
                .unwrap_or_else(|_| entry.file_type().unwrap())
        });

        if file_type.is_dir() {
            lines.push(format!("{}{}/", indent, name));
            walk_dir(base, &entry.path(), depth + 1, lines);
        } else {
            lines.push(format!("{}{}", indent, name));
        }
    }
}
```

**Step 2: Add module to `src/orchestrator/mod.rs`**

Add `pub mod context;` after the existing module declarations. Current content:

```rust
pub mod engine;
pub mod runner;
pub mod state_machine;
pub mod swarm;
```

Add:

```rust
pub mod context;
```

**Step 3: Run the build to verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: no errors

**Step 4: Commit**

```bash
git add src/orchestrator/context.rs src/orchestrator/mod.rs
git commit -m "feat: add orchestrator context helpers (read_claude_md, generate_file_tree)"
```

---

### Task 2: Wire context injection into spawn_team_agent

**Files:**
- Modify: `src/orchestrator/runner.rs:866-887`

**Step 1: Add context import at the top of runner.rs**

Near the existing `use` statements in `runner.rs`, add:

```rust
use crate::orchestrator::context;
```

Note: If `runner.rs` already imports from `crate::orchestrator`, just add the `context` module reference.

**Step 2: Read context before prompt construction**

In `spawn_team_agent()`, after line 841 (where `working_dir` is determined), insert context reading:

```rust
        // Read project context for the agent prompt
        let working_path = std::path::Path::new(&working_dir);
        let claude_md = context::read_claude_md(working_path)
            .unwrap_or_default();
        let file_tree = context::generate_file_tree(working_path);
```

**Step 3: Update the prompt format string**

Replace the prompt construction at lines 868-887 with:

```rust
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
```

**Step 4: Run the build to verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: no errors

**Step 5: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: inject CLAUDE.md and file tree into team agent prompts"
```

---

### Task 3: Create the plan parser module

**Files:**
- Create: `src/orchestrator/plan_parser.rs`

**Step 1: Create `src/orchestrator/plan_parser.rs`**

```rust
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ParsedTask {
    pub task_number: usize,
    pub title: String,
    pub description: String,
    pub role: Option<String>,
    pub depends_on_task_numbers: Vec<usize>,
}

/// Parse a plan markdown file into a list of tasks.
///
/// Splits on `### Task N: Title` headings. Each task body is the description.
/// Supports optional `**Role:**` and `**Depends on:**` lines within task body.
/// If no explicit depends_on, each task depends on the previous one (sequential).
pub fn parse_plan(content: &str) -> Vec<ParsedTask> {
    let heading_re = Regex::new(r"(?m)^### Task (\d+):\s*(.+)$").unwrap();
    let role_re = Regex::new(r"(?m)^\*\*Role:\*\*\s*(.+)$").unwrap();
    let depends_re = Regex::new(r"(?m)^\*\*Depends on:\*\*\s*(.+)$").unwrap();

    let mut tasks = Vec::new();
    let matches: Vec<_> = heading_re.find_iter(content).collect();

    for (i, mat) in matches.iter().enumerate() {
        let caps = heading_re.captures(mat.as_str()).unwrap();
        let task_number: usize = caps[1].parse().unwrap_or(i + 1);
        let title = caps[2].trim().to_string();

        // Extract body between this heading and the next (or end of file)
        let body_start = mat.end();
        let body_end = matches.get(i + 1).map(|m| m.start()).unwrap_or(content.len());
        let body = content[body_start..body_end].trim().to_string();

        // Check for **Role:** override
        let role = role_re.captures(&body).map(|c| c[1].trim().to_string());

        // Check for **Depends on:** override
        let depends_on_task_numbers = if let Some(dep_caps) = depends_re.captures(&body) {
            let dep_str = dep_caps[1].trim();
            // Parse comma-separated task numbers like "1, 3"
            dep_str
                .split(',')
                .filter_map(|s| {
                    s.trim()
                        .trim_start_matches("Task ")
                        .trim()
                        .parse::<usize>()
                        .ok()
                })
                .collect()
        } else if task_number > 1 {
            // Default: sequential dependency on previous task
            vec![task_number - 1]
        } else {
            vec![]
        };

        tasks.push(ParsedTask {
            task_number,
            title,
            description: body,
            role,
            depends_on_task_numbers,
        });
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_plan() {
        let plan = r#"
# Test Plan

### Task 1: Set up database

Create the schema.

### Task 2: Build API

**Depends on:** 1

Implement REST endpoints.

### Task 3: Add tests

**Role:** tester

Write integration tests.
"#;

        let tasks = parse_plan(plan);
        assert_eq!(tasks.len(), 3);

        assert_eq!(tasks[0].task_number, 1);
        assert_eq!(tasks[0].title, "Set up database");
        assert!(tasks[0].depends_on_task_numbers.is_empty());
        assert!(tasks[0].role.is_none());

        assert_eq!(tasks[1].task_number, 2);
        assert_eq!(tasks[1].title, "Build API");
        assert_eq!(tasks[1].depends_on_task_numbers, vec![1]);

        assert_eq!(tasks[2].task_number, 3);
        assert_eq!(tasks[2].title, "Add tests");
        assert_eq!(tasks[2].role.as_deref(), Some("tester"));
        // Default sequential dep on task 2
        assert_eq!(tasks[2].depends_on_task_numbers, vec![2]);
    }

    #[test]
    fn test_parse_empty_plan() {
        let plan = "# No tasks here\n\nJust some text.";
        let tasks = parse_plan(plan);
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_multi_depends() {
        let plan = r#"
### Task 1: A

Do A.

### Task 2: B

Do B.

### Task 3: C

**Depends on:** 1, 2

Do C after A and B.
"#;

        let tasks = parse_plan(plan);
        assert_eq!(tasks[2].depends_on_task_numbers, vec![1, 2]);
    }
}
```

**Step 2: Add module to `src/orchestrator/mod.rs`**

Add `pub mod plan_parser;` to `src/orchestrator/mod.rs`.

**Step 3: Add `regex` to Cargo.toml if not already present**

Run: `grep '^regex' Cargo.toml`

If not found, add `regex = "1"` to `[dependencies]`.

**Step 4: Run tests**

Run: `cargo test plan_parser 2>&1 | tail -20`
Expected: 3 tests passing

**Step 5: Commit**

```bash
git add src/orchestrator/plan_parser.rs src/orchestrator/mod.rs Cargo.toml
git commit -m "feat: add plan parser module with tests"
```

---

### Task 4: Create the plan import API endpoint

**Files:**
- Create: `src/api/plan_import.rs`
- Modify: `src/api/mod.rs:1-13`
- Modify: `src/main.rs:198` (add route)

**Step 1: Create `src/api/plan_import.rs`**

```rust
use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::issue::{Issue, CreateIssue};
use crate::models::project::Project;
use crate::orchestrator::plan_parser;

#[derive(Debug, Deserialize)]
pub struct ImportPlanRequest {
    pub plan_path: String,
    pub default_role: Option<String>,
    pub default_priority: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ImportedIssue {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportPlanResponse {
    pub imported: usize,
    pub issues: Vec<ImportedIssue>,
}

pub async fn import_plan(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(input): Json<ImportPlanRequest>,
) -> Result<(StatusCode, Json<ImportPlanResponse>), (StatusCode, String)> {
    let default_role = input.default_role.unwrap_or_else(|| "senior_coder".to_string());
    let default_priority = input.default_priority.unwrap_or(5);

    // Get project directory
    let project_dir = {
        let conn = state.db.lock().unwrap();
        let project = Project::get_by_id(&conn, &project_id)
            .map_err(|_| (StatusCode::NOT_FOUND, "Project not found".to_string()))?;
        project.directory
    };

    // Read plan file
    let plan_file = std::path::Path::new(&project_dir).join(&input.plan_path);
    let content = std::fs::read_to_string(&plan_file)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Cannot read plan file: {}", e)))?;

    // Parse tasks
    let parsed = plan_parser::parse_plan(&content);
    if parsed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No tasks found in plan file".to_string()));
    }

    // Create issues, mapping task numbers → UUIDs for dependency wiring
    let mut task_to_uuid: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let mut imported_issues = Vec::new();

    let conn = state.db.lock().unwrap();

    for task in &parsed {
        // Resolve dependencies: map task numbers to already-created UUIDs
        let depends_on: Vec<String> = task
            .depends_on_task_numbers
            .iter()
            .filter_map(|n| task_to_uuid.get(n).cloned())
            .collect();

        let role = task.role.clone().unwrap_or_else(|| default_role.clone());

        let create_input = CreateIssue {
            project_id: project_id.clone(),
            issue_type: Some("task".to_string()),
            title: task.title.clone(),
            description: Some(task.description.clone()),
            priority: Some(default_priority),
            depends_on: if depends_on.is_empty() { None } else { Some(depends_on.clone()) },
            workflow_instance_id: None,
            stage_id: None,
            role: Some(role),
            parent_id: None,
            needs_intake: Some(0),
            scope_mode: Some("auto".to_string()),
        };

        let issue = Issue::create(&conn, &create_input)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create issue: {}", e)))?;

        task_to_uuid.insert(task.task_number, issue.id.clone());

        imported_issues.push(ImportedIssue {
            id: issue.id,
            title: task.title.clone(),
            depends_on,
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(ImportPlanResponse {
            imported: imported_issues.len(),
            issues: imported_issues,
        }),
    ))
}
```

**Step 2: Add module to `src/api/mod.rs`**

Add `pub mod plan_import;` to `src/api/mod.rs`.

**Step 3: Add route to `src/main.rs`**

After line 198 (the last `.route(...)` before the auth middleware), add:

```rust
        // Plan import
        .route("/api/projects/{pid}/import-plan", post(api::plan_import::import_plan))
```

**Step 4: Run the build to verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: no errors

**Step 5: Commit**

```bash
git add src/api/plan_import.rs src/api/mod.rs src/main.rs
git commit -m "feat: add plan import API endpoint (POST /api/projects/{pid}/import-plan)"
```

---

### Task 5: Build, deploy, and verify

**Files:**
- None (deploy only)

**Step 1: Run full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: all tests pass, including new `plan_parser` tests

**Step 2: Build release binary**

Run: `cargo build --release 2>&1 | tail -5`
Expected: successful build

**Step 3: Deploy to hl-ironweave**

```bash
rsync -avz --delete \
  --exclude '.git' --exclude 'target' --exclude 'node_modules' \
  /Users/paddyharker/task2/ \
  paddyharker@10.202.28.205:/opt/ironweave/

ssh paddyharker@10.202.28.205 'cd /opt/ironweave && cargo clean -p ironweave && cargo build --release'
ssh paddyharker@10.202.28.205 'sudo systemctl restart ironweave'
```

**Step 4: Verify plan import endpoint**

```bash
curl -sk -X POST https://10.202.28.205/api/projects/{TEST_PROJECT_ID}/import-plan \
  -H 'Content-Type: application/json' \
  -d '{"plan_path": "docs/plans/2026-03-13-agent-context-plan-importer-plan.md", "default_role": "senior_coder"}'
```

Expected: 201 response with imported task count and issue list.

**Step 5: Commit any final adjustments**

```bash
git add -A
git commit -m "chore: final adjustments after deploy verification"
```
