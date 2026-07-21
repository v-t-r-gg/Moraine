//! C1 complete-payload nonleak tests for ordinary redaction projections.
#![cfg(test)]

use super::append_ops::{
    entry_redact, entry_supersede, list_append_ops, run_amend, AmendRequest, RedactRequest,
    SupersedeRequest,
};
use super::findings::{
    create_finding, get_finding, list_findings, respond_to_finding, CreateFindingRequest,
};
use super::ops::{
    run_checkpoint, run_show, run_start, CheckpointInput, RunShowOptions, RunStartRequest,
};
use super::project::init_project;
use super::projection::{assert_json_omits, REDACTED_MARKER, REDACTION_TEST_SENTINEL};
use super::types::{
    ActorCategory, EvidenceItem, EvidenceKind, EvidenceProvenance, FindingKind, RationalItem,
};
use crate::discovery::load_run_detail;
use crate::run_meta::load_run_meta_readonly;
use std::path::Path;
use tempfile::tempdir;
use uuid::Uuid;

const S_SUM: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_SUMMARY";
const S_ACT: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_ACTION";
const S_RCHOICE: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_RCHOICE";
const S_RREASON: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_RREASON";
const S_ELABEL: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_ELABEL";
const S_ECMD: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_ECMD";
const S_EPATH: &str = "/tmp/MORAINE_REDACTION_SENTINEL_7f4d2a91_EPATH";
const S_EURL: &str = "https://example.test/MORAINE_REDACTION_SENTINEL_7f4d2a91_EURL";
const S_RISK: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_RISK";
const S_Q: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_QUESTION";
const S_AMEND: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_AMEND";
const S_SUPER: &str = "MORAINE_REDACTION_SENTINEL_7f4d2a91_SUPER";

fn all_needles() -> Vec<&'static str> {
    vec![
        REDACTION_TEST_SENTINEL,
        S_SUM,
        S_ACT,
        S_RCHOICE,
        S_RREASON,
        S_ELABEL,
        S_ECMD,
        S_EPATH,
        S_EURL,
        S_RISK,
        S_Q,
        S_AMEND,
        S_SUPER,
    ]
}

fn setup_full_checkpoint(dir: &Path) -> (Uuid, std::path::PathBuf, Uuid, String) {
    let project = init_project(Some(dir)).unwrap();
    let start = run_start(RunStartRequest {
        objective: "C1 redaction nonleak".into(),
        idempotency_key: "c1-start".into(),
        project: Some(project.project_root.clone()),
        session_id: None,
    })
    .unwrap();
    let cp = run_checkpoint(
        Some(&project.project_root),
        start.run_id,
        &start.content_hash,
        "c1-cp",
        CheckpointInput {
            summary: S_SUM.into(),
            actions: vec![S_ACT.into()],
            rationales: vec![RationalItem {
                choice: S_RCHOICE.into(),
                reason: S_RREASON.into(),
            }],
            evidence: vec![EvidenceItem {
                kind: EvidenceKind::CommandResult,
                label: S_ELABEL.into(),
                command: Some(S_ECMD.into()),
                exit_code: Some(0),
                path: Some(S_EPATH.into()),
                url: Some(S_EURL.into()),
                provenance: EvidenceProvenance::AgentReported,
            }],
            risks: vec![S_RISK.into()],
            open_questions: vec![S_Q.into()],
        },
    )
    .unwrap();
    (
        start.run_id,
        project.project_root,
        cp.op_id.unwrap(),
        cp.content_hash,
    )
}

#[test]
fn complete_payload_nonleak_after_amend_supersede_redact() {
    let dir = tempdir().unwrap();
    let (run_id, root, cp_id, _hash) = setup_full_checkpoint(dir.path());

    run_amend(
        Some(&root),
        run_id,
        AmendRequest {
            target_id: cp_id,
            target_kind: "checkpoint".into(),
            reason: "incomplete".into(),
            new_content: S_AMEND.into(),
            actor_category: ActorCategory::Agent,
        },
    )
    .unwrap();
    entry_supersede(
        Some(&root),
        run_id,
        SupersedeRequest {
            target_id: cp_id,
            target_kind: "checkpoint".into(),
            reason: "replace".into(),
            new_content: S_SUPER.into(),
            actor_category: ActorCategory::Agent,
        },
    )
    .unwrap();

    let finding = create_finding(
        Some(&root),
        run_id,
        CreateFindingRequest {
            kind: FindingKind::Clarification,
            body: "Finding body is allowed after redaction".into(),
            checkpoint_op_id: cp_id,
        },
    )
    .unwrap();
    let fid = finding.finding_id;
    respond_to_finding(
        Some(&root),
        run_id,
        fid,
        "Agent response is allowed",
        "c1-resp-1",
    )
    .unwrap();

    let red = entry_redact(
        Some(&root),
        run_id,
        RedactRequest {
            target_id: cp_id,
            target_kind: "checkpoint".into(),
            reason: "contains sentinel".into(),
            actor_category: ActorCategory::Human,
        },
    )
    .unwrap();
    assert_json_omits(&red, &all_needles());

    // Core ordinary surfaces
    let show = run_show(Some(&root), run_id, RunShowOptions::default()).unwrap();
    assert_json_omits(&show, &all_needles());
    assert!(show
        .recent_checkpoints
        .iter()
        .any(|c| c.summary == REDACTED_MARKER));

    let show_md = run_show(
        Some(&root),
        run_id,
        RunShowOptions {
            include_markdown: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_json_omits(&show_md, &all_needles());
    assert!(show_md.markdown.as_ref().unwrap().contains(REDACTED_MARKER));

    let listed = list_findings(Some(&root), run_id, false).unwrap();
    assert_json_omits(&listed, &all_needles());
    assert!(listed[0].target.target_redacted);

    let detail = get_finding(Some(&root), run_id, fid).unwrap();
    assert_json_omits(&detail, &all_needles());
    assert!(detail.target_snapshot.redacted);

    let replay = respond_to_finding(
        Some(&root),
        run_id,
        fid,
        "Agent response is allowed",
        "c1-resp-1",
    )
    .unwrap();
    assert!(replay.idempotent_replay);
    assert_json_omits(&replay, &all_needles());

    let ops = list_append_ops(Some(&root), run_id).unwrap();
    assert_json_omits(&ops, &all_needles());

    let md = super::project::find_run_by_id(&root, run_id).unwrap().0;
    let run_detail = load_run_detail(&md, Uuid::nil());
    assert_json_omits(&run_detail, &all_needles());

    let cps = super::findings::load_run_checkpoints_detail(&md).unwrap();
    assert_json_omits(&cps, &all_needles());

    // Canonical sidecar still retains content for integrity.
    let meta = load_run_meta_readonly(&md).unwrap().unwrap();
    let agent = meta.agent.as_ref().unwrap();
    let raw = serde_json::to_string(&agent.checkpoints).unwrap();
    assert!(
        raw.contains(S_SUM),
        "canonical must retain original summary"
    );
    let raw_ops = serde_json::to_string(&agent.append_only_ops).unwrap();
    assert!(
        raw_ops.contains(S_AMEND) || raw_ops.contains(S_SUPER) || raw_ops.contains(S_SUM),
        "canonical append ops retain prior content"
    );
}

#[test]
fn finding_after_redaction_allowed_with_redacted_target() {
    let dir = tempdir().unwrap();
    let (run_id, root, cp_id, _) = setup_full_checkpoint(dir.path());
    entry_redact(
        Some(&root),
        run_id,
        RedactRequest {
            target_id: cp_id,
            target_kind: "checkpoint".into(),
            reason: "pre-find redaction".into(),
            actor_category: ActorCategory::Human,
        },
    )
    .unwrap();
    let created = create_finding(
        Some(&root),
        run_id,
        CreateFindingRequest {
            kind: FindingKind::Other,
            body: "Finding after redaction is allowed".into(),
            checkpoint_op_id: cp_id,
        },
    )
    .unwrap();
    assert!(created.finding.target.target_redacted);
    assert_json_omits(&created, &all_needles());
}
