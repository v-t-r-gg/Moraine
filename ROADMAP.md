# Roadmap

High-level direction. Product model: [VISION.md](./VISION.md), [ARCHITECTURE.md](./ARCHITECTURE.md), [docs/DEVELOPMENT_BLUEPRINT.md](./docs/DEVELOPMENT_BLUEPRINT.md).

**Invariant:** Moraine records review activity; it does not render the verdict.

## Done (foundation)

* Run records as Markdown + `*.md.moraine.json` ledger (schema through v4)
* Stable run ID + SHA-256 content hash; historical decisions preserved for compatibility
* Operation-based annotation mutations; durable suggestion dispositions
* Agent run protocol: `project init`, `run start|checkpoint|show|ready|resume|open`
* Per-document ledger lock + safe atomic replace; deterministic legacy migration
* CLI: protocol + `share`, `status --json`, `init`, file helpers
* Local STDIO MCP (`moraine mcp`) with Codex integration docs
* GUI: comments, suggestions, host Save; run-level decision controls de-centered
* Optional in-memory live relay (secondary)

## Now

* Vision realignment and decision de-centering (Milestone 0)
* Finish MCP / Codex dogfooding (Milestone 1 acceptance)
* Keep CI green

## Next (bounded milestones)

1. **Minimal evidence capture** — trusted command/test capture with clear provenance
2. **Findings and amendment loop** — human findings ↔ agent responses without verdicts
3. **Local run discovery and ledger UX** — project/run list from sidecars; no approval inbox
4. **Second agent integration, packaging, external beta**

## Explicit non-goals for the near term

Approval/rejection as product center, merge gates, remote MCP, hosted multi-user service, full observability, agent orchestration, live-collaboration hardening, compliance features, enterprise policy enforcement, general knowledge-management workspace, Git/PR replacement.

## Compatibility note

`moraine decide` and historical sidecar `decisions[]` remain readable and writable for compatibility. Do not extend decision functionality. Prefer comments, findings, and human notes. Do not expose decisions through MCP or future agent transports.
