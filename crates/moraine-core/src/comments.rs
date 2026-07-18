//! Comment metadata sidecar next to a Markdown file (`note.md.comments.json`).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

pub fn comments_sidecar_path(md_path: &Path) -> PathBuf {
    let mut s = md_path.as_os_str().to_os_string();
    s.push(".comments.json");
    PathBuf::from(s)
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
    let path = comments_sidecar_path(md_path);
    if !path.exists() {
        return Ok(CommentsFile::default());
    }
    let raw = fs::read_to_string(&path)?;
    let file: CommentsFile = serde_json::from_str(&raw)?;
    Ok(file)
}

pub fn write_comments_sidecar(md_path: &Path, file: &CommentsFile) -> Result<()> {
    let path = comments_sidecar_path(md_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let raw = serde_json::to_string_pretty(file).map_err(Error::from)?;
    fs::write(path, raw)?;
    Ok(())
}

/// Merge by id: entries already in `into` win (live Yjs / peer state).
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
    fn roundtrip_and_merge() {
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
                quote: "word".into(),
                created_at: Utc::now(),
                resolved: false,
            }],
        };
        write_comments_sidecar(&md, &file).unwrap();
        let loaded = read_comments_sidecar(&md).unwrap();
        assert_eq!(loaded.comments.len(), 1);
        assert_eq!(loaded.comments[0].body, "hi");

        let mut live = CommentsFile {
            version: 1,
            comments: vec![CommentRecord {
                id,
                body: "from yjs".into(),
                author: "A".into(),
                quote: "word".into(),
                created_at: Utc::now(),
                resolved: true,
            }],
        };
        merge_comments(&mut live, &loaded);
        assert_eq!(live.comments.len(), 1);
        assert_eq!(live.comments[0].body, "from yjs");
    }
}
