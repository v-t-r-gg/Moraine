# Vision

Moraine is a **local-first Markdown collab tool** for teams where **agents do a lot of the writing** and **humans do the review**.

## Positioning

| Role | Primary surface | What they do |
|------|-----------------|--------------|
| Agent / script | `moraine` CLI (`--json`, exit codes) | Read/write files, open share rooms, inspect status and review counts |
| Human | Desktop (Tauri) or web UI | Live edit, comments, accept/reject suggestions, Save to disk |

Plain `.md` on disk stays the durable artifact. No mandatory cloud. Optional self-hosted Yjs relay for multiplayer.

## Design rules that follow from this

1. **CLI is first-class**, not a thin wrapper around the GUI. Agents should not need a browser to contribute.
2. **GUI is for judgment**: presence, comments, suggestions, host Save under collab.
3. **One file = one room.** Keep the product focused; multi-file can be multiple processes.
4. **Scriptable I/O**: stable JSON shapes, exit codes (`0/1/2/3`), short error messages.
5. **Review data next to the file**: `file.md.comments.json` so comments/suggestions survive cold open and fit git workflows.

## What success looks like

- An agent can `share` a doc, edit via CLI or file tools, and leave suggestions/comments for a human.
- A human opens the same room (or the file as host), reviews Accept/Reject, and Saves.
- Scripts can poll `status` without attaching to Yjs awareness.

## Out of scope (for now)

In-app multi-file workspace, auth/TLS product, full MCP server as the only agent path. The CLI is the agent path today.
