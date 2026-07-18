# Moraine

**Local-first, Git-native collaborative Markdown editor** — Google Docs for plain `.md` files.

Moraine prioritizes files that already live on disk (or in a Git repo), real-time multiplayer via CRDTs (Yjs), comments/suggestions, edit history, and a first-class CLI. No mandatory cloud.

> **Status:** Phase 0–1 MVP skeleton — Tauri app + ProseMirror/Tiptap editor, file I/O, watcher, Yjs multi-tab simulation, simple history, and CLI.

## Features (this MVP)

| Area | What’s included |
|------|-----------------|
| Desktop | Tauri 2 + SvelteKit + Tailwind |
| Editor | Tiptap (ProseMirror) + GFM-oriented Markdown |
| Files | Open / save / auto-save, atomic writes |
| Watcher | `notify`-based FS events → UI reload |
| Collab (local) | Yjs + BroadcastChannel multi-tab simulation |
| History | Snapshot log under `~/.local/share/moraine/history` |
| CLI | `moraine` binary: `cat`, `write`, `edit`, `history`, `watch`, `info` |
| Preview | Live Markdown preview (edit / split / preview) |

## Repository layout

```
Moraine/
├── Cargo.toml                 # Rust workspace
├── crates/
│   ├── moraine-core/          # FS, document model, history, watcher
│   └── moraine-cli/           # `moraine` CLI binary
├── src-tauri/                 # Tauri 2 desktop shell (depends on core)
├── src/                       # SvelteKit frontend (Tiptap + Yjs)
├── examples/                  # Sample Markdown
├── scripts/                   # Arch setup + dev helpers
└── package.json
```

## Prerequisites (Linux / Arch first)

- **Rust** 1.77+ (`rustc`, `cargo`)
- **Node.js** 20+ and **npm**
- **Tauri 2 system deps** (WebKitGTK 4.1, GTK3, etc.)

On Arch:

```bash
chmod +x scripts/setup-arch.sh
./scripts/setup-arch.sh
```

Or install manually:

```bash
sudo pacman -S --needed webkit2gtk-4.1 gtk3 base-devel curl wget \
  openssl appmenu-gtk-module libappindicator-gtk3 librsvg \
  rust nodejs npm
```

See also: [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/).

## Quick start

```bash
# Install JS deps
npm install

# Build & test Rust core + CLI (no WebKit required)
cargo test -p moraine-core
cargo run -p moraine-cli -- info
cargo run -p moraine-cli -- cat examples/welcome.md

# Frontend only (browser stubs for file I/O)
npm run dev

# Full desktop app
npm run tauri:dev
```

Install the CLI locally:

```bash
cargo install --path crates/moraine-cli
moraine --help
```

## CLI

```bash
moraine info
moraine cat notes.md
moraine write notes.md --content "# Hi" --history
echo "body" | moraine write notes.md
moraine history notes.md
moraine history notes.md --json
moraine restore notes.md <entry-uuid> --write
moraine edit notes.md              # desktop if available, else $EDITOR
moraine edit notes.md --share      # reserved for multiplayer (stub)
moraine watch ./docs
```

## Desktop usage

1. `npm run tauri:dev`
2. First launch opens `/tmp/moraine-welcome.md`
3. **Open** — pick any `.md` file
4. Edit with the rich toolbar-free editor; **Save** or wait for auto-save (~1.2s)
5. **Preview** / **Split** for rendered Markdown
6. **History** — restore prior snapshots
7. Open the same file path in a second window/tab to exercise Yjs presence (BroadcastChannel)

## Architecture (MVP)

```
┌──────────────────────────────────────────────────────────┐
│  Svelte UI  ·  Tiptap / ProseMirror  ·  Yjs (local)      │
└─────────────────────────┬────────────────────────────────┘
                          │ Tauri IPC (invoke / events)
┌─────────────────────────▼────────────────────────────────┐
│  src-tauri (moraine-app)                                 │
│    commands · AppState · file-changed events             │
└─────────────────────────┬────────────────────────────────┘
                          │
┌─────────────────────────▼────────────────────────────────┐
│  moraine-core                                            │
│    Document I/O · HistoryStore · FileWatcher (notify)    │
└──────────────────────────────────────────────────────────┘
          ▲
          │ shared
┌─────────┴────────┐
│  moraine-cli     │
└──────────────────┘
```

**Later phases:** Yjs over WebSockets (Axum), comments/suggestions as shared types, Git (`git2`), SQLite metadata, Docker self-host, MCP/Ollama agent hooks, optional P2P.

## Development scripts

| Command | Purpose |
|---------|---------|
| `npm run tauri:dev` | Desktop dev (Vite + Tauri) |
| `npm run dev` | Frontend only |
| `npm run cli -- <args>` | Run CLI via cargo |
| `npm run test:rust` | Core/CLI tests |
| `npm run lint:rust` | Clippy on core + CLI |
| `./scripts/dev.sh` | Same as `tauri:dev` |
| `./scripts/dev.sh cli info` | CLI helper |
| `./scripts/dev.sh web` | Vite only |

## Roadmap

1. **Phase 0–1 (current)** — skeleton, editor, FS, CLI, local Yjs, history  
2. **Phase 2** — real-time collab (Axum WebSocket server + auth)  
3. **Phase 3** — comments, suggestion mode (track changes)  
4. **Phase 4** — Git integration (auto-commit, branches, PR-friendly diffs)  
5. **Phase 5** — polish, Docker compose server, agent/MCP hooks  

## Design principles

- **Files-first** — the `.md` on disk is the source of truth  
- **Local-first** — no mandatory cloud; self-host optional  
- **Git-native** — play well with existing repos  
- **Minimal bloat** — Rust core, thin UI  
- **Agent-friendly** — CLI + future MCP  

## License

MIT OR Apache-2.0
