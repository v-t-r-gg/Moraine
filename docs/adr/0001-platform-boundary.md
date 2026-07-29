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

W1-B completed the production runtime-manager factory: unsupported production
platforms select an explicit unsupported backend, never the memory test
implementation.

## Windows decisions carried into W2

- Primary capture transport: per-user named pipe.
- Diagnostics and discovery: loopback HTTP, separate from capture.
- Background runtime: select a user-scoped, no-admin registration mechanism
  before W2 implementation; a system-wide Windows Service is not the default.
- Filesystem layout: local app data for installed binaries and runtime data,
  with `.exe` names described centrally. W3 owns installer application.

## Consequences

Linux paths remain byte-for-byte compatible and are protected by layout tests.
W1-B confines direct Unix IPC to Linux capture backends and removes duplicate
systemd lifecycle ownership. W1-C enforces complete-workspace compilation on a
real `windows-latest` runner and closes unsupported-host product behavior.

Compilation support is not runtime support. Windows capture, background-runtime
registration, desktop product operation, installation, packaging, and product
`Ready` remain unsupported until their W2/W3 implementations. Moraine remains
a Linux-supported product.
