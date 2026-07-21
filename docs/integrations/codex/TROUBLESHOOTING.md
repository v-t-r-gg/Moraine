# Codex pack troubleshooting

## Stale `moraine` path in config

```bash
type -a moraine
moraine doctor --project . --integration codex
moraine integrate codex --project .   # rewrites managed block to current absolute CLI
```

## Hooks not firing

- Confirm `.codex/hooks.json` contains `hook-codex` and managed markers.
- Confirm Codex build supports project hooks (version via `codex --version`).
- Confirm service: `moraine service status` / `moraine doctor`.

## Service offline / spool growing

```bash
moraine service start
moraine service logs
ls ~/.cache/moraine-service/spool
```

## Malformed hooks.json

Moraine refuses to overwrite invalid JSON. Fix or remove the file, then re-run integrate.

## MCP tools/list fail in doctor

```bash
moraine mcp --project /absolute/project   # should block on STDIO; Ctrl-C
moraine doctor --project . --integration codex --json
```

## Duplicate handlers

Re-run `moraine integrate codex --project .` (idempotent managed refresh) or `--remove` then integrate again.

## Global vs project config

C2 manages **project-local** `.codex` only. Global `~/.codex` is not modified by Moraine integrate.
