# Pass A — Capability inventory (what Moraine is today)

**Baseline:** `4f8d1e8`  
**Method:** repository archaeology only; “User-ready” is never inferred from tests alone.

Legend: **I** implemented · **T** tested · **L** live-validated this evaluation · **D** documented · **U** external-user-ready

| Capability | I | T | L | D | U | Notes |
|------------|---|---|---|---|---|-------|
| Workspace crates (core/cli/mcp/service/server/app) | ✓ | ✓ | partial | ✓ | partial | Five crates + Tauri |
| Agent run protocol CLI | ✓ | ✓ | ✓ | ✓ | partial | Needs current binary install path |
| Project init / UUID identity | ✓ | ✓ | ✓ | ✓ | partial | |
| Run start/checkpoint/ready/resume/show/open | ✓ | ✓ | ✓ | ✓ | partial | |
| Local STDIO MCP | ✓ | ✓ | partial | ✓ | partial | Codex-focused docs |
| Findings list/get/respond MCP | ✓ | ✓ | automated | ✓ | partial | Redaction leak on main via snapshots |
| Codex hook adapter | ✓ | ✓ | not full agent session | ✓ | no | Requires Codex + service setup |
| Local service Unix socket | ✓ | ✓ | ✓ | partial | no | systemd --user Linux |
| Spool + seen-markers | ✓ | ✓ | partial | partial | no | |
| Provisional runs | ✓ | ✓ | not full | partial | no | |
| Capture coverage | ✓ | ✓ | no | partial | no | |
| Evidence capture M3 | ✓ | ✓ | automated | ✓ | no | |
| Secret redaction in evidence | ✓ | ✓ | automated | partial | no | |
| Findings desktop | ✓ | ✓ | automated | partial | no | |
| Append-only ops | ✓ | ✓ | automated | ✓ | no | |
| Redaction ordinary timeline/UI | ✓ | ✓ | automated | partial | no | |
| Redaction agent-facing findings | ✗ main | ✓ on PR12 | no | no | no | **Blocker** |
| Discovery core read models | ✓ | ✓ | automated | ✓ | no | |
| Service discovery HTTP | ✓ | ✓ | ✓ loopback | ✓ | no | |
| Desktop discovery workspace | ✓ | ✓ RTL | binary launch only | ✓ | no | |
| Index rebuild nonmutation | ✓ | ✓ | ✓ | ✓ | no | |
| React + Vite + Tauri | ✓ | ✓ | partial | ✓ | no | WebKit deps |
| Tiptap editor | ✓ | ✓ | no | partial | no | Legacy + collab |
| Yjs + moraine-server relay | ✓ | partial | no | partial | no | Secondary |
| Share/join URLs | ✓ | ✓ | no | ✓ | no | |
| Annotations/comments | ✓ | ✓ | no | partial | no | |
| Local edit history | ✓ | ✓ | no | partial | no | |
| Legacy decide CLI | ✓ | partial | no | ✓ | n/a | Compat only |
| Schema migration ≤6 | ✓ | ✓ | automated | partial | no | |
| Unsupported schema reject | ✓ | ✓ | automated | partial | no | |
| CI (fmt/clippy/test/frontend/tauri) | ✓ | ✓ | CI green #11 | ✓ | n/a | |
| Packaging / installers | ✗ | ✗ | ✗ | weak | **no** | Dev scripts only |
| Multi-agent support | Codex only | Codex | no second agent | claims “cross-agent” | **no** | |
| Cold install path | weak | ✗ | not clean env | incomplete | **no** | |
| Screenshots / demo video | sparse | n/a | n/a | incomplete | **no** | |
| SECURITY.md / CONTRIBUTING | ✗ | n/a | n/a | ✗ | no | |
| Changelog | weak | n/a | n/a | incomplete | no | |

## What Moraine actually is (factual)

Moraine is a **Rust workspace + React/Tauri desktop** that records **coding-agent runs** as **project-local Markdown + JSON sidecars**, with:

1. a **semantic agent protocol** (CLI/MCP);
2. a **local integration service** for hook events, provisional runs, spool, and a rebuildable discovery index;
3. a **human ledger workspace** for discovering runs and inspecting append-only review context;
4. optional **live sharing** via an in-memory Yjs relay (not the product center).

It is **not** (today): a hosted multi-user product, an approval gate, a full observability platform, a multi-agent production suite, or a packaged one-click install.

## Binaries / surfaces

| Surface | Role |
|---------|------|
| `moraine` (CLI) | Protocol, files, share, mcp, hook-codex, legacy decide |
| `moraine-service` | Capture runtime + index + diagnostics HTTP |
| `moraine-server` | Optional Yjs WebSocket relay |
| `moraine-app` | Desktop host |
| `moraine mcp` | STDIO MCP server |

## Persistence layout

```text
project/
  .moraine/
    project.json
    runs/*.md
    runs/*.md.moraine.json
    evidence/…   (optional)
  user spool (service, not in project):
    index.json, pending/, processed/, failed/, seen/
```

## Schema

- Sidecar `SCHEMA_VERSION = 6` (append-only ops)
- Findings introduced earlier (v5 era); promotions on load when allowed
- Future versions rejected without destructive rewrite

## Tests (order of magnitude)

- ~142 listed Rust tests (workspace filter)
- 48–63 frontend Vitest tests depending on branch
- Service integration: spool, evidence, discovery index/HTTP
- MCP STDIO lifecycle tests

## Platforms

- **Primary development:** Linux
- **Service unit:** systemd --user
- **Desktop:** Tauri + WebKit dependencies
- **Windows/macOS:** not release-validated here
