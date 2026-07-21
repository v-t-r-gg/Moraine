# Expected capture (Codex + Moraine)

## Paths

| Mechanism | When | What appears in the ledger |
|-----------|------|----------------------------|
| **Hooks** | SessionStart, UserPromptSubmit, Stop (and tool events when emitted) | Session envelope; provisional run on first substantive prompt; mechanical events |
| **MCP tools** | When the model calls Moraine tools | Semantic `run_start` / checkpoint / ready / findings mutations |
| **Spool** | Service down | Hook adapter exits 0; event files under `~/.cache/moraine-service/spool`; processed once after restart |

## Coverage honesty

- **Mechanical-only** (hooks fired, no MCP calls): provisional run and session may exist; coverage must **not** claim full semantic capture.
- **Provisional → confirmed**: `run_start(sessionId)` should reconcile, not duplicate, when MCP is used.
- **Model non-use of MCP** is a normal outcome. Record it as a finding; do not hide it with a custom prompt.

## Actual tool list

Do not rely on stale docs. From the installed suite:

```bash
# doctor probes tools/list when --integration codex and project is initialized
moraine doctor --project . --integration codex --json
```

Current implementation tools (verify live): `run_start`, `run_show`, `run_checkpoint`, `run_ready`, `run_resume`, `list_findings`, `get_finding`, `respond_to_finding`.

## Service-down fallback

1. `moraine service stop`
2. Trigger a hook (or synthetic `hook-codex` stdin JSON)
3. Confirm spool file created; adapter exit 0
4. `moraine service start`
5. Confirm event processed once; doctor clean for suite/service

## Desktop inspection

After capture (desktop may stay closed during the task):

```bash
moraine open
# or
moraine open --path /path/to/run.md
```

Project-local bundles under `.moraine/runs/` remain authoritative.
