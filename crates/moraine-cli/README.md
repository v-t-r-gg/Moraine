# moraine-cli

Terminal entry for agents, scripts, and humans. Prefer the root [README.md](../../README.md) for product positioning (run records + human review).

```bash
cargo run -p moraine-cli -- --help
moraine info --json
moraine status path/to/run-record.md
moraine share path/to/run-record.md --json
moraine cat path/to/run-record.md
moraine write path/to/run-record.md --content "# title"
moraine history path/to/run-record.md
```

Exit codes: `0` ok, `1` error, `2` not found, `3` relay down.
