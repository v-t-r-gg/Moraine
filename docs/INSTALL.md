# Install Moraine (Linux x86_64)

**Supported profile (C2):** x86_64 Linux with **systemd user** services. Validated development target: Arch Linux. Clean target: Ubuntu 24.04 LTS when exercised. Other distributions are unverified.

You do **not** need Rust, Cargo, Node.js, npm, or a Moraine source checkout for normal use.

## Install from a release bundle

```bash
tar -xzf moraine-<version>-linux-x86_64.tar.gz
cd moraine-<version>-linux-x86_64
./install.sh
# optional:
# ./install.sh --prefix "$HOME/.local"
# ./install.sh --dry-run
```

Ensure `~/.local/bin` is on your `PATH` **before** `~/.cargo/bin` so a stale Cargo install does not shadow the suite.

```bash
export PATH="$HOME/.local/bin:$PATH"
moraine version --verbose
moraine service start
moraine doctor
```

## Configure the first reference integration (Codex)

```bash
moraine project init /absolute/path/to/your/repo
moraine setup codex --project /absolute/path/to/your/repo
moraine doctor --project /absolute/path/to/your/repo --integration codex
```

Start Codex in that project as usual. Hooks and MCP use the **installed** `moraine` on `PATH`. The desktop may remain closed while capture runs.

## Desktop

After install, launch `moraine-app` from the menu (if registered) or:

```bash
~/.local/lib/moraine/moraine-app
```

Discovery talks to the local service over **loopback HTTP** with a native client (no `curl` required).

## Uninstall product files (keeps ledgers)

```bash
cd moraine-<version>-linux-x86_64   # or keep a copy of uninstall.sh
./uninstall.sh
```

This removes suite binaries, unit files, and desktop registration. It does **not** delete project-local `.moraine/` run records. Spool/cache under `~/.cache/moraine-service` is retained unless you pass `--purge-user-state`.

Remove Codex project config separately:

```bash
moraine setup codex --project /path/to/repo --remove
```

## Diagnostics

```bash
moraine version --json
moraine doctor --json
moraine service status --json
moraine service logs
```

## Building a bundle (developers only)

```bash
./scripts/build-linux-release.sh
# output: dist/moraine-<version>-linux-x86_64.tar.gz
```

Development workflows (`cargo run`, `npm run tauri:dev`) remain available for contributors; they are **not** the stranger-safe path.
