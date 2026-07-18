//! Versioned run-review ledger sidecar (`file.md.moraine.json`).
//!
//! Content hash: SHA-256 of exact UTF-8 bytes of the Markdown string as held in memory
//! (same bytes Moraine writes to disk; no line-ending normalization).
//!
//! Mutations (ensure, decide, set comments, migration) take a per-sidecar exclusive lock,
//! re-read after lock, then write via safe atomic replace.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::atomic::{write_atomic, SidecarLock};
use crate::comments::{comments_sidecar_path, CommentRecord, CommentsFile};
use crate::error::{Error, Result};

/// Current sidecar schema. Unknown greater versions are rejected.
/// v3: suggestion disposition + two-phase acceptance fields.
pub const SCHEMA_VERSION: u32 = 3;

pub fn moraine_sidecar_path(md_path: &Path) -> PathBuf {
    let mut s = md_path.as_os_str().to_os_string();
    s.push(".moraine.json");
    PathBuf::from(s)
}

/// Archival name after a successful legacy comments migration.
pub fn comments_migrated_path(md_path: &Path) -> PathBuf {
    let mut s = comments_sidecar_path(md_path).into_os_string();
    s.push(".migrated");
    PathBuf::from(s)
}

/// SHA-256 hex of exact UTF-8 bytes of `markdown` (no normalization).
pub fn content_hash(markdown: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(markdown.as_bytes());
    hex::encode(hasher.finalize())
}

/// SHA-256 of file bytes on disk (exact UTF-8 content after decode).
pub fn content_hash_file(md_path: &Path) -> Result<String> {
    let markdown = crate::document::Document::read_file(md_path)?;
    Ok(content_hash(&markdown))
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
    /// False when no durable ledger exists on disk yet.
    pub initialized: bool,
}

pub fn review_snapshot(meta: &RunMeta, markdown: &str) -> ReviewSnapshot {
    review_snapshot_init(meta, markdown, true)
}

fn review_snapshot_init(meta: &RunMeta, markdown: &str, initialized: bool) -> ReviewSnapshot {
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
        initialized,
    }
}

fn parse_meta_raw(raw: &str) -> Result<RunMeta> {
    let mut meta: RunMeta = serde_json::from_str(raw)?;
    if meta.schema_version > SCHEMA_VERSION {
        return Err(Error::other(format!(
            "unsupported moraine sidecar schema version {} (max {})",
            meta.schema_version, SCHEMA_VERSION
        )));
    }
    // Normalize comment compatibility fields in memory (disposition defaults).
    for c in &mut meta.comments {
        c.normalize_compat();
    }
    Ok(meta)
}

/// Ensure written ledger uses current schema version after in-memory migration.
pub(crate) fn promote_schema(meta: &mut RunMeta) {
    if meta.schema_version < SCHEMA_VERSION {
        meta.schema_version = SCHEMA_VERSION;
    }
    for c in &mut meta.comments {
        c.normalize_compat();
    }
}

fn read_moraine_file(path: &Path) -> Result<RunMeta> {
    let raw = fs::read_to_string(path)?;
    parse_meta_raw(&raw)
}

/// Read-only: load existing `.moraine.json` only. Never creates or migrates files.
pub fn load_run_meta_readonly(md_path: &Path) -> Result<Option<RunMeta>> {
    let path = moraine_sidecar_path(md_path);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_moraine_file(&path)?))
}

/// Read-only review status from disk (no sidecar creation).
pub fn status_snapshot(md_path: &Path) -> Result<ReviewSnapshot> {
    let markdown = crate::document::Document::read_file(md_path)?;
    if let Some(meta) = load_run_meta_readonly(md_path)? {
        return Ok(review_snapshot_init(&meta, &markdown, true));
    }
    // Legacy comments only: surface annotation counts via ephemeral meta, not initialized.
    let legacy = comments_sidecar_path(md_path);
    if legacy.exists() {
        let raw = fs::read_to_string(&legacy)?;
        let file: CommentsFile = serde_json::from_str(&raw)?;
        let mut meta = RunMeta::new_run();
        meta.comments = file.comments;
        let mut snap = review_snapshot_init(&meta, &markdown, false);
        // Uninitialized: do not treat ephemeral UUID as durable.
        snap.run_id = Uuid::nil();
        return Ok(snap);
    }
    let meta = RunMeta::new_run();
    let mut snap = review_snapshot_init(&meta, &markdown, false);
    snap.run_id = Uuid::nil();
    Ok(snap)
}

/// Load meta for mutation paths: prefer `.moraine.json`; migrate legacy under caller-held lock.
/// Does not create a new ledger when nothing exists (use `ensure_run_meta`).
pub(crate) fn load_or_migrate_locked(md_path: &Path) -> Result<RunMeta> {
    let path = moraine_sidecar_path(md_path);
    if path.exists() {
        let meta = read_moraine_file(&path)?;
        // If both exist, archive legacy without re-import (deterministic, non-repeating).
        archive_legacy_if_present(md_path)?;
        return Ok(meta);
    }

    let legacy = comments_sidecar_path(md_path);
    if legacy.exists() {
        return migrate_legacy_locked(md_path, &legacy);
    }

    Err(Error::other(
        "run ledger not initialized (use moraine init or open in the desktop app)",
    ))
}

fn migrate_legacy_locked(md_path: &Path, legacy: &Path) -> Result<RunMeta> {
    let raw = fs::read_to_string(legacy)?;
    let file: CommentsFile = serde_json::from_str(&raw).map_err(|e| {
        Error::other(format!(
            "malformed legacy comments sidecar {}: {e}",
            legacy.display()
        ))
    })?;
    let mut meta = RunMeta::new_run();
    meta.comments = file.comments;
    meta.schema_version = SCHEMA_VERSION;
    write_run_meta_unlocked(md_path, &meta)?;
    archive_legacy_path(legacy)?;
    Ok(meta)
}

fn archive_legacy_if_present(md_path: &Path) -> Result<()> {
    let legacy = comments_sidecar_path(md_path);
    if legacy.exists() {
        archive_legacy_path(&legacy)?;
    }
    Ok(())
}

fn archive_legacy_path(legacy: &Path) -> Result<()> {
    let dest = {
        let mut s = legacy.as_os_str().to_os_string();
        s.push(".migrated");
        PathBuf::from(s)
    };
    if dest.exists() {
        // Already archived once: keep a unique residual archive rather than re-import.
        let alt = {
            let mut s = dest.into_os_string();
            s.push(format!(".{}", Uuid::new_v4()));
            PathBuf::from(s)
        };
        fs::rename(legacy, &alt)?;
    } else {
        fs::rename(legacy, &dest)?;
    }
    Ok(())
}

/// Load existing ledger, migrating legacy if needed. Does **not** create an empty ledger.
/// Prefer `load_run_meta_readonly` for pure inspection.
pub fn load_run_meta(md_path: &Path) -> Result<RunMeta> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    load_or_migrate_locked(md_path)
}

/// Load or create durable ledger (assigns stable run id on first write). Migrates legacy once.
pub fn ensure_run_meta(md_path: &Path) -> Result<RunMeta> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    match load_or_migrate_locked(md_path) {
        Ok(m) => Ok(m),
        Err(_) if !side.exists() && !comments_sidecar_path(md_path).exists() => {
            let meta = RunMeta::new_run();
            write_run_meta_unlocked(md_path, &meta)?;
            Ok(meta)
        }
        Err(e) => Err(e),
    }
}

/// Write ledger without taking a lock (caller must hold `SidecarLock`).
pub(crate) fn write_run_meta_unlocked(md_path: &Path, meta: &RunMeta) -> Result<()> {
    let mut meta = meta.clone();
    promote_schema(&mut meta);
    let path = moraine_sidecar_path(md_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let raw = serde_json::to_string_pretty(&meta)?;
    write_atomic(&path, raw.as_bytes())
}

/// Write ledger under exclusive lock (re-read is caller's responsibility for RMW).
pub fn write_run_meta(md_path: &Path, meta: &RunMeta) -> Result<()> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    write_run_meta_unlocked(md_path, meta)
}

/// Record a decision only if on-disk Markdown hash matches `expected_content_hash`.
pub fn record_decision(
    md_path: &Path,
    kind: DecisionKind,
    reviewer_label: impl Into<String>,
    reason: Option<String>,
    expected_content_hash: &str,
) -> Result<(RunMeta, ReviewDecision, ReviewSnapshot)> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;

    let markdown = crate::document::Document::read_file(md_path)?;
    let actual = content_hash(&markdown);
    if actual != expected_content_hash {
        return Err(Error::RevisionConflict {
            expected: expected_content_hash.to_string(),
            actual,
        });
    }

    let mut meta = match load_or_migrate_locked(md_path) {
        Ok(m) => m,
        Err(_) if !side.exists() && !comments_sidecar_path(md_path).exists() => {
            let m = RunMeta::new_run();
            write_run_meta_unlocked(md_path, &m)?;
            m
        }
        Err(e) => return Err(e),
    };

    let recorded = meta
        .append_decision(kind, reviewer_label, reason, actual.clone())
        .clone();
    write_run_meta_unlocked(md_path, &meta)?;
    let snap = review_snapshot_init(&meta, &markdown, true);
    Ok((meta, recorded, snap))
}

/// **Deprecated.** Full-list replacement is not safe for concurrent writers.
/// Use [`crate::annotation_ops`] or [`crate::annotation_ops::reconcile_session_annotations`].
#[deprecated(note = "use annotation_ops instead of full-list annotation replacement")]
pub fn set_comments_and_save(md_path: &Path, comments: Vec<CommentRecord>) -> Result<RunMeta> {
    crate::annotation_ops::import_missing_annotations(md_path, &comments)
}

pub fn comments_from_meta(meta: &RunMeta) -> CommentsFile {
    CommentsFile {
        version: 1,
        comments: meta.comments.clone(),
    }
}

/// Compatibility: read annotations (moraine or legacy), without creating a new empty ledger.
pub fn read_annotations(md_path: &Path) -> Result<Vec<CommentRecord>> {
    if let Some(meta) = load_run_meta_readonly(md_path)? {
        return Ok(meta.comments);
    }
    let legacy = comments_sidecar_path(md_path);
    if legacy.exists() {
        let raw = fs::read_to_string(&legacy)?;
        let file: CommentsFile = serde_json::from_str(&raw)?;
        return Ok(file.comments);
    }
    Ok(Vec::new())
}

/// Verify Markdown on disk still matches `expected_content_hash`.
pub fn assert_disk_revision(md_path: &Path, expected_content_hash: &str) -> Result<String> {
    let actual = content_hash_file(md_path)?;
    if actual != expected_content_hash {
        return Err(Error::RevisionConflict {
            expected: expected_content_hash.to_string(),
            actual,
        });
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;
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
        let b = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(a.run.id, b.run.id);
    }

    #[test]
    fn status_readonly_does_not_create_sidecar() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("plain.md");
        fs::write(&md, "plain\n").unwrap();
        let snap = status_snapshot(&md).unwrap();
        assert!(!snap.initialized);
        assert!(!moraine_sidecar_path(&md).exists());
        assert_eq!(snap.state, ReviewStateKind::Unreviewed);
        assert_eq!(snap.content_hash, content_hash("plain\n"));
    }

    #[test]
    fn migrate_legacy_comments_archives() {
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
        let meta = ensure_run_meta(&md).unwrap();
        assert_eq!(meta.comments.len(), 1);
        assert_eq!(meta.comments[0].body, "note");
        assert!(moraine_sidecar_path(&md).exists());
        assert!(!legacy_path.exists());
        assert!(comments_migrated_path(&md).exists());

        // Reopen does not re-import or change run id
        let again = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(again.run.id, meta.run.id);
        assert_eq!(again.comments.len(), 1);

        // Touch archived file: still ignored
        fs::write(
            comments_migrated_path(&md),
            r#"{"version":1,"comments":[]}"#,
        )
        .unwrap();
        let third = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(third.comments.len(), 1);
        assert_eq!(third.comments[0].body, "note");
    }

    #[test]
    fn both_sidecars_prefers_moraine_archives_legacy() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("both.md");
        fs::write(&md, "x\n").unwrap();
        let mut meta = RunMeta::new_run();
        meta.comments.push(CommentRecord::new(
            Uuid::new_v4(),
            "from-moraine",
            "A",
            "x",
            crate::AnnotationKind::Comment,
        ));
        write_run_meta(&md, &meta).unwrap();
        let legacy = comments_sidecar_path(&md);
        fs::write(
            &legacy,
            r#"{"version":1,"comments":[{"id":"00000000-0000-4000-8000-000000000001","body":"from-legacy","author":"B","quote":"x","createdAt":"2020-01-01T00:00:00Z","resolved":false,"kind":"comment"}]}"#,
        )
        .unwrap();
        let loaded = load_run_meta(&md).unwrap();
        assert_eq!(loaded.comments.len(), 1);
        assert_eq!(loaded.comments[0].body, "from-moraine");
        assert!(!legacy.exists());
        assert!(comments_migrated_path(&md).exists());
    }

    #[test]
    fn malformed_legacy_errors() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("badleg.md");
        fs::write(&md, "x\n").unwrap();
        fs::write(comments_sidecar_path(&md), "{nope").unwrap();
        let err = ensure_run_meta(&md).unwrap_err();
        assert!(
            err.to_string().contains("malformed")
                || err.to_string().contains("serde")
                || err.to_string().contains("expected")
        );
    }

    #[test]
    fn decisions_append_and_stale() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("d.md");
        fs::write(&md, "v1\n").unwrap();
        let h1 = content_hash("v1\n");
        let (_, _, snap) =
            record_decision(&md, DecisionKind::Approved, "Alice", Some("ok".into()), &h1).unwrap();
        assert_eq!(snap.state, ReviewStateKind::Approved);
        assert!(snap.decision_current);

        fs::write(&md, "v2\n").unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let snap2 = review_snapshot(&meta, "v2\n");
        assert_eq!(snap2.state, ReviewStateKind::Stale);
        assert!(!snap2.decision_current);
        assert_eq!(snap2.decision_count, 1);

        let h2 = content_hash("v2\n");
        let (_, _, snap3) =
            record_decision(&md, DecisionKind::ChangesRequested, "Bob", None, &h2).unwrap();
        assert_eq!(snap3.state, ReviewStateKind::ChangesRequested);
        assert!(snap3.decision_current);
        assert_eq!(snap3.decision_count, 2);
    }

    #[test]
    fn decision_revision_conflict() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("c.md");
        fs::write(&md, "v1\n").unwrap();
        let wrong = content_hash("other\n");
        let err = record_decision(&md, DecisionKind::Approved, "A", None, &wrong).unwrap_err();
        match err {
            Error::RevisionConflict { expected, actual } => {
                assert_eq!(expected, wrong);
                assert_eq!(actual, content_hash("v1\n"));
            }
            other => panic!("expected conflict, got {other}"),
        }
        // No decision persisted
        assert!(load_run_meta_readonly(&md).unwrap().is_none());
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
        let err = load_run_meta_readonly(&md).unwrap_err();
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
        assert!(load_run_meta_readonly(&md).is_err());
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
        let loaded = load_run_meta_readonly(&md2).unwrap().unwrap();
        assert_eq!(loaded.run.id, id);
    }

    #[test]
    fn concurrent_decisions_both_survive() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("conc.md");
        fs::write(&md, "body\n").unwrap();
        ensure_run_meta(&md).unwrap();
        let h = content_hash("body\n");
        let barrier = Arc::new(Barrier::new(2));
        let md1 = md.clone();
        let md2 = md.clone();
        let h1 = h.clone();
        let h2 = h.clone();
        let b1 = barrier.clone();
        let b2 = barrier;

        let t1 = thread::spawn(move || {
            b1.wait();
            record_decision(&md1, DecisionKind::Approved, "A", None, &h1).unwrap()
        });
        let t2 = thread::spawn(move || {
            b2.wait();
            record_decision(&md2, DecisionKind::Rejected, "B", None, &h2).unwrap()
        });
        t1.join().unwrap();
        t2.join().unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.decisions.len(), 2);
        let labels: Vec<_> = meta
            .decisions
            .iter()
            .map(|d| d.reviewer_label.as_str())
            .collect();
        assert!(labels.contains(&"A"));
        assert!(labels.contains(&"B"));
    }

    #[test]
    fn concurrent_comment_creates_via_ops() {
        let dir = tempdir().unwrap();
        let md = dir.path().join("ann.md");
        fs::write(&md, "t\n").unwrap();
        ensure_run_meta(&md).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let md1 = md.clone();
        let md2 = md.clone();
        let b1 = barrier.clone();
        let b2 = barrier;

        let t1 = thread::spawn(move || {
            b1.wait();
            for i in 0..20 {
                crate::annotation_ops::create_annotation(
                    &md1,
                    Uuid::new_v4(),
                    format!("w1-{i}"),
                    "W1",
                    "t",
                    crate::AnnotationKind::Comment,
                )
                .unwrap();
            }
        });
        let t2 = thread::spawn(move || {
            b2.wait();
            for i in 0..20 {
                crate::annotation_ops::create_annotation(
                    &md2,
                    Uuid::new_v4(),
                    format!("w2-{i}"),
                    "W2",
                    "t",
                    crate::AnnotationKind::Comment,
                )
                .unwrap();
            }
        });
        t1.join().unwrap();
        t2.join().unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.comments.len(), 40);
        let _ = serde_json::to_string(&meta).unwrap();
    }
}
