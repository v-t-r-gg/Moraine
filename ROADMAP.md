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

## Now

* **M5:** local run discovery and ledger-focused desktop UX (projects → runs → structured ledger)
* Keep CI green
* Dogfood discovery workspace on multi-run projects

## Next (bounded milestones)

1. **Second agent integration, packaging, external beta**

## Explicit non-goals for the near term

Approval/rejection as product center, merge gates, remote MCP, hosted multi-user service, full observability, agent orchestration, live-collaboration hardening, compliance features, enterprise policy enforcement, general knowledge-management workspace, Git/PR replacement.

## Compatibility note

`moraine decide` remains readable/writable via **CLI only** (legacy warning). Historical sidecar `decisions[]` remain loadable. Do not extend decision functionality. Prefer comments, findings, and human notes. Do not expose decisions through MCP or desktop IPC.
