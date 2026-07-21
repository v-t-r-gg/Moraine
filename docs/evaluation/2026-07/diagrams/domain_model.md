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

## Concept table (summary)

| Concept | Storage | Identity | Mutability | Authority |
|---------|---------|----------|------------|-----------|
| Project | `.moraine/project.json` | UUID | Metadata only | Core init/resolve |
| Run | MD + sidecar | UUID | Append structured ops; MD re-rendered | Core ops under lock |
| Checkpoint | sidecar `checkpoints[]` | op_id | Immutable claim body | Core checkpoint |
| Provisional run | agent.provisional | run UUID | Confirm via protocol | Hooks + core |
| Evidence | agent.evidence + files | evidence_id | Append | Core / service |
| Finding | agent.findings | finding id | State + responses | Core |
| Observation/amend/supersede/redact | append_only_ops | op_id | Append-only | Core |
| Service index | spool `index.json` | revision | Rebuildable cache | Service |
| Spool event | spool/*.json | eventId | Processed once (seen/) | Service |
| Markdown projection | runs/*.md | path | Regenerated from agent state | Core render |
| Legacy decision | decisions[] | id | Legacy CLI only | Deprecated product path |

## Redundant / ambiguous

- **Human notes (Markdown region)** vs **human_observation_add** — product moved to observations for protocol runs; notes still appear in renderer/docs residue.
- **Comments/annotations** vs **findings** — parallel review mechanisms; both implemented.
- **History (local edit history)** vs **append-only ledger** — different purposes; both present.
- **Share/Yjs room** vs **durable run** — live collab is secondary and not the product center.
- **curl-based discovery probe** vs **Unix socket** — transport split is accidental complexity for desktop.
