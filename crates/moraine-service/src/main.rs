mod capture;

use anyhow::Result;
use axum::{
    extract::{Path as AxumPath, State},
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::Notify};
use tracing::{error, info};

const MAX_SPOOL_FILES: usize = moraine_service::MAX_PENDING_EVENTS;

#[derive(Clone)]
struct AppState {
    spool_dir: PathBuf,
    capture_endpoint: moraine_platform::CaptureEndpoint,
    http_addr: String,
    started_at_unix: u64,
}

#[derive(Parser)]
#[command(author, version, about = "Moraine local integration runtime")]
struct Args {
    #[command(subcommand)]
    command: Option<ServiceCmd>,

    /// Loopback HTTP listen address for diagnostics only (e.g. 127.0.0.1:33111).
    /// Must not bind to non-loopback interfaces. Hook delivery uses the Unix socket.
    #[arg(long, default_value = "127.0.0.1:33111")]
    http: String,

    /// Unix domain socket for hook / adapter event delivery (primary capture transport).
    #[arg(long)]
    unix_socket: Option<PathBuf>,

    /// Spool directory for undelivered events
    #[arg(long)]
    spool_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum ServiceCmd {
    /// Install a systemd --user unit (Linux)
    Install,
    /// Start the service via systemd --user (Linux)
    Start,
    /// Stop the service via systemd --user (Linux)
    Stop,
    /// Show service status via systemd --user (Linux)
    Status,
    /// Print the unit file to stdout
    UnitFile,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    if let Some(cmd) = args.command.as_ref() {
        // Handle cli-only commands and exit
        match cmd {
            ServiceCmd::UnitFile => {
                println!("{}", systemd_unit());
                return Ok(());
            }
            ServiceCmd::Install => {
                if cfg!(target_os = "linux") {
                    let home_unit = dirs::config_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
                        .join("systemd/user/moraine-service.service");
                    if let Some(parent) = home_unit.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    std::fs::write(&home_unit, systemd_unit())?;
                    let _ = std::process::Command::new("systemctl")
                        .args(["--user", "daemon-reload"])
                        .status();
                    println!("wrote unit to {}", home_unit.display());
                    return Ok(());
                } else {
                    println!("install is only supported on Linux/systemd");
                    return Ok(());
                }
            }
            ServiceCmd::Start => {
                if cfg!(target_os = "linux") {
                    let s = std::process::Command::new("systemctl")
                        .args(["--user", "start", "moraine-service.service"])
                        .status()?;
                    println!("systemctl start returned: {}", s);
                    return Ok(());
                } else {
                    println!("start is only supported on Linux/systemd");
                    return Ok(());
                }
            }
            ServiceCmd::Stop => {
                if cfg!(target_os = "linux") {
                    let s = std::process::Command::new("systemctl")
                        .args(["--user", "stop", "moraine-service.service"])
                        .status()?;
                    println!("systemctl stop returned: {}", s);
                    return Ok(());
                } else {
                    println!("stop is only supported on Linux/systemd");
                    return Ok(());
                }
            }
            ServiceCmd::Status => {
                if cfg!(target_os = "linux") {
                    let s = std::process::Command::new("systemctl")
                        .args(["--user", "status", "moraine-service.service"])
                        .status()?;
                    println!("systemctl status returned: {}", s);
                    return Ok(());
                } else {
                    println!("status is only supported on Linux/systemd");
                    return Ok(());
                }
            }
        }
    }

    let spool_dir = args
        .spool_dir
        .unwrap_or_else(|| moraine_platform::RuntimeLayout::discover().spool_dir);
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

    // Diagnostics HTTP on loopback only — not the hook transport.
    let http_addr: SocketAddr = args.http.parse()?;
    if !http_addr.ip().is_loopback() {
        anyhow::bail!(
            "refusing non-loopback HTTP bind {http_addr}; diagnostics must use 127.0.0.1/::1. \
             Hook delivery uses the Unix domain socket, not TCP."
        );
    }

    let runtime_layout = moraine_platform::RuntimeLayout::discover();
    let capture_endpoint = resolve_capture_endpoint(args.unix_socket.as_deref(), &runtime_layout)?;
    // Capture is the product intake. Bind it before diagnostics can report online.
    let capture_listener = capture::bind(&capture_endpoint).await?;
    let started_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let shutdown = Arc::new(Notify::new());
    let state = AppState {
        spool_dir: spool_dir.clone(),
        capture_endpoint: capture_endpoint.clone(),
        http_addr: args.http.clone(),
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

    info!(
        %http_addr,
        spool_dir = %spool_dir.display(),
        "starting moraine-service (hooks=unix-socket, diagnostics=loopback-http)"
    );

    // Unix domain socket: primary hook/adapter intake (not TCP).
    {
        let spool = spool_dir.clone();
        let shutdown_clone = shutdown.clone();
        tokio::spawn(async move {
            if let Err(e) = capture_listener.run(spool, shutdown_clone).await {
                error!(error = %e, "capture listener failed");
            }
        });
    }

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

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.notified().await;
        })
        .await?;

    Ok(())
}

fn resolve_capture_endpoint(
    explicit: Option<&std::path::Path>,
    layout: &moraine_platform::RuntimeLayout,
) -> Result<moraine_platform::CaptureEndpoint> {
    if let Some(path) = explicit {
        return Ok(moraine_platform::CaptureEndpoint::UnixSocket(
            path.to_path_buf(),
        ));
    }
    match &layout.capture_endpoint {
        moraine_platform::CaptureEndpoint::UnixSocket(path) => {
            Ok(moraine_platform::CaptureEndpoint::UnixSocket(path.clone()))
        }
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
    Json(json!({
        "status": "ok",
        "online": true,
        "version": build.version,
        "productVersion": build.version,
        "gitCommit": build.git_commit,
        "serviceProtocolVersion": build.service_protocol_version,
        "schema": build.schema,
        "executablePath": executable,
        "captureReady": true,
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

/// Render a user unit with an absolute ExecStart for the running binary.
/// Prefer `moraine service install` from the installed suite CLI.
fn systemd_unit() -> String {
    let exec = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/usr/bin/moraine-service".into());
    // Refuse to embed private build paths that would pin a checkout target/.
    let exec = if exec.contains("/target/") {
        // Fall back to suite layout; installers rewrite absolute paths.
        "%h/.local/libexec/moraine/moraine-service".into()
    } else {
        exec
    };
    format!(
        r#"[Unit]
Description=Moraine local integration runtime (per-user)
After=network.target

[Service]
Type=simple
ExecStart={exec} --http 127.0.0.1:33111 --unix-socket %t/moraine-service.sock
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(endpoint: moraine_platform::CaptureEndpoint) -> moraine_platform::RuntimeLayout {
        moraine_platform::RuntimeLayout {
            spool_dir: "/tmp/spool".into(),
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
            resolve_capture_endpoint(Some(std::path::Path::new("/explicit.sock")), &runtime)
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
            resolve_capture_endpoint(None, &runtime).unwrap(),
            moraine_platform::CaptureEndpoint::UnixSocket(PathBuf::from("/runtime/shared.sock"))
        );
    }

    #[test]
    fn unsupported_capture_endpoint_fails_explicitly() {
        let runtime = layout(moraine_platform::CaptureEndpoint::Unsupported);
        assert!(resolve_capture_endpoint(None, &runtime)
            .unwrap_err()
            .to_string()
            .contains("unsupported capture endpoint"));
    }
}
