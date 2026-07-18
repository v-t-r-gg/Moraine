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

## Share a file (local multiplayer)

```bash
# builds the room id from the absolute path, starts moraine-server if needed
cargo run -p moraine-cli -- share examples/welcome.md

# stdout is the join URL, e.g.
# http://localhost:1420/?room=doc_a1b2c3d4
```

Then:

```bash
npm run dev
# open the printed URL (second browser/tab joins the same Yjs room)
```

Options:

```bash
moraine share notes.md --watch          # keep process open, print FS events
moraine share notes.md --open           # also launch desktop if available
moraine share notes.md --no-start       # fail if relay is down (do not spawn)
moraine share notes.md --json
moraine share notes.md --ui http://localhost:1420 --server http://127.0.0.1:3099
moraine edit notes.md --share           # print share URL then open editor
```

Join URL query params (web UI):

| Param | Effect |
|-------|--------|
| `?room=doc_…` | Join that room; enables WS to `ws://127.0.0.1:3099` |
| `?sync=1` | Enable default relay without forcing a room |
| `?sync=ws://host:3099` | Custom relay |
| `?sync=0` | Force offline (no WS) |

Room id is a stable hash of the absolute file path (same in CLI and UI). Relay is in-memory only.

### Host save policy

| Situation | Disk write |
|-----------|------------|
| Solo (no remote peers) | Autosave ~1.2s after edits |
| Remote peers in the room | Autosave paused; status shows it |
| Explicit **Save** | Always writes (host desktop) |
| Last peer leaves | Autosave resumes if dirty |

Only the desktop host that opened the file path should treat disk as source of truth. Browser joiners edit the shared Yjs doc.

### Comments

1. Select text in the editor
2. **Comment** (toolbar) and type a note
3. Open **Comments** sidebar to resolve / reopen / jump to mark

Threads live in the Yjs doc (`comments` map + inline marks), so they sync across clients in the room. Resolve clears the highlight and marks the thread resolved (still listed if you enable "Show resolved").

### Relay only

```bash
./scripts/server-dev.sh
# or: cargo run -p moraine-server
# or: docker compose up --build
curl -s http://127.0.0.1:3099/health
```

## CLI

```bash
moraine info
moraine cat notes.md
moraine write notes.md --content "# Hi" --history
moraine history notes.md
moraine restore notes.md <uuid> --write
moraine share notes.md
moraine edit notes.md          # desktop if found, else $EDITOR
moraine edit notes.md --share
moraine watch ./docs
```

`moraine edit` looks for `moraine-app` on PATH and for `target/debug|release/moraine-app`. It passes the path via argv and `MORAINE_OPEN`.

## Desktop notes

1. `npm run tauri:dev`
2. Without a startup path, opens `/tmp/moraine-welcome.md`
3. Open / Save / History in the toolbar
4. Same path in two windows uses BroadcastChannel; `moraine share` + join URL uses the WS relay

## Roadmap

| Phase | Focus | Effort (rough) |
|-------|--------|----------------|
| 0–1 | Editor, FS, CLI, local Yjs, history | done |
| 2 | Axum WS server, Docker, share CLI, host-save policy | done (first cut) |
| 3 | Comments + suggestion mode on Yjs | basic comments done; suggestions later |
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
