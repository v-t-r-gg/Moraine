# Architecture

## Conceptual center

The central object is an **agent run**, represented by a durable **run bundle**:

* Markdown narrative (human-readable projection)
* Structured sidecar `*.md.moraine.json` (run id, agent protocol state, annotations; historical decisions retained for compatibility)
* Optional evidence references or captured artifacts
* Append-only human observations (protocol runs); Legacy free-form document mode for non-protocol Markdown only
* Comments / suggestions / findings

Moraine is a **ledger**, not an approval gate. Live collaboration is optional infrastructure around that record. See [VISION.md](./VISION.md) and the canonical blueprint [docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md](./docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md).

## Interaction surfaces

```
  Agents / scripts                      Humans
        |                                  |
   moraine CLI / MCP                  GUI (Tauri + React)
   project/run protocol,              ledger workspace:
   status, share                      projects → runs → timeline
        |                                  |
        +---------- moraine-core ----------+
                    |            |         |
                run record    ledger    discovery
                 (.md)   (.md.moraine.json) read models
                    |
             moraine-service (capture + rebuildable index cache)
                    |
             moraine-server (optional live relay)
```

Long-term surfaces over the same core:

```text
moraine-core
    ├── JSON CLI (`moraine run …`)
    ├── local STDIO MCP (`moraine mcp`, crate moraine-mcp)
    ├── local service discovery queries (via moraine-service)
    └── desktop human ledger workspace
```

| Surface | Audience | Role |
|---------|----------|------|
| CLI | Agents, scripts | Project/run protocol, share room URL, status, local history helpers; `decide` is legacy/compatibility-only (CLI only) |
| MCP | Coding agents | Same core operations over local STDIO; no decision tools |
| `moraine-service` | Hooks / desktop | Capture spool + **noncanonical** rebuildable project/run index; loopback discovery HTTP; Unix socket for hooks |
| GUI | Humans | Default **ledger workspace** (discover projects/runs, structured timeline, findings, append-only ops). Legacy free-form edit only for non-protocol documents. No decision IPC |
| `moraine-core` | Shared | Domain library: documents, history, rooms, share URLs, run ledger, agent protocol, **discovery read models** |
| `moraine-server` | Optional | In-memory Yjs WebSocket relay; no auth; no disk persistence |

### Local discovery (M5)

* Project identity = Moraine project UUID (paths canonicalized and deduplicated).
* Run summaries and ledger timelines are built in `moraine-core` (single classification path).
* Service `index.json` is a **cache**: safe to delete and rebuild; never the source of truth for run bytes.
* Desktop discovery goes through Tauri commands + `src/shared/api` (no direct service URLs from React).
* Browsing is nonmutating: no schema promotion, no Markdown rewrite, no sidecar mutation.

Business logic belongs in `moraine-core`. CLI, MCP, and Tauri commands call the same core operations. Core has no Tauri or Axum dependency.

## Crates

| Piece | Role |
|-------|------|
| `moraine-core` | Run-record files, local history store, FS watcher, room ids, share helpers, run ledger, agent protocol |
| `moraine-cli` | Terminal API for agents and humans |
| `moraine-mcp` | Local STDIO MCP server over core protocol operations |
| `moraine-server` | Live Yjs relay only |
| `src-tauri` | Desktop host shell (IPC, dialogs, watcher bridge) |
| `src/` | React + TypeScript + Vite review UI (Tiptap + Yjs); Tauri desktop host |

## Flows

### Agent write flow

```text
Agent (CLI or MCP)
    -> run start / checkpoint / ready / resume
    -> durable Markdown projection + sidecar agent state
    -> optional human later opens path or moraine run open --run-id
    -> human comments / notes / suggestions (not a Moraine verdict)
```

Details: [docs/AGENT_RUN_PROTOCOL.md](./docs/AGENT_RUN_PROTOCOL.md), [docs/MCP.md](./docs/MCP.md).

### Live review flow

```text
Agent or human edits (file and/or GUI)
    -> optional moraine share -> join URL
    -> moraine-server (WS) + Yjs in GUI
    -> human Review (comment / suggest / accept / reject suggestion)
    -> host desktop Save -> .md + .md.moraine.json
```

Relay state is not durable. When the process exits, live rooms are gone; files remain.

### Hindsight review flow

```text
Markdown + .moraine.json on disk
    -> open in GUI as host (or re-share later)
    -> load ledger (run id, annotations, optional historical decisions) into UI / Yjs map
    -> rehydrate marks by quote search (best effort)
    -> human inspects, comments, and adds notes without the original agent session
```

## Feature and audience

| Capability | Primary audience | Current role |
|------------|------------------|--------------|
| CLI / MCP operations | Agents and scripts | Create, inspect, share, status using supported commands |
| Desktop / web editor | Humans | Inspect and annotate run records |
| Comments and suggestions | Humans | Structured review feedback; accept/reject text suggestions |
| Markdown persistence | Agents and humans | Durable portable run narrative |
| Sidecar metadata | Review tooling + humans | Run ID, agent protocol state, operation-based annotations |
| Historical run-level decisions | Compatibility | Preserved in sidecars; not extended; not primary UI |
| Live collaboration | Agents and humans | Optional concurrent review via relay (secondary) |
| Local history | Humans | Revisit local snapshots under data dir (not Git) |

## Persistence details

| Store | What | Notes |
|-------|------|--------|
| `.md` file | Narrative | Source of truth for prose |
| `.md.moraine.json` | Run ledger | schema through v4: run id, agent state, annotations; `decisions[]` retained for compatibility |
| `.md.comments.json` | Legacy annotations | Migrated into `.moraine.json` on load |
| `~/.local/share/moraine/history` (typical) | Local edit snapshots | Separate from Git |
| Yjs (memory / live) | Session collab state | Not a server-side durable store |

Content hash: SHA-256 of exact UTF-8 Markdown bytes (no line-ending normalization).

Ledger mutations (init, legacy decide, annotation operations, migration) take a per-document lock file (`*.moraine.json.lock`), re-read after lock, then write via unique temp file + replace. There is no direct truncate-and-rewrite fallback.

Annotations use explicit operations with a per-annotation monotonic `revision` concurrency token (checked increment; overflow errors). Suggestions store a durable disposition: `pending`, `accepting`, `accepted`, `rejected`, or `resolved_legacy` (schema v3). Acceptance is two-phase: begin (reserve + bind content hash), apply and Save Markdown, then complete. Cancel is allowed only while the disk Markdown hash still equals the acceptance base hash; if the document changed, cancel fails with `acceptance_document_changed` and the human may explicitly finalize against the current saved hash. Host Save reconciles the live session by stable ID without deletes; new session IDs always start at revision 1.

`moraine status` is read-only. `moraine init` (or desktop open / legacy decide) creates the ledger.

Legacy migration: copy comments into `.moraine.json`, then rename `.comments.json` to `.comments.json.migrated`.

Durability boundary: temp payload is `sync_all`'d before replace; directory fsync is best-effort on Unix. Not a full durability certification.

One Markdown path maps to one live room id (stable hash of absolute path).

## Host save policy (desktop)

When remote peers are present, autosave pauses; explicit Save still writes. Browser-only mode uses stubs and does not provide real host disk for arbitrary paths the same way.

Desktop file I/O uses **Rust Tauri commands** (trusted local host), not the webview `fs` plugin. Capabilities deliberately omit broad `fs:**` scopes. MCP remains project-confined at process start.

## Current non-goals and limitations

Moraine is **not** currently:

* an approval or rejection system (product center)
* a merge gate or CI/deployment authorizer
* a general knowledge-management workspace
* a complete agent observability platform
* a replacement for Git or pull requests
* a compliance-grade, immutable, authenticated audit trail
* a production-ready hosted collaboration service
* a guarantee that an agent narrative is truthful or complete
* a system with secure multi-user auth on the relay

Also: limited automatic evidence capture; no automatic Git commits; relay has no durable state; reviewer names are not authenticated identities.

## Product surface (C3)

Default desktop shell is **ledger workspace** (projects → runs → timeline / findings / append-only ops). Live collab and free-form document editing are **secondary/frozen** for beta defaults. Installed suite layout and service discovery: [docs/INSTALL.md](./docs/INSTALL.md), [docs/C3_SURFACE_FREEZE.md](./docs/C3_SURFACE_FREEZE.md).

Desktop CSP is explicit in `src-tauri/tauri.conf.json` (loopback service access only; no open `https:` default-src).

## Future direction

Sequence: **C3** surface freeze → **W1** platform abstraction → **W2/W3** Windows portfolio. Second agent adapter is subordinate to that sequence. Blueprint: [docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md](./docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md).

## Quality preference

Prefer improvements that strengthen **run records**, **evidence provenance**, and **human inspection/hindsight** over general editor features or approval workflow.

## Tests

```bash
./scripts/check.sh
cargo test -p moraine-core
cargo test -p moraine-cli
cargo test -p moraine-mcp
npm test
```

MSRV is declared in the workspace `Cargo.toml` (`rust-version = "1.88"`) and checked in CI.
