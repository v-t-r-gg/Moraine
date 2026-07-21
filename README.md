# Moraine

**Local-first ledger for autonomous agent work.**

Moraine preserves durable, source-adjacent records of coding-agent runs: what the agent did, why, what evidence exists, what remains uncertain, and what humans noted. Files stay on disk next to the work.

**Moraine records review activity; it does not render the verdict.** It is not an approval, merge, or deployment gate.

Early-stage local-first project. Not a production hosted collaboration service and not a compliance-grade audit system.

Repo: https://github.com/v-t-r-gg/Moraine  
[VISION.md](./VISION.md) · [ARCHITECTURE.md](./ARCHITECTURE.md) · [ROADMAP.md](./ROADMAP.md) · [Development blueprint](./docs/DEVELOPMENT_BLUEPRINT.md)

## Who uses what

| Surface | Who | Role today |
|---------|-----|------------|
| CLI (`moraine`) | Agents, scripts, humans in a terminal | Agent run protocol, share room URL, status, local history helpers |
| MCP (`moraine mcp`) | Coding agents (e.g. Codex) | Same protocol over local STDIO |
| GUI (Tauri + React / `npm run dev`) | Humans | Inspect run records, comments, suggestions, findings, human notes, host Save |
| Markdown + sidecar | Both | Durable narrative + structured ledger |

Collaborative editing supports live inspection. The durable **run record** is the center of the product, not "another multiplayer Markdown editor."

## Setup

```bash
./scripts/setup-arch.sh   # Arch: rust, node; webkit for desktop
npm install
```

CLI and server do not require WebKit. Desktop does.

**Rust MSRV:** `1.88` (workspace `rust-version`). Required by `rmcp` 2.2 (edition 2024) and `rmcp-macros` → `darling`. CI runs an MSRV job.

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
2. The agent starts a run and records sparse checkpoints (CLI or MCP)—no per-task Moraine prompt ceremony after one-time setup.
3. The record may **link** or capture evidence (logs, PR URLs, command results). Provenance is shown explicitly.
4. A human opens the record in Moraine (live via share URL, or later by opening the file).
5. The human comments, challenges claims, suggests text changes, edits **Human notes**, and Saves when host.
6. Files remain for **hindsight review** and for optional Git tracking by the user.

External systems (PR, CI, the agent session) retain responsibility for merge and disposition.

### Example: agent finished a migration investigation

Agent leaves `docs/runs/2026-07-18-pg-migration.md` (see `examples/agent-run-migration.md`). Script or agent:

```bash
moraine share docs/runs/2026-07-18-pg-migration.md --json
moraine status docs/runs/2026-07-18-pg-migration.md
```

Human opens the join URL or the file as host, uses **Review** for questions and suggested wording, then **Save**. Later, anyone can reopen the same path without the relay.

### MCP (local STDIO)

Coding agents can use the same agent-run protocol without shelling out:

```bash
moraine mcp --project /absolute/path/to/project
```

Tools: `run_start`, `run_show`, `run_checkpoint`, `run_ready`, `run_resume`.  
No decision/approval tools.  
See [docs/MCP.md](./docs/MCP.md) and [docs/integrations/CODEX.md](./docs/integrations/CODEX.md).

### Agent CLI usage

Preferred **agent run protocol** (compact JSON; no full Markdown rewrite):

```bash
moraine project init [PATH] --json
moraine run start --objective "…" --idempotency-key "…" [--project PATH] --json
moraine run checkpoint --run-id UUID --expected-hash HEX --idempotency-key "…" --input FILE|- --json
moraine run show --run-id UUID [--include-markdown] --json
moraine run ready --run-id UUID --expected-hash HEX --idempotency-key "…" [--summary "…"] --json
moraine run resume --run-id UUID --expected-hash HEX --idempotency-key "…" [--reason "…"] --json
moraine run open --run-id UUID --json
```

See [docs/AGENT_RUN_PROTOCOL.md](./docs/AGENT_RUN_PROTOCOL.md).

Other verified commands:

```bash
moraine info [--json]
moraine status [path|room] [--json|--human]   # JSON default
moraine init <path> [--json]                  # per-file ledger ensure
moraine share <path> [--start] [--json] [--open] [--ui URL] [--server URL]
moraine join <url|room> [--json] [--no-open]
moraine cat <path>
moraine write <path> [--content TEXT] [--history]   # stdin if --content omitted
moraine edit <path> [--create] [--share]
moraine history <path> [--json] [-n N]
moraine restore <path> <entry-id> [--write]
moraine watch <path>
```

Agent lifecycle `ready_for_review` means the run is ready for human **inspection**. It is **not** human approval.

Exit codes: `0` ok, `1` error, `2` not found, `3` relay down.  
With `--json`, failures are structured JSON on stdout; diagnostics on stderr.

```bash
# Start a protocol run (auto-creates .moraine when needed)
moraine run start --objective "Fix flaky test" --idempotency-key "run-1" --json

# Machine-friendly share
moraine share run-record.md --json
```

### Human GUI review

1. Start UI (`npm run tauri:dev` or `npm run dev`).
2. Open a run record (dialog, `MORAINE_OPEN`, or join `?room=`).
3. Read the narrative. Select text → **Comment** or **Suggest**.
4. **Review** sidebar: resolve comments; Accept/Reject suggestions.
5. Edit **Human notes** when present. **Save** as host: writes `.md` and `file.md.moraine.json`.
6. Reopen later for hindsight; marks rehydrate from quote text when the text still matches.

Host save: autosave when solo; paused when remote peers are present; explicit Save always.

## Why Moraine

* **Tool independence:** plain files, not locked inside an agent chat UI.
* **Durable readable narrative:** Markdown next to the work.
* **Source-adjacent records:** path chosen by you; Git is optional and external.
* **Live and hindsight review:** share room optional; files remain.
* **CLI and MCP for automation:** status/share/protocol with JSON and exit codes.
* **Structured review metadata:** comments and suggestions in a sidecar, not only freeform prose.
* **No proprietary session viewer required** for the durable record.
* **No verdict:** inspection and context, not merge authorization.

Live multiplayer is a convenience, not the main differentiation.

## Legacy: run-level decisions (compatibility only)

Older sidecars may contain append-only run-level decisions (`approved` / `changes_requested` / `rejected`) bound to a content hash. That data is **preserved and still loadable**. The product no longer centers on recording new decisions.

* Prefer comments, suggestions, human notes, and (upcoming) findings.
* `moraine decide` remains available as **legacy / compatibility-only** (CLI only; stderr warning on use).
* Decisions are **not** exposed through MCP or the desktop UI IPC.
* Accepting a text **suggestion** is unrelated to run-level authorization.
* Legacy `file.md.comments.json` is migrated into `.moraine.json` on **init**, legacy **decide**, desktop open, or comment save (not on `status`).

```bash
# status is read-only (does not create .moraine.json)
moraine status run.md --json
moraine init run.md --json
```

## Current status and limitations

Early-stage MVP. Useful for local experiments and dogfooding, not a production multi-tenant service.

| Area | Today |
|------|--------|
| Auth | None on relay or files |
| Relay | In-memory, local-oriented, no durable server state |
| Reviewer identity | User-provided label only (not authenticated) |
| Evidence | Mostly manual links in Markdown; capture is near-term work |
| Git | Not integrated (no auto-commit/PR) |
| Annotations | Operation-based mutations; suggestion accept/reject are distinct; two-phase accept |
| Run decisions | Compatibility only; preserved in sidecars; not primary UI |
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

Not an approval system, merge gate, general knowledge-management workspace, full agent observability stack, Git/PR replacement, compliance-grade audit product, or production hosted collab service. Agent narrative is not guaranteed true or complete.

## License

Licensed under the **Apache License, Version 2.0**. See [LICENSE](./LICENSE).
