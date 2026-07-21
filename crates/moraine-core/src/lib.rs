//! Domain library for Moraine run records. No Tauri / Axum / UI deps.
//!
//! One Markdown run record path maps to one live review room.
//! Structured review state lives in `file.md.moraine.json`.

pub mod agent_protocol;
pub mod annotation_ops;
pub mod atomic;
pub mod comments;
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
    create_finding_at_path, derive_capture_coverage, ensure_project, find_run_by_id, get_finding,
    get_finding_at_path, init_project, list_findings, list_findings_at_path, load_evidence_record,
    load_run_checkpoints_detail, load_session, namespace_session_key, provisional_run_ensure,
    record_mechanical_evidence, redact_secrets, resolve_confined_project, resolve_existing_project,
    resolve_or_init_project, respond_to_finding, respond_to_finding_at_path, run_checkpoint,
    run_ready, run_resume, run_show, run_start, session_observe, AgentOpResult, AgentRunState,
    CaptureCoverage, CheckpointInput, CheckpointRecord, CheckpointSummaryDto, CreateFindingRequest,
    EvidenceItem, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceSummary, FindingDetail,
    FindingKind, FindingLedgerEvent, FindingListItem, FindingMutationResult, FindingRecord,
    FindingResponse, FindingState, FindingTarget, FindingTargetContext, FindingTargetKind,
    FindingThreadItem, GitContextSummary, MechanicalEvidenceRequest, OutputMetadata,
    ProjectInitResult, ProjectMeta, ProvisionalRunRequest, RationalItem, RunCheckpointsDetail,
    RunLifecycle, RunShowOptions, RunShowPacket, RunStartRequest, SessionObserveRequest,
    SessionObserveResult, SessionRecord, FINDING_EVENT_CREATED, FINDING_EVENT_RESPONDED,
    FINDING_EVENT_STATE_CHANGED, MAX_FINDING_BODY_CHARS, MAX_JSON_RESPONSE_HINT,
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
