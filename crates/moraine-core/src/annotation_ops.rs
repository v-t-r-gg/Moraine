//! Operation-based annotation mutations under the per-document ledger lock.
//!
//! Each operation re-reads the latest ledger, validates preconditions (including
//! per-annotation `revision`), applies one change, and writes via atomic replace.

use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic::SidecarLock;
use crate::comments::{AnnotationKind, CommentRecord};
use crate::error::{Error, Result};
use crate::run_meta::{
    load_or_migrate_locked, moraine_sidecar_path, write_run_meta_unlocked, RunMeta,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationOpResult {
    pub annotation: CommentRecord,
    pub comments: Vec<CommentRecord>,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileResult {
    pub comments: Vec<CommentRecord>,
    pub created: usize,
    pub updated: usize,
    pub conflicts: Vec<ReconcileConflict>,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileConflict {
    pub annotation_id: Uuid,
    pub expected_revision: u32,
    pub actual_revision: u32,
    pub message: String,
}

fn ensure_meta_locked(md_path: &Path) -> Result<RunMeta> {
    let side = moraine_sidecar_path(md_path);
    match load_or_migrate_locked(md_path) {
        Ok(m) => Ok(m),
        Err(_) if !side.exists() && !crate::comments::comments_sidecar_path(md_path).exists() => {
            let m = RunMeta::new_run();
            write_run_meta_unlocked(md_path, &m)?;
            Ok(m)
        }
        Err(e) => Err(e),
    }
}

fn find_index(meta: &RunMeta, id: Uuid) -> Option<usize> {
    meta.comments.iter().position(|c| c.id == id)
}

fn bump(rec: &mut CommentRecord) {
    rec.revision = rec.revision.saturating_add(1);
}

fn finish(md_path: &Path, meta: &mut RunMeta, id: Uuid) -> Result<AnnotationOpResult> {
    meta.touch();
    write_run_meta_unlocked(md_path, meta)?;
    let annotation = meta
        .comments
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or(Error::AnnotationNotFound { id })?;
    Ok(AnnotationOpResult {
        annotation,
        comments: meta.comments.clone(),
        run_id: meta.run.id,
    })
}

/// Create a new annotation. Fails if `id` already exists.
pub fn create_annotation(
    md_path: &Path,
    id: Uuid,
    body: impl Into<String>,
    author: impl Into<String>,
    quote: impl Into<String>,
    kind: AnnotationKind,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    if find_index(&meta, id).is_some() {
        return Err(Error::DuplicateAnnotation { id });
    }
    let body = body.into();
    let author = author.into();
    let quote = quote.into();
    if kind == AnnotationKind::Comment && body.trim().is_empty() {
        return Err(Error::AnnotationPrecondition {
            id: Some(id),
            message: "comment body must not be empty".into(),
        });
    }
    let rec = CommentRecord::new(id, body, author, quote, kind);
    meta.comments.push(rec);
    finish(md_path, &mut meta, id)
}

/// Update annotation body (and optionally author). Requires matching `expected_revision`.
pub fn update_annotation(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
    body: Option<String>,
    author: Option<String>,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    if rec.revision != expected_revision {
        return Err(Error::AnnotationConflict {
            id,
            expected_revision,
            actual_revision: rec.revision,
        });
    }
    if let Some(b) = body {
        if rec.kind == AnnotationKind::Comment && b.trim().is_empty() {
            return Err(Error::AnnotationPrecondition {
                id: Some(id),
                message: "comment body must not be empty".into(),
            });
        }
        rec.body = b;
    }
    if let Some(a) = author {
        rec.author = a;
    }
    bump(rec);
    finish(md_path, &mut meta, id)
}

/// Mark resolved. Already-resolved with matching token is idempotent (no revision bump).
pub fn resolve_annotation(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
) -> Result<AnnotationOpResult> {
    set_resolved(md_path, id, expected_revision, true)
}

/// Mark open. Already-open with matching token is idempotent (no revision bump).
pub fn reopen_annotation(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
) -> Result<AnnotationOpResult> {
    set_resolved(md_path, id, expected_revision, false)
}

fn set_resolved(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
    resolved: bool,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    if rec.revision != expected_revision {
        return Err(Error::AnnotationConflict {
            id,
            expected_revision,
            actual_revision: rec.revision,
        });
    }
    if rec.resolved == resolved {
        // Idempotent success without bump.
        return Ok(AnnotationOpResult {
            annotation: rec.clone(),
            comments: meta.comments.clone(),
            run_id: meta.run.id,
        });
    }
    rec.resolved = resolved;
    bump(rec);
    finish(md_path, &mut meta, id)
}

/// Accept suggestion: resolve it after the UI has applied the edit. Kind must be suggestion.
pub fn accept_suggestion(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    if rec.kind != AnnotationKind::Suggestion {
        return Err(Error::InvalidAnnotationKind {
            id,
            message: "accept requires a suggestion".into(),
        });
    }
    if rec.revision != expected_revision {
        return Err(Error::AnnotationConflict {
            id,
            expected_revision,
            actual_revision: rec.revision,
        });
    }
    if rec.resolved {
        return Ok(AnnotationOpResult {
            annotation: rec.clone(),
            comments: meta.comments.clone(),
            run_id: meta.run.id,
        });
    }
    rec.resolved = true;
    bump(rec);
    finish(md_path, &mut meta, id)
}

/// Reject suggestion: resolve without applying body as replacement (editor clears mark).
pub fn reject_suggestion(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    if rec.kind != AnnotationKind::Suggestion {
        return Err(Error::InvalidAnnotationKind {
            id,
            message: "reject requires a suggestion".into(),
        });
    }
    if rec.revision != expected_revision {
        return Err(Error::AnnotationConflict {
            id,
            expected_revision,
            actual_revision: rec.revision,
        });
    }
    if rec.resolved {
        return Ok(AnnotationOpResult {
            annotation: rec.clone(),
            comments: meta.comments.clone(),
            run_id: meta.run.id,
        });
    }
    rec.resolved = true;
    bump(rec);
    finish(md_path, &mut meta, id)
}

/// Import annotations that are missing by ID only (never overwrite, never delete).
/// Used by deprecated full-list helper and tests.
pub fn import_missing_annotations(md_path: &Path, incoming: &[CommentRecord]) -> Result<RunMeta> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let mut changed = false;
    for c in incoming {
        if find_index(&meta, c.id).is_none() {
            let mut rec = c.clone();
            if rec.revision == 0 {
                rec.revision = 1;
            }
            meta.comments.push(rec);
            changed = true;
        }
    }
    if changed {
        meta.touch();
        write_run_meta_unlocked(md_path, &meta)?;
    }
    Ok(meta)
}

/// Safe host reconciliation of a live-session snapshot.
///
/// * New IDs → create
/// * Same content → no-op
/// * Same ID, different content, matching `session.revision` == disk.revision → update + bump
/// * Same ID, different content, revision mismatch → conflict (keep disk)
/// * Disk-only annotations → preserved (never deleted)
pub fn reconcile_session_annotations(
    md_path: &Path,
    session: &[CommentRecord],
) -> Result<ReconcileResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut conflicts = Vec::new();

    for s in session {
        match find_index(&meta, s.id) {
            None => {
                let mut rec = s.clone();
                if rec.revision == 0 {
                    rec.revision = 1;
                }
                // New from session: start at revision 1 if client sent 1.
                rec.revision = rec.revision.max(1);
                meta.comments.push(rec);
                created += 1;
            }
            Some(idx) => {
                let disk = &meta.comments[idx];
                if disk.content_matches(s) {
                    continue;
                }
                if s.revision != disk.revision {
                    conflicts.push(ReconcileConflict {
                        annotation_id: s.id,
                        expected_revision: s.revision,
                        actual_revision: disk.revision,
                        message:
                            "session annotation revision does not match disk; kept disk version"
                                .into(),
                    });
                    continue;
                }
                // Apply session content, bump revision.
                let rev = disk.revision;
                let mut next = s.clone();
                next.revision = rev;
                bump(&mut next);
                // Preserve created_at from disk if session rewrote it casually.
                next.created_at = disk.created_at;
                meta.comments[idx] = next;
                updated += 1;
            }
        }
    }

    if created > 0 || updated > 0 {
        meta.touch();
        write_run_meta_unlocked(md_path, &meta)?;
    }

    Ok(ReconcileResult {
        comments: meta.comments.clone(),
        created,
        updated,
        conflicts,
        run_id: meta.run.id,
    })
}

/// Payload for a single mutation (optional typed dispatch).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AnnotationMutation {
    Create {
        id: Uuid,
        body: String,
        author: String,
        quote: String,
        kind: String,
    },
    Update {
        id: Uuid,
        expected_revision: u32,
        body: Option<String>,
        author: Option<String>,
    },
    Resolve {
        id: Uuid,
        expected_revision: u32,
    },
    Reopen {
        id: Uuid,
        expected_revision: u32,
    },
    AcceptSuggestion {
        id: Uuid,
        expected_revision: u32,
    },
    RejectSuggestion {
        id: Uuid,
        expected_revision: u32,
    },
}

pub fn apply_mutation(md_path: &Path, m: AnnotationMutation) -> Result<AnnotationOpResult> {
    match m {
        AnnotationMutation::Create {
            id,
            body,
            author,
            quote,
            kind,
        } => {
            let kind =
                AnnotationKind::parse(&kind).ok_or_else(|| Error::AnnotationPrecondition {
                    id: Some(id),
                    message: format!("invalid annotation kind: {kind}"),
                })?;
            create_annotation(md_path, id, body, author, quote, kind)
        }
        AnnotationMutation::Update {
            id,
            expected_revision,
            body,
            author,
        } => update_annotation(md_path, id, expected_revision, body, author),
        AnnotationMutation::Resolve {
            id,
            expected_revision,
        } => resolve_annotation(md_path, id, expected_revision),
        AnnotationMutation::Reopen {
            id,
            expected_revision,
        } => reopen_annotation(md_path, id, expected_revision),
        AnnotationMutation::AcceptSuggestion {
            id,
            expected_revision,
        } => accept_suggestion(md_path, id, expected_revision),
        AnnotationMutation::RejectSuggestion {
            id,
            expected_revision,
        } => reject_suggestion(md_path, id, expected_revision),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use tempfile::tempdir;

    use crate::run_meta::{content_hash, load_run_meta_readonly, record_decision, DecisionKind};

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let md = dir.path().join("run.md");
        fs::write(&md, "# run\n").unwrap();
        (dir, md)
    }

    #[test]
    fn create_two_different_survive() {
        let (_d, md) = setup();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        create_annotation(&md, a, "a", "A", "q", AnnotationKind::Comment).unwrap();
        create_annotation(&md, b, "b", "B", "q", AnnotationKind::Suggestion).unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.comments.len(), 2);
    }

    #[test]
    fn concurrent_create_different_ids() {
        let (_d, md) = setup();
        crate::run_meta::ensure_run_meta(&md).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let md1 = md.clone();
        let md2 = md.clone();
        let b1 = barrier.clone();
        let b2 = barrier;
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let t1 = thread::spawn(move || {
            b1.wait();
            create_annotation(&md1, id1, "one", "A", "q", AnnotationKind::Comment).unwrap()
        });
        let t2 = thread::spawn(move || {
            b2.wait();
            create_annotation(&md2, id2, "two", "B", "q", AnnotationKind::Comment).unwrap()
        });
        t1.join().unwrap();
        t2.join().unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.comments.len(), 2);
    }

    #[test]
    fn same_annotation_update_conflict() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        let r = create_annotation(&md, id, "v1", "A", "q", AnnotationKind::Comment).unwrap();
        assert_eq!(r.annotation.revision, 1);
        let ok = update_annotation(&md, id, 1, Some("v2".into()), None).unwrap();
        assert_eq!(ok.annotation.revision, 2);
        let err = update_annotation(&md, id, 1, Some("stale".into()), None).unwrap_err();
        match err {
            Error::AnnotationConflict {
                expected_revision,
                actual_revision,
                ..
            } => {
                assert_eq!(expected_revision, 1);
                assert_eq!(actual_revision, 2);
            }
            e => panic!("expected conflict, got {e}"),
        }
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.comments[0].body, "v2");
    }

    #[test]
    fn concurrent_same_token_one_wins() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "v1", "A", "q", AnnotationKind::Comment).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let md1 = md.clone();
        let md2 = md.clone();
        let b1 = barrier.clone();
        let b2 = barrier;
        let t1 = thread::spawn(move || {
            b1.wait();
            update_annotation(&md1, id, 1, Some("a".into()), None)
        });
        let t2 = thread::spawn(move || {
            b2.wait();
            update_annotation(&md2, id, 1, Some("b".into()), None)
        });
        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();
        let wins = r1.is_ok() as u8 + r2.is_ok() as u8;
        let fails = r1.is_err() as u8 + r2.is_err() as u8;
        assert_eq!(wins, 1);
        assert_eq!(fails, 1);
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.comments.len(), 1);
        assert_eq!(meta.comments[0].revision, 2);
    }

    #[test]
    fn resolve_vs_update_conflict() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "x", "A", "q", AnnotationKind::Comment).unwrap();
        resolve_annotation(&md, id, 1).unwrap();
        let err = update_annotation(&md, id, 1, Some("y".into()), None).unwrap_err();
        assert!(matches!(err, Error::AnnotationConflict { .. }));
    }

    #[test]
    fn accept_vs_reject_conflict() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "q", AnnotationKind::Suggestion).unwrap();
        accept_suggestion(&md, id, 1).unwrap();
        let err = reject_suggestion(&md, id, 1).unwrap_err();
        assert!(matches!(err, Error::AnnotationConflict { .. }));
    }

    #[test]
    fn duplicate_create_errors() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "a", "A", "q", AnnotationKind::Comment).unwrap();
        let err = create_annotation(&md, id, "b", "B", "q", AnnotationKind::Comment).unwrap_err();
        assert!(matches!(err, Error::DuplicateAnnotation { .. }));
    }

    #[test]
    fn missing_id_not_found() {
        let (_d, md) = setup();
        let err = resolve_annotation(&md, Uuid::new_v4(), 1).unwrap_err();
        assert!(matches!(err, Error::AnnotationNotFound { .. }));
    }

    #[test]
    fn resolve_idempotent() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "a", "A", "q", AnnotationKind::Comment).unwrap();
        let r1 = resolve_annotation(&md, id, 1).unwrap();
        assert_eq!(r1.annotation.revision, 2);
        let r2 = resolve_annotation(&md, id, 2).unwrap();
        assert_eq!(r2.annotation.revision, 2);
        assert!(r2.annotation.resolved);
    }

    #[test]
    fn decisions_survive_annotation_ops() {
        let (_d, md) = setup();
        let h = content_hash("# run\n");
        record_decision(&md, DecisionKind::Approved, "R", None, &h).unwrap();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "note", "A", "run", AnnotationKind::Comment).unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.decisions.len(), 1);
        assert_eq!(meta.comments.len(), 1);
    }

    #[test]
    fn reconcile_does_not_delete() {
        let (_d, md) = setup();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        create_annotation(&md, a, "a", "A", "q", AnnotationKind::Comment).unwrap();
        create_annotation(&md, b, "b", "B", "q", AnnotationKind::Comment).unwrap();
        // Session only knows A (stale snapshot).
        let only_a = vec![CommentRecord::new(
            a,
            "a",
            "A",
            "q",
            AnnotationKind::Comment,
        )];
        let res = reconcile_session_annotations(&md, &only_a).unwrap();
        assert_eq!(res.comments.len(), 2);
        assert!(res.comments.iter().any(|c| c.id == b));
    }

    #[test]
    fn reconcile_conflict_keeps_disk() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "disk", "A", "q", AnnotationKind::Comment).unwrap();
        update_annotation(&md, id, 1, Some("disk2".into()), None).unwrap();
        let mut session = CommentRecord::new(id, "session", "A", "q", AnnotationKind::Comment);
        session.revision = 1; // stale
        let res = reconcile_session_annotations(&md, &[session]).unwrap();
        assert_eq!(res.conflicts.len(), 1);
        assert_eq!(res.comments[0].body, "disk2");
    }

    #[test]
    fn concurrent_resolve_and_other_create() {
        let (_d, md) = setup();
        let a = Uuid::new_v4();
        create_annotation(&md, a, "a", "A", "q", AnnotationKind::Comment).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let md1 = md.clone();
        let md2 = md.clone();
        let b1 = barrier.clone();
        let b2 = barrier;
        let id_b = Uuid::new_v4();
        let t1 = thread::spawn(move || {
            b1.wait();
            resolve_annotation(&md1, a, 1).unwrap()
        });
        let t2 = thread::spawn(move || {
            b2.wait();
            create_annotation(&md2, id_b, "b", "B", "q", AnnotationKind::Comment).unwrap()
        });
        t1.join().unwrap();
        t2.join().unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.comments.len(), 2);
        assert!(meta.comments.iter().find(|c| c.id == a).unwrap().resolved);
    }

    #[test]
    fn accept_requires_suggestion() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "c", "A", "q", AnnotationKind::Comment).unwrap();
        let err = accept_suggestion(&md, id, 1).unwrap_err();
        assert!(matches!(err, Error::InvalidAnnotationKind { .. }));
    }

    #[test]
    fn legacy_record_without_revision_loads() {
        let (_d, md) = setup();
        crate::run_meta::ensure_run_meta(&md).unwrap();
        let path = moraine_sidecar_path(&md);
        let raw = fs::read_to_string(&path).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        v["comments"] = serde_json::json!([{
            "id": "00000000-0000-4000-8000-000000000042",
            "body": "legacy",
            "author": "A",
            "quote": "run",
            "createdAt": "2020-01-01T00:00:00Z",
            "resolved": false,
            "kind": "comment"
        }]);
        fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.comments[0].revision, 1);
        let id = meta.comments[0].id;
        resolve_annotation(&md, id, 1).unwrap();
    }
}
