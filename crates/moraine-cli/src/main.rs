//! CLI entry. Thin clap surface; logic lives in moraine-core + relay helpers.

mod relay;

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use moraine_core::history::HistorySource;
use moraine_core::{
    share_links, Document, HistoryStore, MorainePaths, DEFAULT_RELAY_HTTP, DEFAULT_UI,
};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use relay::{ensure_relay, launch_desktop};

#[derive(Debug, Parser)]
#[command(
    name = "moraine",
    version,
    about = "Moraine: local-first collaborative Markdown editor (CLI)"
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
        /// Print share URL (and ensure the relay) before opening.
        #[arg(long)]
        share: bool,
    },

    /// Print a collab join URL; starts moraine-server if needed.
    Share {
        path: PathBuf,
        #[arg(long, env = "MORAINE_UI_URL", default_value = DEFAULT_UI)]
        ui: String,
        #[arg(long, env = "MORAINE_SERVER_URL", default_value = DEFAULT_RELAY_HTTP)]
        server: String,
        #[arg(long)]
        no_start: bool,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        open: bool,
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
    if let Ok(paths) = MorainePaths::default_ensure() {
        println!("data dir:    {}", paths.data_dir.display());
        println!("history dir: {}", paths.history_dir.display());
        println!("config dir:  {}", paths.config_dir.display());
    }
    Ok(())
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
        cmd_share(
            path.clone(),
            DEFAULT_UI.into(),
            DEFAULT_RELAY_HTTP.into(),
            false,
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
    no_start: bool,
    watch: bool,
    open: bool,
    json: bool,
) -> Result<()> {
    if !path.exists() {
        bail!("file does not exist: {}", path.display());
    }
    ensure_relay(&server, !no_start)?;
    let links = share_links(&path, &ui, &server);

    if json {
        println!("{}", serde_json::to_string_pretty(&links)?);
    } else {
        println!("{}", links.url);
        eprintln!("room   {}", links.room);
        eprintln!("file   {}", links.path);
        eprintln!("ws     {}", links.ws);
        eprintln!("open the URL in a browser (npm run dev) or another client");
    }

    if open {
        launch_desktop(path.as_path())?;
    }
    if watch {
        cmd_watch(path)?;
    }
    Ok(())
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
