//! Project-confined open/reveal for run Markdown records.

use std::path::{Path, PathBuf};

use moraine_core::resolve_existing_project;

fn map_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Resolve a run Markdown path only when it is inside the project root and
/// matches the declared relative record path (and optional run id binding).
fn resolve_confined_record(
    project_root: &str,
    record_path: &str,
    absolute_path: &str,
    run_id: Option<&str>,
) -> Result<PathBuf, String> {
    let resolved = resolve_existing_project(Some(Path::new(project_root))).map_err(map_err)?;
    let root = resolved.project_root;
    let root_canon = std::fs::canonicalize(&root).unwrap_or(root.clone());

    let rel = record_path.trim().trim_start_matches('/');
    if rel.is_empty() || rel.contains("..") {
        return Err("invalid record path".into());
    }
    let from_rel = root_canon.join(rel);
    let abs = PathBuf::from(absolute_path);
    let abs_canon = std::fs::canonicalize(&abs)
        .map_err(|_| "record path does not exist or is not accessible".to_string())?;

    if !abs_canon.starts_with(&root_canon) {
        return Err("path escapes project root".into());
    }
    let from_rel_canon = std::fs::canonicalize(&from_rel).unwrap_or(from_rel);
    if abs_canon != from_rel_canon {
        return Err("absolute path disagrees with project-relative record path".into());
    }
    if !abs_canon.is_file() {
        return Err("record is not a file".into());
    }

    if let Some(rid) = run_id.map(str::trim).filter(|s| !s.is_empty()) {
        let uid = uuid::Uuid::parse_str(rid).map_err(|_| "invalid runId".to_string())?;
        let (md, _) = moraine_core::find_run_by_id(&root_canon, uid).map_err(map_err)?;
        let md_canon = std::fs::canonicalize(&md).unwrap_or(md);
        if md_canon != abs_canon {
            return Err("run id does not match record path".into());
        }
    }

    Ok(abs_canon)
}

fn host_open(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("open failed: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("open failed: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", path.to_str().unwrap_or("")])
            .spawn()
            .map_err(|e| format!("open failed: {e}"))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        Err("open not supported on this host".into())
    }
}

fn host_reveal(path: &Path) -> Result<(), String> {
    let parent = path.parent().unwrap_or(path);
    #[cfg(target_os = "linux")]
    {
        // Reveal by opening the containing directory (file managers vary).
        host_open(parent)
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R"])
            .arg(path)
            .spawn()
            .map_err(|e| format!("reveal failed: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|e| format!("reveal failed: {e}"))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = parent;
        Err("reveal not supported on this host".into())
    }
}

#[tauri::command]
pub fn reveal_run_record(
    project_root: String,
    record_path: String,
    absolute_path: String,
    run_id: Option<String>,
) -> Result<(), String> {
    let path = resolve_confined_record(
        &project_root,
        &record_path,
        &absolute_path,
        run_id.as_deref(),
    )?;
    host_reveal(&path)
}

#[tauri::command]
pub fn open_run_markdown(
    project_root: String,
    record_path: String,
    absolute_path: String,
    run_id: Option<String>,
) -> Result<(), String> {
    let path = resolve_confined_record(
        &project_root,
        &record_path,
        &absolute_path,
        run_id.as_deref(),
    )?;
    host_open(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use moraine_core::{init_project, run_start, RunStartRequest};
    use tempfile::tempdir;

    #[test]
    fn rejects_path_escape() {
        let dir = tempdir().unwrap();
        let project = init_project(Some(dir.path())).unwrap();
        let err = resolve_confined_record(
            project.project_root.to_str().unwrap(),
            "../etc/passwd",
            "/etc/passwd",
            None,
        )
        .unwrap_err();
        assert!(err.contains("invalid") || err.contains("escape") || err.contains("disagree"));
    }

    #[test]
    fn accepts_matching_run_path() {
        let dir = tempdir().unwrap();
        let project = init_project(Some(dir.path())).unwrap();
        let started = run_start(RunStartRequest {
            objective: "file action test".into(),
            idempotency_key: "file-1".into(),
            project: Some(project.project_root.clone()),
            session_id: None,
        })
        .unwrap();
        let abs = started.absolute_path;
        let rel = started.record_path;
        let got = resolve_confined_record(
            project.project_root.to_str().unwrap(),
            &rel,
            abs.to_str().unwrap(),
            Some(&started.run_id.to_string()),
        )
        .unwrap();
        assert_eq!(
            std::fs::canonicalize(&got).unwrap(),
            std::fs::canonicalize(&abs).unwrap()
        );
    }

    #[test]
    fn run_id_mismatch_fails_closed() {
        let dir = tempdir().unwrap();
        let project = init_project(Some(dir.path())).unwrap();
        let started = run_start(RunStartRequest {
            objective: "mismatch".into(),
            idempotency_key: "file-2".into(),
            project: Some(project.project_root.clone()),
            session_id: None,
        })
        .unwrap();
        let err = resolve_confined_record(
            project.project_root.to_str().unwrap(),
            &started.record_path,
            started.absolute_path.to_str().unwrap(),
            Some("00000000-0000-4000-8000-000000000099"),
        )
        .unwrap_err();
        assert!(!err.is_empty());
    }
}
