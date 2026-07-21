//! Agent run protocol: project discovery, structured checkpoints, lifecycle.
//!
//! Markdown is a durable human-readable projection; structured state lives in the
//! sidecar `agent` object. Mutations use the same per-record lock and revision hash
//! discipline as annotation ops.

mod append_ops;
mod evidence;
mod findings;
mod git;
mod markdown;
mod ops;
mod project;
mod session;
mod types;

pub use append_ops::{
    current_checkpoint_claim, entry_redact, entry_redact_at_path, entry_supersede,
    entry_supersede_at_path, human_observation_add, human_observation_add_at_path, is_redacted,
    list_append_ops, list_append_ops_at_path, run_amend, run_amend_at_path, AmendRequest,
    AppendOpResult, HumanObservationRequest, RedactRequest, SupersedeRequest, MAX_OP_BODY_CHARS,
};
pub use evidence::{
    load_evidence_record, record_mechanical_evidence, redact_secrets, EvidenceRecord,
    MechanicalEvidenceRequest, OutputMetadata, EVIDENCE_DIR, EVIDENCE_SCHEMA_VERSION,
    MAX_COMMAND_LEN, MAX_OUTPUT_BYTES,
};
pub use findings::{
    change_finding_state, change_finding_state_at_path, create_finding, create_finding_at_path,
    get_finding, get_finding_at_path, list_findings, list_findings_at_path,
    load_run_checkpoints_detail, project_target_context, project_target_snapshot,
    respond_to_finding, respond_to_finding_at_path, CheckpointSummaryDto, CreateFindingRequest,
    FindingDetail, FindingListItem, FindingMutationResult, FindingTargetContext,
    FindingTargetSnapshotDto, FindingThreadItem, RunCheckpointsDetail, MAX_FINDING_BODY_CHARS,
    REDACTED_CHECKPOINT_SUMMARY,
};
pub use git::{capture_git_context, GitContextSummary};
pub use markdown::{
    extract_human_notes, human_notes_body_start, render_run_markdown_with_id, HUMAN_NOTES_HEADING,
};
pub use ops::{
    provisional_run_ensure, run_checkpoint, run_ready, run_resume, run_show, run_start,
    AgentOpResult, CheckpointInput, ProvisionalRunRequest, RunShowOptions, RunShowPacket,
    RunStartRequest, MAX_JSON_RESPONSE_HINT,
};
pub use project::{
    discover_project_root, ensure_project, find_run_by_id, init_project, project_meta_path,
    resolve_existing_project, resolve_or_init_project, runs_dir, ProjectInitResult, ProjectMeta,
    StartOpIndex, StartOpStatus, MORAINE_DIR, PROJECT_SCHEMA_VERSION,
};
pub use session::{
    derive_capture_coverage, load_session, namespace_session_key, resolve_confined_project,
    session_observe, SessionObserveRequest, SessionObserveResult, SessionRecord,
    SESSION_SCHEMA_VERSION,
};
pub use types::{
    ActorCategory, AgentRunState, AppendOnlyOpRecord, CaptureCoverage, CheckpointRecord,
    EvidenceItem, EvidenceKind, EvidenceProvenance, EvidenceSummary, FindingKind,
    FindingLedgerEvent, FindingRecord, FindingResponse, FindingState, FindingTarget,
    FindingTargetKind, IdempotencyRecord, IncompleteOp, LedgerRelationship, LifecycleEvent,
    RationalItem, RunLifecycle, FINDING_EVENT_CREATED, FINDING_EVENT_RESPONDED,
    FINDING_EVENT_STATE_CHANGED, MAX_CHECKPOINT_ITEMS, MAX_FIELD_CHARS, MAX_SUMMARY_CHARS,
    OP_ENTRY_REDACT, OP_ENTRY_SUPERSEDE, OP_HUMAN_OBSERVATION_ADD, OP_RUN_AMEND,
};
