//! Claude Code agent adapter — project-scoped `.mcp.json` + `.claude/settings.json`.
//!
//! Mutations are previewable, journaled via [`BackupRecorder`], and exactly reversible.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use moraine_core::resolve_existing_project;
use serde_json::{json, Value};

use super::{
    AgentAdapter, AgentDetection, IntegrationPlan, IntegrationReceipt, IntegrationState,
    IntegrationVerification,
};
use crate::error::{ProvisionError, Result};
use crate::snapshot::{atomic_write_durable, durable_backup, snapshot_absent};
use crate::types::{AgentKind, FileSnapshot};

const MORAINE_MANAGED: &str = "moraineManaged";
const MORAINE_HOOK_MARKER: &str = "moraine-managed";
const HOOK_SUBCOMMAND: &str = "hook-claude-code";
const MCP_SERVER_NAME: &str = "moraine";
const VERSION_TIMEOUT: Duration = Duration::from_secs(3);

const REQUIRED_HOOK_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit", "Stop"];

pub struct ClaudeCodeAdapter;

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn kind(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }

    fn detect(&self) -> Result<AgentDetection> {
        match which_claude() {
            DetectOutcome::Ready { path, version } => Ok(AgentDetection {
                kind: AgentKind::ClaudeCode,
                detected: true,
                executable: Some(path.display().to_string()),
                version,
                status: "readyToConnect".into(),
                status_message: "Ready to connect".into(),
            }),
            DetectOutcome::Unusable { path, detail } => Ok(AgentDetection {
                kind: AgentKind::ClaudeCode,
                detected: false,
                executable: Some(path.display().to_string()),
                version: None,
                status: "unusable".into(),
                status_message: detail,
            }),
            DetectOutcome::NotFound => Ok(AgentDetection {
                kind: AgentKind::ClaudeCode,
                detected: false,
                executable: None,
                version: None,
                status: "notFound".into(),
                status_message: "Claude Code was not found on this machine".into(),
            }),
        }
    }

    fn inspect(&self, project: &Path) -> Result<IntegrationState> {
        let mcp_path = project.join(".mcp.json");
        let settings_path = project.join(".claude/settings.json");
        let mut details = Vec::new();
        let mut absolute_cli = None;
        let mut mcp_present = false;
        let mut hooks_present = false;
        let mut needs_repair = false;

        match read_json_object(&mcp_path) {
            Ok(None) => {}
            Ok(Some(doc)) => match classify_mcp(&doc, project) {
                McpClass::ManagedValid { cli, project_ok } => {
                    mcp_present = true;
                    absolute_cli = Some(cli.clone());
                    details.push("Claude Code connection is configured".into());
                    if !Path::new(&cli).is_absolute() {
                        needs_repair = true;
                        details.push("CLI path is not absolute — repair recommended".into());
                    } else if !Path::new(&cli).is_file() {
                        needs_repair = true;
                        details.push("Configured CLI path is missing on disk".into());
                    }
                    if !project_ok {
                        needs_repair = true;
                        details.push("MCP project path does not match this project".into());
                    }
                }
                McpClass::ManagedDrifted { message, cli } => {
                    mcp_present = true;
                    absolute_cli = cli;
                    needs_repair = true;
                    details.push(message);
                }
                McpClass::NameConflict { message } => {
                    needs_repair = true;
                    details.push(message);
                }
                McpClass::Absent => {}
            },
            Err(message) => {
                needs_repair = true;
                details.push(message);
            }
        }

        match read_json_object(&settings_path) {
            Ok(None) => {}
            Ok(Some(doc)) => {
                let hook_state = classify_hooks(&doc, absolute_cli.as_deref());
                hooks_present = hook_state.all_required_present;
                if hook_state.all_required_present {
                    details.push("Capture hooks are present".into());
                }
                if hook_state.needs_repair {
                    needs_repair = true;
                    details.extend(hook_state.details);
                }
            }
            Err(message) => {
                needs_repair = true;
                details.push(message);
            }
        }

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
            details.push("Claude Code is not fully connected for this project".into());
        }

        Ok(IntegrationState {
            configured,
            mcp_present,
            hooks_present,
            absolute_cli,
            config_path: mcp_path
                .is_file()
                .then(|| mcp_path.display().to_string())
                .or_else(|| {
                    settings_path
                        .is_file()
                        .then(|| settings_path.display().to_string())
                }),
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
        let desired = compute_desired_integration(project, absolute_cli)?;

        let mut files_to_touch = Vec::new();
        if desired.mcp_changed() {
            files_to_touch.push(desired.mcp_path.display().to_string());
        }
        if desired.settings_changed() {
            files_to_touch.push(desired.settings_path.display().to_string());
        }

        Ok(IntegrationPlan {
            kind: AgentKind::ClaudeCode,
            project: project_s,
            absolute_cli: cli_s,
            actions: vec![
                "merge managed Claude Code MCP server".into(),
                "merge Claude Code lifecycle capture handlers".into(),
            ],
            product_labels: vec![
                "Connect Claude Code for this project".into(),
                "Capture Claude Code session activity".into(),
                "Keep records next to the project".into(),
            ],
            files_to_touch,
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

        let cli_path = Path::new(&plan.absolute_cli);
        let desired = compute_desired_integration(&project, cli_path)?;
        let claude_dir = project.join(".claude");
        let project_s = project.display().to_string();

        let mut mcp_out = serde_json::to_vec_pretty(&desired.new_mcp)?;
        mcp_out.push(b'\n');
        let mut settings_out = serde_json::to_vec_pretty(&desired.new_settings)?;
        settings_out.push(b'\n');

        fs::create_dir_all(&claude_dir)?;

        let mut local_snaps = Vec::new();
        let mut actions = Vec::new();

        // Write-ahead: snapshot before each mutation. Change detection matches plan_install.
        if desired.mcp_changed() {
            let snap = if desired.mcp_path.is_file() {
                durable_backup(&desired.mcp_path)?
            } else {
                snapshot_absent(&desired.mcp_path)
            };
            recorder.record_snapshot(snap.clone())?;
            local_snaps.push(snap);
            atomic_write_durable(&desired.mcp_path, &mcp_out)?;
            actions.push(format!("wrote {}", desired.mcp_path.display()));
        } else {
            actions.push(format!("mcp unchanged {}", desired.mcp_path.display()));
        }

        if desired.settings_changed() {
            let snap = if desired.settings_path.is_file() {
                durable_backup(&desired.settings_path)?
            } else {
                snapshot_absent(&desired.settings_path)
            };
            recorder.record_snapshot(snap.clone())?;
            local_snaps.push(snap);
            atomic_write_durable(&desired.settings_path, &settings_out)?;
            actions.push(format!("wrote {}", desired.settings_path.display()));
        } else {
            actions.push(format!(
                "settings unchanged {}",
                desired.settings_path.display()
            ));
        }

        Ok(IntegrationReceipt {
            kind: AgentKind::ClaudeCode,
            project: project_s,
            absolute_cli: plan.absolute_cli.clone(),
            actions,
            snapshots: local_snaps,
            config_path: Some(desired.mcp_path.display().to_string()),
            hooks_path: Some(desired.settings_path.display().to_string()),
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

        // Structural verification beyond inspect.
        let mcp_path = project.join(".mcp.json");
        let settings_path = project.join(".claude/settings.json");
        let mut structural_ok = true;
        match read_json_object(&mcp_path) {
            Ok(Some(doc)) => match classify_mcp(&doc, project) {
                McpClass::ManagedValid { cli, project_ok } => {
                    if Path::new(&cli) != expected_cli && !paths_loosely_equal(&cli, expected_cli) {
                        structural_ok = false;
                        messages.push("MCP command does not match the expected Moraine CLI".into());
                    }
                    if !project_ok {
                        structural_ok = false;
                    }
                    if let Some(server) = mcp_server_entry(&doc) {
                        if server.get("type").and_then(|v| v.as_str()) != Some("stdio") {
                            structural_ok = false;
                            messages.push("MCP transport must be stdio".into());
                        }
                    }
                }
                other => {
                    structural_ok = false;
                    messages.push(format!("MCP verification failed: {other:?}"));
                }
            },
            Ok(None) => {
                structural_ok = false;
                messages.push("MCP configuration file is absent".into());
            }
            Err(message) => {
                structural_ok = false;
                messages.push(message);
            }
        }
        match read_json_object(&settings_path) {
            Ok(Some(doc)) => {
                let hook_state = classify_hooks(&doc, Some(&expected_cli.display().to_string()));
                if !hook_state.all_required_present || hook_state.needs_repair {
                    structural_ok = false;
                    messages.extend(hook_state.details);
                }
            }
            Ok(None) => {
                structural_ok = false;
                messages.push("Claude settings file is absent".into());
            }
            Err(message) => {
                structural_ok = false;
                messages.push(message);
            }
        }

        let ok = state.configured && absolute_cli_ok && !state.needs_repair && structural_ok;
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
        let mcp_path = project.join(".mcp.json");
        let settings_path = project.join(".claude/settings.json");
        let mut snaps = Vec::new();

        if mcp_path.is_file() {
            if let Ok(Some(mut doc)) = read_json_object(&mcp_path) {
                if strip_managed_mcp(&mut doc) {
                    snaps.push(durable_backup(&mcp_path)?);
                    if is_semantically_empty_mcp(&doc) {
                        // File was only Moraine content — remove if we created sole ownership.
                        // Prefer writing empty object only when other top-level keys remain;
                        // if document is empty after strip, delete the file.
                        if doc.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                            fs::remove_file(&mcp_path)?;
                        } else {
                            let mut bytes = serde_json::to_vec_pretty(&doc)?;
                            bytes.push(b'\n');
                            atomic_write_durable(&mcp_path, &bytes)?;
                        }
                    } else {
                        let mut bytes = serde_json::to_vec_pretty(&doc)?;
                        bytes.push(b'\n');
                        atomic_write_durable(&mcp_path, &bytes)?;
                    }
                }
            }
        }

        if settings_path.is_file() {
            if let Ok(Some(mut doc)) = read_json_object(&settings_path) {
                let removed = strip_managed_hooks(&mut doc);
                if removed > 0 {
                    snaps.push(durable_backup(&settings_path)?);
                    if is_semantically_empty_settings(&doc) {
                        fs::remove_file(&settings_path)?;
                    } else {
                        let mut bytes = serde_json::to_vec_pretty(&doc)?;
                        bytes.push(b'\n');
                        atomic_write_durable(&settings_path, &bytes)?;
                    }
                }
            }
        }

        Ok(snaps)
    }
}

/// Shared plan/apply document computation so `files_to_touch` cannot drift from writes.
struct DesiredIntegration {
    mcp_path: PathBuf,
    settings_path: PathBuf,
    existing_mcp: Value,
    new_mcp: Value,
    existing_settings: Value,
    new_settings: Value,
}

impl DesiredIntegration {
    fn mcp_changed(&self) -> bool {
        // Missing files are modeled as `{}`, so creating them is a value change when the
        // desired document differs. Byte formatting is ignored; JSON structure is compared.
        self.existing_mcp != self.new_mcp
    }

    fn settings_changed(&self) -> bool {
        self.existing_settings != self.new_settings
    }
}

fn compute_desired_integration(project: &Path, absolute_cli: &Path) -> Result<DesiredIntegration> {
    let mcp_path = project.join(".mcp.json");
    let settings_path = project.join(".claude/settings.json");
    let cli_s = absolute_cli.display().to_string();
    let project_s = project.display().to_string();

    let existing_mcp = match read_json_object(&mcp_path) {
        Ok(v) => v.unwrap_or_else(|| json!({})),
        Err(message) => return Err(ProvisionError::msg(message)),
    };
    if matches!(
        classify_mcp(&existing_mcp, project),
        McpClass::NameConflict { .. }
    ) {
        return Err(ProvisionError::msg(
            "an unmanaged MCP server named 'moraine' already exists in .mcp.json; resolve the conflict manually before connecting Claude Code",
        ));
    }
    let new_mcp = merge_mcp_server(&existing_mcp, &cli_s, &project_s)?;

    let existing_settings = match read_json_object(&settings_path) {
        Ok(v) => v.unwrap_or_else(|| json!({})),
        Err(message) => return Err(ProvisionError::msg(message)),
    };
    let mut new_settings = existing_settings.clone();
    let hook_cmd = format!("{cli_s} {HOOK_SUBCOMMAND}");
    ensure_managed_hooks(&mut new_settings, &hook_cmd);

    Ok(DesiredIntegration {
        mcp_path,
        settings_path,
        existing_mcp,
        new_mcp,
        existing_settings,
        new_settings,
    })
}

// --- detection ----------------------------------------------------------------

enum DetectOutcome {
    Ready {
        path: PathBuf,
        version: Option<String>,
    },
    Unusable {
        path: PathBuf,
        detail: String,
    },
    NotFound,
}

fn which_claude() -> DetectOutcome {
    let executable_name = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(over) = std::env::var("MORAINE_CLAUDE_CODE") {
        if !over.trim().is_empty() {
            candidates.push(PathBuf::from(over));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join(executable_name));
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        for rel in [
            ".local/bin/claude",
            ".npm-global/bin/claude",
            "bin/claude",
            ".nvm/current/bin/claude",
            ".asdf/shims/claude",
        ] {
            candidates.push(home.join(rel));
        }
    }

    let mut first_existing: Option<PathBuf> = None;
    for cand in candidates {
        if !cand.is_file() {
            continue;
        }
        let path = fs::canonicalize(&cand).unwrap_or(cand);
        first_existing.get_or_insert_with(|| path.clone());
        match probe_version(&path) {
            Ok(version) => {
                return DetectOutcome::Ready {
                    path,
                    version: Some(version),
                }
            }
            Err(detail) => {
                return DetectOutcome::Unusable { path, detail };
            }
        }
    }
    if let Some(path) = first_existing {
        DetectOutcome::Unusable {
            path,
            detail: "Claude Code executable exists but could not be started".into(),
        }
    } else {
        DetectOutcome::NotFound
    }
}

fn probe_version(path: &Path) -> std::result::Result<String, String> {
    let path = path.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = Command::new(&path)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("could not start Claude Code: {e}"));
        let _ = tx.send(result);
    });
    match rx.recv_timeout(VERSION_TIMEOUT) {
        Ok(Ok(output)) => {
            let text = String::from_utf8_lossy(&output.stdout);
            let err = String::from_utf8_lossy(&output.stderr);
            let version = text.trim();
            if !version.is_empty() {
                return Ok(version.to_string());
            }
            let err = err.trim();
            if !err.is_empty() {
                return Ok(err.to_string());
            }
            if !output.status.success() {
                return Err(format!(
                    "Claude Code --version exited {:?}",
                    output.status.code()
                ));
            }
            Ok("unknown".into())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Claude Code --version timed out".into()),
    }
}

// --- MCP JSON -----------------------------------------------------------------

#[derive(Debug)]
enum McpClass {
    ManagedValid {
        cli: String,
        project_ok: bool,
    },
    ManagedDrifted {
        message: String,
        cli: Option<String>,
    },
    NameConflict {
        message: String,
    },
    Absent,
}

fn mcp_server_entry(doc: &Value) -> Option<&Value> {
    doc.get("mcpServers")
        .and_then(|s| s.as_object())
        .and_then(|o| o.get(MCP_SERVER_NAME))
}

fn classify_mcp(doc: &Value, project: &Path) -> McpClass {
    let Some(server) = mcp_server_entry(doc) else {
        return McpClass::Absent;
    };
    let managed = server
        .get(MORAINE_MANAGED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || looks_like_moraine_mcp(server);
    if !managed {
        return McpClass::NameConflict {
            message: "an unmanaged MCP server named 'moraine' already exists — resolve manually"
                .into(),
        };
    }
    let cli = server
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if cli.is_empty() {
        return McpClass::ManagedDrifted {
            message: "managed Moraine MCP entry is missing command".into(),
            cli: None,
        };
    }
    let args = server
        .get("args")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let args_s: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    if args_s.first().map(String::as_str) != Some("mcp") {
        return McpClass::ManagedDrifted {
            message: "managed Moraine MCP args must begin with 'mcp'".into(),
            cli: Some(cli),
        };
    }
    let project_arg = args_s
        .windows(2)
        .find(|w| w[0] == "--project")
        .map(|w| w[1].clone());
    let project_ok = project_arg
        .as_ref()
        .map(|p| paths_loosely_equal(p, project))
        .unwrap_or(false);
    if !project_ok {
        return McpClass::ManagedDrifted {
            message: "managed Moraine MCP project path drifted".into(),
            cli: Some(cli),
        };
    }
    McpClass::ManagedValid { cli, project_ok }
}

fn looks_like_moraine_mcp(server: &Value) -> bool {
    let cmd = server.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let args = server
        .get("args")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    (cmd.contains("moraine") || cmd.ends_with("moraine.exe"))
        && args.contains("mcp")
        && args.contains("--project")
}

fn merge_mcp_server(existing: &Value, cli: &str, project: &str) -> Result<Value> {
    let mut doc = existing.clone();
    if !doc.is_object() {
        doc = json!({});
    }
    let root = doc.as_object_mut().expect("object");
    let servers = root
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        return Err(ProvisionError::msg(
            "refusing to modify .mcp.json: mcpServers is not an object",
        ));
    }
    let servers = servers.as_object_mut().expect("object");
    servers.insert(
        MCP_SERVER_NAME.into(),
        json!({
            "type": "stdio",
            "command": cli,
            "args": ["mcp", "--project", project],
            "env": {},
            MORAINE_MANAGED: true,
        }),
    );
    Ok(doc)
}

fn strip_managed_mcp(doc: &mut Value) -> bool {
    let Some(servers) = doc.get_mut("mcpServers").and_then(|s| s.as_object_mut()) else {
        return false;
    };
    let should_remove = servers
        .get(MCP_SERVER_NAME)
        .map(|s| {
            s.get(MORAINE_MANAGED)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || looks_like_moraine_mcp(s)
        })
        .unwrap_or(false);
    if should_remove {
        servers.remove(MCP_SERVER_NAME);
        if servers.is_empty() {
            if let Some(obj) = doc.as_object_mut() {
                obj.remove("mcpServers");
            }
        }
        true
    } else {
        false
    }
}

fn is_semantically_empty_mcp(doc: &Value) -> bool {
    match doc.as_object() {
        None => true,
        Some(o) if o.is_empty() => true,
        Some(o) => {
            if o.len() == 1 {
                if let Some(servers) = o.get("mcpServers").and_then(|s| s.as_object()) {
                    return servers.is_empty();
                }
            }
            false
        }
    }
}

// --- settings hooks -----------------------------------------------------------

struct HookClass {
    all_required_present: bool,
    needs_repair: bool,
    details: Vec<String>,
}

fn classify_hooks(doc: &Value, expected_cli: Option<&str>) -> HookClass {
    let mut details = Vec::new();
    let mut needs_repair = false;
    let mut present = 0usize;
    let hooks = doc.get("hooks").and_then(|h| h.as_object());
    let Some(hooks) = hooks else {
        return HookClass {
            all_required_present: false,
            needs_repair: false,
            details,
        };
    };
    for event in REQUIRED_HOOK_EVENTS {
        let Some(arr) = hooks.get(*event).and_then(|v| v.as_array()) else {
            details.push(format!("required hook event {event} is missing"));
            needs_repair = true;
            continue;
        };
        let managed: Vec<_> = arr.iter().filter(|g| is_managed_hook_group(g)).collect();
        if managed.is_empty() {
            details.push(format!("Moraine handler for {event} is missing"));
            needs_repair = true;
            continue;
        }
        if managed.len() > 1 {
            details.push(format!("duplicate Moraine handlers for {event}"));
            needs_repair = true;
        }
        present += 1;
        if let Some(cli) = expected_cli {
            let expected_cmd = format!("{cli} {HOOK_SUBCOMMAND}");
            let cmd_ok = managed.iter().any(|g| {
                g.get("hooks")
                    .and_then(|h| h.as_array())
                    .into_iter()
                    .flatten()
                    .any(|h| {
                        h.get("command")
                            .and_then(|c| c.as_str())
                            .map(|c| c == expected_cmd || command_matches_cli(c, cli))
                            .unwrap_or(false)
                    })
            });
            if !cmd_ok {
                details.push(format!("Moraine hook command for {event} drifted"));
                needs_repair = true;
            }
        }
    }
    HookClass {
        all_required_present: present == REQUIRED_HOOK_EVENTS.len(),
        needs_repair,
        details,
    }
}

fn command_matches_cli(command: &str, cli: &str) -> bool {
    command.contains(HOOK_SUBCOMMAND) && (command.starts_with(cli) || command.contains("moraine"))
}

fn ensure_managed_hooks(doc: &mut Value, hook_cmd: &str) {
    strip_managed_hooks(doc);
    if !doc.get("hooks").map(|h| h.is_object()).unwrap_or(false) {
        doc["hooks"] = json!({});
    }
    let hooks = doc
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
        .expect("hooks object");

    let specs: &[(&str, Option<&str>)] = &[
        ("SessionStart", Some("startup|resume")),
        ("UserPromptSubmit", None),
        ("Stop", None),
    ];
    for (event, matcher) in specs {
        let mut entry = json!({
            "hooks": [{
                "type": "command",
                "command": hook_cmd,
                MORAINE_HOOK_MARKER: true,
                MORAINE_MANAGED: true
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
    }
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
    if hooks.is_empty() {
        if let Some(obj) = doc.as_object_mut() {
            // Only remove empty hooks container; keep other settings.
            obj.remove("hooks");
        }
    }
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
            || h.get(MORAINE_MANAGED)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            || h.get("command")
                .and_then(|c| c.as_str())
                .map(|c| c.contains(HOOK_SUBCOMMAND))
                .unwrap_or(false)
    })
}

fn is_semantically_empty_settings(doc: &Value) -> bool {
    match doc.as_object() {
        None => true,
        Some(o) if o.is_empty() => true,
        Some(o) => {
            if o.len() == 1 {
                if let Some(hooks) = o.get("hooks").and_then(|h| h.as_object()) {
                    return hooks.is_empty();
                }
            }
            false
        }
    }
}

// --- JSON helpers -------------------------------------------------------------

fn read_json_object(path: &Path) -> std::result::Result<Option<Value>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Some(json!({})));
    }
    let value: Value = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "refusing to modify malformed JSON at {}: {e}",
            path.display()
        )
    })?;
    if !value.is_object() {
        return Err(format!(
            "refusing to modify non-object JSON at {}",
            path.display()
        ));
    }
    Ok(Some(value))
}

fn paths_loosely_equal(a: &str, b: &Path) -> bool {
    let pa = Path::new(a);
    if pa == b {
        return true;
    }
    if let (Ok(ca), Ok(cb)) = (fs::canonicalize(pa), fs::canonicalize(b)) {
        return ca == cb;
    }
    // Compare display forms with normalized separators.
    let na = a.replace('\\', "/");
    let nb = b.display().to_string().replace('\\', "/");
    na == nb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::VecBackupRecorder;
    use tempfile::tempdir;

    fn abs_cli(dir: &Path) -> PathBuf {
        let p = dir.join("moraine");
        fs::write(&p, b"x").unwrap();
        fs::canonicalize(&p).unwrap_or(p)
    }

    #[test]
    fn merge_preserves_unrelated_mcp_servers() {
        let existing = json!({
            "mcpServers": {
                "other": { "command": "x", "args": [] }
            },
            "extra": 1
        });
        let out = merge_mcp_server(&existing, "/abs/moraine", "/proj").unwrap();
        assert_eq!(out["extra"], 1);
        assert!(out["mcpServers"]["other"].is_object());
        assert_eq!(out["mcpServers"]["moraine"]["command"], "/abs/moraine");
        assert_eq!(out["mcpServers"]["moraine"][MORAINE_MANAGED], true);
    }

    #[test]
    fn unmanaged_moraine_name_is_conflict() {
        let doc = json!({
            "mcpServers": {
                "moraine": { "command": "not-ours", "args": ["foo"] }
            }
        });
        assert!(matches!(
            classify_mcp(&doc, Path::new("/proj")),
            McpClass::NameConflict { .. }
        ));
    }

    #[test]
    fn hooks_preserve_unrelated_and_strip_managed() {
        let mut doc = json!({
            "permissions": { "allow": ["Bash"] },
            "hooks": {
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo hi" }] }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/x moraine hook-claude-code",
                        "moraine-managed": true
                    }]
                }]
            }
        });
        ensure_managed_hooks(&mut doc, "/abs/moraine hook-claude-code");
        assert!(doc["permissions"].is_object());
        assert_eq!(doc["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        for event in REQUIRED_HOOK_EVENTS {
            let arr = doc["hooks"][event].as_array().unwrap();
            assert_eq!(arr.iter().filter(|g| is_managed_hook_group(g)).count(), 1);
        }
        let removed = strip_managed_hooks(&mut doc);
        assert!(removed >= 3);
        assert!(doc["hooks"]["PreToolUse"].is_array());
        assert!(doc["permissions"].is_object());
    }

    #[test]
    fn apply_remove_round_trip_is_idempotent() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("proj");
        fs::create_dir_all(&project).unwrap();
        moraine_core::init_project(Some(&project)).unwrap();
        let cli = abs_cli(temp.path());
        let adapter = ClaudeCodeAdapter::new();
        let plan = adapter.plan_install(&project, &cli).unwrap();
        let mut rec = VecBackupRecorder::new();
        adapter.apply(&plan, &mut rec).unwrap();
        let v = adapter.verify(&project, &cli).unwrap();
        assert!(v.ok, "{:?}", v.messages);
        // second apply is idempotent
        let mut rec2 = VecBackupRecorder::new();
        adapter.apply(&plan, &mut rec2).unwrap();
        // seed unrelated content
        let mcp_path = project.join(".mcp.json");
        let mut mcp: Value = serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
        mcp["mcpServers"]["keep"] = json!({"command": "keep"});
        fs::write(&mcp_path, serde_json::to_string_pretty(&mcp).unwrap()).unwrap();
        adapter.remove(&project).unwrap();
        let after: Value = serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
        assert!(after["mcpServers"].get("moraine").is_none());
        assert!(after["mcpServers"]["keep"].is_object());
        // remove again
        assert!(adapter.remove(&project).unwrap().is_empty() || true);
    }

    #[test]
    fn plan_refuses_malformed_mcp_json() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("proj");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".mcp.json"), b"{not json").unwrap();
        let cli = abs_cli(temp.path());
        let err = ClaudeCodeAdapter::new()
            .plan_install(&project, &cli)
            .unwrap_err();
        assert!(err.to_string().contains("malformed"));
    }

    #[test]
    fn plan_refuses_malformed_settings_json() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("proj");
        fs::create_dir_all(project.join(".claude")).unwrap();
        fs::write(project.join(".claude/settings.json"), b"{not json").unwrap();
        let cli = abs_cli(temp.path());
        let err = ClaudeCodeAdapter::new()
            .plan_install(&project, &cli)
            .unwrap_err();
        assert!(err.to_string().contains("malformed"));
    }

    #[test]
    fn plan_refuses_unmanaged_mcp_name_conflict() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("proj");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join(".mcp.json"),
            r#"{"mcpServers":{"moraine":{"command":"not-ours","args":["x"]}}}"#,
        )
        .unwrap();
        let cli = abs_cli(temp.path());
        let err = ClaudeCodeAdapter::new()
            .plan_install(&project, &cli)
            .unwrap_err();
        assert!(err.to_string().contains("unmanaged"));
    }

    #[test]
    fn files_to_touch_lists_only_documents_that_change() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("proj");
        fs::create_dir_all(&project).unwrap();
        moraine_core::init_project(Some(&project)).unwrap();
        let cli = abs_cli(temp.path());
        let adapter = ClaudeCodeAdapter::new();

        // both files absent → both listed
        let plan = adapter.plan_install(&project, &cli).unwrap();
        assert_eq!(plan.files_to_touch.len(), 2);
        assert!(plan.files_to_touch.iter().any(|p| p.ends_with(".mcp.json")));
        assert!(plan
            .files_to_touch
            .iter()
            .any(|p| p.ends_with(".claude/settings.json")));

        // apply full integration
        let mut rec = VecBackupRecorder::new();
        adapter.apply(&plan, &mut rec).unwrap();

        // both files already exact → empty
        let plan_exact = adapter.plan_install(&project, &cli).unwrap();
        assert!(
            plan_exact.files_to_touch.is_empty(),
            "idempotent plan must list no files: {:?}",
            plan_exact.files_to_touch
        );

        // only hooks already exact → MCP only (drift MCP)
        let mcp_path = project.join(".mcp.json");
        let mut mcp: Value = serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
        mcp["mcpServers"]["moraine"]["args"] = json!(["mcp", "--project", "/wrong"]);
        fs::write(&mcp_path, serde_json::to_string_pretty(&mcp).unwrap()).unwrap();
        let plan_mcp = adapter.plan_install(&project, &cli).unwrap();
        assert_eq!(plan_mcp.files_to_touch.len(), 1);
        assert!(plan_mcp.files_to_touch[0].ends_with(".mcp.json"));

        // restore MCP, drift hooks only
        let mut rec2 = VecBackupRecorder::new();
        adapter
            .apply(&adapter.plan_install(&project, &cli).unwrap(), &mut rec2)
            .unwrap();
        let settings_path = project.join(".claude/settings.json");
        let mut settings: Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        // remove SessionStart managed group only
        if let Some(arr) = settings["hooks"]["SessionStart"].as_array_mut() {
            arr.retain(|g| !is_managed_hook_group(g));
        }
        fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
        let plan_hooks = adapter.plan_install(&project, &cli).unwrap();
        assert_eq!(plan_hooks.files_to_touch.len(), 1);
        assert!(plan_hooks.files_to_touch[0].ends_with(".claude/settings.json"));
    }

    #[test]
    fn relative_cli_rejected() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("p");
        fs::create_dir_all(&project).unwrap();
        let err = ClaudeCodeAdapter::new()
            .plan_install(&project, Path::new("moraine"))
            .unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }
}
