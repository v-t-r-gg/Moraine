# Agent run protocol

This document defines Moraine’s durable, transport-neutral run contract. Code
owns exact schemas, fields, error variants & live MCP tool inventory.

## Project identity

`moraine project init <path>` creates or discovers one project identity under
`.moraine/project.json`. Initialization is idempotent & registers the canonical
project root for rebuildable discovery.

Read-only operations do not create a project. A process confined to a project
does not silently switch roots.

## Run identity

A run has a UUID, project identity, integration, session namespace & durable
record path. Session binding prevents unrelated prompts from creating duplicate
runs.

Canonical files:

```text
.moraine/runs/<run>.md
.moraine/runs/<run>.md.moraine.json
```

The sidecar is structured authority. Markdown is the readable projection.
Human-notes bytes are preserved across projection updates.

## Lifecycle

A run may be provisional from mechanical activity, then confirmed by an agent
start operation. Repeated starts with the same idempotency key resolve to the
same run.

Typical semantic flow:

```text
start or resume
  → checkpoint
  → finding or correction operations
  → ready
  → later resume when work continues
```

`ready` records that the agent considers its work ready for human inspection.
It is not an approval, merge authorization or deployment verdict.

## Checkpoints & evidence

A checkpoint records a bounded summary of work, actions, evidence references,
risks & open questions. Expected revisions/hashes prevent stale mutation.

Evidence may describe commands, files, URLs or captured artifacts. Sensitive
values are redacted before ordinary storage where the capture path supports it.
Evidence is a claim with provenance; it is not automatic proof of correctness.

Mechanical hooks may establish prompt or tool coverage without a semantic
checkpoint. Capture coverage must state that distinction.

## Findings

Findings are durable review items linked to a run or checkpoint. They have
identity, kind, state, statement & response history.

Finding mutations use idempotency & expected state. Findings survive later
checkpoints, projection rebuilds & supported schema promotion.

Findings record review activity. Their state does not authorize an external
product decision.

## Append-only operations

The ledger supports:

* observation; add a new reviewer statement;
* amend; replace the ordinary projection of a prior claim while retaining both;
* supersede; mark a prior claim as replaced by another claim;
* redact; withhold a target from ordinary views while retaining the operation.

Operations target stable IDs. They do not rewrite an earlier entry in place.
Sequential amendment records the immediate prior content needed to understand
the chain.

## Redaction

Ordinary CLI, MCP, discovery, Markdown & desktop projections omit redacted claim
text. The redaction operation remains visible.

Raw sidecars, Git history, backups & independent evidence files remain forensic
sources. See [../SECURITY.md](../SECURITY.md).

## Idempotency & concurrency

Mutating operations require idempotency keys where retries could duplicate
state. Reusing a key with a different payload is a conflict.

Expected revisions, checkpoint hashes & finding state prevent stale writes.
Mutation paths lock, re-read & atomically replace files. Concurrent valid
operations either serialize safely or return a conflict; they do not silently
drop one mutation.

## Persistence & recovery

Incomplete operations are recoverable or discardable according to their
recorded phase. Read paths represent unsupported schemas, broken sidecars &
recovery-required state without mutating them.

Supported older sidecars remain readable. Mutation promotes them to the current
writable schema through tested compatibility paths. The current schema constant
is `moraine_core::run_meta::SCHEMA_CURRENT_WRITABLE`.

## Capture coverage & fidelity

Legacy `captureCoverage` on the run remains a compact compatibility field with
stable serialized values:

| Value | Meaning |
|---|---|
| `full` | Mechanical and semantic channels were both observed |
| `mechanical_only` | Mechanical activity without semantic confirmation |
| `semantic_only` | Semantic activity without a bound mechanical session |
| `partial` | Both channels exist with a known expected observation gap |
| `unknown` | Available state cannot support a stronger conclusion |

`full` means both primary channels were observed. It is **not** complete
knowledge of everything the agent did.

Read surfaces derive a richer, agent-neutral **capture fidelity report** from
the same durable run + session state:

* **Capability** — can this adapter emit this category of observation?
* **Observation** — did Moraine receive durable facts for this run?

Dimensions include session lifecycle, prompt activity, tool activity, semantic
start, checkpoints, mechanical evidence, agent-reported evidence, and review
findings. Absence alone is not delivery failure. Gaps are factual expected
observations that are missing — not scores, severities, or recommendations.

Exact mechanical observation counts live on the session envelope
(`SESSION_SCHEMA_VERSION` = 3). Older sessions remain readable; historical
count completeness stays false because earlier exact counts are unknowable.
Ordinary reads do not rewrite session files.

Inspect via:

```bash
moraine run coverage <RUN_ID> --project /path/to/project
moraine run coverage <RUN_ID> --project /path/to/project --json
```

MCP tool `run_coverage` returns the same compact facts for the server’s fixed
project. No percentage or universal score is calculated — Moraine has no honest
denominator for “everything the agent did.”

## CLI & MCP mapping

The CLI exposes project/run operations for people, automation & compatibility.
The local MCP maps JSON-RPC requests onto the same `moraine-core` operations.
MCP runs over STDIO; the project root is fixed for the server lifetime.

Clients initialize normally, call `tools/list` for the current inventory & use
returned structured errors for conflicts or invalid state. Documentation does
not copy the tool list because code & tests are authoritative.

The Codex adapter adds managed hooks plus this local MCP; see
[integrations/CODEX.md](integrations/CODEX.md).

## Compatibility

Compatibility covers:

* supported readable sidecar schemas;
* stable run, finding & operation identity;
* persisted transaction & receipt shapes where setup recovery needs them;
* CLI/MCP semantics across presentation changes.

Compatibility does not require preserving every internal Rust type name.
Schema changes need fixtures, promotion tests, non-leak tests & an explicit
migration decision.
