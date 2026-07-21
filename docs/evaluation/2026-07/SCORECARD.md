# Scorecard (0–5)

**Baseline:** `4f8d1e8`  
Each score cites concrete evidence. 5 = beta-ready quality.

| Area | Score | Evidence |
|------|------:|----------|
| **Product clarity** | 3 | VISION/README state ledger + no verdict clearly; residual Human notes/Save language and “cross-agent” aspiration confuse ([README](../../../README.md), [UX_AND_ONBOARDING_REVIEW.md](./UX_AND_ONBOARDING_REVIEW.md)). |
| **Workflow value** | 3 | Run bundle + findings/append-only address real post-chat review; value unrealized without easy capture ([PRODUCT_AND_MARKET_EVALUATION.md](./PRODUCT_AND_MARKET_EVALUATION.md)). |
| **Market differentiation** | 4 | Distinct from APM/trace tools and chat transcripts: source-adjacent semantic ledger ([PRODUCT_AND_MARKET…](./PRODUCT_AND_MARKET_EVALUATION.md) market section). |
| **Domain coherence** | 4 | Project/run/checkpoint/finding/append-only model consistent in core ([DOMAIN_MODEL_REVIEW.md](./DOMAIN_MODEL_REVIEW.md)). Dual notes/annotations residual. |
| **Architecture** | 3 | Core boundaries good; service justified; curl probe + surface sprawl ([ARCHITECTURE_REVIEW.md](./ARCHITECTURE_REVIEW.md)). |
| **Integrity** | 3 | Locks, idempotency, spool seen, nonmutation tests strong; **finding redaction leak P0** ([RISK_REGISTER.md](./RISK_REGISTER.md) R1). |
| **Trust** | 2 | Provenance model exists; ordinary redaction incomplete on agent APIs; no SECURITY.md (R1, R7). |
| **Automatic capture** | 2 | Designed + unit/integration tests; not live stranger-proven (R3, LIVE_SCENARIOS). |
| **Desktop UX** | 3 | M5 workspace real ([Workspace.tsx](../../../src/app/Workspace.tsx), tests); collab weight remains. |
| **CLI/MCP UX** | 3 | In-tree protocol/MCP coherent; install binary drift (R2). |
| **Installation** | 1 | No packages; multi-step dev install; stale CLI ([RELEASE_READINESS.md](./RELEASE_READINESS.md)). |
| **Portability** | 2 | Linux de facto; macOS/Windows unclaimed ([COMPATIBILITY…](./COMPATIBILITY_AND_PORTABILITY.md)). |
| **Documentation** | 3 | Protocol/MCP/Codex docs solid; install/security/roadmap freshness weak. |
| **Maintainability** | 3 | ~28k LOC core-area density; many subsystems; good tests but high surface ([SUBSYSTEM_RETENTION.md](./SUBSYSTEM_RETENTION.md)). |
| **Portfolio readiness** | 2 | Demo-able from checkout; not downloadable story. |
| **Beta readiness** | 2 | Blocked by R1–R3, R7, R9 ([RISK_REGISTER.md](./RISK_REGISTER.md)). |
| **1.0 readiness** | 1 | Needs multi-agent or honest single-agent 1.0, packaging, migration policy. |

## Reading the scores

Do **not** average into a single vanity metric.  
Lowest scores (**installation, trust, capture, beta/1.0**) define the consolidation program (C1–C3 in [RECOMMENDED_ROADMAP.md](./RECOMMENDED_ROADMAP.md)).
