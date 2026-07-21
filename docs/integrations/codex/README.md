# Codex reference pack (C2)

Codex is the first concrete agent integration. Moraine remains agent-neutral.

## What gets configured

Project-local only:

- `.codex/config.toml` — managed `[mcp_servers.moraine]` block
- `.codex/hooks.json` — managed SessionStart / UserPromptSubmit / Stop handlers

Commands use the **absolute installed** `moraine` CLI:

```bash
moraine project init .
moraine integrate codex --project .
# aliases: moraine setup codex --project .
moraine doctor --project . --integration codex
```

## Capture model

| Path | What it records |
|------|-----------------|
| Hooks | Session start/stop, user prompts (provisional runs), mechanical tool events |
| MCP | Semantic run protocol tools when the model calls them |
| Service down | Hook events spool under `~/.cache/moraine-service` and process on restart |

Do **not** assume every model call always uses MCP tools. Capture coverage is reported honestly.

## Privacy

- No secrets in managed config
- Project-local config may be committed or gitignored per team policy
- Ledgers stay under `.moraine/`
- Uninstall does not remove Codex config; use `--remove`

## Removal

```bash
moraine integrate codex --project . --remove
```

Removes only Moraine-managed markers; unrelated MCP servers and hooks remain.

## Smoke

1. Install suite (`docs/INSTALL.md`)
2. `moraine setup` && `moraine service start`
3. Init project + integrate Codex
4. Run an ordinary Codex task (desktop may stay closed)
5. Open installed desktop later to inspect the run

See also: [../CODEX.md](../CODEX.md), [../../TROUBLESHOOTING.md](../../TROUBLESHOOTING.md).
