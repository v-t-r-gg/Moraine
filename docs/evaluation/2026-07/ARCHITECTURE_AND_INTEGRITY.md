# Pass B — Architecture and integrity skeptic

**Goal:** Attempt to invalidate trust assumptions.  
**Baseline:** `4f8d1e8`

## Immediate blockers (severity)

| ID | Finding | Severity | Status on main |
|----|---------|----------|----------------|
| B1 | **Finding DTOs leak redacted checkpoint content** via `checkpoint_summary` / full `target_snapshot` to list/get/respond (MCP + desktop). Ordinary timeline fixed; agent path not. | **Critical** | Open PR #12 |
| B2 | **Stale published CLI** (`~/.cargo/bin/moraine`) lacks protocol commands while version still `0.1.0` — users can “install” a useless binary. | High | Packaging gap |
| B3 | Desktop discovery probes service via **`curl` subprocess** to loopback HTTP — fragile, odd for production desktop, fails if curl missing. | Medium | Design smell |
| B4 | Service index can become **operationally treated as truth** by UX copy if offline semantics are weak; core still correct but rescan=full rebuild. | Medium | Documented noncanonical |
| B5 | **Yjs/share/history/annotations** expand surface area without being required for ledger beta — integrity risk is lower than maintenance risk. | Medium | Product bloat |
| B6 | External file edits of plain Markdown still possible; product correctly does not claim protection — must not market “immutable audit trail.” | Info | Expected |

## Architectural strengths

1. **Canonical data in project-local bundles** with explicit noncanonical service index (good).
2. **Core-owned domain operations** for protocol, findings, append-only ops, discovery summaries (mostly).
3. **Per-record sidecar lock** + incomplete-op recovery model (serious integrity design).
4. **Idempotency keys** on protocol ops and finding responses.
5. **Spool `seen/` markers** for durable event dedupe across service restarts (tested).
6. **Unsupported schema rejection** without silent rewrite.
7. **Discovery nonmutation tests** (hash/byte equality after list/rebuild).
8. **No approval authority** in MCP/desktop (product invariant holds in code paths reviewed).

## Domain boundary issues

| Area | Issue |
|------|--------|
| Classification rules | Largely in core discovery; service rebuild uses core `summarize_project` — good. Desktop filters partly client-side over summaries — OK at current scale. |
| Redaction | Multiple layers (timeline, ProtocolLedgerPanel, checkpoints DTO, findings) — **must be one projection**; PR #12 starts this for findings. |
| Markdown authority | Rendered projection; human free-form only legacy — residual “Human notes” language still confuses. |
| Dual review systems | Annotations (selection-based) + findings (checkpoint-based) + observations — cognitive + code cost. |

## Persistence / recovery

- **Incomplete ops:** intended recovery on next op; tests cover finding survival across recovery.
- **Spool corruption:** failed/quarantine paths exist; oversized events capped (`MAX_EVENT_BYTES`).
- **Concurrent capture + discovery:** designed concurrent; not load-tested at multi-agent production scale.
- **Project confinement:** resolve_existing_project / confined paths — need ongoing audit; not fully re-proven in this evaluation pass.

## Transport / security model (local trusted user)

Assumptions:

- Single-user machine trust
- Unix socket permissions
- Loopback-only diagnostics HTTP (enforced non-loopback refuse)

Gaps:

- No cryptographic agent identity
- No authentication (by design for local)
- Redaction is **presentation policy**, not cryptographic erasure — sidecar may retain prior content; ordinary readers must not receive it (**B1**)
- Shelling to `curl` for status is unnecessary attack/fragility surface (local)

## Concurrency / ordering

- Timeline uses deterministic secondary sort keys in discovery — good.
- Filesystem iteration sorted in list_run_summaries — good.
- Event ordering for hooks depends on spool processing; mechanical vs semantic reconciliation complexity is real.

## Duplication of rules

| Rule | Locations |
|------|-----------|
| Project scan | core `scan_project_roots` + historical service code (migrating to core) |
| Run summary | core only for M5 |
| Redaction display | core timeline, ProtocolLedgerPanel, checkpoints DTO, **findings still raw on main** |
| Capture coverage strings | core enum + UI filter options |

## Verdict on architecture

The **ledger core is coherent and mostly trustworthy**. The **product surface is overloaded** (collab editor + share + history + annotations + full protocol + service + discovery). Integrity risk is highest where **presentation layers re-implement or bypass core redaction projections** (B1).

**Trustworthy enough to consolidate, not to market as multi-agent audit product.**
