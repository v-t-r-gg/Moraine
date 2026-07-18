//! Annotation types and legacy `*.md.comments.json` reader.
//! Mutations go through [`crate::annotation_ops`]; this module is types + read/migration helpers.

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

fn default_revision() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationKind {
    #[default]
    Comment,
    Suggestion,
}

impl AnnotationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::Suggestion => "suggestion",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "comment" => Some(Self::Comment),
            "suggestion" => Some(Self::Suggestion),
            _ => None,
        }
    }
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
    /// Monotonic concurrency token. Missing on old records → 1.
    #[serde(default = "default_revision")]
    pub revision: u32,
}

impl CommentRecord {
    pub fn new(
        id: Uuid,
        body: impl Into<String>,
        author: impl Into<String>,
        quote: impl Into<String>,
        kind: AnnotationKind,
    ) -> Self {
        Self {
            id,
            body: body.into(),
            author: author.into(),
            quote: quote.into(),
            created_at: Utc::now(),
            resolved: false,
            kind,
            revision: 1,
        }
    }

    pub fn content_matches(&self, other: &Self) -> bool {
        self.id == other.id
            && self.body == other.body
            && self.author == other.author
            && self.quote == other.quote
            && self.resolved == other.resolved
            && self.kind == other.kind
    }
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

/// Legacy full-list write. **Not for production UI.** Prefer [`crate::annotation_ops`].
///
/// Test-only / migration helper: creates missing records by ID under lock via create ops
/// when empty ledger; if ledger has comments, only adds missing IDs (never deletes, never
/// overwrites same-ID content).
#[deprecated(note = "use annotation_ops create/update/resolve instead of full-list writes")]
pub fn write_comments_sidecar(md_path: &Path, file: &CommentsFile) -> Result<()> {
    crate::annotation_ops::import_missing_annotations(md_path, &file.comments)?;
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
    #[allow(deprecated)]
    fn write_goes_to_moraine_json() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("a.md");
        fs::write(&md, "# a\n").unwrap();
        let id = Uuid::new_v4();
        let file = CommentsFile {
            version: 1,
            comments: vec![CommentRecord::new(
                id,
                "hi",
                "A",
                "a",
                AnnotationKind::Comment,
            )],
        };
        write_comments_sidecar(&md, &file).unwrap();
        assert!(crate::run_meta::moraine_sidecar_path(&md).exists());
        let loaded = read_comments_sidecar(&md).unwrap();
        assert_eq!(loaded.comments[0].body, "hi");
        assert_eq!(loaded.comments[0].revision, 1);
    }

    #[test]
    fn revision_defaults_on_legacy_json() {
        let raw = r#"{"id":"00000000-0000-4000-8000-000000000001","body":"x","author":"A","quote":"q","createdAt":"2020-01-01T00:00:00Z","resolved":false,"kind":"comment"}"#;
        let c: CommentRecord = serde_json::from_str(raw).unwrap();
        assert_eq!(c.revision, 1);
    }
}
