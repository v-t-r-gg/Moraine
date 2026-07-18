# Architecture

## Conceptual center

The central object is an **agent run**, represented by a durable **run record**:

* Markdown narrative (what happened)
* Structured ledger `*.md.moraine.json` (run id, revision-bound decisions, comments, suggestions)

Live collaboration is optional infrastructure around that record. See [VISION.md](./VISION.md).

## Interaction surfaces

```
  Agents / scripts                      Humans
        |                                  |
   moraine CLI                        GUI (Tauri / web)
   file ops, status, decide           review, edit, Save
        |                                  |
        +---------- moraine-core ----------+
                    |            |
                run record    ledger
                 (.md)     (.md.moraine.json)
                    |
             moraine-server (optional live relay)
```

| Surface | Audience | Role |
|---------|----------|------|
| CLI | Agents, scripts | Create/inspect files, share room URL, status, decide, local history helpers |
| GUI | Humans | Open run records, run-level decisions, comments/suggestions, host Save |
| `moraine-core` | Shared | Domain library: documents, history, rooms, share URLs, run ledger |
| `moraine-server` | Optional | In-memory Yjs WebSocket relay; no auth; no disk persistence |

Core has no Tauri or Axum dependency.

## Crates

| Piece | Role |
|-------|------|
| `moraine-core` | Run-record files, local history store, FS watcher, room ids, share helpers, run ledger |
| `moraine-cli` | Terminal API for agents and humans |
| `moraine-server` | Live Yjs relay only |
| `src-tauri` | Desktop host shell (IPC, dialogs, watcher bridge) |
| `src/` | Review UI (Tiptap + Yjs) |

## Flows

### Agent write flow

```text
Agent or script
    -> moraine write / cat / ordinary filesystem
    -> Markdown run record (.md)
    -> optional human later opens same path
    -> optional moraine status / decide (run id, content hash, review state)
```

### Live review flow

```text
Agent or human edits (file and/or GUI)
    -> optional moraine share -> join URL
    -> moraine-server (WS) + Yjs in GUI
    -> human Review (run decide / comment / suggest / accept / reject)
    -> host desktop Save -> .md + .md.moraine.json
```

Relay state is not durable. When the process exits, live rooms are gone; files remain.

### Hindsight review flow

```text
Markdown + .moraine.json on disk
    -> open in GUI as host (or re-share later)
    -> load ledger (run id, decisions, annotations) into UI / Yjs map
    -> rehydrate marks by quote search (best effort)
    -> show current vs stale run-level decision from content hash
    -> human reviews without the original agent session
```

## Feature and audience

| Capability | Primary audience | Current role |
|------------|------------------|--------------|
| CLI operations | Agents and scripts | Create, inspect, share, status using supported commands |
| Desktop / web editor | Humans | Review and edit run records |
| Comments and suggestions | Humans | Structured review feedback; accept/reject text suggestions |
| Markdown persistence | Agents and humans | Durable portable run narrative |
| Sidecar metadata | Review tooling + humans | Run ID, revision-bound decisions, operation-based annotations |
| Run-level decisions | Humans | Approve / request changes / reject bound to content hash |
| Live collaboration | Agents and humans | Optional concurrent review via relay |
| Local history | Humans | Revisit local snapshots under data dir (not Git) |

## Persistence details

| Store | What | Notes |
|-------|------|--------|
| `.md` file | Narrative | Source of truth for prose |
| `.md.moraine.json` | Run ledger | schema v2: run id, decisions[], comments[] |
| `.md.comments.json` | Legacy annotations | Migrated into `.moraine.json` on load |
| `~/.local/share/moraine/history` (typical) | Local edit snapshots | Separate from Git |
| Yjs (memory / live) | Session collab state | Not a server-side durable store |

Content hash: SHA-256 of exact UTF-8 Markdown bytes (no line-ending normalization).

Ledger mutations (init, decide, annotation operations, migration) take a per-document lock file (`*.moraine.json.lock`), re-read after lock, then write via unique temp file + replace. There is no direct truncate-and-rewrite fallback.

Annotations use explicit operations (`create`, `update`, `resolve`, `reopen`, `accept_suggestion`, `reject_suggestion`) with a per-annotation monotonic `revision` concurrency token. Host Save reconciles the live session by stable ID without deleting disk-only annotations or blindly replacing the full list. Same-annotation updates with a stale revision return an explicit conflict.

`moraine status` is read-only. `moraine init` (or desktop open / decide) creates the ledger.

Legacy migration: copy comments into `.moraine.json`, then rename `.comments.json` to `.comments.json.migrated`.

Durability boundary: temp payload is `sync_all`'d before replace; directory fsync is best-effort on Unix. Not a full durability certification.

One Markdown path maps to one live room id (stable hash of absolute path).

## Host save policy (desktop)

When remote peers are present, autosave pauses; explicit Save still writes. Browser-only mode uses stubs and does not provide real host disk for arbitrary paths the same way.

## Current non-goals and limitations

Moraine is **not** currently:

* a general knowledge-management workspace
* a complete agent observability platform
* a replacement for Git or pull requests
* a compliance-grade, immutable, authenticated audit trail
* a production-ready hosted collaboration service
* a guarantee that an agent narrative is truthful or complete
* a system with secure multi-user auth on the relay

Also: no automatic evidence capture; no automatic Git commits; relay has no durable state; reviewer names are not authenticated identities.

## Future direction (not implemented)

Possible later work: evidence capture, authenticated reviewer identity, Git helpers, authenticated collab, multi-run review inbox, optional agent protocol adapters. Present as direction only.

## Quality preference

Prefer improvements that strengthen **run records** and **human review/hindsight** over general editor features.

## Tests

```bash
./scripts/check.sh
cargo test -p moraine-core
cargo test -p moraine-cli
npm test
```
