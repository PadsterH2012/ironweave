# Ironweave — Design Document

> **Date:** 2026-03-11
> **Status:** Approved for implementation planning
> **PRD:** See Obsidian: `Projects/A1 - Main Projects/Ironweave/Ironweave — PRD.md`

---

## 1. Overview

Ironweave is a web-based multi-agent orchestration platform that replaces the tmux-based multi-term console and supersedes the existing Agent Orchestrator (Fastify + React v2.1.0). It supports mixed AI CLI tools (Claude Code, OpenCode, Gemini CLI), composable agent teams and swarms, workflow recording with self-improvement, and two-way Obsidian vault sync.

## 2. Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                     Browser (Svelte 5 SPA)                       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ ┌──────────┐ │
│  │Dashboard │ │Agent     │ │Workflow  │ │Issue   │ │The Loom  │ │
│  │& Teams   │ │Terminal  │ │Visualiser│ │Tracker │ │Log View  │ │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └───┬────┘ └────┬─────┘ │
│       │WS+REST     │WS         │REST        │REST       │WS     │
└───────┼────────────┼───────────┼────────────┼───────────┼───────┘
        │            │           │            │           │
┌───────┴────────────┴───────────┴────────────┴───────────┴───────┐
│                    Rust Backend (Axum)                            │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │              Stateful Workflow Orchestrator (F13)          │   │
│  │  State machine · Checkpointing · Crash-resume · Timeouts  │   │
│  └──────────┬────────────────────────────┬───────────────────┘   │
│             │                            │                       │
│  ┌──────────┴──────┐          ┌──────────┴──────────┐           │
│  │  Process Manager │          │  Team & Swarm Engine │           │
│  │  (F1)            │          │  (F4)                │           │
│  │  Spawn/monitor   │          │  Task pool, claiming │           │
│  │  CLI agents      │          │  Dynamic scaling     │           │
│  │  Stream output   │          │  Coordination modes  │           │
│  └──────────┬──────┘          └──────────┬──────────┘           │
│             │                            │                       │
│  ┌──────────┴────────────────────────────┴──────────┐           │
│  │           Runtime Adapter Layer (F19)              │           │
│  │  ┌─────────┐  ┌──────────┐  ┌─────────────┐      │           │
│  │  │ Claude  │  │ OpenCode │  │ Gemini CLI  │      │           │
│  │  │ Adapter │  │ Adapter  │  │ Adapter     │      │           │
│  │  └─────────┘  └──────────┘  └─────────────┘      │           │
│  └───────────────────────────────────────────────────┘           │
│                                                                  │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────────┐  │
│  │ Worktree   │ │ Merge Queue│ │ The Loom   │ │ Context Mgr  │  │
│  │ Manager    │ │ & Conflict │ │ (F12)      │ │ (F15)        │  │
│  │ (F10)      │ │ Res (F11)  │ │ Append-only│ │ Token budget │  │
│  └────────────┘ └────────────┘ │ agent log  │ │ Tool routing │  │
│                                └────────────┘ └──────────────┘  │
│                                                                  │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────────┐  │
│  │ Workflow   │ │ Issue      │ │ Cost       │ │ Recording &  │  │
│  │ Engine     │ │ Tracker    │ │ Tracker    │ │ Analysis     │  │
│  │ (F3)       │ │ (F5)       │ │ (F14)      │ │ (F6)         │  │
│  └────────────┘ └────────────┘ └────────────┘ └──────────────┘  │
│                                                                  │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────────┐  │
│  │ Project    │ │ Obsidian   │ │ Open Brain │ │ External     │  │
│  │ Manager    │ │ Vault Sync │ │ Integration│ │ Integrations │  │
│  │ (F2)       │ │ (F7)       │ │ (F8)       │ │ (F17)        │  │
│  └────────────┘ └────────────┘ └────────────┘ └──────────────┘  │
│                                                                  │
│  ┌────────────┐ ┌────────────┐                                  │
│  │ CI/CD      │ │ Knowledge  │                                  │
│  │ Integration│ │ Graph      │                                  │
│  │ (F16)      │ │ (F18)      │                                  │
│  └────────────┘ └────────────┘                                  │
│                                                                  │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              │    SQLite (rusqlite)     │
              │  - workflow state       │
              │  - agent sessions       │
              │  - teams & configs      │
              │  - issues & deps        │
              │  - recordings & scores  │
              │  - The Loom entries     │
              │  - cost/token metrics   │
              │  - merge queue          │
              │  - knowledge graph      │
              └────────────┬────────────┘
                           │
              ┌────────────┴────────────┐
              │    External Services     │
              │  - Open Brain (MCP)     │
              │  - Obsidian Vault (fs)  │
              │  - GitHub/GitLab (API)  │
              │  - Slack/Discord (hook) │
              │  - CI/CD (webhooks)     │
              └─────────────────────────┘
```

## 3. Tech Stack

| Component | Technology | Version |
|-----------|-----------|---------|
| Backend | Rust (Axum) | Latest stable |
| Frontend | Svelte 5 (SPA) | 5.x |
| Database | SQLite via rusqlite | - |
| Realtime | tokio-tungstenite (WebSockets) | - |
| Process management | tokio::process | - |
| File watching | notify (Rust crate) | - |
| Git operations | git2 (libgit2 bindings) | - |
| HTTP client | reqwest | - |
| Serialisation | serde + serde_json | - |
| TLS | rustls | - |
| Frontend build | Vite | - |
| Frontend styling | TBD (Tailwind / custom) | - |
| Graph visualisation | Cytoscape.js or D3.js | - |
| Terminal emulation | xterm.js | - |

## 4. Data Model (Key Entities)

### Projects
```
project {
    id: uuid
    name: string
    directory: path
    context: "work" | "homelab"
    obsidian_vault_path: path?
    obsidian_project: string?
    git_remote: url?
    created_at: timestamp
}
```

### Teams
```
team {
    id: uuid
    name: string
    project_id: uuid -> project
    coordination_mode: "pipeline" | "swarm" | "collaborative" | "hierarchical"
    max_agents: int
    token_budget: int?
    cost_budget_daily: float?
    template: bool  // reusable across projects?
}

team_agent_slot {
    id: uuid
    team_id: uuid -> team
    role: string  // e.g. "implementer", "reviewer", "tester"
    runtime: string  // "claude" | "opencode" | "gemini"
    config: json  // runtime-specific config (allowed tools, skills, etc.)
    order: int  // for pipeline mode
}
```

### Workflows
```
workflow_definition {
    id: uuid
    name: string
    project_id: uuid -> project
    team_id: uuid -> team
    dag: json  // stage definitions, edges, conditions
    version: int  // for versioned improvements
    git_sha: string?  // git commit of this workflow version
}

workflow_instance {
    id: uuid
    definition_id: uuid -> workflow_definition
    state: "pending" | "running" | "paused" | "failed" | "completed"
    current_stage: string?
    checkpoint: json  // serialised state for crash recovery
    started_at: timestamp?
    completed_at: timestamp?
    total_tokens: int
    total_cost: float
}
```

### Agents
```
agent_session {
    id: uuid
    team_id: uuid -> team
    slot_id: uuid -> team_agent_slot
    workflow_instance_id: uuid?
    runtime: string
    pid: int?
    worktree_path: path?
    branch: string?
    state: "idle" | "working" | "blocked" | "crashed"
    claimed_task_id: uuid?
    tokens_used: int
    cost: float
    started_at: timestamp
    last_heartbeat: timestamp
}
```

### Issues
```
issue {
    id: string  // hash-based (beads-style)
    project_id: uuid -> project
    type: "bug" | "feature" | "task"
    title: string
    description: text
    status: "open" | "in_progress" | "review" | "closed"
    priority: int
    claimed_by: uuid? -> agent_session
    claimed_at: timestamp?
    depends_on: [string]  // other issue IDs
    summary: text?  // compacted summary after closure
}
```

### The Loom
```
loom_entry {
    id: uuid
    timestamp: timestamp
    agent_id: uuid -> agent_session
    team_id: uuid -> team
    project_id: uuid -> project
    workflow_instance_id: uuid?
    entry_type: "status" | "finding" | "warning" | "delegation" | "escalation" | "completion"
    content: text
}
```

### Recordings
```
recording {
    id: uuid
    workflow_instance_id: uuid -> workflow_instance
    project_id: uuid -> project
    started_at: timestamp
    ended_at: timestamp?
    total_tokens: int
    total_cost: float
    score: float?
    analysis: json?  // chokepoints, recommendations, etc.
}

recording_event {
    id: uuid
    recording_id: uuid -> recording
    timestamp: timestamp
    agent_id: uuid?
    event_type: "agent_output" | "user_input" | "tool_call" | "handoff" | "error" | "state_change"
    content: text
    metadata: json?
}
```

### Merge Queue
```
merge_queue_entry {
    id: uuid
    project_id: uuid -> project
    agent_session_id: uuid -> agent_session
    branch: string
    worktree_path: path
    target_branch: string  // e.g. "main"
    status: "queued" | "validating" | "merging" | "conflict" | "merged" | "failed"
    conflict_tier: int?  // 0, 1, or 2
    queued_at: timestamp
    merged_at: timestamp?
}
```

## 5. Key Design Decisions

### 5.1 CLI-First Agent Execution
Agents are spawned as CLI subprocesses (`claude`, `opencode`, `gemini`). This preserves the existing ecosystem of skills, hooks, and plugins. The Runtime Adapter trait abstracts the differences between tools.

### 5.2 SQLite as Single Data Store
All state lives in a single SQLite database. This simplifies deployment (no external DB), enables atomic transactions across features (e.g. claiming a task and updating agent state), and is backed up trivially.

### 5.3 The Loom as Coordination Primitive
Rather than direct agent-to-agent communication (complex, unreliable), agents communicate indirectly via The Loom — an append-only log. This is simpler to implement, inherently crash-safe, and produces a full audit trail for the analysis engine.

### 5.4 Git Worktrees for Isolation
Each agent gets a git worktree rather than a full clone. Worktrees share the `.git` directory, making them disk-efficient and instant to create. The merge queue handles integration back to the canonical branch.

### 5.5 Tiered Conflict Resolution
Conflicts are resolved mechanically when possible (T0), by AI agent when ambiguous (T1), and by human only when necessary (T2). This minimises human intervention while preventing bad merges.

### 5.6 Versioned Workflow Improvements
When the analysis engine recommends workflow changes, those changes are applied as new versions of the workflow definition, tracked in git. This means improvements are auditable and revertable — the workflow can't silently degrade.

## 6. Feature Priority

| Phase | Features | Description |
|-------|----------|-------------|
| **v1 (MVP)** | F1, F2, F3, F4, F5, F9, F10, F11, F13, F19 | Core orchestration: spawn agents, manage teams, execute workflows, track issues, merge work |
| **v1.1** | F6, F12, F14, F15 | Observability: recording, The Loom, cost tracking, context management |
| **v2** | F7, F8, F16, F17, F18 | Ecosystem: Obsidian sync, Open Brain, CI/CD, external integrations, knowledge graph |

## 7. Deployment

### 7.1 VM Specification

| Resource | Minimum (5 agents) | Recommended (10-15 agents) | Heavy (20+ agents) |
|----------|-------------------|---------------------------|-------------------|
| **vCPU** | 4 cores | 8 cores | 12-16 cores |
| **RAM** | 8 GB | 16 GB | 32 GB |
| **Disk** | 50 GB (local SSD) | 100 GB (local SSD) | 200 GB (local SSD) |
| **OS** | Debian 13 | Debian 13 | Debian 13 |
| **Network** | Bridged NIC, static IP | same | same |

**RAM budget breakdown (recommended tier):**

| Component | RAM Usage |
|-----------|----------|
| Debian 13 OS + systemd | ~300 MB |
| Ironweave server (Rust) | ~50-100 MB |
| SQLite + file watchers | ~50 MB |
| Git operations (worktrees, merges) | ~100 MB peak |
| 10 CLI agent processes @ ~1 GB each | ~10 GB |
| Headroom for spikes | ~5 GB |
| **Total** | **~16 GB** |

**CPU profile:** Agents are mostly I/O-bound (waiting on API responses), so fewer cores than expected. CPU-intensive work comes from the merge queue, analysis engine, knowledge graph indexing, and serving the Svelte UI to multiple users.

**Disk breakdown:**

| Component | Estimated Usage |
|-----------|----------------|
| OS + packages | ~5 GB |
| Ironweave binary + static assets | ~50 MB |
| SQLite database (grows over time) | ~1-5 GB |
| Git worktrees (shared .git, shallow working trees) | ~10-20 GB |
| Obsidian vault copies | ~1-5 GB |
| Recordings and Loom data | ~5-20 GB |
| CLI tools (Node.js, claude, opencode, gemini) | ~2 GB |
| Playwright + browser binaries (Chromium, Firefox) | ~1-2 GB |
| Headroom | ~40 GB |
| **Total** | **~100 GB** |

### 7.2 VM Setup Requirements

| Requirement | Details |
|-------------|---------|
| **Proxmox host** | Any node with available resources |
| **Network** | Bridged NIC, static IP on `10.202.28.x` range |
| **TLS** | HTTPS using homelab CA certificates (rustls) |
| **DNS** | Optional — `ironweave.homelab` or similar |

### 7.3 Software Dependencies

| Software | Purpose | Install Method |
|----------|---------|---------------|
| Debian 13 | Base OS | Proxmox template |
| git | Worktree operations, merge queue | `apt install git` |
| Node.js 22+ | Required by Claude Code CLI | nodesource repo |
| Claude Code CLI | Agent runtime | `npm install -g @anthropic-ai/claude-code` |
| OpenCode CLI | Agent runtime | Binary release or package manager |
| Gemini CLI | Agent runtime | `npm install -g @anthropic-ai/gemini-cli` or similar |
| Playwright | E2E testing — used by Ironweave and by agents testing projects | `npx playwright install --with-deps` |
| Playwright browsers | Chromium, Firefox (headless) | Installed via Playwright CLI |
| Playwright system deps | Shared libraries for headless browsers | `npx playwright install-deps` |
| Rust toolchain | Build only (not needed on prod VM if cross-compiling) | rustup |
| systemd | Process management | Built into Debian |
| certbot / custom CA | TLS certificate provisioning | Manual or scripted |

### 7.4 Confirmed Server

| Property | Value |
|----------|-------|
| Hostname | `hl-ironweave` |
| IP | `10.202.28.205` |
| SSH | `ssh paddy@10.202.28.205` (key auth) |
| Install path | `/home/paddy/ironweave/` |
| Data directory | `/home/paddy/ironweave/data/` |
| Available disk | 153 GB (on `/home` LV) |

### 7.5 Deployment Artefact

- Single Rust binary with Svelte static assets (embedded via `rust-embed` or served from a directory)
- Systemd unit file with auto-restart on failure
- SQLite database file at `/home/paddy/ironweave/data/ironweave.db` (backed up via cron + rsync or Litestream)
- Configuration file: `/home/paddy/ironweave/ironweave.toml`

## 8. Open Questions

1. **Auth model** — simple shared password, per-user accounts, or TLS client certs?
2. **Redis** — keep Redis for pub/sub (as multi-term does) or replace with internal event bus via The Loom + WebSockets?
3. **API fallback** — when should agents use API-direct vs CLI? User-configurable per workflow step?
4. **Vault conflict resolution** — last-write-wins acceptable or need merge UI for Obsidian sync?
5. **Agent CLI installation** — how to manage Claude/OpenCode/Gemini CLI versions on the VM?
6. **Worktree cleanup** — aggressive (delete on merge) or lazy (background garbage collection)?
7. **Frontend styling** — Tailwind CSS, custom design system, or component library?

## 9. Prior Art

| Project | Key Ideas Borrowed |
|---------|-------------------|
| **multi-term** | Project configs, agent roles, Redis events |
| **Agent Orchestrator** | Pipeline stages, review cycles, issue lifecycle |
| **Composio** | Stateful orchestration, managed toolsets, CI auto-fix |
| **Overstory** | Git worktrees, merge queue, tiered conflict resolution, runtime adapters |
| **Goosetown** | Town Wall (→ The Loom), subagent delegation, Beads integration |
| **OpenSwarm** | Knowledge graph, cost tracking, Discord control |
| **Beads** | Dependency DAGs, atomic claiming, crash recovery, context compaction |
