# F18: Knowledge Graph Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a pattern extraction and query system that learns from agent work and surfaces relevant knowledge to agents on-demand.

**Architecture:** New `knowledge_patterns` table + `KnowledgePattern` model with CRUD + hybrid search. API endpoints for query/create/manage. Real-time extraction hooks in orchestrator sweep handlers. Batch extraction every 10 minutes. Frontend Knowledge tab on ProjectDetail. Playwright e2e tests alongside each component.

**Tech Stack:** Rust/Axum (backend), SQLite (storage), Svelte 5 (frontend), Playwright (tests)

---

### Task 1: Database Migration — knowledge_patterns table

**Files:**
- Modify: `src/db/migrations.rs`

**Step 1: Add migration test (TDD — write test first)**

Add to `#[cfg(test)] mod tests` in `src/db/migrations.rs`:

```rust
#[test]
fn test_knowledge_patterns_table_exists() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'proj', '/tmp', 'work')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO knowledge_patterns (id, project_id, pattern_type, title, content, source_type, keywords)
         VALUES ('kp1', 'p1', 'solution', 'Test pattern', 'Test content', 'manual', '[]')",
        [],
    ).unwrap();

    let pt: String = conn
        .query_row("SELECT pattern_type FROM knowledge_patterns WHERE id = 'kp1'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pt, "solution");
}
```

**Step 2: Run test — verify it fails**

Run: `cargo test --lib db::migrations::tests::test_knowledge_patterns_table_exists`
Expected: FAIL (table doesn't exist)

**Step 3: Add migration**

Append before `Ok(())` in `run_migrations()`:

```rust
// ── Knowledge patterns table ─────────────────────────────────────
conn.execute_batch("
    CREATE TABLE IF NOT EXISTS knowledge_patterns (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        pattern_type TEXT NOT NULL CHECK(pattern_type IN ('solution', 'gotcha', 'preference', 'recipe')),
        role TEXT,
        task_type TEXT,
        keywords TEXT NOT NULL DEFAULT '[]',
        title TEXT NOT NULL,
        content TEXT NOT NULL,
        confidence REAL NOT NULL DEFAULT 0.5,
        observations INTEGER NOT NULL DEFAULT 1,
        source_type TEXT NOT NULL CHECK(source_type IN ('trace', 'performance', 'loom', 'manual')),
        source_id TEXT,
        files_involved TEXT,
        is_shared INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE INDEX IF NOT EXISTS idx_knowledge_project ON knowledge_patterns(project_id);
    CREATE INDEX IF NOT EXISTS idx_knowledge_type ON knowledge_patterns(pattern_type);
    CREATE INDEX IF NOT EXISTS idx_knowledge_role ON knowledge_patterns(role);
    CREATE INDEX IF NOT EXISTS idx_knowledge_shared ON knowledge_patterns(is_shared);
")?;
```

**Step 4: Run test — verify it passes**

Run: `cargo test --lib db::migrations::tests`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat(knowledge): add knowledge_patterns table migration"
```

---

### Task 2: KnowledgePattern Model — CRUD

**Files:**
- Create: `src/models/knowledge_pattern.rs`
- Modify: `src/models/mod.rs`

Create `src/models/knowledge_pattern.rs` with:

**Structs:**
- `KnowledgePattern` — all fields from table, Serialize/Deserialize
- `CreateKnowledgePattern` — project_id, pattern_type, role (Option), task_type (Option), keywords, title, content, source_type, source_id (Option), files_involved (Option), is_shared
- `UpdateKnowledgePattern` — all Optional fields for partial update
- `KnowledgeSearchQuery` — query (String), role (Option), task_type (Option), pattern_type (Option), files (Option<Vec<String>>), limit (Option)
- `KnowledgeSearchResult` — extends KnowledgePattern with `score: f64` and `source_project: String`

**Methods:**
- `from_row(row)` — deserialize from rusqlite Row
- `create(conn, input)` — insert with UUID
- `get_by_id(conn, id)` — single lookup
- `list_by_project(conn, project_id, pattern_type, role, limit)` — filtered list
- `search(conn, project_id, query)` — hybrid search (structured filter + keyword scoring)
- `search_cross_project(conn, query)` — search across opted-in shared projects
- `update(conn, id, input)` — partial update
- `delete(conn, id)`
- `merge_or_increment(conn, project_id, pattern_type, role, task_type, title)` — if similar pattern exists, bump observations + recalculate confidence; else create new
- `decay_confidence(conn, id)` — multiply confidence by 0.8

**Helper functions:**
- `extract_keywords(text: &str) -> Vec<String>` — split on whitespace/punctuation, lowercase, remove stopwords, deduplicate
- `keyword_overlap_score(query_keywords: &[String], pattern_keywords: &[String]) -> f64` — matched / total

**Tests (8+):**
1. `test_create_and_get` — create pattern, fetch, verify fields
2. `test_list_by_project` — create multiple, filter by type and role
3. `test_search_keyword_scoring` — create patterns with different keywords, search, verify ordering
4. `test_search_file_boost` — pattern with matching files scores higher
5. `test_search_confidence_weight` — higher confidence patterns rank higher
6. `test_merge_or_increment` — create, then merge similar, verify observations bumped
7. `test_decay_confidence` — create at 0.8, decay, verify 0.64
8. `test_cross_project_search` — create shared + non-shared patterns, verify cross-project only returns shared from opted-in projects
9. `test_extract_keywords` — verify keyword extraction from sample text
10. `test_keyword_overlap_score` — verify scoring math

Add `pub mod knowledge_pattern;` to `src/models/mod.rs`.

**Run:** `cargo test --lib models::knowledge_pattern`
**Commit:** `feat(knowledge): add KnowledgePattern model with CRUD and hybrid search`

---

### Task 3: API Handlers — Knowledge Endpoints

**Files:**
- Create: `src/api/knowledge.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`

**Handlers:**
- `list_patterns(Path(pid), Query(filters), State)` → `Vec<KnowledgePattern>`
- `get_pattern(Path((pid, id)), State)` → `KnowledgePattern`
- `create_pattern(Path(pid), Json(input), State)` → `(CREATED, KnowledgePattern)`
- `search_patterns(Path(pid), Json(query), State)` → `Vec<KnowledgeSearchResult>`
- `cross_project_search(Query(params), State)` → `Vec<KnowledgeSearchResult>`
- `update_pattern(Path((pid, id)), Json(input), State)` → `KnowledgePattern`
- `delete_pattern(Path((pid, id)), State)` → `NO_CONTENT`
- `trigger_extraction(Path(pid), State)` → `{extracted: i64}`

**Routes in main.rs:**
```rust
// Knowledge graph
.route("/api/projects/{pid}/knowledge", get(api::knowledge::list_patterns).post(api::knowledge::create_pattern))
.route("/api/projects/{pid}/knowledge/search", post(api::knowledge::search_patterns))
.route("/api/projects/{pid}/knowledge/extract", post(api::knowledge::trigger_extraction))
.route("/api/projects/{pid}/knowledge/{id}", get(api::knowledge::get_pattern).put(api::knowledge::update_pattern).delete(api::knowledge::delete_pattern))
.route("/api/knowledge/cross-project", get(api::knowledge::cross_project_search))
```

**Run:** `cargo check`
**Commit:** `feat(knowledge): add knowledge API endpoints`

---

### Task 4: Frontend API Client — knowledge object

**Files:**
- Modify: `frontend/src/lib/api.ts`

Add `KnowledgePattern`, `CreateKnowledgePattern`, `KnowledgeSearchQuery`, `KnowledgeSearchResult` interfaces and `knowledge` API object with: `list`, `get`, `create`, `search`, `crossProject`, `update`, `delete`, `extract`.

**Run:** `npm run build`
**Commit:** `feat(knowledge): add knowledge API client`

---

### Task 5: KnowledgePanel Component

**Files:**
- Create: `frontend/src/lib/components/KnowledgePanel.svelte`

Svelte 5 component with:
- Props: `projectId: string`
- State: patterns list, filters, selected pattern, show create form
- Top bar: pattern count, filter dropdowns (type, role, confidence), "Extract Now" button, "Add Pattern" button
- Main area: scrollable pattern cards with type badges, title, content preview, confidence bar, observations, shared badge
- Expandable detail view on click
- Create form: title, content, pattern_type select, role, task_type, keywords input, files input

**Run:** `npm run build`
**Commit:** `feat(knowledge): add KnowledgePanel component`

---

### Task 6: Wire Knowledge Tab into ProjectDetail

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte`

- Import `KnowledgePanel`
- Add `{ key: 'knowledge', label: 'Knowledge' }` to tabs (between Routing and Tests)
- Add tab content: `{:else if activeTab === 'knowledge'} <KnowledgePanel projectId={params.id} />`

**Run:** `npm run build`
**Commit:** `feat(knowledge): add Knowledge tab to project detail`

---

### Task 7: Keyword Extraction + Real-time Pattern Hooks

**Files:**
- Modify: `src/orchestrator/runner.rs`

Add methods to `OrchestratorRunner`:

**`extract_solution_pattern(&self, issue: &Issue, trace: Option<&WorkflowTrace>)`**
- Called when issue status changes to "closed" in sweep_teams
- Extracts keywords from issue title + description
- Builds content from trace steps summary (if trace exists) or issue summary
- Creates pattern via `KnowledgePattern::merge_or_increment`
- Sets `is_shared` based on project's `share_learning` flag

**`extract_gotcha_pattern(&self, issue: &Issue, loom_entries: &[LoomEntry])`**
- Called when agent is reaped as dead/failed
- Extracts keywords from issue + failure context
- Builds content from loom warning/escalation entries
- Creates gotcha pattern

**`extract_preference_pattern(&self, perf_log: &PerformanceLog)`**
- Called when routing override is accepted
- Records model preference for role + task_type

Hook these into the existing sweep handlers:
- In the issue close handler (after `Issue::update` to closed)
- In the agent reap handler (after marking session as dead)

**Run:** `cargo check`
**Commit:** `feat(knowledge): add real-time pattern extraction hooks`

---

### Task 8: Batch Extraction (10-minute sweep)

**Files:**
- Modify: `src/orchestrator/runner.rs`

Add `extract_knowledge_batch(&self)` method:
- Track last extraction time with a field on OrchestratorRunner
- In `sweep()`, check if 10 minutes have passed since last extraction
- Scan completed traces from last 10 minutes
- Group by role + task_type → extract `recipe` patterns (common step sequences)
- Scan performance logs → extract `preference` patterns (model success rates)
- Call `merge_or_increment` for each

**Run:** `cargo test --lib`
**Commit:** `feat(knowledge): add batch knowledge extraction every 10 minutes`

---

### Task 9: Agent Prompt — Knowledge API Instructions

**Files:**
- Modify: `src/orchestrator/runner.rs`

In `spawn_team_agent()`, after the loom reporting instructions (around line 1840), add knowledge query instructions:

```rust
prompt_parts.push(format!(
    "\n## Knowledge Base\n\
    Before starting work, check if similar tasks have been solved before:\n\
    curl -sk -X POST ${{IRONWEAVE_API}}/api/projects/{project_id}/knowledge/search \\\n  \
    -H 'Content-Type: application/json' \\\n  \
    -d '{{\"query\": \"<brief description of your task>\", \"role\": \"{role}\", \"task_type\": \"{task_type}\"}}'\n\n\
    This returns patterns from past work — solutions, gotchas, and recipes that may help.\n\
    Use this context to avoid repeating past mistakes and to follow proven approaches.",
    project_id = team.project_id,
    role = slot.role,
    task_type = issue.issue_type,
));
```

**Run:** `cargo check`
**Commit:** `feat(knowledge): add knowledge API instructions to agent prompts`

---

### Task 10: Playwright E2E Tests — Knowledge Tab

**Files:**
- Create: `tests/e2e/knowledge.spec.ts`
- Create: `tests/e2e/interact-knowledge.spec.ts`
- Create: `tests/e2e/fill-knowledge.spec.ts`

**`knowledge.spec.ts`** — Smoke tests:
- Knowledge tab renders on project detail
- Pattern list loads (empty or with data)
- Filter dropdowns exist (type, role)
- "Add Pattern" button exists
- "Extract Now" button exists

**`interact-knowledge.spec.ts`** — Interaction tests:
- Create a pattern via "Add Pattern" form, verify it appears in list
- Delete pattern via API, verify it's gone
- Click "Extract Now", verify it completes
- Search via API with role + query, verify scored results

**`fill-knowledge.spec.ts`** — API contract tests:
- GET `/api/projects/{pid}/knowledge` returns array
- POST `/api/projects/{pid}/knowledge` creates pattern (verify fields)
- POST `/api/projects/{pid}/knowledge/search` returns scored results
- GET `/api/knowledge/cross-project` returns array
- PUT `/api/projects/{pid}/knowledge/{id}` updates pattern
- DELETE `/api/projects/{pid}/knowledge/{id}` removes pattern
- POST `/api/projects/{pid}/knowledge/extract` returns `{extracted: N}`
- GET with invalid project returns 404

**Run:** Sync to server, trigger test run
**Commit:** `feat(knowledge): add Playwright e2e tests for knowledge graph`

---

### Task 11: Update Feature Checklists + Coverage Matrix

**Files:**
- Modify: `docs/BACKEND_FEATURES.md` — add knowledge endpoints + model
- Modify: `docs/FRONTEND_FEATURES.md` — add KnowledgePanel + Knowledge tab + API client
- Modify: `frontend/src/lib/components/TestRunPanel.svelte` — add Knowledge to coverage matrix
- Modify: `frontend/src/routes/Projects.svelte` — (if adding knowledge indicator to tiles)

**Run:** `python3 scripts/audit-features.py`
**Commit:** `docs: update checklists and coverage matrix for knowledge graph`

---

### Task 12: Update Obsidian Feature Tracker

**Files:**
- Modify: `/Volumes/Breakaway/obsidian/Homelab/Projects/A1 - Main Projects/Ironweave/Ironweave Feature Tracker.md`

Update F18 section from 💭 to ✅ for implemented items.

**Commit:** N/A (Obsidian vault, not in git)

---

### Task 13: Full Build + Deploy + Verify

**Step 1:** `cargo test --lib` — all Rust tests pass
**Step 2:** `npm run build` — frontend builds
**Step 3:** Deploy to prod + dev
**Step 4:** Trigger full e2e suite from Tests tab — all tests pass (211 existing + ~15 new = ~226+)
**Step 5:** Verify Knowledge tab renders in UI with screenshot

**Commit:** `feat(knowledge): complete F18 knowledge graph implementation`
