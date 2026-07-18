use std::path::PathBuf;

use moraine_core::{read_comments_sidecar, write_comments_sidecar, CommentRecord, CommentsFile};
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
        }
    }
}

fn from_dto(c: CommentDto) -> Result<CommentRecord, String> {
    let id = uuid::Uuid::parse_str(&c.id).map_err(|e| e.to_string())?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&c.created_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .map_err(|e| e.to_string())?;
    Ok(CommentRecord {
        id,
        body: c.body,
        author: c.author,
        quote: c.quote,
        created_at,
        resolved: c.resolved,
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
    moraine_core::comments_sidecar_path(PathBuf::from(path).as_path())
        .display()
        .to_string()
}
