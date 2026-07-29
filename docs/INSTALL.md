# Install Moraine on Linux

Moraine supports x86_64 Linux with a systemd user session & glibc. The workspace
compiles on Windows, but no supported Windows runtime or installer exists before
W2/W3.

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
