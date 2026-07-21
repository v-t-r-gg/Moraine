# Security

## Trust model

Moraine is designed for a **local trusted-user** environment:

* The person who can run Moraine on a machine can also read and write the project files.
* There is **no multi-tenant authentication** and **no cryptographic agent identity**.
* Moraine aims for **tamper-evident structured history** in bounded ways (locks, hashes, append-only ops), not **tamper-proof** protection against a filesystem owner.

## Redaction is not secure erasure

Redaction is an **append-only ordinary-view withholding** operation:

* A redaction remains **detectable** in the ledger.
* Ordinary desktop, CLI, Tauri, service discovery, and MCP projections must not return the protected claim content.
* The **canonical sidecar may retain prior content** for integrity.
* Git history, backups, older clones, screenshots, and evidence artifact files are **outside** Moraine redaction.

See [docs/REDACTION.md](./docs/REDACTION.md).

## If a secret appears in a ledger

1. **Rotate or revoke** the secret in the system that issued it.
2. Remove it from **source history and backups** where policy requires.
3. **Redact** the relevant Moraine checkpoint (or other target) so ordinary Moraine views withhold it.
4. Inspect **evidence artifacts** and external references separately.
5. Do **not** treat Moraine redaction alone as remediation.

## Reporting vulnerabilities

This repository does not currently define a private security reporting channel.

If you discover a vulnerability, open a **private maintainer contact** if one is listed on the GitHub organization or profile; otherwise open a carefully minimized public issue **without** pasting secrets or exploit payloads.
