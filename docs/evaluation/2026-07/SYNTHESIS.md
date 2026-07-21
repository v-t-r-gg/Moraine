# Final synthesis

## Answers to required questions

1. **What is Moraine today?**  
   A local-first coding-agent **run ledger** (Markdown + sidecar) with MCP/CLI protocol, a Linux-oriented local capture service, Codex hooks, findings/append-only review, and a React/Tauri discovery workspace — plus secondary collab editing heritage.

2. **Is vision coherent?**  
   **Yes.** “Records review activity; does not render the verdict” still holds.

3. **Does architecture support vision?**  
   **Mostly yes** for the ledger core. Surfaces and transport choices dilute focus.

4. **Trustworthy & maintainable?**  
   Core integrity is serious. Maintainability threatened by subsystem count. One critical redaction gap on agent APIs.

5. **Obsolete/conflicting subsystems?**  
   Freeze collab expansion; deprecate decisions; dual review mechanisms (annotations vs findings) should not both grow.

6. **What blocks external beta?**  
   Install/packaging, single-integration dogfood package, complete redaction story, honest docs, cold-start path.

7. **Continue / consolidate / reframe?**  
   **Consolidate.** Do not reframe the product; do not advance M6 packaging/second-agent as if M5 closed all trust gaps.

## Recommended conclusion

### **Consolidate**

Smallest credible path to external beta:

1. Land redaction-complete ordinary projections (PR #12 class).
2. Ship **one** install path that always provides current CLI + service + MCP + desktop build instructions.
3. Publish a **Codex dogfood pack** (scripted config, expected run shape, troubleshooting).
4. Freeze Yjs/share/annotation expansion.
5. Update ROADMAP to post-M5 reality; define beta exit criteria narrowly.
6. Only then consider second agent or polish packaging formats.

## Twenty synthesis prompts (condensed)

| # | Answer |
|---|--------|
| 1 What is it? | Local coding-agent run ledger |
| 2 Vision OK? | Yes |
| 3 Architecture OK? | Core yes; edges heavy |
| 4 Trust? | Mostly; fix finding redaction |
| 5 Capture OK? | Design yes; ops no |
| 6 Desktop role? | Inspection workspace — keep |
| 7 Service role? | Capture runtime — keep, simplify |
| 8 Index? | Noncanonical — keep saying so |
| 9 Markdown? | Projection — keep |
| 10 Second agent next? | **No** — after consolidate |
| 11 Yjs/share valuable strategically? | Freeze; not beta-critical |
| 12 Remove before beta? | Nothing large; freeze many |
| 13 Install blocker? | No packages; binary drift |
| 14 Smallest public beta? | Linux + Codex + current CLI/service + desktop ledger + redaction sealed |
| 15 Beta vs 1.0? | Beta: one OS + one agent + install docs. 1.0: multi-platform + second agent + migration policy + support docs |
| 16 M6 next invalid? | As “next milestone” yes invalid until consolidate |
| 17 Invalid roadmap assumptions | “Cross-agent” now; M5 still “current” after merge; second agent as immediate next |
| 18 Next three milestones | See RECOMMENDED_ROADMAP.md |
| 19 Wait after beta | Multi-agent, hosted sync, analytics, collab hardening |
| 20 Never become | Approval gate; observability APM; chat replacement; compliance theater without real guarantees |

## Proposed issues (for human triage — not auto-created)

| Title | Severity | Area | Acceptance |
|-------|----------|------|------------|
| Seal agent-facing finding redaction (land PR #12) | Critical | findings/MCP | list/get/respond JSON never contains redacted claim text |
| Pin install path / prevent stale CLI | High | release | Documented install produces `project`/`run`/`mcp` |
| Codex stranger dogfood pack | High | integrations | New machine reproduces capture→desktop discover |
| Replace desktop curl probe with native client | Medium | desktop/service | No curl dependency for discovery |
| Freeze collab feature expansion | Medium | product | Explicit freeze note in ROADMAP |
| Add SECURITY.md + threat model (local trusted user) | Medium | docs | Threat model matches claims |
| Refresh ROADMAP post-M5 | Low | docs | M5 complete; next = consolidate |
| Investigate version-history (existing #4) | Low | history | Keep freeze until prioritized |
