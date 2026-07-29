# Security

## Trust model

Moraine is a local product for one trusted operating-system user. It is not a
multi-user security boundary, hosted audit service or tamper-proof archive.

Do not expose its capture endpoint, diagnostics server, collaboration relay or
project files to untrusted networks or users.

Linux ProductCapture uses a user-scoped Unix socket & loopback diagnostics.
The Windows workspace compiles, but no supported Windows capture, runtime or
installer exists yet. Windows security claims begin with the native backend.

## Data locations

Canonical project data may include:

* `.moraine/project.json`;
* `.moraine/runs/*.md`;
* `.moraine/runs/*.md.moraine.json`;
* `.moraine/evidence/`;
* `.codex/config.toml` & `.codex/hooks.json`.

User-level state includes the project registry, capture spool, setup journals,
service registration & rebuildable discovery index. Exact paths come from the
platform layout.

Uninstall & setup rollback do not delete project ledgers.

## Integrity

Moraine uses expected hashes, revisions, idempotency keys, file locks & atomic
replacement to detect stale work & reduce partial writes. These controls do not
prevent a local user or malicious process from changing files.

Git history, filesystem permissions, backups & external signing remain separate
integrity layers.

## Redaction

Redaction is target-scoped. It records an append-only operation that withholds a
claim from ordinary Moraine projections.

Ordinary list, show, timeline, Markdown, discovery, desktop & MCP views must not
reveal the redacted claim text. The redaction event remains visible so the
ledger does not pretend that history never changed.

Redaction does not automatically erase:

* raw sidecars opened directly;
* Git history or older clones;
* backups, logs or screenshots;
* separately stored evidence artifacts;
* content copied into another finding or observation.

Raw project files are forensic access. Anyone who can read them may recover
historical values. Evidence artifacts have their own lifecycle; redacting a
claim does not silently delete an evidence file.

## Secret response

If a secret enters Moraine:

1. Revoke or rotate it at the source.
2. Redact affected claims from ordinary Moraine views.
3. Remove or replace separate evidence artifacts when policy allows.
4. Clean Git history, backups, logs & screenshots as required.
5. Review spool & transaction-journal copies.
6. Record a non-sensitive correction describing the response.

Do not treat Moraine redaction as secret revocation or secure erasure.

## Agent & evidence risk

Agent-provided summaries, commands, paths, URLs & findings are untrusted input.
Evidence references state what was recorded; they do not prove that a command
was safe, a test was sufficient or a file was authentic.

The desktop renders ordinary projections. Features that expose raw JSON or
evidence bytes must label that forensic boundary.

## Vulnerability reporting

Do not open a public issue for a suspected vulnerability that would expose
secrets or an exploitable path. Use GitHub’s private vulnerability reporting
for the repository, or contact the maintainers privately when that feature is
unavailable.

Include the affected version, platform, reproduction, impact & whether project
or user-state files were exposed. Never attach real secrets or private ledgers.
