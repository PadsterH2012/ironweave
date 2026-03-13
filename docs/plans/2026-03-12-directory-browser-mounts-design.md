# Directory Browser & Remote Mount Manager — Design

**Goal:** Add server-side filesystem browsing for project directory selection, plus managed NFS/SMB/SSHFS mounts that auto-mount when a project is active and store credentials encrypted at rest.

**Architecture:** New `src/mount/` module with MountManager, credential encryption, and idle monitor. New filesystem browse API. Frontend gets a DirectoryBrowser modal and Mounts management page. Projects optionally link to a mount.

---

## Database

### New `mounts` table

```sql
CREATE TABLE mounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mount_type TEXT NOT NULL CHECK(mount_type IN ('nfs', 'smb', 'sshfs')),
    remote_path TEXT NOT NULL,
    local_mount_point TEXT NOT NULL,
    username TEXT,
    password TEXT,
    ssh_key TEXT,
    mount_options TEXT,
    auto_mount INTEGER NOT NULL DEFAULT 1,
    state TEXT NOT NULL DEFAULT 'unmounted'
        CHECK(state IN ('mounted', 'unmounted', 'error')),
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Projects table update

Add column: `mount_id TEXT REFERENCES mounts(id)`. When set, `directory` is relative to the mount's `local_mount_point`.

---

## Mount Manager Module (`src/mount/`)

### `manager.rs` — MountManager

- `mount(mount_id)` — executes system command based on type:
  - NFS: `sudo mount -t nfs <remote> <local>`
  - SMB: `sudo mount -t cifs <remote> <local> -o username=...,password=...`
  - SSHFS: `sshfs <user@host:path> <local> -o IdentityFile=...`
- `unmount(mount_id)` — `sudo umount <local_mount_point>`
- `check_status(mount_id)` — verify mountpoint is accessible
- `ensure_mounted(mount_id)` — mount if not already, no-op if mounted
- All operations update `state` column in DB
- Mount points created under `/home/paddy/ironweave/mounts/<mount-name>/`

### `crypto.rs` — Credential Encryption

- AES-256-GCM via `aes-gcm` crate
- Master key from `ironweave.toml` `[security]` section (base64-encoded 32-byte key)
- `encrypt(plaintext, key) -> ciphertext` and `decrypt(ciphertext, key) -> plaintext`
- CLI command `ironweave generate-key` to create a random key
- Credentials never returned in API responses — redacted as `"***"`

### `idle_monitor.rs` — Background Task

- Tokio task running every 5 minutes
- Checks for mounts with no active project sessions
- Unmounts after configurable idle timeout (default 30 minutes)

---

## API Endpoints

### Filesystem Browser

- `GET /api/filesystem/browse?path=/home/paddy` — returns directory listing
  ```json
  {
    "path": "/home/paddy",
    "parent": "/home",
    "entries": [
      { "name": "ironweave", "type": "directory" },
      { "name": "file.txt", "type": "file" }
    ]
  }
  ```
- Only returns directories by default; `include_files=true` to include files
- Restricted to configurable `browse_root` paths (default `["/home/paddy"]`)

### Mount CRUD

- `GET /api/mounts` — list all mounts with status
- `POST /api/mounts` — create mount config (credentials encrypted before storage)
- `GET /api/mounts/{id}` — get mount details (credentials redacted)
- `DELETE /api/mounts/{id}` — unmount if mounted, then delete config
- `POST /api/mounts/{id}/mount` — manually trigger mount
- `POST /api/mounts/{id}/unmount` — manually trigger unmount
- `GET /api/mounts/{id}/status` — check if mount is alive

### Project Update

- `POST /api/projects` accepts optional `mount_id`

---

## Frontend

### `DirectoryBrowser.svelte` — Modal Component

- Breadcrumb navigation (e.g. `/home` > `paddy` > `ironweave`)
- Folder list with click-to-navigate
- Back/up button for parent directory
- "Select" button picks current path
- Opened via "Browse" button next to directory input

### Updated `Projects.svelte` Create Form

- "Source" dropdown: Local, NFS Share, SMB Share, SSH Remote
- **Local:** directory input + Browse button
- **NFS:** remote path (`server:/export`), mount options
- **SMB:** remote path (`//server/share`), username, password, domain (optional), mount options
- **SSH:** remote path (`user@host:/path`), password or SSH key (paste/upload), port (default 22)
- On submit: creates mount first (if remote), then creates project linked to it

### New `Mounts.svelte` Page

- Lists all configured mounts with status badges (mounted=green, unmounted=gray, error=red)
- Manual mount/unmount buttons
- Shows which projects use each mount
- Create/delete mounts independently
- Accessible from sidebar between "Projects" and "Agents"

---

## System Dependencies

### VM Packages

- `cifs-utils` — SMB mounts
- `nfs-common` — NFS mounts
- `sshfs` — SSH filesystem mounts
- `fuse3` — FUSE support for SSHFS

### Permissions

Sudoers entry for mount commands only:
```
paddy ALL=(root) NOPASSWD: /usr/bin/mount, /usr/bin/umount, /usr/bin/sshfs
```

### Config Additions

```toml
[security]
master_key = "base64-encoded-32-byte-key"

[filesystem]
browse_roots = ["/home/paddy"]
mount_base = "/home/paddy/ironweave/mounts"
idle_unmount_minutes = 30
```

### Error Handling

- Mount failures stored in `last_error` column, state set to `error`
- Frontend shows error details with retry button
- SSHFS timeouts: `ServerAliveInterval=15,ServerAliveCountMax=3`

---

## Rust Crate Additions

- `aes-gcm` — credential encryption
- `base64` (already present) — key encoding
