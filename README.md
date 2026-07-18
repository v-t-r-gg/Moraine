# Moraine

Local-first, Git-native collaborative Markdown editor. Plain `.md` files on disk, real-time multiplayer via Yjs, history, and a CLI. No mandatory cloud.

**Status:** Phase 0–1 MVP is usable. Phase 2 starts with an optional Yjs WebSocket relay (`moraine-server` on port 3099).

Repo: https://github.com/v-t-r-gg/Moraine

## What works today

| Area | Notes |
|------|--------|
| Desktop | Tauri 2 + SvelteKit + Tailwind (needs WebKitGTK on Linux) |
| Editor | Tiptap / ProseMirror, edit / split / preview |
| Files | Open, save, autosave, atomic writes |
| Watcher | Reload when the file changes on disk (if not dirty) |
| History | Snapshots under `~/.local/share/moraine/history` |
| Local collab | Yjs + BroadcastChannel (same origin tabs) |
| Network collab | Optional `moraine-server` WS relay |
| CLI | `moraine` cat/write/edit/history/restore/watch/info |

## Layout

```
crates/moraine-core/     document I/O, history, watcher
crates/moraine-cli/      moraine binary
crates/moraine-server/   Yjs WebSocket relay
src-tauri/               desktop shell
src/                     Svelte UI
scripts/                 Arch setup, dev helpers
docker-compose.yml       server only
```

## Prerequisites (Arch / Linux)

- Rust 1.77+
- Node 20+ and npm
- For desktop: WebKitGTK 4.1, GTK3

```bash
./scripts/setup-arch.sh
# or:
sudo pacman -S --needed webkit2gtk-4.1 gtk3 base-devel curl wget \
  openssl appmenu-gtk-module libappindicator-gtk3 librsvg \
  rust nodejs npm
```

CLI and server do not need WebKit.

## Quick start

```bash
npm install

# Core + CLI (no desktop deps)
cargo test -p moraine-core
cargo run -p moraine-cli -- info
cargo run -p moraine-cli -- cat examples/welcome.md

# Frontend only (browser stubs for file I/O)
npm run dev
# optional collab: npm run server  (then open with ?sync=1)

# Desktop
npm run tauri:dev
# open a path: MORAINE_OPEN=/path/to/note.md npm run tauri:dev
# or after build: cargo run -p moraine-cli -- edit /path/to/note.md
```

Install CLI:

```bash
cargo install --path crates/moraine-cli
```

## Collab server (optional)

Relay only. No auth, no disk persistence.

```bash
./scripts/server-dev.sh
# or: cargo run -p moraine-server
# or: docker compose up --build

curl -s http://127.0.0.1:3099/health
# WS: ws://127.0.0.1:3099/ws/<room_id>
```

In the UI, enable sync with `?sync=1` (uses `ws://127.0.0.1:3099`) or `?sync=ws://host:3099`.  
Or set `VITE_MORAINE_SYNC_URL=ws://127.0.0.1:3099` before `npm run dev`.

Room id is derived from the document path (same hash as local multi-tab). Host desktop still owns file autosave.

## CLI

```bash
moraine info
moraine cat notes.md
moraine write notes.md --content "# Hi" --history
moraine history notes.md
moraine restore notes.md <uuid> --write
moraine edit notes.md          # desktop if found, else $EDITOR
moraine watch ./docs
```

`moraine edit` looks for `moraine-app` on PATH and for `target/debug|release/moraine-app`. It passes the path via argv and `MORAINE_OPEN`.

## Desktop notes

1. `npm run tauri:dev`
2. Without a startup path, opens `/tmp/moraine-welcome.md`
3. Open / Save / History in the toolbar
4. Same path in two windows uses BroadcastChannel; with server + `?sync=1`, rooms can span processes

## Roadmap

| Phase | Focus | Effort (rough) |
|-------|--------|----------------|
| 0–1 | Editor, FS, CLI, local Yjs, history | done |
| 2 | Axum WS server, Docker, share flow, host-only save policy | in progress (relay first cut) |
| 3 | Comments + suggestion mode on Yjs | after multiplayer feels real |
| 4 | SQLite metadata, git2, agent/MCP hooks | later |

**Not in the next slices:** auth, TLS (put a reverse proxy in front if needed), P2P, full Git auto-commit.

## Dev scripts

```bash
npm run tauri:dev
npm run dev
npm run server          # cargo run -p moraine-server
npm run test:rust
./scripts/dev.sh
./scripts/server-dev.sh
```

## License

MIT OR Apache-2.0
