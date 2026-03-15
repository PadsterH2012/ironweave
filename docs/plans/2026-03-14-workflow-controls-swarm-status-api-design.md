# API Contract: Workflow Instance Controls & Swarm Status

**Date:** 2026-03-14
**Status:** Design complete, ready for implementation

---

## 1. Workflow Instance Controls

Three new endpoints on existing workflow instance routes. These map directly to the `WorkflowState` transitions already defined in `src/orchestrator/state_machine.rs`.

### State Machine Reference

Valid transitions (from `state_machine.rs:22-33`):
- `running` → `paused` (pause)
- `paused` → `running` (resume)
- `running` → `failed` (cancel — uses Failed state since there is no Cancelled variant)
- `paused` → `failed` (cancel from paused)

---

### POST /api/workflows/{wid}/instances/{iid}/pause

Pause a running workflow instance. Stops dispatching new stages but lets in-flight stages complete.

**Path Parameters:**
| Param | Type   | Description |
|-------|--------|-------------|
| `wid` | string | Workflow definition ID |
| `iid` | string | Workflow instance ID |

**Request Body:** None

**Response 200:**
```json
{
  "id": "uuid",
  "definition_id": "uuid",
  "state": "paused",
  "current_stage": "build" | null,
  "checkpoint": "{...}",
  "started_at": "2026-03-14T10:00:00Z" | null,
  "completed_at": null,
  "total_tokens": 12500,
  "total_cost": 0.42,
  "created_at": "2026-03-14T09:59:00Z"
}
```

Returns the updated `WorkflowInstance` object (same shape as existing instance responses).

**Error Responses:**
| Status | Condition |
|--------|-----------|
| 404    | Instance not found |
| 409    | Invalid state transition (instance is not `running`) |
| 500    | Internal error |

**Backend behavior:**
1. Load instance from DB
2. Call `StateMachine::restore()` then `transition(WorkflowState::Paused)`
3. Notify orchestrator to stop scheduling new stages for this instance
4. Return updated instance

---

### POST /api/workflows/{wid}/instances/{iid}/resume

Resume a paused workflow instance. Re-enters the orchestrator loop and dispatches ready stages.

**Path Parameters:** Same as pause.

**Request Body:** None

**Response 200:** Updated `WorkflowInstance` with `"state": "running"`.

**Error Responses:**
| Status | Condition |
|--------|-----------|
| 404    | Instance not found |
| 409    | Invalid state transition (instance is not `paused`) |
| 500    | Internal error |

**Backend behavior:**
1. Load instance, restore state machine
2. `transition(WorkflowState::Running)`
3. Notify orchestrator to resume scheduling
4. Return updated instance

---

### POST /api/workflows/{wid}/instances/{iid}/cancel

Cancel a running or paused workflow instance. Terminates all associated agents and marks the instance as failed.

**Path Parameters:** Same as pause.

**Request Body:** None (optional future extension: `{ "reason": "user cancelled" }`)

**Response 200:** Updated `WorkflowInstance` with `"state": "failed"` and `completed_at` set.

**Error Responses:**
| Status | Condition |
|--------|-----------|
| 404    | Instance not found |
| 409    | Invalid state transition (instance is not `running` or `paused`) |
| 500    | Internal error |

**Backend behavior:**
1. Load instance, restore state machine
2. `transition(WorkflowState::Failed)`
3. Signal orchestrator to stop all agents for this instance
4. For each agent session with `workflow_instance_id == iid`:
   - Send stop signal via `ProcessManager`
   - Update agent session state to `"stopped"`
5. Mark any `Running`/`Pending` stages as `Skipped` in the checkpoint
6. Return updated instance

---

### Frontend API Client Additions (`frontend/src/lib/api.ts`)

```typescript
// Add to workflows.instances:
export const workflows = {
  // ... existing ...
  instances: {
    // ... existing ...
    pause: (workflowId: string, instanceId: string) =>
      post<WorkflowInstance>(`/workflows/${workflowId}/instances/${instanceId}/pause`, {}),
    resume: (workflowId: string, instanceId: string) =>
      post<WorkflowInstance>(`/workflows/${workflowId}/instances/${instanceId}/resume`, {}),
    cancel: (workflowId: string, instanceId: string) =>
      post<WorkflowInstance>(`/workflows/${workflowId}/instances/${instanceId}/cancel`, {}),
  },
};
```

### Route Registration (`src/main.rs`)

```rust
// Add after line 167 (approve_gate route):
.route("/api/workflows/{wid}/instances/{iid}/pause", post(api::workflows::pause_instance))
.route("/api/workflows/{wid}/instances/{iid}/resume", post(api::workflows::resume_instance))
.route("/api/workflows/{wid}/instances/{iid}/cancel", post(api::workflows::cancel_instance))
```

---

## 2. Swarm Status Endpoint

### GET /api/projects/{pid}/swarm-status

Returns the current swarm coordination status for a project, aggregating data from `SwarmCoordinator`, agent sessions, and recent issue completion metrics.

**Path Parameters:**
| Param | Type   | Description |
|-------|--------|-------------|
| `pid` | string | Project ID |

**Query Parameters:** None

**Response 200:**
```json
{
  "coordination_mode": "swarm",
  "active_agents": 3,
  "idle_agents": 1,
  "total_agents": 5,
  "crashed_agents": 1,
  "max_agents": 8,
  "task_pool_depth": 7,
  "throughput_issues_per_hour": 2.5,
  "scaling_recommendation": {
    "action": "SpawnMore",
    "count": 2
  },
  "agents": [
    {
      "session_id": "uuid",
      "role": "Senior Coder",
      "status": "working",
      "runtime": "claude",
      "model": "claude-sonnet-4-6",
      "current_issue_id": "uuid" | null,
      "current_issue_title": "Implement login page" | null,
      "runtime_seconds": 342,
      "uptime_seconds": 1847,
      "worktree_path": "/home/paddy/.worktrees/proj-abc123" | null,
      "branch": "agent/coder-1/issue-42" | null,
      "tokens_used": 45000,
      "cost": 1.23,
      "last_heartbeat": "2026-03-14T10:05:30Z"
    }
  ]
}
```

**Field Descriptions:**

| Field | Type | Source | Description |
|-------|------|--------|-------------|
| `coordination_mode` | string | `teams.coordination_mode` | Active team's coordination mode (`"swarm"`, `"pipeline"`, `"round_robin"`, `"manual"`) |
| `active_agents` | number | `SwarmCoordinator::scaling_detail()` | Agents currently working on a task |
| `idle_agents` | number | `SwarmCoordinator::scaling_detail()` | Agents waiting for a task |
| `total_agents` | number | `SwarmCoordinator::scaling_detail()` | All registered agents (idle + active + crashed) |
| `crashed_agents` | number | `SwarmCoordinator::scaling_detail()` | Agents that missed heartbeat timeout |
| `max_agents` | number | `SwarmCoordinator::scaling_detail()` | Team's max_agents setting |
| `task_pool_depth` | number | `SwarmCoordinator::scaling_detail()` | Count of ready/unclaimed issues |
| `throughput_issues_per_hour` | number | Computed from DB | Issues moved to `closed` status in the last 60 minutes |
| `scaling_recommendation` | object | `SwarmCoordinator::scaling_detail()` | Maps from `ScalingAction` enum |

**Per-Agent Fields:**

| Field | Type | Source | Description |
|-------|------|--------|-------------|
| `session_id` | string | `agent_sessions.id` | Agent session UUID |
| `role` | string | `team_agent_slots.role` via `agent_sessions.slot_id` | Agent's role (e.g. "Senior Coder") |
| `status` | string | `agent_sessions.state` | One of: `"idle"`, `"working"`, `"crashed"`, `"stopped"` |
| `runtime` | string | `agent_sessions.runtime` | Runtime adapter name |
| `model` | string? | `team_agent_slots.model` | Model being used, if set |
| `current_issue_id` | string? | `agent_sessions.claimed_task_id` | Currently claimed issue ID |
| `current_issue_title` | string? | `issues.title` via claimed_task_id | Title of claimed issue (for display) |
| `runtime_seconds` | number | Computed | Seconds since `claimed_at` on current issue (0 if idle) |
| `uptime_seconds` | number | Computed | Seconds since `agent_sessions.started_at` |
| `worktree_path` | string? | `agent_sessions.worktree_path` | Git worktree path |
| `branch` | string? | `agent_sessions.branch` | Git branch name |
| `tokens_used` | number | `agent_sessions.tokens_used` | Total tokens consumed |
| `cost` | number | `agent_sessions.cost` | Total cost in USD |
| `last_heartbeat` | string | `agent_sessions.last_heartbeat` | ISO 8601 timestamp |

**Error Responses:**
| Status | Condition |
|--------|-----------|
| 404    | Project not found or no active team for this project |
| 500    | Internal error |

**Notes:**
- If no swarm coordinator is active for the project, returns a response with zero counts and an empty agents array rather than 404 — the frontend can show "No active swarm" state.
- `throughput_issues_per_hour` is computed via SQL: `SELECT COUNT(*) FROM issues WHERE project_id = ? AND status = 'closed' AND updated_at >= datetime('now', '-1 hour')`
- The `scaling_recommendation` object serializes the `ScalingAction` enum's existing `#[serde(tag = "action", content = "count")]` format, producing `{"action": "SpawnMore", "count": 2}`, `{"action": "DrainExcess", "count": 1}`, or `{"action": "NoChange"}` (no count field).

---

### Frontend Types (`frontend/src/lib/api.ts`)

```typescript
export interface SwarmAgentStatus {
  session_id: string;
  role: string;
  status: string;
  runtime: string;
  model: string | null;
  current_issue_id: string | null;
  current_issue_title: string | null;
  runtime_seconds: number;
  uptime_seconds: number;
  worktree_path: string | null;
  branch: string | null;
  tokens_used: number;
  cost: number;
  last_heartbeat: string;
}

export interface SwarmStatus {
  coordination_mode: string;
  active_agents: number;
  idle_agents: number;
  total_agents: number;
  crashed_agents: number;
  max_agents: number;
  task_pool_depth: number;
  throughput_issues_per_hour: number;
  scaling_recommendation: {
    action: 'SpawnMore' | 'DrainExcess' | 'NoChange';
    count?: number;
  };
  agents: SwarmAgentStatus[];
}
```

### Frontend API Client Addition

```typescript
// Add to a new export or extend projects:
export const swarm = {
  status: (projectId: string) => get<SwarmStatus>(`/projects/${projectId}/swarm-status`),
};
```

### Route Registration (`src/main.rs`)

```rust
// Add near the team/project routes:
.route("/api/projects/{pid}/swarm-status", get(api::teams::swarm_status))
```

The handler lives in `api::teams` since it queries the project's active team and its swarm coordinator.

---

## 3. Implementation Checklist

### Workflow Instance Controls
- [ ] Add `pause_instance`, `resume_instance`, `cancel_instance` handlers to `src/api/workflows.rs`
- [ ] Add 3 routes to `src/main.rs`
- [ ] Add `pause`, `resume`, `cancel` to frontend `workflows.instances` in `api.ts`
- [ ] Add control buttons to `WorkflowView.svelte` (pause/resume/cancel based on instance state)

### Swarm Status
- [ ] Create `SwarmStatusResponse` and `SwarmAgentStatusResponse` structs in `src/api/teams.rs` (or a new response types module)
- [ ] Add `swarm_status` handler to `src/api/teams.rs`
- [ ] Add throughput query helper to `src/models/issue.rs` (`count_closed_last_hour`)
- [ ] Add route to `src/main.rs`
- [ ] Add `SwarmStatus`, `SwarmAgentStatus` types to frontend `api.ts`
- [ ] Add `swarm.status()` API call to frontend `api.ts`
- [ ] Build `SwarmStatus.svelte` component consuming this endpoint

---

## 4. Design Decisions

1. **Cancel maps to Failed state**: The existing `WorkflowState` enum has no `Cancelled` variant. Rather than adding one (which would require migration and state machine changes), cancel transitions to `Failed`. The checkpoint data will include cancellation context if needed in the future.

2. **Swarm status on project, not team**: The endpoint is `GET /api/projects/{pid}/swarm-status` rather than on a team because the frontend navigates by project. The handler resolves the active team internally.

3. **Throughput is a rolling window**: `throughput_issues_per_hour` uses a 60-minute sliding window rather than calendar-hour buckets. This gives smoother, more actionable data for the dashboard.

4. **Per-agent issue title included**: The `current_issue_title` is denormalized into the response to avoid N+1 fetches on the frontend. The backend joins `issues` on `claimed_task_id` in a single query.

5. **Graceful degradation**: If no swarm coordinator is running, the endpoint returns zeroed counts and empty agents rather than 404. This lets the SwarmStatus component render an "inactive" state without error handling.
