//! Versioned run-review ledger sidecar (`file.md.moraine.json`).
//!
//! Content hash: SHA-256 of exact UTF-8 bytes of the Markdown string as held in memory
//! (same bytes Moraine writes to disk; no line-ending normalization).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::comments::{comments_sidecar_path, CommentRecord, CommentsFile};
use crate::error::{Error, Result};

/// Current sidecar schema. Unknown greater versions are rejected.
pub const SCHEMA_VERSION: u32 = 2;

pub fn moraine_sidecar_path(md_path: &Path) -> PathBuf {
    let mut s = md_path.as_os_str().to_os_string();
    s.push(".moraine.json");
    PathBuf::from(s)
}

/// SHA-256 hex of exact UTF-8 bytes of `markdown` (no normalization).
pub fn content_hash(markdown: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(markdown.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Approved,
    ChangesRequested,
    Rejected,
}

impl DecisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "approved" => Some(Self::Approved),
            "changes_requested" | "changes-requested" | "request_changes" => {
                Some(Self::ChangesRequested)
            }
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDecision {
    pub id: Uuid,
    pub decision: DecisionKind,
    pub reviewer_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
    /// SHA-256 hex of Markdown at decision time.
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunInfo {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunMeta {
    pub schema_version: u32,
    pub run: RunInfo,
    #[serde(default)]
    pub decisions: Vec<ReviewDecision>,
    /// Annotations (comments + suggestions). Field name matches legacy CommentsFile.
    #[serde(default)]
    pub comments: Vec<CommentRecord>,
}

impl RunMeta {
    pub fn new_run() -> Self {
        let now = Utc::now();
        Self {
            schema_version: SCHEMA_VERSION,
            run: RunInfo {
                id: Uuid::new_v4(),
                created_at: now,
                updated_at: now,
            },
            decisions: Vec::new(),
            comments: Vec::new(),
        }
    }

    pub fn touch(&mut self) {
        self.run.updated_at = Utc::now();
    }

    pub fn latest_decision(&self) -> Option<&ReviewDecision> {
        self.decisions.last()
    }

    pub fn append_decision(
        &mut self,
        kind: DecisionKind,
        reviewer_label: impl Into<String>,
        reason: Option<String>,
        content_hash: impl Into<String>,
    ) -> &ReviewDecision {
        let d = ReviewDecision {
            id: Uuid::new_v4(),
            decision: kind,
            reviewer_label: reviewer_label.into(),
            reason,
            created_at: Utc::now(),
            content_hash: content_hash.into(),
        };
        self.decisions.push(d);
        self.touch();
        self.decisions.last().expect("just pushed")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStateKind {
    Unreviewed,
    Approved,
    ChangesRequested,
    Rejected,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSnapshot {
    pub state: ReviewStateKind,
    /// True when latest decision's hash matches current content hash.
    pub decision_current: bool,
    pub decision_count: usize,
    pub latest: Option<ReviewDecision>,
    pub content_hash: String,
    pub run_id: Uuid,
}

pub fn review_snapshot(meta: &RunMeta, markdown: &str) -> ReviewSnapshot {
    let hash = content_hash(markdown);
    let latest = meta.latest_decision().cloned();
    let decision_count = meta.decisions.len();
    let (state, decision_current) = match &latest {
        None => (ReviewStateKind::Unreviewed, true),
        Some(d) if d.content_hash == hash => {
            let st = match d.decision {
                DecisionKind::Approved => ReviewStateKind::Approved,
                DecisionKind::ChangesRequested => ReviewStateKind::ChangesRequested,
                DecisionKind::Rejected => ReviewStateKind::Rejected,
            };
            (st, true)
        }
        Some(_) => (ReviewStateKind::Stale, false),
    };
    ReviewSnapshot {
        state,
        decision_current,
        decision_count,
        latest,
        content_hash: hash,
        run_id: meta.run.id,
    }
}

/// Load run meta: prefer `.moraine.json`, else migrate legacy `.comments.json`.
pub fn load_run_meta(md_path: &Path) -> Result<RunMeta> {
    let path = moraine_sidecar_path(md_path);
    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let meta: RunMeta = serde_json::from_str(&raw)?;
        if meta.schema_version > SCHEMA_VERSION {
            return Err(Error::other(format!(
                "unsupported moraine sidecar schema version {} (max {})",
                meta.schema_version, SCHEMA_VERSION
            )));
        }
        return Ok(meta);
    }

    let legacy = comments_sidecar_path(md_path);
    if legacy.exists() {
        let raw = fs::read_to_string(&legacy)?;
        let file: CommentsFile = serde_json::from_str(&raw)?;
        let mut meta = RunMeta::new_run();
        meta.comments = file.comments;
        meta.schema_version = SCHEMA_VERSION;
        write_run_meta(md_path, &meta)?;
        return Ok(meta);
    }

    Ok(RunMeta::new_run())
}

/// Load or create and ensure sidecar exists (assigns stable run id on first write).
pub fn ensure_run_meta(md_path: &Path) -> Result<RunMeta> {
    let path = moraine_sidecar_path(md_path);
    let legacy = comments_sidecar_path(md_path);
    if path.exists() || legacy.exists() {
        return load_run_meta(md_path);
    }
    let meta = RunMeta::new_run();
    write_run_meta(md_path, &meta)?;
    Ok(meta)
}

pub fn write_run_meta(md_path: &Path, meta: &RunMeta) -> Result<()> {
    let path = moraine_sidecar_path(md_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let raw = serde_json::to_string_pretty(meta)?;
    atomic_write(&path, raw.as_bytes())?;
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("json")
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).or_else(|_| {
        fs::write(path, bytes)?;
        let _ = fs::remove_file(&tmp);
        Ok::<(), std::io::Error>(())
    })?;
    Ok(())
}

/// Replace annotations in meta and persist (used by comment save path).
pub fn set_comments_and_save(md_path: &Path, comments: Vec<CommentRecord>) -> Result<RunMeta> {
    let mut meta = load_run_meta(md_path)?;
    meta.comments = comments;
    meta.touch();
    write_run_meta(md_path, &meta)?;
    Ok(meta)
}

pub fn comments_from_meta(meta: &RunMeta) -> CommentsFile {
    CommentsFile {
        version: 1,
        comments: meta.comments.clone(),
    }
}

/// Compatibility: read annotations (moraine or legacy).
pub fn read_annotations(md_path: &Path) -> Result<Vec<CommentRecord>> {
    Ok(load_run_meta(md_path)?.comments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn hash_is_deterministic_exact_bytes() {
        assert_eq!(content_hash("hello\n"), content_hash("hello\n"));
        assert_ne!(content_hash("hello\n"), content_hash("hello\r\n"));
        assert_ne!(content_hash("a"), content_hash("b"));
    }

    #[test]
    fn unicode_hash() {
        let h = content_hash("café 日本語\n");
        assert_eq!(h.len(), 64);
        assert_eq!(h, content_hash("café 日本語\n"));
    }

    #[test]
    fn new_run_stable_id_across_reload() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("run.md");
        fs::write(&md, "# run\n").unwrap();
        let a = ensure_run_meta(&md).unwrap();
        let b = load_run_meta(&md).unwrap();
        assert_eq!(a.run.id, b.run.id);
    }

    #[test]
    fn migrate_legacy_comments() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("legacy.md");
        fs::write(&md, "body\n").unwrap();
        let legacy_path = comments_sidecar_path(&md);
        fs::write(
            &legacy_path,
            r#"{"version":1,"comments":[{"id":"00000000-0000-4000-8000-000000000099","body":"note","author":"A","quote":"body","createdAt":"2020-01-01T00:00:00Z","resolved":false,"kind":"comment"}]}"#,
        )
        .unwrap();
        assert!(!moraine_sidecar_path(&md).exists());
        let meta = load_run_meta(&md).unwrap();
        assert_eq!(meta.comments.len(), 1);
        assert_eq!(meta.comments[0].body, "note");
        assert!(moraine_sidecar_path(&md).exists());
        // Reopen retains same run id and comments
        let again = load_run_meta(&md).unwrap();
        assert_eq!(again.run.id, meta.run.id);
        assert_eq!(again.comments.len(), 1);
    }

    #[test]
    fn decisions_append_and_stale() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("d.md");
        fs::write(&md, "v1\n").unwrap();
        let mut meta = ensure_run_meta(&md).unwrap();
        let h1 = content_hash("v1\n");
        meta.append_decision(
            DecisionKind::Approved,
            "Alice",
            Some("ok".into()),
            h1.clone(),
        );
        write_run_meta(&md, &meta).unwrap();

        let snap = review_snapshot(&meta, "v1\n");
        assert_eq!(snap.state, ReviewStateKind::Approved);
        assert!(snap.decision_current);

        let snap2 = review_snapshot(&meta, "v2\n");
        assert_eq!(snap2.state, ReviewStateKind::Stale);
        assert!(!snap2.decision_current);
        assert_eq!(snap2.decision_count, 1);

        meta.append_decision(
            DecisionKind::ChangesRequested,
            "Bob",
            None,
            content_hash("v2\n"),
        );
        let snap3 = review_snapshot(&meta, "v2\n");
        assert_eq!(snap3.state, ReviewStateKind::ChangesRequested);
        assert!(snap3.decision_current);
        assert_eq!(snap3.decision_count, 2);
        assert_eq!(meta.decisions[0].decision, DecisionKind::Approved);
    }

    #[test]
    fn unsupported_schema() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("x.md");
        fs::write(&md, "x\n").unwrap();
        let path = moraine_sidecar_path(&md);
        fs::write(
            &path,
            r#"{"schemaVersion":99,"run":{"id":"00000000-0000-4000-8000-000000000001","createdAt":"2020-01-01T00:00:00Z","updatedAt":"2020-01-01T00:00:00Z"},"decisions":[],"comments":[]}"#,
        )
        .unwrap();
        let err = load_run_meta(&md).unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn empty_markdown_unreviewed() {
        let meta = RunMeta::new_run();
        let s = review_snapshot(&meta, "");
        assert_eq!(s.state, ReviewStateKind::Unreviewed);
        assert_eq!(s.decision_count, 0);
    }

    #[test]
    fn all_decision_kinds() {
        let mut meta = RunMeta::new_run();
        let h = content_hash("x");
        for k in [
            DecisionKind::Approved,
            DecisionKind::ChangesRequested,
            DecisionKind::Rejected,
        ] {
            meta.append_decision(k, "R", None, h.clone());
        }
        assert_eq!(meta.decisions.len(), 3);
        assert_eq!(
            meta.latest_decision().unwrap().decision,
            DecisionKind::Rejected
        );
    }

    #[test]
    fn malformed_metadata_errors() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("bad.md");
        fs::write(&md, "x\n").unwrap();
        let path = moraine_sidecar_path(&md);
        fs::write(&path, "{not json").unwrap();
        assert!(load_run_meta(&md).is_err());
    }

    #[test]
    fn md_without_meta_gets_id_on_ensure() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("plain.md");
        fs::write(&md, "plain\n").unwrap();
        assert!(!moraine_sidecar_path(&md).exists());
        let a = ensure_run_meta(&md).unwrap();
        assert!(moraine_sidecar_path(&md).exists());
        let b = ensure_run_meta(&md).unwrap();
        assert_eq!(a.run.id, b.run.id);
    }

    #[test]
    fn path_move_keeps_run_id_in_sidecar() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("a.md");
        fs::write(&md, "body\n").unwrap();
        let meta = ensure_run_meta(&md).unwrap();
        let id = meta.run.id;
        let side = moraine_sidecar_path(&md);
        let md2 = dir.path().join("moved.md");
        let side2 = moraine_sidecar_path(&md2);
        fs::rename(&md, &md2).unwrap();
        fs::rename(&side, &side2).unwrap();
        let loaded = load_run_meta(&md2).unwrap();
        assert_eq!(loaded.run.id, id);
    }
}
