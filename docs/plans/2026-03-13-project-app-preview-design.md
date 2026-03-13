# Project App Preview — Design

**Goal:** Let users preview web apps built by agents — start/stop local apps with auto-detection, or link to remote project URLs.

**Architecture:** Lightweight subprocess management. Ironweave spawns the app process, tracks PID/port in DB, and serves the URL in the project header. No containers, no new infrastructure.

---

## Data Model

New `project_apps` table:

```sql
CREATE TABLE project_apps (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    pid INTEGER,
    port INTEGER,
    run_command TEXT NOT NULL,
    state TEXT CHECK(state IN ('stopped', 'starting', 'running', 'error')) DEFAULT 'stopped',
    last_error TEXT,
    started_at TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);
```

Add `app_url TEXT` column to `projects` table for remote project URLs.

One app per project.

## Auto-Detection

Scan project directory, first match wins:

| File Found | Command | Port Passing |
|---|---|---|
| `app.py` / `main.py` (Flask) | `python app.py` | `PORT=<assigned>` env |
| `manage.py` | `python manage.py runserver 0.0.0.0:<port>` | via arg |
| `package.json` (with `start`) | `npm start` | `PORT=<assigned>` env |
| `Cargo.toml` | `cargo run` | `PORT=<assigned>` env |
| `go.mod` | `go run .` | `PORT=<assigned>` env |
| `index.html` (static) | `python -m http.server <port>` | via arg |

Port range: 8100–8199, auto-assigned (first free port).

## API Endpoints

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/api/projects/{id}/app/start` | Auto-detect, assign port, spawn process |
| `POST` | `/api/projects/{id}/app/stop` | Kill process, update state |
| `GET` | `/api/projects/{id}/app/status` | Return state, port, URL, PID |

Remote project `app_url` managed via existing `PUT /api/projects/{id}`.

## Frontend UI

**Project header (ProjectDetail.svelte):**

- **Local projects:** Start/Stop button + clickable URL link when running + state badge (green/grey/red dot)
- **Remote projects (app_url set):** Clickable link icon only, no start/stop
- **Settings tab:** `app_url` field for remote projects

Status fetched on page load via `/app/status`. No polling.
