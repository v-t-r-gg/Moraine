# Moraine project evaluation — 2026-07

**Baseline product:** `4f8d1e85011d8ea49d02ea537c45b29b579ce52b` (`origin/main`, PR #11)  
**Branch:** `audit/project-evaluation-2026-07`  
**Conclusion:** **Consolidate** (not Continue, not Reframe)

## Required deliverables (OBJECTIVE §14)

| File | Role |
|------|------|
| [CURRENT_STATE.md](./CURRENT_STATE.md) | What Moraine is today |
| [CAPABILITY_MATRIX.md](./CAPABILITY_MATRIX.md) | I/T/L/D/U matrix |
| [CAPABILITY_INVENTORY.md](./CAPABILITY_INVENTORY.md) | Extended inventory |
| [DOMAIN_MODEL_REVIEW.md](./DOMAIN_MODEL_REVIEW.md) | ~25 concepts |
| [ARCHITECTURE_REVIEW.md](./ARCHITECTURE_REVIEW.md) | Boundaries & service |
| [INTEGRITY_AND_SECURITY_REVIEW.md](./INTEGRITY_AND_SECURITY_REVIEW.md) | Trust |
| [PRODUCT_AND_MARKET_EVALUATION.md](./PRODUCT_AND_MARKET_EVALUATION.md) | Pass C |
| [UX_AND_ONBOARDING_REVIEW.md](./UX_AND_ONBOARDING_REVIEW.md) | Pass D |
| [UX_ONBOARDING_RELEASE.md](./UX_ONBOARDING_RELEASE.md) | Extended UX notes |
| [PERFORMANCE_AND_SCALE.md](./PERFORMANCE_AND_SCALE.md) | Measured scale |
| [COMPATIBILITY_AND_PORTABILITY.md](./COMPATIBILITY_AND_PORTABILITY.md) | Support matrix |
| [RELEASE_READINESS.md](./RELEASE_READINESS.md) | Packaging/beta |
| [SUBSYSTEM_RETENTION.md](./SUBSYSTEM_RETENTION.md) | Keep/freeze/remove |
| [RISK_REGISTER.md](./RISK_REGISTER.md) | P0–P3 with blockers |
| [SCORECARD.md](./SCORECARD.md) | 17 areas 0–5 |
| [RECOMMENDED_ROADMAP.md](./RECOMMENDED_ROADMAP.md) | C1–C3 |
| [SYNTHESIS.md](./SYNTHESIS.md) | Final answers |
| [LIVE_SCENARIOS.md](./LIVE_SCENARIOS.md) | Live honesty |
| [00_BASELINE.md](./00_BASELINE.md) | Starting gate |

## Diagrams

| Diagram | File |
|---------|------|
| Runtime architecture | [diagrams/architecture.md](./diagrams/architecture.md) |
| Domain model | [diagrams/domain_model.md](./diagrams/domain_model.md) |
| Persistence & authority | [diagrams/persistence_authority.md](./diagrams/persistence_authority.md) |
| Session→run reconciliation | [diagrams/session_reconciliation.md](./diagrams/session_reconciliation.md) |
| Near-1.0 user workflow | [diagrams/user_workflow_near_1_0.md](./diagrams/user_workflow_near_1_0.md) |

## One-paragraph summary

Moraine’s **ledger core is sound** and matches the “no verdict” vision. M5 delivered a real discovery workspace. **External beta is blocked** by incomplete agent-facing redaction (PR #12 open), install honesty, and unproven cold Codex capture. Next milestones: **C1 seal redaction → C2 install+Codex pack → C3 beta harden** — not second-agent expansion first.
