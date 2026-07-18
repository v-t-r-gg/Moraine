# Moraine

**Review ledger for autonomous agent work.**

Agents write durable Markdown **run records** (what they did, decided, verified, and left open). Humans **review** those records in the desktop or web UI: read, comment, suggest edits, accept or reject suggestions, and Save. Files stay on disk next to the work. Live multiplayer is optional.

Early-stage local-first project. Not a production hosted collaboration service and not a compliance-grade audit system.

Repo: https://github.com/v-t-r-gg/Moraine  
[VISION.md](./VISION.md) · [ARCHITECTURE.md](./ARCHITECTURE.md)

## Who uses what

| Surface | Who | Role today |
|---------|-----|------------|
| CLI (`moraine`) | Agents, scripts, humans in a terminal | Create/inspect files, share a room URL, status, local history helpers |
| GUI (Tauri / `npm run dev`) | Humans | Review run records, comments, suggestions, host Save |
| Markdown + sidecar | Both | Durable narrative + structured annotations |

Collaborative editing supports live review. The durable **run record** is the center of the product, not "another multiplayer Markdown editor."

## Setup

```bash
./scripts/setup-arch.sh   # Arch: rust, node; webkit for desktop
npm install
```

CLI and server do not require WebKit. Desktop does.

## Quick start

```bash
# Optional: live share relay (in-memory; no auth)
cargo run -p moraine-server

# Agent/script: share a run record path (prints join URL)
cargo run -p moraine-cli -- share path/to/run-record.md --json

# Human: open the printed http://localhost:1420/?room=… URL
npm run dev

# Or open the file as desktop host
npm run tauri:dev
# MORAINE_OPEN=/absolute/path/to/run-record.md npm run tauri:dev
```

## Human and agent workflow

Intended loop:

1. An agent performs a **bounded unit of work** (agent run).
2. The agent writes or updates a **run record** Markdown file (actions, decisions, outcomes, open questions).
3. The record may **link** to evidence (logs, PR URLs, command output files). Moraine does not auto-capture evidence today.
4. A human opens the record in Moraine (live via share URL, or later by opening the file).
5. The human comments, suggests text changes, accepts or rejects suggestions, and Saves when host.
6. Files remain for **hindsight review** and for optional Git tracking by the user.

### Example: agent finished a migration investigation

Agent leaves `docs/runs/2026-07-18-pg-migration.md` (see `examples/agent-run-migration.md`). Script or agent:

```bash
moraine share docs/runs/2026-07-18-pg-migration.md --json
moraine status docs/runs/2026-07-18-pg-migration.md
```

Human opens the join URL or the file as host, uses **Review** for questions and suggested wording, then **Save**. Later, anyone can reopen the same path without the relay.

### Agent CLI usage

Verified commands (from current CLI):

```bash
moraine info [--json]
moraine status [path|room] [--json|--human]   # JSON default
moraine share <path> [--start] [--json] [--open] [--ui URL] [--server URL]
moraine join <url|room> [--json] [--no-open]
moraine cat <path>
moraine write <path> [--content TEXT] [--history]   # stdin if --content omitted
moraine edit <path> [--create] [--share]
moraine history <path> [--json] [-n N]
moraine restore <path> <entry-id> [--write]
moraine watch <path>
```

Exit codes: `0` ok, `1` error, `2` not found, `3` relay down.  
With `--json` on share/status/info/join, failures are also JSON: `{"ok":false,"error":"…","code":N}`.

```bash
# Machine-friendly share
moraine share run-record.md --json
# -> ok, path, room, url, ws, server

# Review counts from sidecar (not live peers)
moraine status run-record.md
# -> room, joinUrl, relay.ok, annotations.suggestionsOpen, …

# URL only for another tool
moraine join doc_abc123 --json --no-open
```

There is **no** `moraine run` command. Agents write Markdown with `write`, ordinary tools, or editors.

### Human GUI review

1. Start UI (`npm run tauri:dev` or `npm run dev`).
2. Open a run record (dialog, `MORAINE_OPEN`, or join `?room=`).
3. Read the narrative. Select text → **Comment** or **Suggest**.
4. **Review** sidebar: resolve comments; Accept/Reject suggestions.
5. **Save** as host: writes `.md` and `file.md.comments.json`.
6. Reopen later for hindsight; marks rehydrate from quote text when the text still matches.

Host save: autosave when solo; paused when remote peers are present; explicit Save always.

## Why Moraine

* **Tool independence:** plain files, not locked inside an agent chat UI.
* **Durable readable narrative:** Markdown next to the work.
* **Source-adjacent records:** path chosen by you; Git is optional and external.
* **Live and hindsight review:** share room optional; files remain.
* **CLI for automation:** status/share/write with JSON and exit codes.
* **Structured review metadata:** comments and suggestions in a sidecar, not only freeform prose.
* **No proprietary session viewer required** for the durable record.

Live multiplayer is a convenience, not the main differentiation.

## Current status and limitations

Early-stage MVP. Useful for local experiments and dogfooding, not a production multi-tenant service.

| Area | Today |
|------|--------|
| Auth | None on relay or files |
| Relay | In-memory, local-oriented, no durable server state |
| Reviewer identity | Display name only (local/random in UI) |
| Evidence | Manual links in Markdown only |
| Git | Not integrated (no auto-commit/PR) |
| Annotations | Sidecar + best-effort mark rehydration |
| Concurrent external editors | Limited handling; host Save and dirty flags |
| Security | Do not expose the relay to untrusted networks |
| Production deploy | Not claimed |

## Host save (desktop)

| Situation | Disk |
|-----------|------|
| Solo | Autosave ~1.2s |
| Remote peers | Autosave paused |
| Explicit Save | Always |

## Checks

```bash
./scripts/check.sh
```

## Non-goals (now)

Not a general knowledge-management workspace, full agent observability stack, Git/PR replacement, compliance-grade audit product, or production hosted collab service. Agent narrative is not guaranteed true or complete.

## License

MIT OR Apache-2.0
