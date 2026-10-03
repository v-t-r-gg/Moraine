# Install Moraine on Linux

Moraine supports x86_64 Linux with a systemd user session & glibc. Windows
compile support is not runtime support. A manually staged Windows demo suite
is described below. A supported Windows installer does not exist, and Windows
Product Ready remains No.

Normal installation does not require Rust, Node.js or a source checkout.

## Install the release archive

```bash
tar -xzf moraine-<version>-linux-x86_64.tar.gz
cd moraine-<version>-linux-x86_64
./install.sh
export PATH="$HOME/.local/bin:$PATH"
```

The default prefix is `~/.local`. Use a custom user prefix when needed:

```bash
./install.sh --prefix /absolute/user/path
```

Keep the selected `bin` directory before stale Cargo installs on `PATH`.

## Set up the product

Launch `moraine-app` from the desktop menu & follow onboarding, or inspect the
installation from the CLI:

```bash
moraine setup
moraine doctor
```

For explicit project setup:

```bash
moraine project init /path/to/project
moraine integrate codex --project /path/to/project
moraine doctor --project /path/to/project --integration codex
```

The desktop may stay closed while capture runs.

## Installed files

The suite includes:

* `~/.local/bin/moraine`;
* `~/.local/libexec/moraine/moraine-service`;
* `~/.local/lib/moraine/moraine-app`;
* a suite manifest;
* a systemd user registration;
* a desktop entry & icon;
* current end-user documentation.

Environment overrides are supported for advanced installations & tests; normal
users should keep one coherent prefix.

## Diagnose

```bash
moraine version --json
moraine doctor --json
moraine service status --json
moraine service logs
```

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for repair paths.

## Uninstall

Run `uninstall.sh` from the extracted archive before deleting it:

```bash
./uninstall.sh
```

The normal path calls the installed CLI runtime backend before removing the
suite. A legacy fallback handles damaged older registrations.

Uninstall removes product binaries, registrations & desktop files. It does not
delete project-local `.moraine/` ledgers. User spool/cache is retained unless
`--purge-user-state` is requested.

Remove managed Codex configuration separately:

```bash
moraine integrate codex --project /path/to/project --remove
```

Contributor build instructions live in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Windows demo suite

The Linux archive above remains the supported product install. The Windows
demo is a staged suite for a follow-along on Windows 11 x86-64. It is not a
signed installer, MSIX, WiX, or WinGet package. Windows Product Ready remains
No. Hosted CI and smoke scripts are not standard-user acceptance.

Build the archive on Windows 11 x86-64:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build-windows-demo.ps1
```

The build script does not register a scheduled task and does not modify the
user profile. The zip is `dist/moraine-<version>-windows-x86_64.zip` with a
SHA-256 sidecar. A Linux run of that script is not a Windows demo.

On the viewer machine, unzip and run `stage-windows-demo.ps1` from the
archive. The script copies `moraine.exe`, `moraine-service.exe`, and
`moraine-app.exe` into the prefix SuitePaths already discovers:
`%LOCALAPPDATA%\Moraine`. Those executables sit in that directory. The
manifest is `%LOCALAPPDATA%\Moraine\share\moraine\manifest.json`. The script
refuses an elevated token. Do not run `./install.sh` on Windows. Stage the
demo suite; no installer is available.

The viewer script is [windows-demo.md](windows-demo.md). Demo uninstall stops
the service, removes the current-user task with `moraine service uninstall`,
and deletes only the suite directory. Project `.moraine/` directories stay.
