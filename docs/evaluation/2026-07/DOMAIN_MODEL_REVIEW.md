# Domain model review

**Baseline:** `4f8d1e8`  
**Diagram:** [diagrams/domain_model.md](./diagrams/domain_model.md)

For each concept: purpose, canonical storage, identity, mutability, authority, lifecycle, relationships, public / agent / human representation.

## Concepts

### project
- **Purpose:** Bound runs and paths to a Moraine-initialized tree.
- **Storage:** `.moraine/project.json`
- **Identity:** Project UUID
- **Mutability:** Metadata; root path may become unavailable
- **Authority:** Core `init_project` / `resolve_existing_project`
- **Lifecycle:** Created once; may be rescanned; never deleted by discovery
- **Relationships:** Has many runs; referenced by service index
- **Public/agent/human:** UUID + path (local human); agents use project root; no remote registry

### integration
- **Purpose:** Name vendor/adapter (e.g. Codex) for mechanical events.
- **Storage:** Hook payload / spool event fields; docs
- **Identity:** String name (`codex`)
- **Mutability:** Config-level
- **Authority:** Adapter + service intake
- **Lifecycle:** Configured per host
- **Relationships:** Emits mechanical events → service → provisional runs
- **Public/agent/human:** Agent config docs; not a first-class UI entity

### agent session
- **Purpose:** Host agent conversation/session binding for capture.
- **Storage:** Session records / observe APIs; sidecar session fields when present
- **Identity:** Session id string (integration-defined)
- **Mutability:** Observed over time
- **Authority:** Hooks + `session_observe` core ops
- **Lifecycle:** start → prompts/tools → stop
- **Relationships:** May bind provisional/confirmed runs
- **Public/agent/human:** Mostly agent-side; humans see resulting runs

### session envelope
- **Purpose:** Structured session metadata carried with events.
- **Storage:** Event payloads / session state under project or service handling
- **Identity:** Session id
- **Mutability:** Append observations
- **Authority:** Core + service
- **Lifecycle:** Tied to agent session
- **Relationships:** Links to provisional run ensure
- **Public/agent/human:** Internal; not primary desktop object

### provisional run
- **Purpose:** Capture work before semantic `run start` confirms.
- **Storage:** Agent state `provisional: true` on run sidecar
- **Identity:** Run UUID
- **Mutability:** May reconcile/confirm
- **Authority:** Service/hooks + core ensure
- **Lifecycle:** provisional → confirmed/semantic active
- **Relationships:** Same run id as later protocol run when reconciled
- **Public/agent/human:** Shown as provisional in discovery summaries

### confirmed run
- **Purpose:** Semantically started agent-run with objective.
- **Storage:** Run MD + sidecar `agent` block
- **Identity:** Run UUID
- **Mutability:** Structured ops only (protocol)
- **Authority:** Core `run_start` etc.
- **Lifecycle:** active → ready_for_review (and any later descriptive states)
- **Relationships:** Checkpoints, findings, evidence, append-only ops
- **Public/agent/human:** Primary discovery object; MCP/CLI JSON; desktop ledger

### checkpoint
- **Purpose:** Sparse semantic claim about work done.
- **Storage:** `agent.checkpoints[]` (immutable claim body)
- **Identity:** `op_id` UUID
- **Mutability:** Body immutable; current claim via amend/supersede/redact chain
- **Authority:** Core `run_checkpoint`
- **Lifecycle:** Created once; may be amended/superseded/redacted in append-only log
- **Relationships:** Findings target checkpoints; timeline entries
- **Public/agent/human:** Timeline + protocol panel; agents see via show/MCP

### mechanical event
- **Purpose:** Deterministic hook/adapter signal (session_start, tool use, …).
- **Storage:** Spool JSON then processed into core ops
- **Identity:** `eventId` (durable seen markers)
- **Mutability:** Process once
- **Authority:** Service intake
- **Lifecycle:** pending → processed | failed
- **Relationships:** May ensure provisional run / session observe
- **Public/agent/human:** Not shown as first-class UI; affects capture coverage

### evidence
- **Purpose:** Trustworthy captured or linked proof with provenance.
- **Storage:** `agent.evidence[]` + optional `.moraine/evidence/` artifacts
- **Identity:** `evidence_id`
- **Mutability:** Append
- **Authority:** Core evidence APIs / service capture path
- **Lifecycle:** Created with provenance; not silently upgraded
- **Relationships:** May attach to checkpoints; timeline
- **Public/agent/human:** Desktop evidence sections; agent-reported vs captured distinguished

### finding
- **Purpose:** Human descriptive review challenge (not a verdict).
- **Storage:** `agent.findings[]`
- **Identity:** Finding UUID
- **Mutability:** Body fixed; state open/addressed/archived; responses append
- **Authority:** Core create/list/get/respond/state
- **Lifecycle:** open → addressed/archived
- **Relationships:** Targets checkpoint snapshot; responses; ledger events
- **Public/agent/human:** Desktop panel; MCP list/get/respond

### finding response
- **Purpose:** Agent (or author) reply in finding thread.
- **Storage:** Nested under finding `responses[]`
- **Identity:** Response UUID + idempotency key
- **Mutability:** Append; idempotent replay
- **Authority:** Core `respond_to_finding`
- **Lifecycle:** Created once (or replayed)
- **Relationships:** Finding parent
- **Public/agent/human:** Thread UI; MCP

### finding state
- **Purpose:** Workflow label for human tracking (not approval).
- **Storage:** Finding `state` enum
- **Identity:** Enum string
- **Mutability:** Explicit state-change ops + events
- **Authority:** Core
- **Lifecycle:** open/addressed/archived
- **Relationships:** Finding events
- **Public/agent/human:** Filters and badges

### human observation
- **Purpose:** Append-only human note on protocol runs.
- **Storage:** `append_only_ops` with observation relationship
- **Identity:** op_id
- **Mutability:** Append-only (never rewrite prior)
- **Authority:** Core `human_observation_add`
- **Lifecycle:** Created; shown chronologically
- **Relationships:** Optional checkpoint target
- **Public/agent/human:** Protocol ledger panel; replaces free-form Human notes for protocol

### amendment
- **Purpose:** Correct incomplete prior claim without erasing original.
- **Storage:** append_only_ops `run_amend`
- **Identity:** op_id
- **Mutability:** Append; freezes immediate prior claim content
- **Authority:** Core
- **Lifecycle:** Stacks sequentially
- **Relationships:** Targets checkpoint
- **Public/agent/human:** Timeline original → amendment → current

### supersession
- **Purpose:** Replace current claim with new statement; history remains.
- **Storage:** append_only_ops `entry_supersede`
- **Identity:** op_id
- **Mutability:** Append
- **Authority:** Core
- **Lifecycle:** Updates current claim
- **Relationships:** Checkpoint target
- **Public/agent/human:** Timeline + current statement

### redaction
- **Purpose:** Explicitly mark claim content withheld in ordinary UI/APIs.
- **Storage:** append_only_ops `entry_redact`; prior may remain in sidecar for integrity
- **Identity:** op_id
- **Mutability:** Append; current claim becomes `[REDACTED]`
- **Authority:** Core; projections must honor `is_redacted`
- **Lifecycle:** Detectable forever
- **Relationships:** Checkpoint (and finding target projections)
- **Public/agent/human:** Ordinary readers see marker; **main still leaks via finding snapshots** (PR #12)

### lifecycle
- **Purpose:** Operational stage of run (active, ready_for_review, …).
- **Storage:** `agent.lifecycle` + lifecycle_events
- **Identity:** Enum + event op_ids
- **Mutability:** Transitions via protocol ops
- **Authority:** Core
- **Lifecycle:** Not approval states
- **Relationships:** Discovery filters active/ready
- **Public/agent/human:** Run list badges

### capture coverage
- **Purpose:** Honesty about how much automatic capture applied.
- **Storage:** `agent.capture_coverage`
- **Identity:** Enum string
- **Mutability:** Set by capture path
- **Authority:** Core/service derivation
- **Lifecycle:** Run-scoped
- **Relationships:** Discovery filters
- **Public/agent/human:** Run summary field

### run bundle
- **Purpose:** Unit of durable agent work record.
- **Storage:** Pair MD + sidecar (+ optional evidence files)
- **Identity:** Run UUID + paths
- **Mutability:** Via core ops under lock
- **Authority:** Project-local files are canonical
- **Lifecycle:** Created → updated → inspectable forever
- **Relationships:** Belongs to project
- **Public/agent/human:** Discovery list item + detail

### Markdown projection
- **Purpose:** Human-readable narrative projection of structured state.
- **Storage:** `.moraine/runs/*.md`
- **Identity:** Path (content hash of bytes)
- **Mutability:** Regenerated by protocol ops; free-form only in legacy mode
- **Authority:** Core renderer
- **Lifecycle:** Projection of agent state + human notes region (legacy)
- **Relationships:** Sidecar is structured authority for protocol
- **Public/agent/human:** Readable file; not MCP primary write path

### sidecar
- **Purpose:** Structured ledger JSON.
- **Storage:** `*.md.moraine.json`
- **Identity:** Path next to MD; schemaVersion
- **Mutability:** Locked atomic replace
- **Authority:** Core load/write under lock
- **Lifecycle:** Schema ≤ max; promote older; reject newer
- **Relationships:** All structured domains
- **Public/agent/human:** Hidden from casual view; powers all tools

### evidence artifact
- **Purpose:** Optional binary/text artifact file for evidence.
- **Storage:** `.moraine/evidence/…`
- **Identity:** Path + evidence id
- **Mutability:** Write-once style
- **Authority:** Capture path
- **Lifecycle:** Grows with capture
- **Relationships:** Evidence records
- **Public/agent/human:** Linked from UI when present

### service index
- **Purpose:** Rebuildable cache of projects/runs for discovery.
- **Storage:** Spool `index.json` with monotonic `revision`
- **Identity:** File under spool dir
- **Mutability:** Rebuild/rescan only (index-only)
- **Authority:** Service; **noncanonical**
- **Lifecycle:** Delete/rebuild anytime
- **Relationships:** Points at project roots by UUID
- **Public/agent/human:** Powers desktop listing via service; never source of ledger truth

### spool event
- **Purpose:** Durable undelivered/pending mechanical payload.
- **Storage:** spool pending/processed/failed + `seen/*.seen`
- **Identity:** eventId / content hash filename
- **Mutability:** Process once
- **Authority:** Service
- **Lifecycle:** Write → process → mark seen
- **Relationships:** Mechanical event processing
- **Public/agent/human:** Ops/debug only

## Redundancy / ambiguity

| Issue | Recommendation |
|-------|----------------|
| Human notes region vs human_observation | Prefer observations for protocol; freeze notes as legacy |
| Annotations vs findings | Freeze annotation expansion; findings = protocol review |
| History vs append-only ledger | Different purposes; freeze history growth |
| Share room vs run bundle | Freeze share for beta |
| Index vs bundle | Keep index noncanonical messaging strict |

## Delete/combine candidates

- Long-term combine dual review UIs (annotations + findings) for protocol runs
- Do not delete Yjs in panic — freeze first
