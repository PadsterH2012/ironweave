# Project Details & Features Tabs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Details and Features tabs to every project — a versioned intent/reality document system and a full feature lifecycle tracker with auto-promotion, smart nudge, and implement-to-issue bridging.

**Architecture:** Three new tables (`features`, `feature_tasks`, `project_documents`). New API handlers for features CRUD + lifecycle actions, feature tasks CRUD + implement, and versioned documents. Two new Svelte components (`FeaturePanel`, `ProjectDetailsPanel`). Orchestrator hooks for auto-promotion and smart nudge. Dashboard widget for cross-project feature summary.

**Tech Stack:** Rust/Axum (backend), SQLite (storage), Svelte 5 (frontend), Playwright (tests)

---

### Task 1: Database Migrations — features, feature_tasks, project_documents

**Files:**
- Modify: `src/db/migrations.rs`

Add three tables at the end of `run_migrations()` before `Ok(())`:

**features:**
```sql
CREATE TABLE IF NOT EXISTS features (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'idea'
        CHECK(status IN ('idea', 'designed', 'in_progress', 'implemented', 'verified', 'parked', 'abandoned')),
    prd_content TEXT,
    implementation_notes TEXT,
    parked_at TEXT,
    parked_reason TEXT,
    priority INTEGER NOT NULL DEFAULT 5,
    keywords TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_features_project ON features(project_id);
CREATE INDEX IF NOT EXISTS idx_features_status ON features(status);
```

**feature_tasks:**
```sql
CREATE TABLE IF NOT EXISTS feature_tasks (
    id TEXT PRIMARY KEY,
    feature_id TEXT NOT NULL REFERENCES features(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo', 'done', 'skipped')),
    issue_id TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_feature_tasks_feature ON feature_tasks(feature_id);
```

**project_documents:**
```sql
CREATE TABLE IF NOT EXISTS project_documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    doc_type TEXT NOT NULL CHECK(doc_type IN ('intent', 'reality', 'changelog')),
    content TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 1,
    previous_content TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_by TEXT NOT NULL DEFAULT 'user'
);
CREATE INDEX IF NOT EXISTS idx_project_documents_project ON project_documents(project_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_project_documents_unique ON project_documents(project_id, doc_type);
```

Add migration tests for all three tables.

**Run:** `cargo test --lib db::migrations::tests`
**Commit:** `feat(details-features): add features, feature_tasks, project_documents migrations`

---

### Task 2: Feature Model — CRUD + lifecycle

**Files:**
- Create: `src/models/feature.rs`
- Modify: `src/models/mod.rs`

**Structs:** `Feature`, `CreateFeature`, `UpdateFeature`

**Methods:**
- `from_row`, `create`, `get_by_id`, `list_by_project(conn, project_id, status_filter: Option<&str>, limit)`, `update`, `delete` (soft — sets status to abandoned)
- `park(conn, id, reason)` — sets status=parked, parked_at, parked_reason
- `verify(conn, id)` — sets status=verified
- `update_status(conn, id, status)` — generic status update
- `summary(conn)` — cross-project counts: `Vec<{project_id, project_name, idea, designed, in_progress, implemented, verified, parked}>`
- `find_related_parked(conn, project_id, keywords: &[String], min_score: f64)` — uses `keyword_overlap_score` to find parked features matching given keywords

**Tests (5+):** create_and_get, list_with_filter, park_and_resume, soft_delete_to_abandoned, summary_cross_project, find_related_parked

**Run:** `cargo test --lib models::feature`
**Commit:** `feat(details-features): add Feature model with lifecycle`

---

### Task 3: FeatureTask Model — CRUD + implement

**Files:**
- Create: `src/models/feature_task.rs`
- Modify: `src/models/mod.rs`

**Structs:** `FeatureTask`, `CreateFeatureTask`, `UpdateFeatureTask`

**Methods:**
- `from_row`, `create`, `get_by_id`, `list_by_feature(conn, feature_id)`, `update`, `delete`
- `implement(conn, task_id, issue_id)` — sets issue_id on the task, linking it to an Ironweave issue
- `all_complete(conn, feature_id) -> bool` — returns true if all tasks for a feature are done or skipped (none todo)

**Tests (4+):** create_and_list, update_status, implement_links_issue, all_complete_check

**Run:** `cargo test --lib models::feature_task`
**Commit:** `feat(details-features): add FeatureTask model with implement`

---

### Task 4: ProjectDocument Model — versioned documents

**Files:**
- Create: `src/models/project_document.rs`
- Modify: `src/models/mod.rs`

**Structs:** `ProjectDocument`, `UpdateDocument`

**Methods:**
- `from_row`, `get_or_create(conn, project_id, doc_type)` — upsert: returns existing or creates empty
- `update_content(conn, project_id, doc_type, content, updated_by)` — saves previous_content, increments version, sets new content
- `get_history(conn, project_id, doc_type)` — returns current + previous versions
- `detect_removals(old_content, new_content) -> Vec<String>` — standalone function, returns lines present in old but missing in new

**Tests (4+):** get_or_create_idempotent, update_preserves_previous, version_increments, detect_removals_finds_missing_lines

**Run:** `cargo test --lib models::project_document`
**Commit:** `feat(details-features): add ProjectDocument model with versioning`

---

### Task 5: Feature API Handlers

**Files:**
- Create: `src/api/features.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`

**Handlers:**
- `list_features` — GET `/api/projects/{pid}/features` (query: status, limit)
- `get_feature` — GET `/api/projects/{pid}/features/{id}` (returns feature + tasks)
- `create_feature` — POST `/api/projects/{pid}/features`
- `update_feature` — PUT `/api/projects/{pid}/features/{id}`
- `delete_feature` — DELETE `/api/projects/{pid}/features/{id}` (soft delete)
- `park_feature` — POST `/api/projects/{pid}/features/{id}/park` (body: {reason})
- `verify_feature` — POST `/api/projects/{pid}/features/{id}/verify`
- `import_prd` — POST `/api/projects/{pid}/features/import` (body: {text})
- `feature_summary` — GET `/api/features/summary`

**Routes in main.rs** (before knowledge routes):
```rust
// Features
.route("/api/projects/{pid}/features", get(api::features::list_features).post(api::features::create_feature))
.route("/api/projects/{pid}/features/import", post(api::features::import_prd))
.route("/api/projects/{pid}/features/{id}", get(api::features::get_feature).put(api::features::update_feature).delete(api::features::delete_feature))
.route("/api/projects/{pid}/features/{id}/park", post(api::features::park_feature))
.route("/api/projects/{pid}/features/{id}/verify", post(api::features::verify_feature))
.route("/api/features/summary", get(api::features::feature_summary))
```

The `import_prd` handler should accept raw text and extract features + tasks. Simple approach: split on headings (##, ###) or numbered lists to create feature tasks. Store the raw text as `prd_content`.

**Run:** `cargo check`
**Commit:** `feat(details-features): add feature API endpoints`

---

### Task 6: Feature Task API Handlers

**Files:**
- Modify: `src/api/features.rs` (add task handlers to same file)
- Modify: `src/main.rs`

**Handlers:**
- `list_tasks` — GET `/api/features/{fid}/tasks`
- `create_task` — POST `/api/features/{fid}/tasks`
- `update_task` — PUT `/api/features/{fid}/tasks/{id}`
- `delete_task` — DELETE `/api/features/{fid}/tasks/{id}`
- `implement_task` — POST `/api/features/{fid}/tasks/{id}/implement`

The `implement_task` handler:
1. Gets the feature task
2. Gets the parent feature (for project_id, description context)
3. Creates an Ironweave issue via `Issue::create` with title from task, description from feature
4. Updates the feature_task with the new issue_id
5. Returns the created issue

**Routes:**
```rust
// Feature tasks
.route("/api/features/{fid}/tasks", get(api::features::list_tasks).post(api::features::create_task))
.route("/api/features/{fid}/tasks/{id}", put(api::features::update_task).delete(api::features::delete_task))
.route("/api/features/{fid}/tasks/{id}/implement", post(api::features::implement_task))
```

**Run:** `cargo check`
**Commit:** `feat(details-features): add feature task API with implement-to-issue`

---

### Task 7: Project Document API Handlers

**Files:**
- Create: `src/api/project_documents.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`

**Handlers:**
- `get_document` — GET `/api/projects/{pid}/documents/{type}`
- `update_document` — PUT `/api/projects/{pid}/documents/{type}` (body: {content, updated_by?})
- `get_history` — GET `/api/projects/{pid}/documents/{type}/history`
- `trigger_scan` — POST `/api/projects/{pid}/documents/scan` (placeholder — returns current reality or triggers agent)
- `get_gaps` — GET `/api/projects/{pid}/documents/gaps` (compares intent vs reality keywords)

The `update_document` handler should:
1. Get current document
2. Call `detect_removals` between old and new content
3. Store the update (preserving previous)
4. Return the updated document + any removals detected

**Routes:**
```rust
// Project documents
.route("/api/projects/{pid}/documents/scan", post(api::project_documents::trigger_scan))
.route("/api/projects/{pid}/documents/gaps", get(api::project_documents::get_gaps))
.route("/api/projects/{pid}/documents/{type}", get(api::project_documents::get_document).put(api::project_documents::update_document))
.route("/api/projects/{pid}/documents/{type}/history", get(api::project_documents::get_history))
```

Note: `/documents/scan` and `/documents/gaps` must come before `/documents/{type}` to avoid path capture.

**Run:** `cargo check`
**Commit:** `feat(details-features): add project document API with versioning`

---

### Task 8: Frontend API Client

**Files:**
- Modify: `frontend/src/lib/api.ts`

Add interfaces: `Feature`, `CreateFeature`, `FeatureTask`, `CreateFeatureTask`, `FeatureSummary`, `ProjectDocument`, `GapAnalysis`

Add API objects: `features` (list, get, create, update, delete, park, verify, import, summary), `featureTasks` (list, create, update, delete, implement), `projectDocuments` (get, update, history, scan, gaps)

**Run:** `npm run build`
**Commit:** `feat(details-features): add frontend API client for features and documents`

---

### Task 9: FeaturePanel Component

**Files:**
- Create: `frontend/src/lib/components/FeaturePanel.svelte`

Svelte 5 component matching the design in Section 5 of the design doc:
- Status filter tabs (All, Ideas, Designed, In Progress, Implemented, Verified, Parked)
- Add Feature + Import PRD buttons
- Expandable feature cards with status badges, task progress bars, description, PRD, task list
- Each task has checkbox + "Implement" button (creates issue via API)
- Park/Verify/Abandon action buttons
- Import PRD modal with textarea + preview

**Run:** `npm run build`
**Commit:** `feat(details-features): add FeaturePanel component`

---

### Task 10: ProjectDetailsPanel Component

**Files:**
- Create: `frontend/src/lib/components/ProjectDetailsPanel.svelte`

Split layout:
- Left: Intent editor (markdown textarea, save button, version indicator, removal warning)
- Right: Reality viewer (read-only rendered content, rescan button, timestamp)
- Bottom: Gap analysis (missing red, undocumented amber, "Create Feature" action)

**Run:** `npm run build`
**Commit:** `feat(details-features): add ProjectDetailsPanel component`

---

### Task 11: Wire Both Tabs into ProjectDetail

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte`

Add imports for both components. Add tabs at the BEGINNING of the tabs array:
```typescript
{ key: 'details', label: 'Details' },
{ key: 'features', label: 'Features' },
{ key: 'teams', label: 'Teams' },
...
```

Add tab content blocks.

**Run:** `npm run build`
**Commit:** `feat(details-features): wire Details and Features tabs into ProjectDetail`

---

### Task 12: Dashboard Features Widget

**Files:**
- Modify: `frontend/src/routes/Dashboard.svelte`

Add a features summary card using the `GET /api/features/summary` endpoint. Shows: "X ideas · Y in progress · Z implemented · W awaiting verification · V parked" with click-through links.

**Run:** `npm run build`
**Commit:** `feat(details-features): add features summary widget to dashboard`

---

### Task 13: Orchestrator — Auto-Promotion

**Files:**
- Modify: `src/orchestrator/runner.rs`

In the sweep loop, after issue status handling, add feature auto-promotion:

1. When an issue linked to a feature_task is closed → mark the feature_task as done
2. Check if all tasks for that feature are complete → if so, promote feature to "implemented"
3. When a feature_task gets its first issue_id → promote feature from "designed" to "in_progress"

This runs on each sweep cycle (30s). Query: find features where status is "designed" or "in_progress", check their tasks.

**Run:** `cargo check`
**Commit:** `feat(details-features): add feature auto-promotion in orchestrator sweep`

---

### Task 14: Orchestrator — Smart Nudge for Parked Features

**Files:**
- Modify: `src/orchestrator/runner.rs`

In `spawn_team_agent()`, after the knowledge base instructions, add smart nudge:

1. Extract keywords from the issue being worked on
2. Call `Feature::find_related_parked(conn, project_id, keywords, 0.5)`
3. If matches found, add to prompt: "Related parked features: {titles}"
4. Log to loom for visibility

**Run:** `cargo check`
**Commit:** `feat(details-features): add smart nudge for parked features in agent prompts`

---

### Task 15: Playwright E2E Tests — Features Tab

**Files:**
- Create: `tests/e2e/features.spec.ts`
- Create: `tests/e2e/interact-features.spec.ts`
- Create: `tests/e2e/fill-features.spec.ts`

**features.spec.ts** — smoke: tab renders, feature list, status filters, buttons
**interact-features.spec.ts** — create feature via API + verify in UI, create task, implement button creates issue, park, verify
**fill-features.spec.ts** — all API contracts (CRUD, park, verify, import, implement, summary)

**Commit:** `feat(details-features): add Playwright e2e tests for features`

---

### Task 16: Playwright E2E Tests — Details Tab

**Files:**
- Create: `tests/e2e/details.spec.ts`
- Create: `tests/e2e/interact-details.spec.ts`
- Create: `tests/e2e/fill-details.spec.ts`

**details.spec.ts** — smoke: tab renders, intent panel, reality panel, gap section
**interact-details.spec.ts** — save intent, version increments, gap analysis display
**fill-details.spec.ts** — document API contracts (get, put, history, scan, gaps)

**Commit:** `feat(details-features): add Playwright e2e tests for details`

---

### Task 17: Update Feature Checklists + Coverage Matrix

**Files:**
- Modify: `docs/BACKEND_FEATURES.md`
- Modify: `docs/FRONTEND_FEATURES.md`
- Modify: `frontend/src/lib/components/TestRunPanel.svelte`

Add features endpoints, feature_tasks endpoints, project_documents endpoints, Feature model, FeatureTask model, ProjectDocument model, FeaturePanel component, ProjectDetailsPanel component, Details/Features tabs, dashboard widget, and coverage matrix entries.

**Run:** `python3 scripts/audit-features.py`
**Commit:** `docs: update checklists and coverage for details + features tabs`

---

### Task 18: Update Obsidian Feature Tracker + Deploy

- Update `Ironweave Feature Tracker.md` with new feature entries
- Copy plan doc to Obsidian plans folder
- Full deploy to prod + dev
- Run full e2e suite — all tests pass
- Verify both tabs render in UI with screenshots

**Commit:** `feat(details-features): complete Details & Features tabs implementation`
