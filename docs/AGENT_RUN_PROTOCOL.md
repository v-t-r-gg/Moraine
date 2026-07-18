# Agent run protocol

Compact JSON CLI for agents to manage durable Markdown run records without
rewriting the whole document on every step.

**This document describes current CLI capabilities.** MCP transport is future
work and is not implemented.

## Authority boundary

| Actor | Can do | Cannot do |
| ----- | ------ | --------- |
| Agent (`moraine run …`) | start, checkpoint, ready, resume, show, open | `approved` / `changes_requested` / `rejected`, reviewer identity |
| Human (`moraine decide`, GUI) | review decisions, annotations, suggestions | agent lifecycle (except by editing Markdown) |

Agent `ready_for_review` is **not** human approval.

## Commands

```bash
moraine project init [PATH] --json

moraine run start --objective "…" --idempotency-key "…" [--project PATH] --json
moraine run show --run-id UUID [--project PATH] [--include-markdown] --json
moraine run checkpoint --run-id UUID --expected-hash HEX --idempotency-key "…" --input FILE|- [--project PATH] --json
moraine run ready --run-id UUID --expected-hash HEX --idempotency-key "…" [--summary "…"] [--project PATH] --json
moraine run resume --run-id UUID --expected-hash HEX --idempotency-key "…" [--reason "…"] [--project PATH] --json
moraine run open --run-id UUID [--project PATH] --json
```

Existing `init`, `status`, `decide`, `share`, and file helpers remain.

## Lifecycle

```text
active ──run ready──► ready_for_review ──run resume──► active
```

Human `decide` is independent. Changing Markdown after a decision makes that
decision **stale** (content-hash bound).

## Project discovery

- `project init` is idempotent.
- Prefers Git repository root when available; otherwise the given path or cwd.
- Creates `.moraine/`, `.moraine/runs/`, `.moraine/project.json`, and
  `.moraine/.gitignore` for transient files only.
- Does **not** modify the repository root `.gitignore` by default.
- Never ignores durable run records or their sidecars.
- `run start` auto-initializes the minimal project structure when absent.
- No central database. Git is optional.

## Checkpoint schema

```json
{
  "summary": "required concise string",
  "actions": ["optional"],
  "rationales": [{ "choice": "…", "reason": "…" }],
  "evidence": [{
    "kind": "command_result",
    "label": "…",
    "command": "cargo test …",
    "exitCode": 0,
    "path": null,
    "url": null,
    "provenance": "agent_reported"
  }],
  "risks": [],
  "openQuestions": []
}
```

- `summary` required; size limits apply to fields and collections.
- Agent-reported command results are **not** Moraine-captured evidence.
- Moraine attaches Git context mechanically at checkpoint time.
- No full Markdown replacement; no polished prose required.

## Persistence and recovery

- Structured agent state lives in the sidecar `agent` object (schema **v4**).
- Markdown is a deterministic projection plus a preserved `## Human notes`
  region (plain headings/lists so desktop Markdown round-trips keep structure).
- Mutations after start require `--expected-hash` (exact UTF-8 SHA-256).
- Per-record exclusive lock; re-read; hash check; idempotency; one logical
  mutation; atomic write.
- Incomplete ops use a two-phase sidecar mark → Markdown write → finalize path.
  Ambiguous external change returns `operation_recovery_required`.

## Idempotency

- Every mutating agent op requires `--idempotency-key`.
- Same key + same logical payload → original success (no duplicate content).
- Same key + different payload → `idempotency_conflict`.

## Errors

JSON envelope:

```json
{
  "ok": false,
  "error": {
    "code": "revision_conflict",
    "message": "…",
    "details": { }
  }
}
```

Codes include: `project_not_found`, `run_not_found`, `invalid_checkpoint`,
`revision_conflict`, `idempotency_conflict`, `run_state_conflict`,
`run_record_structure_invalid`, `operation_recovery_required`,
`unsupported_schema_version`.

With `--json`, diagnostics go to stderr; stdout is only the JSON object.

## Token efficiency

- Mutations and default `run show` omit full Markdown.
- Use `--include-markdown` only when necessary.
- Recent checkpoint summaries are capped; aggregate counts are returned.
- Target: typical success responses under ~2 KiB serialized JSON.

## Honest limitations

- Not automatic evidence capture, signing, authenticated agents, or compliance
  audit.
- Markdown + sidecar are not one ACID transaction; recovery is explicit.
- Desktop open scans project sidecars; no review inbox.
- MCP is not implemented.

## Future work

- MCP transport over the same core operations
- Review inbox / multi-run views
- Richer evidence capture (optional, explicit)
