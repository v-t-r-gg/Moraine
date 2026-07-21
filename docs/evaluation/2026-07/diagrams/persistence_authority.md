# Persistence and authority

```mermaid
flowchart TB
  subgraph canonical [Canonical project-local]
    PJ[".moraine/project.json"]
    MD["runs/*.md Markdown projection"]
    SC["runs/*.md.moraine.json sidecar SCHEMA≤6"]
    EV[".moraine/evidence/* optional"]
  end

  subgraph cache [Noncanonical cache]
    IDX["service spool/index.json revision++"]
  end

  subgraph intake [Capture intake]
    HOOK["hooks → Unix socket"]
    SPL["spool pending/processed/failed/seen"]
  end

  CORE["moraine-core ops under sidecar lock"]
  SVC["moraine-service"]
  DESK["desktop discovery"]
  MCP["moraine mcp"]
  CLI["moraine CLI"]

  CLI --> CORE
  MCP --> CORE
  HOOK --> SPL --> SVC --> CORE
  CORE --> MD
  CORE --> SC
  CORE --> EV
  CORE --> PJ
  SVC --> IDX
  DESK -->|loopback HTTP or FS scan| SVC
  DESK -->|direct summarize| CORE
  IDX -.->|points at roots only| PJ
```

## Authority rules

1. **Run ledger truth** = project sidecar (+ MD projection).
2. **Index** may be deleted; rebuild from disk.
3. **Browsing** must not mutate bundles (asserted by tests).
4. **Filesystem owner** can still edit files externally — not prevented.
