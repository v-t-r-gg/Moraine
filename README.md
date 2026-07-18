# Moraine

Local-first Markdown for **agent work logs + human review**.

Agents document what they did (changes, decisions, outcomes) as plain `.md` while they work. That history stays on disk and can be reviewed **in real time** or **later**. Humans use the desktop/web UI for comments, suggestion accept/reject, and Save. **One file = one room.** No mandatory cloud.

Repo: https://github.com/v-t-r-gg/Moraine  
See [VISION.md](./VISION.md) and [ARCHITECTURE.md](./ARCHITECTURE.md).

## Why two surfaces

| | CLI (`moraine`) | GUI (Tauri / web) |
|--|-----------------|-------------------|
| Built for | Agents logging work; scripts and CI | Humans reviewing and approving |
| Strengths | Write/read files, `share`, `status --json`, exit codes | Presence, Comment/Suggest, Accept/Reject, host Save |
| Durable state | `.md` work log + `file.md.comments.json` | Same files; Yjs for a live session |

Agents produce the record. Humans audit it. Real-time collab is optional glue when both are online.

## Setup

```bash
./scripts/setup-arch.sh   # Arch: rust, node, webkit for desktop
npm install
```

## Quick start

```bash
cargo run -p moraine-server

# Agent: expose a room for a work log (or notes file)
cargo run -p moraine-cli -- share path/to/work-log.md --json

# Human: open the printed join URL (after npm run dev) or open the file as host
npm run dev
# or: npm run tauri:dev
```

## Human + agent workflows

### Agent documents the task (CLI / files)

While working, the agent appends or rewrites Markdown (any editor, `moraine write`, etc.): summary of changes, decisions, open questions. Optional:

```bash
moraine share work-log.md --json
# room + url for a human who wants to watch live

moraine status work-log.md
# room, joinUrl, relay.ok, open comment/suggestion counts from sidecar
```

Exit codes: `0` ok, `1` error, `2` not found, `3` relay down.  
With `--json`, failures also print `{"ok":false,"error":"…","code":N}` on stdout.

```bash
moraine info --json
moraine status work-log.md              # JSON by default
moraine status work-log.md --human
moraine share work-log.md --json
moraine share work-log.md --start --json
moraine join doc_abc --json --no-open
```

`status` is for audit and automation (disk + sidecar + relay). It does not report live peer count (that is UI/Yjs only).

### Human reviews (GUI), live or later

**Live:** open the `?room=` URL while the agent still has the room shared. Presence + Review work as usual.

**Later (hindsight):** open the same `.md` as host (desktop) or open the file again after the fact. Sidecar comments/suggestions rehydrate; missing quotes show as "quote not found." Accept/Reject and Save still apply.

```bash
cargo run -p moraine-server   # only needed for live multiplayer
cargo run -p moraine-cli -- share work-log.md
npm run dev                   # or tauri:dev as host
```

In the UI: read the log, **Comment** / **Suggest**, **Review** Accept/Reject, **Save**.

### Host save (desktop)

| Situation | Disk write |
|-----------|------------|
| Solo | Autosave ~1.2s |
| Remote peers in room | Autosave paused |
| Explicit Save | Always |

## Review (comments + suggestions)

Review is how humans structure oversight on agent-written Markdown (or any collab edit).

| Action | How |
|--------|-----|
| Comment | Select text -> **Comment** |
| Suggest | Select text -> **Suggest** (empty = delete) |
| Accept | **Review** -> Accept (applies replacement) |
| Reject | **Review** -> Reject (drops mark only) |

Session state is Yjs; durable review state is `file.md.comments.json` on host Save (and on add/resolve/accept/reject). Cold open rehydrates marks from quote text when possible.

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
```

## Checks

```bash
./scripts/check.sh
```

## Non-goals

In-app multi-file workspace, auth product, MCP as the only agent path. CLI is the agent path; GUI is the review/audit path. Multi-file = multiple processes.

## License

MIT OR Apache-2.0
