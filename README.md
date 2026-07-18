# Moraine

Local-first Markdown editor with optional multiplayer. **One file = one room.** Host disk is source of truth; Yjs handles live collab. No required cloud.

Repo: https://github.com/v-t-r-gg/Moraine · see [ARCHITECTURE.md](./ARCHITECTURE.md)

## Setup

```bash
./scripts/setup-arch.sh   # Arch: rust, node, webkit for desktop
npm install
```

## Run

```bash
# Relay (required for share)
cargo run -p moraine-server
# or: npm run server

# Share a file (fails clearly if relay is down)
cargo run -p moraine-cli -- share examples/welcome.md
# optional: --start  (spawn relay once, then print URL)
# optional: --json / --open

# Join in the browser
npm run dev
# open the printed http://localhost:1420/?room=doc_… URL

# Desktop host
npm run tauri:dev
```

```bash
moraine join doc_abc123          # open room URL in browser
moraine edit notes.md --share    # print URL (relay up) + open editor
```

## Host save

| Situation | Disk |
|-----------|------|
| Solo | Autosave ~1.2s |
| Remote peers | Autosave paused (status bar) |
| Explicit Save | Always |

## Review (comments + suggestions)

| Action | How |
|--------|-----|
| Comment | Select text -> **Comment** -> note |
| Suggest | Select text -> **Suggest** -> replacement text |
| Accept | **Review** sidebar -> Accept (applies replacement) |
| Reject | **Review** sidebar -> Reject (drops mark only) |

Stored in the same Yjs map and sidecar (`file.md.comments.json`) with `kind: "comment" | "suggestion"`. Host Save still respects peer presence for the markdown file; suggestion accept marks the doc dirty and schedules autosave when solo.

## CLI

```bash
moraine info|cat|write|history|restore|watch
moraine share <path> [--start] [--json] [--open]
moraine join <url|room>
moraine edit <path> [--share]
```

## Checks

```bash
./scripts/check.sh
```

## Non-goals

In-app multi-file workspace, auth, full suggestion mode (yet). Multiple terminals/instances cover multi-file.

## License

MIT OR Apache-2.0
