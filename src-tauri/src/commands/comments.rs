use std::path::PathBuf;

use moraine_core::{
    read_comments_sidecar, write_comments_sidecar, AnnotationKind, CommentRecord, CommentsFile,
};
use serde::{Deserialize, Serialize};

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
}

fn default_kind() -> String {
    "comment".into()
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
            kind: match c.kind {
                AnnotationKind::Suggestion => "suggestion".into(),
                AnnotationKind::Comment => "comment".into(),
            },
        }
    }
}

fn from_dto(c: CommentDto) -> Result<CommentRecord, String> {
    let id = uuid::Uuid::parse_str(&c.id).map_err(|e| e.to_string())?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&c.created_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .map_err(|e| e.to_string())?;
    let kind = if c.kind == "suggestion" {
        AnnotationKind::Suggestion
    } else {
        AnnotationKind::Comment
    };
    Ok(CommentRecord {
        id,
        body: c.body,
        author: c.author,
        quote: c.quote,
        created_at,
        resolved: c.resolved,
        kind,
    })
}

#[tauri::command]
pub fn load_comments(path: String) -> Result<Vec<CommentDto>, String> {
    let file = read_comments_sidecar(PathBuf::from(path).as_path()).map_err(|e| e.to_string())?;
    Ok(file.comments.into_iter().map(CommentDto::from).collect())
}

#[tauri::command]
pub fn save_comments(path: String, comments: Vec<CommentDto>) -> Result<(), String> {
    let mut records = Vec::with_capacity(comments.len());
    for c in comments {
        records.push(from_dto(c)?);
    }
    let file = CommentsFile {
        version: 1,
        comments: records,
    };
    write_comments_sidecar(PathBuf::from(path).as_path(), &file).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn comments_sidecar_path_cmd(path: String) -> String {
    moraine_core::moraine_sidecar_path(PathBuf::from(path).as_path())
        .display()
        .to_string()
}
