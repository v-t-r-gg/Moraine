# Pass C — Product and market review

**Baseline:** `4f8d1e8`  
**Product statements evaluated:**

> Moraine is a local-first ledger for autonomous agent work.  
> Near-term: local-first, cross-agent ledger for coding-agent work.

## Problem clarity

| Question | Assessment |
|----------|------------|
| Immediately understandable? | **Partially.** “Review agent work without the chat transcript” is clear; stack (service+MCP+hooks+sidecar) is not. |
| Value beyond agent chat? | **Yes, if capture is automatic and records are sparse/semantic.** Today automatic capture is Codex-path only and setup-heavy. |
| Value beyond Git/PR? | **Yes in theory** — PR shows diffs, not sparse claims/risks/evidence/findings over a *bounded agent run*. **Weak in practice** until install+capture are trivial. |
| Avoids full observability competition? | **Intentionally yes** — not traces/tokens/latency dashboards. Market is flooding with agent observability; Moraine must stay **human review ledger**, not APM. |
| Run bundle coherent? | **Yes** as product object (MD + sidecar). |
| Markdown meaningful? | **Useful projection** for hindsight/Git; **not** the write path for protocol claims. Risk if users edit MD expecting durability of free-form notes. |
| Append-only ceremony? | **Correct for integrity**; can feel heavy. Needs excellent UI (original → amendment → current). |
| Local service justified? | **Yes for capture-without-desktop** and spool. **Cost:** install/debug burden. |
| Desktop role clear? | **Improving (M5 workspace)** — still ships a full collab editor heritage. |
| CLI role clear? | **Agents/scripts yes; humans secondary.** Stale cargo install breaks this. |
| “All autonomous agents” too broad? | **Yes.** Only Codex integration is real. Cross-agent is **aspiration**, not product fact. |
| Second agent before beta? | **No as first consolidation step.** Better: one solid install path + redaction fix + dogfood pack. Second agent is **beta expander**, not foundation fix. |
| Strongest user group | Solo/power-user developers using **one** coding agent who need **hindsight review** of agent claims next to the repo. |
| Distractions | Live Yjs collab, share rooms, rich annotations, generic Markdown editing, legacy decide, multi-surface discovery probes. |

## Market context (primary sources, 2026)

Cited for orientation; **not independent customer validation.** Web access was available for this pass.

1. **MCP observability** as enterprise monitoring of tool calls, policy, and identity — different from local run ledgers:  
   https://obot.ai/blog/mcp-observability-how-to-monitor-ai-agent-activity-in-the-enterprise/ (Obot, 2026).

2. **Coding-agent audit trails** via gateway capture for compliance:  
   https://www.mintmcp.com/blog/build-audit-trails-ai-coding-agents (MintMCP, 2026).

3. **Agent observability tools** (Braintrust, LangSmith, Arize, Datadog, etc.) compete on traces/evals:  
   https://www.augmentcode.com/tools/best-ai-agent-observability-tools (Augment Code, 2026).

4. **Claude Code-class agents** use append-only JSONL session transcripts (design-space paper):  
   https://arxiv.org/html/2604.14228v1 (2026).

5. **OpenHands / coding agents** push work into PR/commit review surfaces:  
   https://theaiengineer.substack.com/p/the-open-source-agent-toolkit-in (2026).

Market implication: Moraine should not pitch as APM; pitch **source-adjacent semantic review ledger after the agent leaves.**

### Positioning implication

Moraine should **not** pitch against Datadog/LangSmith. Pitch:

> After the agent leaves, open a **source-adjacent run record** that states what it claims it did, with evidence, risks, and human challenges — without replaying the chat.

If capture is unreliable or install is hard, the pitch collapses to “another JSON sidecar.”

## Product coherence score

| Dimension | Score /5 | Note |
|-----------|----------|------|
| Problem importance | 4 | Real pain for agent users |
| Solution fit | 3 | Right object (run ledger); heavy path |
| Scope discipline | 2 | Too many surfaces for team size |
| Differentiation | 4 | Local files + human review + no verdict |
| GTM readiness | 1 | No packaging, one integration, incomplete redaction story on main |

## Recommendation (product)

**Consolidate**, do not expand integrations until:

1. Redaction is airtight on ordinary + agent APIs (merge-ready PR #12).
2. Install path is one documented path with current binaries.
3. Codex happy path is reproducible by a stranger.
4. Desktop defaults stay ledger-first; freeze collab expansion.
