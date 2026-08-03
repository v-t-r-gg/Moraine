# Claude Code integration

Claude Code is a second production agent adapter for Moraine. It uses the same
project-local ledger, capture service & ProductCapture path as Codex.

W2-E Windows interactive acceptance remains pending & independent of this
adapter. This document does not claim Windows Product Ready.

## Set up

```bash
moraine project init /absolute/path/to/project
moraine integrate claude-code --project /absolute/path/to/project
# or full product path:
moraine enable --agent claude-code --project /absolute/path/to/project
```

## Files managed

Moraine owns only project-scoped files:

* `.mcp.json` — managed `mcpServers.moraine` stdio entry;
* `.claude/settings.json` — managed lifecycle hooks.

Moraine does **not** modify:

* `~/.claude.json`
* `~/.claude/settings.json`
* `.claude/settings.local.json`
* `CLAUDE.md`

## MCP

The managed server looks like:

```json
{
  "mcpServers": {
    "moraine": {
      "type": "stdio",
      "command": "/absolute/path/to/moraine",
      "args": ["mcp", "--project", "/absolute/path/to/project"],
      "env": {},
      "moraineManaged": true
    }
  }
}
```

Unrelated MCP servers are preserved. An unmanaged server already named
`moraine` is a conflict and is not overwritten. Claude Code may still ask the
user to approve project-scoped MCP servers; that is expected.

## Hooks

Lifecycle events used:

* `SessionStart` (matcher `startup|resume` when applicable)
* `UserPromptSubmit`
* `Stop`

Each managed handler runs:

```text
/absolute/path/to/moraine hook-claude-code
```

Handlers are observational. They exit zero after spooling when capture is
temporarily unavailable so Claude Code is not disrupted.

## Capture fidelity

Claude Code capability profile (this slice):

| Dimension | Capability |
|---|---|
| Session lifecycle | supported (`SessionStart`, `Stop`) |
| Prompt activity | supported (`UserPromptSubmit`) |
| Tool activity | **not_supported** (tool-call hooks are not captured yet) |
| Semantic protocol | supported (MCP run/checkpoint operations) |

A Claude run may still report legacy coverage `full` when lifecycle + semantic
start both land. Tool activity remains `not_supported` rather than failure.

Inspect:

```bash
moraine run coverage <RUN_ID> --project /absolute/path/to/project
```

## Capture & privacy

Hook payloads may include `session_id`, `cwd`, `prompt`, `transcript_path`,
`last_assistant_message` and related fields.

Moraine maps only existing mechanical event kinds. By default:

* full prompt text is **not** stored;
* full assistant messages are **not** stored;
* `transcript_path` is never read;
* session IDs are stored as `claude-code:<session_id>` so they cannot collide
  with Codex sessions.

Self-test markers of the form `Moraine self-test verification_id=…` may be
retained as a bounded `objectiveHint` for ProductCapture verification only.

## Check & repair

```bash
moraine doctor --project /path --integration claude-code
moraine self-test --agent claude-code --project /path
moraine integrate claude-code --project /path --check
```

Health & desktop repair use the shared provisioning planner. Unmanaged name
conflicts require manual resolution.

## Remove

```bash
moraine integrate claude-code --project /path --remove
```

Removes only Moraine-managed MCP & hook entries. Project `.moraine` ledgers
remain.

## Detection

```text
MORAINE_CLAUDE_CODE override
→ PATH (`claude` / `claude.exe`)
→ common user-local locations on Linux
→ claude --version (bounded timeout)
```

Statuses: `readyToConnect`, `notFound`, `unusable`.

## Limitations

* Compatibility is proven against controlled fixtures and, optionally, a real
  local Claude Code install via `scripts/claude-code-product-capture-smoke.sh`.
* Hook schema & MCP approval UX are defined by Claude Code; Moraine does not
  bypass approval.
* Tool-call-level capture, blocking hooks, plugins & Anthropic API auth are out
  of scope.
