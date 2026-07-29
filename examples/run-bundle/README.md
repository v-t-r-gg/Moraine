# Generated run bundle

`run.md` & `run.md.moraine.json` are generated through public `moraine-core`
operations:

```bash
cargo run -p moraine-core --example generate_run_bundle -- examples/run-bundle
cargo test -p moraine-core --test run_bundle_fixture
```

The fixture demonstrates a checkpoint, evidence, finding, human observation,
amendment, supersession & target redaction. IDs & timestamps are generated; do
not hand-edit either artifact.
