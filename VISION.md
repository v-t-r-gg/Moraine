# Vision

## The problem

Autonomous agents now perform real work: code changes, investigations, migrations, deploys, ops fixes. That work often leaves behind chat transcripts, tool logs, or nothing durable at all. Humans need a **reviewable record** of what happened that they can open later, share with peers, put next to the code, and decide on.

Moraine is a **review ledger for autonomous agent work**.

Agents create durable, human-readable **run records** (Markdown plus optional structured sidecar metadata). Humans use Moraine to **review** those records: read the narrative, leave comments, propose text changes as suggestions, and accept or reject suggestions using the current GUI.

Collaborative live editing exists as a supporting capability. It is not the product headline.

## Agent self-documentation

An **agent run** is a bounded unit of work (one task, one investigation, one migration attempt). During or after that run, the agent (or a wrapper script) writes a **run record**:

* objective and context
* actions taken
* decisions and rationale
* outcome
* verification or checks performed
* risks
* unresolved questions
* items that need a human

The run record is a plain `.md` file on disk. Agents and scripts use the **CLI** and ordinary filesystem tools to create and update it. The CLI is first-class for automation (`--json` where supported, stable exit codes).

An agent's narrative is a **claim about its work**, not independent proof. Prefer explicit verification notes and pointers to evidence (command output paths, PR links, log files). Automatic evidence capture is **not** assumed by Moraine today; referencing evidence in Markdown is the current pattern.

## Human review

Humans primarily use the **GUI** (desktop Tauri app or web UI in development):

* open a run record file
* read the narrative
* comment on selections
* suggest replacements (accept applies text; reject drops the mark)
* save the Markdown file and review sidecar when acting as host

Review may happen:

* **Live:** while a share room is open and the optional relay is running
* **Hindsight:** after the agent session ended, by reopening the durable files

Use "human review" as the default term. "Audit" means a careful human look at the record, not a compliance-grade immutable trail.

## Durable artifacts

| Artifact | Role |
|----------|------|
| `*.md` | Human-readable run narrative |
| `*.md.moraine.json` | Run ledger: stable run id, revision-bound decisions, comments, suggestions |
| `*.md.comments.json` | Legacy annotations only; migrated into `.moraine.json` on open |
| Local history store | Optional local edit snapshots under the Moraine data directory (not Git) |

Files sit next to the work they describe and can be versioned with Git **by the user**. Moraine does not automatically commit, branch, or open pull requests.

## Evidence and trust

* Agent text can be wrong, incomplete, or optimistic.
* Supporting evidence should be linked or attached in the narrative when available.
* Humans can leave comments, accept/reject text suggestions, and record run-level decisions (`approved` / `changes_requested` / `rejected`) bound to a content hash. Reviewer labels are user-provided, not authenticated identity.
* When Markdown changes after a decision, that decision remains in history but is reported as **stale** until a new decision is recorded for the current revision.
* Moraine does **not** provide authenticated reviewer identity, cryptographic integrity, or a tamper-proof audit log.

## Current scope

Implemented today (high level):

* CLI for file I/O, local history helpers, share/join URLs, status, and decide
* Desktop/web editor over one Markdown file
* Optional live multiplayer via an in-memory WebSocket relay
* Host save policy when remote peers are present
* Comments and suggestions with Yjs session state and host ledger persistence
* Run-level review decisions bound to SHA-256 content hash of Markdown
* Mark rehydration from quote text on cold open (best effort)

## Current limitations

* Early-stage, not production-ready as a hosted collaboration service
* No authentication; the relay is local/in-memory with no durable server-side state
* No secure multi-tenant remote collaboration
* No automatic evidence capture pipeline
* No compliance-grade or immutable review ledger
* No built-in Git automation
* Live peer count is not available via CLI `status` (UI/Yjs only)
* Quote-based mark rehydration fails if the text moved or changed
* Concurrent external editors of the same file are not fully guarded
* One Markdown file maps to one live collab room in the current model

## Design principles

1. The primary conceptual object is an **agent run**, not a generic document.
2. The **CLI** is first-class for agents, scripts, and automation.
3. The **GUI** is primarily a **human review** interface.
4. **Hindsight** and durable files matter more than real-time editing.
5. Agent narrative is useful and not automatically trustworthy.
6. Run records should be able to **reference** supporting evidence (capture automation is future work).
7. Human decisions should be structured where the product supports it (comments/suggestions today; richer decisions later).
8. Plain files and portable formats are deliberate.
9. One file / one session is acceptable for the current implementation.
10. Docs must separate **current capability** from **direction**.

## Longer-term direction

Possible future work (not current guarantees):

* stronger run identity and review decision records
* better evidence capture and attachment
* Git integrations (still optional)
* authenticated collaboration
* review inboxes across many run records
* optional MCP or other agent protocol adapters

The CLI remains a valid agent interface even if those appear.
