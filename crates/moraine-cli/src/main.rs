use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use moraine_core::history::HistorySource;
use moraine_core::{room_id_for_path, Document, HistoryStore, MorainePaths};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

const DEFAULT_SERVER_HTTP: &str = "http://127.0.0.1:3099";
const DEFAULT_UI: &str = "http://localhost:1420";

#[derive(Debug, Parser)]
#[command(
    name = "moraine",
    version,
    about = "Moraine: local-first collaborative Markdown editor (CLI)",
    long_about = "Work with plain .md files from the terminal. Open in the desktop app, \
                  read/write content, share a collab room, and inspect local edit history."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Info,

    Cat {
        path: PathBuf,
    },

    Write {
        path: PathBuf,
        #[arg(long, short)]
        content: Option<String>,
        #[arg(long)]
        history: bool,
    },

    Edit {
        path: PathBuf,
        #[arg(long, default_value_t = true)]
        create: bool,
        /// Print a share URL (and ensure the relay) before opening.
        #[arg(long)]
        share: bool,
    },

    /// Print a collab room URL for a Markdown file (starts the relay if needed).
    Share {
        path: PathBuf,
        /// UI base URL printed for browsers / vite dev
        #[arg(long, env = "MORAINE_UI_URL", default_value = DEFAULT_UI)]
        ui: String,
        /// Relay HTTP base (health check + start target)
        #[arg(long, env = "MORAINE_SERVER_URL", default_value = DEFAULT_SERVER_HTTP)]
        server: String,
        /// Do not try to spawn moraine-server if health fails
        #[arg(long)]
        no_start: bool,
        /// After printing, watch the file for changes (Ctrl+C to stop)
        #[arg(long)]
        watch: bool,
        /// Also open the file in the desktop app if available
        #[arg(long)]
        open: bool,
        /// Emit JSON only (url, room, ws)
        #[arg(long)]
        json: bool,
    },

    History {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, short = 'n', default_value_t = 20)]
        limit: usize,
    },

    Restore {
        path: PathBuf,
        entry_id: String,
        #[arg(long)]
        write: bool,
    },

    Watch {
        path: PathBuf,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Commands::Info => cmd_info(),
        Commands::Cat { path } => cmd_cat(path),
        Commands::Write {
            path,
            content,
            history,
        } => cmd_write(path, content, history),
        Commands::Edit {
            path,
            create,
            share,
        } => cmd_edit(path, create, share),
        Commands::Share {
            path,
            ui,
            server,
            no_start,
            watch,
            open,
            json,
        } => cmd_share(path, ui, server, no_start, watch, open, json),
        Commands::History { path, json, limit } => cmd_history(path, json, limit),
        Commands::Restore {
            path,
            entry_id,
            write,
        } => cmd_restore(path, entry_id, write),
        Commands::Watch { path } => cmd_watch(path),
    }
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

fn cmd_info() -> Result<()> {
    println!("moraine {}", env!("CARGO_PKG_VERSION"));
    println!("core: local-first Markdown (Phase 0–1 MVP)");
    if let Ok(paths) = MorainePaths::default_ensure() {
        println!("data dir:    {}", paths.data_dir.display());
        println!("history dir: {}", paths.history_dir.display());
        println!("config dir:  {}", paths.config_dir.display());
    }
    Ok(())
}

fn cmd_cat(path: PathBuf) -> Result<()> {
    let content = Document::read_file(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
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
        let store = HistoryStore::new(paths);
        store.push(
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
        cmd_share(
            path.clone(),
            DEFAULT_UI.into(),
            DEFAULT_SERVER_HTTP.into(),
            false,
            false,
            false,
            false,
        )?;
    }

    if try_launch_desktop(&path)? {
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
    no_start: bool,
    watch: bool,
    open: bool,
    json: bool,
) -> Result<()> {
    if !path.exists() {
        bail!("file does not exist: {}", path.display());
    }
    let abs = std::fs::canonicalize(&path)
        .with_context(|| format!("canonicalize {}", path.display()))?;
    let room = room_id_for_path(&abs);
    let server = server.trim_end_matches('/').to_string();
    let ui = ui.trim_end_matches('/').to_string();

    ensure_relay(&server, !no_start)?;

    let ws = http_to_ws(&server);
    let share_url = format!("{ui}/?room={room}");
    let ws_url = format!("{ws}/ws/{room}");

    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": abs,
                "room": room,
                "url": share_url,
                "ws": ws_url,
                "server": server,
            })
        );
    } else {
        println!("{share_url}");
        eprintln!("room   {room}");
        eprintln!("file   {}", abs.display());
        eprintln!("ws     {ws_url}");
        eprintln!("open that URL in a browser (npm run dev) or second client with the same room");
    }

    if open {
        let _ = try_launch_desktop(&abs)?;
    }

    if watch {
        cmd_watch(abs)?;
    }
    Ok(())
}

fn ensure_relay(server_http: &str, may_start: bool) -> Result<()> {
    if health_ok(server_http) {
        return Ok(());
    }
    if !may_start {
        bail!(
            "relay not reachable at {server_http}/health\n\
             start it: cargo run -p moraine-server\n\
             or:       npm run server\n\
             or:       docker compose up --build"
        );
    }

    eprintln!("relay not up; trying to start moraine-server…");
    if !try_start_server(server_http)? {
        bail!(
            "could not start moraine-server and {server_http}/health is down\n\
             start it yourself:\n\
               cargo run -p moraine-server\n\
               npm run server\n\
               docker compose up --build"
        );
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if health_ok(server_http) {
            eprintln!("relay ready at {server_http}");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(150));
    }
    bail!("started moraine-server but health check still failing at {server_http}/health");
}

fn health_ok(server_http: &str) -> bool {
    let Some((host, port, _)) = parse_http_base(server_http) else {
        return false;
    };
    let Ok(mut stream) =
        TcpStream::connect_timeout(&socket_addr(&host, port), Duration::from_millis(400))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let req = format!("GET /health HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
    buf.contains("\"ok\"") || buf.contains("moraine-server")
}

fn parse_http_base(url: &str) -> Option<(String, u16, String)> {
    let url = url.trim();
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let (hostport, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, String::new()),
    };
    let (host, port) = if let Some((h, p)) = hostport.split_once(':') {
        (h.to_string(), p.parse().ok()?)
    } else {
        let port = if url.starts_with("https://") { 443 } else { 80 };
        (hostport.to_string(), port)
    };
    Some((host, port, path))
}

fn socket_addr(host: &str, port: u16) -> std::net::SocketAddr {
    use std::net::{SocketAddr, ToSocketAddrs};
    format!("{host}:{port}")
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], port)))
}

fn http_to_ws(server_http: &str) -> String {
    if let Some(rest) = server_http.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = server_http.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("ws://{server_http}")
    }
}

fn try_start_server(server_http: &str) -> Result<bool> {
    let bind = parse_http_base(server_http)
        .map(|(h, p, _)| format!("{h}:{p}"))
        .unwrap_or_else(|| "127.0.0.1:3099".into());

    let mut bins: Vec<PathBuf> = Vec::new();
    if which_exists("moraine-server") {
        bins.push(PathBuf::from("moraine-server"));
    }
    for rel in [
        "target/debug/moraine-server",
        "target/release/moraine-server",
        "../target/debug/moraine-server",
        "../target/release/moraine-server",
    ] {
        let p = PathBuf::from(rel);
        if p.is_file() {
            bins.push(p);
        }
    }

    for bin in bins {
        let mut cmd = Command::new(&bin);
        cmd.arg("--bind")
            .arg(&bind)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.spawn() {
            Ok(_) => {
                eprintln!("spawned {}", bin.display());
                return Ok(true);
            }
            Err(e) => eprintln!("spawn {} failed: {e}", bin.display()),
        }
    }
    Ok(false)
}

fn try_launch_desktop(path: &PathBuf) -> Result<bool> {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
    let mut candidates: Vec<PathBuf> = ["moraine-app", "moraine-desktop"]
        .into_iter()
        .filter(|n| which_exists(n))
        .map(PathBuf::from)
        .collect();

    // Dev builds from workspace root or crates/
    for rel in [
        "target/debug/moraine-app",
        "target/release/moraine-app",
        "../target/debug/moraine-app",
        "../target/release/moraine-app",
    ] {
        let p = PathBuf::from(rel);
        if p.is_file() {
            candidates.push(p);
        }
    }

    for bin in candidates {
        let mut cmd = Command::new(&bin);
        cmd.arg(&abs)
            .env("MORAINE_OPEN", &abs)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if cmd.spawn().is_ok() {
            eprintln!("opened in {}: {}", bin.display(), abs.display());
            return Ok(true);
        }
    }
    Ok(false)
}

fn which_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let p = dir.join(name);
                p.is_file()
            })
        })
        .unwrap_or(false)
}

fn cmd_history(path: PathBuf, json: bool, limit: usize) -> Result<()> {
    let paths = MorainePaths::default_ensure()?;
    let store = HistoryStore::new(paths);
    let mut entries = store.list_meta(&path)?;
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
        eprintln!("restored {id} → {}", path.display());
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
