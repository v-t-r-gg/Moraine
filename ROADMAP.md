# Roadmap

High-level direction only. Details live in [VISION.md](./VISION.md) and [ARCHITECTURE.md](./ARCHITECTURE.md).

## Now (foundation through agent run protocol)

* Run records as Markdown + `*.md.moraine.json` ledger (schema through v4)
* Stable run ID + SHA-256 content hash + append-only human decisions
* Operation-based annotation mutations; durable suggestion dispositions
* Agent run protocol: `project init`, `run start|checkpoint|show|ready|resume|open`
* Decisions only against saved Markdown; revision preconditions; read-only status
* Per-document ledger lock + safe atomic replace; deterministic legacy migration
* CLI: protocol + `share`, `status --json`, `init`, `decide`, file helpers
* GUI: comments, suggestions, run-level review panel, host Save
* Optional in-memory live relay

## Next

* Dogfood agent protocol on real development runs
* Stronger annotation rehydration / anchors (follow-on issues)
* Version-history UX (issue #4)
* MCP transport over the same core operations (not yet implemented)
* Keep CI green

## Later (not scheduled)

* Evidence capture / attachment helpers (beyond agent-reported pointers)
* Authenticated reviewer identity (not just labels)
* Optional Git integrations beyond mechanical context capture
* Authenticated collaboration
* Multi-run review inbox

## Explicit non-goals for the near term

General knowledge-management workspace, compliance-grade audit product, production multi-tenant hosting, Git/PR replacement.
