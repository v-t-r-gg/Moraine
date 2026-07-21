# Risk register

**Baseline:** `4f8d1e8`  
Severity: **P0** trust/data loss · **P1** core claim false/unreliable · **P2** significant usability/scope · **P3** polish

| ID | Title | Sev | Classes | Evidence | Components | Consequence | Recommendation | Beta? | 1.0? | Conf. |
|----|-------|-----|---------|----------|------------|-------------|----------------|-------|------|-------|
| R1 | Finding DTOs leak redacted checkpoint content | **P0** | integrity, security, correctness | Code path `target_context` clones `checkpoint.summary`; MCP serializes get/list/respond; PR #12 unmerged | core findings, MCP, desktop findings | Agents recover “redacted” secrets | Land PR #12; single projection | **Yes** | Yes | High |
| R2 | Stale installed CLI lacks protocol commands | **P1** | packaging, UX, product | `~/.cargo/bin/moraine --help` lacked project/run during eval; workspace binary correct | cli, docs, release | Users believe product broken | Document install from current tree/releases; version stamp | **Yes** | Yes | High |
| R3 | Automatic capture not stranger-validated | **P1** | product, UX, documentation | No clean Codex live Scenario 1 in eval | service, hooks, integrations | False marketing of “just works” | Codex dogfood pack + Scenario 1 evidence | **Yes** | Yes | Medium |
| R4 | Desktop discovery depends on curl subprocess | **P2** | architecture, packaging, portability | `src-tauri/.../discovery.rs` spawns curl | tauri, service | Fails without curl; odd attack surface | Native HTTP client | No | Prefer yes | High |
| R5 | Product surface sprawl (Yjs/share/annotations) | **P2** | architecture, product, maintainability | Multiple subsystems + tests vs ledger focus | desktop, server, annotations | Dilutes beta narrative | Freeze expansion | No | No | High |
| R6 | ROADMAP claims M5 “Now” after merge | **P3** | documentation | `ROADMAP.md` vs main history | docs | Confuses prioritization | Update post-eval | No | No | High |
| R7 | Missing SECURITY.md / threat model | **P2** | security, documentation | Repo root lacks SECURITY.md | docs | Overstated trust by readers | Write local-trust model | **Yes** | Yes | High |
| R8 | Service index misread as canonical | **P2** | integrity, product, UX | Index is cache; UX must stay honest | service, desktop | Wrong recovery expectations | Keep messaging; tests | No | No | Medium |
| R9 | Cross-platform claims unsupported | **P2** | compatibility, documentation | Only Linux live-tested | all | Broken installs | Explicit Linux beta | **Yes** | Yes | High |
| R10 | Dual review systems (annotations + findings) | **P3** | architecture, UX | Both implemented | desktop, core | Cognitive load | Freeze annotations | No | Prefer | Medium |
| R11 | External owner can edit MD | **P3** | integrity, product | Filesystem reality | persistence | Not a bug if claimed honestly | Never claim cryptographic immutability | No | No | High |
| R12 | Scale untested at 10k+ runs | **P3** | performance | Scale test ~1k OK | service, desktop | Future slowness | Defer | No | Maybe | Medium |
| R13 | Version history open issue #4 | **P3** | correctness, UX | GitHub issue #4 | history | Edge-case confusion | Freeze history expansion | No | No | Medium |
| R14 | Incomplete live Scenario 2/5 | **P2** | documentation, product | LIVE_SCENARIOS.md | eval process | Unknown recovery UX | Complete before beta branding | **Yes** | Yes | Medium |

## Immediate blockers summary

- **Beta P0/P1:** R1, R2, R3, R7, R9, R14  
- **Not blockers but freeze:** R5, R10, R13
