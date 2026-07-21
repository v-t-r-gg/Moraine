//! Project-local session envelopes for deterministic hook capture.
//!
//! Session files are rebuildable runtime state beside the project. Canonical
//! ledger content remains in run bundles.
//!
//! **Cardinality policy (M2):** at most one *active provisional* run per
//! session. A session may accumulate many confirmed runs over time. MCP
//! `run_start` is the authoritative semantic boundary for new runs after the
//! first provisional has been confirmed.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::project::{
    discover_project_root, ensure_project, resolve_existing_project, resolve_or_init_project,
    MORAINE_DIR,
};
use super::types::{CaptureCoverage, MAX_SUMMARY_CHARS};
use crate::atomic::{write_atomic, SidecarLock};
use crate::error::{Error, Result};

pub const SESSION_SCHEMA_VERSION: u32 = 2;
pub const SESSIONS_DIR: &str = "sessions";
pub const MAX_PROMPT_CONTEXT: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub schema_version: u32,
    /// Namespaced durable key: `{integration}:{project_id}:{external_session_id}`.
    pub session_key: String,
    /// Vendor / host session id preserved for diagnostics.
    pub external_session_id: String,
    pub integration: String,
    pub project_id: Uuid,
    /// Locked at first observe; later hook events must not retarget the project.
    pub project_root: String,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    /// At most one active provisional run for this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_provisional_run_id: Option<Uuid>,
    /// Currently capture-active run for this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_active_run_id: Option<Uuid>,
    /// All runs associated with this session (provisional and confirmed).
    #[serde(default)]
    pub run_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_task: Option<String>,
    /// Later user prompts (bounded); never auto-split into new runs.
    #[serde(default)]
    pub prompt_context: Vec<String>,
    /// Observe sources already recorded (startup, resume, user_prompt, stop, …).
    #[serde(default)]
    pub sources_seen: Vec<String>,
}

impl SessionRecord {
    pub fn has_mechanical_hooks(&self) -> bool {
        self.sources_seen.iter().any(|s| {
            matches!(
                s.as_str(),
                "startup"
                    | "resume"
                    | "clear"
                    | "compact"
                    | "user_prompt"
                    | "stop"
                    | "session_start"
            )
        })
    }
}

/// Build the durable namespaced session key.
pub fn namespace_session_key(
    integration: &str,
    project_id: Uuid,
    external_session_id: &str,
) -> String {
    let integration = {
        let i = integration.trim();
        if i.is_empty() {
            "unknown"
        } else {
            i
        }
    };
    format!("{integration}:{project_id}:{external_session_id}")
}

#[derive(Debug, Clone)]
pub struct SessionObserveRequest {
    /// External (vendor) session id — namespaced internally with project + integration.
    pub session_id: String,
    pub integration: String,
    pub project: Option<PathBuf>,
    /// Lifecycle source label (e.g. startup, resume, user_prompt, stop).
    pub source: String,
    pub initial_task: Option<String>,
    /// When true, close the session envelope only (does not mutate run lifecycle).
    pub ended: bool,
    /// When true (hooks), require an existing Moraine project; never auto-init.
    pub confine_existing_project: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionObserveResult {
    pub session_key: String,
    pub external_session_id: String,
    pub project_root: PathBuf,
    pub project_id: Uuid,
    pub active_provisional_run_id: Option<Uuid>,
    pub capture_active_run_id: Option<Uuid>,
    pub run_ids: Vec<Uuid>,
    pub initial_task: Option<String>,
    pub ended: bool,
    pub created: bool,
    /// True when this observe was the first user_prompt and no provisional exists yet.
    pub should_ensure_provisional: bool,
}

pub fn sessions_dir(project_root: &Path) -> PathBuf {
    project_root.join(MORAINE_DIR).join(SESSIONS_DIR)
}

pub fn session_path(project_root: &Path, session_key: &str) -> PathBuf {
    sessions_dir(project_root).join(format!("{}.json", session_file_stem(session_key)))
}

fn session_file_stem(session_key: &str) -> String {
    let trimmed = session_key.trim();
    if trimmed.is_empty() {
        return "empty".into();
    }
    let mut hasher = Sha256::new();
    hasher.update(trimmed.as_bytes());
    let hex = hex::encode(hasher.finalize());
    let safe: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' {
                if c == ':' {
                    '_'
                } else {
                    c
                }
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    format!("{safe}-{}", &hex[..12])
}

fn require_external_session_id(session_id: &str) -> Result<String> {
    let s = session_id.trim();
    if s.is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "session_id is required".into(),
        });
    }
    if s.len() > 256 {
        return Err(Error::InvalidCheckpoint {
            message: "session_id exceeds 256 characters".into(),
        });
    }
    Ok(s.to_string())
}

fn bound_task(task: Option<String>) -> Result<Option<String>> {
    match task {
        None => Ok(None),
        Some(t) => {
            let t = t.trim();
            if t.is_empty() {
                return Ok(None);
            }
            let mut out = String::new();
            for ch in t.chars() {
                if ch == '\n' || ch == '\r' {
                    if !out.ends_with(' ') {
                        out.push(' ');
                    }
                } else if ch == '\0' {
                    continue;
                } else {
                    out.push(ch);
                }
                if out.len() >= MAX_SUMMARY_CHARS {
                    break;
                }
            }
            let out = out.trim().to_string();
            if out.is_empty() {
                Ok(None)
            } else {
                Ok(Some(out))
            }
        }
    }
}

/// Resolve a hook-supplied path to an existing Moraine project only.
/// Never auto-inits; rejects paths that do not discover a `.moraine` root.
pub fn resolve_confined_project(path: Option<&Path>) -> Result<super::project::ProjectInitResult> {
    let hint = match path {
        Some(p) => {
            if p.as_os_str().is_empty() {
                return Err(Error::InvalidCheckpoint {
                    message: "hook project path is empty".into(),
                });
            }
            if p.exists() {
                fs::canonicalize(p).map_err(|e| Error::InvalidCheckpoint {
                    message: format!("cannot canonicalize project path: {e}"),
                })?
            } else {
                return Err(Error::ProjectNotFound {
                    path: p.to_path_buf(),
                });
            }
        }
        None => std::env::current_dir()?,
    };
    // Ensure discovered root is a prefix of the canonical hint (no escape).
    let root = discover_project_root(&hint)
        .ok_or_else(|| Error::ProjectNotFound { path: hint.clone() })?;
    let root_canon = fs::canonicalize(&root).unwrap_or(root.clone());
    if !hint.starts_with(&root_canon) && hint != root_canon {
        return Err(Error::InvalidCheckpoint {
            message: "project path escapes discovered Moraine project root".into(),
        });
    }
    resolve_existing_project(Some(&root_canon))
}

pub fn load_session(project_root: &Path, session_key: &str) -> Result<Option<SessionRecord>> {
    let path = session_path(project_root, session_key);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    let mut rec: SessionRecord = serde_json::from_str(&raw)?;
    if rec.schema_version > SESSION_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchemaVersion {
            version: rec.schema_version,
            max: SESSION_SCHEMA_VERSION,
        });
    }
    // Soft-migrate v1 → v2 field defaults already via serde defaults.
    if rec.schema_version < SESSION_SCHEMA_VERSION {
        rec.schema_version = SESSION_SCHEMA_VERSION;
    }
    Ok(Some(rec))
}

pub fn update_session<F, T>(project_root: &Path, session_key: &str, f: F) -> Result<T>
where
    F: FnOnce(&mut SessionRecord) -> Result<T>,
{
    let path = session_path(project_root, session_key);
    let dir = sessions_dir(project_root);
    fs::create_dir_all(&dir)?;
    let _lock = SidecarLock::acquire(&path)?;
    let mut rec = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw)?
    } else {
        return Err(Error::other(format!("session not found: {session_key}")));
    };
    let out = f(&mut rec)?;
    let raw = serde_json::to_string_pretty(&rec)?;
    write_atomic(&path, format!("{raw}\n").as_bytes())?;
    Ok(out)
}

/// Upsert a session envelope.
///
/// `session_stop` / `ended` only updates the envelope — never run lifecycle.
pub fn session_observe(req: SessionObserveRequest) -> Result<SessionObserveResult> {
    let external = require_external_session_id(&req.session_id)?;
    let source = req.source.trim();
    if source.is_empty() {
        return Err(Error::InvalidCheckpoint {
            message: "session observe source is required".into(),
        });
    }
    let integration = {
        let i = req.integration.trim();
        if i.is_empty() {
            "unknown"
        } else {
            i
        }
    }
    .to_string();
    let prompt = bound_task(req.initial_task)?;

    let project = if req.confine_existing_project {
        resolve_confined_project(req.project.as_deref())?
    } else {
        resolve_or_init_project(req.project.as_deref())?
    };
    let project_root = project.project_root.clone();
    ensure_project(&project_root)?;
    fs::create_dir_all(sessions_dir(&project_root))?;

    let session_key = namespace_session_key(&integration, project.project_id, &external);
    let path = session_path(&project_root, &session_key);
    let _lock = SidecarLock::acquire(&path)?;

    let mut created = false;
    let mut rec = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let existing: SessionRecord = serde_json::from_str(&raw)?;
        // Reject project retarget for the same session key.
        let locked = PathBuf::from(&existing.project_root);
        let locked_canon = fs::canonicalize(&locked).unwrap_or(locked);
        if locked_canon != project_root {
            return Err(Error::InvalidCheckpoint {
                message: format!(
                    "session {} is locked to project {}, refusing {}",
                    session_key,
                    existing.project_root,
                    project_root.display()
                ),
            });
        }
        existing
    } else {
        created = true;
        SessionRecord {
            schema_version: SESSION_SCHEMA_VERSION,
            session_key: session_key.clone(),
            external_session_id: external.clone(),
            integration: integration.clone(),
            project_id: project.project_id,
            project_root: project_root.display().to_string(),
            started_at: Utc::now(),
            ended_at: None,
            active_provisional_run_id: None,
            capture_active_run_id: None,
            run_ids: vec![],
            initial_task: None,
            prompt_context: vec![],
            sources_seen: vec![],
        }
    };

    if !rec.sources_seen.iter().any(|s| s == source) {
        rec.sources_seen.push(source.to_string());
    }
    if rec.integration == "unknown" && integration != "unknown" {
        rec.integration = integration;
    }

    let is_user_prompt = source == "user_prompt";
    let should_ensure_provisional = is_user_prompt
        && rec.active_provisional_run_id.is_none()
        && rec.run_ids.is_empty()
        && !req.ended;

    if is_user_prompt {
        if let Some(t) = prompt {
            if rec.initial_task.is_none() {
                rec.initial_task = Some(t.clone());
            } else if rec.initial_task.as_ref() != Some(&t)
                && rec.prompt_context.len() < MAX_PROMPT_CONTEXT
                && !rec.prompt_context.iter().any(|p| p == &t)
            {
                rec.prompt_context.push(t);
            }
        }
    } else if rec.initial_task.is_none() {
        if let Some(t) = prompt {
            rec.initial_task = Some(t);
        }
    }

    // Envelope close only — never touches run lifecycle / ready_for_review.
    if req.ended {
        rec.ended_at = Some(Utc::now());
    }

    let raw = serde_json::to_string_pretty(&rec)?;
    write_atomic(&path, format!("{raw}\n").as_bytes())?;

    Ok(SessionObserveResult {
        session_key: rec.session_key,
        external_session_id: rec.external_session_id,
        project_root,
        project_id: project.project_id,
        active_provisional_run_id: rec.active_provisional_run_id,
        capture_active_run_id: rec.capture_active_run_id,
        run_ids: rec.run_ids,
        initial_task: rec.initial_task,
        ended: rec.ended_at.is_some(),
        created,
        should_ensure_provisional,
    })
}

pub fn set_session_provisional_run(
    project_root: &Path,
    session_key: &str,
    run_id: Uuid,
) -> Result<()> {
    update_session(project_root, session_key, |rec| {
        rec.active_provisional_run_id = Some(run_id);
        rec.capture_active_run_id = Some(run_id);
        if !rec.run_ids.contains(&run_id) {
            rec.run_ids.push(run_id);
        }
        Ok(())
    })
}

/// After confirm or a new semantic start: clear provisional pointer and record the run.
pub fn register_session_run(
    project_root: &Path,
    session_key: &str,
    run_id: Uuid,
    clear_provisional: bool,
) -> Result<()> {
    update_session(project_root, session_key, |rec| {
        if clear_provisional && rec.active_provisional_run_id == Some(run_id) {
            rec.active_provisional_run_id = None;
        }
        rec.capture_active_run_id = Some(run_id);
        if !rec.run_ids.contains(&run_id) {
            rec.run_ids.push(run_id);
        }
        Ok(())
    })
}

/// Derive capture coverage from observable state (never agent-declared).
pub fn derive_capture_coverage(
    provisional: bool,
    session: Option<&SessionRecord>,
    checkpoint_count: usize,
) -> CaptureCoverage {
    let mechanical = session
        .map(|s| {
            s.has_mechanical_hooks()
                || s.sources_seen
                    .iter()
                    .any(|x| x == "provisional_ensure" || x == "session_start")
        })
        .unwrap_or(false);
    let semantic = !provisional || checkpoint_count > 0;
    match (mechanical, semantic) {
        (true, true) if !provisional => CaptureCoverage::Full,
        (true, _) if provisional => CaptureCoverage::MechanicalOnly,
        (true, false) => CaptureCoverage::MechanicalOnly,
        (false, true) if !provisional => CaptureCoverage::SemanticOnly,
        (false, true) => CaptureCoverage::Unknown,
        (false, false) => CaptureCoverage::Unknown,
        (true, true) => CaptureCoverage::Full,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn namespace_includes_integration_and_project() {
        let id = Uuid::nil();
        let k = namespace_session_key("codex", id, "ext-1");
        assert_eq!(k, format!("codex:{id}:ext-1"));
    }

    #[test]
    fn later_prompts_do_not_request_new_provisional() {
        let dir = tempdir().unwrap();
        let project = crate::agent_protocol::project::init_project(Some(dir.path())).unwrap();
        let first = session_observe(SessionObserveRequest {
            session_id: "ext".into(),
            integration: "codex".into(),
            project: Some(project.project_root.clone()),
            source: "user_prompt".into(),
            initial_task: Some("Feature X".into()),
            ended: false,
            confine_existing_project: true,
        })
        .unwrap();
        assert!(first.should_ensure_provisional);
        set_session_provisional_run(&project.project_root, &first.session_key, Uuid::new_v4())
            .unwrap();

        let second = session_observe(SessionObserveRequest {
            session_id: "ext".into(),
            integration: "codex".into(),
            project: Some(project.project_root.clone()),
            source: "user_prompt".into(),
            initial_task: Some("Also add a test".into()),
            ended: false,
            confine_existing_project: true,
        })
        .unwrap();
        assert!(!second.should_ensure_provisional);
        let rec = load_session(&project.project_root, &first.session_key)
            .unwrap()
            .unwrap();
        assert_eq!(rec.initial_task.as_deref(), Some("Feature X"));
        assert!(rec.prompt_context.iter().any(|p| p.contains("test")));
    }

    #[test]
    fn stop_does_not_clear_runs() {
        let dir = tempdir().unwrap();
        let project = crate::agent_protocol::project::init_project(Some(dir.path())).unwrap();
        let obs = session_observe(SessionObserveRequest {
            session_id: "ext".into(),
            integration: "codex".into(),
            project: Some(project.project_root.clone()),
            source: "startup".into(),
            initial_task: None,
            ended: false,
            confine_existing_project: true,
        })
        .unwrap();
        let rid = Uuid::new_v4();
        set_session_provisional_run(&project.project_root, &obs.session_key, rid).unwrap();
        let stopped = session_observe(SessionObserveRequest {
            session_id: "ext".into(),
            integration: "codex".into(),
            project: Some(project.project_root.clone()),
            source: "stop".into(),
            initial_task: None,
            ended: true,
            confine_existing_project: true,
        })
        .unwrap();
        assert!(stopped.ended);
        assert_eq!(stopped.active_provisional_run_id, Some(rid));
    }

    #[test]
    fn confined_project_rejects_missing_moraine() {
        let dir = tempdir().unwrap();
        let err = resolve_confined_project(Some(dir.path())).unwrap_err();
        assert!(matches!(err, Error::ProjectNotFound { .. }));
    }
}
