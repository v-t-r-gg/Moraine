# Current architecture (factual)

```text
Supported agent (Codex today)
├── MCP semantic channel  →  moraine mcp (STDIO)  →  moraine-core
├── Codex hook adapter    →  moraine hook-codex   →  Unix socket / spool
└── optional CLI wrappers →  moraine run …

Moraine local service (moraine-service)
├── Unix socket intake (hooks; primary capture transport)
├── spool (pending / processed / failed / seen)
├── provisional run ensure + session observe
├── rebuildable index.json (noncanonical cache)
├── discovery HTTP (loopback only): /status /projects /runs /rebuild /rescan
└── systemd --user unit helpers (Linux)

moraine-core
├── project identity (.moraine/project.json)
├── run protocol (start/checkpoint/ready/resume/show)
├── run_meta sidecar SCHEMA_VERSION=6
├── incomplete-op recovery
├── evidence capture + secret redaction helpers
├── findings + append-only ops
├── discovery read models (summaries, timeline, filters)
├── annotations / comments / history / share URLs
└── document IO + locks

Project-local artifacts
├── .moraine/project.json
├── .moraine/runs/*.md              (Markdown projection)
├── .moraine/runs/*.md.moraine.json (canonical structured ledger)
├── .moraine/evidence/…            (when captured)
├── session envelopes (as implemented)
└── locks / temp files (gitignored)

Human surfaces
├── CLI (moraine-cli)
├── React + Tauri desktop (src/ + src-tauri)
│   ├── discovery workspace (default)
│   ├── protocol ledger panels
│   ├── findings / annotations
│   └── legacy free-form document mode
└── optional moraine-server Yjs WebSocket relay + share URLs
```

## Authority map (as designed)

| Concern | Authority |
|---------|-----------|
| Run ledger facts | Project-local sidecar + Markdown projection via core ops |
| Service index | Cache only; rebuildable |
| Capture while desktop closed | Service + hooks + MCP (desktop not required) |
| Classification/filter rules | Prefer `moraine-core` discovery module |
| Desktop discovery transport | Tauri → service loopback HTTP (curl) or direct FS scan |
