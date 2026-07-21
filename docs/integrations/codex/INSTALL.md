# Codex integration install

Prerequisites: installed Moraine suite (`docs/INSTALL.md`), Codex CLI on `PATH`.

```bash
export PATH="$HOME/.local/bin:$PATH"   # before ~/.cargo/bin
moraine setup
moraine service start

cd /absolute/path/to/your/repo
moraine project init .
moraine integrate codex --project .
# or: moraine setup codex --project .
moraine doctor --project . --integration codex
```

## What is written

| File | Content |
|------|---------|
| `<project>/.codex/config.toml` | Managed `[mcp_servers.moraine]` block (absolute `moraine mcp --project …`) |
| `<project>/.codex/hooks.json` | Managed SessionStart / UserPromptSubmit / Stop → `moraine hook-codex` |

Unrelated MCP servers and hook handlers are preserved. Existing files are backed up before change.

## Check / remove

```bash
moraine integrate codex --project . --check
moraine integrate codex --project . --dry-run
moraine integrate codex --project . --remove
```

`--remove` deletes only Moraine-managed markers, not hand-written Moraine-like entries without markers.

## Smoke

1. Keep desktop closed.
2. Run an ordinary Codex task in the project (no custom Moraine prompt required).
3. Confirm `.moraine/runs/` gains a provisional or confirmed run.
4. Open installed desktop later: `moraine open` or menu entry.
5. `moraine doctor --project . --integration codex` remains healthy.

See [EXPECTED_CAPTURE.md](./EXPECTED_CAPTURE.md) and [TROUBLESHOOTING.md](../../TROUBLESHOOTING.md).
