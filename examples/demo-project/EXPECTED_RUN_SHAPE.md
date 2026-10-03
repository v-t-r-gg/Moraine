# Expected run shape

This file describes what a live demo creates. It is not a run. Nothing under
this project is a pre-baked agent result.

After `moraine project init` and a real Codex or Claude Code session, Explorer
should show a project-local ledger:

```text
.moraine/
├── project.json
└── runs/
    ├── <run>.md
    └── <run>.md.moraine.json
```

`<run>.md` is the readable record. `<run>.md.moraine.json` is the sidecar.
The Moraine source tree's `examples/run-bundle/` is a generated fixture of
that shape. It is not this demo's result.

Redaction does not erase a raw sidecar. Do not put secrets in the project.
