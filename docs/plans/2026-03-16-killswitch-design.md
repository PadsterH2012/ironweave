# Killswitch — Dispatch Pause & Scheduling

> Enable/disable agent work globally or per-project, with cron-based scheduling to prevent unattended cost.

## Requirements

- **Global killswitch**: one toggle to pause all agent dispatch across all projects
- **Per-project killswitch**: pause/resume individual projects independently
- **Global overrides project**: if global is paused, project-level state is irrelevant
- **Soft drain on pause**: running agents finish current work; no new agents dispatched
- **Forced stop after timeout**: reuses existing idle escalation (nudge 5m, warn 7m, kill 9m)
- **Cron-style scheduling**: paired resume/pause rules (e.g. "resume 09:00 Mon-Fri" + "pause 18:00 Mon-Fri")
- **API + UI + scheduled**: all three trigger mechanisms

## Database Changes

### Projects table — new columns

```sql
ALTER TABLE projects ADD COLUMN is_paused INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN paused_at TEXT;
ALTER TABLE projects ADD COLUMN pause_reason TEXT;
```

### Global state — settings keys

| Key | Default | Purpose |
|-----|---------|---------|
| `global_dispatch_paused` | `"false"` | Global pause flag |
| `global_paused_at` | `null` | ISO timestamp of last global pause |
| `global_pause_reason` | `null` | Optional reason text |

### New table: `dispatch_schedules`

```sql
CREATE TABLE dispatch_schedules (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL CHECK(scope IN ('global', 'project')),
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    cron_expression TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('resume', 'pause')),
    timezone TEXT NOT NULL DEFAULT 'Europe/London',
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    description TEXT
);
```

Schedules work as paired rules — e.g. "resume at 09:00 Mon-Fri" + "pause at 18:00 Mon-Fri". Full cron expression flexibility.

## Sweep Loop Integration

```
sweep() {
    1. Check global_dispatch_paused → if true, skip all team dispatch
    2. Evaluate global dispatch_schedules → auto-pause/resume if cron matches now
    3. For each project:
       a. Evaluate project dispatch_schedules → auto-pause/resume
       b. If project.is_paused → skip team dispatch for this project
    4. Existing logic continues (team dispatch, merge queue, etc.)
}
```

### Drain behaviour

When pause is triggered (manual or scheduled):
1. Set pause flag (project `is_paused = 1` or global setting)
2. Running agents continue — no new agents dispatched
3. Existing idle health monitor escalates naturally: nudge 5m → warn 7m → kill 9m
4. No new timeout mechanism needed — agents finish current task, go idle, get reaped

## API Endpoints

### Global killswitch

```
POST   /api/dispatch/pause              { reason?: string }
POST   /api/dispatch/resume
GET    /api/dispatch/status             → { paused, paused_at, reason, active_schedules }
```

### Per-project killswitch

```
POST   /api/projects/{pid}/dispatch/pause    { reason?: string }
POST   /api/projects/{pid}/dispatch/resume
GET    /api/projects/{pid}/dispatch/status
```

### Schedules CRUD

```
GET    /api/dispatch/schedules
POST   /api/dispatch/schedules          { scope, project_id?, cron_expression, action, timezone, description? }
GET    /api/dispatch/schedules/{id}
PUT    /api/dispatch/schedules/{id}
DELETE /api/dispatch/schedules/{id}
```

## Frontend

### Dashboard (global)

- Prominent pause/resume toggle in header — red when paused, green when active
- Status: "Paused (manual)" / "Paused (scheduled)" / "Active"
- Schedule management: list, add/edit/delete, enable/disable rules

### ProjectDetail (per-project)

- Pause/resume button in project header
- "Overridden by global pause" indicator when global is active
- Per-project schedule management in Settings tab

### Visual states

| State | Colour | Controls |
|-------|--------|----------|
| Active | Green | Pause button |
| Paused (manual) | Red | Resume button |
| Paused (scheduled) | Amber | Resume button + "next resume: X" |
| Draining | Orange | Agent count still running |
