# Ironweave v1 Gap Closure Design

> **Goal:** Close the remaining v1 feature gaps identified by codebase audit against the PRD.

**Gaps addressed:**
1. Workflow execution loop (F3/F13)
2. Coordination mode enforcement (F4)
3. Merge queue UI + auto-resolver (F11)
4. Dashboard analytics & monitoring (F9)
5. Frontend execution UIs

---

## 1. Basic DAG Workflow Execution (F3/F13)

The DAG engine (`engine.rs`) already parses definitions and produces topological tiers. The missing piece is an execution loop.

**Execution loop behaviour:**
- Takes a workflow instance and its DAG definition
- Iterates through tiers sequentially; stages within a tier run in parallel
- For each stage: spawns the configured agent, waits for completion (issue closed), collects output
- Advances to next tier when all stages in current tier complete
- Supports manual gates: pauses until human approves via API
- Persists state after each tier via the existing state machine (crash recovery)
- Handles stage failure: retry once, then fail the workflow

**Integration point:** hooks into the existing orchestrator 30s sweep loop. Each tick checks active workflow instances and advances any with completed stages.

---

## 2. Coordination Mode Enforcement (F4)

Four distinct dispatch behaviours based on `team.coordination_mode`:

**Pipeline:** Issues assigned sequentially by role order defined in the team's agent slots. Slot 1 (e.g. implementer) works first; when done, slot 2 (e.g. reviewer) picks up; then slot 3 (e.g. tester). The orchestrator only makes issues available to the next role when the previous role's work is closed.

**Swarm:** Existing behaviour — all agents pull from the shared ready pool independently. No changes needed.

**Collaborative:** Multiple agents assigned the same issue simultaneously, each working independently. The orchestrator creates a synthesis issue when all agents complete, containing their combined findings. Use case: parallel code review from different perspectives.

**Hierarchical:** One slot marked as `is_lead`. The lead agent gets issues first and decomposes them into child issues (via existing intake/child issue mechanism). Sub-agents only work on children created by the lead. The lead monitors progress and can reassign.

**Implementation:** Dispatch logic in `runner.rs` / `swarm.rs` branches based on `team.coordination_mode`.

---

## 3. Merge Queue UI + Auto-Resolver (F11)

### Backend

- New API endpoints:
  - `GET /api/projects/{pid}/merge-queue` — list queued branches
  - `POST /api/projects/{pid}/merge-queue/{id}/resolve` — manually trigger resolver
  - `POST /api/projects/{pid}/merge-queue/{id}/approve` — human approves T2 escalation
- Merge queue model in SQLite: branch name, agent ID, issue ID, status (pending/merging/conflicted/resolved/merged), conflict files, resolver agent ID
- Orchestrator enqueues completed agent branches automatically
- FIFO processing: try merge → clean = auto-merge → conflicts = auto-spawn resolver agent (T1) → resolver fails = escalate to human (T2) with diff in UI

### Frontend

- Merge queue panel on project detail page: queued branches, status badges, conflict file list
- T2 escalation: inline diff viewer with approve/reject buttons
- Resolver agent progress indicator

---

## 4. Dashboard Analytics & Monitoring (F9)

### Backend

- `GET /api/dashboard/activity` — paginated activity feed (orchestrator events)
- `GET /api/dashboard/metrics` — time-series: issues opened/closed per day, agent sessions per day, merge success/conflict ratio, average resolution time
- `GET /api/dashboard/system` — system health: CPU, memory, disk, agent process count (via `sysinfo` crate)
- New `activity_log` SQLite table, written by orchestrator as events occur

### Frontend (D3.js)

- Activity feed: real-time scrolling log, colour-coded event types, 5s auto-refresh
- Issue throughput: D3 line chart (opened vs closed, 7d/30d toggle)
- Agent utilisation: D3 stacked bar (idle vs active time per day)
- Merge health: D3 doughnut (clean vs conflict vs escalation)
- System health: CPU/RAM/disk gauges
- Merge queue status: live list of queued branches

D3 components as reusable Svelte wrappers.

---

## 5. Frontend Execution UIs

### Workflow Runner (ProjectDetail)

- Start/pause/resume/cancel workflow buttons
- DagGraph node colouring: grey (pending) → blue (running) → green (complete) / red (failed)
- Stage detail panel: click node to see assigned agent, terminal output, status

### Swarm Status (ProjectDetail)

- Team card: coordination mode badge, active/idle/total agent counts, task pool depth, throughput (issues/hour)
- Per-agent row: current issue, runtime, uptime, status badge
- Scaling indicator: spawn more / drain excess recommendation from SwarmCoordinator

All integrated into existing ProjectDetail.svelte — no new pages.

---

## Tech Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Charting | D3.js | Maximum flexibility, already have Cytoscape for DAGs |
| Conflict resolution | Auto-spawn resolver (T1) | Fast feedback; escalate to human only on failure |
| Coordination modes | All four for v1 | Pipeline, Swarm, Collaborative, Hierarchical |
| System metrics | `sysinfo` crate | Cross-platform, well-maintained, reads /proc on Linux |
| Workflow execution | Tier-based in sweep loop | Hooks into existing 30s orchestrator tick |
