use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("path not found: {0}")]
    NotFound(PathBuf),

    #[error("not a file: {0}")]
    NotAFile(PathBuf),

    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("invalid UTF-8 in file: {0}")]
    InvalidUtf8(PathBuf),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("watcher error: {0}")]
    Watcher(String),

    #[error("history error: {0}")]
    History(String),

    #[error("revision conflict: expected content hash {expected}, actual {actual}")]
    RevisionConflict { expected: String, actual: String },

    #[error("ledger busy: {0}")]
    LedgerBusy(String),

    #[error("annotation not found: {id}")]
    AnnotationNotFound { id: uuid::Uuid },

    #[error("duplicate annotation id: {id}")]
    DuplicateAnnotation { id: uuid::Uuid },

    #[error(
        "annotation conflict on {id}: expected revision {expected_revision}, actual {actual_revision}"
    )]
    AnnotationConflict {
        id: uuid::Uuid,
        expected_revision: u32,
        actual_revision: u32,
    },

    #[error("annotation precondition failed: {message}")]
    AnnotationPrecondition {
        id: Option<uuid::Uuid>,
        message: String,
    },

    #[error("invalid annotation kind for {id}: {message}")]
    InvalidAnnotationKind { id: uuid::Uuid, message: String },

    #[error("incomplete acceptance for {id}: {message}")]
    IncompleteAcceptance { id: uuid::Uuid, message: String },

    /// Markdown changed after acceptance began; cancel is unsafe.
    #[error(
        "acceptance document changed for {id}: base {base_content_hash}, current {current_content_hash}"
    )]
    AcceptanceDocumentChanged {
        id: uuid::Uuid,
        base_content_hash: String,
        current_content_hash: String,
    },

    #[error("annotation revision overflow for {id}")]
    RevisionOverflow { id: uuid::Uuid },

    #[error("project not found at {path}")]
    ProjectNotFound { path: PathBuf },

    #[error("run not found: {id}")]
    RunNotFound { id: uuid::Uuid },

    #[error("invalid checkpoint: {message}")]
    InvalidCheckpoint { message: String },

    #[error("idempotency conflict for key {key}: {message}")]
    IdempotencyConflict { key: String, message: String },

    #[error("run state conflict: expected {expected}, actual {actual}")]
    RunStateConflict { expected: String, actual: String },

    #[error("run record structure invalid: {message}")]
    RunRecordStructureInvalid { message: String },

    #[error("operation recovery required: {message}")]
    OperationRecoveryRequired { message: String },

    #[error("unsupported moraine schema version {version} (max {max})")]
    UnsupportedSchemaVersion { version: u32, max: u32 },

    #[error("idempotency index full (max {max})")]
    IdempotencyIndexFull { max: usize },

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::NotFound(_) => "not_found",
            Self::NotAFile(_) => "not_a_file",
            Self::NotADirectory(_) => "not_a_directory",
            Self::InvalidUtf8(_) => "invalid_utf8",
            Self::Serde(_) => "serde",
            Self::Watcher(_) => "watcher",
            Self::History(_) => "history",
            Self::RevisionConflict { .. } => "revision_conflict",
            Self::LedgerBusy(_) => "ledger_busy",
            Self::AnnotationNotFound { .. } => "annotation_not_found",
            Self::DuplicateAnnotation { .. } => "duplicate_annotation",
            Self::AnnotationConflict { .. } => "annotation_conflict",
            Self::AnnotationPrecondition { .. } => "annotation_precondition",
            Self::InvalidAnnotationKind { .. } => "invalid_annotation_kind",
            Self::IncompleteAcceptance { .. } => "incomplete_acceptance",
            Self::AcceptanceDocumentChanged { .. } => "acceptance_document_changed",
            Self::RevisionOverflow { .. } => "revision_overflow",
            Self::ProjectNotFound { .. } => "project_not_found",
            Self::RunNotFound { .. } => "run_not_found",
            Self::InvalidCheckpoint { .. } => "invalid_checkpoint",
            Self::IdempotencyConflict { .. } => "idempotency_conflict",
            Self::RunStateConflict { .. } => "run_state_conflict",
            Self::RunRecordStructureInvalid { .. } => "run_record_structure_invalid",
            Self::OperationRecoveryRequired { .. } => "operation_recovery_required",
            Self::UnsupportedSchemaVersion { .. } => "unsupported_schema_version",
            Self::IdempotencyIndexFull { .. } => "idempotency_index_full",
            Self::Other(_) => "error",
        }
    }

    /// Stable protocol error code for agent JSON envelopes.
    pub fn protocol_code(&self) -> &'static str {
        match self {
            Self::RevisionConflict { .. } => "revision_conflict",
            // Keep legacy kind alias for annotation/document consumers that
            // already check document_revision_conflict.
            other => other.kind_str(),
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::RevisionConflict { expected, actual } => serde_json::json!({
                "code": "revision_conflict",
                "kind": "document_revision_conflict",
                "expectedContentHash": expected,
                "actualContentHash": actual,
                "expectedHash": expected,
                "actualHash": actual,
                "message": "The Markdown file changed before the operation completed.",
            }),
            Self::LedgerBusy(msg) => serde_json::json!({
                "code": "ledger_busy",
                "kind": "ledger_busy",
                "message": msg,
            }),
            Self::AnnotationNotFound { id } => serde_json::json!({
                "code": "annotation_not_found",
                "kind": "annotation_not_found",
                "annotationId": id.to_string(),
                "message": "The annotation was not found in the ledger.",
            }),
            Self::DuplicateAnnotation { id } => serde_json::json!({
                "code": "duplicate_annotation",
                "kind": "duplicate_annotation",
                "annotationId": id.to_string(),
                "message": "An annotation with this id already exists.",
            }),
            Self::AnnotationConflict {
                id,
                expected_revision,
                actual_revision,
            } => serde_json::json!({
                "code": "annotation_conflict",
                "kind": "annotation_conflict",
                "annotationId": id.to_string(),
                "expectedRevision": expected_revision,
                "actualRevision": actual_revision,
                "message": "The annotation changed before the operation completed.",
            }),
            Self::AnnotationPrecondition { id, message } => serde_json::json!({
                "code": "annotation_precondition",
                "kind": "annotation_precondition",
                "annotationId": id.map(|u| u.to_string()),
                "message": message,
            }),
            Self::InvalidAnnotationKind { id, message } => serde_json::json!({
                "code": "invalid_annotation_kind",
                "kind": "invalid_annotation_kind",
                "annotationId": id.to_string(),
                "message": message,
            }),
            Self::IncompleteAcceptance { id, message } => serde_json::json!({
                "code": "incomplete_acceptance",
                "kind": "incomplete_acceptance",
                "annotationId": id.to_string(),
                "message": message,
            }),
            Self::AcceptanceDocumentChanged {
                id,
                base_content_hash,
                current_content_hash,
            } => serde_json::json!({
                "code": "acceptance_document_changed",
                "kind": "acceptance_document_changed",
                "annotationId": id.to_string(),
                "baseContentHash": base_content_hash,
                "currentContentHash": current_content_hash,
                "message": "The Markdown changed after acceptance began. Finalize the acceptance or restore the original document revision before cancelling.",
            }),
            Self::RevisionOverflow { id } => serde_json::json!({
                "code": "revision_overflow",
                "kind": "revision_overflow",
                "annotationId": id.to_string(),
                "message": "Annotation revision cannot be advanced further.",
            }),
            Self::ProjectNotFound { path } => serde_json::json!({
                "code": "project_not_found",
                "kind": "project_not_found",
                "path": path.display().to_string(),
                "message": "No Moraine project was found at this path.",
            }),
            Self::RunNotFound { id } => serde_json::json!({
                "code": "run_not_found",
                "kind": "run_not_found",
                "runId": id.to_string(),
                "message": "No run with this id was found in the project.",
            }),
            Self::InvalidCheckpoint { message } => serde_json::json!({
                "code": "invalid_checkpoint",
                "kind": "invalid_checkpoint",
                "message": message,
            }),
            Self::IdempotencyConflict { key, message } => serde_json::json!({
                "code": "idempotency_conflict",
                "kind": "idempotency_conflict",
                "idempotencyKey": key,
                "message": message,
            }),
            Self::RunStateConflict { expected, actual } => serde_json::json!({
                "code": "run_state_conflict",
                "kind": "run_state_conflict",
                "expected": expected,
                "actual": actual,
                "message": "The run lifecycle state does not allow this operation.",
            }),
            Self::RunRecordStructureInvalid { message } => serde_json::json!({
                "code": "run_record_structure_invalid",
                "kind": "run_record_structure_invalid",
                "message": message,
            }),
            Self::OperationRecoveryRequired { message } => serde_json::json!({
                "code": "operation_recovery_required",
                "kind": "operation_recovery_required",
                "message": message,
            }),
            Self::UnsupportedSchemaVersion { version, max } => serde_json::json!({
                "code": "unsupported_schema_version",
                "kind": "unsupported_schema_version",
                "version": version,
                "max": max,
                "message": format!("Unsupported moraine sidecar schema version {version} (max {max})"),
            }),
            Self::IdempotencyIndexFull { max } => serde_json::json!({
                "code": "idempotency_index_full",
                "kind": "idempotency_index_full",
                "max": max,
                "message": format!("Idempotency index is full (max {max}); refuse silent eviction"),
            }),
            other => serde_json::json!({
                "code": other.kind_str(),
                "kind": other.kind_str(),
                "message": other.to_string(),
            }),
        }
    }
}
