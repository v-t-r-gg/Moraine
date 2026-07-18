# Architecture

Practical map of the repo as it stands. Prefer this over hunting through crates.

## Crates and apps

| Piece | Role | Depends on |
|-------|------|------------|
| `moraine-core` | Pure domain: document I/O, history store, FS watcher, room ids, share URL helpers | std + small libs |
| `moraine-cli` (`moraine`) | CLI for files, history, share, watch | core |
| `moraine-server` | In-memory Yjs WebSocket relay | axum (not core) |
| `src-tauri` (`moraine-app`) | Desktop shell: IPC, dialogs, watcher bridge | core + Tauri |
| `src/` | SvelteKit UI: Tiptap, Yjs, comments, host-save policy | Tauri IPC or browser stubs |

**Rule of thumb:** network protocol and process spawn stay out of core. Core must stay free of Tauri and Axum so CLI and tests stay light.

```
  browser / Tauri webview
           |
      Yjs (BroadcastChannel and/or WS)
           |
    moraine-server (optional relay)
           |
  Tauri IPC  <--->  moraine-core  <--->  .md files + history dir
           |
      moraine CLI
```

## Data flow

1. **Open file (host desktop):** CLI or UI -> `open_document` -> core `Document` + history "open" snapshot -> Yjs session seeded from markdown.
2. **Edit:** Tiptap + Yjs XmlFragment. Local tabs use BroadcastChannel. `?room=` / `?sync=` enable WS to `moraine-server`.
3. **Host save:** Solo: debounced autosave. Peers present: pause autosave; explicit Save still writes. Peers leave: resume if dirty. See `src/lib/editor/hostSave.ts`.
4. **Comments:** Inline mark `comment` in the collab doc + metadata in Yjs map `comments` (`src/lib/editor/comments.ts`). Syncs with the room; not written as separate JSON on disk yet.
5. **Share:** `moraine share path` builds room id from absolute path (`room_id_for_path`), ensures relay (health + optional spawn + PID file under data dir), prints UI join URL.

## Room ids

Same algorithm in Rust (`moraine_core::room`) and TS (`roomIdForPath`): Java-style hash over UTF-16 code units, `doc_{hex}`. Always hash the absolute path when possible.

## Defaults

| Name | Value |
|------|--------|
| Relay HTTP | `http://127.0.0.1:3099` |
| Relay WS | `ws://127.0.0.1:3099` |
| UI (vite) | `http://localhost:1420` |
| Join URL | `{ui}/?room={room}` |

## What deliberately is not here yet

Auth, TLS, SQLite, Git, suggestion mode, multi-file workspaces, P2P.

## Tests

```bash
cargo test -p moraine-core
cargo test -p moraine-cli          # share_flow needs built moraine-server
npm test                           # vitest: collab, hostSave, comments
./scripts/check.sh
```
