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

/// Durable outcome for suggestions (schema v3).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionDisposition {
    #[default]
    Pending,
    Accepting,
    Accepted,
    Rejected,
    /// Loaded from pre-v3 resolved suggestion; outcome unknown.
    ResolvedLegacy,
}

impl SuggestionDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepting => "accepting",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::ResolvedLegacy => "resolved_legacy",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "accepting" => Some(Self::Accepting),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "resolved_legacy" => Some(Self::ResolvedLegacy),
            _ => None,
        }
    }

    /// Terminal outcomes that count as "resolved" for UI filtering.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Accepted | Self::Rejected | Self::ResolvedLegacy)
    }

    pub fn is_open(self) -> bool {
        matches!(self, Self::Pending | Self::Accepting)
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
    /// For comments: open/resolved. For suggestions: derived from disposition when present.
    pub resolved: bool,
    #[serde(default)]
    pub kind: AnnotationKind,
    /// Monotonic concurrency token. Missing on old records → 1.
    #[serde(default = "default_revision")]
    pub revision: u32,
    /// Suggestion outcome (schema v3). Absent on pure comments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<SuggestionDisposition>,
    /// Active acceptance reservation id (accepting only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_op_id: Option<Uuid>,
    /// Markdown content hash when acceptance began.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_base_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_started_at: Option<DateTime<Utc>>,
    /// Markdown content hash after successful accept finalize.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_completed_at: Option<DateTime<Utc>>,
}

impl CommentRecord {
    pub fn new(
        id: Uuid,
        body: impl Into<String>,
        author: impl Into<String>,
        quote: impl Into<String>,
        kind: AnnotationKind,
    ) -> Self {
        let disposition = match kind {
            AnnotationKind::Suggestion => Some(SuggestionDisposition::Pending),
            AnnotationKind::Comment => None,
        };
        Self {
            id,
            body: body.into(),
            author: author.into(),
            quote: quote.into(),
            created_at: Utc::now(),
            resolved: false,
            kind,
            revision: 1,
            disposition,
            acceptance_op_id: None,
            acceptance_base_hash: None,
            acceptance_started_at: None,
            applied_content_hash: None,
            acceptance_completed_at: None,
        }
    }

    /// Normalize legacy fields after deserialize / migration.
    pub fn normalize_compat(&mut self) {
        if self.revision == 0 {
            self.revision = 1;
        }
        match self.kind {
            AnnotationKind::Comment => {
                self.disposition = None;
                self.clear_acceptance_fields();
            }
            AnnotationKind::Suggestion => {
                if self.disposition.is_none() {
                    self.disposition = Some(if self.resolved {
                        SuggestionDisposition::ResolvedLegacy
                    } else {
                        SuggestionDisposition::Pending
                    });
                }
                self.sync_resolved_from_disposition();
            }
        }
    }

    pub fn sync_resolved_from_disposition(&mut self) {
        if self.kind == AnnotationKind::Suggestion {
            if let Some(d) = self.disposition {
                self.resolved = d.is_terminal();
            }
        }
    }

    pub fn clear_acceptance_fields(&mut self) {
        self.acceptance_op_id = None;
        self.acceptance_base_hash = None;
        self.acceptance_started_at = None;
    }

    pub fn suggestion_disposition(&self) -> Option<SuggestionDisposition> {
        if self.kind == AnnotationKind::Suggestion {
            self.disposition.or(Some(if self.resolved {
                SuggestionDisposition::ResolvedLegacy
            } else {
                SuggestionDisposition::Pending
            }))
        } else {
            None
        }
    }

    pub fn content_matches(&self, other: &Self) -> bool {
        self.id == other.id
            && self.body == other.body
            && self.author == other.author
            && self.quote == other.quote
            && self.resolved == other.resolved
            && self.kind == other.kind
            && self.disposition == other.disposition
            && self.acceptance_op_id == other.acceptance_op_id
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
    if let Some(meta) = crate::run_meta::load_run_meta_readonly(md_path)? {
        return Ok(crate::run_meta::comments_from_meta(&meta));
    }
    let path = comments_sidecar_path(md_path);
    if !path.exists() {
        return Ok(CommentsFile::default());
    }
    let raw = fs::read_to_string(&path)?;
    let mut file: CommentsFile = serde_json::from_str(&raw)?;
    for c in &mut file.comments {
        c.normalize_compat();
    }
    Ok(file)
}

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
        let mut c: CommentRecord = serde_json::from_str(raw).unwrap();
        c.normalize_compat();
        assert_eq!(c.revision, 1);
    }

    #[test]
    fn legacy_resolved_suggestion_becomes_resolved_legacy() {
        let raw = r#"{"id":"00000000-0000-4000-8000-000000000001","body":"x","author":"A","quote":"q","createdAt":"2020-01-01T00:00:00Z","resolved":true,"kind":"suggestion"}"#;
        let mut c: CommentRecord = serde_json::from_str(raw).unwrap();
        c.normalize_compat();
        assert_eq!(c.disposition, Some(SuggestionDisposition::ResolvedLegacy));
        assert!(c.resolved);
    }

    #[test]
    fn legacy_open_suggestion_becomes_pending() {
        let raw = r#"{"id":"00000000-0000-4000-8000-000000000002","body":"x","author":"A","quote":"q","createdAt":"2020-01-01T00:00:00Z","resolved":false,"kind":"suggestion"}"#;
        let mut c: CommentRecord = serde_json::from_str(raw).unwrap();
        c.normalize_compat();
        assert_eq!(c.disposition, Some(SuggestionDisposition::Pending));
        assert!(!c.resolved);
    }
}
