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
    capture_git_context, ensure_project, find_run_by_id, init_project, resolve_existing_project,
    resolve_or_init_project, run_checkpoint, run_ready, run_resume, run_show, run_start,
    AgentOpResult, AgentRunState, CheckpointInput, CheckpointRecord, GitContextSummary,
    ProjectInitResult, ProjectMeta, RunLifecycle, RunShowOptions, RunShowPacket, RunStartRequest,
    MAX_JSON_RESPONSE_HINT,
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
