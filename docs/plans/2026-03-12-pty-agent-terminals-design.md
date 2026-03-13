# PTY-Based Agent Terminals — Design

> **Project:** Ironweave
> **Feature:** F1 (Agent Process Manager) — end-to-end
> **Created:** 2026-03-12
> **Status:** Approved

---

## Goal

Replace the current piped-stdout agent spawning with PTY-based bidirectional terminals. Users spawn agents from the web UI and interact with them through xterm.js terminals that provide the full CLI experience (ANSI rendering, keyboard input, resize).

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Terminal mode | Bidirectional (read + write) | Users can interact with agents mid-run, respond to prompts, debug |
| Agent lifecycle | Ephemeral/disposable | Agents are spawned for a task and torn down. No persistent identity. Orchestrator decides what to spawn. |
| Spawn model | Standalone first, team optional | No project/team required to spawn an agent. Simplifies initial E2E. |
| PTY library | `portable-pty` (wezterm) | Mature, actively maintained, supports resize, clean API |
| WebSocket format | Binary frames (PTY data) + text frames (control) | Zero encoding overhead for terminal output, structured control channel |

## Architecture

```
Browser                          Server (hl-ironweave)
┌─────────────┐                  ┌──────────────────────────┐
│  xterm.js   │◄─── binary ────►│  WebSocket handler       │
│             │◄─── text ──────►│    ↕                     │
│  (renders   │                  │  PTY master fd           │
│   ANSI,     │                  │    ↕                     │
│   handles   │                  │  agent CLI process       │
│   keyboard) │                  │  (claude/opencode/gemini)│
└─────────────┘                  └──────────────────────────┘
```

### Data Flow

- **Input (browser → agent):** xterm.js keystrokes → binary WS frame → PTY master write → agent stdin
- **Output (agent → browser):** agent stdout/stderr → PTY master read → binary WS frame → xterm.js render
- **Control (text frames):** resize (`{"type":"resize","cols":N,"rows":N}`), exit (`{"type":"exit","code":N}`), errors

## Component Changes

### RuntimeAdapter Trait

Returns `SpawnedPty` instead of `SpawnedAgent`:

```
SpawnedPty {
    master: portable_pty::MasterPty,    // read/write PTY output/input
    child: portable_pty::Child,          // for kill/wait
    pid: u32,
}
```

The adapter no longer handles stdout/stderr piping — the PTY merges them naturally.

### ProcessManager

- Stores `SpawnedPty` per session (keyed by session ID)
- Provides `get_pty(session_id)` for WebSocket handlers
- Tracks child process for `stop_agent()` kill
- Background task per agent: `child.wait()` → cleanup on exit

### WebSocket Handler

Bidirectional:
- PTY→WS reader: async read from PTY master → binary frames
- WS→PTY writer: binary frames → PTY master write, text frames → parse for control (resize)
- On resize text frame: call `master.resize()`

### Terminal.svelte

- Send keystrokes as binary WebSocket frames
- Binary frames received → `term.write()`
- Text frames received → JSON parse for control events (exit, error)
- On xterm resize → send resize text frame

### AgentSession DB Model

Becomes an audit log:
- Created on spawn: runtime, prompt, working_directory, pid, started_at
- Updated on exit: exit_code, ended_at
- team_id/slot_id nullable (standalone agents don't need them)
- No heartbeat — PTY child.wait() handles lifecycle

## API Routes

| Route | Method | Purpose |
|-------|--------|---------|
| `POST /api/agents/spawn` | POST | Spawn agent, returns session info |
| `GET /api/agents` | GET | List active sessions with basic info |
| `GET /api/agents/{id}` | GET | Get single session details |
| `POST /api/agents/{id}/stop` | POST | Kill process, clean up PTY |
| `GET /ws/agents/{id}` | WS | Bidirectional PTY stream |

Spawn request (simplified for standalone):

```json
{
  "runtime": "claude",
  "working_directory": "/path/to/project",
  "prompt": "Fix the failing tests",
  "env": {}
}
```

Server generates session ID, creates DB row, spawns PTY, returns session info.

## Frontend Changes

- Spawn form: add optional project dropdown to pre-fill working_directory
- Agent cards: show runtime badge, state (running/exited), PID, duration
- Terminal: bidirectional input, exit code display on completion
- Scrollback preserved in browser after agent exits

## Error Handling

| Scenario | Behaviour |
|----------|-----------|
| Agent crash | `child.wait()` detects exit → text frame `{"type":"exit","code":N}` → DB updated → PTY removed |
| WS disconnect | Agent keeps running, output discarded. Reconnect streams from that point (no replay). |
| Multiple tabs | Multiple WS connections fine. Last writer wins for input. |
| Server restart | PTY processes orphaned/killed. DB sessions marked `crashed`. |
| Resize race | `portable-pty` handles safely. Agent gets SIGWINCH. |

## YAGNI Decisions

- **No output replay/buffer** — scrollback only exists in browser xterm. Recording is F6 (v1.1).
- **No input multiplexing** — multiple tabs can connect but no coordination.
- **No checkpoint/resume** — server restart kills all agents. Resumable workflows are F13.
- **No team/project enforcement** — standalone agents for now. Team integration layered later.

## Dependencies

- `portable-pty` crate (Cargo.toml)
- Existing: `axum` WebSocket support, `xterm.js` + `@xterm/addon-fit` in frontend
