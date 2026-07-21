# Troubleshooting (installed suite)

## Stale CLI on PATH

Symptoms: `moraine version` shows an unexpected commit or old version; doctor warns about `~/.cargo/bin`.

```bash
type -a moraine
moraine doctor --json
# put suite first:
export PATH="$HOME/.local/bin:$PATH"
hash -r   # bash
# zsh: rehash
```

Do not delete binaries blindly; deprioritize Cargo installs for normal use.

## Service unit points at Cargo

```bash
grep ExecStart ~/.config/systemd/user/moraine-service.service
moraine service install
systemctl --user daemon-reload
moraine service restart
```

`ExecStart` must be an absolute path under `~/.local/libexec/moraine/` (or your prefix).

## Service offline

```bash
moraine service start
moraine service status --json
moraine service logs
```

Hook events spool under `~/.cache/moraine-service` when the service is down and process on restart.

## Codex integration

```bash
moraine doctor --project . --integration codex
moraine setup codex --project . --check
# repair absolute paths:
moraine setup codex --project .
# remove only Moraine-managed entries:
moraine setup codex --project . --remove
```

Unrelated MCP servers and non-Moraine hooks are preserved. Config without managed markers is not auto-removed.

## Desktop offline / version mismatch

The status bar shows service online/offline and mismatch. Discovery falls back to direct filesystem inspection when the service is down. Run `moraine doctor` for remediation.

## Uninstall without losing ledgers

```bash
./uninstall.sh
# project .moraine/ directories remain
# spool retained unless --purge-user-state
```

## Support claim

x86_64 Linux + systemd user services. Validated profiles are listed in [INSTALL.md](./INSTALL.md).
