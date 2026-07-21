//! Project-scoped Codex MCP + hooks configuration (C2).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::json;

/// Configure Codex for a project. Does not write secrets. Backs up existing files.
pub fn setup_codex(project: &Path, dry_run: bool, json: bool) -> Result<()> {
    let project = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let codex_dir = project.join(".codex");
    let cfg_path = codex_dir.join("config.toml");
    let hooks_path = codex_dir.join("hooks.json");
    let cli = which_moraine();

    let mcp_block = format!(
        r#"
# --- Moraine (managed) ---
[mcp_servers.moraine]
command = "{cli}"
args = ["mcp", "--project", "{project}"]
# --- end Moraine ---
"#,
        cli = toml_escape(&cli.display().to_string()),
        project = toml_escape(&project.display().to_string()),
    );

    let hooks = json!({
        "description": "Moraine session capture (desktop may remain closed).",
        "hooks": {
            "SessionStart": [{
                "matcher": "startup|resume",
                "hooks": [{
                    "type": "command",
                    "command": format!("{} hook-codex", cli.display()),
                    "statusMessage": "Moraine session observe"
                }]
            }],
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("{} hook-codex", cli.display()),
                    "statusMessage": "Moraine provisional run"
                }]
            }],
            "Stop": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("{} hook-codex", cli.display()),
                    "statusMessage": "Moraine session stop"
                }]
            }]
        }
    });

    let mut actions = Vec::new();
    if dry_run {
        actions.push(format!("would ensure {}", codex_dir.display()));
        actions.push(format!("would write/merge {}", cfg_path.display()));
        actions.push(format!("would write {}", hooks_path.display()));
    } else {
        fs::create_dir_all(&codex_dir)?;
        // config.toml merge: append managed block if missing
        let mut cfg = if cfg_path.is_file() {
            fs::read_to_string(&cfg_path)?
        } else {
            String::new()
        };
        if !cfg.contains("[mcp_servers.moraine]") {
            if !cfg.is_empty() && !cfg.ends_with('\n') {
                cfg.push('\n');
            }
            cfg.push_str(&mcp_block);
            if cfg_path.is_file() {
                let bak = backup_path(&cfg_path);
                fs::copy(&cfg_path, &bak)?;
                actions.push(format!("backup {}", bak.display()));
            }
            fs::write(&cfg_path, cfg)?;
            actions.push(format!("wrote {}", cfg_path.display()));
        } else {
            actions.push(format!(
                "left existing [mcp_servers.moraine] in {}",
                cfg_path.display()
            ));
        }

        if hooks_path.is_file() {
            let bak = backup_path(&hooks_path);
            fs::copy(&hooks_path, &bak)?;
            actions.push(format!("backup {}", bak.display()));
        }
        fs::write(
            &hooks_path,
            serde_json::to_string_pretty(&hooks).context("hooks json")?,
        )?;
        actions.push(format!("wrote {}", hooks_path.display()));
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "project": project.display().to_string(),
                "cli": cli.display().to_string(),
                "actions": actions,
                "dryRun": dry_run,
            }))?
        );
    } else {
        println!("Codex integration for {}", project.display());
        for a in actions {
            println!("  {a}");
        }
        println!("Start service: moraine service start");
        println!(
            "Doctor: moraine doctor --project {} --integration codex",
            project.display()
        );
    }
    Ok(())
}

pub fn remove_codex(project: &Path, dry_run: bool, json: bool) -> Result<()> {
    let project = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    let cfg_path = project.join(".codex/config.toml");
    let hooks_path = project.join(".codex/hooks.json");
    let mut actions = Vec::new();
    if cfg_path.is_file() {
        let cfg = fs::read_to_string(&cfg_path)?;
        if cfg.contains("[mcp_servers.moraine]") {
            // Strip managed block between markers if present; else leave and warn
            if cfg.contains("# --- Moraine (managed) ---") {
                let stripped = strip_managed_block(&cfg);
                if !dry_run {
                    let bak = backup_path(&cfg_path);
                    fs::copy(&cfg_path, &bak)?;
                    fs::write(&cfg_path, stripped)?;
                    actions.push(format!(
                        "removed managed MCP block; backup {}",
                        bak.display()
                    ));
                } else {
                    actions.push("would remove managed MCP block".into());
                }
            } else {
                actions.push(
                    "config contains moraine MCP but not managed markers; not auto-removed".into(),
                );
            }
        }
    }
    if hooks_path.is_file() {
        let h = fs::read_to_string(&hooks_path)?;
        if h.contains("hook-codex") {
            if !dry_run {
                let bak = backup_path(&hooks_path);
                fs::copy(&hooks_path, &bak)?;
                fs::remove_file(&hooks_path)?;
                actions.push(format!(
                    "removed hooks with hook-codex; backup {}",
                    bak.display()
                ));
            } else {
                actions.push("would remove hooks.json containing hook-codex".into());
            }
        }
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "actions": actions,
                "dryRun": dry_run,
            }))?
        );
    } else {
        for a in actions {
            println!("{a}");
        }
    }
    Ok(())
}

fn which_moraine() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("moraine"))
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn backup_path(p: &Path) -> PathBuf {
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
    p.with_extension(format!("bak.{ts}"))
}

fn strip_managed_block(cfg: &str) -> String {
    let start = "# --- Moraine (managed) ---";
    let end = "# --- end Moraine ---";
    if let (Some(a), Some(b)) = (cfg.find(start), cfg.find(end)) {
        let mut out = String::new();
        out.push_str(&cfg[..a]);
        out.push_str(&cfg[b + end.len()..]);
        return out;
    }
    cfg.to_string()
}
