use std::path::PathBuf;

use moraine_core::{
    acceptance_recovery_status, begin_accept_suggestion, cancel_accept_suggestion,
    complete_accept_suggestion, create_annotation, read_comments_sidecar,
    reconcile_session_annotations, reject_suggestion, reopen_annotation, resolve_annotation,
    update_annotation, AnnotationKind, CommentRecord, Error as CoreError, SuggestionDisposition,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentDto {
    pub id: String,
    pub body: String,
    pub author: String,
    pub quote: String,
    pub created_at: String,
    pub resolved: bool,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_revision")]
    pub revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_op_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_base_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_completed_at: Option<String>,
}

fn default_kind() -> String {
    "comment".into()
}

fn default_revision() -> u32 {
    1
}

impl From<CommentRecord> for CommentDto {
    fn from(c: CommentRecord) -> Self {
        Self {
            id: c.id.to_string(),
            body: c.body,
            author: c.author,
            quote: c.quote,
            created_at: c.created_at.to_rfc3339(),
            resolved: c.resolved,
            kind: c.kind.as_str().into(),
            revision: c.revision,
            disposition: c.disposition.map(|d| d.as_str().into()),
            acceptance_op_id: c.acceptance_op_id.map(|u| u.to_string()),
            acceptance_base_hash: c.acceptance_base_hash,
            acceptance_started_at: c.acceptance_started_at.map(|t| t.to_rfc3339()),
            applied_content_hash: c.applied_content_hash,
            acceptance_completed_at: c.acceptance_completed_at.map(|t| t.to_rfc3339()),
        }
    }
}

/// Structured error returned to the frontend (JSON string of this object).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationErrorDto {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_revision: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_content_hash: Option<String>,
    pub message: String,
}

fn map_err(e: CoreError) -> String {
    let dto = match &e {
        CoreError::AnnotationConflict {
            id,
            expected_revision,
            actual_revision,
        } => AnnotationErrorDto {
            kind: "annotation_conflict".into(),
            annotation_id: Some(id.to_string()),
            expected_revision: Some(*expected_revision),
            actual_revision: Some(*actual_revision),
            expected_content_hash: None,
            actual_content_hash: None,
            message: "The annotation changed before the operation completed.".into(),
        },
        CoreError::RevisionConflict { expected, actual } => AnnotationErrorDto {
            kind: "document_revision_conflict".into(),
            annotation_id: None,
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: Some(expected.clone()),
            actual_content_hash: Some(actual.clone()),
            message: "The Markdown file changed before the operation completed.".into(),
        },
        CoreError::AnnotationNotFound { id } => AnnotationErrorDto {
            kind: "annotation_not_found".into(),
            annotation_id: Some(id.to_string()),
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: None,
            actual_content_hash: None,
            message: "The annotation was not found in the ledger.".into(),
        },
        CoreError::DuplicateAnnotation { id } => AnnotationErrorDto {
            kind: "duplicate_annotation".into(),
            annotation_id: Some(id.to_string()),
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: None,
            actual_content_hash: None,
            message: "An annotation with this id already exists.".into(),
        },
        CoreError::AnnotationPrecondition { id, message } => AnnotationErrorDto {
            kind: "annotation_precondition".into(),
            annotation_id: id.map(|u| u.to_string()),
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: None,
            actual_content_hash: None,
            message: message.clone(),
        },
        CoreError::InvalidAnnotationKind { id, message } => AnnotationErrorDto {
            kind: "invalid_annotation_kind".into(),
            annotation_id: Some(id.to_string()),
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: None,
            actual_content_hash: None,
            message: message.clone(),
        },
        CoreError::IncompleteAcceptance { id, message } => AnnotationErrorDto {
            kind: "incomplete_acceptance".into(),
            annotation_id: Some(id.to_string()),
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: None,
            actual_content_hash: None,
            message: message.clone(),
        },
        CoreError::AcceptanceDocumentChanged {
            id,
            base_content_hash,
            current_content_hash,
        } => AnnotationErrorDto {
            kind: "acceptance_document_changed".into(),
            annotation_id: Some(id.to_string()),
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: Some(base_content_hash.clone()),
            actual_content_hash: Some(current_content_hash.clone()),
            message: "The Markdown changed after acceptance began. Finalize the acceptance or restore the original document revision before cancelling.".into(),
        },
        CoreError::RevisionOverflow { id } => AnnotationErrorDto {
            kind: "revision_overflow".into(),
            annotation_id: Some(id.to_string()),
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: None,
            actual_content_hash: None,
            message: "Annotation revision cannot be advanced further.".into(),
        },
        other => AnnotationErrorDto {
            kind: other.kind_str().into(),
            annotation_id: None,
            expected_revision: None,
            actual_revision: None,
            expected_content_hash: None,
            actual_content_hash: None,
            message: other.to_string(),
        },
    };
    serde_json::to_string(&dto).unwrap_or_else(|_| e.to_string())
}

fn parse_kind(s: &str) -> Result<AnnotationKind, String> {
    AnnotationKind::parse(s).ok_or_else(|| format!("invalid annotation kind: {s}"))
}

fn parse_id(s: &str) -> Result<Uuid, String> {
    Uuid::parse_str(s).map_err(|e| e.to_string())
}

fn from_dto(c: CommentDto) -> Result<CommentRecord, String> {
    let id = parse_id(&c.id)?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&c.created_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .map_err(|e| e.to_string())?;
    let kind = parse_kind(&c.kind)?;
    let disposition = c
        .disposition
        .as_deref()
        .and_then(SuggestionDisposition::parse);
    let mut rec = CommentRecord {
        id,
        body: c.body,
        author: c.author,
        quote: c.quote,
        created_at,
        resolved: c.resolved,
        kind,
        revision: if c.revision == 0 { 1 } else { c.revision },
        disposition,
        acceptance_op_id: c
            .acceptance_op_id
            .as_deref()
            .and_then(|s| Uuid::parse_str(s).ok()),
        acceptance_base_hash: c.acceptance_base_hash,
        acceptance_started_at: c.acceptance_started_at.as_deref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc))
        }),
        applied_content_hash: c.applied_content_hash,
        acceptance_completed_at: c.acceptance_completed_at.as_deref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc))
        }),
    };
    rec.normalize_compat();
    Ok(rec)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationOpDto {
    pub annotation: CommentDto,
    pub comments: Vec<CommentDto>,
    pub run_id: String,
}

fn op_dto(r: moraine_core::AnnotationOpResult) -> AnnotationOpDto {
    AnnotationOpDto {
        annotation: r.annotation.into(),
        comments: r.comments.into_iter().map(CommentDto::from).collect(),
        run_id: r.run_id.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginAcceptDto {
    pub annotation: CommentDto,
    pub comments: Vec<CommentDto>,
    pub run_id: String,
    pub acceptance_op_id: String,
    pub base_content_hash: String,
}

#[tauri::command]
pub fn load_comments(path: String) -> Result<Vec<CommentDto>, String> {
    let file = read_comments_sidecar(PathBuf::from(path).as_path()).map_err(map_err)?;
    Ok(file.comments.into_iter().map(CommentDto::from).collect())
}

#[tauri::command]
pub fn create_annotation_cmd(
    path: String,
    id: String,
    body: String,
    author: String,
    quote: String,
    kind: String,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let kind = parse_kind(&kind)?;
    let r = create_annotation(PathBuf::from(path).as_path(), id, body, author, quote, kind)
        .map_err(map_err)?;
    Ok(op_dto(r))
}

#[tauri::command]
pub fn update_annotation_cmd(
    path: String,
    id: String,
    expected_revision: u32,
    body: Option<String>,
    author: Option<String>,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let r = update_annotation(
        PathBuf::from(path).as_path(),
        id,
        expected_revision,
        body,
        author,
    )
    .map_err(map_err)?;
    Ok(op_dto(r))
}

#[tauri::command]
pub fn resolve_annotation_cmd(
    path: String,
    id: String,
    expected_revision: u32,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let r = resolve_annotation(PathBuf::from(path).as_path(), id, expected_revision)
        .map_err(map_err)?;
    Ok(op_dto(r))
}

#[tauri::command]
pub fn reopen_annotation_cmd(
    path: String,
    id: String,
    expected_revision: u32,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let r =
        reopen_annotation(PathBuf::from(path).as_path(), id, expected_revision).map_err(map_err)?;
    Ok(op_dto(r))
}

#[tauri::command]
pub fn begin_accept_suggestion_cmd(
    path: String,
    id: String,
    expected_revision: u32,
    expected_content_hash: String,
) -> Result<BeginAcceptDto, String> {
    let id = parse_id(&id)?;
    let r = begin_accept_suggestion(
        PathBuf::from(path).as_path(),
        id,
        expected_revision,
        &expected_content_hash,
    )
    .map_err(map_err)?;
    Ok(BeginAcceptDto {
        annotation: r.annotation.into(),
        comments: r.comments.into_iter().map(CommentDto::from).collect(),
        run_id: r.run_id.to_string(),
        acceptance_op_id: r.acceptance_op_id.to_string(),
        base_content_hash: r.base_content_hash,
    })
}

#[tauri::command]
pub fn complete_accept_suggestion_cmd(
    path: String,
    id: String,
    expected_revision: u32,
    acceptance_op_id: String,
    expected_saved_hash: String,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let op = parse_id(&acceptance_op_id)?;
    let r = complete_accept_suggestion(
        PathBuf::from(path).as_path(),
        id,
        expected_revision,
        op,
        &expected_saved_hash,
    )
    .map_err(map_err)?;
    Ok(op_dto(r))
}

#[tauri::command]
pub fn cancel_accept_suggestion_cmd(
    path: String,
    id: String,
    expected_revision: u32,
    acceptance_op_id: String,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let op = parse_id(&acceptance_op_id)?;
    let r = cancel_accept_suggestion(PathBuf::from(path).as_path(), id, expected_revision, op)
        .map_err(map_err)?;
    Ok(op_dto(r))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceRecoveryDto {
    pub annotation_id: String,
    pub disposition: String,
    pub revision: u32,
    pub acceptance_op_id: Option<String>,
    pub base_content_hash: Option<String>,
    pub current_content_hash: String,
    pub cancel_safe: bool,
}

#[tauri::command]
pub fn acceptance_recovery_status_cmd(
    path: String,
    id: String,
) -> Result<AcceptanceRecoveryDto, String> {
    let id = parse_id(&id)?;
    let st = acceptance_recovery_status(PathBuf::from(path).as_path(), id).map_err(map_err)?;
    Ok(AcceptanceRecoveryDto {
        annotation_id: st.annotation_id.to_string(),
        disposition: st.disposition.as_str().into(),
        revision: st.revision,
        acceptance_op_id: st.acceptance_op_id.map(|u| u.to_string()),
        base_content_hash: st.base_content_hash,
        current_content_hash: st.current_content_hash,
        cancel_safe: st.cancel_safe,
    })
}

#[tauri::command]
pub fn reject_suggestion_cmd(
    path: String,
    id: String,
    expected_revision: u32,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let r =
        reject_suggestion(PathBuf::from(path).as_path(), id, expected_revision).map_err(map_err)?;
    Ok(op_dto(r))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileDto {
    pub comments: Vec<CommentDto>,
    pub created: usize,
    pub updated: usize,
    pub conflicts: Vec<ReconcileConflictDto>,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileConflictDto {
    pub annotation_id: String,
    pub expected_revision: u32,
    pub actual_revision: u32,
    pub message: String,
}

#[tauri::command]
pub fn reconcile_session_annotations_cmd(
    path: String,
    comments: Vec<CommentDto>,
) -> Result<ReconcileDto, String> {
    let mut records = Vec::with_capacity(comments.len());
    for c in comments {
        records.push(from_dto(c)?);
    }
    let r =
        reconcile_session_annotations(PathBuf::from(path).as_path(), &records).map_err(map_err)?;
    Ok(ReconcileDto {
        comments: r.comments.into_iter().map(CommentDto::from).collect(),
        created: r.created,
        updated: r.updated,
        conflicts: r
            .conflicts
            .into_iter()
            .map(|c| ReconcileConflictDto {
                annotation_id: c.annotation_id.to_string(),
                expected_revision: c.expected_revision,
                actual_revision: c.actual_revision,
                message: c.message,
            })
            .collect(),
        run_id: r.run_id.to_string(),
    })
}

#[tauri::command]
pub fn comments_sidecar_path_cmd(path: String) -> String {
    moraine_core::moraine_sidecar_path(PathBuf::from(path).as_path())
        .display()
        .to_string()
}
