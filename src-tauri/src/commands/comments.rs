use std::path::PathBuf;

use moraine_core::{
    accept_suggestion, create_annotation, read_comments_sidecar, reconcile_session_annotations,
    reject_suggestion, reopen_annotation, resolve_annotation, update_annotation, AnnotationKind,
    CommentRecord, Error as CoreError,
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
        }
    }
}

fn map_err(e: CoreError) -> String {
    // Prefer machine-readable kind prefix for frontend detection.
    match &e {
        CoreError::AnnotationConflict {
            id,
            expected_revision,
            actual_revision,
        } => format!(
            "annotation_conflict: id={id} expectedRevision={expected_revision} actualRevision={actual_revision}"
        ),
        CoreError::AnnotationNotFound { id } => format!("annotation_not_found: id={id}"),
        CoreError::DuplicateAnnotation { id } => format!("duplicate_annotation: id={id}"),
        CoreError::AnnotationPrecondition { message, .. } => {
            format!("annotation_precondition: {message}")
        }
        CoreError::InvalidAnnotationKind { id, message } => {
            format!("invalid_annotation_kind: id={id} {message}")
        }
        other => other.to_string(),
    }
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
    Ok(CommentRecord {
        id,
        body: c.body,
        author: c.author,
        quote: c.quote,
        created_at,
        resolved: c.resolved,
        kind,
        revision: if c.revision == 0 { 1 } else { c.revision },
    })
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
pub fn accept_suggestion_cmd(
    path: String,
    id: String,
    expected_revision: u32,
) -> Result<AnnotationOpDto, String> {
    let id = parse_id(&id)?;
    let r =
        accept_suggestion(PathBuf::from(path).as_path(), id, expected_revision).map_err(map_err)?;
    Ok(op_dto(r))
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

/// Safe host merge of live-session annotations: no full-list replace, no deletes.
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
