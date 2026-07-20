//! Agent run protocol: project discovery, structured checkpoints, lifecycle.
//!
//! Markdown is a durable human-readable projection; structured state lives in the
//! sidecar `agent` object. Mutations use the same per-record lock and revision hash
//! discipline as annotation ops.

mod git;
mod markdown;
mod ops;
mod project;
mod session;
mod types;

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
    resolve_existing_project, resolve_or_init_project, ProjectInitResult, ProjectMeta,
    StartOpIndex, StartOpStatus, MORAINE_DIR, PROJECT_SCHEMA_VERSION,
};
pub use session::{
    session_observe, SessionObserveRequest, SessionObserveResult, SessionRecord,
    SESSION_SCHEMA_VERSION,
};
pub use types::{
    AgentRunState, CaptureCoverage, CheckpointRecord, EvidenceItem, EvidenceKind,
    EvidenceProvenance, IdempotencyRecord, IncompleteOp, LifecycleEvent, RationalItem,
    RunLifecycle, MAX_CHECKPOINT_ITEMS, MAX_FIELD_CHARS, MAX_SUMMARY_CHARS,
};
