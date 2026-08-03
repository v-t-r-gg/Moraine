# Roadmap

Moraine records review activity; it does not render a verdict.

## Completed

* Project-local Markdown run records with structured schema-v6 sidecars.
* Local STDIO agent protocol with checkpoints, evidence & findings.
* Append-only observations, amendments, supersessions & target redaction.
* Codex hooks plus MCP integration.
* Claude Code ProductCapture adapter (project MCP & lifecycle hooks).
* Multi-agent capture fidelity reporting (capability vs observation, session v3
  counts, CLI/MCP/desktop shared report; no percentage scores).
* Background capture, durable spool & rebuildable project discovery.
* Transactional Linux & staged-Windows provisioning, rollback, repair &
  self-verification.
* Linux archive installation with a user-scoped systemd runtime.
* Platform, path, capture & runtime boundaries.
* Required Windows workspace compilation with fail-closed product behavior.

## Current

### W2; native Windows 11 runtime

Complete real Windows 11 acceptance for the implemented runtime:

* native desktop lifecycle acceptance on Windows 11.
* standard non-administrator setup, capture, restart, repair & rollback;
* cross-account named-pipe denial;
* sign-out/restart autostart behavior.

W2 does not include a signed installer or WinGet publication.

**W2-E Windows interactive acceptance remains pending** (`not_executed`). Draft
PR #26 records that evidence gate. The external-beta review workspace proceeds
independently and does not alter Windows support claims.

### External beta review workspace

Desktop Projects → Runs → Review flow for evaluators without schema knowledge:
overview, checkpoints, evidence, findings, history, capture fidelity, and
recovery notices. See [docs/EXTERNAL_BETA_REVIEW.md](docs/EXTERNAL_BETA_REVIEW.md).

## Later

1. **W3; signed installer & WinGet**
   * no-admin installation where practical;
   * uninstall that preserves project ledgers;
   * signing, upgrade & package-manager validation.
2. **External beta evidence expansion**
   * clean-machine workflows beyond the coordinated review workspace;
   * broader graphical demonstration packs when reproducible.
3. **Deferred**
   * broad evidence expansion;
   * semantic or vector search;
   * relay authentication;
   * richer Git/PR integration;
   * hosted or live-collaboration expansion;
   * general public API reorganization.
   * Claude tool-call capture (beyond current lifecycle hooks).

Implementation constraints & acceptance gates live in
[docs/DEVELOPMENT_BLUEPRINT.md](docs/DEVELOPMENT_BLUEPRINT.md). Current
architecture lives in [ARCHITECTURE.md](ARCHITECTURE.md).
