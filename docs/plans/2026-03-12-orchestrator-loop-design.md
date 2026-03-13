# Orchestrator Loop — Design

> **Project:** Ironweave
> **Feature:** Workflow execution E2E — connecting F1, F3, F4, F5, F13
> **Created:** 2026-03-12
> **Status:** Approved

---

## Goal

Wire the three existing but disconnected islands — data layer (workflow/issue models), orchestration logic (DAG engine, state machine, swarm coordinator), and agent spawning (PTY process manager) — into a single background orchestrator loop that executes workflows end-to-end.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Stage dispatch | One agent per stage | Simplest E2E path. Team-based dispatch layered later via swarm coordinator. |
| Completion detection | DB-driven (bead polling) | Agents update issue status when done. Orchestrator polls issues. Survives restarts. |
| Crash detection | PTY exit as fallback | If PTY exits without issue marked done → stage failed. |
| Idle escalation | Nudge via PTY → kill | 5min idle → nudge, 7min → final warning, 9min → kill PTY and fail stage. |
| Loop model | Event-triggered + timer sweep | Immediate wake on new instance, 30s sweep for housekeeping. |
| Sweep interval | 30 seconds | Good balance for agent workflows that run minutes. |
| Nudge threshold | 5 minutes | Reasonable default; configurable per-stage later. |
| Completion action | Silent (DB update only) | Loom logging and summary beads deferred to F12/F6 integration. |
| Auto-retry | No (v1) | Failed stages stay failed. Human or future team coordinator decides. |

## Architecture

```
                    ┌─────────────────────┐
  API creates       │   Orchestrator      │     Spawns agents via
  instance ──notify─►   Loop              ├───► ProcessManager
                    │                     │
                    │  - Sweep timer (30s)│     Reads/writes
                    │  - Event channel    ├───► SQLite (issues,
                    │  - Nudge tracker    │     workflow_instances)
                    │                     │
                    │  select! {          │     Sends nudges via
                    │    event_rx,        ├───► PTY write
                    │    tick_interval    │
                    │  }                  │
                    └─────────────────────┘
```

### Event Flow

**On event (new instance created):**

1. Load the workflow definition and parse the DAG
2. Transition instance to `Running` via StateMachine
3. Find ready stages → create an issue (bead) per stage → spawn an agent per stage
4. Store stage↔agent↔issue mapping

**On sweep tick (every 30s):**

1. Check all active workflow instances
2. Poll issue statuses for each running stage
3. If issue marked done → mark stage `Completed` → find newly ready stages → spawn
4. If agent idle >5min → send nudge through PTY
5. If multiple nudges unanswered → kill PTY, fail stage, return issue to pool
6. If all stages complete → transition workflow to `Completed`

## Stage Lifecycle

```
Pending → Spawning → Running → [Nudged] → Completed/Failed
```

### Stage → Issue Mapping

When a stage becomes ready, the orchestrator creates an issue (bead) with:

- `project_id` from the workflow definition
- `title`: stage name
- `description`: stage prompt + context from previous stage outputs
- `type`: `task`
- `workflow_instance_id` + `stage_id` linking back to the workflow

The agent is spawned with a prompt that includes:

- The stage's prompt from the DAG definition
- The issue ID (so the agent can update it)
- Instructions to mark the issue as `closed` when done
- Output from predecessor stages (if any)

### In-Memory State

```rust
WorkflowRunState {
    instance_id: String,
    definition: DagDefinition,
    execution: DagExecutionState,
    stage_agents: HashMap<StageId, StageAgent>,
    state_machine: StateMachine,
}

StageAgent {
    agent_session_id: String,
    issue_id: String,
    spawned_at: Instant,
    last_activity: Instant,
    nudge_count: u32,
}
```

On server restart, active instances are restored from the DB — `StateMachine::restore()` loads the checkpoint, the DAG definition is re-parsed, and stages with missing agents get marked as `Failed`.

## Nudge & Kill Escalation

| Time idle | Action | Detail |
|-----------|--------|--------|
| 0–5 min | Nothing | Agent working normally |
| 5 min | Nudge 1 | PTY write: "Please update your issue status with your current progress." |
| 7 min | Nudge 2 | PTY write: "No status update received. Please respond or your session will be terminated." |
| 9 min | Kill | Kill PTY, mark stage `Failed`, issue back to `open` (unclaimed) |

No auto-retry for v1. Failed stages stay failed. Human or future team coordinator decides next steps.

## Component Changes

| File | Change |
|------|--------|
| **Create:** `src/orchestrator/runner.rs` | Main orchestrator loop, `OrchestratorHandle`, `WorkflowRunState` |
| **Modify:** `src/state.rs` | Add `orchestrator_handle: OrchestratorHandle` to `AppState` |
| **Modify:** `src/main.rs` | Spawn orchestrator task at startup, restore running instances |
| **Modify:** `src/api/workflows.rs` | Send event on channel after `WorkflowInstance::create()` |
| **Modify:** `src/process/manager.rs` | Add `write_to_agent(session_id, bytes)` for nudges |
| **Modify:** `src/models/issue.rs` | Add nullable `workflow_instance_id` and `stage_id` fields |
| **Modify:** `src/orchestrator/engine.rs` | Fix `#[serde(skip)]` on `dag` — checkpoint restore needs it |

## Error Handling

| Scenario | Behaviour |
|----------|-----------|
| Agent crashes (PTY exits) | Next sweep sees agent gone + issue not closed → stage `Failed`, issue back to `open` |
| Server restart | Scan for `running` instances, restore from checkpoint, mark stages with missing agents as `Failed` |
| Partial spawn failure | Already-spawned agents continue. Failed spawns → stage `Failed`. Workflow continues with successes. |
| DB unavailable | Sweep logs error, retries next tick. No panic. |
| Duplicate events | Skip if instance already tracked. Idempotent. |

## Data Model Notes

### Model Selection (for future use)

Agent model selection should be added as a field to support different models per runtime:

- **Claude Code:** haiku, sonnet, opus (via `--model` flag)
- **OpenCode:** ollama or openrouter.ai models
- **Gemini CLI:** gemini model variants

Fields to add (not wired in orchestrator v1, but present in data model):

- `SpawnRequest.model` (optional string)
- `AgentConfig.model` (optional string)
- DAG stage definition `model` field
- `TeamAgentSlot.model` field

## Future Integration Points

These are explicitly deferred. Wire in when parent features are built.

| Feature | Integration Point | How |
|---------|------------------|-----|
| **F4 Teams** | Replace one-agent-per-stage with team pool dispatch | `SwarmCoordinator.claim_next_task()` assigns stages to pooled agents by coordination mode |
| **F12 The Loom** | Log all orchestrator actions | Append entries on stage start/complete/fail/nudge/kill |
| **F14 Cost Tracking** | Accumulate per-stage token costs | Parse agent output or API, update `workflow_instances.total_tokens/total_cost` |
| **F6 Recording** | Capture stage execution for replay | Record PTY output per stage, link to workflow instance |
| **Auto-retry** | Retry failed stages with backoff | Add retry policy per stage in DAG definition |
| **Completion summary** | Generate summary bead on workflow complete | Create issue linking all stage outcomes |
| **Completion notification** | Notify via Loom/webhook on complete | Log to Loom, dispatch webhook per F17 config |

## YAGNI Decisions

- **No auto-retry** — failed is failed for v1. Avoids token-wasting loops.
- **No team dispatch** — one agent per stage. Team routing comes with F4 wiring.
- **No Loom logging** — silent orchestration. Loom integration is F12.
- **No cost tracking** — token metering deferred to F14.
- **No output replay** — stage output lives in the PTY scrollback only.

## Dependencies

- Existing: `tokio` (channels, timers, select), `portable-pty`, `rusqlite`
- Existing orchestrator modules: `engine.rs`, `state_machine.rs`
- Existing models: `issue.rs`, `workflow.rs`
