# Agent run protocol

Compact JSON CLI for agents to manage durable Markdown run records without
rewriting the whole document on every step.

**This document describes current CLI capabilities.** MCP transport is future
work and is not implemented.

## Authority model A (current)

| Region | Source of truth | Human GUI editing |
| ------ | --------------- | ----------------- |
| Objective, protocol status, Git, checkpoints, risks, questions, lifecycle, ready text | Structured sidecar `agent` state; Markdown is a projection | **Read-only in desktop** for protocol records. Comments allowed; suggestion create/accept that rewrites managed text is blocked. |
| `## Human notes` | Exact bytes preserved (LF/CRLF, trailing blanks, Unicode) | Free-form human notes (editable) |

Protocol records are detected via `## Protocol status` + `## Human notes` and the
managed-region notice. Legacy Markdown keeps full edit behavior.

Agent `ready_for_review` is **not** human approval.

## Authority boundary

| Actor | Can do | Cannot do |
| ----- | ------ | --------- |
| Agent (`moraine run …`) | start, checkpoint, ready, resume, show, open | `approved` / `changes_requested` / `rejected`, reviewer identity |
| Human (`moraine decide`, GUI) | review decisions, annotations, suggestions | agent lifecycle commands |

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
- `run show` and `run open` **discover** only; they return `project_not_found`
  without creating `.moraine`.
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

- Scalar fields reject CR/LF and control characters (structure-injection safety).
- Agent evidence cannot claim `moraine_captured` (rejected).
- Moraine attaches Git context mechanically at checkpoint time.
- Size limits apply to fields and collection lengths.

## Persistence and recovery

- Structured agent state lives in the sidecar `agent` object (schema **v4**).
- Markdown is a deterministic projection plus a preserved `## Human notes`
  region (plain headings/lists for Tiptap `html: false`).
- Mutations after start require `--expected-hash` (exact UTF-8 SHA-256).
- Per-record exclusive lock; re-read; hash check; idempotency; one logical
  mutation.

### Two-phase commit (Markdown + sidecar)

1. **Committed** `agent` state remains unchanged except an `incomplete_op` intent
   that holds `pending_agent` (the next state) plus base/expected hashes.
2. Write projected Markdown.
3. On success, **promote** `pending_agent` to committed and record idempotency.
4. Recovery:
   - disk hash == base → discard pending (no mutation committed)
   - disk hash == expected → promote pending exactly once
   - neither → `operation_recovery_required`

A failed Markdown write must **not** leave checkpoints or lifecycle changes
committed.

### Start idempotency

Under the project lock, start **reserves** `run_id`, path, and payload hash with
status `pending` before creating files, then marks `complete` after files exist.
Retries recover the reservation; concurrent same-key starts share one run.

## Idempotency

- Every mutating agent op requires `--idempotency-key`.
- Same key + same logical payload → original success (no duplicate content).
- Same key + different payload → `idempotency_conflict`.
- Compact lifetime index on the run sidecar; no silent eviction of keys.
- If the index reaches a hard ceiling, further new keys fail closed.

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
`unsupported_schema_version`, `desktop_launch_failed`.

With `--json`, diagnostics go to stderr; stdout is only the JSON object.

## Token efficiency

- Mutations and default `run show` omit full Markdown.
- Use `--include-markdown` only when necessary.
- Recent checkpoint summaries, risks, and open questions are capped; totals
  are returned for full counts.
- Target: typical success responses under ~2 KiB serialized JSON.

## Honest limitations

- Not automatic evidence capture, signing, authenticated agents, or compliance
  audit.
- Markdown + sidecar are not one ACID transaction; recovery is explicit.
- Desktop open scans project sidecars; no review inbox.
- MCP is not implemented.
- GUI does not yet hard-disable edits inside managed Markdown regions (Model A
  is documented; enforcement is process/UI follow-up).

## Future work

- MCP transport over the same core operations
- Review inbox / multi-run views
- GUI enforcement of managed vs Human notes regions
- Richer evidence capture (optional, explicit)
