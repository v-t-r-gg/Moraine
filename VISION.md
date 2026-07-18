# Vision

Moraine is a **local-first Markdown collab tool** where **agents document their own work** and **humans review that record**.

## Positioning

While coding or operating systems, agents write plain Markdown: what changed, why, decisions, outcomes. That becomes a durable audit trail next to the code (or any folder). Humans review it **live** or **later**, with comments, suggestions, and accept/reject, without forcing a cloud SaaS doc product into the loop.

| Role | Primary surface | Job |
|------|-----------------|-----|
| Agent / script | `moraine` CLI + writing `.md` | Log work as Markdown; share rooms; `status` for review counts |
| Human | Desktop / web UI | Read history, comment, accept/reject suggestions, Save |

**One file = one room.** The artifact is always a real `.md` file (plus optional `file.md.comments.json`). No mandatory cloud.

## Why this split

- Agents already work in terminals and tools. The CLI (`cat`/`write`, `share`, `status --json`, exit codes) is the agent API.
- Humans need presence, Review UI, and judgment. The GUI is for oversight, not for blocking agents from writing.
- Persistent files mean hindsight works: open the same path tomorrow, rehydrate marks from the sidecar, audit what was proposed and accepted.

## Design rules

1. **Agent logging is a first-class use case.** Docs written during a task should survive cold open and git.
2. **Human review is first-class.** Comments and suggestions exist so oversight is structured, not only "read the whole file."
3. **CLI is not secondary.** Scripts must not need a browser to contribute or inspect status.
4. **Real-time is optional.** Live Yjs rooms help when a human joins mid-task; offline file + sidecar still matter for later audit.
5. **Keep scope narrow.** One file per room; multi-file = multi process. Prefer polish on review and CLI over workspace UI.

## Success

- An agent finishes a task and leaves Markdown + review state a human can open hours later and still understand.
- A human can join a live room *or* open the file alone and Accept/Reject leftover suggestions.
- Automation can `status` a path without attaching to Yjs awareness.

## Out of scope (for now)

In-app multi-file workspace, auth product, MCP as the only agent path. CLI remains the agent path; GUI remains the review path.
