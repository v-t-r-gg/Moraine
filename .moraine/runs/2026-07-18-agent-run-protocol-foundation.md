# Agent run protocol foundation

## Objective

Implement a safe, compact agent-run protocol (core + JSON CLI) so agents can
start, checkpoint, resume, and open runs without manually managing paths or
rewriting full Markdown records.

## Scope

- Project init/discovery under `.moraine`
- `run start|show|checkpoint|ready|resume|open` with `--json`
- Structured checkpoints, lifecycle, revision hash, idempotency, recovery
- Durable Markdown projection with preserved human notes
- Tests, docs, dogfood evidence

## Out of scope

MCP, exec/shell interception, CI providers, local-model summarization,
transcript ingestion, auth/signing, remote services, central DB, review inbox,
annotation-anchor redesign, history #4, broad GUI redesign.

## Starting branch and base

- Branch: `feat/agent-run-protocol-foundation`
- Base: `caf53be` (`origin/main`)

## Major design decisions

1. **Sidecar holds structured agent state** (`RunMeta.agent`, schema **v4**);
   Markdown is a deterministic projection plus a free-form human region.
2. **Managed Markdown uses ATX headings/lists** (not HTML comments/frontmatter)
   so Tiptap `html: false` can round-trip structure; `## Human notes` body is
   preserved exactly.
3. **Two-phase incomplete op** on the sidecar around Markdown atomic replace for
   recovery without pretending MD+sidecar is ACID.
4. **Project index** (`.moraine/project.json`) tracks start idempotency keys;
   per-run completed ops live on the run sidecar.
5. **Git context** captured mechanically; agents supply objective/checkpoint
   semantics only.
6. **Agent cannot record human decisions**; `ready_for_review` ≠ approved.

## Implementation checkpoints

1. Repository inspection — done (`caf53be`, branch created).
2. Persistence design — done (above).
3. Core implementation — `agent_protocol` module + schema v4.
4. CLI implementation — `project` / `run` subcommands, JSON envelope.
5. Concurrency/recovery testing — core + CLI integration tests.
6. Complete verification — `./scripts/check.sh` + temp-Git e2e.
7. Review candidate — PR opened; no human Moraine approval recorded by agent.

## Tests and evidence

```text
cargo test -p moraine-core
cargo test -p moraine-cli
./scripts/check.sh   # EXIT:0 expected before PR

Core: project init, start/checkpoint/ready/resume, concurrent revision
conflict, many-checkpoint show bound, human notes preservation,
idempotency conflicts.

CLI: project init repeat, full lifecycle, auto-init, open by run id.

E2E (temp Git repo): project init → start → checkpoint → human edit →
show → checkpoint → ready → decide → resume (stale) → open.
Default show ~1.2 KiB; start/checkpoint/ready compact.
```

## Risks

- Desktop Save may reflow whitespace inside managed sections; structure relies
  on headings remaining intact.
- Incomplete recovery paths need more adversarial soak testing.
- `run open` depends on a local desktop binary being discoverable.

## Unresolved questions

- Should `run start` accept an optional explicit slug override?
- Is project-level start-op index enough long-term vs scan-only?

## Dogfood observations

- Could the work be understood without reopening this agent transcript?
  - Largely yes via this record + `docs/AGENT_RUN_PROTOCOL.md` + tests.
- Did the record contain enough evidence?
  - Yes for core/CLI; desktop open launch not verified with a built app.
- Did comments survive later edits?
  - N/A for annotations; human notes under `## Human notes` survived protocol
    mutations in e2e.
- Did any annotation become ambiguous or orphaned?
  - Not exercised in protocol e2e.
- Did accepting or rejecting suggestions feel predictable?
  - N/A this milestone.
- Did incomplete acceptance require recovery?
  - N/A (annotation acceptance); incomplete agent ops use sidecar phase marks.
- Did watcher or viewport behavior interfere?
  - Not exercised.
- Did history behavior interfere?
  - Not exercised (issue #4 still open).
- How long should human review take?
  - 45–90 minutes for protocol + concurrency tests + docs.
- What felt unnecessarily manual?
  - Writing the dogfood record with the old template once more (this milestone
    is intended to remove that friction for future runs).

## Review candidate

- Branch: `feat/agent-run-protocol-foundation`
- Implementation commits: see git log on branch (core / CLI / docs splits).
- Do not embed this commit’s own SHA here.
- No human approval recorded by the implementing agent.

## Needs human review

- Schema v4 migration path and optional `agent` field.
- Idempotency and concurrency semantics.
- Token/response-size bounds vs completeness of `run show`.
- Desktop open behavior when binary is missing.


## Review response (request changes)

Addressed PR #6 merge blockers and high-priority findings:

1. Two-phase recovery: committed agent vs pending_agent; failed MD write discards pending.
2. Authority model A documented in Markdown projection and docs; managed regions regenerated.
3. Start reservation under project lock (pending → complete).
4. Agent evidence cannot claim moraine_captured.
5. Scalar fields reject newlines/control chars.
6. Lifetime idempotency map (no silent 200-eviction).
7. Bounded risks/openQuestions in run show (total + recent).
8. run open JSON fails with desktop_launch_failed when not launched.
9. Checkpoint input I/O errors use JSON envelope.
10. run show discovers only (no auto-init).
11. record_revision uses checked_add.

Remaining for human: desktop GUI enforcement of managed regions; full tauri package build optional.

## Follow-up correction (post request-changes)

Head before this work: `584d452`.

### Remaining findings addressed

1. **Human notes byte-for-byte** — delimiter located by byte offsets; body is a raw slice after the original line ending (LF/CRLF preserved). Later `## Human notes` lines in body are content, not delimiters. Tests cover LF, CRLF, no final NL, trailing blanks, fence, Unicode.
2. **Idempotency capacity preflight** — new keys fail with `idempotency_index_full` before incomplete intent or Markdown write; existing keys still replay. Regression fills the index to max.
3. **Desktop Model A enforcement** — `managedRegion` ProseMirror plugin blocks ReplaceSteps in managed content; suggestions targeting managed ranges blocked at create/accept; comments allowed. Protocol detection via Markdown markers.
4. **Mutations do not auto-init project** — checkpoint/ready/resume use discover-only; tests assert no `.moraine` creation on failure.
5. **Manual desktop** — `npm run tauri build -- --no-bundle` EXIT 0; release `moraine-app` launched with `MORAINE_OPEN` on a protocol-created run; CLI lifecycle with Human notes survival; full interactive click-through of managed-region typing is limited in this agent session (enforcement covered by unit tests + plugin).

### Commands/results

```
./scripts/check.sh → EXIT 0
npm run tauri build -- --no-bundle → EXIT 0 (built target/release/moraine-app)
cargo test agent_protocol → pass (incl. human notes + capacity + no-init)
npm test → 32 passed (incl. managedRegion)
CLI e2e: start → human notes → checkpoint → ready → decide → resume (stale)
```

### Known limitations

- Full interactive GUI suggestion/comment click-through still benefits from human spot-check.
- Collaborative multi-peer dirty Human notes + external checkpoint conflict path not re-dogfooded beyond existing disk-watch tests.
