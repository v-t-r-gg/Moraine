Moraine local integration runtime

Background service for Milestone 2 deterministic capture.

## Transport model (precise)

| Path | Transport | Purpose |
|------|-----------|---------|
| Hooks / adapters | **Unix domain socket** | Event intake (primary) |
| Diagnostics | **Loopback TCP HTTP** (`127.0.0.1` only) | `/health`, `/status`, `/projects` |

Hooks never use TCP. The HTTP listener refuses non-loopback binds.

```text
Hook adapter (moraine hook-codex)
    → Unix domain socket ($XDG_RUNTIME_DIR/moraine-service.sock)
    → moraine-service
         ↳ core ops + project-local run bundles
         ↳ bounded spool under cache dir (recovery)

Diagnostics clients
    → http://127.0.0.1:33111/{health,status,projects}
```

## Run locally

```bash
cargo run -p moraine-service -- --http 127.0.0.1:33111 --unix-socket /tmp/moraine-service.sock
```

Systemd user unit (Linux):

```bash
cargo run -p moraine-service -- install
cargo run -p moraine-service -- start
```

Unit template: [crates/moraine-service/systemd/moraine-service.service](crates/moraine-service/systemd/moraine-service.service)

Codex hooks: [docs/integrations/CODEX.md](../../docs/integrations/CODEX.md).

## Spool guarantees

- One file per event; atomic create (temp + rename)
- Stable `eventId` dedupe with durable `spool/seen/` markers (survives restart)
- Max event size 1 MiB; pending file soft-cap; corrupt → `quarantine/`; failed → `failed/` (no poison retry)
- File mode `0600`, directory mode `0700` on Unix
