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
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, UnixListener},
    sync::Notify,
};
use tracing::{error, info};

const MAX_SPOOL_FILES: usize = moraine_service::MAX_PENDING_EVENTS;

#[derive(Clone)]
struct AppState {
    spool_dir: PathBuf,
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
                    std::fs::create_dir_all(home_unit.parent().unwrap()).ok();
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

    let spool_dir = args.spool_dir.unwrap_or_else(|| {
        dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("moraine-service/spool")
    });
    std::fs::create_dir_all(&spool_dir)?;
    tokio::fs::create_dir_all(spool_dir.join("processed"))
        .await
        .ok();
    tokio::fs::create_dir_all(spool_dir.join("failed"))
        .await
        .ok();

    let shutdown = Arc::new(Notify::new());
    let state = AppState {
        spool_dir: spool_dir.clone(),
    };

    // Diagnostics HTTP on loopback only — not the hook transport.
    let http_addr: SocketAddr = args.http.parse()?;
    if !http_addr.ip().is_loopback() {
        anyhow::bail!(
            "refusing non-loopback HTTP bind {http_addr}; diagnostics must use 127.0.0.1/::1. \
             Hook delivery uses the Unix domain socket, not TCP."
        );
    }
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
    if let Some(socket_path) = args.unix_socket {
        let spool = spool_dir.clone();
        let shutdown_clone = shutdown.clone();
        tokio::spawn(async move {
            if let Err(e) = unix_listener_loop(socket_path, spool, shutdown_clone).await {
                error!(error = %e, "unix listener failed");
            }
        });
    } else {
        // Default to $XDG_RUNTIME_DIR/moraine-service.sock (matches systemd unit).
        let default_sock = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("moraine-service.sock");
        let spool = spool_dir.clone();
        let shutdown_clone = shutdown.clone();
        info!(socket=%default_sock.display(), "binding default unix hook socket");
        tokio::spawn(async move {
            if let Err(e) = unix_listener_loop(default_sock, spool, shutdown_clone).await {
                error!(error = %e, "unix listener failed");
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
        let base = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        tokio::spawn(async move {
            let out = spool.join("index.json");
            loop {
                if let Err(e) = moraine_service::rebuild_index(base.clone(), out.clone(), 6).await {
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
    Json(json!({
        "status": "ok",
        "online": true,
        "spoolDir": state.spool_dir.display().to_string(),
        "spool": {
            "pending": pending,
            "processed": processed,
            "failed": failed,
        },
        "indexMtimeUnix": index_mtime,
        "revision": revision,
        "indexRevision": revision,
    }))
}

async fn handle_projects(State(state): State<AppState>) -> Json<Value> {
    if let Some(doc) = moraine_service::read_index_projects(&state.spool_dir) {
        return Json(doc);
    }
    // Fallback one-shot scan (does not write a second durable index here).
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let roots = moraine_core::scan_project_roots(&cwd, 4);
    let mut projects = vec![];
    for d in roots {
        if let Ok(s) = moraine_core::summarize_project(&d) {
            projects.push(json!({
                "projectId": s.project_id.to_string(),
                "name": s.name,
                "root": s.root_path,
                "rootPath": s.root_path,
                "available": s.available,
                "run_count": s.run_counts.recent,
                "runCounts": s.run_counts,
                "openFindingCount": s.open_finding_count,
                "lastActivityAt": s.last_activity_at,
                "warning": s.warning,
            }));
        }
    }
    Json(json!({
        "projects": projects,
        "revision": 0,
        "fallback": true,
    }))
}

async fn handle_project_runs(
    State(state): State<AppState>,
    AxumPath(project_id): AxumPath<String>,
) -> Json<Value> {
    let root =
        moraine_service::find_project_root_in_index(&state.spool_dir, &project_id).or_else(|| {
            // Fallback: scan cwd
            let cwd = std::env::current_dir().ok()?;
            moraine_core::scan_project_roots(&cwd, 4)
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
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let out = state.spool_dir.join("index.json");
    let before = moraine_service::index_revision(&state.spool_dir);
    match moraine_service::rebuild_index(base, out, 6).await {
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

async fn unix_listener_loop(
    socket_path: PathBuf,
    spool_dir: PathBuf,
    shutdown: Arc<Notify>,
) -> Result<()> {
    let _ = std::fs::remove_file(&socket_path);
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let listener = UnixListener::bind(&socket_path)?;
    info!(socket=%socket_path.display(), "unix socket bound");
    tokio::fs::create_dir_all(spool_dir.join("processed"))
        .await
        .ok();
    tokio::fs::create_dir_all(spool_dir.join("failed"))
        .await
        .ok();

    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                let mut buf = Vec::new();
                // Read one byte past the accepted maximum so an oversized event is
                // rejected instead of being silently truncated into a valid payload.
                let mut limited = stream.take((moraine_service::MAX_EVENT_BYTES + 1) as u64);
                match tokio::io::AsyncReadExt::read_to_end(&mut limited, &mut buf).await {
                    Ok(_) => {
                        match moraine_service::write_spooled_payload(&spool_dir, &buf).await {
                            Ok(p) => info!(file=%p.display(), "spooled event"),
                            Err(e) => error!(error=%e, "failed to spool payload"),
                        }
                    }
                    Err(e) => error!(%e, "failed to read unix socket payload"),
                }
            }
            _ = shutdown.notified() => {
                info!(socket=%socket_path.display(), "shutting down unix listener");
                break;
            }
        }
    }

    Ok(())
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

fn systemd_unit() -> &'static str {
    include_str!("../systemd/moraine-service.service")
}
