# Roadmap

High-level direction only. Details live in [VISION.md](./VISION.md) and [ARCHITECTURE.md](./ARCHITECTURE.md).

## Now (v0.1 / v0.2 foundation + v0.2.1 correctness)

* Run records as Markdown + `*.md.moraine.json` ledger
* Stable run ID + SHA-256 content hash + append-only decisions
* Decisions only against saved Markdown; revision preconditions; read-only status
* Per-document ledger lock + safe atomic replace; deterministic legacy migration
* CLI: `share`, `status --json`, `init`, `decide`, file helpers
* GUI: comments, suggestions, run-level review panel, host Save
* Optional in-memory live relay
* Docs positioned as a review ledger for agent work

## Next (polish hindsight review)

* Stronger annotation durability and rehydration
* Clearer run-record conventions for agents (templates, examples)
* Evidence pointers / capture helpers (still optional)
* Keep CI green

## Later (not scheduled)

* Evidence capture / attachment helpers
* Authenticated reviewer identity (not just labels)
* Optional Git integrations (still user-controlled)
* Authenticated collaboration
* Multi-run review inbox

## Explicit non-goals for the near term

General knowledge-management workspace, compliance-grade audit product, production multi-tenant hosting, Git/PR replacement.
