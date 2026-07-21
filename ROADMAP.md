# Roadmap

High-level direction. Product model: [VISION.md](./VISION.md), [ARCHITECTURE.md](./ARCHITECTURE.md), [docs/DEVELOPMENT_BLUEPRINT.md](./docs/DEVELOPMENT_BLUEPRINT.md).

**Invariant:** Moraine records review activity; it does not render the verdict.

## Done (foundation)

* Run records as Markdown + `*.md.moraine.json` ledger (schema through v6 append-only ops)
* Stable run ID + SHA-256 content hash; historical decisions preserved for compatibility
* Operation-based annotation mutations; durable suggestion dispositions
* Agent run protocol: `project init`, `run start|checkpoint|show|ready|resume|open`
* Per-document ledger lock + safe atomic replace; deterministic legacy migration
* CLI: protocol + `share`, `status --json`, `init`, file helpers
* **Milestone 0:** vision realignment and decision de-centering (docs, legacy `decide`, primary UI)
* **Milestone 1:** local STDIO MCP (`moraine mcp`) + Codex docs; CI covers `moraine-mcp`; no decision tools
* **M2:** local integration service + rebuildable project index foundation
* **M3:** minimal trustworthy evidence capture
* **M4:** checkpoint findings with MCP list/get/respond and desktop thread
* **M4.5:** React + TypeScript + Vite desktop migration (Svelte removed)
* **M4.6:** append-only ledger semantics (observations, amendments, supersessions, redactions)
* GUI: comments, suggestions, host Save; run-level decision controls removed from desktop IPC
* Optional in-memory live relay (secondary)

## Done (recent consolidation)

* **C1 — Seal redaction and trust projections: complete**
  * Trust claim: target-scoped ordinary-view withholding across core, desktop, discovery, service, and MCP projections
  * Explicit exclusions: secure erasure, global duplicate-text scrubbing, evidence-artifact deletion, Git-history removal, credential remediation
  * Docs: [SECURITY.md](./SECURITY.md), [docs/REDACTION.md](./docs/REDACTION.md)

## Now

* **C2 — Stranger-safe Linux installation and first reference-integration pack** (Codex is the current concrete adapter) — *in progress on `feat/stranger-safe-install-reference-pack`*
* Keep CI green

## Next (bounded milestones)

1. **C3 — Beta hardening and product-surface freeze**
2. Second agent / broader packaging only after C2–C3

Evaluation artifacts: [docs/evaluation/2026-07/](./docs/evaluation/2026-07/) (historical baseline `4f8d1e8`; PR #12 was open at that freeze).

## Explicit non-goals for the near term

Approval/rejection as product center, merge gates, remote MCP, hosted multi-user service, full observability, agent orchestration, live-collaboration hardening, compliance features, enterprise policy enforcement, general knowledge-management workspace, Git/PR replacement.

## Compatibility note

`moraine decide` remains readable/writable via **CLI only** (legacy warning). Historical sidecar `decisions[]` remain loadable. Do not extend decision functionality. Prefer comments, findings, and human notes. Do not expose decisions through MCP or desktop IPC.
