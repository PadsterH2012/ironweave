# Teams & Model Selection — Design

> **Project:** Ironweave
> **Feature:** Team templates, per-slot model selection, runtime adapter model wiring
> **Created:** 2026-03-12
> **Status:** Approved

---

## Goal

Let users create teams of agents with per-slot runtime and model configuration, seeded with preset templates for common workflows. Wire model selection through runtime adapters so spawned agents actually use the specified model.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Model storage | Dedicated `model` column on `team_agent_slots` | First-class field, easy to query/display |
| Model delivery to CLI | Each adapter passes `--model {value}` | All three CLIs support `--model` flag |
| Model lists in UI | Runtime-specific dropdowns with custom entry | Dropdowns for common models, free-text for exotic |
| Preset templates | DB-seeded with `is_template = true` | Reusable, editable, available via API |
| Template scope | Global (`project_id = NULL`) and project-specific | Users can create templates scoped to a project or available everywhere |
| Orchestrator integration | Deferred | Stages keep their own runtime/model; team-based dispatch is a separate feature |

## Architecture

```
Team Template (is_template=true, project_id=NULL)
  ├── Slot: Architect (claude / opus)
  ├── Slot: Coder (claude / sonnet)
  └── Slot: Reviewer (claude / sonnet)
        │
        ▼ "Use Template" in project
Project Team (is_template=false, project_id=X)
  ├── Slot: Architect (claude / opus)     ──► AgentConfig { model: "claude-opus-4-6" }
  ├── Slot: Coder (claude / sonnet)       ──► AgentConfig { model: "claude-sonnet-4-6" }
  └── Slot: Reviewer (claude / sonnet)    ──► AgentConfig { model: "claude-sonnet-4-6" }
                                                    │
                                                    ▼
                                              RuntimeAdapter.build_command()
                                                claude --model claude-sonnet-4-6 ...
```

## Schema Changes

### team_agent_slots

```sql
ALTER TABLE team_agent_slots ADD COLUMN model TEXT;
```

Nullable — `NULL` means "use the runtime's default model".

### teams

```sql
-- project_id must become nullable for global templates
-- Currently has FK to projects; need to allow NULL
```

Templates: `project_id IS NULL AND is_template = 1`
Project teams: `project_id = ? AND is_template = 0`

## Adapter Changes

Each adapter's `build_command()` adds:

```rust
if let Some(ref model) = config.model {
    cmd.arg("--model");
    cmd.arg(model);
}
```

Applied to `ClaudeAdapter`, `OpenCodeAdapter`, `GeminiAdapter`.

## Frontend Model Constants

```typescript
export const RUNTIME_MODELS: Record<string, string[]> = {
  claude: ['claude-sonnet-4-6', 'claude-opus-4-6', 'claude-haiku-4-5-20251001'],
  opencode: [],  // free-text only, depends on local setup
  gemini: ['gemini-2.5-pro', 'gemini-2.5-flash'],
};
```

## Preset Team Templates

Seeded on first run via `seed_team_templates()`. Idempotent (`INSERT OR IGNORE`).

| Template | Mode | Slots |
|----------|------|-------|
| **Dev Team** | pipeline | Architect (claude/opus) → Coder (claude/sonnet) → Reviewer (claude/sonnet) |
| **Fix Team** | pipeline | Investigator (claude/sonnet) → Fixer (claude/sonnet) → Tester (claude/haiku) |
| **Research Team** | collaborative | Researcher (claude/opus) + Writer (claude/sonnet) |
| **Docs Team** | pipeline | Analyst (claude/opus) → Documenter (claude/sonnet) → Gap Reviewer (claude/sonnet) |
| **Mixed Fleet** | swarm | Claude (claude/sonnet) + OpenCode (opencode/null) + Gemini (gemini/null) |
| **Budget Squad** | swarm | Worker 1 (claude/haiku) + Worker 2 (claude/haiku) + Worker 3 (claude/haiku) |

## API Changes

### New Endpoints

| Route | Method | Purpose |
|-------|--------|---------|
| `GET /api/teams/templates` | GET | List global templates (`project_id IS NULL`) |
| `GET /api/projects/{pid}/teams/templates` | GET | List global + project-specific templates |
| `POST /api/projects/{pid}/teams/from-template/{tid}` | POST | Clone template into project |
| `PUT /api/teams/{tid}/slots/{id}` | PUT | Update slot (role, runtime, model) |

### Existing Endpoints (unchanged)

- Team CRUD: list, create, get, delete
- Slot CRUD: list, create, delete

## UI Changes

### Team List (enhanced)

- Shows teams with slot count and coordination mode badge
- "New Team" button → template picker + custom option
- Click team → expands to show slot management

### Template Picker (new)

- Grid of preset templates with name, mode, and slot summary
- "Custom Team" option for blank team
- Shows global templates, then project-specific templates

### Slot Management (new)

- List of slots: role, runtime, model, slot_order
- "Add Slot" form: role (text), runtime (dropdown), model (runtime-specific dropdown + custom entry)
- Model dropdown changes based on selected runtime
- Delete slot, reorder slots

## Component Changes

| File | Change |
|------|--------|
| **Modify:** `src/db/migrations.rs` | ALTER TABLE for `model` column and nullable `project_id` |
| **Create:** `src/db/seeds.rs` | `seed_team_templates()` function |
| **Modify:** `src/main.rs` | Call `seed_team_templates()` on startup |
| **Modify:** `src/models/team.rs` | Add `model` to `TeamAgentSlot`, add `UpdateTeamAgentSlot` struct, add clone method |
| **Modify:** `src/api/teams.rs` | Add template list, clone, and slot update handlers |
| **Modify:** `src/runtime/claude.rs` | Wire `--model` flag in `build_command()` |
| **Modify:** `src/runtime/opencode.rs` | Wire `--model` flag in `build_command()` |
| **Modify:** `src/runtime/gemini.rs` | Wire `--model` flag in `build_command()` |
| **Modify:** `frontend/src/lib/api.ts` | Add slot types, template endpoints, model constants |
| **Modify:** `frontend/src/routes/ProjectDetail.svelte` | Template picker, slot management UI |

## What Doesn't Change

- Orchestrator `spawn_stage_agent` — still reads runtime/model from DAG stage config
- Workflow definitions — still reference runtime/model per stage
- Agent spawn API — already accepts model field
- Issue/bead system — unaffected

## Deferred Items

- **Orchestrator team dispatch** — spawn agents from team slots instead of stage config (Teams feature)
- **Cost tracking per slot/team** — token/cost budget enforcement (Loom feature)
- **"Save team as template"** — convert a project team into a reusable template
- **Token budget enforcement** — `token_budget` and `cost_budget_daily` on teams
