//! Codex agent adapter — structured config mutation with write-ahead snapshots.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use moraine_core::resolve_existing_project;
use serde_json::{json, Value};

use super::{
    AgentAdapter, AgentDetection, IntegrationPlan, IntegrationReceipt, IntegrationState,
    IntegrationVerification,
};
use crate::error::{ProvisionError, Result};
use crate::snapshot::{atomic_write_durable, durable_backup, snapshot_absent};
use crate::types::{AgentKind, FileSnapshot};

const MANAGED_TOML_START: &str = "# --- Moraine (managed) ---";
const MANAGED_TOML_END: &str = "# --- end Moraine ---";
const MORAINE_HOOK_MARKER: &str = "moraine-managed";

pub struct CodexAdapter;

impl CodexAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn detect(&self) -> Result<AgentDetection> {
        let exe = which_codex();
        let version = exe.as_ref().and_then(|p| {
            Command::new(p)
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        });
        let detected = exe.is_some();
        Ok(AgentDetection {
            kind: AgentKind::Codex,
            detected,
            executable: exe.map(|p| p.display().to_string()),
            version,
            status: if detected {
                "readyToConnect".into()
            } else {
                "notFound".into()
            },
            status_message: if detected {
                "Ready to connect".into()
            } else {
                "Codex was not found on this machine".into()
            },
        })
    }

    fn inspect(&self, project: &Path) -> Result<IntegrationState> {
        let cfg_path = project.join(".codex/config.toml");
        let hooks_path = project.join(".codex/hooks.json");
        let mut details = Vec::new();
        let mut absolute_cli = None;
        let mut mcp_present = false;
        let mut hooks_present = false;
        let mut needs_repair = false;

        if cfg_path.is_file() {
            let cfg = fs::read_to_string(&cfg_path)?;
            if cfg.contains("[mcp_servers.moraine]") {
                mcp_present = true;
                details.push("Codex connection is configured".into());
                if let Some(cmd) = extract_command_from_toml(&cfg) {
                    absolute_cli = Some(cmd.clone());
                    if !Path::new(&cmd).is_absolute() {
                        needs_repair = true;
                        details.push("CLI path is not absolute — repair recommended".into());
                    } else if !Path::new(&cmd).is_file() {
                        needs_repair = true;
                        details.push("Configured CLI path is missing on disk".into());
                    }
                }
                if !cfg.contains(MANAGED_TOML_START) {
                    needs_repair = true;
                    details.push("Configuration is present but not Moraine-managed".into());
                }
            }
        }
        if hooks_path.is_file() {
            let raw = fs::read_to_string(&hooks_path)?;
            if raw.contains("hook-codex") || raw.contains(MORAINE_HOOK_MARKER) {
                hooks_present = true;
                details.push("Capture hooks are present".into());
            }
        }
        // Fully configured only when BOTH MCP and hooks are present.
        let configured = mcp_present && hooks_present;
        if mcp_present && !hooks_present {
            needs_repair = true;
            details.push("Connection present but capture hooks missing".into());
        }
        if hooks_present && !mcp_present {
            needs_repair = true;
            details.push("Capture hooks present but connection missing".into());
        }
        if !configured {
            details.push("Codex is not fully connected for this project".into());
        }

        Ok(IntegrationState {
            configured,
            mcp_present,
            hooks_present,
            absolute_cli,
            config_path: cfg_path.is_file().then(|| cfg_path.display().to_string()),
            details,
            needs_repair,
        })
    }

    fn plan_install(&self, project: &Path, absolute_cli: &Path) -> Result<IntegrationPlan> {
        if !absolute_cli.is_absolute() {
            return Err(ProvisionError::msg(format!(
                "CLI path must be absolute, got {}",
                absolute_cli.display()
            )));
        }
        let project_s = project.display().to_string();
        let cli_s = absolute_cli.display().to_string();
        let cfg = project.join(".codex/config.toml");
        let hooks = project.join(".codex/hooks.json");
        Ok(IntegrationPlan {
            kind: AgentKind::Codex,
            project: project_s,
            absolute_cli: cli_s,
            actions: vec![
                "write managed Codex connection block".into(),
                "merge capture lifecycle handlers".into(),
            ],
            product_labels: vec![
                "Connect Codex for this project".into(),
                "Keep records next to the project".into(),
            ],
            files_to_touch: vec![
                cfg.display().to_string(),
                hooks.display().to_string(),
            ],
        })
    }

    fn apply(
        &self,
        plan: &IntegrationPlan,
        recorder: &mut dyn super::BackupRecorder,
    ) -> Result<IntegrationReceipt> {
        let project = PathBuf::from(&plan.project);
        let _resolved = resolve_existing_project(Some(&project)).map_err(|e| {
            ProvisionError::msg(format!(
                "project not initialized at {}: {e}",
                project.display()
            ))
        })?;

        let codex_dir = project.join(".codex");
        let cfg_path = codex_dir.join("config.toml");
        let hooks_path = codex_dir.join("hooks.json");
        let cli_s = &plan.absolute_cli;
        let project_s = project.display().to_string();

        let mcp_block = format!(
            r#"
{MANAGED_TOML_START}
[mcp_servers.moraine]
command = "{cli}"
args = ["mcp", "--project", "{project}"]
{MANAGED_TOML_END}
"#,
            cli = toml_escape(cli_s),
            project = toml_escape(&project_s),
        );

        fs::create_dir_all(&codex_dir)?;
        let mut local_snaps = Vec::new();
        let mut actions = Vec::new();

        // Validate hooks *before* mutating config so we fail without partial MCP write when possible.
        let hooks_raw = if hooks_path.is_file() {
            Some(fs::read_to_string(&hooks_path)?)
        } else {
            None
        };
        let mut hooks_doc = if let Some(ref raw) = hooks_raw {
            match serde_json::from_str::<Value>(raw) {
                Ok(v) => v,
                Err(e) => {
                    return Err(ProvisionError::msg(format!(
                        "refusing to modify malformed hooks at {}: {e}",
                        hooks_path.display()
                    )));
                }
            }
        } else {
            json!({ "hooks": {} })
        };

        let existing_cfg = if cfg_path.is_file() {
            fs::read_to_string(&cfg_path)?
        } else {
            String::new()
        };
        let new_cfg = merge_mcp_block(&existing_cfg, &mcp_block);

        // Write-ahead: snapshot BEFORE each mutation (Existing backup or Absent marker).
        if new_cfg != existing_cfg {
            let snap = if cfg_path.is_file() {
                durable_backup(&cfg_path)?
            } else {
                snapshot_absent(&cfg_path)
            };
            recorder.record_snapshot(snap.clone())?;
            local_snaps.push(snap);
            atomic_write_durable(&cfg_path, new_cfg.as_bytes())?;
            actions.push(format!("wrote {}", cfg_path.display()));
        } else {
            actions.push(format!("config unchanged {}", cfg_path.display()));
        }

        let hook_cmd = format!("{cli_s} hook-codex");
        ensure_managed_hooks(&mut hooks_doc, &hook_cmd);
        let hooks_bytes = serde_json::to_string_pretty(&hooks_doc)?;
        let snap = if hooks_path.is_file() {
            durable_backup(&hooks_path)?
        } else {
            snapshot_absent(&hooks_path)
        };
        recorder.record_snapshot(snap.clone())?;
        local_snaps.push(snap);
        atomic_write_durable(&hooks_path, hooks_bytes.as_bytes())?;
        actions.push(format!("wrote {}", hooks_path.display()));

        Ok(IntegrationReceipt {
            kind: AgentKind::Codex,
            project: project_s,
            absolute_cli: cli_s.clone(),
            actions,
            snapshots: local_snaps,
            config_path: Some(cfg_path.display().to_string()),
            hooks_path: Some(hooks_path.display().to_string()),
        })
    }

    fn verify(&self, project: &Path, expected_cli: &Path) -> Result<IntegrationVerification> {
        let state = self.inspect(project)?;
        let mut messages = state.details.clone();
        let absolute_cli_ok = state
            .absolute_cli
            .as_ref()
            .map(|c| {
                Path::new(c).is_absolute()
                    && (c == &expected_cli.display().to_string() || Path::new(c).is_file())
            })
            .unwrap_or(false);
        if !absolute_cli_ok {
            messages.push("Configured CLI path is missing or not absolute".into());
        }
        let ok = state.configured && absolute_cli_ok && !state.needs_repair;
        Ok(IntegrationVerification {
            ok,
            absolute_cli_ok,
            config_present: state.configured,
            mcp_present: state.mcp_present,
            hooks_present: state.hooks_present,
            messages,
        })
    }

    fn remove(&self, project: &Path) -> Result<Vec<FileSnapshot>> {
        let cfg_path = project.join(".codex/config.toml");
        let hooks_path = project.join(".codex/hooks.json");
        let mut snaps = Vec::new();

        if cfg_path.is_file() {
            let cfg = fs::read_to_string(&cfg_path)?;
            if cfg.contains(MANAGED_TOML_START) {
                snaps.push(durable_backup(&cfg_path)?);
                let stripped = strip_managed_block(&cfg);
                atomic_write_durable(&cfg_path, stripped.as_bytes())?;
            }
        }
        if hooks_path.is_file() {
            let raw = fs::read_to_string(&hooks_path)?;
            if let Ok(mut doc) = serde_json::from_str::<Value>(&raw) {
                let removed = strip_managed_hooks(&mut doc);
                if removed > 0 {
                    snaps.push(durable_backup(&hooks_path)?);
                    atomic_write_durable(
                        &hooks_path,
                        serde_json::to_string_pretty(&doc)?.as_bytes(),
                    )?;
                }
            }
        }
        Ok(snaps)
    }
}

fn which_codex() -> Option<PathBuf> {
    // Desktop apps often have a leaner PATH than interactive shells.
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("codex"));
        }
    }
    if let Some(home) = dirs::home_dir() {
        for rel in [
            ".local/bin/codex",
            ".npm-global/bin/codex",
            "bin/codex",
            ".nvm/current/bin/codex",
            ".asdf/shims/codex",
        ] {
            candidates.push(home.join(rel));
        }
    }
    // Optional override for advanced installs / onboarding picker.
    if let Ok(over) = std::env::var("MORAINE_CODEX") {
        candidates.insert(0, PathBuf::from(over));
    }
    for cand in candidates {
        if cand.is_file() {
            return Some(fs::canonicalize(&cand).unwrap_or(cand));
        }
    }
    None
}

fn extract_command_from_toml(cfg: &str) -> Option<String> {
    let mut in_block = false;
    for line in cfg.lines() {
        let t = line.trim();
        if t.starts_with("[mcp_servers.moraine]") {
            in_block = true;
            continue;
        }
        if in_block && t.starts_with('[') {
            break;
        }
        if in_block && t.starts_with("command") {
            if let Some(rest) = t.split_once('=') {
                let v = rest.1.trim().trim_matches('"').to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn merge_mcp_block(existing: &str, mcp_block: &str) -> String {
    if existing.contains(MANAGED_TOML_START) {
        let stripped = strip_managed_block(existing);
        let mut out = stripped;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(mcp_block.trim_start());
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    } else if existing.contains("[mcp_servers.moraine]") {
        // Unmanaged block — leave untouched.
        existing.to_string()
    } else {
        let mut out = existing.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(mcp_block.trim_start());
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }
}

fn ensure_managed_hooks(doc: &mut Value, hook_cmd: &str) -> usize {
    strip_managed_hooks(doc);
    if !doc
        .get("hooks")
        .map(|h| h.is_object())
        .unwrap_or(false)
    {
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
    hooks.retain(|_, v| v.as_array().map(|a| !a.is_empty()).unwrap_or(true));
    removed
}

fn is_managed_hook_group(item: &Value) -> bool {
    let Some(inner) = item.get("hooks").and_then(|h| h.as_array()) else {
        return false;
    };
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
                    .unwrap_or(true)
    })
}

fn strip_managed_block(cfg: &str) -> String {
    if let (Some(a), Some(b)) = (cfg.find(MANAGED_TOML_START), cfg.find(MANAGED_TOML_END)) {
        let mut out = String::new();
        out.push_str(&cfg[..a]);
        out.push_str(&cfg[b + MANAGED_TOML_END.len()..]);
        while out.contains("\n\n\n") {
            out = out.replace("\n\n\n", "\n\n");
        }
        return out.trim_start_matches('\n').to_string();
    }
    cfg.to_string()
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_preserves_unrelated_toml() {
        let existing = "foo = 1\n";
        let block = format!(
            "\n{MANAGED_TOML_START}\n[mcp_servers.moraine]\ncommand = \"/abs/moraine\"\n{MANAGED_TOML_END}\n"
        );
        let out = merge_mcp_block(existing, &block);
        assert!(out.contains("foo = 1"));
        assert!(out.contains("/abs/moraine"));
    }

    #[test]
    fn strip_restores_user_config() {
        let cfg = "foo = 1\n# --- Moraine (managed) ---\n[mcp_servers.moraine]\ncommand = \"x\"\n# --- end Moraine ---\nbar = 2\n";
        let out = strip_managed_block(cfg);
        assert!(out.contains("foo = 1"));
        assert!(out.contains("bar = 2"));
        assert!(!out.contains("mcp_servers.moraine"));
    }
}
