# Evaluation baseline

**Date:** 2026-07-21  
**Evaluation branch:** `audit/project-evaluation-2026-07`  
**Product baseline (evaluated):** `4f8d1e85011d8ea49d02ea537c45b29b579ce52b` = `origin/main` = merge of PR #11

## Starting-gate results

| Item | Result |
|------|--------|
| PR #11 M5 discovery UX | **MERGED** into `main` at `4f8d1e8` |
| PR #11 CI (final) | rust/msrv/frontend/tauri-check **SUCCESS** |
| PR #12 finding redaction projection | **OPEN**, not merged (`d0efb03` on `fix/redacted-finding-projection`) |
| Open issues | #4 version-history during editing |
| Tags | `durable-annotations-v0.3`, `review-ledger-v0.2.1` |
| Working tree for evaluation | Branch from clean `main`; evaluation docs only |

## Capability presence on `main` (yes/partial/no)

| Required theme | On `main`? | Notes |
|----------------|------------|--------|
| Decision de-centering | Yes | Legacy `decide` CLI-only; no decision MCP/desktop IPC |
| Agent-run protocol | Yes | CLI + MCP + core |
| Local MCP | Yes | `moraine mcp` STDIO |
| Deterministic session capture | Yes | Codex hooks + service spool |
| Local service | Yes | Unix socket + loopback HTTP |
| Provisional-run reconciliation | Yes | M2 |
| Capture coverage | Yes | On agent state |
| Evidence (M3) | Yes | Provenance model |
| Findings (M4) | Yes | Checkpoint findings |
| React migration (M4.5) | Yes | Svelte removed |
| Append-only ledger (M4.6) | Yes | obs/amend/supersede/redact |
| Schema through v6 | Yes | `SCHEMA_VERSION = 6` |
| Local discovery (M5) | Yes | Core + service + desktop |
| Ledger workspace (M5) | Yes | Projects → runs → ledger |
| Redaction ordinary UI | Partial | Timeline + ProtocolLedgerPanel fixed on main |
| Redaction agent-facing APIs | **No on main** | Finding DTOs still leak frozen checkpoint content; fix is PR #12 |
| Full interactive M5 acceptance | Partial | Automated + binary launch; limited screenshots |
| Green CI on merged head | Yes for #11 | |

## Baseline stability decision

**Proceed evaluating `origin/main` @ `4f8d1e8`.**

PR #11 is the intended product merge for M5 and is CI-green.  
**Do not treat PR #12 as part of the evaluated product.** Its absence is recorded as an **integrity/blocker finding** (agent-facing redaction bypass via findings). Evaluation policy forbids merging PRs during this audit.

## Dogfood run

- Run ID: `ce58b532-6469-415e-8ec8-f8b82b923a65`
- Path: `.moraine/runs/2026-07-21-conduct-a-complete-product-architecture-ce58b532.md`
- Objective: complete product/architecture/integrity/UX/release evaluation

## Installed-binary caveat

`~/.cargo/bin/moraine` at evaluation time advertised **0.1.0** but **lacked** `project`/`run`/`mcp` subcommands (stale install). Workspace `./target/debug/moraine` is current. This is itself a release/onboarding finding.
