# Moraine quick start (Linux)

Install from a release bundle — not from a source checkout.

```bash
tar -xzf moraine-<version>-linux-x86_64.tar.gz
cd moraine-<version>-linux-x86_64
./install.sh
export PATH="$HOME/.local/bin:$PATH"   # before ~/.cargo/bin
moraine setup
moraine doctor
```

## First project + Codex

```bash
cd /path/to/your/repo
moraine project init .
moraine integrate codex --project .
# or: moraine setup codex --project .
moraine doctor --project . --integration codex
```

Start Codex in that project as usual. Keep the Moraine desktop closed if you only need capture; open it later to inspect runs.

## Useful commands

| Command | Purpose |
|---------|---------|
| `moraine version --verbose` | Suite / PATH / service identity |
| `moraine service status` | systemd user unit + diagnostics |
| `moraine doctor --json` | Actionable health report |
| `moraine setup` | Repair unit, start service, print next steps |

Full install notes: [INSTALL.md](./INSTALL.md). Codex details: [integrations/CODEX.md](./integrations/CODEX.md).
