//! Operation-based annotation mutations under the per-document ledger lock.

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic::SidecarLock;
use crate::comments::{AnnotationKind, CommentRecord, SuggestionDisposition};
use crate::error::{Error, Result};
use crate::run_meta::{
    content_hash, load_or_migrate_locked, moraine_sidecar_path, write_run_meta_unlocked, RunMeta,
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
pub struct BeginAcceptResult {
    pub annotation: CommentRecord,
    pub comments: Vec<CommentRecord>,
    pub run_id: Uuid,
    pub acceptance_op_id: Uuid,
    pub base_content_hash: String,
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

fn bump_checked(rec: &mut CommentRecord) -> Result<()> {
    let next = rec
        .revision
        .checked_add(1)
        .ok_or(Error::RevisionOverflow { id: rec.id })?;
    rec.revision = next;
    Ok(())
}

fn check_revision(rec: &CommentRecord, expected: u32) -> Result<()> {
    if rec.revision != expected {
        return Err(Error::AnnotationConflict {
            id: rec.id,
            expected_revision: expected,
            actual_revision: rec.revision,
        });
    }
    Ok(())
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

/// Create a new annotation. Fails if `id` already exists. Always revision 1.
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
    check_revision(rec, expected_revision)?;
    if rec.kind == AnnotationKind::Suggestion {
        let d = rec
            .suggestion_disposition()
            .unwrap_or(SuggestionDisposition::Pending);
        if d == SuggestionDisposition::Accepting {
            return Err(Error::IncompleteAcceptance {
                id,
                message: "cannot update while acceptance is in progress; cancel first".into(),
            });
        }
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
    bump_checked(rec)?;
    finish(md_path, &mut meta, id)
}

/// Resolve a comment (not a suggestion disposition). Suggestions should use reject/accept.
pub fn resolve_annotation(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    check_revision(rec, expected_revision)?;
    if rec.kind == AnnotationKind::Suggestion {
        return Err(Error::InvalidAnnotationKind {
            id,
            message: "use accept_suggestion or reject_suggestion for suggestions".into(),
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
    bump_checked(rec)?;
    finish(md_path, &mut meta, id)
}

/// Reopen a comment, or return a suggestion to pending (clearing terminal outcome).
pub fn reopen_annotation(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    check_revision(rec, expected_revision)?;

    if rec.kind == AnnotationKind::Suggestion {
        let d = rec
            .suggestion_disposition()
            .unwrap_or(SuggestionDisposition::Pending);
        if d == SuggestionDisposition::Accepting {
            return Err(Error::IncompleteAcceptance {
                id,
                message: "cancel the active acceptance before reopening".into(),
            });
        }
        if d == SuggestionDisposition::Pending {
            return Ok(AnnotationOpResult {
                annotation: rec.clone(),
                comments: meta.comments.clone(),
                run_id: meta.run.id,
            });
        }
        rec.disposition = Some(SuggestionDisposition::Pending);
        rec.resolved = false;
        rec.clear_acceptance_fields();
        rec.applied_content_hash = None;
        rec.acceptance_completed_at = None;
        bump_checked(rec)?;
        return finish(md_path, &mut meta, id);
    }

    if !rec.resolved {
        return Ok(AnnotationOpResult {
            annotation: rec.clone(),
            comments: meta.comments.clone(),
            run_id: meta.run.id,
        });
    }
    rec.resolved = false;
    bump_checked(rec)?;
    finish(md_path, &mut meta, id)
}

/// Phase A: reserve acceptance (no Markdown change yet).
pub fn begin_accept_suggestion(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
    expected_markdown_hash: &str,
) -> Result<BeginAcceptResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;

    let markdown = crate::document::Document::read_file(md_path)?;
    let actual_hash = content_hash(&markdown);
    if actual_hash != expected_markdown_hash {
        return Err(Error::RevisionConflict {
            expected: expected_markdown_hash.to_string(),
            actual: actual_hash,
        });
    }

    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    if rec.kind != AnnotationKind::Suggestion {
        return Err(Error::InvalidAnnotationKind {
            id,
            message: "begin_accept requires a suggestion".into(),
        });
    }
    check_revision(rec, expected_revision)?;
    let d = rec
        .suggestion_disposition()
        .unwrap_or(SuggestionDisposition::Pending);
    match d {
        SuggestionDisposition::Pending => {}
        SuggestionDisposition::Accepting => {
            return Err(Error::IncompleteAcceptance {
                id,
                message: "acceptance already in progress".into(),
            });
        }
        SuggestionDisposition::Accepted => {
            return Err(Error::AnnotationPrecondition {
                id: Some(id),
                message: "suggestion already accepted".into(),
            });
        }
        SuggestionDisposition::Rejected => {
            return Err(Error::AnnotationPrecondition {
                id: Some(id),
                message: "suggestion already rejected".into(),
            });
        }
        SuggestionDisposition::ResolvedLegacy => {
            return Err(Error::AnnotationPrecondition {
                id: Some(id),
                message: "legacy resolved suggestion cannot be accepted; reopen first".into(),
            });
        }
    }

    let op_id = Uuid::new_v4();
    rec.disposition = Some(SuggestionDisposition::Accepting);
    rec.resolved = false;
    rec.acceptance_op_id = Some(op_id);
    rec.acceptance_base_hash = Some(actual_hash.clone());
    rec.acceptance_started_at = Some(Utc::now());
    bump_checked(rec)?;
    meta.touch();
    write_run_meta_unlocked(md_path, &meta)?;
    let annotation = meta.comments[idx].clone();
    Ok(BeginAcceptResult {
        acceptance_op_id: op_id,
        base_content_hash: actual_hash,
        annotation,
        comments: meta.comments.clone(),
        run_id: meta.run.id,
    })
}

/// Phase C: finalize after Markdown saved with applied suggestion.
pub fn complete_accept_suggestion(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
    acceptance_op_id: Uuid,
    expected_saved_hash: &str,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;

    let markdown = crate::document::Document::read_file(md_path)?;
    let actual_hash = content_hash(&markdown);
    if actual_hash != expected_saved_hash {
        return Err(Error::RevisionConflict {
            expected: expected_saved_hash.to_string(),
            actual: actual_hash,
        });
    }

    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    if rec.kind != AnnotationKind::Suggestion {
        return Err(Error::InvalidAnnotationKind {
            id,
            message: "complete_accept requires a suggestion".into(),
        });
    }
    check_revision(rec, expected_revision)?;
    if rec.disposition != Some(SuggestionDisposition::Accepting) {
        return Err(Error::IncompleteAcceptance {
            id,
            message: "suggestion is not in accepting state".into(),
        });
    }
    if rec.acceptance_op_id != Some(acceptance_op_id) {
        return Err(Error::IncompleteAcceptance {
            id,
            message: "acceptance operation id does not match".into(),
        });
    }

    rec.disposition = Some(SuggestionDisposition::Accepted);
    rec.resolved = true;
    rec.applied_content_hash = Some(actual_hash);
    rec.acceptance_completed_at = Some(Utc::now());
    rec.clear_acceptance_fields();
    bump_checked(rec)?;
    finish(md_path, &mut meta, id)
}

/// Cancel an active acceptance reservation; return to pending.
///
/// Cancellation is only allowed when the current Markdown content hash still equals
/// the stored `acceptance_base_hash` (no durable document change since begin).
pub fn cancel_accept_suggestion(
    md_path: &Path,
    id: Uuid,
    expected_revision: u32,
    acceptance_op_id: Uuid,
) -> Result<AnnotationOpResult> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;

    let markdown = crate::document::Document::read_file(md_path)?;
    let current_hash = content_hash(&markdown);

    let idx = find_index(&meta, id).ok_or(Error::AnnotationNotFound { id })?;
    let rec = &mut meta.comments[idx];
    check_revision(rec, expected_revision)?;
    if rec.disposition != Some(SuggestionDisposition::Accepting) {
        return Err(Error::IncompleteAcceptance {
            id,
            message: "suggestion is not in accepting state".into(),
        });
    }
    if rec.acceptance_op_id != Some(acceptance_op_id) {
        return Err(Error::IncompleteAcceptance {
            id,
            message: "acceptance operation id does not match".into(),
        });
    }
    let base = rec
        .acceptance_base_hash
        .clone()
        .ok_or_else(|| Error::IncompleteAcceptance {
            id,
            message: "acceptance base content hash is missing".into(),
        })?;
    if current_hash != base {
        return Err(Error::AcceptanceDocumentChanged {
            id,
            base_content_hash: base,
            current_content_hash: current_hash,
        });
    }

    rec.disposition = Some(SuggestionDisposition::Pending);
    rec.resolved = false;
    rec.clear_acceptance_fields();
    bump_checked(rec)?;
    finish(md_path, &mut meta, id)
}

/// Snapshot for UI recovery of an incomplete acceptance (no mutation).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceRecoveryStatus {
    pub annotation_id: Uuid,
    pub disposition: SuggestionDisposition,
    pub revision: u32,
    pub acceptance_op_id: Option<Uuid>,
    pub base_content_hash: Option<String>,
    pub current_content_hash: String,
    /// True when current disk Markdown hash equals acceptance base (cancel is safe).
    pub cancel_safe: bool,
}

/// Read recovery status for an accepting suggestion without mutating state.
pub fn acceptance_recovery_status(md_path: &Path, id: Uuid) -> Result<AcceptanceRecoveryStatus> {
    let markdown = crate::document::Document::read_file(md_path)?;
    let current_hash = content_hash(&markdown);
    let meta = load_run_meta_readonly_or_err(md_path)?;
    let rec = meta
        .comments
        .iter()
        .find(|c| c.id == id)
        .ok_or(Error::AnnotationNotFound { id })?;
    let d = rec
        .suggestion_disposition()
        .unwrap_or(SuggestionDisposition::Pending);
    let base = rec.acceptance_base_hash.clone();
    let cancel_safe = matches!(
        (d, base.as_ref()),
        (SuggestionDisposition::Accepting, Some(b)) if b == &current_hash
    );
    Ok(AcceptanceRecoveryStatus {
        annotation_id: id,
        disposition: d,
        revision: rec.revision,
        acceptance_op_id: rec.acceptance_op_id,
        base_content_hash: base,
        current_content_hash: current_hash,
        cancel_safe,
    })
}

fn load_run_meta_readonly_or_err(md_path: &Path) -> Result<RunMeta> {
    crate::run_meta::load_run_meta_readonly(md_path)?
        .ok_or_else(|| Error::other("run ledger not initialized"))
}

/// Reject a pending suggestion.
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
    check_revision(rec, expected_revision)?;
    let d = rec
        .suggestion_disposition()
        .unwrap_or(SuggestionDisposition::Pending);
    match d {
        SuggestionDisposition::Rejected => {
            return Ok(AnnotationOpResult {
                annotation: rec.clone(),
                comments: meta.comments.clone(),
                run_id: meta.run.id,
            });
        }
        SuggestionDisposition::Accepted => {
            return Err(Error::AnnotationPrecondition {
                id: Some(id),
                message: "cannot reject an accepted suggestion".into(),
            });
        }
        SuggestionDisposition::Accepting => {
            return Err(Error::IncompleteAcceptance {
                id,
                message: "cancel acceptance before rejecting".into(),
            });
        }
        SuggestionDisposition::ResolvedLegacy => {
            return Err(Error::AnnotationPrecondition {
                id: Some(id),
                message: "reopen legacy resolved suggestion before rejecting".into(),
            });
        }
        SuggestionDisposition::Pending => {}
    }
    rec.disposition = Some(SuggestionDisposition::Rejected);
    rec.resolved = true;
    rec.clear_acceptance_fields();
    bump_checked(rec)?;
    finish(md_path, &mut meta, id)
}

/// Import missing IDs only. New records always revision 1.
pub fn import_missing_annotations(md_path: &Path, incoming: &[CommentRecord]) -> Result<RunMeta> {
    let side = moraine_sidecar_path(md_path);
    let _lock = SidecarLock::acquire(&side)?;
    let mut meta = ensure_meta_locked(md_path)?;
    let mut changed = false;
    for c in incoming {
        if find_index(&meta, c.id).is_none() {
            let mut rec = CommentRecord::new(
                c.id,
                c.body.clone(),
                c.author.clone(),
                c.quote.clone(),
                c.kind,
            );
            // Preserve body/kind from client but force revision 1 and disposition rules.
            rec.body = c.body.clone();
            rec.author = c.author.clone();
            rec.quote = c.quote.clone();
            rec.created_at = c.created_at;
            rec.revision = 1;
            if rec.kind == AnnotationKind::Suggestion {
                rec.disposition = Some(SuggestionDisposition::Pending);
                rec.resolved = false;
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
                // Always revision 1 for unknown IDs; ignore client revision/disposition terminals.
                let mut rec = CommentRecord::new(
                    s.id,
                    s.body.clone(),
                    s.author.clone(),
                    s.quote.clone(),
                    s.kind,
                );
                rec.created_at = s.created_at;
                if rec.kind == AnnotationKind::Comment {
                    rec.resolved = s.resolved;
                } else {
                    rec.disposition = Some(SuggestionDisposition::Pending);
                    rec.resolved = false;
                }
                rec.revision = 1;
                meta.comments.push(rec);
                created += 1;
            }
            Some(idx) => {
                let disk = &meta.comments[idx];
                if disk.content_matches(s) {
                    continue;
                }
                // Protect accepting and terminal outcomes from stale session pending.
                if disk.kind == AnnotationKind::Suggestion {
                    let dd = disk
                        .suggestion_disposition()
                        .unwrap_or(SuggestionDisposition::Pending);
                    let sd = s
                        .suggestion_disposition()
                        .unwrap_or(SuggestionDisposition::Pending);
                    if dd == SuggestionDisposition::Accepting {
                        conflicts.push(ReconcileConflict {
                            annotation_id: s.id,
                            expected_revision: s.revision,
                            actual_revision: disk.revision,
                            message: "disk has active acceptance reservation; kept disk".into(),
                        });
                        continue;
                    }
                    if dd.is_terminal() && sd == SuggestionDisposition::Pending {
                        conflicts.push(ReconcileConflict {
                            annotation_id: s.id,
                            expected_revision: s.revision,
                            actual_revision: disk.revision,
                            message:
                                "session pending cannot revert durable accepted/rejected suggestion"
                                    .into(),
                        });
                        continue;
                    }
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
                let rev = disk.revision;
                let created_at = disk.created_at;
                let mut next = s.clone();
                next.revision = rev;
                next.created_at = created_at;
                next.normalize_compat();
                if next.kind == AnnotationKind::Suggestion {
                    if next.disposition.is_none() {
                        next.disposition = Some(if next.resolved {
                            SuggestionDisposition::ResolvedLegacy
                        } else {
                            SuggestionDisposition::Pending
                        });
                    }
                    next.sync_resolved_from_disposition();
                }
                if let Err(e) = bump_checked(&mut next) {
                    conflicts.push(ReconcileConflict {
                        annotation_id: s.id,
                        expected_revision: rev,
                        actual_revision: rev,
                        message: e.to_string(),
                    });
                    continue;
                }
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
        AnnotationMutation::RejectSuggestion {
            id,
            expected_revision,
        } => reject_suggestion(md_path, id, expected_revision),
    }
}

// Keep old name as alias for reject only path used by UI after two-phase accept.
pub use reject_suggestion as reject_suggestion_op;

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

    fn hash_md(md: &Path) -> String {
        content_hash(&fs::read_to_string(md).unwrap())
    }

    #[test]
    fn create_suggestion_pending() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        let r = create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        assert_eq!(
            r.annotation.disposition,
            Some(SuggestionDisposition::Pending)
        );
        assert!(!r.annotation.resolved);
        assert_eq!(r.annotation.revision, 1);
    }

    #[test]
    fn reject_sets_rejected() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let r = reject_suggestion(&md, id, 1).unwrap();
        assert_eq!(
            r.annotation.disposition,
            Some(SuggestionDisposition::Rejected)
        );
        assert!(r.annotation.resolved);
        assert_eq!(r.annotation.revision, 2);
    }

    #[test]
    fn two_phase_accept_success() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "NEW", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h0 = hash_md(&md);
        let begin = begin_accept_suggestion(&md, id, 1, &h0).unwrap();
        assert_eq!(
            begin.annotation.disposition,
            Some(SuggestionDisposition::Accepting)
        );
        assert_eq!(begin.annotation.revision, 2);
        // Apply markdown change and save
        fs::write(&md, "# NEW\n").unwrap();
        let h1 = hash_md(&md);
        let done = complete_accept_suggestion(&md, id, 2, begin.acceptance_op_id, &h1).unwrap();
        assert_eq!(
            done.annotation.disposition,
            Some(SuggestionDisposition::Accepted)
        );
        assert_eq!(done.annotation.revision, 3);
        assert_eq!(
            done.annotation.applied_content_hash.as_deref(),
            Some(h1.as_str())
        );
    }

    #[test]
    fn begin_requires_markdown_hash() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let err = begin_accept_suggestion(&md, id, 1, "0".repeat(64).as_str()).unwrap_err();
        assert!(matches!(err, Error::RevisionConflict { .. }));
    }

    #[test]
    fn concurrent_begin_one_wins() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h = hash_md(&md);
        let barrier = Arc::new(Barrier::new(2));
        let md1 = md.clone();
        let md2 = md.clone();
        let h1 = h.clone();
        let h2 = h;
        let b1 = barrier.clone();
        let b2 = barrier;
        let t1 = thread::spawn(move || {
            b1.wait();
            begin_accept_suggestion(&md1, id, 1, &h1)
        });
        let t2 = thread::spawn(move || {
            b2.wait();
            begin_accept_suggestion(&md2, id, 1, &h2)
        });
        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();
        assert_eq!(r1.is_ok() as u8 + r2.is_ok() as u8, 1);
        assert_eq!(r1.is_err() as u8 + r2.is_err() as u8, 1);
    }

    #[test]
    fn cancel_restores_pending() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h = hash_md(&md);
        let b = begin_accept_suggestion(&md, id, 1, &h).unwrap();
        let c = cancel_accept_suggestion(&md, id, 2, b.acceptance_op_id).unwrap();
        assert_eq!(
            c.annotation.disposition,
            Some(SuggestionDisposition::Pending)
        );
        assert_eq!(c.annotation.revision, 3);
    }

    #[test]
    fn cancel_after_markdown_change_fails_and_preserves_accepting() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h = hash_md(&md);
        let b = begin_accept_suggestion(&md, id, 1, &h).unwrap();
        fs::write(&md, "# changed\n").unwrap();
        let err = cancel_accept_suggestion(&md, id, 2, b.acceptance_op_id).unwrap_err();
        match err {
            Error::AcceptanceDocumentChanged {
                base_content_hash,
                current_content_hash,
                ..
            } => {
                assert_eq!(base_content_hash, h);
                assert_ne!(current_content_hash, h);
            }
            e => panic!("expected AcceptanceDocumentChanged, got {e}"),
        }
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        let rec = meta.comments.iter().find(|c| c.id == id).unwrap();
        assert_eq!(rec.disposition, Some(SuggestionDisposition::Accepting));
        assert_eq!(rec.acceptance_op_id, Some(b.acceptance_op_id));
        assert_eq!(rec.acceptance_base_hash.as_deref(), Some(h.as_str()));
        assert_eq!(rec.revision, 2);

        // Explicit finalize against current disk succeeds
        let h1 = hash_md(&md);
        let done = complete_accept_suggestion(&md, id, 2, b.acceptance_op_id, &h1).unwrap();
        assert_eq!(
            done.annotation.disposition,
            Some(SuggestionDisposition::Accepted)
        );
    }

    #[test]
    fn recovery_status_cancel_safe_flag() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h = hash_md(&md);
        begin_accept_suggestion(&md, id, 1, &h).unwrap();
        let st = acceptance_recovery_status(&md, id).unwrap();
        assert!(st.cancel_safe);
        fs::write(&md, "x\n").unwrap();
        let st2 = acceptance_recovery_status(&md, id).unwrap();
        assert!(!st2.cancel_safe);
        assert_eq!(st2.disposition, SuggestionDisposition::Accepting);
    }

    #[test]
    fn complete_retry_after_failed_hash_then_ok() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h0 = hash_md(&md);
        let b = begin_accept_suggestion(&md, id, 1, &h0).unwrap();
        fs::write(&md, "applied\n").unwrap();
        let h1 = hash_md(&md);
        // Wrong hash fails
        let err = complete_accept_suggestion(&md, id, 2, b.acceptance_op_id, &h0).unwrap_err();
        assert!(matches!(err, Error::RevisionConflict { .. }));
        // Retry with correct hash
        let done = complete_accept_suggestion(&md, id, 2, b.acceptance_op_id, &h1).unwrap();
        assert_eq!(
            done.annotation.disposition,
            Some(SuggestionDisposition::Accepted)
        );
    }

    #[test]
    fn reject_accepted_fails() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h0 = hash_md(&md);
        let b = begin_accept_suggestion(&md, id, 1, &h0).unwrap();
        fs::write(&md, "x\n").unwrap();
        let h1 = hash_md(&md);
        complete_accept_suggestion(&md, id, 2, b.acceptance_op_id, &h1).unwrap();
        let err = reject_suggestion(&md, id, 3).unwrap_err();
        assert!(matches!(err, Error::AnnotationPrecondition { .. }));
    }

    #[test]
    fn accept_rejected_fails() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        reject_suggestion(&md, id, 1).unwrap();
        let h = hash_md(&md);
        let err = begin_accept_suggestion(&md, id, 2, &h).unwrap_err();
        assert!(matches!(err, Error::AnnotationPrecondition { .. }));
    }

    #[test]
    fn reject_idempotent() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        reject_suggestion(&md, id, 1).unwrap();
        let r = reject_suggestion(&md, id, 2).unwrap();
        assert_eq!(r.annotation.revision, 2);
    }

    #[test]
    fn new_session_ids_force_revision_1() {
        let (_d, md) = setup();
        crate::run_meta::ensure_run_meta(&md).unwrap();
        let mut s0 = CommentRecord::new(Uuid::new_v4(), "a", "A", "q", AnnotationKind::Suggestion);
        s0.revision = 0;
        let mut s999 =
            CommentRecord::new(Uuid::new_v4(), "b", "B", "q", AnnotationKind::Suggestion);
        s999.revision = 999;
        let res = reconcile_session_annotations(&md, &[s0, s999]).unwrap();
        assert_eq!(res.created, 2);
        for c in res.comments {
            assert_eq!(c.revision, 1);
            assert_eq!(c.disposition, Some(SuggestionDisposition::Pending));
        }
    }

    #[test]
    fn reconcile_stale_pending_vs_accepted() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h0 = hash_md(&md);
        let b = begin_accept_suggestion(&md, id, 1, &h0).unwrap();
        fs::write(&md, "done\n").unwrap();
        let h1 = hash_md(&md);
        complete_accept_suggestion(&md, id, 2, b.acceptance_op_id, &h1).unwrap();
        let mut stale = CommentRecord::new(id, "rep", "A", "run", AnnotationKind::Suggestion);
        stale.revision = 1;
        stale.disposition = Some(SuggestionDisposition::Pending);
        let res = reconcile_session_annotations(&md, &[stale]).unwrap();
        assert_eq!(res.conflicts.len(), 1);
        assert_eq!(
            res.comments[0].disposition,
            Some(SuggestionDisposition::Accepted)
        );
    }

    #[test]
    fn reconcile_vs_accepting() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h = hash_md(&md);
        begin_accept_suggestion(&md, id, 1, &h).unwrap();
        let mut s = CommentRecord::new(id, "rep", "A", "run", AnnotationKind::Suggestion);
        s.revision = 2;
        s.disposition = Some(SuggestionDisposition::Pending);
        let res = reconcile_session_annotations(&md, &[s]).unwrap();
        assert_eq!(res.conflicts.len(), 1);
        assert_eq!(
            res.comments[0].disposition,
            Some(SuggestionDisposition::Accepting)
        );
    }

    #[test]
    fn complete_requires_op_id() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h0 = hash_md(&md);
        begin_accept_suggestion(&md, id, 1, &h0).unwrap();
        fs::write(&md, "x\n").unwrap();
        let h1 = hash_md(&md);
        let err = complete_accept_suggestion(&md, id, 2, Uuid::new_v4(), &h1).unwrap_err();
        assert!(matches!(err, Error::IncompleteAcceptance { .. }));
    }

    #[test]
    fn decisions_survive() {
        let (_d, md) = setup();
        let h = content_hash("# run\n");
        record_decision(&md, DecisionKind::Approved, "R", None, &h).unwrap();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "n", "A", "run", AnnotationKind::Comment).unwrap();
        let meta = load_run_meta_readonly(&md).unwrap().unwrap();
        assert_eq!(meta.decisions.len(), 1);
        assert_eq!(meta.schema_version, crate::run_meta::SCHEMA_VERSION);
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
        assert_eq!(
            load_run_meta_readonly(&md).unwrap().unwrap().comments.len(),
            2
        );
    }

    #[test]
    fn reopen_accepted_to_pending() {
        let (_d, md) = setup();
        let id = Uuid::new_v4();
        create_annotation(&md, id, "rep", "A", "run", AnnotationKind::Suggestion).unwrap();
        let h0 = hash_md(&md);
        let b = begin_accept_suggestion(&md, id, 1, &h0).unwrap();
        fs::write(&md, "x\n").unwrap();
        let h1 = hash_md(&md);
        complete_accept_suggestion(&md, id, 2, b.acceptance_op_id, &h1).unwrap();
        let r = reopen_annotation(&md, id, 3).unwrap();
        assert_eq!(
            r.annotation.disposition,
            Some(SuggestionDisposition::Pending)
        );
        assert!(!r.annotation.resolved);
    }
}
