//! Local project/run discovery commands (M5). Read-only over run bundles.

use std::path::PathBuf;

use moraine_core::{
    list_run_summaries, load_run_detail, resolve_existing_project, scan_project_roots,
    summarize_project, ProjectSummary, RunDetail, RunSummary,
};
use serde::Serialize;
use uuid::Uuid;

fn map_err(e: moraine_core::Error) -> String {
    e.to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryStatusDto {
    pub online: bool,
    pub revision: u64,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Probe local service diagnostics HTTP; fall back to direct filesystem discovery.
#[tauri::command]
pub fn discovery_status() -> Result<DiscoveryStatusDto, String> {
    // Best-effort loopback probe (service optional).
    let client_ok = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:33111".parse().unwrap(),
        std::time::Duration::from_millis(80),
    )
    .is_ok();
    if client_ok {
        if let Ok(body) = ureq_get("http://127.0.0.1:33111/status") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                let revision = v
                    .get("revision")
                    .or_else(|| v.get("indexRevision"))
                    .and_then(|r| r.as_u64())
                    .unwrap_or(0);
                return Ok(DiscoveryStatusDto {
                    online: true,
                    revision,
                    mode: "service".into(),
                    message: None,
                });
            }
        }
    }
    Ok(DiscoveryStatusDto {
        online: false,
        revision: 0,
        mode: "direct".into(),
        message: Some("local service unavailable; using direct project inspection".into()),
    })
}

fn ureq_get(url: &str) -> Result<String, String> {
    // Diagnostics-only probe via curl (loopback service). Direct FS scan is the fallback.
    let out = std::process::Command::new("curl")
        .args(["-fsS", "--max-time", "1", url])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("curl failed".into());
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn discovery_projects(scan_root: Option<String>) -> Result<Vec<ProjectSummary>, String> {
    let base = scan_root
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    // Prefer service index when available.
    if let Ok(body) = ureq_get("http://127.0.0.1:33111/projects") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(arr) = v.get("projects").and_then(|p| p.as_array()) {
                let mut out = Vec::new();
                for p in arr {
                    let root = p
                        .get("rootPath")
                        .or_else(|| p.get("root"))
                        .and_then(|x| x.as_str());
                    if let Some(root) = root {
                        if let Ok(s) = summarize_project(Path::new(root)) {
                            out.push(s);
                        }
                    }
                }
                if !out.is_empty() {
                    return Ok(out);
                }
            }
        }
    }
    let roots = scan_project_roots(&base, 5);
    let mut out = Vec::new();
    for r in roots {
        match summarize_project(&r) {
            Ok(s) => out.push(s),
            Err(e) => {
                // Represent broken project entry
                out.push(ProjectSummary {
                    project_id: Uuid::nil(),
                    name: r
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .into(),
                    root_path: r.display().to_string(),
                    available: false,
                    run_counts: moraine_core::ProjectRunCounts {
                        active: 0,
                        ready: 0,
                        recent: 0,
                    },
                    open_finding_count: 0,
                    last_activity_at: None,
                    warning: Some(e.to_string()),
                });
            }
        }
    }
    Ok(out)
}

use std::path::Path;

#[tauri::command]
#[allow(clippy::too_many_arguments)] // flat Tauri invoke args for typed TS wrappers
pub fn discovery_runs(
    project_id: String,
    root_path: Option<String>,
    category: Option<String>,
    open_findings_only: Option<bool>,
    has_risks: Option<bool>,
    has_questions: Option<bool>,
    query: Option<String>,
    capture_coverage: Option<String>,
) -> Result<Vec<RunSummary>, String> {
    let root = if let Some(p) = root_path {
        PathBuf::from(p)
    } else {
        // Find by scanning
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        scan_project_roots(&base, 6)
            .into_iter()
            .find(|r| {
                resolve_existing_project(Some(r))
                    .map(|x| x.project_id.to_string() == project_id)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("project_not_found: {project_id}"))?
    };
    let resolved = resolve_existing_project(Some(&root)).map_err(map_err)?;
    let runs = list_run_summaries(&resolved.project_root, resolved.project_id);
    let filtered = moraine_core::filter_runs_ext(
        &runs,
        category.as_deref(),
        open_findings_only.unwrap_or(false),
        has_risks.unwrap_or(false),
        has_questions.unwrap_or(false),
        query.as_deref(),
        capture_coverage.as_deref(),
    );
    Ok(filtered.into_iter().cloned().collect())
}

#[tauri::command]
pub fn discovery_run_detail(
    path: Option<String>,
    run_id: Option<String>,
    project_root: Option<String>,
) -> Result<RunDetail, String> {
    if let Some(p) = path {
        let pb = PathBuf::from(p);
        let pid = resolve_existing_project(Some(
            pb.parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .unwrap_or(Path::new(".")),
        ))
        .map(|r| r.project_id)
        .unwrap_or(Uuid::nil());
        return Ok(load_run_detail(&pb, pid));
    }
    let rid = run_id
        .as_deref()
        .ok_or_else(|| "runId or path required".to_string())?;
    let uid = Uuid::parse_str(rid).map_err(|_| "invalid runId".to_string())?;
    let root = project_root
        .map(PathBuf::from)
        .ok_or_else(|| "projectRoot required when using runId".to_string())?;
    let (md, _) = moraine_core::find_run_by_id(&root, uid).map_err(map_err)?;
    let pid = resolve_existing_project(Some(&root))
        .map(|r| r.project_id)
        .unwrap_or(Uuid::nil());
    Ok(load_run_detail(&md, pid))
}

#[tauri::command]
pub fn discovery_rebuild_index(scan_root: Option<String>) -> Result<serde_json::Value, String> {
    let base = scan_root
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    // Prefer service rebuild
    if ureq_post("http://127.0.0.1:33111/index/rebuild").is_ok() {
        if let Ok(body) = ureq_get("http://127.0.0.1:33111/status") {
            return Ok(serde_json::from_str(&body).unwrap_or(serde_json::json!({"ok": true})));
        }
        return Ok(serde_json::json!({ "ok": true, "mode": "service" }));
    }
    // Direct scan only (no durable secondary index from desktop).
    let roots = scan_project_roots(&base, 6);
    Ok(serde_json::json!({
        "ok": true,
        "mode": "direct",
        "projectCount": roots.len(),
    }))
}

fn ureq_post(url: &str) -> Result<String, String> {
    let out = std::process::Command::new("curl")
        .args(["-fsS", "-X", "POST", "--max-time", "5", url])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("post failed".into());
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn discovery_rescan_project(project_id: String) -> Result<serde_json::Value, String> {
    let url = format!("http://127.0.0.1:33111/projects/{project_id}/rescan");
    if let Ok(body) = ureq_post(&url) {
        return Ok(serde_json::from_str(&body).unwrap_or(serde_json::json!({"ok": true})));
    }
    discovery_rebuild_index(None)
}

#[tauri::command]
pub fn discovery_add_existing_project(path: String) -> Result<ProjectSummary, String> {
    let p = PathBuf::from(path);
    if !p.join(".moraine").is_dir() {
        return Err("not an initialized Moraine project (missing .moraine/)".into());
    }
    summarize_project(&p).map_err(map_err)
}
