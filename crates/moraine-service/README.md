Moraine local integration runtime

Background service for Milestone 2 deterministic capture (hooks → spool → core).

Run locally:

```bash
cargo run -p moraine-service -- --http 127.0.0.1:33111 --unix-socket /tmp/moraine-service.sock
```

Endpoints: `/health`, `/status`, `/projects` (index-backed when `index.json` exists).

Systemd user unit (Linux):

```bash
cargo run -p moraine-service -- install
cargo run -p moraine-service -- start
```

Unit template: [crates/moraine-service/systemd/moraine-service.service](crates/moraine-service/systemd/moraine-service.service)

Codex hooks deliver via `moraine hook-codex` (see [docs/integrations/CODEX.md](../../docs/integrations/CODEX.md)).
