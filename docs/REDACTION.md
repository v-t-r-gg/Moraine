# Redaction in Moraine

## Invariant

> A redacted entry remains **detectable** as a historical ledger entry, but its protected content is **withheld from ordinary Moraine projections**.

## What redaction is

* A structured **append-only** operation (`entry_redact`)
* Visible as having occurred (timeline / ops list)
* Associated with stable target identity, reason, actor category, timestamps, and hashes
* Honored by ordinary desktop, CLI, Tauri, service discovery, and MCP projections via a **single core projection layer**

## What redaction is not

* Secure erasure
* Credential rotation
* Deletion from Git history or backups
* Proof that no local file retains prior content
* An authenticated security action

## Ordinary projections (must withhold)

After a checkpoint claim is redacted, ordinary DTOs and UI must not expose:

* original / amended / superseding claim text
* actions, rationales, risks, open questions from that checkpoint
* embedded evidence labels, commands, paths, URLs
* frozen finding target snapshot content
* `previousContent` / unprotected `newContent` on related append-only ops

They may expose:

* `targetRedacted: true` / `redacted: true`
* stable IDs, timestamps, snapshot hashes
* summary marker `[REDACTED]`
* finding **thread** body (unless independently redacted)

## Checkpoint redaction vs evidence files

* **Checkpoint redaction** withholds checkpoint-embedded evidence **text** from ordinary checkpoint projections.
* It does **not** automatically delete a separately stored evidence artifact under `.moraine/evidence/`.
* If the product lacks an evidence-artifact redaction operation, treat artifact removal as a separate manual/ops step.

## Raw forensic access

Canonical project files remain readable with ordinary filesystem tools. That is **forensic / owner** access, not an ordinary Moraine review path.

Ordinary `run show`, discovery, finding list/get/respond, and MCP tools must not silently become forensic bypasses.

## Implementation note

Projection helpers live in `moraine-core` (`projection` module, finding `project_target_*`, discovery timeline, Markdown render). Transports must not re-serialize raw sidecar checkpoints for ordinary results.
