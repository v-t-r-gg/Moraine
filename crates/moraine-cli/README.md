# moraine-cli

Terminal entry for agents, scripts, and humans. Prefer the root [README.md](../../README.md) for product positioning (agent-run ledger; review without verdict).

```bash
cargo run -p moraine-cli -- --help
moraine info --json
moraine run start --objective "…" --idempotency-key "…" --json
moraine status path/to/run-record.md
moraine mcp --project /absolute/path/to/project
moraine share path/to/run-record.md --json
```

`moraine decide` is legacy/compatibility-only.

Exit codes: `0` ok, `1` error, `2` not found, `3` relay down.
