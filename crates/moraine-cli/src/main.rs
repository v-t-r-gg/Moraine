//! CLI for agent run records and review helpers. Fail-fast share unless `--start`.

mod relay;

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use moraine_core::history::HistorySource;
use moraine_core::{
    content_hash, ensure_run_meta, moraine_sidecar_path, review_snapshot, room_id_for_path,
    share_links, write_run_meta, AnnotationKind, DecisionKind, Document, HistoryStore,
    MorainePaths, ReviewStateKind, DEFAULT_RELAY_HTTP, DEFAULT_UI,
};
use serde::Serialize;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use relay::{health_ok, launch_desktop, try_spawn_server};

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
                  Exit codes: 0 ok, 1 error, 2 not found, 3 relay down.\n\
                  Prefer --json on share/status/info/join when calling from scripts."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
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

    /// Record a run-level review decision bound to the current Markdown hash
    #[command(after_help = "Examples:\n  \
        moraine decide run.md --decision approved --reviewer Alice --json\n  \
        moraine decide run.md --decision changes_requested --reviewer Bob --reason 'fix outcomes'")]
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
        #[arg(long)]
        json: bool,
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
        Commands::Decide {
            path,
            decision,
            reviewer,
            reason,
            json,
        } => cmd_decide(path, decision, reviewer, reason, json),
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

fn cmd_info(json: bool) -> Result<i32> {
    let paths = MorainePaths::default_ensure().ok();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "name": "moraine",
                "version": env!("CARGO_PKG_VERSION"),
                "dataDir": paths.as_ref().map(|p| p.data_dir.display().to_string()),
                "historyDir": paths.as_ref().map(|p| p.history_dir.display().to_string()),
                "configDir": paths.as_ref().map(|p| p.config_dir.display().to_string()),
            }))?
        );
    } else {
        println!("moraine {}", env!("CARGO_PKG_VERSION"));
        if let Some(p) = paths {
            println!("data dir:    {}", p.data_dir.display());
            println!("history dir: {}", p.history_dir.display());
            println!("config dir:  {}", p.config_dir.display());
        }
    }
    Ok(EXIT_OK)
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
    id: String,
    content_hash: String,
    review_state: String,
    decision_current: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewStatusJson {
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
            let markdown = match Document::read_file(&abs) {
                Ok(s) => s,
                Err(e) => return Ok(emit_err(json, EXIT_ERR, &e.to_string())),
            };
            let meta = match ensure_run_meta(&abs) {
                Ok(m) => m,
                Err(e) => return Ok(emit_err(json, EXIT_ERR, &e.to_string())),
            };
            let snap = review_snapshot(&meta, &markdown);
            let room = room_id_for_path(&abs);
            let side = moraine_sidecar_path(&abs);
            out.path = Some(abs.display().to_string());
            out.room = Some(room.clone());
            out.exists = Some(true);
            out.join_url = Some(format!("{}/?room={room}", ui.trim_end_matches('/')));
            out.sidecar = Some(side.display().to_string());
            out.sidecar_exists = Some(side.exists());
            out.annotations = Some(annotation_counts_from(&meta.comments));
            out.run = Some(RunStatusJson {
                id: snap.run_id.to_string(),
                content_hash: snap.content_hash.clone(),
                review_state: review_state_str(snap.state).into(),
                decision_current: snap.decision_current,
            });
            out.review = Some(ReviewStatusJson {
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
            println!("run    {}", run.id);
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

fn cmd_decide(
    path: PathBuf,
    decision: String,
    reviewer: String,
    reason: Option<String>,
    json: bool,
) -> Result<i32> {
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
        Err(e) => return Ok(emit_err(json, EXIT_ERR, &e.to_string())),
    };
    let hash = content_hash(&markdown);
    let mut meta = match ensure_run_meta(&abs) {
        Ok(m) => m,
        Err(e) => return Ok(emit_err(json, EXIT_ERR, &e.to_string())),
    };
    let recorded = meta
        .append_decision(kind, reviewer.trim(), reason, hash)
        .clone();
    if let Err(e) = write_run_meta(&abs, &meta) {
        return Ok(emit_err(json, EXIT_ERR, &e.to_string()));
    }
    let snap = review_snapshot(&meta, &markdown);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "run": {
                    "id": snap.run_id.to_string(),
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
