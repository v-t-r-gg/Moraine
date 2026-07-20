//! Project-local session envelopes for deterministic hook capture.
//!
//! Session files are rebuildable runtime state beside the project. Canonical
//! ledger content remains in run bundles.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::project::{ensure_project, resolve_or_init_project, MORAINE_DIR};
use super::types::MAX_SUMMARY_CHARS;
use crate::atomic::{write_atomic, SidecarLock};
use crate::error::{Error, Result};

pub const SESSION_SCHEMA_VERSION: u32 = 1;
pub const SESSIONS_DIR: &str = "sessions";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub schema_version: u32,
    pub session_id: String,
    pub integration: String,
    pub project_root: String,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_run_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_task: Option<String>,
    /// Observe sources already recorded (startup, resume, user_prompt, …).
    #[serde(default)]
    pub sources_seen: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SessionObserveRequest {
    pub session_id: String,
    pub integration: String,
    pub project: Option<PathBuf>,
    /// Lifecycle source label (e.g. startup, resume, user_prompt, stop).
    pub source: String,
    pub initial_task: Option<String>,
    pub ended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionObserveResult {
    pub session_id: String,
    pub project_root: PathBuf,
    pub project_id: Uuid,
    pub active_run_id: Option<Uuid>,
    pub initial_task: Option<String>,
    pub ended: bool,
    pub created: bool,
}

pub fn sessions_dir(project_root: &Path) -> PathBuf {
    project_root.join(MORAINE_DIR).join(SESSIONS_DIR)
}

pub fn session_path(project_root: &Path, session_id: &str) -> PathBuf {
    sessions_dir(project_root).join(format!("{}.json", session_file_stem(session_id)))
}

fn session_file_stem(session_id: &str) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        return "empty".into();
    }
    let safe: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    if safe.is_empty() {
        let mut h = Sha256::new();
        h.update(trimmed.as_bytes());
        format!("s-{}", &hex::encode(h.finalize())[..16])
    } else if safe.len() < trimmed.len() || safe != trimmed {
        let mut h = Sha256::new();
        h.update(trimmed.as_bytes());
        format!("{safe}-{}", &hex::encode(h.finalize())[..8])
    } else {
        safe
    }
}

fn require_session_id(session_id: &str) -> Result<String> {
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

pub fn load_session(project_root: &Path, session_id: &str) -> Result<Option<SessionRecord>> {
    let path = session_path(project_root, session_id);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    let rec: SessionRecord = serde_json::from_str(&raw)?;
    if rec.schema_version > SESSION_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchemaVersion {
            version: rec.schema_version,
            max: SESSION_SCHEMA_VERSION,
        });
    }
    Ok(Some(rec))
}

pub fn update_session<F, T>(project_root: &Path, session_id: &str, f: F) -> Result<T>
where
    F: FnOnce(&mut SessionRecord) -> Result<T>,
{
    let path = session_path(project_root, session_id);
    let dir = sessions_dir(project_root);
    fs::create_dir_all(&dir)?;
    let _lock = SidecarLock::acquire(&path)?;
    let mut rec = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw)?
    } else {
        return Err(Error::other(format!("session not found: {session_id}")));
    };
    let out = f(&mut rec)?;
    let raw = serde_json::to_string_pretty(&rec)?;
    write_atomic(&path, format!("{raw}\n").as_bytes())?;
    Ok(out)
}

/// Upsert a session envelope. Idempotent for repeated `(session_id, source)`.
pub fn session_observe(req: SessionObserveRequest) -> Result<SessionObserveResult> {
    let session_id = require_session_id(&req.session_id)?;
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
    let initial_task = bound_task(req.initial_task)?;

    let project = resolve_or_init_project(req.project.as_deref())?;
    let project_root = project.project_root.clone();
    ensure_project(&project_root)?;
    fs::create_dir_all(sessions_dir(&project_root))?;

    let path = session_path(&project_root, &session_id);
    let _lock = SidecarLock::acquire(&path)?;

    let mut created = false;
    let mut rec = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str::<SessionRecord>(&raw)?
    } else {
        created = true;
        SessionRecord {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id: session_id.clone(),
            integration: integration.clone(),
            project_root: project_root.display().to_string(),
            started_at: Utc::now(),
            ended_at: None,
            active_run_id: None,
            initial_task: None,
            sources_seen: vec![],
        }
    };

    if !rec.sources_seen.iter().any(|s| s == source) {
        rec.sources_seen.push(source.to_string());
    }
    if rec.integration == "unknown" && integration != "unknown" {
        rec.integration = integration;
    }
    if rec.initial_task.is_none() {
        if let Some(t) = initial_task {
            rec.initial_task = Some(t);
        }
    }
    if req.ended {
        rec.ended_at = Some(Utc::now());
    }

    let raw = serde_json::to_string_pretty(&rec)?;
    write_atomic(&path, format!("{raw}\n").as_bytes())?;

    Ok(SessionObserveResult {
        session_id: rec.session_id,
        project_root,
        project_id: project.project_id,
        active_run_id: rec.active_run_id,
        initial_task: rec.initial_task,
        ended: rec.ended_at.is_some(),
        created,
    })
}

pub fn set_session_active_run(project_root: &Path, session_id: &str, run_id: Uuid) -> Result<()> {
    update_session(project_root, session_id, |rec| {
        rec.active_run_id = Some(run_id);
        Ok(())
    })
}
