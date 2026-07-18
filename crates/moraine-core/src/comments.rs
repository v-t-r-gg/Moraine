//! Annotation types and legacy `*.md.comments.json` reader/writer.
//! Prefer `run_meta` for the durable ledger; this module remains for migration.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

pub fn comments_sidecar_path(md_path: &Path) -> PathBuf {
    let mut s = md_path.as_os_str().to_os_string();
    s.push(".comments.json");
    PathBuf::from(s)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationKind {
    #[default]
    Comment,
    Suggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentRecord {
    pub id: Uuid,
    pub body: String,
    pub author: String,
    pub quote: String,
    pub created_at: DateTime<Utc>,
    pub resolved: bool,
    #[serde(default)]
    pub kind: AnnotationKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentsFile {
    pub version: u32,
    pub comments: Vec<CommentRecord>,
}

impl Default for CommentsFile {
    fn default() -> Self {
        Self {
            version: 1,
            comments: Vec::new(),
        }
    }
}

pub fn read_comments_sidecar(md_path: &Path) -> Result<CommentsFile> {
    // Prefer unified ledger when present (read-only; does not migrate).
    if let Some(meta) = crate::run_meta::load_run_meta_readonly(md_path)? {
        return Ok(crate::run_meta::comments_from_meta(&meta));
    }
    let path = comments_sidecar_path(md_path);
    if !path.exists() {
        return Ok(CommentsFile::default());
    }
    let raw = fs::read_to_string(&path)?;
    let file: CommentsFile = serde_json::from_str(&raw)?;
    Ok(file)
}

pub fn write_comments_sidecar(md_path: &Path, file: &CommentsFile) -> Result<()> {
    // Write into unified ledger (creates run id if needed).
    crate::run_meta::set_comments_and_save(md_path, file.comments.clone())?;
    Ok(())
}

pub fn merge_comments(into: &mut CommentsFile, from_disk: &CommentsFile) {
    for c in &from_disk.comments {
        if !into.comments.iter().any(|x| x.id == c.id) {
            into.comments.push(c.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sidecar_path() {
        let p = Path::new("/tmp/note.md");
        assert_eq!(
            comments_sidecar_path(p),
            PathBuf::from("/tmp/note.md.comments.json")
        );
    }

    #[test]
    fn write_goes_to_moraine_json() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("a.md");
        fs::write(&md, "# a\n").unwrap();
        let id = Uuid::new_v4();
        let file = CommentsFile {
            version: 1,
            comments: vec![CommentRecord {
                id,
                body: "hi".into(),
                author: "A".into(),
                quote: "a".into(),
                created_at: Utc::now(),
                resolved: false,
                kind: AnnotationKind::Comment,
            }],
        };
        write_comments_sidecar(&md, &file).unwrap();
        assert!(crate::run_meta::moraine_sidecar_path(&md).exists());
        let loaded = read_comments_sidecar(&md).unwrap();
        assert_eq!(loaded.comments[0].body, "hi");
    }
}
