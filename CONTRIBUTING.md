# Contributing

Moraine is a Rust workspace with a React/Tauri desktop. Product purpose lives
in [VISION.md](VISION.md); current boundaries live in
[ARCHITECTURE.md](ARCHITECTURE.md).

## Source dependencies

Install:

* Rust stable plus the repository MSRV where compatibility work requires it;
* Node.js 20+ & npm;
* Linux Tauri/WebKit development packages;
* Git;
* systemd user tools for real Linux lifecycle tests.

The Windows CI job compiles the workspace; Windows runtime testing begins with
the native backend work.

## Common commands

```bash
cargo fmt --all
cargo check --workspace --all-targets
cargo test -p moraine-core
cargo test -p moraine-provision
npm ci
npm run check
npm test
npm run build
```

Run the authoritative local gate before pushing:

```bash
./scripts/check.sh
```

It covers formatting, strict Clippy, production Rust crates, Rust tests, Tauri
checks & command tests, frontend typecheck, tests, build & documentation
contracts.

Build the Linux release bundle separately when packaging changes:

```bash
./scripts/build-linux-release.sh
```

## Pull requests

* Branch from current `main`.
* Keep changes bounded; avoid unrelated refactors.
* Add regression coverage for behavior or compatibility changes.
* Preserve user files, supported sidecars & persisted transaction shapes.
* State which validation ran & what environment it used.
* Keep generated build output out of commits.
* Do not merge a failing required check.

Local Moraine records under `.moraine/` are untracked by default. They may be
published deliberately as a validated case study; they are never required as a
merge authorization mechanism.

## Validation language

Use exact claims:

* **Implemented** means code is present.
* **Tested locally** names the command & environment.
* **Tested in CI** means the relevant required job passed.
* **Compile-tested** does not mean runtime-supported.
* **Headless-tested** does not mean a graphical lifecycle passed.
* **Planned** means no implementation claim.

For Linux desktop or systemd behavior, state whether a real graphical/user
session was available.

## Compatibility

Changes to sidecars, run Markdown, events, findings, journals, receipts or
public JSON require:

* an explicit compatibility decision;
* fixtures for the oldest supported readable form;
* round-trip or migration coverage;
* truthful writable-schema documentation;
* rollback review when setup state is affected.

Do not rename serialized operation variants merely to improve internal naming.
Prefer compatibility aliases when a public Rust name must move.

## Documentation

An implementation change must update the smallest authoritative document whose
public contract changed. Do not create a new document when an existing
authority can express the change.

Authority order:

* `README.md`; current public entry point;
* `VISION.md`; stable purpose & non-goals;
* `ARCHITECTURE.md`; implemented structure & ownership;
* `ROADMAP.md`; current & later work;
* `docs/DEVELOPMENT_BLUEPRINT.md`; forward acceptance plan;
* focused user guides under `docs/`;
* ADRs; historical architectural decisions.

Code owns exact schemas, flags, error codes & tool inventories. Prefer generated
or checked claims instead of copied lists.

Public docs should be concise, casual & easy to scan. Use short paragraphs,
plain language, `&` instead of “and” & semicolons instead of em dashes.

## Security

Do not add network exposure, secret capture or multi-user assumptions without a
separate security review. Redaction changes require ordinary-view non-leak
tests across CLI, MCP, discovery, Markdown & desktop projections.

Report vulnerabilities through the process in [SECURITY.md](SECURITY.md).
