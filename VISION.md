# Vision

## Product invariant

> Moraine records review activity; it does not render the verdict.

Moraine is a **local-first ledger for coding-agent work**.

It preserves what an agent did, why, what evidence exists, what remains open, and what humans observed—as **files on disk** next to the work. It does **not** decide whether work is accepted, rejected, mergeable, deployable, or authorized.

## The problem

Autonomous agents perform real work. That work often leaves chat transcripts, tool logs, or nothing durable. Humans need a **reviewable record** they can open later, put next to the code, comment on, and challenge—without a vendor session viewer.

**Review agent work without relying on agent chat.**

## What Moraine is

* **Semantic run protocol** — sparse checkpoints, lifecycle, findings, append-only human observations  
* **Mechanical capture** — supported agent hooks (Codex first) even when the model skips MCP  
* **Discovery desktop** — projects → runs → structured ledger  
* **Installable Linux suite** — CLI, service, MCP/hooks, desktop, doctor (C2)  

Collaborative live editing is **secondary** and frozen for beta (C3). Moraine is **not** an approval system.

## Agent run

An **agent run** is a bounded unit of work. During or after it, Moraine holds a **run bundle**:

* Markdown projection (human-readable)  
* Structured sidecar `*.md.moraine.json`  
* Checkpoints, evidence references, findings, append-only ops  
* Capture coverage (honest: mechanical vs semantic)  

Agents use **CLI**, **local MCP**, or hooks. Humans use the **installed desktop ledger workspace** (or CLI doctor/open).

## Human review (without verdict)

* Launch ledger workspace; discover projects/runs without knowing paths  
* Inspect timeline, findings, evidence, capture coverage  
* Append observations; create/respond to findings  
* **Legacy document mode** only for free-form non-protocol Markdown (secondary)  

Review may happen **live** (capture while desktop closed) or **hindsight** (open files later).

## Durable artifacts

| Artifact | Role |
|----------|------|
| `.moraine/runs/*.md` + `*.moraine.json` | Canonical run bundles |
| `.moraine/project.json` | Project identity |
| Session / spool under user cache | Mechanical capture fallback |
| Installed suite under `~/.local` | Product binaries + manifest |

## Evidence and trust

* Agent text can be wrong or incomplete.  
* Evidence carries provenance.  
* Redaction withholds target-scoped claims in ordinary views (C1).  
* No authenticated identity or compliance-grade tamper-proof audit.  

## Current scope (beta)

Implemented:

* Agent run protocol + MCP (including findings tools)  
* Mechanical Codex hooks + spool  
* Discovery desktop + offline direct inspection  
* Stranger-safe Linux install (C2 candidate → on main)  
* Append-only correction and target-scoped redaction  

C3 focus: **surface freeze**, CSP, service hardening, lifecycle validation, App decomposition.

## Explicit non-goals (near term)

Approval as product center, merge gates, remote MCP, hosted multi-tenant collab, second agent before Windows portfolio (W1–W3), semantic/vector search, relay auth, `moraine-core::prelude` churn.

## Direction sequence

```text
C3  Beta hardening and surface freeze
W1  Platform abstraction
W2  Native Windows 11 port
W3  Signed installer and WinGet
```

Canonical blueprint: [docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md](./docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md).
