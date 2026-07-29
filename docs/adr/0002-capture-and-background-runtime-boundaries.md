# ADR 0002: Capture and background-runtime boundaries

Status: accepted for W1

## Context

W1-A centralized host and layout descriptions, but hook delivery and service
intake still directly owned Unix socket calls, while provisioning, the CLI, and
the service executable each owned parts of the systemd lifecycle. Diagnostics
could also report online after capture binding failed.

## Decision

The CLI owns endpoint-dispatched delivery of serialized hook payloads. The
service owns endpoint-dispatched listener binding. Linux Unix socket code is
confined to each product's `capture/linux_unix.rs` backend. The event schema,
mapping, durable spool, size limit, ordering, and deduplication remain shared
product behavior and are unchanged.

Capture binds before diagnostics. `/status` reports both `online` and
`captureReady`; provisioning requires both. A supported but temporarily
unavailable Linux socket causes durable spooling. An unsupported endpoint emits
an explicit diagnostic and does not create an indefinitely unprocessable item.

Provisioning owns the `BackgroundRuntimeManager` contract. Its Linux systemd
user backend is the sole Rust owner of registration rendering, lifecycle
commands, logs, registration snapshots, fingerprints, restoration, and
registration reload. The CLI, desktop commands, setup, repair, apply, rollback,
and inspection all use this authority. The service executable only runs the
runtime.

Unsupported production hosts select `UnsupportedRuntimeManager`. The memory
backend requires explicit test injection and is never a production factory
fallback. Existing `ServiceManager` names and Linux journal fields remain
source/serialization compatibility vocabulary; new implementation code uses
platform-neutral runtime terms.

## Consequences

Linux paths, command names, hook payloads, spool behavior, and transaction
journal JSON remain compatible. Registration recovery is platform-owned, so a
future Windows backend can restore its registration without teaching the
transaction engine about Task Scheduler or another mechanism.

This decision does not implement named pipes or a Windows runtime registration
backend. Windows layouts are modeled & the complete workspace compiles in
required `windows-latest` CI. Runtime capture, registration, desktop operation
& installation remain unsupported. Linux remains the only supported runtime.
