# F18: Knowledge Graph — Design

> **Date:** 2026-03-16
> **Status:** Approved
> **Goal:** Extract, store, and surface reusable patterns from past agent work so agents can query relevant context on-demand, improving task success rates and reducing wasted tokens.

---

## Decisions

- **Cross-project from the start** — patterns shared across opted-in projects (uses existing `share_learning` flag)
- **Dual extraction** — real-time for task completions/failures, batch (every 10min) for deeper analysis
- **Agent-driven queries** — agents call the knowledge API when they need context (not injected into prompts)
- **Hybrid matching** — structured field filter (role, task_type) + keyword overlap scoring + file path boost

---

## 1. Data Model

### New table: `knowledge_patterns`

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT PK | UUID |
| `project_id` | TEXT FK | Source project |
| `pattern_type` | TEXT | solution, gotcha, preference, recipe |
| `role` | TEXT | Which role this applies to (nullable for general) |
| `task_type` | TEXT | bug, task, feature (nullable) |
| `keywords` | TEXT | JSON array of extracted keywords |
| `title` | TEXT | Short summary |
| `content` | TEXT | The actual knowledge/advice |
| `confidence` | REAL | 0.0 to 1.0 |
| `observations` | INTEGER | How many times pattern was observed |
| `source_type` | TEXT | trace, performance, loom, manual |
| `source_id` | TEXT | ID of the source record |
| `files_involved` | TEXT | JSON array of file paths (nullable) |
| `is_shared` | BOOLEAN | Visible to other opted-in projects |
| `created_at` | TEXT | ISO timestamp |
| `updated_at` | TEXT | ISO timestamp |

### Pattern types

- **solution** — "This type of issue was solved by doing X" (from successful traces)
- **gotcha** — "Watch out for Y when working on Z" (from loom warnings/failures)
- **preference** — "Senior Coder works best with sonnet on implementation tasks" (from performance logs)
- **recipe** — "The typical workflow for this is: step1 → step2 → step3" (from trace step sequences)

---

## 2. Pattern Extraction

### Real-time (on event)

Triggered when:
- **Agent completes a task successfully** → extract `solution` from trace steps + issue context
- **Agent fails a task** → extract `gotcha` from failure reason + loom warnings
- **Routing override accepted** → extract `preference`

Happens in existing orchestrator event handlers — after issue closure or agent failure reap.

### Batch (every 10 minutes)

New `extract_knowledge()` method on `OrchestratorRunner`:
- Scans recent completed traces (last 10 minutes)
- Groups by role + task_type to find `recipe` patterns (common step sequences)
- Compares performance logs across models to generate `preference` patterns
- Merges similar patterns (bumps `observations`, recalculates `confidence`)
- Cross-project: marks patterns as `is_shared = true` if project has `share_learning` enabled

### Confidence scoring

- New pattern starts at `confidence: 0.5`
- Each additional observation: `confidence = min(0.95, 0.5 + observations * 0.05)`
- Failed patterns decay: `confidence *= 0.8`

### Keyword extraction

Simple approach — split issue title + description into words, remove stopwords, lowercase. No NLP library needed. Store as JSON array.

---

## 3. Query API

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/projects/{pid}/knowledge` | List patterns for project (with filters) |
| GET | `/api/projects/{pid}/knowledge/{id}` | Get single pattern |
| POST | `/api/projects/{pid}/knowledge/search` | Hybrid search with scoring |
| POST | `/api/projects/{pid}/knowledge` | Manual pattern creation |
| PUT | `/api/projects/{pid}/knowledge/{id}` | Update pattern |
| DELETE | `/api/projects/{pid}/knowledge/{id}` | Delete pattern |
| GET | `/api/knowledge/cross-project` | Cross-project pattern search |
| POST | `/api/projects/{pid}/knowledge/extract` | Trigger manual batch extraction |

### Search flow (hybrid matching)

1. **Structured filter** — narrow by role, task_type, pattern_type (exact match)
2. **Cross-project expand** — include `is_shared` patterns from opted-in projects
3. **Keyword score** — compute keyword overlap: `matched / total`
4. **File boost** — overlapping `files_involved` adds 0.2 to score
5. **Confidence weight** — `final_score = keyword_score * confidence`
6. **Return** — top N sorted by score

### Search request body

```json
{
  "query": "fix authentication middleware",
  "role": "Senior Coder",
  "task_type": "bug",
  "files": ["src/auth/mod.rs"]
}
```

### Search response

```json
[
  {
    "id": "...",
    "title": "Auth middleware token validation gotcha",
    "content": "The session token check must happen before CORS...",
    "confidence": 0.85,
    "observations": 3,
    "pattern_type": "gotcha",
    "source_project": "Ironweave",
    "score": 0.72
  }
]
```

---

## 4. Frontend — Knowledge Tab

New tab on `ProjectDetail.svelte` between Routing and Tests.

### Top bar
- Pattern count badge: "42 patterns (8 shared)"
- Filter dropdowns: pattern_type, role, confidence threshold
- "Extract Now" button — triggers manual batch extraction

### Main area
- Pattern cards (scrollable list):
  - Type badge (green=solution, amber=gotcha, purple=preference, blue=recipe)
  - Title + content preview (expandable)
  - Confidence bar + observation count
  - Source: project name + source type
  - Files involved (mono tags)
  - Shared badge
- Click to expand full content

### Manual pattern creation
- "Add Pattern" button → form with: title, content, pattern_type, role, task_type, keywords, files_involved
- For humans to codify knowledge agents can't auto-extract

---

## 5. Orchestrator Integration

### Real-time hooks (in existing sweep handlers)

In `sweep_teams()` when an issue is closed:
```rust
// After marking issue as closed
if issue.status == "closed" {
    self.extract_solution_pattern(&issue, &trace);
}
```

In agent reap when failure detected:
```rust
// After marking agent as dead/failed
self.extract_gotcha_pattern(&issue, &loom_entries);
```

### Batch extraction (new method)

```rust
async fn extract_knowledge(&self) {
    // Called every 10 minutes from the sweep loop
    self.extract_recipe_patterns();    // from trace step sequences
    self.extract_preference_patterns(); // from performance logs
    self.merge_duplicate_patterns();    // consolidate similar patterns
}
```

---

## 6. Testing (TDD)

### Rust unit tests (written first)

- KnowledgePattern model CRUD
- Keyword extraction from text
- Hybrid search scoring (structured filter, keyword overlap, file boost, confidence weight)
- Pattern merging (duplicate detection, observation increment)
- Confidence decay
- Cross-project visibility (shared flag + opted-in check)
- Real-time extraction (trace → solution, failure → gotcha)
- Batch extraction (traces → recipe, perf logs → preference)

### Playwright e2e tests

- `knowledge.spec.ts` — Tab renders, pattern list, filters
- `interact-knowledge.spec.ts` — Manual create/delete, Extract Now, search
- `fill-knowledge.spec.ts` — All API endpoints, response structure, cross-project

---

## Related

- `docs/BACKEND_FEATURES.md` — Backend feature checklist
- `docs/FRONTEND_FEATURES.md` — Frontend feature checklist
- `docs/plans/2026-03-16-feature-regression-testing-design.md` — Test runner design
- Existing data sources: `model_performance_log`, `workflow_traces`, `workflow_trace_steps`, `workflow_chokepoints`, `code_graph_nodes`, `code_graph_edges`, `model_routing_overrides`, `loom_entries`, `cost_tracking`
