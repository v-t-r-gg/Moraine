# Scorecard

Scores are 1–5 for baseline `4f8d1e8`. 5 = beta-ready.

| Area | Score | Comment |
|------|-------|---------|
| Vision coherence | 4 | Clear invariant; scope creep in surfaces |
| Architecture fit | 3.5 | Core ledger good; surface overload |
| Integrity / trust | 3 | Strong locks/idempotency; **finding redaction leak** |
| Capture reliability story | 2.5 | Designed well; one integration; not stranger-proven |
| Desktop usefulness | 3.5 | M5 workspace real; still heavy |
| CLI/MCP usefulness | 3.5 | Strong in-tree; install drift |
| Documentation | 3 | Protocol docs good; install/security weak |
| Onboarding | 1.5 | Major beta blocker |
| Packaging | 1 | No releases |
| Multi-agent claim honesty | 2 | Codex-only in practice |
| Maintainability | 3 | Large for solo; too many subsystems |
| Market differentiation | 4 | Clear niche if delivered |
| External beta readiness | **2** | Integrity fix + install path required |

## Highest-severity findings (ranked)

1. Agent-facing finding redaction bypass on main (PR #12 open)
2. No stranger-safe install/packaging; stale CLI binary drift
3. Capture path not live-validated for external beta marketing
4. Product surface sprawl (collab/share/annotations) vs ledger focus
5. ROADMAP still says M5 “Now” after merge
6. Missing SECURITY.md / CONTRIBUTING / release notes discipline
