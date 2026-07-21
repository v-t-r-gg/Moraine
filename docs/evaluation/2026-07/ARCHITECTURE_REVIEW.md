# Architecture review

**Baseline:** `4f8d1e8`  
**Diagrams:** [runtime](./diagrams/architecture.md), [persistence](./diagrams/persistence_authority.md), [session reconciliation](./diagrams/session_reconciliation.md)

## Runtime shape

```text
Codex (or future agent)
  ├── MCP STDIO ──► moraine-mcp ──► moraine-core
  └── hooks ──► moraine hook-codex ──► Unix socket / spool ──► moraine-service ──► core

moraine-service
  ├── spool reconciliation + seen markers
  ├── provisional run ensure / session observe
  ├── index.json rebuild (cache)
  └── loopback HTTP discovery diagnostics

moraine-core ── domain + persistence authority for run bundles

Humans: CLI · React/Tauri desktop · optional moraine-server Yjs relay
```

## Business-rule ownership

| Rule | Owner | Leakage risk |
|------|-------|--------------|
| Run protocol mutations | core | Low |
| Findings / append-only | core | Low |
| Discovery summaries | core | Low (good) |
| Redaction current claim | core `is_redacted` / `current_checkpoint_claim` | **High** if UI/MCP re-read raw frozen snapshot |
| Index rebuild | service uses core summarize | Low |
| Desktop discovery transport | Tauri + curl loopback | Medium fragility |
| Filters | core filter + client UI | OK at scale |

## Is the service justified?

**Yes** for capture-without-desktop, spool durability, and provisional reconciliation.

**Risks**

- Becomes *perceived* DB if index language is sloppy
- Linux systemd orientation
- Install/debug burden for beta users
- Fixed loopback diagnostics port patterns

**Not** yet an accidental canonical DB in code (bundles remain truth), but operationally central for capture.

## Transport review

| Mechanism | Assessment |
|-----------|------------|
| Unix socket | Correct primary for hooks |
| Loopback HTTP | OK diagnostics; must stay loopback-only |
| curl from Tauri | **Consolidate away** — unnecessary dependency |
| FS fallback scan | Keep for offline honesty |
| Desktop revision polling | Acceptable local-only |
| Yjs WebSocket | Freeze — secondary product |
| Tiptap | Freeze expansion for protocol path |
| Legacy document editing | Labeled legacy only |

## Architecture verdict

**Coherent ledger core; overloaded product shell.**  
Consolidate transports and freeze collab before expanding integrations.
