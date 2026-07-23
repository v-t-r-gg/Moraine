//! Project-scoped Codex MCP + hooks configuration (C2).
//!
//! Merges Moraine-managed entries without wiping unrelated Codex config or hooks.
//! Managed markers let `--remove` delete only Moraine-owned content.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use moraine_core::resolve_existing_project;
use serde_json::{json, Value};

const MANAGED_TOML_START: &str = "# --- Moraine (managed) ---";
const MANAGED_TOML_END: &str = "# --- end Moraine ---";
const MORAINE_HOOK_MARKER: &str = "moraine-managed";

/// Configure Codex for an initialized Moraine project.
pub fn setup_codex(project: &Path, dry_run: bool, json: bool) -> Result<()> {
    let report = apply_codex(project, dry_run, false)?;
    emit_report(json, &report)
}

/// Validate configuration without writing.
pub fn check_codex(project: &Path, json: bool) -> Result<()> {
    let report = apply_codex(project, true, true)?;
    emit_report(json, &report)
}

/// Remove only Moraine-managed MCP block and managed hook handlers.
pub fn remove_codex(project: &Path, dry_run: bool, json: bool) -> Result<()> {
    let project = canonicalize_project(project)?;
    let cfg_path = project.join(".codex/config.toml");
    let hooks_path = project.join(".codex/hooks.json");
    let mut actions = Vec::new();
    let mut warnings = Vec::new();

    if cfg_path.is_file() {
        let cfg = fs::read_to_string(&cfg_path)?;
        if cfg.contains("[mcp_servers.moraine]") {
            if cfg.contains(MANAGED_TOML_START) {
                let stripped = strip_managed_block(&cfg);
                if dry_run {
                    actions.push("would remove managed MCP block".into());
                } else {
                    let bak = backup_path(&cfg_path);
                    fs::copy(&cfg_path, &bak)?;
                    atomic_write(&cfg_path, stripped.as_bytes())?;
                    actions.push(format!(
                        "removed managed MCP block; backup {}",
                        bak.display()
                    ));
                }
            } else {
                warnings.push(
                    "config contains [mcp_servers.moraine] without managed markers; not auto-removed"
                        .into(),
                );
            }
        }
    }

    if hooks_path.is_file() {
        let raw = fs::read_to_string(&hooks_path)?;
        match serde_json::from_str::<Value>(&raw) {
            Ok(mut doc) => {
                let removed = strip_managed_hooks(&mut doc);
                if removed > 0 {
                    if dry_run {
                        actions.push(format!(
                            "would remove {removed} Moraine-managed hook handler(s)"
                        ));
                    } else {
                        let bak = backup_path(&hooks_path);
                        fs::copy(&hooks_path, &bak)?;
                        atomic_write(&hooks_path, serde_json::to_string_pretty(&doc)?.as_bytes())?;
                        actions.push(format!(
                            "removed {removed} Moraine-managed hook handler(s); backup {}",
                            bak.display()
                        ));
                    }
                } else if raw.contains("hook-codex") {
                    warnings.push(
                        "hooks mention hook-codex but no managed markers; left untouched".into(),
                    );
                }
            }
            Err(e) => warnings.push(format!("hooks.json parse error: {e}; left untouched")),
        }
    }

    let body = json!({
        "ok": warnings.is_empty(),
        "action": "remove",
        "project": project.display().to_string(),
        "actions": actions,
        "warnings": warnings,
        "dryRun": dry_run,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        for a in actions {
            println!("{a}");
        }
        for w in warnings {
            eprintln!("warning: {w}");
        }
    }
    Ok(())
}

fn apply_codex(project: &Path, dry_run: bool, check_only: bool) -> Result<Value> {
    let project = canonicalize_project(project)?;
    // Require initialized Moraine project (§16).
    let resolved = resolve_existing_project(Some(&project)).with_context(|| {
        format!(
            "Moraine project not initialized at {}; run: moraine project init {}",
            project.display(),
            project.display()
        )
    })?;

    let codex_dir = project.join(".codex");
    let cfg_path = codex_dir.join("config.toml");
    let hooks_path = codex_dir.join("hooks.json");
    // Prefer suite-owned absolute path so capture does not depend on shell PATH.
    let cli = which_moraine();
    let cli_s = cli.display().to_string();
    let project_s = project.display().to_string();

    let mcp_block = format!(
        r#"
{MANAGED_TOML_START}
[mcp_servers.moraine]
command = "{cli}"
args = ["mcp", "--project", "{project}"]
{MANAGED_TOML_END}
"#,
        cli = toml_escape(&cli_s),
        project = toml_escape(&project_s),
    );

    let mut actions: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut checks: Vec<Value> = Vec::new();

    // Codex capability detection
    let codex_bin = which_codex();
    let codex_version = codex_bin.as_ref().and_then(|p| {
        Command::new(p)
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    });
    if codex_bin.is_none() {
        warnings.push(
            "codex executable not found on PATH; MCP/hooks config will still be written".into(),
        );
    }

    // MCP config merge (rewrite managed block when present so CLI path stays absolute/current)
    let existing_cfg = if cfg_path.is_file() {
        fs::read_to_string(&cfg_path)?
    } else {
        String::new()
    };
    let new_cfg = if existing_cfg.contains(MANAGED_TOML_START) {
        let stripped = strip_managed_block(&existing_cfg);
        let mut out = stripped;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(mcp_block.trim_start());
        if !out.ends_with('\n') {
            out.push('\n');
        }
        actions.push("refresh managed [mcp_servers.moraine] block".into());
        out
    } else if existing_cfg.contains("[mcp_servers.moraine]") {
        warnings.push(
            "found unmanaged [mcp_servers.moraine]; leaving it (use managed markers or --remove after manual cleanup)"
                .into(),
        );
        existing_cfg.clone()
    } else {
        let mut out = existing_cfg.clone();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(mcp_block.trim_start());
        if !out.ends_with('\n') {
            out.push('\n');
        }
        actions.push("add managed [mcp_servers.moraine] block".into());
        out
    };

    // Hooks merge — refuse malformed existing files (do not wipe user data).
    let mut hooks_doc = if hooks_path.is_file() {
        let raw = fs::read_to_string(&hooks_path)?;
        match serde_json::from_str::<Value>(&raw) {
            Ok(v) => v,
            Err(e) => {
                bail!(
                    "refusing to modify malformed hooks.json at {}: {e}; fix or remove the file, then re-run",
                    hooks_path.display()
                );
            }
        }
    } else {
        json!({ "hooks": {} })
    };
    if !hooks_doc
        .get("hooks")
        .map(|h| h.is_object())
        .unwrap_or(false)
    {
        hooks_doc["hooks"] = json!({});
    }
    let hook_cmd = format!("{cli_s} hook-codex");
    let inserted = ensure_managed_hooks(&mut hooks_doc, &hook_cmd);
    if inserted > 0 {
        actions.push(format!("merge {inserted} Moraine-managed hook handler(s)"));
    } else {
        actions.push("hooks already contain managed Moraine handlers".into());
    }

    checks.push(json!({
        "id": "project.initialized",
        "status": "pass",
        "projectId": resolved.project_id.to_string(),
    }));
    checks.push(json!({
        "id": "cli.absolute",
        "status": if cli.is_absolute() { "pass" } else { "warn" },
        "path": cli_s,
    }));

    if dry_run || check_only {
        actions.insert(0, format!("would ensure {}", codex_dir.display()));
    } else {
        fs::create_dir_all(&codex_dir)?;
        if cfg_path.is_file() && new_cfg != existing_cfg {
            let bak = backup_path(&cfg_path);
            fs::copy(&cfg_path, &bak)?;
            actions.push(format!("backup {}", bak.display()));
        }
        if new_cfg != existing_cfg {
            atomic_write(&cfg_path, new_cfg.as_bytes())?;
            actions.push(format!("wrote {}", cfg_path.display()));
        } else {
            actions.push(format!("config unchanged {}", cfg_path.display()));
        }

        if hooks_path.is_file() {
            let bak = backup_path(&hooks_path);
            fs::copy(&hooks_path, &bak)?;
            actions.push(format!("backup {}", bak.display()));
        }
        atomic_write(
            &hooks_path,
            serde_json::to_string_pretty(&hooks_doc)?.as_bytes(),
        )?;
        actions.push(format!("wrote {}", hooks_path.display()));
    }

    Ok(json!({
        // Missing codex binary is a warning, not a hard failure of config write.
        "ok": true,
        "action": if check_only { "check" } else { "setup" },
        "project": project_s,
        "projectId": resolved.project_id.to_string(),
        "cli": cli_s,
        "codex": {
            "path": codex_bin.map(|p| p.display().to_string()),
            "version": codex_version,
        },
        "actions": actions,
        "warnings": warnings,
        "checks": checks,
        "dryRun": dry_run || check_only,
        "configPath": cfg_path.display().to_string(),
        "hooksPath": hooks_path.display().to_string(),
    }))
}

fn ensure_managed_hooks(doc: &mut Value, hook_cmd: &str) -> usize {
    // Remove prior managed handlers first (idempotent refresh of command path).
    strip_managed_hooks(doc);
    if !doc.get("hooks").map(|h| h.is_object()).unwrap_or(false) {
        doc["hooks"] = json!({});
    }
    let hooks = doc
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
        .expect("hooks object");

    let specs: &[(&str, Option<&str>, &str)] = &[
        (
            "SessionStart",
            Some("startup|resume"),
            "Moraine session observe",
        ),
        ("UserPromptSubmit", None, "Moraine provisional run"),
        ("Stop", None, "Moraine session stop"),
    ];

    let mut added = 0usize;
    for (event, matcher, status) in specs {
        let mut entry = json!({
            "hooks": [{
                "type": "command",
                "command": hook_cmd,
                "statusMessage": status,
                MORAINE_HOOK_MARKER: true
            }]
        });
        if let Some(m) = matcher {
            entry
                .as_object_mut()
                .unwrap()
                .insert("matcher".into(), json!(m));
        }
        let arr = hooks
            .entry((*event).to_string())
            .or_insert_with(|| json!([]));
        if !arr.is_array() {
            *arr = json!([]);
        }
        arr.as_array_mut().unwrap().push(entry);
        added += 1;
    }
    added
}

/// Remove handlers that carry the Moraine managed marker (or sole managed shape).
fn strip_managed_hooks(doc: &mut Value) -> usize {
    let Some(hooks) = doc.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
        return 0;
    };
    let mut removed = 0usize;
    for (_event, val) in hooks.iter_mut() {
        let Some(arr) = val.as_array_mut() else {
            continue;
        };
        let before = arr.len();
        arr.retain(|item| !is_managed_hook_group(item));
        removed += before - arr.len();
    }
    // Drop empty event arrays for cleanliness
    hooks.retain(|_, v| v.as_array().map(|a| !a.is_empty()).unwrap_or(true));
    removed
}

fn is_managed_hook_group(item: &Value) -> bool {
    let Some(inner) = item.get("hooks").and_then(|h| h.as_array()) else {
        return false;
    };
    // Managed if every command hook is marked, or any is marked (group is Moraine-owned).
    inner.iter().any(|h| {
        h.get(MORAINE_HOOK_MARKER)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || h.get("command")
                .and_then(|c| c.as_str())
                .map(|c| c.contains("hook-codex") && c.contains("moraine"))
                .unwrap_or(false)
                && h.get("statusMessage")
                    .and_then(|s| s.as_str())
                    .map(|s| s.starts_with("Moraine "))
                    .unwrap_or(false)
    })
}

fn emit_report(json_mode: bool, report: &Value) -> Result<()> {
    if json_mode {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "Codex integration for {}",
            report
                .get("project")
                .and_then(|p| p.as_str())
                .unwrap_or("?")
        );
        if let Some(acts) = report.get("actions").and_then(|a| a.as_array()) {
            for a in acts {
                if let Some(s) = a.as_str() {
                    println!("  {s}");
                }
            }
        }
        if let Some(ws) = report.get("warnings").and_then(|a| a.as_array()) {
            for w in ws {
                if let Some(s) = w.as_str() {
                    eprintln!("warning: {s}");
                }
            }
        }
        println!("Doctor: moraine doctor --project <path> --integration codex");
    }
    Ok(())
}

fn canonicalize_project(project: &Path) -> Result<PathBuf> {
    let p = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    if !p.is_dir() {
        bail!("project path is not a directory: {}", p.display());
    }
    Ok(p)
}

fn which_moraine() -> PathBuf {
    // Suite absolute path first (product contract: never depend on PATH resolution).
    let suite = moraine_provision::SuitePaths::discover();
    let abs = suite.absolute_cli();
    if abs.is_file() {
        return abs;
    }
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("moraine"))
}

fn which_codex() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join("codex");
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn backup_path(p: &Path) -> PathBuf {
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
    p.with_extension(format!("bak.{ts}"))
}

fn strip_managed_block(cfg: &str) -> String {
    if let (Some(a), Some(b)) = (cfg.find(MANAGED_TOML_START), cfg.find(MANAGED_TOML_END)) {
        let mut out = String::new();
        out.push_str(&cfg[..a]);
        out.push_str(&cfg[b + MANAGED_TOML_END.len()..]);
        // collapse extra blank lines
        while out.contains("\n\n\n") {
            out = out.replace("\n\n\n", "\n\n");
        }
        return out.trim_start_matches('\n').to_string();
    }
    cfg.to_string()
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("moraine")
    ));
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        f.write_all(data)?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_hooks_preserves_unrelated() {
        let mut doc = json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "echo user" }]
                }]
            }
        });
        let n = ensure_managed_hooks(&mut doc, "/tmp/moraine hook-codex");
        assert_eq!(n, 3);
        let ups = doc["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert!(ups.len() >= 2, "user handler must remain: {ups:?}");
        assert!(ups.iter().any(|g| {
            g["hooks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["command"] == "echo user")
        }));
        assert!(ups.iter().any(is_managed_hook_group));
    }

    #[test]
    fn strip_managed_leaves_user_hooks() {
        let mut doc = json!({
            "hooks": {
                "Stop": [
                    { "hooks": [{ "type": "command", "command": "echo keep" }] },
                    {
                        "hooks": [{
                            "type": "command",
                            "command": "/x/moraine hook-codex",
                            "statusMessage": "Moraine session stop",
                            "moraine-managed": true
                        }]
                    }
                ]
            }
        });
        let n = strip_managed_hooks(&mut doc);
        assert_eq!(n, 1);
        let stop = doc["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert_eq!(stop[0]["hooks"][0]["command"], "echo keep");
    }

    #[test]
    fn strip_managed_toml_block() {
        let cfg = "foo = 1\n# --- Moraine (managed) ---\n[mcp_servers.moraine]\ncommand = \"x\"\n# --- end Moraine ---\nbar = 2\n";
        let out = strip_managed_block(cfg);
        assert!(out.contains("foo = 1"));
        assert!(out.contains("bar = 2"));
        assert!(!out.contains("mcp_servers.moraine"));
    }
}
