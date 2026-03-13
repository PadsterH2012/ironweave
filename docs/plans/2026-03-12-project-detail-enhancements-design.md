# Project Detail Enhancements Design

## Goal

Enhance the project detail page with three new capabilities: rsync-based file synchronisation with jj version control as a safety net, a read-only file browser, and editable project details including a new description field.

## Architecture

The backend manages all sync operations (rsync + jj) server-side via a new SyncManager module, following the same `Command`-based pattern used by MountManager. The frontend adds three new tabs (Files, History, Settings) to the existing ProjectDetail page. File browsing reads from a local synced copy for speed, falling back to the SSHFS mount point if no sync has occurred yet.

## Key Decisions

- **rsync over SSHFS for browsing** — local copy is fast, SSHFS through 3-hop proxy is sluggish
- **jj (Jujutsu) for local version control** — acts as Ironweave's safety net, not replacing remote git. Tracks every sync as a snapshot so changes can be reviewed and restored.
- **One-way sync** — remote is source of truth. Agents/users edit on the remote, rsync pulls changes back, jj commits the new state.
- **Auto-sync on project open** — frontend triggers sync when navigating to project detail. Backend uses rsync `--dry-run` first to check if changes exist before doing a full sync.
- **Backend-driven** — all rsync/jj logic runs server-side on hl-ironweave. Frontend only calls API endpoints.

---

## 1. Data Model Changes

### Project model additions

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `description` | TEXT nullable | NULL | Free-text project description |
| `sync_path` | TEXT nullable | NULL | Local path for rsync'd files |
| `last_synced_at` | TEXT nullable | NULL | ISO timestamp of last sync |
| `sync_state` | TEXT | "idle" | One of: idle, syncing, error |

`sync_path` is auto-derived: `{config.sync_base}/{project-id}`

### UpdateProject struct

New struct with all optional fields for `PUT /api/projects/{id}`:
- name, directory, context, description, git_remote
- obsidian_vault_path, obsidian_project, mount_id

---

## 2. Sync Manager (Backend)

New module: `src/sync/manager.rs`

### SyncManager

Holds `DbPool` and sync config (base path).

### Core operations

**`sync_project(project_id)`** — main sync flow:
1. Look up project + its mount
2. Ensure mount is active via `MountManager::ensure_mounted`
3. Set `sync_state = "syncing"` in DB
4. Run `rsync -az --delete {mount_local_path}/ {sync_path}/`
5. If rsync transferred files, run `jj commit` in the sync_path repo
6. Update `last_synced_at` and `sync_state = "idle"`
7. On error, set `sync_state = "error"`

**`init_sync_repo(project_id)`** — called once on first sync:
1. Create sync directory
2. Run `jj git init`
3. Initial rsync + `jj commit -m "initial sync"`

**`get_history(project_id, limit)`** — recent jj changes:
1. Run `jj log` with template to extract change_id, description, timestamp
2. Parse into `Vec<SyncSnapshot>`

**`get_diff(project_id, change_id)`** — diff for a specific commit:
1. Run `jj diff -r {change_id}`
2. Return raw diff text

**`restore(project_id, change_id)`** — restore to previous state:
1. Run `jj restore --from {change_id}`
2. Auto-commit with message "restored from {change_id}"

### New API endpoints

```
POST /api/projects/{id}/sync                    — trigger sync
GET  /api/projects/{id}/sync/status              — sync state + last_synced_at
GET  /api/projects/{id}/sync/history             — jj snapshot list
GET  /api/projects/{id}/sync/diff/{change_id}    — diff for a snapshot
POST /api/projects/{id}/sync/restore             — restore to a snapshot
```

---

## 3. File Browser

### Backend

**`GET /api/projects/{id}/files?path=`** — browse project files:
- If sync_path exists → browse local copy (fast)
- If no sync but has mount → ensure mount, browse SSHFS mount point (fallback)
- If neither → empty response with message

**`GET /api/projects/{id}/files/content?path=`** — file content:
- Returns raw text for a relative path within sync/mount directory
- Capped at 1MB

Reuses existing `BrowseEntry`/`BrowseResponse` types.

### Frontend — "Files" tab

- **Sync status bar** — last synced timestamp, state indicator, "Sync Now" button
- **Source indicator** — "Local copy" or "Live (SSHFS)" label
- **Tree browser** — left panel, expandable directory tree, lazy-loaded
- **File viewer** — right panel, monospace plain text (no syntax highlighting for now)

---

## 4. Project Settings Tab

### Frontend — "Settings" tab

Editable form for all project fields:
- Name (text), Description (textarea), Context (dropdown)
- Directory (text + browse), Git Remote (text)
- Mount (dropdown with state indicator)

Behaviour:
- Fields pre-populated from current project
- Explicit "Save Changes" button (no auto-save)
- Calls `PUT /api/projects/{id}` with changed fields only
- Success/error banners

### Header update

Description shown below directory path in the project header (truncated to 2 lines, read-only in header).

---

## 5. jj History Tab

### Frontend — "History" tab (visible only if project has a mount)

- **Snapshot timeline** — list of recent sync snapshots with change ID, message, timestamp
- **Diff viewer** — click a snapshot to see the diff with +/- colouring
- **Restore button** — per-snapshot, with confirmation dialog
- **Empty state** — "No sync history yet. Open the Files tab to trigger your first sync."

---

## 6. Tab Layout

| Tab | Visibility | Purpose |
|-----|-----------|---------|
| Teams | Always | Existing — manage agent teams |
| Issues | Always | Existing — issue board |
| Workflows | Always | Existing — workflow definitions |
| Files | Always | Browse synced/mounted files, sync controls |
| History | Has mount | jj snapshot timeline + diff + restore |
| Settings | Always | Edit project details |

Order: Teams, Issues, Workflows, Files, History, Settings

---

## Dependencies

- `rsync` — must be installed on hl-ironweave
- `jj` (Jujutsu) — must be installed on hl-ironweave
- Existing MountManager — used by SyncManager to ensure mounts are active
- Existing filesystem browse types — reused for file browser API
