# Architecture

## Product model

**One markdown file = one collab room.** Open multiple instances for multiple files. No in-app workspace/folder tree in core.

## Crates

| Piece | Role |
|-------|------|
| `moraine-core` | Files, history, watcher, room ids, share URLs, **comment sidecar** |
| `moraine-cli` | CLI; thin relay health check; optional `--start` spawn only |
| `moraine-server` | In-memory Yjs WebSocket relay (Axum) |
| `src-tauri` | Desktop IPC shell |
| `src/` | Svelte UI (Tiptap + Yjs) |

Core stays free of Tauri and Axum.

## Data flow

1. Host opens `.md` via Tauri IPC / CLI.
2. Yjs session: `resolveSessionConfig` + `createYjsSession` in `yjsSession.ts` (BroadcastChannel and optional WS).
3. Host save: autosave when solo; paused when remote peers present; explicit Save always writes.
4. Annotations: Yjs map `comments` + marks (`kind` comment|suggestion); host persists to `file.md.comments.json`. Accept replaces mark range with suggested body.
5. Share: print join URL; relay must already be up unless `moraine share --start`.

## Annotation sidecar

Path: `{markdown_path}.comments.json`  
Example: `notes.md` -> `notes.md.comments.json`

Each record has `kind` (`comment` default, or `suggestion`), `quote`, `body` (comment text or replacement), author, resolved.

Load on host open (merge by id; live wins). Write on host Save and on add/resolve/accept/reject.

Marks are not rehydrated from sidecar (list still shows quote/body).

## Quality gate

No new major feature until the last one has: persistence (if needed), 2–3 integration tests, and a manual dogfood pass. Prefer changes that make **single-file collab** better.

## Non-goals (for now)

Multi-file workspace, suggestion mode, Git, SQLite, auth, TLS, P2P, process supervisor for the relay.

## Tests

```bash
./scripts/check.sh
cargo test -p moraine-core
cargo test -p moraine-cli
npm test
```
