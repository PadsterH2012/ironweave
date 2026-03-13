# Agent Context Injection + Plan Importer Design

> **Date:** 2026-03-13
> **Status:** Approved
> **Goal:** Give team agents codebase awareness and provide an API to bulk-import plan tasks as issues

---

## Motivation

Team agents currently receive a bare prompt: role name, task title, description, and curl commands. They have zero knowledge of project conventions, file structure, or architecture. This makes them ineffective on unfamiliar codebases.

Additionally, there is no way to import a structured implementation plan into the issue tracker — each task must be created manually. For self-bootstrapping scenarios (a cloned Ironweave instance finishing its own build), both features are prerequisites.

---

## Feature 1: Agent Context Injection

### What Changes

In `spawn_team_agent()`, before constructing the prompt, two pieces of context are read from the agent's working directory (the worktree path):

1. **CLAUDE.md** — Read `{worktree_path}/CLAUDE.md` if it exists, capped at 8KB
2. **File tree** — Walk the directory excluding `.git`, `node_modules`, `target`, `dist`, capped at 200 lines

### Prompt Structure (after)

```
You are a {role} agent working on project {project_name}.

## Project Guidelines
{claude_md_contents}

## Project Structure
{file_tree}

## Your Task
**{issue.title}**

{issue.description}

[curl instructions to close/update issue]
```

### New Module

`src/orchestrator/context.rs` with two public functions:

- `read_claude_md(dir: &Path) -> Option<String>` — Reads and truncates CLAUDE.md
- `generate_file_tree(dir: &Path) -> String` — Walks directory with exclusion list, returns indented tree listing

### Scope

- **Applies to:** `spawn_team_agent()` only
- **Does not apply to:** Intake agents (already have git log + file tree), stage agents (keep existing behaviour)

### Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Cap CLAUDE.md at 8KB | Yes | Prevents prompt bloat; most CLAUDE.md files are 1-4KB |
| Cap file tree at 200 lines | Yes | Deep repos could produce thousands of lines |
| Exclude dirs | `.git`, `node_modules`, `target`, `dist` | Standard build/dependency directories add noise |
| Read from worktree path | Yes | Worktree is the agent's working directory and has the right branch content |

---

## Feature 2: Plan Importer API

### Endpoint

```
POST /api/projects/{pid}/import-plan
```

### Request Body

```json
{
  "plan_path": "docs/plans/2026-03-13-feature-plan.md",
  "default_role": "senior_coder",
  "default_priority": 5
}
```

- `plan_path` — Relative to the project's directory
- `default_role` — Applied to all tasks unless overridden per-task
- `default_priority` — Applied to all tasks (default: 5)

### Response

```json
{
  "imported": 7,
  "issues": [
    { "id": "uuid-1", "title": "Task 1: Hook installation", "depends_on": [] },
    { "id": "uuid-2", "title": "Task 2: Recovery modes", "depends_on": ["uuid-1"] }
  ]
}
```

### Parser Logic

New module `src/orchestrator/plan_parser.rs`:

1. Read file from `{project_directory}/{plan_path}`
2. Split on `### Task N:` headings (regex: `^### Task \d+:\s*(.+)`)
3. For each task extract:
   - **Title** — Heading text after `Task N:`
   - **Description** — Everything between this heading and the next `### Task` heading
   - **Role** — `**Role:**` line if present, else `default_role`
   - **Dependencies** — Sequential by default (Task N depends on Task N-1), unless a `**Depends on:**` line specifies task numbers
4. Create issues via `Issue::create()` in task-number order, mapping task numbers → UUIDs for dependency wiring
5. All issues created with `needs_intake = 0` (already decomposed)

### API Handler

New file `src/api/plan_import.rs` with a single `import_plan()` handler.

Route added in `main.rs`:
```rust
.route("/api/projects/{pid}/import-plan", post(api::plan_import::import_plan))
```

### Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Explicit API (not auto-detect) | Yes | Predictable; clone can call during bootstrap |
| Sequential dependencies by default | Yes | Plan tasks are ordered; override via `**Depends on:**` |
| `needs_intake = 0` for all imports | Yes | Plan tasks are already decomposed into actionable work |
| Read plan from project directory | Yes | Plans live in the repo alongside code |
| Default role with per-task override | Yes | Simple; `**Role:**` line in plan markdown is optional |

---

## Files to Create/Modify

| Action | File |
|--------|------|
| Create | `src/orchestrator/context.rs` |
| Create | `src/orchestrator/plan_parser.rs` |
| Create | `src/api/plan_import.rs` |
| Modify | `src/orchestrator/runner.rs` — call context functions in `spawn_team_agent()` |
| Modify | `src/orchestrator/mod.rs` — add `pub mod context; pub mod plan_parser;` |
| Modify | `src/api/mod.rs` — add `pub mod plan_import;` |
| Modify | `src/main.rs` — add import-plan route |
