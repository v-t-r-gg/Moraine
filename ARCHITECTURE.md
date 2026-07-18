# Architecture

## Product model

**One markdown file = one collab room.**

**Agent work logs + human audit:** agents write Markdown that records what they did; humans review that record live or later. CLI is the agent surface; GUI is the review surface. See [VISION.md](./VISION.md).

No in-app multi-file workspace. Multiple files = multiple instances.

## Dual-access layout

```
  Agent (logs work in .md)              Human (reviews / audits)
         |                                      |
    moraine CLI                            Tauri / web UI
    write, share, status                   edit, presence, Review
         |                                      |
         +----------- moraine-core -------------+
                       |            |
                   .md files    .md.comments.json
                       |
                moraine-server (optional live Yjs)
```

| Surface | Responsibility |
|---------|----------------|
| `moraine` CLI | File I/O, share/join, status for scripts; JSON + exit codes |
| GUI | Live edit, presence, comments, suggestion accept/reject, host Save |
| `moraine-core` | Documents, history, watcher, rooms, share URLs, annotation sidecar |
| `moraine-server` | In-memory Yjs WebSocket relay |

Core has no Tauri or Axum so agents keep a small CLI.

## Crates

| Piece | Role |
|-------|------|
| `moraine-core` | Domain: files, history, watcher, rooms, share helpers, sidecar |
| `moraine-cli` | Terminal entry for agents and humans |
| `moraine-server` | Yjs relay (Axum) |
| `src-tauri` | Desktop host (IPC, dialogs, FS watcher bridge) |
| `src/` | Svelte UI (Tiptap + Yjs + Review) |

## Data flow

1. **Log (agent):** Markdown on disk is updated during a task (CLI or any tool). That file is the durable work history.
2. **Share (optional live):** `moraine share` prints room/URL so a human can join mid-task. Relay must be up unless `--start`.
3. **Live session:** GUI joins `?room=`; Yjs from `yjsSession.ts` (BroadcastChannel and/or WS).
4. **Host save:** autosave when solo; paused when remote peers present; explicit Save always writes.
5. **Review (human):** comments/suggestions in Yjs map `comments` + marks. Host persists to `file.md.comments.json`. Accept applies text; reject drops mark.
6. **Audit later:** open the same path without peers; sidecar rehydrates list and quote-based marks for hindsight.
7. **Status (agent):** `moraine status` reads path/room, relay health, sidecar counts (not live peers).

## Annotation sidecar

Path: `{markdown_path}.comments.json`  
Example: `work-log.md` -> `work-log.md.comments.json`

Fields: `kind` (`comment` or `suggestion`), `quote`, `body`, author, resolved.

Supports both live review and later audit of what was proposed or discussed.

## Features vs audiences

| Feature | Agent (logging / scripts) | Human (oversight) |
|---------|---------------------------|-------------------|
| Writing `.md` work logs | Primary | Read / lightly edit |
| `share` / `join` / `status --json` | Primary | Convenience |
| Live presence | N/A today | Primary when online |
| Comments + suggestions UI | Indirect (`status` counts) | Primary |
| Host Save under collab | If host process is agent-driven desktop | Primary |

## Quality gate

Prefer changes that improve **agent-written history** or **human review/audit** of that history on a single file. Persistence + tests before the next large feature.

## Non-goals (for now)

Multi-file workspace UI, auth/TLS product, Git/SQLite productization, MCP as required agent API (CLI first).

## Tests

```bash
./scripts/check.sh
cargo test -p moraine-core
cargo test -p moraine-cli
npm test
```
