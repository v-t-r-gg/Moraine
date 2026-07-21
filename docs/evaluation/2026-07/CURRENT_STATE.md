# Current state of Moraine

**Baseline commit:** `4f8d1e85011d8ea49d02ea537c45b29b579ce52b` (`origin/main`, PR #11 merge)  
**Evaluation branch:** `audit/project-evaluation-2026-07`  
**Date:** 2026-07-21

## One sentence

Moraine is a **local-first ledger for coding-agent runs**: durable project-local Markdown + JSON sidecars, semantic CLI/MCP protocol, a Linux-oriented capture service (hooks + spool + rebuildable index), and a React/Tauri human workspace for discovery and append-only review — **not** an approval gate, hosted SaaS, or multi-agent observability platform.

## Product invariant (still holds)

> Moraine records review activity; it does not render the verdict.

## What works in the repository today

| Layer | State |
|-------|--------|
| Agent run protocol | Implemented, tested, dogfoodable via `./target/debug/moraine` |
| Schema | Sidecar `SCHEMA_VERSION = 6` (append-only ops); load promotes older; rejects future |
| Findings | Create/list/get/respond/state; MCP + desktop |
| Append-only ops | Observation, amend, supersede, redact |
| Evidence | Provenance-aware capture model (M3) |
| Local service | Unix socket intake, spool, provisional runs, discovery HTTP |
| Desktop | Default Projects → Runs → Ledger workspace (M5) |
| Redaction (ordinary timeline/UI) | Present on main |
| Redaction (finding DTOs / MCP) | **Not on main** — open PR #12 |

## What is not true yet (honest)

- “Cross-agent” — only **Codex** integration is real.
- “Install with `cargo install` and go” — published/stale binary may lack protocol subcommands.
- “External beta ready” — packaging, cold install, full agent live path not sealed.
- “Immutable audit trail” — owner can edit plain files; redaction is ordinary-reader policy.

## Related open work (not baseline)

| PR | State | Relevance |
|----|-------|-----------|
| #11 M5 discovery UX | Merged | Baseline |
| #12 Finding redaction projection | Open | Critical integrity gap on main |
| Issue #4 version history | Open | Freeze-tier |

## Starting-point documents

- Inventory: [CAPABILITY_MATRIX.md](./CAPABILITY_MATRIX.md), [CAPABILITY_INVENTORY.md](./CAPABILITY_INVENTORY.md)
- Trust: [INTEGRITY_AND_SECURITY_REVIEW.md](./INTEGRITY_AND_SECURITY_REVIEW.md), [RISK_REGISTER.md](./RISK_REGISTER.md)
- Path forward: [RECOMMENDED_ROADMAP.md](./RECOMMENDED_ROADMAP.md)
