# Vision

## Product invariant

> Moraine records review activity; it does not render the verdict.

Moraine is a **local-first ledger for autonomous agent work**. Near-term product focus:

> **Moraine is a local-first, cross-agent ledger for coding-agent work.**

It preserves what an agent did, why it did it, what evidence exists, what risks or unresolved questions remain, and what human context accumulated around the work. It does **not** decide whether work is accepted, rejected, mergeable, deployable, or authorized. Those decisions remain in the coding-agent session, pull request, issue tracker, CI, or other workflow that already owns them.

## The problem

Autonomous agents perform real work: code changes, investigations, migrations, ops fixes. That work often leaves behind chat transcripts, tool logs, or nothing durable. Humans need a **reviewable record** they can open later, put next to the code, comment on, and challenge—without relying on a vendor session viewer.

Concise positioning: **Review agent work without relying on agent chat.**

## What Moraine is

Agents create durable, human-readable **run records** (Markdown plus structured sidecar metadata). Humans inspect those records: read the narrative, leave comments, challenge evidence, add notes, and preserve review discussion.

Collaborative live editing exists as a supporting capability. It is not the product headline. Moraine is **not** an approval system.

## Agent run

An **agent run** is a bounded unit of work (one feature, one investigation, one migration attempt). During or after that run, the agent writes a **run record**:

* objective and context
* actions taken
* implementation rationale
* outcome
* verification or checks performed
* risks
* unresolved questions
* evidence references (with clear provenance)

The run record is a plain `.md` file on disk. Agents use the **CLI**, **local MCP**, or ordinary filesystem tools. The CLI and MCP are first-class for automation (`--json` where supported, stable exit codes, compact tool results).

An agent's narrative is a **claim about its work**, not independent proof. Prefer explicit verification notes and pointers to evidence. Automatic evidence capture is emerging; until then, referencing evidence in Markdown is the baseline pattern.

## Human review (without verdict)

Humans primarily use the **GUI** (desktop Tauri app or web UI in development):

* launch a **ledger workspace** and discover projects/runs without knowing paths
* open a run and inspect the structured chronological timeline
* comment on selections; suggest replacements (accept applies text; reject drops the mark)
* add **append-only human observations** (and other core-backed append-only ops) on protocol runs
* use **Legacy document mode** only for temporary free-form Markdown editing on non-protocol documents

Review means inspection, comment, challenge, context, and response. It does **not** need to end in an approval state. Protocol run claims are not free-form-edited in place.

Review may happen:

* **Live:** while a share room is open and the optional relay is running
* **Hindsight:** after the agent session ended, by reopening the durable files

Use "human review" as the default term. "Audit" means a careful human look at the record, not a compliance-grade immutable trail.

## Durable artifacts

| Artifact | Role |
|----------|------|
| `*.md` | Human-readable run narrative |
| `*.md.moraine.json` | Run ledger: stable run id, structured agent state, annotations; historical decisions preserved for compatibility |
| `*.md.comments.json` | Legacy annotations only; migrated into `.moraine.json` on open |
| Local history store | Optional local edit snapshots under the Moraine data directory (not Git) |

Files sit next to the work they describe and can be versioned with Git **by the user**. Moraine does not automatically commit, branch, or open pull requests.

## Evidence and trust

* Agent text can be wrong, incomplete, or optimistic.
* Supporting evidence should be linked or attached when available, with provenance (`agent_reported` vs captured vs external vs human).
* Humans can leave comments and accept/reject **text suggestions**. That is not run-level authorization.
* Moraine does **not** provide authenticated reviewer identity, cryptographic integrity, or a tamper-proof audit log.
* Historical run-level decisions (`approved` / `changes_requested` / `rejected`) may still exist in older sidecars; they are **compatibility data**, not the product center. Prefer comments, findings, and human notes.

## Current scope

Implemented today (high level):

* CLI for agent run protocol, file I/O, local history helpers, share/join URLs, status
* Local STDIO MCP transport (`moraine mcp`) over the same core operations
* Desktop/web editor over one Markdown file
* Optional live multiplayer via an in-memory WebSocket relay
* Host save policy when remote peers are present
* Comments and suggestions with Yjs session state and host ledger persistence
* Mark rehydration from quote text on cold open (best effort)

## Current limitations

* Early-stage, not production-ready as a hosted collaboration service
* No authentication; the relay is local/in-memory with no durable server-side state
* No secure multi-tenant remote collaboration
* Limited automatic evidence capture
* No compliance-grade or immutable review ledger
* No built-in Git automation
* Live peer count is not available via CLI `status` (UI/Yjs only)
* Quote-based mark rehydration fails if the text moved or changed
* Concurrent external editors of the same file are not fully guarded
* One Markdown file maps to one live collab room in the current model

## Design principles

1. The primary conceptual object is an **agent run**, not a generic document.
2. Moraine is a **ledger**, not a workflow gate.
3. The **CLI** and **MCP** are first-class for agents and automation.
4. The **GUI** is primarily a **human inspection** interface.
5. **Hindsight** and durable files matter more than real-time editing.
6. Agent narrative is useful and not automatically trustworthy.
7. Run records should **reference** supporting evidence (capture automation is near-term work).
8. Plain files and portable formats are deliberate.
9. Tool independence: no single agent vendor owns the record format.
10. Docs must separate **current capability** from **direction**.

## Longer-term direction

See [docs/DEVELOPMENT_BLUEPRINT.md](./docs/DEVELOPMENT_BLUEPRINT.md). Near-term sequence:

* minimal trustworthy evidence capture
* human findings and agent amendments
* local run discovery and ledger-focused UX
* second agent integration and external beta

Explicit non-goals for the MVP: approval/rejection as product center, merge gates, full observability, hosted multi-tenant collab, compliance-grade audit, cryptographic signing.
