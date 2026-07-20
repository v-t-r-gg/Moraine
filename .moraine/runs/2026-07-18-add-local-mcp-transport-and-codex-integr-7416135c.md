# Moraine run record

## Objective

Add local MCP transport and Codex integration

## Protocol status

> **Managed regions:** Everything above `## Human notes` is regenerated from Moraine structured state. Human free-form edits and accepted suggestion text outside Human notes are **not** preserved on the next agent operation. Review managed content with comments / request-changes; put free-form notes only under Human notes.

- **Run ID:** `7416135c-f8f6-4377-801c-a7c47674a372`
- **Lifecycle:** `active`
- **Record revision:** `2`
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
- **HEAD:** `e09f68d436d74bbe0253e669437c6d1a0b122408`
- **Working tree:** `dirty`
- **Changed files:** 4
  - `.moraine/.gitignore`
  - `.moraine/project.json`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md`
  - `.moraine/runs/2026-07-18-add-local-mcp-transport-and-codex-integr-7416135c.md.moraine.json`

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


## Risks

- rmcp API version churn; pin via Cargo.lock

## Open questions

- Exact Codex config.toml keys for tool allowlists and timeouts on current Codex docs

## Lifecycle events

_None yet._

---

## Human notes
