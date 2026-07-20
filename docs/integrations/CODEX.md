# Codex + Moraine (local STDIO MCP + hooks)

One-time, **project-scoped** configuration so Codex can use Moraine tools without
per-task Moraine prompt text, and so deterministic session capture works with the
desktop closed.

## Requirements

- Moraine CLI on `PATH` (or absolute `command`)
- `moraine-service` running as a per-user background process (Linux: systemd `--user`)
- Codex CLI that supports local STDIO MCP (`codex mcp …`) and lifecycle hooks
- A project directory for `--project`

Verified against **Codex CLI 0.144.x** local help and common config patterns.
Re-check keys if your Codex build differs.

## Local service (required for desktop-closed capture)

```bash
cargo install --path crates/moraine-service
moraine-service install   # writes systemd --user unit
moraine-service start
```

Default Unix socket: `$XDG_RUNTIME_DIR/moraine-service.sock`  
Override with `--unix-socket` / `MORAINE_SOCKET`.

## Recommended project config

Create **project-local** `.codex/config.toml` (do not commit secrets):

```toml
[mcp_servers.moraine]
command = "moraine"
args = ["mcp", "--project", "/absolute/path/to/project"]
# Optional timeouts (when supported by your Codex build):
# startup_timeout_sec = 10
# tool_timeout_sec = 60
```

Use an **absolute** project path so the MCP child does not depend on cwd quirks.

## Deterministic hooks

Add project-local `.codex/hooks.json` (trust via `/hooks` in Codex):

```json
{
  "description": "Moraine session capture (desktop may remain closed).",
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          {
            "type": "command",
            "command": "moraine hook-codex",
            "statusMessage": "Moraine session observe"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "moraine hook-codex",
            "statusMessage": "Moraine provisional run"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "moraine hook-codex"
          }
        ]
      }
    ]
  }
}
```

`moraine hook-codex` reads Codex hook JSON from stdin and:

1. Maps `SessionStart` → session envelope  
2. Maps `UserPromptSubmit` → provisional run (bounded prompt text only)  
3. Maps `Stop` → session end  
4. Delivers to the local service socket; on failure, writes the event to the spool and exits 0  

When the agent later calls MCP `run_start` with the same `sessionId`, Moraine
confirms the provisional run instead of creating a duplicate.

Privacy: Moraine does **not** store full transcripts. Only a bounded initial-task
string is retained for provisional objectives.

## CLI registration

```bash
codex mcp add moraine -- moraine mcp --project /absolute/path/to/project
codex mcp list
```

`codex mcp list` should show `moraine` and that it initializes.

## Enabled tools

Moraine MCP exposes only:

```text
run_start
run_show
run_checkpoint
run_ready
run_resume
```

`run_start` accepts optional `sessionId` to reconcile with a provisional run.

There is **no** human decision or approval tool. Moraine records work; it does
not authorize merge or deployment. Prefer desktop comments and human notes for
review context (`moraine decide` is legacy/compatibility-only).

If your Codex build supports `enabled_tools` / tool allowlists, pin the list to
those five. If not, rely on the server tool list (still only five tools).

## Expected agent behavior

From **MCP server instructions** alone, Codex should:

1. Call `run_start` before substantive work (pass `sessionId` when known)  
2. Record a small number of meaningful checkpoints  
3. Call `run_ready` after validation  
4. Reuse `runId` + `contentHash`  
5. Never imply human approval or merge authority  

Hooks capture the session envelope even if the model skips MCP. Capture coverage
is reported honestly on the run record.

If a real session ignores server instructions, record that as a dogfood finding.
Do not paper over it with large per-task prompts.

## Manual install note

This milestone does **not** auto-edit `~/.codex/config.toml` or project
`.codex/config.toml`. Configure once by hand or via `codex mcp add`. Trust new
hooks with Codex `/hooks`.

## Security

- STDIO only; no network listener from Moraine MCP  
- Hooks talk to a local Unix socket protected by user filesystem permissions  
- Project fixed at MCP server start  
- No shell tool, no arbitrary file write tool  
- Bounded hook payloads; no full-transcript ingestion by default  
