# Roadmap

High-level direction. Product model: [VISION.md](./VISION.md), [ARCHITECTURE.md](./ARCHITECTURE.md), [docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md](./docs/DEVELOPMENT_BLUEPRINT_ALIGNED.md).

**Invariant:** Moraine records review activity; it does not render the verdict.

## Done (foundation)

* Run records as Markdown + `*.md.moraine.json` (schema through v6)  
* Agent run protocol + local STDIO MCP (run + findings tools)  
* Local service + rebuildable discovery index  
* Minimal trustworthy evidence on checkpoints  
* Checkpoint findings (MCP + desktop)  
* React desktop migration (Svelte removed from app)  
* Append-only ledger semantics; target-scoped redaction (C1)  
* **C2 — Stranger-safe Linux installation and Codex reference pack** (merged)  

## Now

* **C3 — Beta hardening and product-surface freeze** (this branch)  
  * Transactional provisioning, write-ahead service repair, and exact rollback
  * Desktop ProductCapture onboarding with full `ApplyOutcome` state
  * Durable canonical-root registry feeding rebuildable discovery
  * Self-test capture lifecycle without ordinary-run pollution
  * Complete local/CI-equivalent source gate and Linux package smoke
  * Reconcile product/install/development documentation

### Validation language

* **Implemented:** present in this branch.
* **Tested locally:** covered by hermetic Rust/Tauri/frontend tests and the
  authoritative source gate.
* **Tested in CI:** only after the corresponding PR checks report success.
* **Graphical Linux session:** not claimed by headless tests; a real
  WebKit/systemd user-session lifecycle remains manual acceptance evidence.
* **Planned:** W1–W3 below.
* **Deferred:** the explicitly listed capabilities below.

## Next (ordered)

1. **W1 — Platform abstraction** (IPC/paths beyond Linux assumptions)  
2. **W2 — Native Windows 11 port**  
3. **W3 — Signed installer and WinGet**  
4. Second agent adapter (subordinate to Windows portfolio reach)  

## Deferred (do not expand in C3)

* `moraine-core::prelude` public API reorg  
* Broad evidence expansion  
* Semantic/vector search  
* Relay authentication  
* Richer Git/PR integration  
* Hosted collaboration  

## Explicit non-goals (near term)

Approval/rejection as product center, merge gates, remote MCP, full observability, agent orchestration, live-collaboration hardening for untrusted networks, compliance features, enterprise policy, general KM workspace, Git/PR replacement.

## Compatibility

`moraine decide` remains CLI-only legacy. Prefer findings and append-only observations. Live collab/Yjs is frozen for beta defaults.
