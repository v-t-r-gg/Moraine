# Moraine

Local-first Markdown collab for **humans + agents**.

Agents and scripts do heavy writing through the **CLI** and the files on disk. Humans review and approve in the **desktop/web UI** (comments, suggestion accept/reject, Save). **One file = one room.** No mandatory cloud.

Repo: https://github.com/v-t-r-gg/Moraine  
See [VISION.md](./VISION.md) and [ARCHITECTURE.md](./ARCHITECTURE.md).

## Why two surfaces

| | CLI (`moraine`) | GUI (Tauri / web) |
|--|-----------------|-------------------|
| Built for | Agents, CI, shell tools | Human review and live editing |
| Strengths | `cat`/`write`, `share`, `status --json`, exit codes | Presence, Comment/Suggest, Accept/Reject, host Save |
| Durable state | Plain `.md` + `file.md.comments.json` | Same files; Yjs for live session |

## Setup

```bash
./scripts/setup-arch.sh   # Arch: rust, node, webkit for desktop
npm install
```

## Quick start

```bash
# Relay (needed for multiplayer join URLs)
cargo run -p moraine-server

# Agent/script: publish a room for a file
cargo run -p moraine-cli -- share examples/welcome.md --json

# Human: open the printed join URL after starting the UI
npm run dev
# or host desktop: npm run tauri:dev
```

## Human + agent workflows

### Agent writes, human reviews

Typical loop:

1. **Agent** edits the markdown file (editor, `moraine write`, or any tool).
2. **Agent** shares the room so a human can join live if needed:
   ```bash
   moraine share notes.md --json
   # {"ok":true,"room":"doc_…","url":"http://localhost:1420/?room=doc_…", ...}
   ```
3. **Agent** (or host) checks review state without a browser:
   ```bash
   moraine status notes.md
   # annotations.suggestionsOpen, commentsOpen, relay.ok, joinUrl
   ```
4. **Human** opens the join URL or the file as host, uses **Review** to comment, Accept/Reject suggestions, then **Save**.

Exit codes for scripts: `0` ok, `1` error, `2` not found, `3` relay down.  
With `--json`, failures also print `{"ok":false,"error":"…","code":N}` on stdout.

```bash
moraine info --json
moraine status notes.md              # JSON by default
moraine status notes.md --human
moraine share notes.md --json
moraine share notes.md --start --json   # spawn relay once if down
moraine join doc_abc --json --no-open   # URL only, no browser
```

`status` does not report live peer count (that is Yjs/UI only). It is reliable for automation: room id, relay health, sidecar review counts.

### Human collab (GUI)

```bash
cargo run -p moraine-server
cargo run -p moraine-cli -- share notes.md
npm run dev   # open the ?room= URL in one or more tabs
```

In the UI: select text, **Comment** or **Suggest**, open **Review**, Accept/Reject, **Save**.

### Host save (desktop)

| Situation | Disk write |
|-----------|------------|
| Solo | Autosave ~1.2s |
| Remote peers in room | Autosave paused |
| Explicit Save | Always |

## Review (comments + suggestions)

Both humans and agent-driven edits land as plain text; review is human-first in the UI.

| Action | How |
|--------|-----|
| Comment | Select text -> **Comment** |
| Suggest | Select text -> **Suggest** (empty replacement = delete) |
| Accept | **Review** -> Accept (applies replacement) |
| Reject | **Review** -> Reject (drops mark only) |

Stored in Yjs during the session and in `file.md.comments.json` on host Save (and on add/resolve/accept/reject). On cold open, marks rehydrate from quote text; if the quote is gone, Review shows "quote not found".

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

In-app multi-file workspace, auth product, MCP as the only agent path. The **CLI is the agent path** today. Multi-file = multiple processes/terminals.

## License

MIT OR Apache-2.0
