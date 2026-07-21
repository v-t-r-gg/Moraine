//! Domain library for Moraine run records. No Tauri / Axum / UI deps.
//!
//! One Markdown run record path maps to one live review room.
//! Structured review state lives in `file.md.moraine.json`.

pub mod agent_protocol;
pub mod annotation_ops;
pub mod atomic;
pub mod comments;
pub mod discovery;
pub mod document;
pub mod error;
pub mod history;
pub mod paths;
pub mod room;
pub mod run_meta;
pub mod share;
pub mod watcher;

pub use agent_protocol::{
    capture_git_context, change_finding_state, change_finding_state_at_path, create_finding,
    create_finding_at_path, current_checkpoint_claim, derive_capture_coverage, ensure_project,
    entry_redact, entry_redact_at_path, entry_supersede, entry_supersede_at_path, find_run_by_id,
    get_finding, get_finding_at_path, human_observation_add, human_observation_add_at_path,
    init_project, is_redacted, list_append_ops, list_append_ops_at_path, list_findings,
    list_findings_at_path, load_evidence_record, load_run_checkpoints_detail, load_session,
    namespace_session_key, project_target_context, project_target_snapshot, provisional_run_ensure,
    record_mechanical_evidence, redact_secrets, resolve_confined_project, resolve_existing_project,
    resolve_or_init_project, respond_to_finding, respond_to_finding_at_path, run_amend,
    run_amend_at_path, run_checkpoint, run_ready, run_resume, run_show, run_start, session_observe,
    ActorCategory, AgentOpResult, AgentRunState, AmendRequest, AppendOnlyOpRecord, AppendOpResult,
    CaptureCoverage, CheckpointInput, CheckpointRecord, CheckpointSummaryDto, CreateFindingRequest,
    EvidenceItem, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceSummary, FindingDetail,
    FindingKind, FindingLedgerEvent, FindingListItem, FindingMutationResult, FindingRecord,
    FindingResponse, FindingState, FindingTarget, FindingTargetContext, FindingTargetKind,
    FindingTargetSnapshotDto, FindingThreadItem, GitContextSummary, HumanObservationRequest,
    LedgerRelationship, MechanicalEvidenceRequest, OutputMetadata, ProjectInitResult, ProjectMeta,
    ProvisionalRunRequest, RationalItem, RedactRequest, RunCheckpointsDetail, RunLifecycle,
    RunShowOptions, RunShowPacket, RunStartRequest, SessionObserveRequest, SessionObserveResult,
    SessionRecord, SupersedeRequest, FINDING_EVENT_CREATED, FINDING_EVENT_RESPONDED,
    FINDING_EVENT_STATE_CHANGED, MAX_FINDING_BODY_CHARS, MAX_JSON_RESPONSE_HINT, MAX_OP_BODY_CHARS,
    OP_ENTRY_REDACT, OP_ENTRY_SUPERSEDE, OP_HUMAN_OBSERVATION_ADD, OP_RUN_AMEND,
    REDACTED_CHECKPOINT_SUMMARY,
};
pub use annotation_ops::{
    acceptance_recovery_status, apply_mutation, begin_accept_suggestion, cancel_accept_suggestion,
    complete_accept_suggestion, create_annotation, reconcile_session_annotations,
    reject_suggestion, reopen_annotation, resolve_annotation, update_annotation,
    AcceptanceRecoveryStatus, AnnotationMutation, AnnotationOpResult, BeginAcceptResult,
    ReconcileResult,
};
#[allow(deprecated)]
pub use comments::{
    comments_sidecar_path, merge_comments, read_comments_sidecar, write_comments_sidecar,
    AnnotationKind, CommentRecord, CommentsFile, SuggestionDisposition,
};
pub use discovery::{
    build_timeline, canonicalize_existing, dedupe_project_roots, filter_runs, filter_runs_ext,
    list_run_summaries, load_run_detail, project_display_name, scan_project_roots,
    summarize_project, summarize_run_path, ProjectRunCounts, ProjectSummary, RunDetail,
    RunIntegrity, RunSummary, TimelineEntry,
};
pub use document::{Document, DocumentId, DocumentMeta, DocumentSnapshot};
pub use error::{Error, Result};
pub use history::{HistoryEntry, HistoryStore};
pub use paths::MorainePaths;
pub use room::{room_id_for_path, room_id_for_str};
pub use run_meta::{
    assert_disk_revision, comments_migrated_path, content_hash, content_hash_file, ensure_run_meta,
    load_run_meta, load_run_meta_readonly, moraine_sidecar_path, record_decision, review_snapshot,
    status_snapshot, write_run_meta, DecisionKind, ReviewDecision, ReviewSnapshot, ReviewStateKind,
    RunInfo, RunMeta, SCHEMA_VERSION,
};
pub use share::{
    bind_from_http, http_to_ws, share_links, ShareLinks, DEFAULT_RELAY_BIND, DEFAULT_RELAY_HTTP,
    DEFAULT_RELAY_WS, DEFAULT_UI,
};
pub use watcher::{FileChange, FileWatcher, WatchEvent};
