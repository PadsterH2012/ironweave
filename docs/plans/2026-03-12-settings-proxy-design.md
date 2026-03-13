# Settings Page & Proxy Tunnel Design

**Goal:** Add a Settings page with sub-routes for managing global app settings, SSH proxy chains, and API keys — all stored in the database. Enable SSHFS mounts to route through multi-hop proxy chains to reach otherwise unreachable hosts.

**Architecture:** Hybrid storage — a key-value `settings` table for simple scalars, plus a typed `proxy_configs` table for SSH proxy chains. TOML config seeds the DB on first run; DB values take precedence after that. Frontend uses separate sub-routes under `/settings`.

**Tech Stack:** Rust/Axum backend, SQLite, Svelte 5 frontend, AES-256-GCM credential encryption (existing crypto module).

---

## 1. Database Schema

### `settings` table — key-value for simple scalars

```sql
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'general',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Stores: `browse_roots`, `mount_base`, `idle_unmount_minutes`, `master_key`, API keys.

Rule of thumb: if it has more than 2 fields or something else needs to FK to it, it gets a typed table. Otherwise key-value is fine.

### `proxy_configs` table — typed table for SSH proxy chains

```sql
CREATE TABLE IF NOT EXISTS proxy_configs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    hops TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Each hop in the JSON array:

```json
{
  "host": "10.202.28.230",
  "port": 22,
  "username": "paddy",
  "auth_type": "key",
  "credential": null
}
```

- `auth_type`: `"key"` (SSH key auth, no credential needed) or `"password"` (credential is AES-256-GCM encrypted)
- `credential`: encrypted password or `null` for key-based auth

### `mounts` table change

```sql
ALTER TABLE mounts ADD COLUMN proxy_config_id TEXT REFERENCES proxy_configs(id);
```

Links an SSHFS mount to a proxy chain for multi-hop connectivity.

---

## 2. Backend API

### Settings API (`/api/settings`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/settings` | List all settings grouped by category |
| GET | `/api/settings/:key` | Get single setting |
| PUT | `/api/settings/:key` | Create or update `{ value, category }` |
| DELETE | `/api/settings/:key` | Remove a setting |

Sensitive values (`master_key`, API keys) are redacted in GET responses.

### Proxy Configs API (`/api/proxy-configs`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/proxy-configs` | List all proxy configs |
| POST | `/api/proxy-configs` | Create `{ name, hops }` |
| GET | `/api/proxy-configs/:id` | Get single (credentials redacted) |
| PUT | `/api/proxy-configs/:id` | Update |
| DELETE | `/api/proxy-configs/:id` | Delete (fails if mounts reference it) |
| POST | `/api/proxy-configs/:id/test` | Test connectivity through hop chain |

Credentials encrypted before storage, decrypted only at mount time.

### Mount Manager Changes

- When mounting SSHFS with a `proxy_config_id`, build `-o ProxyJump=hop1,hop2,...` from the hop chain
- For password-auth hops, use `sshpass` or `SSH_ASKPASS`
- Add support for custom SSH port (`-p <port>`) on SSHFS mounts

### Settings Bootstrap

On startup, if the `settings` table is empty, seed from TOML config values (browse_roots, mount_base, idle_unmount_minutes, master_key). After that, DB values take precedence over TOML. TOML becomes "first-run defaults".

---

## 3. Frontend — Settings Pages

### Navigation

Add "Settings" to the sidebar at the bottom (before logout) as a utility item.

### Routes

| Route | Page |
|-------|------|
| `/settings` | Redirects to `/settings/general` |
| `/settings/general` | General settings (browse roots, mount base, timeouts) |
| `/settings/proxies` | Proxy chain configurations |
| `/settings/api-keys` | API key management |

### Settings Layout

Each sub-route gets a sidebar nav within the settings area (General / Proxies / API Keys), content area on the right. Dark theme consistent with the rest of the app.

### General Page (`/settings/general`)

- Editable fields for browse_roots, mount_base, idle_unmount_minutes
- Save button
- "Reset to defaults" button re-seeds from TOML

### Proxies Page (`/settings/proxies`)

- List of configured proxy chains: name, hop count, active/inactive badge
- Create/edit form: name field + hop builder (add/remove/reorder hops)
- Each hop: host, port, username, auth type (key/password), credential (password field)
- "Test Connection" button — hits `/test` endpoint, shows result with per-hop status
- Delete with confirmation (blocked if mounts reference it)

### API Keys Page (`/settings/api-keys`)

- Key-value list: name + masked value
- Add/edit/delete
- Copy-to-clipboard (value shown once on create, masked after)

### Mount Form Changes

- SSHFS mounts: add optional "Proxy" dropdown populated from proxy_configs
- Add port field for custom SSH ports
- Applies to both the Mounts page and the Create Project form (SSH Remote source type)

---

## 4. Error Handling & Security

### Credential Encryption

All passwords and SSH keys in proxy_configs hops are encrypted with AES-256-GCM (using existing `mount/crypto.rs`) before DB storage. Decrypted only at mount time, never sent to frontend.

### Proxy Test Endpoint

- 10-second timeout
- Tests each hop in sequence
- Reports which hop failed if connectivity breaks
- Does not store any output

### Cascade Protection

- Cannot delete a proxy config referenced by a mount
- Frontend shows which mounts use a given proxy config
- Cannot delete an API key marked as "in use" (future-proofing)

### Settings Bootstrap Conflict

DB wins over TOML. TOML values only seed on first run (empty settings table). "Reset to defaults" re-seeds from TOML.

### Validation

- Proxy hop: host required, port 1-65535, username required
- Settings: key must be alphanumeric + underscores, value max 4096 chars
- API keys: name unique, value non-empty

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Hybrid storage (key-value + typed tables) | Key-value for simple scalars, typed tables for complex relational data (proxy chains). Avoids JSON-in-value awkwardness and preserves FK integrity. |
| Generic SSH proxy chain (hop array) | Covers the current 3-hop scenario (proxy -> WSL -> CUK) and any future multi-hop setups without code changes. |
| Separate sub-routes for settings | Categories can grow independently without one page becoming unwieldy. |
| DB takes precedence over TOML | TOML is first-run defaults only. Avoids config-source ambiguity. |
| AES-256-GCM for proxy credentials | Reuses existing crypto module. Credentials never leave the server unencrypted. |
