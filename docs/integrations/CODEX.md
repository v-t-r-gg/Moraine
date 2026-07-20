# Codex + Moraine (local STDIO MCP)

One-time, **project-scoped** configuration so Codex can use Moraine tools without
per-task Moraine prompt text.

## Requirements

- Moraine CLI on `PATH` (or absolute `command`)
- Codex CLI that supports local STDIO MCP (`codex mcp …`)
- A project directory for `--project`

Verified against **Codex CLI 0.144.x** local help and common config patterns.
Re-check keys if your Codex build differs.

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

There is **no** human decision or approval tool. Moraine records work; it does
not authorize merge or deployment. Prefer desktop comments and human notes for
review context (`moraine decide` is legacy/compatibility-only).

If your Codex build supports `enabled_tools` / tool allowlists, pin the list to
those five. If not, rely on the server tool list (still only five tools).

## Expected agent behavior

From **MCP server instructions** alone, Codex should:

1. Call `run_start` before substantive work  
2. Record a small number of meaningful checkpoints  
3. Call `run_ready` after validation  
4. Reuse `runId` + `contentHash`  
5. Never imply human approval or merge authority  

If a real session ignores server instructions, record that as a dogfood finding.
Do not paper over it with large per-task prompts in this milestone.

## Manual install note

This milestone does **not** auto-edit `~/.codex/config.toml` or project
`.codex/config.toml`. Configure once by hand or via `codex mcp add`.

## Security

- STDIO only; no network listener from Moraine MCP  
- Project fixed at server start  
- No shell tool, no arbitrary file write tool  
