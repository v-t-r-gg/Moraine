# Agent run protocol

Compact JSON CLI and local STDIO MCP for durable **agent run** ledgers. Agents do not rewrite full Markdown on every step; humans inspect a structured projection and append-only review ops—not free-form “Human notes + Save” as the primary path.

Related: [MCP.md](./MCP.md) · [REDACTION.md](./REDACTION.md) · [integrations/codex/](./integrations/codex/) · [INSTALL.md](./INSTALL.md).

---

## Authority model (current)

Protocol runs are **append-only** for review context.

| Region / surface | Source of truth | Human interaction |
|------------------|-----------------|-------------------|
| Objective, lifecycle, Git context, checkpoints, risks, questions, ready text | Structured sidecar (`agent` state); Markdown is a **projection** | Desktop presents managed protocol content as structured ledger; free-form rewrite of managed regions is not the product path |
| Checkpoints & agent claims | Immutable once committed (correct via amend / supersede / redact) | Inspect; create findings against targets |
| Human context on protocol runs | **Append-only** `human_observation_add` (and related append ops) | Structured observation UI / CLI ops—not an editable Human notes blob as the durable write path |
| Findings | Structured finding records + events | Create, respond, state change; ordinary views respect redaction |
| Evidence | Checkpoint-linked items; mechanical capture where configured | Inspect with provenance labels |
| Legacy free-form Markdown | Non-protocol documents only | **Historical/compatibility surface** — host Save / free-form edit may still apply to non-protocol docs |

Legacy detection of `## Protocol status` + `## Human notes` regions may still appear in older projections; **new protocol work uses the structured ledger model**, not free-form Human notes as the durable human write path.

`ready_for_review` means ready for human **inspection**. It is **not** approval, merge authority, or deployment authorization.

---

## Authority boundary

| Actor | Can do | Cannot do |
|-------|--------|-----------|
| Agent (`moraine run …` / MCP) | start, checkpoint, ready, resume, show, open; findings list/get/respond via MCP | product approval/rejection, merge authority, authenticated identity |
| Human (desktop / CLI) | inspect ledger, append observations, findings, amend/supersede/redact | agent lifecycle as “approval” |
| Human (`moraine decide`) | **legacy compatibility** decisions only | primary product workflow |

---

## Schema

- Current writable sidecar schema: **v6** (`SCHEMA_VERSION` in `moraine-core`).
- Readable range: minimum **3** through current maximum readable (**6**); see suite `manifest.json` / `moraine version --json`.
- v4 sidecars load with empty findings defaults; v5+ carry findings; v6 continues findings + append-only ops evolution.

Do not assume “schema v4 only” in new docs or integrations.

---

## Commands (CLI)

```bash
moraine project init [PATH] --json

moraine run start --objective "…" --idempotency-key "…" [--project PATH] --json
moraine run show --run-id UUID [--project PATH] [--include-markdown] --json
moraine run checkpoint --run-id UUID --expected-hash HEX --idempotency-key "…" --input FILE|- [--project PATH] --json
moraine run ready --run-id UUID --expected-hash HEX --idempotency-key "…" [--summary "…"] [--project PATH] --json
moraine run resume --run-id UUID --expected-hash HEX --idempotency-key "…" [--reason "…"] [--project PATH] --json
moraine run open --run-id UUID [--project PATH] --json
```

Installed product helpers (C2):

```bash
moraine version [--verbose|--json]
moraine setup
moraine doctor [--project PATH] [--integration codex] [--json]
moraine service install|start|stop|restart|status|logs|uninstall
moraine integrate codex --project PATH [--check|--remove|--dry-run|--json]
moraine open [--path PATH] [--run-id ID] [--project PATH]
```

`moraine decide` remains **legacy / compatibility-only** (stderr warning; not MCP).

---

## MCP (implemented)

Local STDIO MCP is **implemented** (`moraine mcp --project /absolute/path`).

**Do not hardcode a five-tool list.** Ask the live server (`tools/list`) or `moraine doctor --project . --integration codex --json` (`integration.codex.mcp_tools`).

Current implementation tools include:

| Tool | Role |
|------|------|
| `run_start` | Start or reconcile provisional run (`sessionId` optional) |
| `run_show` | Compact run packet |
| `run_checkpoint` | Sparse checkpoint |
| `run_ready` | Lifecycle → ready_for_review |
| `run_resume` | Resume active work |
| `list_findings` | List findings |
| `get_finding` | Finding detail |
| `respond_to_finding` | Human/agent response on a finding |

No decision/approval MCP tools. See [MCP.md](./MCP.md).

---

## Lifecycle

```text
active ──run ready──► ready_for_review ──run resume──► active
```

Lifecycle is operational stage, not approval. Historical run-level decisions in sidecars remain readable; content-hash binding can mark them **stale** after later edits.

**Provisional runs** (mechanical hooks) can be **confirmed** by `run_start` with matching `sessionId` rather than duplicated.

---

## Project discovery and desktop workspace

- `project init` is idempotent; creates `.moraine/`, `runs/`, `project.json`.
- `run start` may auto-init project structure; `run show` / `run open` discover only.
- **Discovery desktop (implemented):** projects → runs → structured ledger (timeline, findings, capture coverage).
- Service index (`index.json` under spool) is a **rebuildable nonauthoritative cache**. Canonical data remains project-local bundles.
- Capture continues with the desktop closed; offline desktop can use direct filesystem inspection.
- Service must not require the Moraine source checkout as CWD for installed use.

---

## Checkpoints and evidence

Checkpoint input (summary, actions, rationales, evidence, risks, open questions) is validated for injection safety and size. Agent evidence cannot claim `moraine_captured`. Moraine may attach Git context mechanically.

**Evidence capture (minimal, implemented):** structured evidence items on checkpoints; mechanical hook/spool path for lifecycle events; provenance is explicit. Not a full host observability or signing stack.

---

## Append-only correction and redaction

| Op | Role |
|----|------|
| `human_observation_add` | Append human observation |
| amend / supersede | Correct earlier claims without silent rewrite |
| redact | Target-scoped withholding in **ordinary** projections (C1) |

Ordinary list/show/timeline/MCP views must not leak redacted claim text. Privileged recovery is separate. See [REDACTION.md](./REDACTION.md).

---

## Findings

Findings are **implemented**: create, list, get, respond, state changes; target checkpoints or other structured targets. Ordinary views respect redaction. Prefer findings + observations over legacy run-level decisions.

---

## Persistence and recovery

- Structured state in sidecar; Markdown is projection.
- Mutations after start require `--expected-hash` (UTF-8 SHA-256).
- Two-phase commit (incomplete_op → Markdown write → promote) with explicit recovery codes.
- Start idempotency reserves `run_id` under the project lock.

---

## Idempotency and errors

Mutating ops require `--idempotency-key`. Same key + same payload → original success; conflict on payload change.

JSON errors use stable codes (`revision_conflict`, `idempotency_conflict`, `operation_recovery_required`, `unsupported_schema_version`, …). With `--json`, stdout is only the JSON object.

---

## Capture coverage (honesty)

| Path | What it proves |
|------|----------------|
| Hooks only | Mechanical session/provisional capture; may lack semantic checkpoints |
| MCP tools | Semantic ledger ops when the model calls them |
| Service down | Hook adapter spools; process once on restart |

Do **not** claim full semantic capture when only mechanical hooks ran. See [integrations/codex/EXPECTED_CAPTURE.md](./integrations/codex/EXPECTED_CAPTURE.md).

---

## Managed-region presentation

Structured protocol presentation in the desktop (Protocol ledger panel) is the primary human surface for protocol runs. Free-form edit of managed agent narrative is not the supported durable path. Legacy free-form document mode remains for **non-protocol** Markdown only and should be labeled as historical/compatibility when documented.

---

## Decisions (legacy)

Run-level `approved` / `changes_requested` / `rejected` may exist in older sidecars. They are **compatibility-only**: preserved, loadable, not the product center, not MCP, not the preferred desktop workflow. Prefer comments, observations, and findings.

---

## Honest limitations

- Not authenticated identity, remote MCP, or compliance-grade audit.
- Markdown + sidecar are not a single ACID transaction; recovery is explicit.
- Model may skip MCP tools; coverage stays honest.
- Live multiplayer/relay is secondary and unsafe on untrusted networks.
- Windows / macOS install are out of scope for C2.
- Evidence capture is minimal, not full host telemetry.

---

## Future work (not C2 claims)

- Broader agent adapters beyond Codex  
- Richer evidence kinds and signing  
- Platform install (W1+)  
- Beta surface freeze and fault matrix (C3)  
- Hosted collaboration is **not** planned for beta  

---

## Related surfaces labeled legacy

| Surface | Status |
|---------|--------|
| `moraine share` / `moraine-server` live rooms | Secondary / experimental collab—not the primary install story |
| Free-form Human notes + host Save | Historical/compatibility for non-protocol docs |
| `moraine decide` | Legacy compatibility only |
| Editor-oriented “Review + Save” examples | Superseded by agent-run ledger + discovery workspace |
