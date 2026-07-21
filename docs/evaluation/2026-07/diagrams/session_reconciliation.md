# Session → run reconciliation

```mermaid
sequenceDiagram
  participant Agent as Coding agent (Codex)
  participant Hook as hook-codex
  participant Svc as moraine-service
  participant Spool as spool+seen
  participant Core as moraine-core
  participant Disk as project run bundle
  participant MCP as moraine mcp

  Agent->>Hook: session_start / user_prompt / tool events
  Hook->>Svc: Unix socket event (or write spool if down)
  alt service down
    Hook->>Spool: durable pending file
  end
  Svc->>Spool: process pending; mark seen
  Svc->>Core: provisional_run_ensure / session_observe
  Core->>Disk: provisional run sidecar+md
  Agent->>MCP: run_start / checkpoint (semantic)
  MCP->>Core: run protocol ops
  Core->>Disk: confirm/reconcile same run when designed
  Note over Disk: Desktop discovers via index/FS without path
```

## Failure modes reviewed

| Mode | Expected |
|------|----------|
| Service stop mid-session | Events spool; process on restart; seen prevents dup |
| Duplicate delivery | eventId / content hash + seen markers |
| Semantic start without hooks | Protocol-only run still valid (coverage may be semantic_only) |
| Hooks without MCP | Provisional / mechanical path; may lack rich checkpoints |
