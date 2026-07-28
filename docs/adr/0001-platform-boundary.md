# ADR 0001: Platform boundary

Status: accepted for W1

## Context

Moraine's canonical run bundles and append-only domain model are portable, but
the installed suite, capture endpoint, and background runtime currently encode
Linux assumptions in several product crates. W1 must make Windows a bounded
backend implementation without claiming Windows runtime support.

## Decision

`moraine-platform` is the narrow authority for:

- host identity and product capability status;
- user data, config, cache, and runtime directories;
- installed-suite component names and locations;
- runtime spool, project-registry, transaction-journal, diagnostics, and
  capture-endpoint descriptions.

`moraine-core` remains the authority for run bundles and ledger semantics and
depends only on the path descriptions it needs for rebuildable user metadata.

Concrete capture clients/listeners remain in the CLI and service. Concrete
background-runtime lifecycle implementations remain in provisioning. Those
backends consume `moraine-platform` descriptions; they do not move into the
platform crate.

Linux reports supported product capabilities. During W1, Windows, macOS, and
unknown hosts report unsupported capabilities and cannot produce product
`Ready`. Test doubles require explicit injection and are never selected because
of an unsupported production host.

W1-A does not change the existing production runtime-manager factory. Removing
its non-Linux memory fallback is a blocking W1-B task: unsupported production
platforms must select an explicit unsupported backend, never
`MemoryServiceManager`.

## Windows decisions carried into W2

- Primary capture transport: per-user named pipe.
- Diagnostics and discovery: loopback HTTP, separate from capture.
- Background runtime: select a user-scoped, no-admin registration mechanism
  before W2 implementation; a system-wide Windows Service is not the default.
- Filesystem layout: local app data for installed binaries and runtime data,
  with `.exe` names described centrally. W3 owns installer application.

## Consequences

Linux paths must remain byte-for-byte compatible and are protected by layout
tests. Windows layouts can be constructed, serialized, and contract-tested on
the current host without claiming that the Moraine workspace compiles or runs
on Windows. W1-B removes direct Unix IPC and duplicate systemd lifecycle
ownership; Windows compilation becomes enforced in W1-C together with
capability-aware product closure.

W1-A establishes a Windows layout model only. Windows runtime support and
Windows CI are not yet established; Moraine remains a Linux-supported product.
