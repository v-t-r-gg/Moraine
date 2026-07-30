# Roadmap

Moraine records review activity; it does not render a verdict.

## Completed

* Project-local Markdown run records with structured schema-v6 sidecars.
* Local STDIO agent protocol with checkpoints, evidence & findings.
* Append-only observations, amendments, supersessions & target redaction.
* Codex hooks plus MCP integration.
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

## Later

1. **W3; signed installer & WinGet**
   * no-admin installation where practical;
   * uninstall that preserves project ledgers;
   * signing, upgrade & package-manager validation.
2. **Second agent adapter**
   * begin only after the shared protocol survives Linux & Windows backends;
   * preserve one run model rather than adding agent-specific domain state.
3. **External beta evidence & product presentation**
   * clean-machine workflows;
   * real graphical acceptance;
   * current screenshots or demonstrations only when reproducible.
4. **Deferred**
   * broad evidence expansion;
   * semantic or vector search;
   * relay authentication;
   * richer Git/PR integration;
   * hosted or live-collaboration expansion;
   * general public API reorganization.

Implementation constraints & acceptance gates live in
[docs/DEVELOPMENT_BLUEPRINT.md](docs/DEVELOPMENT_BLUEPRINT.md). Current
architecture lives in [ARCHITECTURE.md](ARCHITECTURE.md).
