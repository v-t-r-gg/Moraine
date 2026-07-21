# Recommended roadmap (post-evaluation)

**Supersedes assumptions in root `ROADMAP.md` “Now” section for planning purposes.**  
Product invariant unchanged: Moraine records review activity; it does not render the verdict.

## Keep

- Agent-run protocol (CLI + MCP)
- Project-local Markdown + sidecar ledger
- Local service for capture + spool + rebuildable index
- Evidence with explicit provenance
- Findings + append-only observations/amend/supersede/redact
- Ledger-first React desktop discovery
- Core-owned discovery & timeline read models
- Noncanonical index doctrine

## Consolidate

- **Redaction projections** (single path for all ordinary readers including findings/MCP)
- **Install/versioning** (one current CLI/service/desktop path)
- **Codex integration packaging** (scripts + docs + expected failure modes)
- Desktop discovery transport (prefer native HTTP/Unix client over curl)
- Documentation honesty (cross-agent claims, offline, redaction retention model)
- ROADMAP/blueprint status flags vs reality

## Freeze

- Yjs / live share / moraine-server investment
- Annotation feature growth
- Tiptap capability expansion for protocol runs
- Legacy `decide` (read/compat only)
- Additional discovery categories / search features
- Performance virtualization until measured need

## Deprecate or remove (scheduled, not emergency)

- Residual “editable Human notes as durable protocol path” language and UX
- Long-term: dual annotation+finding growth → prefer findings/observations for protocol runs
- Any approval-shaped UI if it reappears

## Defer (after beta)

- Second agent integration
- Multi-platform installers
- Hosted sync / teams
- Vector/full-text search
- Advanced analytics / token dashboards
- Cryptographic signing / auth

---

## Next three milestones

### Milestone C1 — Seal trust projections & honesty

**Objective:** Ordinary readers (desktop + MCP + CLI JSON) never receive redacted checkpoint claim content; docs match reality.  
**User problem:** Users/agents must not bypass redaction.  
**Scope:** Land finding projection fix; audit remaining DTO leaks; document retention model; update ROADMAP status.  
**Non-goals:** New ops, new agents, packaging formats.  
**Acceptance:** Automated tests prove list/get/respond JSON omit secrets; desktop RTL remains green; docs state sidecar may retain.  
**Beta impact:** Required.  
**Arch risk:** Low.

### Milestone C2 — Stranger-safe install + Codex dogfood pack

**Objective:** A developer outside the repo can install current tools, start service, configure Codex, produce a discoverable run.  
**User problem:** Today only in-tree experts succeed.  
**Scope:** Install docs; binary versioning story; service start/stop; Codex config pack; troubleshooting for offline/service down; fix stale CLI drift.  
**Non-goals:** Second agent; Windows/macOS polish; collab.  
**Acceptance:** Documented steps work on a clean Linux machine; evaluation Scenario 1 completes with screenshots.  
**Beta impact:** Required.  
**Arch risk:** Medium (packaging choices).

### Milestone C3 — Beta hardening (capture reliability + desktop thinness)

**Objective:** Capture survives service restart without duplication; desktop remains usable offline; freeze non-ledger surfaces.  
**User problem:** Trust that reopening tomorrow works.  
**Scope:** Live Scenario 2/4 automation where missing; offline UX polish; freeze notes in code/docs; optional curl removal.  
**Non-goals:** New features.  
**Acceptance:** Documented recovery; nonmutation; no duplicate spool processing in live test; beta checklist signed.  
**Beta impact:** Required for calling it beta.  
**Arch risk:** Medium.

---

## Beta exit criteria (smallest credible external beta)

- Linux x86_64 primary
- One supported agent: **Codex**
- Install path produces working CLI (`project`/`run`/`mcp`), service, and desktop build or binary
- Capture works with desktop closed
- Desktop discovers projects/runs without path
- Redaction sealed for ordinary UI + MCP findings
- Append-only review (observation/finding) works end-to-end
- Explicit non-claims: not multi-agent, not hosted, not compliance-certified, not immutable against filesystem owner
- SECURITY.md local-trust model
- Known limitations section

## 1.0 exit criteria (direction)

- ≥2 agent integrations with equivalent run bundles
- Documented platforms (at least Linux + one other or explicit single-platform 1.0)
- Schema migration policy for N→N+1; unsupported future reject
- Service install/uninstall/status stable
- Recovery guarantees for incomplete ops + spool
- Compatibility policy for legacy decisions/annotations
- Test matrix: core integrity + service + MCP + desktop acceptance
- Trust claims only for properties proven (no “audit-grade” without evidence)

---

## Explicit non-roadmap

Moraine should **never** become:

- a merge/approval gate
- a multi-tenant hosted compliance product by default
- a full APM/observability suite
- a chat transcript warehouse
- an agent orchestrator
