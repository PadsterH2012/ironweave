# Intake Agent & Autonomous Task Decomposition — Design

> **Project:** Ironweave
> **Feature:** Intake Agent, Parent/Child Issues, Git Worktree Isolation, Merge Queue
> **Status:** Approved
> **Created:** 2026-03-13

---

## Vision

Submit any ticket — bug, tweak, feature, performance issue — and the system analyses it, breaks it into role-tagged subtasks with dependencies, and the swarm builds it autonomously. Like talking to a senior dev who reads the codebase, creates the work items, and manages the execution order.

---

## Approach

**Approach A: Intake as a Reserved Orchestrator Role** (selected)

The orchestrator watches for new issues without a `parent_id`. When one appears, it spawns an "Intake Agent" using the strongest available model. The intake agent reads the codebase, creates child issues via the API (with roles, dependencies, descriptions), and marks the parent as decomposed. Child tasks unlock progressively as dependencies complete. Each agent works in an isolated git worktree. Completed branches merge via a FIFO queue.

### Why this approach

- Fits naturally into existing sweep loop
- Intake agent is just another CLI agent with a special prompt template
- No new infrastructure — uses existing PTY spawn, issue API, depends_on blocking
- Full codebase access via Claude Code tools

### Alternatives considered

- **Separate API service** — faster but can't inspect codebase, limited decomposition quality
- **Workflow DAG trigger** — clean but heavyweight, tightly couples intake to workflow system

---

## Design

### 1. Data Model Changes

Issues table gets new columns:

```sql
parent_id    TEXT REFERENCES issues(id)   -- links subtask to parent ticket
needs_intake INTEGER DEFAULT 1            -- 1 = needs decomposition, 0 = ready for agents
scope_mode   TEXT DEFAULT 'auto'          -- 'auto' or 'conversational'
```

**Behaviour:**
- New issues without `parent_id` start with `needs_intake = 1`
- Intake agent analyses, creates children with `parent_id` set, then sets `needs_intake = 0` on parent
- Parent stays open as tracker — swarm ignores it (has children)
- When ALL children close → parent auto-closes
- Simple tweaks: intake decides no decomposition needed, sets `needs_intake = 0`, leaves issue for direct pickup

**Conversational mode:** When `scope_mode = 'conversational'`, intake agent posts questions to parent's `summary` field, sets status to `awaiting_input`, and exits. User updates description with answers → next sweep re-triggers intake.

### 2. Intake Agent Trigger & Lifecycle

**Trigger in sweep loop:** Before dispatching swarm work, query for issues where `needs_intake = 1 AND parent_id IS NULL AND status = 'open'`.

**Concurrency:** One intake agent per project at a time. Tracked in `HashMap<project_id, session_id>`.

**Model:** Always uses strongest available (Sonnet by default). Configurable per team as `intake_model`.

**Intake agent receives:**
- Ticket title + description
- Project file tree
- Recent git log (20 commits)
- Team's available roles and descriptions
- Scope mode (auto or conversational)
- API instructions for creating child issues and updating parent

**Intake agent does:**
1. Reads codebase to understand project structure
2. Analyses ticket scope and type
3. Creates child issues via API with: `parent_id`, `role`, `depends_on`, description with acceptance criteria, `needs_intake = 0`
4. Updates parent: `needs_intake = 0`, summary listing created tasks
5. Exits (code 0)

**Intake adapts by input type:**

| Input | Behaviour | Typical output |
|-------|-----------|----------------|
| Bug report | Investigate, identify root cause, plan fix | 1-3 tasks |
| Tweak | Quick scan, small change | 1-2 tasks, minimal deps |
| Feature | Full scope, architecture, phased breakdown | 4-10 tasks with dependency chains |
| Performance | Profile, identify bottleneck, plan optimisation | 2-4 tasks |

### 3. Git Worktree Isolation

**When agent claims a child task:**
1. `WorktreeManager::create_worktree()` creates branch `ironweave/{role}/{issue-id-short}`
2. Worktree path: `{project_dir}/.worktrees/{session-id}/`
3. Agent's `working_directory` set to the worktree (not main project dir)
4. Agent works in isolation

**On completion (exit 0):** Branch enters merge queue.

**On crash (non-zero exit):** Issue unclaimed, returned to pool. Worktree kept for debugging.

**Existing code:** `src/worktree/manager.rs` has `create_worktree()` and `remove_worktree()`. Needs wiring into `spawn_team_agent()`.

### 4. Merge Queue

**FIFO queue processed in sweep loop** (one merge per sweep cycle per project).

**Flow:**
1. Task completion → `MergeQueue::enqueue(branch, issue_id)`
2. Merge worker: `git checkout main && git merge --no-ff {branch}`
3. Clean merge → done, remove worktree
4. Conflict → escalate

**Conflict resolution (v1):**

| Tier | Handler | When |
|------|---------|------|
| T0: Auto | `git merge` succeeds | No conflicts |
| T1: Agent resolver | Spawn resolver agent | Conflicts detected |

Human escalation (T2) deferred to v1.1. Failed resolver → merge stays queued, warning in UI.

### 5. Parent Auto-Close

After any child moves to `closed`:
1. Query: all children of this parent where `status != 'closed'`
2. If none remain → auto-close parent with aggregated summary

### 6. Frontend Changes (Minimal)

- **Issue cards:** Child issues show "↳ parent title" badge. Parents show "3/5 done" progress.
- **Issue detail modal:** Full description, summary, children list, parent link, clickable agent link → terminal, dependencies.
- **Issue creation:** Add `scope_mode` toggle (Auto / Needs Scoping). Default Auto.
- **Merge queue indicator:** Badge on project page showing pending merges.

### Not in scope (v1.1)

- DAG visualisation of dependencies
- Issue filtering/search
- Bulk operations
- Issue comments/audit trail
- T2 human escalation for merge conflicts

---

## Progressive Unlock Example

```
You submit: "Add webhook notifications"

Intake agent creates:
  Task 1: Design webhook data model (Architect)        ← unlocked immediately
  Task 2: Create webhook DB table (Senior Coder)        ← depends on 1
  Task 3: Build webhook dispatch service (Senior Coder)  ← depends on 2
  Task 4: Add webhook config UI (UI/UX Coder)           ← depends on 2
  Task 5: Write webhook integration tests (Tester)      ← depends on 3, 4
  Task 6: Document webhook API (Documentor)             ← depends on 3

Execution:
  Wave 1: Task 1 unlocks → Architect picks up
  Wave 2: Task 1 closes → Task 2 unlocks → Senior Coder picks up
  Wave 3: Task 2 closes → Tasks 3, 4 unlock in parallel
  Wave 4: Tasks 3, 4 close → Task 5 unlocks; Task 3 closes → Task 6 unlocks
  Wave 5: All close → parent auto-closes
```

Each task runs in its own worktree. Completed branches merge via the queue.
