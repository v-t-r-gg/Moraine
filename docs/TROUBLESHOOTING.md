# Troubleshooting

Start with read-only inspection:

```bash
moraine version --json
moraine doctor --json
moraine service status --json
```

## Unsupported platform

Windows, macOS & unknown hosts can inspect Moraine but cannot run
ProductCapture. Setup, runtime mutation, agent repair & desktop onboarding fail
closed. A present executable or initialized project does not change that state.

Windows compilation is tested; runtime support begins with W2.

## Suite is incomplete

If `doctor` reports missing or incoherent components:

1. Check that `PATH` resolves the CLI from the installed prefix.
2. Re-run `install.sh` from one release archive.
3. Run `moraine version --json` & compare suite component versions/hashes.

Do not mix Cargo-built binaries with a release suite.

## Background capture is unavailable

On supported Linux:

```bash
moraine service status --json
moraine service logs
moraine service restart --json
```

If registration is missing or invalid, run `moraine setup` or use desktop
health repair. A diagnostics response without live capture intake is not ready.

## Project is missing after restart

Project runs remain canonical under `.moraine/`. Re-register an existing
project through desktop “Add project” or:

```bash
moraine project init /absolute/path/to/project
```

The command is idempotent & registers the canonical root. Missing registered
paths remain visible in diagnostics.

## Codex is detected but capture is absent

```bash
moraine integrate codex --project /absolute/path/to/project
moraine doctor --project /absolute/path/to/project --integration codex
moraine self-test --project /absolute/path/to/project --json
```

See [integrations/CODEX.md](integrations/CODEX.md) for managed files & coverage.

## Setup failed

Provisioning attempts automatic rollback. Inspect the returned outcome:

* `rolledBack`; prior mutable state was restored;
* `rollbackRequired`; restoration needs manual attention;
* `degraded`; safe project ledger state was intentionally retained.

Do not delete `.moraine/` to repair setup. Transaction journals live in the
Moraine user-data directory & support explicit rollback/recovery.

## Runs exist but the desktop is empty

Confirm the project is registered & readable:

```bash
moraine doctor --project /absolute/path/to/project
moraine open --path /absolute/path/to/project
```

The service index is rebuildable. Broken or future-schema sidecars should be
reported rather than suppressing healthy runs.

## Redacted text is still on disk

This is expected. Target redaction withholds text from ordinary projections;
raw sidecars, Git history, backups & separate evidence artifacts remain
forensic sources. Follow the secret-response guidance in
[../SECURITY.md](../SECURITY.md).

## Uninstall left project files

Project-local `.moraine/` directories are retained by design. Uninstall removes
the installed suite & user registration; project ledgers belong to the project
owner.
