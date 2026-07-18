# Architecture

## Conceptual center

The central object is an **agent run**, represented by a durable **run record**:

* Markdown narrative (what happened)
* Optional sidecar `*.md.comments.json` (comments and suggestions)

Live collaboration is optional infrastructure around that record. See [VISION.md](./VISION.md).

## Interaction surfaces

```
  Agents / scripts                      Humans
        |                                  |
   moraine CLI                        GUI (Tauri / web)
   file ops, share, status            review, edit, Save
        |                                  |
        +---------- moraine-core ----------+
                    |            |
                run record    sidecar
                 (.md)     (.md.comments.json)
                    |
             moraine-server (optional live relay)
```

| Surface | Audience | Role |
|---------|----------|------|
| CLI | Agents, scripts | Create/inspect files, share room URL, status, local history helpers |
| GUI | Humans | Open run records, Review (comments/suggestions), host Save |
| `moraine-core` | Shared | Domain library: documents, history, watcher, rooms, share URLs, sidecar |
| `moraine-server` | Optional | In-memory Yjs WebSocket relay; no auth; no disk persistence |

Core has no Tauri or Axum dependency.

## Crates

| Piece | Role |
|-------|------|
| `moraine-core` | Run-record files, local history store, FS watcher, room ids, share helpers, annotation sidecar |
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
    -> optional moraine status (sidecar counts, room id, relay health)
```

### Live review flow

```text
Agent or human edits (file and/or GUI)
    -> optional moraine share -> join URL
    -> moraine-server (WS) + Yjs in GUI
    -> human Review (comment / suggest / accept / reject)
    -> host desktop Save -> .md + .md.comments.json
```

Relay state is not durable. When the process exits, live rooms are gone; files remain.

### Hindsight review flow

```text
Markdown + sidecar on disk
    -> open in GUI as host (or re-share later)
    -> load sidecar into Yjs map
    -> rehydrate marks by quote search (best effort)
    -> human reviews without the original agent session
```

## Feature and audience

| Capability | Primary audience | Current role |
|------------|------------------|--------------|
| CLI operations | Agents and scripts | Create, inspect, share, status using supported commands |
| Desktop / web editor | Humans | Review and edit run records |
| Comments and suggestions | Humans | Structured review feedback; accept/reject text suggestions |
| Markdown persistence | Agents and humans | Durable portable run narrative |
| Sidecar metadata | Review tooling + humans | Annotations and resolved state |
| Live collaboration | Agents and humans | Optional concurrent review via relay |
| Local history | Humans | Revisit local snapshots under data dir (not Git) |

## Persistence details

| Store | What | Notes |
|-------|------|--------|
| `.md` file | Narrative | Source of truth for prose |
| `.md.comments.json` | Comments/suggestions | Written by host GUI; merge on open |
| `~/.local/share/moraine/history` (typical) | Local edit snapshots | Separate from Git |
| Yjs (memory / live) | Session collab state | Not a server-side durable store |

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

Possible later work: evidence capture, stronger run identity, structured approve/reject decisions, Git helpers, authenticated collab, multi-run review inbox, optional agent protocol adapters. Present as direction only.

## Quality preference

Prefer improvements that strengthen **run records** and **human review/hindsight** over general editor features.

## Tests

```bash
./scripts/check.sh
cargo test -p moraine-core
cargo test -p moraine-cli
npm test
```
