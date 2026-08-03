# Development blueprint

This is the forward plan from the current platform boundary. Current behavior
belongs in [ARCHITECTURE.md](../ARCHITECTURE.md); durable product purpose belongs
in [VISION.md](../VISION.md).

## Starting point

Moraine has a supported Linux product with:

* project-local schema-v6 run bundles;
* Codex hooks & local MCP;
* background capture, durable spool & discovery;
* transactional setup, rollback, health & verification;
* explicit platform, capture & runtime backends;
* required Windows workspace compilation;
* fail-closed unsupported-host behavior.

Windows paths, SID-scoped named-pipe capture, current-user Task Scheduler
lifecycle, rotating service logs & shared product control paths are
implemented. Runtime capabilities are enabled for coherent manually staged
suites. Real standard-user acceptance & installation remain unsupported.

## Product invariant

Moraine records agent work & review activity. It does not authorize merges,
deployments, approvals or rejections.

Run bundles remain canonical, source-adjacent & readable without the desktop.
Service indexes & project registries remain rebuildable. New platforms must not
change the run, event, finding or append-only protocol merely to fit an
operating-system backend.

## Architectural constraints

* `moraine-platform` describes hosts, capabilities, paths, layouts, names &
  endpoints; it contains no domain or runtime implementation.
* `moraine-core` owns the durable ledger & projection.
* Capture clients stay with the CLI; listeners stay with the service.
* `moraine-provision` owns background-runtime lifecycle & transaction recovery.
* Unsupported production hosts use explicit unsupported backends; never memory
  test doubles.
* The desktop consumes product-level capability & health state; it does not
  understand systemd, Unix sockets, named pipes or Windows registration.
* Project ledgers survive rollback & uninstall.
* Local IPC & diagnostics must not broaden network exposure.

## W2 acceptance; native Windows 11 runtime

W2 supplies concrete Windows backends without reopening shared orchestration.

### Capture

* Use a per-user named pipe for primary agent event delivery.
* Keep loopback HTTP for diagnostics & discovery.
* Apply an explicit per-user security descriptor.
* Preserve payload limits, event IDs, ordering, deduplication & spool fallback.
* A failed pipe bind must prevent readiness.
* Service-down hooks remain non-disruptive without claiming successful capture.

### Background runtime

* Keep Task Scheduler 2.0 COM operations on dedicated MTA workers.
* Retain production-backed inspect, install, uninstall, start, stop, restart,
  trigger-based autostart & application-log behavior.
* Preserve exact Task Scheduler-returned XML plus ACL restoration.
* Preserve prior running & autostart state through failed setup.
* Return stable unsupported or unavailable states when host facilities are
  missing.

### Product closure

* CLI setup, service commands, doctor, health & repair agree on runtime state.
* Tauri commands retain defense-in-depth guards.
* Native Windows routes through onboarding only when all required backends are
  available.
* Product verification proves real CLI delivery, pipe intake, spool processing,
  session binding, run materialization & discovery.
* Linux behavior & release packaging remain unchanged.

### Validation

* Required Windows unit & integration tests run on `windows-latest`; hosted CI
  proves current-account API mechanics, not a standard-user desktop session.
* A real Windows 11 user session validates runtime registration, restart,
  onboarding, capture, repair, rollback & uninstall behavior.
* Compile support is not reported as runtime support.
* No TCP capture fallback or fake in-memory production backend is accepted.

The fixed Windows task, named-pipe, security, restoration & capability
contracts are recorded in
[ADR 0004](adr/0004-windows-user-runtime-and-capture.md). W2 implementation
must follow that decision rather than reopening the mechanism choice.

## W3 acceptance; signed installation & WinGet

W3 turns the W2 runtime into a distributable Windows product.

* Install CLI, service & desktop as one coherent versioned suite.
* Prefer a user-scoped path & avoid administrator rights where possible.
* Register the chosen background runtime through the shared lifecycle backend.
* Support upgrade, repair & uninstall without deleting project ledgers.
* Sign shipped executables & installer artifacts.
* Publish reproducible package metadata suitable for WinGet.
* Test clean install, reinstall, upgrade, partial failure, rollback & removal.
* Keep package claims separate from graphical acceptance evidence.

Linux remains supported throughout W3. Its archive & installer continue using
the same runtime authority as CLI provisioning.

## Second-agent gate

Claude Code is the second production adapter. It maps into existing run,
checkpoint, evidence, finding & append-only operations; keeps agent-specific
configuration outside `moraine-core`; and uses transactional configuration,
removal & ProductCapture verification with no approval or verdict semantics.

**W2-E Windows interactive acceptance remains pending.** Second-agent work
proceeds independently because it does not alter Windows support claims or
domain protocols.

Further multi-agent fidelity (coverage honesty across adapters) is a following
slice. If a future adapter requires a domain schema change, that change receives
its own compatibility review.

## External-beta gate

External beta evidence requires:

* a clean supported-host install by someone outside the implementation loop;
* one successful real agent run with stated capture coverage;
* project discovery after service & desktop restart;
* diagnosis & recovery from at least one injected failure;
* uninstall confirmation with project ledgers retained;
* current screenshots or demonstration material generated from that workflow;
* limitations stated beside the evidence.

Synthetic hook payloads & headless tests remain useful regression evidence; they
do not substitute for graphical or user-session acceptance.

## Deferred work

Do not mix these into W2 or W3:

* macOS runtime support;
* remote MCP;
* TCP agent capture;
* hosted collaboration;
* multi-user trust;
* broad evidence collection;
* semantic search;
* relay hardening;
* Git or pull-request automation;
* general `moraine-core` public API cleanup.

## Quality & evidence

Every implementation slice should:

* preserve supported schema compatibility;
* add a regression that proves the mutation or failure path actually occurred;
* test fail-closed behavior for unavailable capabilities;
* run the authoritative local gate;
* distinguish local, CI, headless & real-session evidence;
* avoid generated build output in commits;
* keep dogfood run records local unless deliberately curated as an example.

Release-affecting changes also validate the complete package manifest & install
smoke. Windows runtime claims require a real Windows runner; Linux lifecycle
claims require a suitable user session.

## Change control

* Update the smallest authoritative document whose public contract changed.
* Do not create a new guide when an existing authority can express the change.
* Keep exact schemas, flags, errors & tool inventories in code or generated
  checks.
* Use ADRs for accepted architectural decisions; do not turn them into current
  user instructions.
* Keep commits bounded by behavior, compatibility or documentation authority.
* Never use a Moraine review record as merge authorization.
