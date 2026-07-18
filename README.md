# Moraine

Local-first Markdown editor with optional multiplayer. Files on disk stay the source of truth for the host; Yjs handles live collab. CLI first-class. No required cloud.

Repo: https://github.com/v-t-r-gg/Moraine

See [ARCHITECTURE.md](./ARCHITECTURE.md) for crate boundaries and data flow.

## Status

Usable MVP: desktop + CLI + local history + share rooms + host-save policy + basic comments. Relay is in-memory, no auth.

## Layout

```
crates/moraine-core/     domain: files, history, watcher, rooms, share URLs
crates/moraine-cli/      moraine binary
crates/moraine-server/   Yjs WebSocket relay
src-tauri/               desktop (Tauri)
src/                     Svelte UI
```

## Setup (Arch)

```bash
./scripts/setup-arch.sh
# needs: rust, node, and for desktop: webkit2gtk-4.1 gtk3 ...
```

CLI and server do not need WebKit.

```bash
npm install
cargo test -p moraine-core
```

## Run

```bash
# CLI
cargo run -p moraine-cli -- info
cargo run -p moraine-cli -- cat examples/welcome.md

# Share (starts relay if needed, prints join URL)
cargo build -p moraine-server -q
cargo run -p moraine-cli -- share examples/welcome.md
# -> http://localhost:1420/?room=doc_...

# UI (open printed URL; second tab joins the room)
npm run dev

# Desktop
npm run tauri:dev
# MORAINE_OPEN=/path/to/note.md npm run tauri:dev
```

```bash
cargo install --path crates/moraine-cli
moraine share notes.md
```

### Share options

```bash
moraine share notes.md --watch
moraine share notes.md --open
moraine share notes.md --no-start
moraine share notes.md --json
moraine edit notes.md --share
```

| Query | Meaning |
|-------|---------|
| `?room=doc_…` | Join room; WS to default relay |
| `?sync=1` | Default relay, path-based room |
| `?sync=ws://host:3099` | Custom relay base |
| `?sync=0` | Offline (BroadcastChannel only) |

### Host save

| Situation | Disk |
|-----------|------|
| Solo | Autosave ~1.2s |
| Remote peers | Autosave paused |
| Explicit Save | Always (host) |
| Peers leave | Autosave resumes if dirty |

### Comments

Select text -> **Comment** -> sidebar resolve/reopen. Threads live in Yjs (map + marks), so they follow the room.

### Relay only

```bash
npm run server
# or docker compose up --build
curl -s http://127.0.0.1:3099/health
```

## CLI cheat sheet

```bash
moraine info
moraine cat|write|history|restore|watch|share|edit
```

## Checks

```bash
./scripts/check.sh
# or: npm run test:rust && npm test && npm run check
```

## Roadmap

| Phase | Focus | State |
|-------|--------|--------|
| 0–1 | Editor, FS, CLI, local Yjs | done |
| 2 | Relay, share, host-save | done (first cut) |
| 3 | Comments + suggestions | basic comments |
| 4 | SQLite, Git, agent hooks | later |

Not next: auth, TLS, full suggestion mode, P2P.

## License

MIT OR Apache-2.0
