use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic::{write_atomic, SidecarLock};
use crate::error::{Error, Result};
use crate::run_meta::{load_run_meta_readonly, moraine_sidecar_path, RunMeta};

pub const PROJECT_SCHEMA_VERSION: u32 = 1;
pub const PROJECT_META_FILE: &str = "project.json";
pub const RUNS_DIR: &str = "runs";
pub const MORAINE_DIR: &str = ".moraine";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub schema_version: u32,
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Idempotent `run start` keys → run id + path fingerprint.
    #[serde(default)]
    pub start_ops: BTreeMap<String, StartOpIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartOpIndex {
    pub run_id: Uuid,
    pub objective: String,
    pub record_path: String,
    pub payload_hash: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInitResult {
    pub project_root: PathBuf,
    pub project_id: Uuid,
    pub created: bool,
    pub moraine_dir: PathBuf,
    pub runs_dir: PathBuf,
}

impl ProjectMeta {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            start_ops: BTreeMap::new(),
        }
    }
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self::new()
    }
}

pub fn project_meta_path(project_root: &Path) -> PathBuf {
    project_root.join(MORAINE_DIR).join(PROJECT_META_FILE)
}

pub fn runs_dir(project_root: &Path) -> PathBuf {
    project_root.join(MORAINE_DIR).join(RUNS_DIR)
}

/// Discover project root containing `.moraine/`, walking parents from `start`.
pub fn discover_project_root(start: &Path) -> Option<PathBuf> {
    let start = if start.exists() {
        fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf())
    } else {
        start.to_path_buf()
    };
    let mut cur = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start
    };
    loop {
        if cur.join(MORAINE_DIR).is_dir() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

fn git_toplevel(cwd: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let p = PathBuf::from(s.trim());
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// Resolve where a project should live when initializing from `path` (or cwd).
pub fn resolve_init_root(path: Option<&Path>) -> Result<PathBuf> {
    let base = match path {
        Some(p) if p.as_os_str().is_empty() => std::env::current_dir()?,
        Some(p) => {
            if p.exists() {
                fs::canonicalize(p)?
            } else {
                // Create directory if needed
                fs::create_dir_all(p)?;
                fs::canonicalize(p)?
            }
        }
        None => std::env::current_dir()?,
    };
    let base = if base.is_file() {
        base.parent()
            .ok_or_else(|| Error::other("cannot init project from root path"))?
            .to_path_buf()
    } else {
        base
    };
    if let Some(git) = git_toplevel(&base) {
        return Ok(git);
    }
    Ok(base)
}

/// Idempotent project initialization.
pub fn init_project(path: Option<&Path>) -> Result<ProjectInitResult> {
    let project_root = resolve_init_root(path)?;
    ensure_project(&project_root)
}

/// Ensure `.moraine` structure exists at `project_root` (must be a directory).
pub fn ensure_project(project_root: &Path) -> Result<ProjectInitResult> {
    let project_root = if project_root.exists() {
        fs::canonicalize(project_root)?
    } else {
        fs::create_dir_all(project_root)?;
        fs::canonicalize(project_root)?
    };
    if !project_root.is_dir() {
        return Err(Error::NotADirectory(project_root));
    }

    let moraine_dir = project_root.join(MORAINE_DIR);
    let runs = runs_dir(&project_root);
    let meta_path = project_meta_path(&project_root);
    let lock_side = meta_path.clone();
    let _lock = SidecarLock::acquire(&lock_side)?;

    let mut created = false;
    if !moraine_dir.exists() {
        fs::create_dir_all(&moraine_dir)?;
        created = true;
    }
    if !runs.exists() {
        fs::create_dir_all(&runs)?;
        created = true;
    }

    // Nested gitignore for transient Moraine files only.
    let gi = moraine_dir.join(".gitignore");
    if !gi.exists() {
        write_atomic(
            &gi,
            b"# Transient Moraine files (do not ignore durable runs)\n\
*.lock\n\
*.tmp\n\
.*.tmp\n\
",
        )?;
        created = true;
    }

    let (meta, meta_created) = if meta_path.exists() {
        let raw = fs::read_to_string(&meta_path)?;
        let mut meta: ProjectMeta = serde_json::from_str(&raw)?;
        if meta.schema_version > PROJECT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchemaVersion {
                version: meta.schema_version,
                max: PROJECT_SCHEMA_VERSION,
            });
        }
        if meta.schema_version < PROJECT_SCHEMA_VERSION {
            meta.schema_version = PROJECT_SCHEMA_VERSION;
            meta.updated_at = Utc::now();
            write_project_meta_unlocked(&meta_path, &meta)?;
        }
        (meta, false)
    } else {
        let meta = ProjectMeta::new();
        write_project_meta_unlocked(&meta_path, &meta)?;
        (meta, true)
    };

    Ok(ProjectInitResult {
        project_root,
        project_id: meta.id,
        created: created || meta_created,
        moraine_dir,
        runs_dir: runs,
    })
}

fn write_project_meta_unlocked(path: &Path, meta: &ProjectMeta) -> Result<()> {
    let raw = serde_json::to_string_pretty(meta)?;
    write_atomic(path, format!("{raw}\n").as_bytes())
}

#[allow(dead_code)]
pub fn load_project_meta(project_root: &Path) -> Result<ProjectMeta> {
    let path = project_meta_path(project_root);
    if !path.exists() {
        return Err(Error::ProjectNotFound {
            path: project_root.to_path_buf(),
        });
    }
    let raw = fs::read_to_string(&path)?;
    let meta: ProjectMeta = serde_json::from_str(&raw)?;
    if meta.schema_version > PROJECT_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchemaVersion {
            version: meta.schema_version,
            max: PROJECT_SCHEMA_VERSION,
        });
    }
    Ok(meta)
}

pub fn update_project_meta<F, T>(project_root: &Path, f: F) -> Result<T>
where
    F: FnOnce(&mut ProjectMeta) -> Result<T>,
{
    let path = project_meta_path(project_root);
    let _lock = SidecarLock::acquire(&path)?;
    let mut meta = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw)?
    } else {
        return Err(Error::ProjectNotFound {
            path: project_root.to_path_buf(),
        });
    };
    let out = f(&mut meta)?;
    meta.updated_at = Utc::now();
    write_project_meta_unlocked(&path, &meta)?;
    Ok(out)
}

/// Locate a run record by UUID by scanning `.moraine/runs/*.md.moraine.json`.
pub fn find_run_by_id(project_root: &Path, run_id: Uuid) -> Result<(PathBuf, RunMeta)> {
    let runs = runs_dir(project_root);
    if !runs.is_dir() {
        return Err(Error::RunNotFound { id: run_id });
    }
    for entry in fs::read_dir(&runs)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.ends_with(".md.moraine.json") {
            continue;
        }
        // corresponding md path
        let md_name = name.trim_end_matches(".moraine.json");
        let md_path = runs.join(md_name);
        if !md_path.is_file() {
            continue;
        }
        match load_run_meta_readonly(&md_path)? {
            Some(meta) if meta.run.id == run_id => return Ok((md_path, meta)),
            _ => continue,
        }
    }
    // Also accept direct sidecar named incorrectly? skip.
    let _ = moraine_sidecar_path;
    Err(Error::RunNotFound { id: run_id })
}

/// Resolve project root from optional path; auto-init minimal structure when missing.
pub fn resolve_or_init_project(path: Option<&Path>) -> Result<ProjectInitResult> {
    let hint = match path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    if let Some(root) = discover_project_root(&hint) {
        return ensure_project(&root);
    }
    // Prefer git root when present.
    let init_root = resolve_init_root(path)?;
    ensure_project(&init_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_non_git_and_repeat() {
        let dir = tempdir().unwrap();
        let r1 = init_project(Some(dir.path())).unwrap();
        assert!(r1.created);
        assert!(r1.runs_dir.is_dir());
        assert!(project_meta_path(&r1.project_root).is_file());
        let gi = r1.moraine_dir.join(".gitignore");
        assert!(gi.is_file());
        let root_gi = dir.path().join(".gitignore");
        assert!(!root_gi.exists());
        let r2 = init_project(Some(dir.path())).unwrap();
        assert!(!r2.created);
        assert_eq!(r1.project_id, r2.project_id);
    }

    #[test]
    fn preserves_existing_runs() {
        let dir = tempdir().unwrap();
        let r = init_project(Some(dir.path())).unwrap();
        let marker = r.runs_dir.join("keep.md");
        fs::write(&marker, "x\n").unwrap();
        let _ = init_project(Some(dir.path())).unwrap();
        assert_eq!(fs::read_to_string(&marker).unwrap(), "x\n");
    }
}
