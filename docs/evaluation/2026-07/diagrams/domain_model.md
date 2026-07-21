# Domain model (current)

```text
Project (UUID)
  └── Run bundle (run UUID)
        ├── Markdown projection (.md)
        ├── Sidecar (.md.moraine.json)  [schema ≤ 6]
        │     ├── run meta (id, hashes, timestamps)
        │     ├── agent state
        │     │     ├── lifecycle, provisional, captureCoverage
        │     │     ├── checkpoints[] (immutable claims)
        │     │     ├── lifecycle_events[]
        │     │     ├── evidence[]
        │     │     ├── findings[] + finding_events[]
        │     │     ├── append_only_ops[] (obs/amend/supersede/redact)
        │     │     ├── incomplete_op?
        │     │     └── risks / open_questions
        │     ├── comments / annotations
        │     └── historical decisions[] (legacy)
        └── Evidence artifacts (optional files under .moraine/evidence)

Integration (Codex) ──hooks──► Mechanical events ──► Service spool
MCP semantic ops ──────────────────────────────► Core mutations
Session envelope / session_id ──► provisional run binding

Service index.json ──► ProjectSummary / run list (cache)
Discovery read models ──► timeline entries (ephemeral DTOs)
```

## Full per-concept tables

See **[DOMAIN_MODEL_REVIEW.md](../DOMAIN_MODEL_REVIEW.md)** for purpose, storage, identity, mutability, authority, lifecycle, relationships, and public/agent/human representation for:

project, integration, agent session, session envelope, provisional run, confirmed run, checkpoint, mechanical event, evidence, finding, finding response, finding state, human observation, amendment, supersession, redaction, lifecycle, capture coverage, run bundle, Markdown projection, sidecar, evidence artifact, service index, spool event.

## Concept table (summary)

| Concept | Storage | Identity | Mutability | Authority |
|---------|---------|----------|------------|-----------|
| Project | `.moraine/project.json` | UUID | Metadata only | Core init/resolve |
| Integration | hook/config | name string | Config | Adapter + service |
| Agent session | observe/session records | session id | Observed | Hooks + core |
| Session envelope | event payloads | session id | Append | Core + service |
| Provisional run | agent.provisional | run UUID | Reconcile | Hooks + core |
| Confirmed run | MD + sidecar | run UUID | Protocol ops | Core |
| Checkpoint | checkpoints[] | op_id | Immutable body | Core |
| Mechanical event | spool | eventId | Process once | Service |
| Evidence | agent.evidence + files | evidence_id | Append | Core/service |
| Finding | findings[] | finding id | State + responses | Core |
| Finding response | responses[] | response id | Append/idempotent | Core |
| Finding state | enum | — | Explicit change | Core |
| Observation/amend/supersede/redact | append_only_ops | op_id | Append-only | Core |
| Lifecycle | agent.lifecycle | enum | Transitions | Core |
| Capture coverage | agent field | enum | Set by path | Core/service |
| Run bundle | MD+sidecar(+evidence) | run UUID | Locked ops | Project-local |
| Markdown projection | runs/*.md | path/hash | Regenerated | Core render |
| Sidecar | *.moraine.json | path/schema | Locked replace | Core |
| Evidence artifact | .moraine/evidence | path | Write-once | Capture |
| Service index | index.json | revision | Rebuildable cache | Service (noncanonical) |
| Spool event | spool/* | eventId | Once | Service |
| Legacy decision | decisions[] | id | Legacy CLI | Deprecated |

## Redundant / ambiguous

- **Human notes (Markdown region)** vs **human_observation_add** — prefer observations for protocol runs.
- **Comments/annotations** vs **findings** — parallel review mechanisms; freeze annotations.
- **History** vs **append-only ledger** — different purposes; freeze history growth.
- **Share/Yjs room** vs **durable run** — collab secondary.
- **curl discovery probe** vs **Unix socket** — transport accidental complexity.
