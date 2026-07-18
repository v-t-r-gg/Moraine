# Moraine

**Review ledger for autonomous agent work.**

Agents write durable Markdown **run records** (what they did, decided, verified, and left open). Humans **review** those records in the desktop or web UI: read, comment, suggest edits, accept or reject suggestions, and Save. Files stay on disk next to the work. Live multiplayer is optional.

Early-stage local-first project. Not a production hosted collaboration service and not a compliance-grade audit system.

Repo: https://github.com/v-t-r-gg/Moraine  
[VISION.md](./VISION.md) · [ARCHITECTURE.md](./ARCHITECTURE.md) · [ROADMAP.md](./ROADMAP.md)

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
moraine init <path> [--json]
moraine decide <path> --decision <kind> --reviewer <label> [--reason TEXT] [--expected-hash HEX] [--json]
moraine share <path> [--start] [--json] [--open] [--ui URL] [--server URL]
moraine join <url|room> [--json] [--no-open]
moraine cat <path>
moraine write <path> [--content TEXT] [--history]   # stdin if --content omitted
moraine edit <path> [--create] [--share]
moraine history <path> [--json] [-n N]
moraine restore <path> <entry-id> [--write]
moraine watch <path>
```

Decision kinds: `approved`, `changes_requested`, `rejected`.

Exit codes: `0` ok, `1` error, `2` not found, `3` relay down.  
With `--json` on share/status/info/join/decide, failures are also JSON: `{"ok":false,"error":"…","code":N}`.

```bash
# Machine-friendly share
moraine share run-record.md --json
# -> ok, path, room, url, ws, server

# Run review status (run id, content hash, decision state, annotation counts)
moraine status run-record.md
# -> room, run.id, run.contentHash, run.reviewState, review.latestDecision, …

# Record a run-level decision bound to the current Markdown revision
moraine decide run-record.md --decision approved --reviewer "Ada" --json

# URL only for another tool
moraine join doc_abc123 --json --no-open
```

There is **no** `moraine run` command. Agents write Markdown with `write`, ordinary tools, or editors.

### Human GUI review

1. Start UI (`npm run tauri:dev` or `npm run dev`).
2. Open a run record (dialog, `MORAINE_OPEN`, or join `?room=`).
3. Read the narrative. Select text → **Comment** or **Suggest**.
4. **Review** sidebar: resolve comments; Accept/Reject suggestions.
5. **Save** as host: writes `.md` and `file.md.moraine.json` (annotations + decisions).
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

## Run-level review decisions (v0.2 / v0.2.1)

Besides comments/suggestions on selections, a human can record a **run-level** decision for the whole **saved** Markdown revision:

* `approved` / `changes_requested` / `rejected`
* Bound to a **content hash** (SHA-256 of exact UTF-8 Markdown bytes; no line-ending normalization)
* Stored append-only in `file.md.moraine.json` with a stable **run ID**
* Decisions apply only to **persisted** Markdown. The desktop UI disables Approve / Request changes / Reject while the editor is dirty.
* If the Markdown changes later, the decision stays but is reported as **stale** until a new decision is recorded
* Concurrent ledger writers take a per-file lock and re-read before mutating. Sidecar writes use temp-file + replace (no truncate fallback).

```bash
# status is read-only (does not create .moraine.json)
moraine status run.md --json
# run.initialized may be false until:
moraine init run.md --json

moraine decide run.md --decision approved --reviewer "Ada" --reason "verified steps" --json
# optional: --expected-hash <sha256> rejects if disk content differs (revision_conflict)
```

Desktop: **Run review** bar (Approve / Request changes / Reject). This is separate from accepting a text **suggestion**. Save before deciding. External disk edits surface a conflict before Save/Decide.

Legacy `file.md.comments.json` is migrated into `.moraine.json` on **init**, **decide**, desktop open, or comment save (not on `status`). After a successful migration the legacy file is renamed to `file.md.comments.json.migrated`.

## Current status and limitations

Early-stage MVP. Useful for local experiments and dogfooding, not a production multi-tenant service.

| Area | Today |
|------|--------|
| Auth | None on relay or files |
| Relay | In-memory, local-oriented, no durable server state |
| Reviewer identity | User-provided label only (not authenticated) |
| Evidence | Manual links in Markdown only |
| Git | Not integrated (no auto-commit/PR) |
| Annotations | Operation-based mutations; suggestion accept/reject are distinct; two-phase accept |
| Run decisions | Append-only; stale when content hash mismatches |
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

Licensed under the **Apache License, Version 2.0**. See [LICENSE](./LICENSE).
