# Moraine

Moraine is a local-first ledger for coding-agent work.

Agent work often disappears into chat history. Moraine keeps the run, evidence,
findings, corrections & review activity as durable files beside the project.
It records what happened; it does not approve merges, deployments or releases.

## Platform support

| Capability | Linux | Windows |
|---|---:|---:|
| Workspace compiles | Yes | Yes |
| Ledger format | Yes | Yes |
| Desktop product runtime | Yes | No |
| Agent capture | Unix socket | No |
| Background runtime | systemd user | No |
| Installer | User archive | No |
| Product Ready | Yes | No |

The supported product is x86_64 Linux with a systemd user session. Windows is
compile-tested in GitHub Actions; capture, runtime registration, desktop use &
installation remain unsupported.

## Install

Download the Linux release archive, extract it & run:

```bash
./install.sh
export PATH="$HOME/.local/bin:$PATH"
moraine setup
moraine doctor
```

The archive installs into `~/.local` by default. See
[docs/INSTALL.md](docs/INSTALL.md) for removal, custom prefixes & limitations.

## First run

The desktop onboarding flow is the normal setup path. The equivalent CLI flow
is:

```bash
moraine project init /path/to/project
moraine integrate codex --project /path/to/project
moraine doctor --project /path/to/project --integration codex
```

Start Codex in that project. Hooks deliver mechanical events while the local
MCP gives the agent semantic run operations. Open the desktop later:

```bash
moraine open
```

## Files Moraine creates

Project records are canonical & source-adjacent:

```text
.moraine/
├── project.json
├── runs/
│   ├── <run>.md
│   └── <run>.md.moraine.json
├── sessions/
└── evidence/
```

The Markdown file is the readable projection. The sidecar is the structured
run ledger. User-level project registration, transaction journals, spool data &
the service index are rebuildable support state; uninstall never deletes
project ledgers.

## Product parts

| Part | Responsibility |
|---|---|
| `moraine` | Setup, diagnosis, project/run operations & compatibility commands |
| `moraine mcp` | Local STDIO agent protocol |
| `moraine-service` | Capture intake, durable spool & rebuildable discovery |
| `moraine-app` | Projects, runs, timeline, findings & health |
| Codex integration | Managed hooks & MCP configuration |

The desktop is not required for capture. Local collaboration & the free-form
editor remain compatibility surfaces; they are not the primary product.

## Trust boundary

Moraine assumes one trusted local user. Its sidecars are integrity-aware, not
tamper-proof. Redaction hides a target from ordinary projections; it does not
erase Git history, backups, raw forensic files or separate evidence artifacts.
Do not expose the local service to untrusted networks.

See [SECURITY.md](SECURITY.md) for the complete trust model.

## Documentation

| Document | Authority |
|---|---|
| [VISION.md](VISION.md) | Stable product purpose & boundaries |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current implemented architecture |
| [ROADMAP.md](ROADMAP.md) | Current & later work |
| [docs/INSTALL.md](docs/INSTALL.md) | Supported install & removal |
| [docs/AGENT_RUN_PROTOCOL.md](docs/AGENT_RUN_PROTOCOL.md) | Durable run contract |
| [docs/integrations/CODEX.md](docs/integrations/CODEX.md) | Codex setup & diagnosis |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Operational recovery |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Source development & documentation rules |

Exact flags, schemas, errors & MCP tool inventories are defined by code.

Licensed under Apache-2.0; see [LICENSE](LICENSE).
