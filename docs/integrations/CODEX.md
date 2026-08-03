# Codex integration

Codex is Moraine’s supported reference agent on Linux. Windows Codex capture is
not supported until the native Windows runtime exists.

## Set up

Initialize the project, then use the canonical integration command:

```bash
moraine project init /absolute/path/to/project
moraine integrate codex --project /absolute/path/to/project
```

`moraine setup codex` remains a compatibility alias.

## Files modified

Moraine manages bounded blocks in:

* `.codex/config.toml`; local STDIO MCP registration;
* `.codex/hooks.json`; mechanical hook handlers.

Unrelated user configuration is preserved. Existing managed files are backed up
by transactional provisioning & restored byte-for-byte on rollback.

Project ledger files live under `.moraine/`. Decide with your team whether to
commit them. Moraine’s own development records are untracked by default.

## Hooks & MCP

Hooks send mechanical lifecycle signals such as session start, prompt & tool
activity. They prove activity coverage; they do not contain the agent’s full
semantic account.

The local MCP lets Codex start or resume a run, record checkpoints & evidence,
manage findings & append corrections. The process is confined to one project
root. Discover the live tool inventory through MCP `tools/list`.

Healthy ProductCapture needs both paths. A run with hooks but no semantic
confirmation should not be presented as complete evidence.

## Capture fidelity

Codex capability profile (this slice):

| Dimension | Capability |
|---|---|
| Session lifecycle | supported |
| Prompt activity | supported |
| Tool activity | supported (depends on received Pre/Post tool hook events) |
| Semantic protocol | supported |

Tool activity appears as `observed` only when Moraine received durable tool or
command mechanical events (or mechanical evidence) for the run. Missing tool
events report `not_observed`, not adapter failure.

Legacy coverage `full` means mechanical + semantic channels were both observed —
not a complete transcript of agent work.

```bash
moraine run coverage <RUN_ID> --project /absolute/path/to/project --json
```

## Check & repair

```bash
moraine doctor --project /absolute/path/to/project --integration codex
moraine service status --json
```

Desktop health offers repair only when the platform supports ProductCapture.
CLI reconfiguration is idempotent:

```bash
moraine integrate codex --project /absolute/path/to/project
```

## Remove

```bash
moraine integrate codex --project /absolute/path/to/project --remove
```

Removal strips only Moraine-managed blocks. Unmanaged entries with the same MCP
name are left for manual review.

## Privacy & Git

Moraine sends no project data to a Moraine cloud service. Codex itself has its
own data policy; review it separately.

Potentially sensitive locations include:

* `.moraine/runs/`;
* `.moraine/evidence/`;
* `.codex/config.toml`;
* `.codex/hooks.json`;
* user spool & transaction journals.

Redaction changes ordinary Moraine projections; it does not erase Git history,
backups, raw sidecars or separate evidence files. See
[../../SECURITY.md](../../SECURITY.md).

## Troubleshooting

If no run appears:

1. Run `moraine doctor --project ... --integration codex`.
2. Confirm `moraine service status --json` reports registration, diagnostics &
   capture ready.
3. Re-run the integration command.
4. Start a new Codex session in the configured project.
5. Check `moraine service logs`.

If hooks spool while the service is down, starting the service should process
valid queued events. Unsupported endpoints do not create unprocessable spool
items.

## Live verification

Use the product self-test when diagnosing setup:

```bash
moraine self-test --project /absolute/path/to/project --json
```

Product verification uses the real CLI, hook payload, service intake, session
binding, run materialization & discovery. Direct development verification is
separate & cannot report product `Ready`.

Codex configuration formats may change between Codex releases. Moraine tests
the managed format it supports; use `doctor` after upgrading Codex.
