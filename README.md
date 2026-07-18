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
# Terminal A: collab relay
cargo run -p moraine-server

# Terminal B: share a file (prints join URL)
cargo run -p moraine-cli -- share examples/welcome.md
# open http://localhost:1420/?room=doc_… after: npm run dev

# Desktop host (optional)
npm run tauri:dev
```

## Host save

| Situation | Disk |
|-----------|------|
| Solo | Autosave ~1.2s |
| Remote peers | Autosave paused (status bar) |
| Explicit Save | Always |

## Review (comments + suggestions)

1. Select text in the editor.
2. **Comment** (note) or **Suggest** (replacement; empty = delete that text).
3. Open **Review**: resolve comments, or **Accept** / **Reject** suggestions.
4. Host **Save** writes the doc and `file.md.comments.json`.

Accept applies the replacement and marks the doc dirty (autosave runs when solo). Reject drops the highlight only. On cold open, marks rehydrate from quote text; if the quote is gone, Review shows "quote not found".

## Human + agent workflows

### Human (GUI)

```bash
cargo run -p moraine-server
cargo run -p moraine-cli -- share notes.md --open   # if desktop is built
# or: npm run dev and open the printed ?room= URL in two tabs
```

In the UI: edit, Comment/Suggest, Review accept/reject, Save.

### Agent / scripts (CLI)

Exit codes: `0` ok, `1` error, `2` not found, `3` relay down.  
With `--json`, errors are also JSON on stdout: `{"ok":false,"error":"…","code":3}`.

```bash
# Version + dirs
moraine info --json

# Path, room id, join URL, sidecar counts (default JSON)
moraine status notes.md
# {
#   "ok": true,
#   "room": "doc_…",
#   "joinUrl": "http://localhost:1420/?room=doc_…",
#   "relay": { "url": "http://127.0.0.1:3099", "ok": true },
#   "annotations": { "suggestionsOpen": 1, "commentsOpen": 0, ... }
# }
moraine status notes.md --human

# Share for another client (stdout URL, or full object with --json)
moraine share notes.md --json
moraine share notes.md --start --json   # spawn relay once if down

# Join URL only (no browser)
moraine join doc_abc123 --json --no-open
```

Status does not include live peer count (that is UI/Yjs only). It is safe for automation: room id, relay health, and review counts from the sidecar.

## CLI cheat sheet

```bash
moraine info [--json]
moraine status [path|room] [--json|--human]
moraine share <path> [--start] [--json] [--open]
moraine join <url|room> [--json] [--no-open]
moraine edit <path> [--share]
moraine cat|write|history|restore|watch
```

```bash
moraine --help
moraine status --help
moraine share --help
moraine join --help
```

## Checks

```bash
./scripts/check.sh
```

## Non-goals

In-app multi-file workspace, auth, MCP server. Multiple terminals cover multi-file.

## License

MIT OR Apache-2.0
