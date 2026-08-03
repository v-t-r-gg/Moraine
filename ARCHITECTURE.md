# Architecture

This document describes the implemented system. Future work belongs in
[ROADMAP.md](ROADMAP.md); historical decisions belong in [docs/adr](docs/adr/).

## Core invariants

* The agent run is the primary domain object.
* Project-local run bundles are canonical.
* User-level indexes, registries, spool data & journals are rebuildable support
  state.
* The desktop is not required for capture.
* Review activity is recorded; Moraine does not authorize outcomes.
* Ordinary views respect redaction; raw local files remain forensic data.

## Dependency direction

```text
moraine-platform
      ↑
moraine-core
moraine-cli
moraine-service
moraine-provision
src-tauri
```

`moraine-platform` is foundational & does not depend on Moraine domain,
provisioning, service, IPC implementation or Tauri types. `moraine-core` owns
the domain & does not depend on runtime control.

## Crate authorities

### `moraine-platform`

Owns host identity, capability descriptions, user directories, suite layout,
runtime-layout defaults, executable names & capture endpoint descriptions.

It derives the Windows account SID, stable capture scope & named-pipe
description. It does not create pipes, manage background registration or
derive product readiness.

### `moraine-core`

Owns projects, runs, Markdown projection, schema-v6 sidecars, checkpoints,
evidence metadata, findings, append-only operations, redaction projection,
idempotency, recovery & read-only discovery models.

It also owns registry serialization because registered project roots are
domain-adjacent rebuildable metadata. Registry file placement comes from the
platform layout.

`moraine-core` does not install services, bind IPC, derive product setup plans
or depend on the desktop.

### `moraine-provision`

Owns system inspection, setup plans, approved-plan application, transaction
journals, rollback, health, repair, agent integration adapters (Codex & Claude
Code) & background-runtime lifecycle.

The transaction engine is platform-neutral. Runtime backends capture & restore
their own registration state. Linux uses a systemd user backend. Windows uses a
current-user Task Scheduler 2.0 backend with dedicated MTA workers; no COM
interface crosses a worker boundary. macOS & unknown production hosts use an
explicit unsupported backend. The memory runtime is available only through
test injection.

Provisioning guards ProductCapture before planning, witness calculation,
journal creation or mutation. A serialized plan is validated from both its
intent & its actual operations.

### `moraine-cli`

Owns command-line presentation, agent hook mapping & capture delivery.

Hooks parse Codex (`hook-codex`) or Claude Code (`hook-claude-code`) input,
create stable event identifiers & decide whether to spool. Linux delivers
through a Unix socket. Windows delivers through a bounded write-only named-pipe
client. Unsupported endpoints fail explicitly; they do not use a fake transport.
Claude Code session IDs are namespaced so they cannot collide with Codex.

Service lifecycle commands call the provisioning runtime manager. The CLI does
not contain a second systemd implementation.

### `moraine-service`

Owns capture listener backends, spool intake, event processing, loopback
diagnostics & the rebuildable discovery index.

Startup binds capture before publishing diagnostics readiness. Unexpected
listener failure stops truthful readiness. Linux Unix-socket & Windows
named-pipe behavior live in separate backends; the service executable does not
install or control its operating-system registration.

On Windows, the service runs without a console window & writes size-bounded
UTF-8 application logs below the user runtime layout. Task Scheduler controls
registration & lifecycle; it is not used as an application-log source.

### `moraine-mcp`

Maps local STDIO JSON-RPC calls onto `moraine-core` run operations. The project
root is fixed for the process lifetime. Tool inventory comes from code; clients
should discover it with `tools/list`.

### `moraine-server`

Hosts the legacy collaboration relay. It is compatibility infrastructure, not
canonical persistence & not part of ProductCapture readiness.

### Tauri & React

Tauri commands are a typed boundary over core, provisioning & discovery
operations. Product-mutating commands repeat capability guards for early,
predictable failure; the underlying library remains authoritative.

React owns presentation, routing & temporary UI state. Native routing waits for
inspection, derives desktop support from explicit capabilities & never treats
local storage, executable presence or diagnostics alone as product readiness.

The ledger workspace is the primary surface. The free-form editor & live
collaboration code remain compatibility-only.

## Canonical project data

```text
<project>/.moraine/
├── project.json
├── runs/
│   ├── <run>.md
│   └── <run>.md.moraine.json
├── sessions/
└── evidence/
```

The sidecar is the structured authority for a run. Markdown is a deterministic,
human-readable projection with preserved human-notes bytes. Optional evidence
artifacts are separate files referenced by the ledger.

Mutation paths use per-document locks, re-read after locking & replace through
unique temporary files. Supported older sidecars load through compatibility
paths; writes promote to the current writable schema. Exact schema constants &
types live in `moraine-core`.

## Rebuildable user state

The runtime layout provides:

* a registry of canonical project roots;
* setup transaction journals;
* capture spool directories;
* diagnostics & capture endpoints.

The registry is not a second run database. The service rebuilds discovery by
scanning registered roots. Missing projects remain diagnosable. Registry or
index loss does not change canonical project records.

## Capture path

```text
Codex hook
  → CLI event mapping
  → platform capture endpoint
  → service listener
  → durable spool
  → event validation & deduplication
  → session binding
  → project run materialization
  → rebuildable discovery index
```

If the selected local endpoint is temporarily unavailable, valid hook payloads
spool without disrupting the agent. Windows access denial is reported as a
local security/configuration diagnostic while preserving the same fallback.
Product verification still requires a real, session-bound run to materialize &
remain readable; direct core tests cannot produce product `Ready`.

Mechanical hooks establish activity coverage. MCP checkpoints add semantic
intent, evidence & findings. Moraine reports the distinction through a shared
capture fidelity report: adapter capability profiles are separate from observed
facts; legacy `captureCoverage` remains a compact compatibility field; `full`
means both primary channels were observed, not complete agent knowledge.

The desktop review workspace coordinates project and run discovery with a single
selected-run review surface (overview, checkpoints, evidence, findings, history).
Capability tables remain at the provision adapter boundary; the UI does not
reimplement them.

## Setup & rollback

The desktop or CLI creates an exact setup plan with a state witness. Apply
checks the witness, writes a transaction journal before mutation & records
runtime mutation attempts before external side effects.

Rollback restores agent configuration bytes, runtime registration, reload
state, prior running state & prior autostart state. Restoration errors return a
manual-recovery outcome. Newly initialized project ledgers are retained to
avoid deleting records; the receipt reports that retained state.

The Windows backend snapshots Task Scheduler-returned XML plus its effective
security descriptor. Restoration re-registers that normalized definition &
requires the combined fingerprint to match. Task absence is also an explicit
restorable state.

## Readiness

Product readiness requires:

* supported capture & background-runtime capabilities;
* supported desktop capability for native desktop setup;
* valid runtime registration;
* live diagnostics & capture intake;
* a detected agent;
* an initialized registered project;
* configured agent integration that does not need repair;
* successful ProductCapture verification where setup is applied.

Unsupported hosts remain inspectable but cannot plan, apply, repair or verify
ProductCapture.

## Trust & projection

Moraine assumes one trusted local user. Loopback diagnostics are not a remote
API; capture uses local IPC. Sidecar hashes & expected-revision checks detect
stale or conflicting work but do not make files tamper-proof.

Target redaction withholds a claim from ordinary list, show, timeline, Markdown
& MCP projections. The append-only redaction operation remains visible. Raw
sidecars, Git history, backups & independent evidence artifacts are forensic
access paths; see [SECURITY.md](SECURITY.md).

## Platform status

Linux is the supported runtime: Unix capture, systemd user registration &
archive installation.

The full Rust workspace compiles on Windows in required CI. The SID-scoped
named-pipe transport, current-user Task Scheduler backend & rotating service
logs are implemented & production-tested on the hosted runner. Shared setup,
doctor, health, repair, rollback, verification & desktop onboarding now use
those Windows backends for coherent manually staged suites. Windows
installation & real standard-user graphical acceptance remain unsupported;
public runtime support waits for W2-E.

## Protocol & operations

The durable run contract is documented in
[docs/AGENT_RUN_PROTOCOL.md](docs/AGENT_RUN_PROTOCOL.md). Installation is in
[docs/INSTALL.md](docs/INSTALL.md). Exact commands, serialized variants, schema
fields & tool names remain code authorities.
