# ADR 0003: Capability-aware product closure

Status: accepted for W1-C

## Context

Compiling Moraine on a host does not mean that ProductCapture is implemented
or available there. Executable presence, a detected agent, an initialized
ledger, diagnostics HTTP, or a test double must not manufacture product
readiness.

## Decision

ProductCapture support requires supported capture transport, background
runtime, and user installation capabilities. Native desktop product operation
additionally requires a supported desktop host.

Unsupported hosts may inspect version, platform capabilities, health, service
status, and existing ledgers. Portable project-ledger initialization and
rollback/recovery remain available. Product planning, application,
verification, enablement, service mutations, agent-integration repair, and
desktop onboarding fail before mutation.

Serialized setup plans are treated as an untrusted process boundary. Apply
derives its capability requirement from both declared intent and actual
operations, and rejects unsupported or inconsistent plans before witness
calculation or transaction-journal creation.

The native desktop waits for inspection before routing. Unsupported hosts see a
dedicated non-actionable explanation rather than onboarding, service repair, or
enable controls. Browser preview behavior remains separate.

## CI contract

GitHub Actions runs `cargo check --workspace --all-targets` on
`windows-latest`, plus portable platform tests and the Tauri host check. This
proves compilation only. It does not prove Windows runtime startup, graphical
operation, capture, registration, installation, or packaging.

## Consequences

Linux remains the only supported product runtime. W2 owns Windows named-pipe
capture and user-scoped background-runtime registration. W3 owns Windows
installation, signing, and WinGet. No run, event, ledger, finding, or
append-only schema changes are introduced by this decision.
