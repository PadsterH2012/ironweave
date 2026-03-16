# Feature Regression Testing — Design

> **Date:** 2026-03-16
> **Status:** Approved
> **Goal:** Prevent accidental feature removal by running Playwright e2e tests against the live UI, triggered from within the Ironweave UI per project.

---

## Overview

Each project in Ironweave has an `app_url` field storing its live web UI URL. The regression testing system uses this as the Playwright target. Tests are flat files in each project's repo (`tests/e2e/*.spec.ts`). Results are stored in the database. Tests can be triggered manually from the UI, or automatically by the orchestrator after merges and deploys.

---

## 1. Data Model

### New table: `test_runs`

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT PK | UUID |
| `project_id` | TEXT FK | References projects |
| `status` | TEXT | pending, running, passed, failed, error |
| `test_type` | TEXT | e2e, unit, full |
| `target_url` | TEXT | Snapshot of app_url at run time |
| `total_tests` | INTEGER | Total test count |
| `passed` | INTEGER | Passed count |
| `failed` | INTEGER | Failed count |
| `skipped` | INTEGER | Skipped count |
| `duration_seconds` | REAL | Run duration |
| `output` | TEXT | Full Playwright stdout |
| `failed_tests` | TEXT | JSON array of failed test names |
| `triggered_by` | TEXT | manual, orchestrator, merge-queue |
| `created_at` | TEXT | ISO timestamp |
| `completed_at` | TEXT | ISO timestamp |

No changes to the `projects` table — `app_url` already exists.

---

## 2. API Endpoints

| Method | Path | Handler | Purpose |
|--------|------|---------|---------|
| POST | `/api/projects/{pid}/tests/run` | `tests::trigger_run` | Start a test run (body: `{ test_type }`) |
| GET | `/api/projects/{pid}/tests/runs` | `tests::list_runs` | List past test runs |
| GET | `/api/projects/{pid}/tests/runs/{id}` | `tests::get_run` | Get single run detail + output |
| GET | `/api/projects/{pid}/tests/latest` | `tests::latest_run` | Latest run result (for badges) |
| POST | `/api/projects/{pid}/tests/runs/{id}/stop` | `tests::stop_run` | Abort a running test |

### Run trigger flow

1. Resolve project's `directory` (or mount path) to find `tests/e2e/`
2. Read `app_url` from project record as `BASE_URL`
3. Spawn `npx playwright test` on the server with `BASE_URL` env var
4. Create a `test_runs` row with status `running`
5. Stream output via WebSocket (reuse existing `/ws/` pattern)
6. On completion, update the row with pass/fail counts + output

---

## 3. Frontend

### Tests Tab (ProjectDetail.svelte)

New tab alongside Issues, Teams, Merge Queue, etc.

**Top bar:**
- "Run Tests" dropdown button (E2E / Unit / Full)
- Latest run status badge (green pass, red fail, gray no runs)
- Quick stats: "Last run: 42 passed, 0 failed — 2 min ago"

**Left panel — Test Run History:**
- Each row: timestamp, type, status badge, pass/fail counts, duration
- Click a row to view details

**Right panel — Run Detail:**
- Status + duration header
- Pass/fail/skipped breakdown
- Failed test names (expandable with failure output)
- Full output log (collapsible terminal-style viewer using `Terminal.svelte`)

### Quick-Trigger Button (Projects.svelte)

Small play icon on each project tile card at `/#/projects`. Shows spinning indicator while running, then green/red result.

---

## 4. Orchestrator Integration

### Automatic trigger points

1. **Post-merge verification** — After a branch merges successfully in the merge queue, orchestrator triggers e2e run. If tests fail → merge flagged, status set to `failed`, snag created.

2. **Post-deploy verification** — When `/ironweave deploy` completes, trigger a test run against the target environment.

3. **Sweep loop check** — Per-project setting `auto_test_on_merge` (boolean, default: `true`) controls whether post-merge tests run automatically. Toggled in project settings.

### Snag auto-creation on failure

- Creates entry in Obsidian at `{project}/snags/` with failed test names + output snippet
- Logs to Loom so other agents see the failure
- Activity log entry with `event_type: test_failed`

---

## 5. Test File Convention

### Directory structure per project

```
{project_dir}/
├── tests/
│   ├── e2e/
│   │   ├── playwright.config.ts
│   │   ├── routes.spec.ts
│   │   ├── components.spec.ts
│   │   └── ...
│   └── unit/
```

### Ironweave's own e2e suite

| Test File | Verifies |
|-----------|----------|
| `navigation.spec.ts` | All 11 routes render, sidebar links, SPA routing |
| `dashboard.spec.ts` | Stats cards, KillSwitch, activity feed, metrics, system health |
| `projects.spec.ts` | Project list, create form, pause/resume badges, tile controls |
| `project-detail.spec.ts` | All tabs render, dispatch badges, tab switching |
| `issues.spec.ts` | Issue board columns, create/edit/delete, claim/unclaim |
| `teams.spec.ts` | Team CRUD, slot management, activate/deactivate, templates |
| `mounts.spec.ts` | Mount list, create form, mount/unmount, SSH test |
| `agents.spec.ts` | Agent list, spawn, stop, WebSocket terminal |
| `workflows.spec.ts` | DAG view, instance controls, gate approvals |
| `settings.spec.ts` | General/Proxies/API Keys tabs, CRUD operations |
| `killswitch.spec.ts` | Global toggle, schedule CRUD, per-project toggle |
| `costs-quality.spec.ts` | Cost dashboard, quality sliders, routing suggestions |

Each test file maps to sections in `docs/FRONTEND_FEATURES.md`. A test failure identifies exactly which checklist item regressed.

---

## Related

- `docs/BACKEND_FEATURES.md` — Backend feature checklist
- `docs/FRONTEND_FEATURES.md` — Frontend feature checklist
- `scripts/audit-features.py` — Daily code-level audit (complements this UI-level testing)
- `docs/plans/2026-03-16-killswitch-design.md` — Killswitch design (same pattern)
