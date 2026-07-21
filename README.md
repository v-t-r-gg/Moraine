# Moraine

**Local-first ledger for coding-agent work.**

Moraine keeps durable, source-adjacent records of what an agent did, why, what evidence exists, what remains open, and what humans observed—as files on disk next to the work, not locked in a chat transcript.

**Moraine records review activity; it does not authorize merge or deployment.**

## Supported platform (beta)

> **x86_64 Linux** with **systemd user** services (glibc). C2 is validated on Arch Linux and Ubuntu 24.04 LTS (install/CLI/service smoke). Other distributions, Windows, and macOS are not claimed for this beta.

Early-stage / **beta**. Useful for local dogfood. Not a production multi-tenant or compliance-grade audit product.

## Install (Linux)

No Rust, Node, or source checkout required for normal use. Full detail: **[docs/INSTALL.md](./docs/INSTALL.md)**.

```bash
tar -xzf moraine-<version>-linux-x86_64.tar.gz
cd moraine-<version>-linux-x86_64
./install.sh
export PATH="$HOME/.local/bin:$PATH"   # prefer before ~/.cargo/bin
moraine setup
moraine doctor
```

## Three-minute quickstart

```bash
# after install + moraine setup
cd /path/to/your/repo
moraine project init .
moraine integrate codex --project .    # first reference agent; optional
moraine doctor --project . --integration codex

# ordinary coding task with Codex (desktop may stay closed)
# later:
moraine open                             # installed desktop ledger workspace
# or open a run path once you have one under .moraine/runs/
```

More: **[docs/QUICKSTART.md](./docs/QUICKSTART.md)** · Codex pack: **[docs/integrations/codex/](./docs/integrations/codex/)**.

## Product workflow

1. **Install** one coherent Moraine suite (CLI, service, MCP/hooks via `moraine`, optional desktop).
2. **Initialize** a project: `moraine project init` → project-local `.moraine/` run ledger.
3. **Capture** while the agent works: Codex hooks (mechanical) + MCP tools (semantic) when the model calls them; service can stay up with the desktop closed.
4. **Inspect** later in the installed desktop: projects → runs → structured timeline (checkpoints, evidence, findings, append-only observations).
5. **Diagnose** drift with `moraine doctor`; **uninstall** product files without deleting project ledgers.

| Surface | Role |
|---------|------|
| `moraine` CLI | Version, setup, doctor, service, project/run protocol, integrate |
| `moraine mcp` | Local STDIO MCP (same core ops + findings tools) |
| `moraine-service` | Per-user capture runtime + rebuildable discovery index |
| Desktop (`moraine-app`) | Ledger workspace; offline direct path open when service is down |
| `.moraine/runs/*` | Canonical run bundles (Markdown projection + sidecar) |

## Demo / screenshot

_Placeholder:_ installed desktop showing **Projects → Runs → Ledger** for a protocol run (timeline, findings, capture coverage). Add a real screenshot when available under `docs/screenshots/`.

## Capability status

| Capability | Current description |
|------------|---------------------|
| Semantic run protocol | **Implemented** |
| Mechanical capture | **Implemented** for supported Codex hooks |
| Evidence | **Minimal trustworthy capture** implemented |
| Findings | **Implemented** |
| Append-only correction | **Implemented** (observations, amend, supersede) |
| Redaction | **Target-scoped ordinary-view withholding** |
| Discovery desktop | **Implemented** |
| Stranger-safe Linux install | **C2 candidate** (this PR / pack) |
| Live collaboration | **Legacy/secondary**; unsupported for untrusted networks |
| Windows | **Planned** |
| macOS | **Planned** |
| Hosted collaboration | **Not planned for beta** |

## Example: agent-run ledger (current)

After a bounded task, a project may contain:

```text
.moraine/
  project.json
  runs/
    2026-07-21-add-hello-txt-821eec9a.md
    2026-07-21-add-hello-txt-821eec9a.md.moraine.json
  sessions/
    …
```

Illustrative narrative shape: [examples/agent-run-migration.md](./examples/agent-run-migration.md) (sample **agent-run ledger** content; real runs are written under `.moraine/runs/` by protocol/hooks).

```bash
moraine run show --run-id <uuid> --json
moraine doctor --project . --integration codex
moraine open --path .moraine/runs/<run>.md
```

## Protocol and integrations

- Agent protocol: [docs/AGENT_RUN_PROTOCOL.md](./docs/AGENT_RUN_PROTOCOL.md)
- MCP tools: [docs/MCP.md](./docs/MCP.md) (live list from `tools/list`, not a fixed five-tool set)
- Codex: [docs/integrations/CODEX.md](./docs/integrations/CODEX.md)
- Redaction: [docs/REDACTION.md](./docs/REDACTION.md)
- Troubleshooting: [docs/TROUBLESHOOTING.md](./docs/TROUBLESHOOTING.md)

## Why Moraine

* Tool-independent durable files next to the work  
* Sparse semantic checkpoints + honest mechanical capture coverage  
* Human inspection without turning Moraine into a merge gate  
* Desktop discovery without requiring a path at launch  
* Installable suite without Rust/Node for normal use  

## Non-goals (now)

Not an approval system, merge gate, general knowledge-management workspace, full agent observability stack, Git/PR replacement, compliance-grade audit product, or production hosted collab service. Agent narrative is not guaranteed true or complete.

## Docs map

| Doc | Audience |
|-----|----------|
| [docs/INSTALL.md](./docs/INSTALL.md) | Users (install/uninstall) |
| [docs/QUICKSTART.md](./docs/QUICKSTART.md) | Users (first project) |
| [docs/DEVELOPMENT.md](./docs/DEVELOPMENT.md) | **Contributors** (cargo/npm, process) |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Design overview |
| [docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md](./docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md) | Canonical product blueprint |
| [ROADMAP.md](./ROADMAP.md) | Direction |

## License

Licensed under the **Apache License, Version 2.0**. See [LICENSE](./LICENSE).

Repo: https://github.com/v-t-r-gg/Moraine
