# Agent Stage Completion — Design

> **Project:** Ironweave
> **Feature:** Agent stage completion mechanism
> **Created:** 2026-03-12
> **Status:** Approved

---

## Goal

Let orchestrator-spawned agents complete their stages by updating their issue (bead) status via the Ironweave API. Currently agents exit without closing their issue, causing all stages to be marked as Failed.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Completion signal | Bead update via API | Stays true to the bead-driven architecture. Agent closes its issue when done. |
| API endpoint | General-purpose PATCH | Matches existing pattern (projects, mounts, proxy-configs). Allows agents to update summary, not just status. |
| API URL delivery | Environment variable (`IRONWEAVE_API`) | Works regardless of deployment topology. No hardcoded addresses. |
| URL source | Config setting with default | Defaults to `https://localhost:443`. Configurable for remote agent scenarios. |
| Agent instructions | curl command in prompt | Works with any runtime (Claude, OpenCode, Gemini). No MCP dependency. |

## Architecture

```
Agent spawned by orchestrator
  │
  ├── Receives prompt with: issue_id, project_id, curl instructions
  ├── Receives env: IRONWEAVE_API=https://localhost:443
  │
  ├── Does its work...
  │
  └── On completion:
      curl -X PATCH ${IRONWEAVE_API}/api/projects/{pid}/issues/{iid} \
        -H 'Content-Type: application/json' \
        -d '{"status": "closed", "summary": "What I did"}'
          │
          ▼
      Ironweave API
        ├── Updates issue status to "closed"
        ├── Updates updated_at timestamp
        └── Returns updated issue
          │
          ▼
      Orchestrator sweep (every 30s)
        ├── Polls issue status
        ├── Sees "closed" → marks stage Completed
        └── Finds newly ready stages → spawns agents
```

## Component Changes

| File | Change |
|------|--------|
| **Create:** `src/api/issues.rs` update handler | PATCH handler for issue updates |
| **Modify:** `src/models/issue.rs` | Add `UpdateIssue` struct and `update()` method |
| **Modify:** `src/main.rs` | Add PATCH route for issues |
| **Modify:** `src/orchestrator/runner.rs` | Pass `IRONWEAVE_API` env var, include curl instructions in prompt |
| **Modify:** `frontend/src/lib/api.ts` | Add `issues.update()` method |

## UpdateIssue struct

```rust
#[derive(Debug, Deserialize)]
pub struct UpdateIssue {
    pub status: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub priority: Option<i32>,
}
```

## Prompt Template

The orchestrator builds the agent prompt as:

```
{stage.prompt}

You are working on issue {issue_id} in project {project_id}.

When you have completed your work, close your issue by running:
curl -X PATCH ${IRONWEAVE_API}/api/projects/{project_id}/issues/{issue_id} \
  -H 'Content-Type: application/json' \
  -d '{"status": "closed", "summary": "Brief description of what you accomplished"}'

You can also post progress updates at any time:
curl -X PATCH ${IRONWEAVE_API}/api/projects/{project_id}/issues/{issue_id} \
  -H 'Content-Type: application/json' \
  -d '{"summary": "Current progress update"}'
```

Progress updates reset the orchestrator's idle timer, preventing nudge/kill escalation.

## What Doesn't Change

- Orchestrator sweep logic (already polls for `status == "closed"`)
- Nudge/kill escalation (already works)
- DAG advancement (already triggers on stage completion)
- Issue creation by orchestrator (already works)

## API URL Configuration

The `IRONWEAVE_API` value comes from:
1. Settings table: key `api_url`, category `system`
2. Falls back to `https://localhost:443` if not set

This avoids a config file change and uses the existing settings infrastructure.
