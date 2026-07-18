# Architecture

## Product model

**One markdown file = one collab room.**  
**Dual access:** agents/scripts use the CLI; humans use the GUI for live review.  
See [VISION.md](./VISION.md).

Open multiple instances for multiple files. No in-app workspace tree in core.

## Dual-access layout

```
  Agent / script                    Human
       |                               |
  moraine CLI                     Tauri / web UI
  (files, share, status)          (edit, presence, Review)
       |                               |
       +-------- moraine-core ---------+
                  |        |
              .md files   .md.comments.json
                  |
           moraine-server (optional Yjs relay)
```

| Surface | Responsibility |
|---------|----------------|
| `moraine` CLI | File I/O, share/join URLs, status for automation, history; JSON + exit codes |
| GUI | Tiptap/Yjs editing, presence, comments, suggestion accept/reject, host Save |
| `moraine-core` | Domain only: documents, history, watcher, rooms, share URLs, annotation sidecar |
| `moraine-server` | In-memory Yjs WebSocket relay (no auth) |

Core stays free of Tauri and Axum so the CLI stays light for agents.

## Crates

| Piece | Role |
|-------|------|
| `moraine-core` | Files, history, watcher, room ids, share URLs, annotation sidecar |
| `moraine-cli` | Agent/human terminal entry; thin relay health; optional `--start` spawn |
| `moraine-server` | Yjs WebSocket relay (Axum) |
| `src-tauri` | Desktop host shell (IPC, dialogs, watcher bridge) |
| `src/` | Svelte UI (Tiptap + Yjs + Review) |

## Data flow

1. **Write path (often agent):** CLI or any tool updates `.md` on disk; host desktop may also edit via GUI.
2. **Share path:** `moraine share` prints join URL / room id; relay must be up unless `--start`.
3. **Live path:** GUI joins via `?room=` (and optional WS to relay). Yjs session from `yjsSession.ts`.
4. **Host save:** autosave when solo; paused when remote peers present; explicit Save always writes.
5. **Review path (often human):** annotations in Yjs map `comments` + marks (`kind` comment|suggestion). Host persists to `file.md.comments.json`. Accept applies replacement into the doc; reject drops the mark.
6. **Status path (agent):** `moraine status` reads path/room, relay health, sidecar counts (not live peer count).

## Annotation sidecar

Path: `{markdown_path}.comments.json`  
Example: `notes.md` -> `notes.md.comments.json`

Fields: `kind` (`comment` default, or `suggestion`), `quote`, `body`, author, resolved.

Load on host open (merge by id; live Yjs wins). Write on host Save and on add/resolve/accept/reject. Marks rehydrate by searching for `quote` in the document; missing quotes are orphaned in the UI.

## Features vs audiences

| Feature | Agent | Human |
|---------|-------|-------|
| `cat` / `write` / history | Primary | Occasional |
| `share` / `join` / `status --json` | Primary | Convenience |
| Live cursors / presence | N/A (today) | Primary |
| Comments + suggestions UI | Indirect (sidecar counts via status) | Primary |
| Host Save under collab | If driving the desktop host | Primary |

## Quality gate

No new major feature until the last one has: persistence where needed, a few real tests, and a dogfood pass. Prefer work that makes **single-file human+agent collab** better.

## Non-goals (for now)

Multi-file workspace UI, auth/TLS product, Git/SQLite productization, process supervisor for the relay, MCP as required agent API (CLI first).

## Tests

```bash
./scripts/check.sh
cargo test -p moraine-core
cargo test -p moraine-cli
npm test
```
