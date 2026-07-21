# Moraine MCP (local STDIO)

Local-only Model Context Protocol transport for the **agent-run protocol**.

## What it is

```text
moraine-core  →  same operations as `moraine run …`
moraine-mcp   →  STDIO MCP server (official `rmcp` 2.2 SDK)
moraine mcp   →  CLI entry: `moraine mcp --project PATH`
```

Coding agents that speak MCP can start runs, checkpoint, and mark ready **without**
shelling out to Moraine, inventing filenames, or reading full Markdown.

## What it is not

- Not remote MCP / HTTP / OAuth
- Not human approval or workflow authorization (`approved` / `changes_requested` / `rejected` are never MCP tools)
- Not shell execution or arbitrary file I/O
- Not an orchestrator or full trace platform
- Not automatic command capture

## Start server

```bash
moraine mcp --project /absolute/path/to/project
```

- Protocol frames: **stdout** only  
- Diagnostics: **stderr** only  
- Project root is **resolved once** and fixed for the process lifetime  
- Tool arguments cannot switch projects  

If `--project` is omitted, discovery uses the current directory (and may create
minimal `.moraine` structure).

## Tools

| Tool | Role |
|------|------|
| `run_start` | Start run (auto project init if needed) |
| `run_show` | Read-only compact status (no Markdown body) |
| `run_checkpoint` | Structured checkpoint |
| `run_ready` | Active → ready_for_review (not approval) |
| `run_resume` | Ready → active |
| `list_findings` | List open findings (and target context) for a run |
| `get_finding` | Full finding thread + original target checkpoint snapshot |
| `respond_to_finding` | Agent response (requires `idempotencyKey`; not an approval) |

Human create and state-change for findings go through the desktop/host path, not MCP.

## Server instructions (size budget)

Advertised at MCP initialize. First 512 characters state the lifecycle
(`run_start` → checkpoints → `run_ready`, reuse hashes, no human approval).

Soft budget: **1800 bytes** total (`SERVER_INSTRUCTIONS_MAX_BYTES`).

## Token / size budgets (byte proxies)

| Artifact | Budget |
|----------|--------|
| Server instructions | ≤ 1800 B |
| Complete tools/list | ≤ 12 KiB |
| Typical tool success body | ~2 KiB target |
| Individual tool description | ~400 characters |

These are **byte-size proxies**, not provider token counts.

## Errors

Domain failures return MCP tool results with `isError: true` and a JSON body:

```json
{
  "ok": false,
  "error": {
    "code": "revision_conflict",
    "message": "…",
    "details": { }
  }
}
```

Codes match the agent-run protocol (`project_not_found`, `run_not_found`,
`invalid_checkpoint`, `revision_conflict`, `idempotency_conflict`,
`run_state_conflict`, `run_record_structure_invalid`,
`operation_recovery_required`, `unsupported_schema_version`,
`idempotency_index_full`, `finding_not_found`, `invalid_finding`, …).

Normal domain errors do **not** terminate the server.

## SDK

- Official crate: **`rmcp` 2.2.x** (`server`, `macros`, `transport-io`, `schemars`)
- Locked via workspace `Cargo.lock`
- Requires workspace **MSRV 1.88** (`rmcp` edition 2024 / `darling`)

## Manual checks

```bash
# MCP Inspector (if installed)
npx @modelcontextprotocol/inspector moraine mcp --project /abs/path

# Or line-oriented JSON-RPC on stdio (newline-delimited)
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | moraine mcp --project .
```

See also [integrations/CODEX.md](./integrations/CODEX.md).
