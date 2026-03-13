# Team Dispatch — Design

> **Project:** Ironweave
> **Feature:** Orchestrator team-based agent dispatch
> **Created:** 2026-03-12
> **Status:** Approved

---

## Goal

Let the orchestrator spawn agents from team slots to work on project issues, matching issues to agents by role. Teams define a pool of available agents; the orchestrator dispatches them on-demand as matching work appears.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Stage-slot mapping | Stage-slot binding via `role` field | Explicit control, reuse same role across issues |
| Fallback behaviour | Lenient — fall back to DAG stage config if no slot match | Backwards-compatible with existing workflows |
| Agent lifecycle | On-demand spawn/exit per issue | No idle agents, cheaper, fits existing PTY model |
| Trigger mechanism | Orchestrator poll (30s sweep) + manual issue status | Simple, reuses existing sweep; event-driven deferred |
| Auto-pickup filtering | Configurable by issue type (task/bug/feature) | Teams control what they auto-claim |
| Parallel agents | Up to slot count per role | Multiple Senior Coders work simultaneously |
| Dispatch approach | Extend existing orchestrator sweep (Approach A) | Minimal new code, reuses sweep/escalation infra |

## Architecture

```
Project
  ├── Team (is_active=1, auto_pickup_types=["task","bug"])
  │     ├── Slot: Architect (claude/opus) ─────────────┐
  │     ├── Slot: Senior Coder (claude/sonnet) ────────┤
  │     ├── Slot: Senior Coder (claude/sonnet) ────────┤ Orchestrator
  │     ├── Slot: UI/UX Coder (gemini/2.5-pro) ───────┤ matches role
  │     └── Slot: Tester (claude/haiku) ───────────────┤ → spawns agent
  │                                                     │
  ├── Issue (role: "senior_coder", type: "task") ──────┘
  ├── Issue (role: "architect", type: "feature") ──────→ Architect slot
  └── Issue (role: "tester", type: "task") ────────────→ Tester slot

Sweep loop (every 30s):
  1. Existing: sweep DAG workflows (unchanged)
  2. NEW: sweep active teams
     → Find open issues with roles matching available slots
     → Spawn agents up to slot count per role
     → Track via AgentSession records
     → Agent closes issue via PATCH → next sweep cleans up
```

## Schema Changes

### issues

```sql
ALTER TABLE issues ADD COLUMN role TEXT;
```

Nullable. When set, only agents with a matching slot role pick up this issue. When NULL, any agent can claim it (backwards-compatible).

### teams

```sql
ALTER TABLE teams ADD COLUMN auto_pickup_types TEXT DEFAULT '["task","bug","feature"]';
ALTER TABLE teams ADD COLUMN is_active INTEGER DEFAULT 0;
```

- `auto_pickup_types`: JSON array of issue types the team auto-claims
- `is_active`: 0 = dormant, 1 = orchestrator sweeps for work. Templates always have `is_active = 0`

### No changes to

- `team_agent_slots` — already has role, runtime, model
- `agent_sessions` — already has team_id, slot_id, workflow_instance_id
- `workflow_definitions` — DAG workflows still work as before

## Orchestrator Sweep Extension

### sweep_teams() method

```
For each team WHERE is_active = 1:
  a. Get team's slots grouped by role
  b. Count running agents per role (from agent_sessions WHERE state = 'running')
  c. Query open issues WHERE:
     - project_id = team's project_id
     - role matches a slot role in the team
     - issue_type IN team's auto_pickup_types
     - status = 'open'
     - claimed_by IS NULL
  d. For each matching issue:
     - Check: running agents for this role < slot count for this role
     - If under limit: spawn agent from slot, create AgentSession, claim issue
     - If at limit: skip (will pick up next sweep)
```

### Agent Lifecycle

```
Issue created (role: "senior_coder", type: "task")
  → Sweep finds it, matches to Senior Coder slot
  → Creates AgentSession (team_id, slot_id, runtime from slot)
  → Spawns PTY agent with slot's runtime + model
  → Agent prompt includes issue details + curl completion instructions
  → Agent works, PATCHes issue to "closed" with summary
  → Next sweep detects closure, cleans up AgentSession
  → If more matching issues exist, spawns next agent from same slot
```

### Idle/Kill Escalation

Reuse existing escalation logic (5min nudge → 7min warning → 9min kill) applied to team-dispatched agents. Track via a generic agent tracking struct shared between DAG and team dispatch.

## Agent Prompt & Config

### Prompt Template

```
You are a {slot.role} working on project {project.name}.

Your current task:
Title: {issue.title}
Description: {issue.description}

{slot.config}

When you have completed your work, close your issue:
curl -X PATCH ${IRONWEAVE_API}/api/projects/{pid}/issues/{iid} \
  -H 'Content-Type: application/json' \
  -d '{"status": "closed", "summary": "Brief description"}'
```

The `slot.config` field stores role-specific instructions (e.g. "Focus on performance and security"). Already a TEXT field on the slot — no schema change needed.

### AgentConfig

```rust
AgentConfig {
    working_directory: project.directory,
    prompt: <above>,
    model: slot.model,
    environment: { IRONWEAVE_API: <from settings> },
}
```

Runtime comes from `slot.runtime`, passed to ProcessManager.

### AgentSession Record

```rust
AgentSession::create(conn, &CreateAgentSession {
    team_id: team.id,
    slot_id: slot.id,
    runtime: slot.runtime,
    workflow_instance_id: None,
    pid: Some(child_pid),
    worktree_path: None,
    branch: None,
})
```

## API Changes

### Modified Endpoints

| Route | Change |
|-------|--------|
| `POST /api/projects/{pid}/issues` | `CreateIssue` gains optional `role` field |
| `PATCH /api/projects/{pid}/issues/{id}` | `UpdateIssue` gains optional `role` field |

### New Endpoints

| Route | Method | Purpose |
|-------|--------|---------|
| `PUT /api/projects/{pid}/teams/{id}/activate` | PUT | Set `is_active = 1` |
| `PUT /api/projects/{pid}/teams/{id}/deactivate` | PUT | Set `is_active = 0` |
| `PUT /api/projects/{pid}/teams/{id}/config` | PUT | Update `auto_pickup_types` |
| `GET /api/projects/{pid}/teams/{id}/status` | GET | Live status: agents per role, running/idle counts |

## UI Changes

### Issue Creation — Role Field

Role dropdown populated from active team's slot roles (deduplicated). Free-text field if no team is active.

### Team Card — Activation & Status

- Activate/Deactivate toggle button
- Green "Active" badge when active, live agent count per role
- Auto-pickup config checkboxes (task, bug, feature)

### Issue Board — Role Badge

Small role badge on issues. Claimed issues show which agent/slot is working on them.

## Component Changes

| File | Change |
|------|--------|
| **Modify:** `src/db/migrations.rs` | ALTER TABLE for `role` on issues, `auto_pickup_types` and `is_active` on teams |
| **Modify:** `src/models/issue.rs` | Add `role` to `Issue`, `CreateIssue`, `UpdateIssue` |
| **Modify:** `src/models/team.rs` | Add `auto_pickup_types` and `is_active` to `Team`, add `activate`/`deactivate`/`update_config` methods |
| **Modify:** `src/orchestrator/runner.rs` | Add `sweep_teams()` method, team-based agent tracking, spawn from slots, cleanup on issue close |
| **Modify:** `src/api/teams.rs` | Add activate, deactivate, config update, team status handlers |
| **Modify:** `src/main.rs` | Wire new team API routes |
| **Modify:** `frontend/src/lib/api.ts` | Add role to issue types, team activation/status endpoints |
| **Modify:** `frontend/src/routes/ProjectDetail.svelte` | Team activate toggle, auto-pickup config, role dropdown on issue creation, role badges |

## What Doesn't Change

- DAG workflow execution — completely separate codepath
- Manual agent spawn API — unaffected
- SwarmCoordinator — untouched (future consolidation candidate)
- Existing team CRUD, slot CRUD, template system — all preserved

## Deferred Items

- **Event-driven dispatch** — IssueReady events for instant response (future optimisation)
- **Token/cost tracking per team** — Loom feature
- **Agent output streaming in team context** — WebSocket per team view
- **SwarmCoordinator consolidation** — merge into team dispatch system
- **Role-based prompt libraries** — pre-built system prompts per role template
