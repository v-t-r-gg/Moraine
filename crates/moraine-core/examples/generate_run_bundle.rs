//! Generate the committed current-format run-bundle example through public core APIs.

use std::fs;
use std::path::PathBuf;

use moraine_core::{
    create_finding, entry_redact, entry_supersede, find_run_by_id, human_observation_add,
    run_amend, run_checkpoint, run_start, ActorCategory, AmendRequest, CheckpointInput,
    CreateFindingRequest, EvidenceItem, EvidenceKind, EvidenceProvenance, FindingKind,
    HumanObservationRequest, RedactRequest, RunStartRequest, SupersedeRequest,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: generate_run_bundle <output-directory>")?;
    fs::create_dir_all(&output)?;

    let project = tempfile::tempdir()?;
    let start = run_start(RunStartRequest {
        objective: "Document a current Moraine run bundle".into(),
        idempotency_key: "example-start".into(),
        project: Some(project.path().to_path_buf()),
        session_id: Some("example-session".into()),
    })?;
    let checkpoint = run_checkpoint(
        Some(project.path()),
        start.run_id,
        &start.content_hash,
        "example-checkpoint",
        CheckpointInput {
            summary: "Generated the run-bundle fixture through public core operations.".into(),
            actions: vec!["Created a schema-current sidecar & Markdown projection.".into()],
            rationales: vec![],
            evidence: vec![EvidenceItem {
                kind: EvidenceKind::CommandResult,
                label: "Core fixture validation passed".into(),
                command: Some("cargo test -p moraine-core --test run_bundle_fixture".into()),
                exit_code: Some(0),
                path: None,
                url: None,
                provenance: EvidenceProvenance::AgentReported,
            }],
            risks: vec!["Generated IDs & timestamps change when refreshed.".into()],
            open_questions: vec![],
        },
    )?;
    let checkpoint_id = checkpoint.op_id.ok_or("checkpoint missing operation id")?;

    create_finding(
        Some(project.path()),
        start.run_id,
        CreateFindingRequest {
            kind: FindingKind::Clarification,
            body: "Keep the generator beside the fixture so its provenance stays clear.".into(),
            checkpoint_op_id: checkpoint_id,
        },
    )?;
    human_observation_add(
        Some(project.path()),
        start.run_id,
        HumanObservationRequest {
            body: "The fixture is documentation, not a merge verdict.".into(),
            reason: "Record the product boundary.".into(),
            target_id: Some(checkpoint_id),
            target_kind: Some("checkpoint".into()),
        },
    )?;
    run_amend(
        Some(project.path()),
        start.run_id,
        AmendRequest {
            target_id: checkpoint_id,
            target_kind: "checkpoint".into(),
            reason: "Clarify that both artifacts are generated.".into(),
            new_content: "Generated & validated the current Markdown and sidecar bundle.".into(),
            actor_category: ActorCategory::Agent,
        },
    )?;
    entry_supersede(
        Some(project.path()),
        start.run_id,
        SupersedeRequest {
            target_id: checkpoint_id,
            target_kind: "checkpoint".into(),
            reason: "Use the final concise statement.".into(),
            new_content: "Current run-bundle fixture generated & validated.".into(),
            actor_category: ActorCategory::Agent,
        },
    )?;
    entry_redact(
        Some(project.path()),
        start.run_id,
        RedactRequest {
            target_id: checkpoint_id,
            target_kind: "checkpoint".into(),
            reason: "Demonstrate ordinary-view withholding.".into(),
            actor_category: ActorCategory::Human,
        },
    )?;

    let (markdown, _) = find_run_by_id(project.path(), start.run_id)?;
    let sidecar = moraine_core::moraine_sidecar_path(&markdown);
    fs::copy(markdown, output.join("run.md"))?;
    fs::copy(sidecar, output.join("run.md.moraine.json"))?;
    Ok(())
}
