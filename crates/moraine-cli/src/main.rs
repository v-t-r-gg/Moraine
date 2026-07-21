//! CLI for agent run records and review helpers. Fail-fast share unless `--start`.

mod codex_setup;
mod doctor;
mod hook_codex;
mod relay;
mod run_cli;
mod service_cmd;
mod setup_cmd;
mod suite;

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use moraine_core::history::HistorySource;
use moraine_core::Error as CoreError;
use moraine_core::{
    content_hash, ensure_run_meta, load_run_meta_readonly, moraine_sidecar_path, record_decision,
    room_id_for_path, share_links, status_snapshot, AnnotationKind, DecisionKind, Document,
    HistoryStore, MorainePaths, ReviewStateKind, DEFAULT_RELAY_HTTP, DEFAULT_UI,
};
use serde::Serialize;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use relay::{health_ok, launch_desktop, launch_desktop_workspace, try_spawn_server};

/// Exit codes for scripts/agents.
const EXIT_OK: i32 = 0;
const EXIT_ERR: i32 = 1;
const EXIT_NOT_FOUND: i32 = 2;
const EXIT_RELAY: i32 = 3;

#[derive(Debug, Parser)]
#[command(
    name = "moraine",
    version,
    about = "Moraine CLI: run records and review helpers for agents and scripts",
    long_about = "Create and inspect Markdown run records, share live review rooms, and read sidecar status.\n\
                  Agent protocol: moraine project init; moraine run start|show|checkpoint|ready|resume|open --json.\n\
                  Install suite: moraine version|doctor|service|setup.\n\
                  Exit codes: 0 ok, 1 error, 2 not found, 3 relay down.\n\
                  Prefer --json on share/status/info/join/run/project when calling from scripts."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Increase log verbosity (`-v`, `-vv`). Global; not the same as `version --verbose`.
    #[arg(short = 'v', action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print version and data directories
    Info {
        /// Machine-readable object on stdout
        #[arg(long)]
        json: bool,
    },

    /// Snapshot path/room, relay health, and sidecar review counts
    #[command(after_help = "Examples:\n  \
        moraine status notes.md\n  \
        moraine status notes.md --human\n  \
        moraine status doc_abc123\n  \
        moraine status")]
    Status {
        /// Markdown path or room id (`doc_…`). Omit for relay-only status.
        target: Option<String>,
        #[arg(long, env = "MORAINE_SERVER_URL", default_value = DEFAULT_RELAY_HTTP)]
        server: String,
        #[arg(long, env = "MORAINE_UI_URL", default_value = DEFAULT_UI)]
        ui: String,
        /// JSON on stdout (default). Use --human for lines.
        #[arg(long, default_value_t = true)]
        json: bool,
        /// Human-readable lines instead of JSON
        #[arg(long)]
        human: bool,
    },

    /// Print file contents
    Cat { path: PathBuf },

    /// Write content or stdin to a file
    Write {
        path: PathBuf,
        #[arg(long, short)]
        content: Option<String>,
        #[arg(long)]
        history: bool,
    },

    /// Open a file in the desktop app or $EDITOR
    Edit {
        path: PathBuf,
        #[arg(long, default_value_t = true)]
        create: bool,
        /// Print share URL first (relay must be up)
        #[arg(long)]
        share: bool,
    },

    /// Print a collab join URL for a Markdown file
    #[command(after_help = "Examples:\n  \
        moraine share notes.md\n  \
        moraine share notes.md --json\n  \
        moraine share notes.md --start --json\n  \
        moraine share notes.md --open")]
    Share {
        path: PathBuf,
        #[arg(long, env = "MORAINE_UI_URL", default_value = DEFAULT_UI)]
        ui: String,
        #[arg(long, env = "MORAINE_SERVER_URL", default_value = DEFAULT_RELAY_HTTP)]
        server: String,
        /// Spawn moraine-server once if health check fails
        #[arg(long)]
        start: bool,
        /// Structured result (or error with code) on stdout
        #[arg(long)]
        json: bool,
        /// Also launch the desktop app for this path
        #[arg(long)]
        open: bool,
    },

    /// Resolve a room/URL for joining (optionally open a browser)
    #[command(after_help = "Examples:\n  \
        moraine join doc_abc123 --json --no-open\n  \
        moraine join 'http://localhost:1420/?room=doc_abc123'")]
    Join {
        /// Full UI URL or bare room id (`doc_…`)
        target: String,
        #[arg(long, env = "MORAINE_UI_URL", default_value = DEFAULT_UI)]
        ui: String,
        /// Structured result on stdout
        #[arg(long)]
        json: bool,
        /// Print URL only; do not open a browser
        #[arg(long)]
        no_open: bool,
    },

    /// List local edit history for a path
    History {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, short = 'n', default_value_t = 20)]
        limit: usize,
    },

    /// Restore a history entry
    Restore {
        path: PathBuf,
        entry_id: String,
        #[arg(long)]
        write: bool,
    },

    /// Watch a path for filesystem events
    Watch { path: PathBuf },

    /// Create or migrate the run ledger for a Markdown file (idempotent)
    #[command(after_help = "Examples:\n  \
        moraine init run.md --json")]
    Init {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Record a run-level review decision (legacy / compatibility-only)
    #[command(
        after_help = "LEGACY: Run-level decisions are compatibility-only. Prefer comments, suggestions, and human notes.\n\
Moraine records review activity; it does not authorize merge or deployment.\n\n\
Examples:\n  \
        moraine decide run.md --decision approved --reviewer Alice --json\n  \
        moraine decide run.md --decision changes_requested --reviewer Bob --reason 'fix outcomes'\n  \
        moraine decide run.md --decision approved --reviewer Ada --expected-hash <sha256> --json"
    )]
    Decide {
        path: PathBuf,
        /// approved | changes_requested | rejected
        #[arg(long)]
        decision: String,
        /// Reviewer label (not authenticated identity)
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        reason: Option<String>,
        /// Optional expected content hash; defaults to current on-disk hash
        #[arg(long)]
        expected_hash: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Moraine project init/discovery under `.moraine`
    Project {
        #[command(subcommand)]
        cmd: run_cli::ProjectCmd,
    },

    /// Agent run protocol: start, checkpoint, ready, resume, show, open
    Run {
        #[command(subcommand)]
        cmd: run_cli::RunCmd,
    },

    /// Local STDIO MCP server for agent-run tools (project-scoped)
    #[command(after_help = "Examples:\n  \
        moraine mcp --project /absolute/path/to/repo\n  \
        moraine mcp\n\
        Protocol frames on stdout; diagnostics on stderr. No network listener.")]
    Mcp {
        /// Project root (Git root or directory with `.moraine`). Fixed for process lifetime.
        #[arg(long)]
        project: Option<PathBuf>,
    },

    /// Codex lifecycle hook adapter (stdin JSON → local service / spool)
    #[command(
        name = "hook-codex",
        after_help = "Intended for Codex hooks.json command handlers.\n\
Reads Codex hook JSON from stdin, maps SessionStart / UserPromptSubmit / Stop\n\
to Moraine mechanical events, and delivers them to the local service Unix socket.\n\
On delivery failure, events are written to the local spool (exit 0)."
    )]
    HookCodex {
        /// Unix socket path (default: $MORAINE_SOCKET or $XDG_RUNTIME_DIR/moraine-service.sock)
        #[arg(long)]
        socket: Option<PathBuf>,
        /// Spool directory used when the service is unavailable
        #[arg(long)]
        spool_dir: Option<PathBuf>,
    },

    /// Product version and installed-suite identity
    Version {
        #[arg(long)]
        json: bool,
        /// Include suite/service/desktop paths and PATH drift (same as --json detail).
        /// Named `--verbose` here (not `-v`) so it does not collide with the global `-v` count.
        #[arg(long = "verbose", visible_alias = "long")]
        long: bool,
    },

    /// Installation and integration health report
    Doctor {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        integration: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Per-user moraine-service lifecycle (systemd --user on Linux)
    Service {
        #[command(subcommand)]
        cmd: ServiceSub,
    },

    /// Post-install suite check (and optional project integrations)
    Setup {
        /// Integration subcommand; omit for bare post-install setup
        #[command(subcommand)]
        cmd: Option<SetupSub>,
        #[arg(long, global = true)]
        json: bool,
    },

    /// Alias for project-scoped integrations (`setup codex`)
    Integrate {
        #[command(subcommand)]
        cmd: IntegrateSub,
    },

    /// Open the installed desktop (ledger workspace or a run path)
    Open {
        /// Optional Markdown run path
        #[arg(long)]
        path: Option<PathBuf>,
        /// Optional run id (resolved under project if --project set)
        #[arg(long)]
        run_id: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ServiceSub {
    Install {
        #[arg(long)]
        json: bool,
    },
    Start {
        #[arg(long)]
        json: bool,
    },
    Stop {
        #[arg(long)]
        json: bool,
    },
    Restart {
        #[arg(long)]
        json: bool,
    },
    Status {
        #[arg(long)]
        json: bool,
    },
    Logs {
        #[arg(long)]
        json: bool,
    },
    Uninstall {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SetupSub {
    /// Configure project-scoped Codex MCP + hooks for Moraine
    Codex {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
        /// Validate without writing
        #[arg(long)]
        check: bool,
        /// Remove managed Moraine Codex configuration (with backups)
        #[arg(long)]
        remove: bool,
    },
}

#[derive(Debug, Subcommand)]
enum IntegrateSub {
    /// Configure project-scoped Codex MCP + hooks (same as `setup codex`)
    Codex {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        remove: bool,
    },
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            // Non-json fallback for unexpected errors
            eprintln!("error: {err:#}");
            std::process::exit(EXIT_ERR);
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let code = match cli.command {
        Commands::Info { json } => cmd_info(json),
        Commands::Status {
            target,
            server,
            ui,
            json,
            human,
        } => cmd_status(target, server, ui, json && !human),
        Commands::Cat { path } => cmd_cat(path).map(|_| EXIT_OK),
        Commands::Write {
            path,
            content,
            history,
        } => cmd_write(path, content, history).map(|_| EXIT_OK),
        Commands::Edit {
            path,
            create,
            share,
        } => cmd_edit(path, create, share).map(|_| EXIT_OK),
        Commands::Share {
            path,
            ui,
            server,
            start,
            json,
            open,
        } => cmd_share(path, ui, server, start, json, open),
        Commands::Join {
            target,
            ui,
            json,
            no_open,
        } => cmd_join(target, ui, json, no_open),
        Commands::History { path, json, limit } => cmd_history(path, json, limit).map(|_| EXIT_OK),
        Commands::Restore {
            path,
            entry_id,
            write,
        } => cmd_restore(path, entry_id, write).map(|_| EXIT_OK),
        Commands::Watch { path } => cmd_watch(path).map(|_| EXIT_OK),
        Commands::Init { path, json } => cmd_init(path, json),
        Commands::Decide {
            path,
            decision,
            reviewer,
            reason,
            expected_hash,
            json,
        } => cmd_decide(path, decision, reviewer, reason, expected_hash, json),
        Commands::Project { cmd } => run_cli::dispatch_project(cmd),
        Commands::Run { cmd } => run_cli::dispatch_run(cmd),
        Commands::Mcp { project } => {
            // Blocking STDIO MCP loop; never write human diagnostics to stdout.
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("tokio runtime")?;
            match rt.block_on(moraine_mcp::run_stdio_server(project)) {
                Ok(()) => Ok(EXIT_OK),
                Err(e) => {
                    eprintln!("error: {e:#}");
                    Ok(EXIT_ERR)
                }
            }
        }
        Commands::HookCodex { socket, spool_dir } => hook_codex::run_hook_codex(socket, spool_dir),
        Commands::Version { json, long } => cmd_version(json, long),
        Commands::Doctor {
            project,
            integration,
            json,
        } => cmd_doctor(project, integration, json),
        Commands::Service { cmd } => cmd_service(cmd),
        Commands::Setup { cmd, json } => match cmd {
            None => setup_cmd::setup_post_install(json),
            Some(sub) => cmd_setup_codex(sub),
        },
        Commands::Integrate { cmd } => cmd_integrate(cmd),
        Commands::Open {
            path,
            run_id,
            project,
            json,
        } => cmd_open(path, run_id, project, json),
    }?;
    Ok(code)
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .try_init();
}

fn emit_err(json: bool, code: i32, msg: &str) -> i32 {
    // Keep agent and human wording aligned: short, actionable.
    if json {
        let _ = writeln!(
            io::stdout(),
            "{}",
            serde_json::json!({ "ok": false, "error": msg, "code": code })
        );
    } else {
        eprintln!("error: {msg}");
    }
    code
}

fn emit_core_err(json: bool, err: &CoreError) -> i32 {
    let code = match err {
        CoreError::NotFound(_) => EXIT_NOT_FOUND,
        CoreError::RevisionConflict { .. } | CoreError::LedgerBusy(_) => EXIT_ERR,
        _ => EXIT_ERR,
    };
    if json {
        let _ = writeln!(
            io::stdout(),
            "{}",
            serde_json::json!({
                "ok": false,
                "error": err.to_json_value(),
                "code": code,
            })
        );
    } else {
        eprintln!("error: {err}");
    }
    code
}

fn cmd_info(json: bool) -> Result<i32> {
    let paths = MorainePaths::default_ensure().ok();
    let build = moraine_core::BuildIdentity::current();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "name": "moraine",
                "version": build.version,
                "gitCommit": build.git_commit,
                "schema": build.schema,
                "dataDir": paths.as_ref().map(|p| p.data_dir.display().to_string()),
                "historyDir": paths.as_ref().map(|p| p.history_dir.display().to_string()),
                "configDir": paths.as_ref().map(|p| p.config_dir.display().to_string()),
            }))?
        );
    } else {
        println!("moraine {}", build.version);
        if let Some(p) = paths {
            println!("data dir:    {}", p.data_dir.display());
            println!("history dir: {}", p.history_dir.display());
            println!("config dir:  {}", p.config_dir.display());
        }
    }
    Ok(EXIT_OK)
}

fn cmd_version(json: bool, long: bool) -> Result<i32> {
    let report = suite::collect_version_report();
    if json || long {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Moraine {}", report.cli.version);
        if let Some(s) = &report.suite {
            println!("suite {} ({})", s.version, s.manifest_path);
        }
        if report.service.online {
            println!(
                "service online{}",
                report
                    .service
                    .version
                    .as_ref()
                    .map(|v| format!(" {v}"))
                    .unwrap_or_default()
            );
        } else {
            println!("service offline");
        }
        if let Some(w) = &report.warnings {
            for line in w {
                eprintln!("warning: {line}");
            }
        }
    }
    // Version inspection always exits 0; warnings are advisory (doctor is the gate).
    Ok(EXIT_OK)
}

fn cmd_doctor(project: Option<PathBuf>, integration: Option<String>, json: bool) -> Result<i32> {
    let report = doctor::run_doctor(project.as_deref(), integration.as_deref());
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Moraine doctor — {} {}",
            report.build.product, report.build.version
        );
        for c in &report.checks {
            let mark = match c.status.as_str() {
                "pass" | "ok" => "PASS",
                "warn" => "WARN",
                "info" => "INFO",
                _ => "FAIL",
            };
            println!("[{mark}] {}: {}", c.id, c.message);
            if let (Some(o), Some(e)) = (&c.observed, &c.expected) {
                if c.status == "fail" || c.status == "warn" {
                    println!("       observed: {o}");
                    println!("       expected: {e}");
                }
            }
            if let Some(r) = &c.remediation {
                if c.status == "fail" || c.status == "warn" {
                    println!("       → {r}");
                }
            }
        }
        if let Some(p) = &report.project {
            println!("project: {} initialized={}", p.path, p.initialized);
        }
        if let Some(i) = &report.integration {
            println!("integration {}: configured={}", i.name, i.configured);
        }
        println!(
            "{}",
            if report.ok {
                "doctor: all checks passed"
            } else {
                "doctor: one or more checks failed"
            }
        );
    }
    Ok(if report.ok { EXIT_OK } else { EXIT_ERR })
}

fn cmd_service(cmd: ServiceSub) -> Result<i32> {
    match cmd {
        ServiceSub::Install { json } => service_cmd::service_install(json)?,
        ServiceSub::Start { json } => service_cmd::service_start(json)?,
        ServiceSub::Stop { json } => service_cmd::service_stop(json)?,
        ServiceSub::Restart { json } => service_cmd::service_restart(json)?,
        ServiceSub::Status { json } => service_cmd::service_status(json)?,
        ServiceSub::Logs { json } => service_cmd::service_logs(json)?,
        ServiceSub::Uninstall { json } => service_cmd::service_uninstall(json)?,
    }
    Ok(EXIT_OK)
}

fn cmd_setup_codex(cmd: SetupSub) -> Result<i32> {
    match cmd {
        SetupSub::Codex {
            project,
            dry_run,
            json,
            check,
            remove,
        } => dispatch_codex_integration(&project, dry_run, json, check, remove),
    }
}

fn cmd_integrate(cmd: IntegrateSub) -> Result<i32> {
    match cmd {
        IntegrateSub::Codex {
            project,
            dry_run,
            json,
            check,
            remove,
        } => dispatch_codex_integration(&project, dry_run, json, check, remove),
    }
}

fn dispatch_codex_integration(
    project: &std::path::Path,
    dry_run: bool,
    json: bool,
    check: bool,
    remove: bool,
) -> Result<i32> {
    if remove {
        codex_setup::remove_codex(project, dry_run, json)?;
    } else if check {
        codex_setup::check_codex(project, json)?;
    } else {
        codex_setup::setup_codex(project, dry_run, json)?;
    }
    Ok(EXIT_OK)
}

fn cmd_open(
    path: Option<PathBuf>,
    run_id: Option<String>,
    project: Option<PathBuf>,
    json: bool,
) -> Result<i32> {
    let open_path = if let Some(p) = path {
        Some(p)
    } else if let (Some(rid), Some(proj)) = (run_id.as_ref(), project.as_ref()) {
        // Best-effort: find run markdown under project .moraine/runs
        let runs = proj.join(".moraine/runs");
        let found = std::fs::read_dir(&runs).ok().and_then(|rd| {
            rd.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
                p.extension().and_then(|x| x.to_str()) == Some("md")
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.contains(rid.as_str()) || n.ends_with(&format!("{rid}.md")))
                        .unwrap_or(false)
            })
        });
        found
    } else {
        None
    };
    let launched = if let Some(ref p) = open_path {
        launch_desktop(p)?
    } else {
        launch_desktop_workspace(None)?
    };
    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": launched,
                "path": open_path.as_ref().map(|p| p.display().to_string()),
                "runId": run_id,
                "message": if launched {
                    "launched installed desktop"
                } else {
                    "desktop binary not found; install suite with moraine-app or set PATH"
                }
            })
        );
    } else if launched {
        println!("opened installed desktop");
    } else {
        eprintln!("error: could not launch moraine-app (install suite desktop component)");
        return Ok(EXIT_ERR);
    }
    Ok(if launched { EXIT_OK } else { EXIT_ERR })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusOut {
    ok: bool,
    path: Option<String>,
    room: Option<String>,
    exists: Option<bool>,
    relay: RelayStatus,
    ui: String,
    join_url: Option<String>,
    sidecar: Option<String>,
    sidecar_exists: Option<bool>,
    annotations: Option<AnnotationCounts>,
    run: Option<RunStatusJson>,
    review: Option<ReviewStatusJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunStatusJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    initialized: bool,
    content_hash: String,
    review_state: String,
    decision_current: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewStatusJson {
    review_state: String,
    latest_decision: Option<DecisionJson>,
    decision_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionJson {
    id: String,
    decision: String,
    reviewer_label: String,
    reason: Option<String>,
    created_at: String,
    content_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RelayStatus {
    url: String,
    ok: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationCounts {
    comments_open: usize,
    suggestions_open: usize,
    comments_resolved: usize,
    suggestions_resolved: usize,
}

fn cmd_status(target: Option<String>, server: String, ui: String, json: bool) -> Result<i32> {
    let relay_ok = health_ok(&server);
    let mut out = StatusOut {
        ok: true,
        path: None,
        room: None,
        exists: None,
        relay: RelayStatus {
            url: server.clone(),
            ok: relay_ok,
        },
        ui: ui.clone(),
        join_url: None,
        sidecar: None,
        sidecar_exists: None,
        annotations: None,
        run: None,
        review: None,
    };

    if let Some(t) = target {
        if t.starts_with("doc_") {
            out.room = Some(t.clone());
            out.join_url = Some(format!("{}/?room={t}", ui.trim_end_matches('/')));
        } else {
            let path = PathBuf::from(&t);
            if !path.exists() {
                return Ok(emit_err(
                    json,
                    EXIT_NOT_FOUND,
                    &format!("path not found: {}", path.display()),
                ));
            }
            let abs = std::fs::canonicalize(&path).unwrap_or(path);
            // Read-only: never create or migrate the ledger.
            let snap = match status_snapshot(&abs) {
                Ok(s) => s,
                Err(e) => return Ok(emit_core_err(json, &e)),
            };
            let comments = match load_run_meta_readonly(&abs) {
                Ok(Some(m)) => m.comments,
                Ok(None) => moraine_core::read_comments_sidecar(&abs)
                    .map(|f| f.comments)
                    .unwrap_or_default(),
                Err(e) => return Ok(emit_core_err(json, &e)),
            };
            let room = room_id_for_path(&abs);
            let side = moraine_sidecar_path(&abs);
            out.path = Some(abs.display().to_string());
            out.room = Some(room.clone());
            out.exists = Some(true);
            out.join_url = Some(format!("{}/?room={room}", ui.trim_end_matches('/')));
            out.sidecar = Some(side.display().to_string());
            out.sidecar_exists = Some(side.exists());
            out.annotations = Some(annotation_counts_from(&comments));
            out.run = Some(RunStatusJson {
                id: if snap.initialized {
                    Some(snap.run_id.to_string())
                } else {
                    None
                },
                initialized: snap.initialized,
                content_hash: snap.content_hash.clone(),
                review_state: review_state_str(snap.state).into(),
                decision_current: snap.decision_current,
            });
            out.review = Some(ReviewStatusJson {
                review_state: review_state_str(snap.state).into(),
                latest_decision: snap.latest.as_ref().map(decision_json),
                decision_count: snap.decision_count,
            });
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "relay  {} ({})",
            server,
            if relay_ok { "up" } else { "down" }
        );
        if let Some(p) = &out.path {
            println!("path   {p}");
        }
        if let Some(r) = &out.room {
            println!("room   {r}");
        }
        if let Some(run) = &out.run {
            if run.initialized {
                if let Some(id) = &run.id {
                    println!("run    {id}");
                }
            } else {
                println!("run    (not initialized; moraine init <file>)");
            }
            let short = &run.content_hash[..12.min(run.content_hash.len())];
            println!("hash   {short}…");
            if run.decision_current {
                println!("state  {}", run.review_state);
            } else {
                println!(
                    "state  {} (stale: content changed since decision)",
                    run.review_state
                );
            }
        }
        if let Some(u) = &out.join_url {
            println!("join   {u}");
        }
        if let Some(a) = &out.annotations {
            println!(
                "notes  comments open={} resolved={} suggestions open={} resolved={}",
                a.comments_open, a.comments_resolved, a.suggestions_open, a.suggestions_resolved
            );
        }
    }
    Ok(EXIT_OK)
}

fn review_state_str(s: ReviewStateKind) -> &'static str {
    match s {
        ReviewStateKind::Unreviewed => "unreviewed",
        ReviewStateKind::Approved => "approved",
        ReviewStateKind::ChangesRequested => "changes_requested",
        ReviewStateKind::Rejected => "rejected",
        ReviewStateKind::Stale => "stale",
    }
}

fn decision_json(d: &moraine_core::ReviewDecision) -> DecisionJson {
    DecisionJson {
        id: d.id.to_string(),
        decision: d.decision.as_str().into(),
        reviewer_label: d.reviewer_label.clone(),
        reason: d.reason.clone(),
        created_at: d.created_at.to_rfc3339(),
        content_hash: d.content_hash.clone(),
    }
}

fn annotation_counts_from(comments: &[moraine_core::CommentRecord]) -> AnnotationCounts {
    let mut c = AnnotationCounts {
        comments_open: 0,
        suggestions_open: 0,
        comments_resolved: 0,
        suggestions_resolved: 0,
    };
    for item in comments {
        let sug = item.kind == AnnotationKind::Suggestion;
        match (sug, item.resolved) {
            (false, false) => c.comments_open += 1,
            (false, true) => c.comments_resolved += 1,
            (true, false) => c.suggestions_open += 1,
            (true, true) => c.suggestions_resolved += 1,
        }
    }
    c
}

fn cmd_init(path: PathBuf, json: bool) -> Result<i32> {
    if !path.exists() {
        return Ok(emit_err(
            json,
            EXIT_NOT_FOUND,
            &format!("path not found: {}", path.display()),
        ));
    }
    let abs = std::fs::canonicalize(&path).unwrap_or(path);
    let meta = match ensure_run_meta(&abs) {
        Ok(m) => m,
        Err(e) => return Ok(emit_core_err(json, &e)),
    };
    let markdown = match Document::read_file(&abs) {
        Ok(s) => s,
        Err(e) => return Ok(emit_core_err(json, &e)),
    };
    let hash = content_hash(&markdown);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "run": {
                    "id": meta.run.id.to_string(),
                    "initialized": true,
                    "contentHash": hash,
                },
                "sidecar": moraine_sidecar_path(&abs).display().to_string(),
            }))?
        );
    } else {
        println!("initialized run {}", meta.run.id);
        println!("sidecar {}", moraine_sidecar_path(&abs).display());
    }
    Ok(EXIT_OK)
}

fn cmd_decide(
    path: PathBuf,
    decision: String,
    reviewer: String,
    reason: Option<String>,
    expected_hash: Option<String>,
    json: bool,
) -> Result<i32> {
    eprintln!(
        "warning: `moraine decide` is legacy/compatibility-only; prefer comments and human notes"
    );
    if !path.exists() {
        return Ok(emit_err(
            json,
            EXIT_NOT_FOUND,
            &format!("path not found: {}", path.display()),
        ));
    }
    let kind = match DecisionKind::parse(&decision) {
        Some(k) => k,
        None => {
            return Ok(emit_err(
                json,
                EXIT_ERR,
                "invalid --decision (use approved, changes_requested, or rejected)",
            ));
        }
    };
    if reviewer.trim().is_empty() {
        return Ok(emit_err(json, EXIT_ERR, "--reviewer must not be empty"));
    }
    let abs = std::fs::canonicalize(&path).unwrap_or(path);
    let markdown = match Document::read_file(&abs) {
        Ok(s) => s,
        Err(e) => return Ok(emit_core_err(json, &e)),
    };
    let disk_hash = content_hash(&markdown);
    let expected = expected_hash.unwrap_or_else(|| disk_hash.clone());
    let (_meta, recorded, snap) =
        match record_decision(&abs, kind, reviewer.trim(), reason, &expected) {
            Ok(v) => v,
            Err(e) => return Ok(emit_core_err(json, &e)),
        };
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "run": {
                    "id": snap.run_id.to_string(),
                    "initialized": true,
                    "contentHash": snap.content_hash,
                    "reviewState": review_state_str(snap.state),
                    "decisionCurrent": snap.decision_current,
                },
                "review": {
                    "latestDecision": decision_json(&recorded),
                    "decisionCount": snap.decision_count,
                }
            }))?
        );
    } else {
        println!(
            "recorded {} by {} on run {}",
            recorded.decision.as_str(),
            recorded.reviewer_label,
            snap.run_id
        );
        println!("content hash {}", recorded.content_hash);
    }
    Ok(EXIT_OK)
}

fn cmd_cat(path: PathBuf) -> Result<()> {
    let content =
        Document::read_file(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(content.as_bytes())?;
    if !content.ends_with('\n') {
        stdout.write_all(b"\n")?;
    }
    Ok(())
}

fn cmd_write(path: PathBuf, content: Option<String>, history: bool) -> Result<()> {
    let body = match content {
        Some(c) => c,
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };
    Document::write_file(&path, &body)
        .with_context(|| format!("failed to write {}", path.display()))?;
    if history {
        let paths = MorainePaths::default_ensure()?;
        HistoryStore::new(paths).push(
            &path,
            &body,
            HistorySource::Manual,
            Some("cli write".into()),
        )?;
    }
    eprintln!("wrote {} ({} bytes)", path.display(), body.len());
    Ok(())
}

fn cmd_edit(path: PathBuf, create: bool, share: bool) -> Result<()> {
    if !path.exists() {
        if create {
            Document::create(&path, "# New document\n\n")?;
            eprintln!("created {}", path.display());
        } else {
            bail!("file does not exist: {}", path.display());
        }
    }
    if share {
        let _ = cmd_share(
            path.clone(),
            DEFAULT_UI.into(),
            DEFAULT_RELAY_HTTP.into(),
            false,
            false,
            false,
        )?;
    }
    if launch_desktop(&path)? {
        return Ok(());
    }
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "nano".into());
    let status = Command::new(&editor)
        .arg(&path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to launch editor `{editor}`"))?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(())
}

fn cmd_share(
    path: PathBuf,
    ui: String,
    server: String,
    start: bool,
    json: bool,
    open: bool,
) -> Result<i32> {
    if !path.exists() {
        return Ok(emit_err(
            json,
            EXIT_NOT_FOUND,
            &format!("file does not exist: {}", path.display()),
        ));
    }
    if start && !health_ok(&server) {
        if let Err(e) = try_spawn_server(&server) {
            return Ok(emit_err(json, EXIT_RELAY, &e.to_string()));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if health_ok(&server) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    if !health_ok(&server) {
        return Ok(emit_err(
            json,
            EXIT_RELAY,
            &format!(
                "relay not reachable at {server}/health (start with: cargo run -p moraine-server, or pass --start)"
            ),
        ));
    }
    let links = share_links(&path, &ui, &server);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "path": links.path,
                "room": links.room,
                "url": links.url,
                "ws": links.ws,
                "server": links.server,
            }))?
        );
    } else {
        println!("{}", links.url);
        eprintln!("room   {}", links.room);
        eprintln!("file   {}", links.path);
        eprintln!("ws     {}", links.ws);
    }
    if open {
        let _ = launch_desktop(path.as_path())?;
    }
    Ok(EXIT_OK)
}

fn cmd_join(target: String, ui: String, json: bool, no_open: bool) -> Result<i32> {
    let url = if target.starts_with("http://") || target.starts_with("https://") {
        target
    } else if target.starts_with("doc_") {
        format!("{}/?room={target}", ui.trim_end_matches('/'))
    } else {
        return Ok(emit_err(
            json,
            EXIT_ERR,
            &format!("expected a URL or room id (doc_…), got: {target}"),
        ));
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "url": url,
            }))?
        );
    } else {
        println!("{url}");
    }

    if !no_open {
        let status = Command::new("xdg-open")
            .arg(&url)
            .status()
            .or_else(|_| Command::new("open").arg(&url).status());
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                return Ok(emit_err(
                    json,
                    EXIT_ERR,
                    &format!("browser open failed: {s}"),
                ));
            }
            Err(e) => {
                if !json {
                    eprintln!("note: could not open browser ({e}); URL printed above");
                }
            }
        }
    }
    Ok(EXIT_OK)
}

fn cmd_history(path: PathBuf, json: bool, limit: usize) -> Result<()> {
    let paths = MorainePaths::default_ensure()?;
    let mut entries = HistoryStore::new(paths).list_meta(&path)?;
    entries.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }
    if entries.is_empty() {
        println!("(no history for {})", path.display());
        return Ok(());
    }
    println!("{:<36}  {:<20}  {:>8}  SOURCE", "ID", "WHEN (UTC)", "BYTES");
    for e in entries {
        let when = e.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let source = format!("{:?}", e.source).to_lowercase();
        let label = e.label.unwrap_or_default();
        println!(
            "{:<36}  {:<20}  {:>8}  {} {}",
            e.id, when, e.byte_len, source, label
        );
    }
    Ok(())
}

fn cmd_restore(path: PathBuf, entry_id: String, write: bool) -> Result<()> {
    let id = Uuid::parse_str(&entry_id).with_context(|| format!("invalid UUID: {entry_id}"))?;
    let paths = MorainePaths::default_ensure()?;
    let store = HistoryStore::new(paths);
    let content = store.restore_content(&path, id)?;
    if write {
        Document::write_file(&path, &content)?;
        store.push(
            &path,
            &content,
            HistorySource::Manual,
            Some(format!("restore {id}")),
        )?;
        eprintln!("restored {id} -> {}", path.display());
    } else {
        print!("{content}");
        if !content.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn cmd_watch(path: PathBuf) -> Result<()> {
    use moraine_core::FileWatcher;
    let (watcher, rx) = FileWatcher::start()?;
    watcher.watch(&path)?;
    eprintln!("watching {} (Ctrl+C to stop)", path.display());
    for event in rx {
        println!("{:?}\t{}", event.change, event.path.display());
    }
    Ok(())
}
