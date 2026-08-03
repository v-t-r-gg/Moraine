#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use axum::{
    extract::{Path as AxumPath, State},
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{net::TcpListener, sync::Notify};
use tracing::{error, info};

const MAX_SPOOL_FILES: usize = moraine_service::MAX_PENDING_EVENTS;

#[derive(Clone)]
struct AppState {
    spool_dir: PathBuf,
    capture_endpoint: moraine_platform::CaptureEndpoint,
    capture_ready: Arc<AtomicBool>,
    http_addr: String,
    started_at_unix: u64,
}

#[derive(Parser)]
#[command(author, version, about = "Moraine local integration runtime")]
struct Args {
    /// Loopback HTTP listen address for diagnostics only (e.g. 127.0.0.1:33111).
    /// Must not bind to non-loopback interfaces. Hook delivery uses local capture IPC.
    #[arg(long)]
    http: Option<String>,

    /// Unix domain socket for hook / adapter event delivery (primary capture transport).
    #[arg(long)]
    unix_socket: Option<PathBuf>,

    /// Windows named pipe for hook / adapter event delivery.
    #[arg(long)]
    named_pipe: Option<String>,

    /// Spool directory for undelivered events
    #[arg(long)]
    spool_dir: Option<PathBuf>,

    /// Windows application log directory.
    #[arg(long)]
    log_dir: Option<PathBuf>,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Args {
        http,
        unix_socket,
        named_pipe,
        spool_dir,
        log_dir,
    } = Args::parse();
    let runtime_layout = moraine_platform::RuntimeLayout::try_discover()?;
    #[cfg(target_os = "windows")]
    {
        let log_dir = log_dir.unwrap_or_else(|| runtime_layout.log_dir.clone());
        moraine_service::logging::init_windows_file_logging(&log_dir)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = log_dir;
        tracing_subscriber::fmt::init();
    }
    let capture_endpoint = resolve_capture_endpoint(
        unix_socket.as_deref(),
        named_pipe.as_deref(),
        &runtime_layout,
        moraine_platform::HostPlatform::current(),
    )?;
    let spool_dir = spool_dir.unwrap_or_else(|| runtime_layout.spool_dir.clone());
    std::fs::create_dir_all(&spool_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&spool_dir, std::fs::Permissions::from_mode(0o700));
    }
    tokio::fs::create_dir_all(spool_dir.join("processed"))
        .await
        .ok();
    tokio::fs::create_dir_all(spool_dir.join("failed"))
        .await
        .ok();

    let http = http.unwrap_or_else(|| runtime_layout.diagnostics_endpoint.to_string());
    // Diagnostics HTTP on loopback only — not the hook transport.
    let http_addr: SocketAddr = http.parse()?;
    if !http_addr.ip().is_loopback() {
        anyhow::bail!(
            "refusing non-loopback HTTP bind {http_addr}; diagnostics must use 127.0.0.1/::1. \
             Hook delivery uses local capture IPC, not TCP."
        );
    }

    // Capture is the product intake. Bind it before diagnostics can report online.
    let capture_listener = moraine_service::capture::bind(&capture_endpoint).await?;
    let started_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let shutdown = Arc::new(Notify::new());
    let capture_ready = Arc::new(AtomicBool::new(true));
    let state = AppState {
        spool_dir: spool_dir.clone(),
        capture_endpoint: capture_endpoint.clone(),
        capture_ready: capture_ready.clone(),
        http_addr: http,
        started_at_unix,
    };
    let app = Router::new()
        .route("/health", get(|| async { Json(Health { status: "ok" }) }))
        .route("/status", get(handle_status))
        .route("/projects", get(handle_projects))
        .route("/projects/{project_id}/runs", get(handle_project_runs))
        .route("/runs/{run_id}", get(handle_run_detail))
        .route("/index/rebuild", post(handle_rebuild))
        .route("/projects/{project_id}/rescan", post(handle_rescan_project))
        .with_state(state);
    let listener = TcpListener::bind(http_addr).await?;

    let capture_kind = match &capture_endpoint {
        moraine_platform::CaptureEndpoint::UnixSocket(_) => "unix-socket",
        moraine_platform::CaptureEndpoint::WindowsNamedPipe(_) => "windows-named-pipe",
        moraine_platform::CaptureEndpoint::Unsupported => "unsupported",
    };
    info!(
        %http_addr,
        spool_dir = %spool_dir.display(),
        hooks = capture_kind,
        "starting moraine-service (diagnostics=loopback-http)"
    );

    // Spool processing task: periodically scan spool dir and process events
    {
        let spool = spool_dir.clone();
        let shutdown_clone = shutdown.clone();
        tokio::spawn(async move {
            if let Err(e) = spool_processor_loop(spool, shutdown_clone).await {
                error!(error = %e, "spool processor failed");
            }
        });
    }

    // Index rebuild task: periodically scan for projects and write index.json
    {
        let spool = spool_dir.clone();
        let shutdown_clone = shutdown.clone();
        tokio::spawn(async move {
            let out = spool.join("index.json");
            loop {
                if let Err(e) = moraine_service::rebuild_registered_index(out.clone()).await {
                    error!(error=%e, "index rebuild failed");
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {},
                    _ = shutdown_clone.notified() => break,
                }
            }
        });
    }

    // Wait for ctrl-c and then notify shutdown
    let notify = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        notify.notify_waiters();
    });

    let http_shutdown = shutdown.clone();
    let http = async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                http_shutdown.notified().await;
            })
            .await
    };
    let capture_shutdown = shutdown.clone();
    let capture = capture_listener.run(spool_dir, capture_shutdown);
    tokio::pin!(http);
    tokio::pin!(capture);

    tokio::select! {
        result = &mut capture => {
            capture_ready.store(false, Ordering::Release);
            shutdown.notify_waiters();
            match result {
                Ok(()) => {
                    http.await?;
                    Ok(())
                }
                Err(error) => {
                    error!(%error, "capture listener failed; stopping runtime");
                    Err(error)
                }
            }
        }
        result = &mut http => {
            shutdown.notify_waiters();
            result?;
            capture.await?;
            Ok(())
        }
    }
}

fn resolve_capture_endpoint(
    unix_socket: Option<&std::path::Path>,
    named_pipe: Option<&str>,
    layout: &moraine_platform::RuntimeLayout,
    host: moraine_platform::HostPlatform,
) -> Result<moraine_platform::CaptureEndpoint> {
    if unix_socket.is_some() && named_pipe.is_some() {
        anyhow::bail!("--unix-socket and --named-pipe are mutually exclusive");
    }
    if let Some(path) = unix_socket {
        if host != moraine_platform::HostPlatform::Linux {
            anyhow::bail!("--unix-socket is supported only on Linux");
        }
        return Ok(moraine_platform::CaptureEndpoint::UnixSocket(
            path.to_path_buf(),
        ));
    }
    if let Some(name) = named_pipe {
        if host != moraine_platform::HostPlatform::Windows {
            anyhow::bail!("--named-pipe is supported only on Windows");
        }
        return Ok(moraine_platform::CaptureEndpoint::WindowsNamedPipe(
            name.to_owned(),
        ));
    }
    match &layout.capture_endpoint {
        moraine_platform::CaptureEndpoint::UnixSocket(path) => {
            Ok(moraine_platform::CaptureEndpoint::UnixSocket(path.clone()))
        }
        moraine_platform::CaptureEndpoint::WindowsNamedPipe(name) => Ok(
            moraine_platform::CaptureEndpoint::WindowsNamedPipe(name.clone()),
        ),
        endpoint => anyhow::bail!("unsupported capture endpoint for moraine-service: {endpoint:?}"),
    }
}

async fn handle_status(State(state): State<AppState>) -> Json<Value> {
    let (pending, processed, failed) = moraine_service::spool_counts(&state.spool_dir)
        .await
        .unwrap_or((0, 0, 0));
    let index_path = state.spool_dir.join("index.json");
    let index_mtime = std::fs::metadata(&index_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        });
    let revision = moraine_service::index_revision(&state.spool_dir);
    let build = moraine_core::BuildIdentity::current();
    let executable = std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string());
    let scope_id = match &state.capture_endpoint {
        moraine_platform::CaptureEndpoint::WindowsNamedPipe(name) => name
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(name)
            .strip_prefix("moraine.capture.v1.")
            .filter(|scope| !scope.is_empty())
            .map(str::to_owned),
        _ => None,
    };
    Json(json!({
        "status": "ok",
        "online": true,
        "product": moraine_core::SERVICE_PRODUCT_ID,
        "protocolVersion": build.service_protocol_version,
        "version": build.version,
        "productVersion": build.version,
        "gitCommit": build.git_commit,
        "serviceProtocolVersion": build.service_protocol_version,
        "schema": build.schema,
        "executablePath": executable,
        "scopeId": scope_id,
        "captureReady": state.capture_ready.load(Ordering::Acquire),
        "captureEndpoint": state.capture_endpoint,
        "socketPath": match &state.capture_endpoint {
            moraine_platform::CaptureEndpoint::UnixSocket(path) => {
                Some(path.display().to_string())
            }
            _ => None,
        },
        "httpAddr": state.http_addr,
        "spoolDir": state.spool_dir.display().to_string(),
        "indexPath": index_path.display().to_string(),
        "spool": {
            "pending": pending,
            "processed": processed,
            "failed": failed,
        },
        "indexMtimeUnix": index_mtime,
        "revision": revision,
        "indexRevision": revision,
        "startedAtUnix": state.started_at_unix,
    }))
}

async fn handle_projects(State(state): State<AppState>) -> Json<Value> {
    if let Some(doc) = moraine_service::read_index_projects(&state.spool_dir) {
        return Json(doc);
    }
    let out = state.spool_dir.join("index.json");
    if moraine_service::rebuild_registered_index(out).await.is_ok() {
        if let Some(doc) = moraine_service::read_index_projects(&state.spool_dir) {
            return Json(doc);
        }
    }
    Json(json!({
        "projects": [],
        "revision": 0,
        "fallback": true,
        "warning": "project registry unavailable",
    }))
}

async fn handle_project_runs(
    State(state): State<AppState>,
    AxumPath(project_id): AxumPath<String>,
) -> Json<Value> {
    let root =
        moraine_service::find_project_root_in_index(&state.spool_dir, &project_id).or_else(|| {
            moraine_core::registered_project_roots()
                .ok()?
                .into_iter()
                .find(|p| {
                    moraine_core::resolve_existing_project(Some(p))
                        .map(|r| r.project_id.to_string() == project_id)
                        .unwrap_or(false)
                })
        });
    let Some(root) = root else {
        return Json(json!({
            "error": { "code": "project_not_found", "projectId": project_id },
            "runs": []
        }));
    };
    match moraine_service::list_project_runs(&root) {
        Ok(runs) => Json(json!({
            "projectId": project_id,
            "rootPath": root.display().to_string(),
            "runs": runs,
            "revision": moraine_service::index_revision(&state.spool_dir),
        })),
        Err(e) => Json(json!({
            "error": { "code": "list_failed", "message": e.to_string() },
            "projectId": project_id,
            "runs": []
        })),
    }
}

async fn handle_run_detail(
    State(state): State<AppState>,
    AxumPath(run_id): AxumPath<String>,
) -> Json<Value> {
    let Ok(uid) = uuid::Uuid::parse_str(&run_id) else {
        return Json(json!({ "error": { "code": "invalid_run_id", "runId": run_id } }));
    };
    // Search indexed projects for the run (read-only).
    if let Some(doc) = moraine_service::read_index_projects(&state.spool_dir) {
        if let Some(projects) = doc.get("projects").and_then(|p| p.as_array()) {
            for p in projects {
                let root = p
                    .get("rootPath")
                    .or_else(|| p.get("root"))
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from);
                let Some(root) = root else { continue };
                if let Ok((md, _meta)) = moraine_core::find_run_by_id(&root, uid) {
                    let pid = moraine_core::resolve_existing_project(Some(&root))
                        .map(|r| r.project_id)
                        .unwrap_or(uuid::Uuid::nil());
                    let detail = moraine_core::load_run_detail(&md, pid);
                    return Json(json!({
                        "run": detail,
                        "projectRoot": root.display().to_string()
                    }));
                }
            }
        }
    }
    Json(json!({ "error": { "code": "run_not_found", "runId": run_id } }))
}

async fn handle_rebuild(State(state): State<AppState>) -> Json<Value> {
    let out = state.spool_dir.join("index.json");
    let before = moraine_service::index_revision(&state.spool_dir);
    match moraine_service::rebuild_registered_index(out).await {
        Ok(()) => {
            let after = moraine_service::index_revision(&state.spool_dir);
            let doc = moraine_service::read_index_projects(&state.spool_dir);
            Json(json!({
                "ok": true,
                "revisionBefore": before,
                "revision": after,
                "projectCount": doc.as_ref()
                    .and_then(|d| d.get("projects"))
                    .and_then(|p| p.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0),
            }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn handle_rescan_project(
    State(state): State<AppState>,
    AxumPath(project_id): AxumPath<String>,
) -> Json<Value> {
    // Rescan is a full index rebuild that re-reads project roots (index-only mutation).
    let _ = project_id;
    handle_rebuild(State(state)).await
}

fn is_spool_event_file(path: &std::path::Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("event-") && n.ends_with(".json"))
            .unwrap_or(false)
}

async fn spool_processor_loop(spool_dir: PathBuf, shutdown: Arc<Notify>) -> Result<()> {
    let processed_dir = spool_dir.join("processed");
    let failed_dir = spool_dir.join("failed");
    tokio::fs::create_dir_all(&processed_dir).await.ok();
    tokio::fs::create_dir_all(&failed_dir).await.ok();

    loop {
        tokio::select! {
            _ = shutdown.notified() => {
                info!(spool=%spool_dir.display(), "shutting down spool processor");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                if let Ok(mut entries) = tokio::fs::read_dir(&spool_dir).await {
                    let mut files = Vec::new();
                    while let Ok(Some(ent)) = entries.next_entry().await {
                        let p = ent.path();
                        if is_spool_event_file(&p) {
                            if let Ok(md) = tokio::fs::metadata(&p).await {
                                if let Ok(t) = md.modified() {
                                    files.push((t, p));
                                }
                            }
                        }
                    }
                    // Hook delivery is sequential, but read_dir order is not. Preserve
                    // arrival order so the first prompt remains the session objective.
                    files.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                    if files.len() > MAX_SPOOL_FILES {
                        let remove_count = files.len() - MAX_SPOOL_FILES;
                        for (_t, p) in files.drain(..remove_count) {
                            let dest = failed_dir.join(p.file_name().unwrap());
                            let _ = tokio::fs::rename(&p, &dest).await;
                            info!(file=%p.display(), "moved old spool file to failed due to size limits");
                        }
                    }

                    for (_modified, path) in files {
                        if let Err(e) = moraine_service::process_spool_file(&path, &processed_dir, &failed_dir).await {
                            error!(file=%path.display(), error=%e, "processing spool file failed");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(endpoint: moraine_platform::CaptureEndpoint) -> moraine_platform::RuntimeLayout {
        moraine_platform::RuntimeLayout {
            spool_dir: "/tmp/spool".into(),
            log_dir: "/tmp/logs".into(),
            project_registry: "/tmp/projects.json".into(),
            transaction_journals: "/tmp/journals".into(),
            diagnostics_endpoint: "127.0.0.1:33111".parse().unwrap(),
            capture_endpoint: endpoint,
        }
    }

    #[test]
    fn explicit_capture_socket_wins() {
        let runtime = layout(moraine_platform::CaptureEndpoint::UnixSocket(
            "/runtime/default.sock".into(),
        ));
        assert_eq!(
            resolve_capture_endpoint(
                Some(std::path::Path::new("/explicit.sock")),
                None,
                &runtime,
                moraine_platform::HostPlatform::Linux,
            )
            .unwrap(),
            moraine_platform::CaptureEndpoint::UnixSocket(PathBuf::from("/explicit.sock"))
        );
    }

    #[test]
    fn layout_capture_socket_is_used_for_status_and_listener() {
        let runtime = layout(moraine_platform::CaptureEndpoint::UnixSocket(
            "/runtime/shared.sock".into(),
        ));
        assert_eq!(
            resolve_capture_endpoint(None, None, &runtime, moraine_platform::HostPlatform::Linux,)
                .unwrap(),
            moraine_platform::CaptureEndpoint::UnixSocket(PathBuf::from("/runtime/shared.sock"))
        );
    }

    #[test]
    fn unsupported_capture_endpoint_fails_explicitly() {
        let runtime = layout(moraine_platform::CaptureEndpoint::Unsupported);
        assert!(resolve_capture_endpoint(
            None,
            None,
            &runtime,
            moraine_platform::HostPlatform::Other,
        )
        .unwrap_err()
        .to_string()
        .contains("unsupported capture endpoint"));
    }

    #[test]
    fn duplicate_or_cross_platform_capture_flags_fail() {
        let runtime = layout(moraine_platform::CaptureEndpoint::Unsupported);
        assert!(resolve_capture_endpoint(
            Some(std::path::Path::new("/capture.sock")),
            Some(r"\\.\pipe\capture"),
            &runtime,
            moraine_platform::HostPlatform::Windows,
        )
        .unwrap_err()
        .to_string()
        .contains("mutually exclusive"));
        assert!(resolve_capture_endpoint(
            None,
            Some(r"\\.\pipe\capture"),
            &runtime,
            moraine_platform::HostPlatform::Linux,
        )
        .unwrap_err()
        .to_string()
        .contains("only on Windows"));
    }
}
