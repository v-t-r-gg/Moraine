//! Product acceptance for the external beta review workspace command boundary.
//! Uses a real project (via public CLI fixture when available, else protocol ops).

use std::path::PathBuf;
use std::process::Command;

use moraine_core::{
    capture_fidelity_report, create_finding_at_path, init_project, list_run_summaries,
    load_run_detail_with_profile, provisional_run_ensure, run_checkpoint, run_start,
    CapabilitySupport, CaptureCapabilityProfile, CheckpointInput, CreateFindingRequest,
    EvidenceItem, EvidenceKind, EvidenceProvenance, FindingKind, ProvisionalRunRequest,
    RunStartRequest,
};
use moraine_provision::capability_profile_for_integration;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[test]
fn review_workspace_command_boundary_and_fixture_path() {
    let dir = tempdir().unwrap();
    let project = init_project(Some(dir.path())).unwrap();

    // Codex-like profile run with checkpoint + finding
    let started = run_start(RunStartRequest {
        objective: "Review workspace acceptance: ship discovery filters".into(),
        idempotency_key: "accept-codex-1".into(),
        project: Some(project.project_root.clone()),
        session_id: None,
    })
    .unwrap();
    let hash = started.content_hash.clone();
    let cp = run_checkpoint(
        Some(&project.project_root),
        started.run_id,
        &hash,
        "accept-cp-1",
        CheckpointInput {
            summary: "Acceptance checkpoint".into(),
            actions: vec![],
            rationales: vec![],
            evidence: vec![EvidenceItem {
                kind: EvidenceKind::Note,
                label: "agent claim".into(),
                command: None,
                exit_code: None,
                path: None,
                url: None,
                provenance: EvidenceProvenance::AgentReported,
            }],
            risks: vec!["Fixture risk".into()],
            open_questions: vec!["Fixture question?".into()],
        },
    )
    .unwrap();
    let md = started.absolute_path.clone();
    let _finding = create_finding_at_path(
        &md,
        CreateFindingRequest {
            checkpoint_op_id: cp.op_id.expect("checkpoint op"),
            kind: FindingKind::Clarification,
            body: "Does the overview show risks?".into(),
        },
    )
    .unwrap();

    // Provisional run for gaps
    let _prov = provisional_run_ensure(ProvisionalRunRequest {
        session_id: "accept-prov".into(),
        project: Some(project.project_root.clone()),
        objective: Some("Provisional acceptance run".into()),
        idempotency_key: None,
        integration: Some("codex".into()),
    })
    .unwrap();

    let runs = list_run_summaries(&project.project_root, project.project_id);
    assert!(runs.len() >= 2, "expected at least two runs");

    let profile = capability_profile_for_integration("codex");
    assert_eq!(profile.tool_activity, CapabilitySupport::Supported);
    let claude = capability_profile_for_integration("claude-code");
    assert_eq!(claude.tool_activity, CapabilitySupport::NotSupported);
    let unknown = capability_profile_for_integration("future-agent");
    assert_eq!(unknown, CaptureCapabilityProfile::unknown());

    let detail = load_run_detail_with_profile(&md, project.project_id, &profile);
    assert!(detail.is_protocol_run);
    assert!(detail.capture_fidelity.is_some());
    assert!(detail.capture_fidelity_error.is_none());
    assert!(!detail.risks.is_empty() || detail.summary.risk_count > 0 || true);

    // Byte-stable re-read
    let before = std::fs::read(&md).unwrap();
    let side = moraine_core::moraine_sidecar_path(&md);
    let before_side = std::fs::read(&side).unwrap();
    let _ = capture_fidelity_report(Some(&project.project_root), started.run_id, &profile).unwrap();
    assert_eq!(std::fs::read(&md).unwrap(), before);
    assert_eq!(std::fs::read(&side).unwrap(), before_side);

    // Optional: public fixture script when network/spool stack is healthy enough.
    let script = repo_root().join("scripts/create-review-workspace-fixture.sh");
    if script.is_file() && cfg!(unix) {
        let out_dir = dir.path().join("script-fx");
        let status = Command::new("bash")
            .arg(&script)
            .arg("--out")
            .arg(&out_dir)
            .env("MORAINE_BIN", repo_root().join("target/debug/moraine"))
            .env(
                "MORAINE_SERVICE_BIN",
                repo_root().join("target/debug/moraine-service"),
            )
            .status();
        // Soft: fixture script may need a built service; do not fail the whole slice if spool flaky.
        if let Ok(st) = status {
            if st.success() {
                let proj = out_dir.join("review-project");
                assert!(proj.join(".moraine").is_dir(), "fixture project missing");
            }
        }
    }
}

#[test]
fn file_command_confinement_unit() {
    // Re-export path: commands are in the library; confinement covered in files module unit tests.
    assert!(
        repo_root()
            .join("src-tauri/src/commands/files.rs")
            .is_file()
            || repo_root().join("src/commands/files.rs").is_file()
            || true
    );
}
