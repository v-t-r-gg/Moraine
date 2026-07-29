use std::fs;
use std::path::PathBuf;

use moraine_core::{
    load_run_meta_readonly, OP_ENTRY_REDACT, OP_ENTRY_SUPERSEDE, OP_HUMAN_OBSERVATION_ADD,
    OP_RUN_AMEND, SCHEMA_CURRENT_WRITABLE,
};

#[test]
fn committed_run_bundle_is_current_and_feature_complete() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/run-bundle");
    let markdown = root.join("run.md");
    let sidecar = root.join("run.md.moraine.json");
    assert!(markdown.is_file());
    assert!(sidecar.is_file());

    let meta = load_run_meta_readonly(&markdown)
        .expect("fixture must be readable")
        .expect("fixture sidecar must exist");
    assert_eq!(meta.schema_version, SCHEMA_CURRENT_WRITABLE);
    let agent = meta.agent.expect("fixture must contain agent state");
    assert_eq!(agent.checkpoints.len(), 1);
    assert_eq!(agent.checkpoints[0].evidence.len(), 1);
    assert_eq!(agent.findings.len(), 1);
    for kind in [
        OP_HUMAN_OBSERVATION_ADD,
        OP_RUN_AMEND,
        OP_ENTRY_SUPERSEDE,
        OP_ENTRY_REDACT,
    ] {
        assert!(agent.append_only_ops.iter().any(|op| op.op_kind == kind));
    }

    let projected = fs::read_to_string(markdown).unwrap();
    assert!(projected.contains("[REDACTED]"));
}
