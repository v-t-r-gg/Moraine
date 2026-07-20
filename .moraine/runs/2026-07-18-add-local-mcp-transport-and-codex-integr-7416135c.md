# Moraine run record

## Objective

Add local MCP transport and Codex integration

## Protocol status

> **Managed regions:** Everything above `## Human notes` is regenerated from Moraine structured state. Human free-form edits and accepted suggestion text outside Human notes are **not** preserved on the next agent operation. Review managed content with comments / request-changes; put free-form notes only under Human notes.

- **Run ID:** `7416135c-f8f6-4377-801c-a7c47674a372`
- **Lifecycle:** `ready_for_review`
- **Record revision:** `8`
- **Record path:** `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md`
- **Project ID:** `a6ceb4ba-c909-4977-a2af-b4488e4ea313`

## Starting Git context

- **Repository root:** `/home/bone/Projects/Moraine`
- **Branch:** `feat/local-mcp-codex-integration`
- **HEAD:** `e09f68d436d74bbe0253e669437c6d1a0b122408`
- **Working tree:** `dirty`
- **Changed files:** 2
  - `.moraine/.gitignore`
  - `.moraine/project.json`

## Current Git context

- **Repository root:** `/home/bone/Projects/Moraine`
- **Branch:** `feat/local-mcp-codex-integration`
- **HEAD:** `27d3cdb8819ecba5675225e4582436d7ca2375e6`
- **Upstream:** `origin/feat/local-mcp-codex-integration`
- **Working tree:** `dirty`
- **Changed files:** 20
  - `.github/workflows/ci.yml`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md.moraine.json`
  - `ARCHITECTURE.md`
  - `Cargo.lock`
  - `Cargo.toml`
  - `README.md`
  - `ROADMAP.md`
  - `crates/moraine-core/src/agent_protocol/git.rs`
  - `docs/DEVELOPMENT.md`
  - `docs/DEVELOPMENT_BLUEPRINT.md`
  - `docs/MCP.md`
  - `package-lock.json`
  - `package.json`
  - `scripts/check.sh`
  - `src-tauri/Cargo.toml`
  - `src-tauri/capabilities/default.json`
  - `src-tauri/src/commands/review.rs`
  - `src-tauri/src/lib.rs`
  - `src/lib/api.ts`

## Checkpoints

### Checkpoint 1 — 2026-07-18T17:59:17Z

- **Op ID:** `683c13c3-2b63-4161-a174-70dc3447e938`
- **Summary:** Confirmed PR #6 merged at e09f68d; started branch and dogfood run via protocol CLI

#### Actions

- Verified main at e09f68d contains agent-run protocol recovery, Human notes, managed regions
- Created feat/local-mcp-codex-integration
- Selected official rmcp Rust SDK for STDIO MCP server

#### Rationales

- **Use official rmcp crate with server+transport-io features:** Official Model Context Protocol Rust SDK; stdio transport is first-class

#### Git at checkpoint

- **Repository root:** `/home/bone/Projects/Moraine`
- **Branch:** `feat/local-mcp-codex-integration`
- **HEAD:** `e09f68d436d74bbe0253e669437c6d1a0b122408`
- **Working tree:** `dirty`
- **Changed files:** 4
  - `.moraine/.gitignore`
  - `.moraine/project.json`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md.moraine.json`


### Checkpoint 2 — 2026-07-20T20:34:02Z

- **Op ID:** `f62723c5-5528-464f-a545-3483c00f5f74`
- **Summary:** Shipped MCP crate, Codex docs, and M0 ledger realignment on feat/local-mcp-codex-integration

#### Actions

- Added crates/moraine-mcp STDIO server over core run ops (five tools only)
- Documented Codex one-time project config in docs/integrations/CODEX.md
- Merged M0 vision realignment: decide legacy, primary UI without verdict controls
- Pushed PR branch with dogfood project metadata

#### Rationales

- **Reuse moraine-core operations behind MCP rather than a second persistence path:** Blueprint requires CLI/MCP/desktop to share one core

#### Evidence

- [command_result | agent_reported] cargo test -p moraine-mcp — `cargo test -p moraine-mcp` (exit 0)

#### Git at checkpoint

- **Repository root:** `/home/bone/Projects/Moraine`
- **Branch:** `feat/local-mcp-codex-integration`
- **HEAD:** `27d3cdb8819ecba5675225e4582436d7ca2375e6`
- **Upstream:** `origin/feat/local-mcp-codex-integration`
- **Working tree:** `dirty`
- **Changed files:** 17
  - `github/workflows/ci.yml`
  - `RCHITECTURE.md`
  - `argo.lock`
  - `argo.toml`
  - `EADME.md`
  - `OADMAP.md`
  - `ocs/DEVELOPMENT.md`
  - `ocs/DEVELOPMENT_BLUEPRINT.md`
  - `ocs/MCP.md`
  - `ackage-lock.json`
  - `ackage.json`
  - `cripts/check.sh`
  - `rc-tauri/Cargo.toml`
  - `rc-tauri/capabilities/default.json`
  - `rc-tauri/src/commands/review.rs`
  - `rc-tauri/src/lib.rs`
  - `rc/lib/api.ts`


### Checkpoint 3 — 2026-07-20T20:34:17Z

- **Op ID:** `a6e40cbe-c6da-46af-906e-99c248d56e3d`
- **Summary:** Closed M0 cleanup and M1 acceptance gaps: CI, FS scope, decision IPC, MSRV, DoD

#### Actions

- Added moraine-mcp to clippy/tests in scripts/check.sh, CI, and npm scripts
- Removed unused webview fs:** capability wildcards and direct tauri-plugin-fs usage
- Removed record_run_decision from Tauri command + TypeScript IPC (CLI decide remains legacy)
- Raised workspace MSRV to 1.88 and added CI msrv job
- Documented dogfood definition-of-done requiring ready_for_review

#### Rationales

- **Remove desktop decision IPC instead of only warning:** Primary UI already hid controls; live unguarded path contradicted M0 boundary
- **MSRV 1.88:** rmcp edition 2024 and darling via rmcp-macros require it; prior 1.77 claim was incomplete

#### Evidence

- [command_result | agent_reported] vitest + svelte-check — `npm test && npx svelte-check --threshold error` (exit 0)
- [command_result | agent_reported] cargo test -p moraine-mcp — `cargo build -p moraine-cli && cargo test -p moraine-mcp` (exit 0)

#### Git at checkpoint

- **Repository root:** `/home/bone/Projects/Moraine`
- **Branch:** `feat/local-mcp-codex-integration`
- **HEAD:** `27d3cdb8819ecba5675225e4582436d7ca2375e6`
- **Upstream:** `origin/feat/local-mcp-codex-integration`
- **Working tree:** `dirty`
- **Changed files:** 19
  - `github/workflows/ci.yml`
  - `moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md`
  - `moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md.moraine.json`
  - `RCHITECTURE.md`
  - `argo.lock`
  - `argo.toml`
  - `EADME.md`
  - `OADMAP.md`
  - `ocs/DEVELOPMENT.md`
  - `ocs/DEVELOPMENT_BLUEPRINT.md`
  - `ocs/MCP.md`
  - `ackage-lock.json`
  - `ackage.json`
  - `cripts/check.sh`
  - `rc-tauri/Cargo.toml`
  - `rc-tauri/capabilities/default.json`
  - `rc-tauri/src/commands/review.rs`
  - `rc-tauri/src/lib.rs`
  - `rc/lib/api.ts`


### Checkpoint 4 — 2026-07-20T20:35:41Z

- **Op ID:** `c42c8bf8-f232-45e3-96c5-a59d250e41bb`
- **Summary:** Fixed git porcelain path parsing that stripped leading path characters

#### Actions

- Stopped trim_start before XY PATH slice in capture_git_context
- Added porcelain_path unit tests for unstaged, untracked dotfiles, and renames
- Resumed dogfood run to refresh Current Git context with correct paths

#### Rationales

- **Treat as M1 dogfood integrity fix, not a separate milestone:** Corrupt changed-file lists made the flagship run record dishonest

#### Evidence

- [command_result | agent_reported] porcelain unit test — `cargo test -p moraine-core porcelain_keeps` (exit 0)

#### Git at checkpoint

- **Repository root:** `/home/bone/Projects/Moraine`
- **Branch:** `feat/local-mcp-codex-integration`
- **HEAD:** `27d3cdb8819ecba5675225e4582436d7ca2375e6`
- **Upstream:** `origin/feat/local-mcp-codex-integration`
- **Working tree:** `dirty`
- **Changed files:** 20
  - `.github/workflows/ci.yml`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md.moraine.json`
  - `ARCHITECTURE.md`
  - `Cargo.lock`
  - `Cargo.toml`
  - `README.md`
  - `ROADMAP.md`
  - `crates/moraine-core/src/agent_protocol/git.rs`
  - `docs/DEVELOPMENT.md`
  - `docs/DEVELOPMENT_BLUEPRINT.md`
  - `docs/MCP.md`
  - `package-lock.json`
  - `package.json`
  - `scripts/check.sh`
  - `src-tauri/Cargo.toml`
  - `src-tauri/capabilities/default.json`
  - `src-tauri/src/commands/review.rs`
  - `src-tauri/src/lib.rs`
  - `src/lib/api.ts`


## Risks

- rmcp API version churn; pin via Cargo.lock
- Initial PR commit mixed M0+M1 contrary to one-problem-per-milestone preference
- tauri-plugin-dialog still depends on tauri-plugin-fs transitively; capability scopes must stay non-wildcard

## Open questions

- Exact Codex config.toml keys for tool allowlists and timeouts on current Codex docs
- Whether a real Codex session with zero Moraine prompt text reliably follows server instructions on every Codex build

## Lifecycle events

- **ready** at 2026-07-20T20:34:24Z (op `743096d4-0b62-4055-954c-f573d6f1acde`) — Local MCP + Codex docs + M0/M1 acceptance gaps closed; CI covers moraine-mcp; dogfood lifecycle ready_for_review
- **resume** at 2026-07-20T20:35:29Z (op `1fc954cf-904a-416f-90ac-39e8aa600e83`) — Fix porcelain path parsing before final ready
- **ready** at 2026-07-20T20:35:41Z (op `71ee4ae9-e088-47a5-8c26-f44de349913c`) — M1 acceptance complete: MCP in CI, decision IPC removed, MSRV 1.88, FS scopes tightened, dogfood ready_for_review with honest git paths

## Ready for review

This run is **ready for human review**. Human decisions use `moraine decide` and are separate from agent lifecycle.

**Outcome summary:** M1 acceptance complete: MCP in CI, decision IPC removed, MSRV 1.88, FS scopes tightened, dogfood ready_for_review with honest git paths

---

## Human notes
