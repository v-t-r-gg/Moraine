//! `moraine doctor` health report (C2).

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use moraine_core::{resolve_existing_project, BuildIdentity};
use serde::Serialize;

use crate::suite::{
    collect_version_report, current_exe_path, default_socket_path, enumerate_moraine_on_path,
    http_get_loopback, SuitePaths,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub ok: bool,
    pub build: BuildIdentity,
    pub checks: Vec<DoctorCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration: Option<IntegrationCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    pub id: String,
    /// pass | warn | fail | info
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCheck {
    pub path: String,
    pub initialized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationCheck {
    pub name: String,
    pub configured: bool,
    pub details: Vec<String>,
}

fn check(
    id: &str,
    status: &str,
    message: impl Into<String>,
    observed: Option<&str>,
    expected: Option<&str>,
    remediation: Option<&str>,
) -> DoctorCheck {
    DoctorCheck {
        id: id.into(),
        status: status.into(),
        message: message.into(),
        observed: observed.map(|s| s.into()),
        expected: expected.map(|s| s.into()),
        remediation: remediation.map(|s| s.into()),
    }
}

pub fn run_doctor(project: Option<&Path>, integration: Option<&str>) -> DoctorReport {
    let build = BuildIdentity::current();
    let suite = SuitePaths::discover();
    let ver = collect_version_report();
    let mut checks = Vec::new();
    let current = current_exe_path();

    checks.push(check(
        "suite.cli_path",
        "info",
        format!("running executable {}", current.display()),
        Some(&current.display().to_string()),
        None,
        None,
    ));

    let manifest_ok = suite.manifest.is_file();
    checks.push(check(
        "suite.manifest",
        if manifest_ok { "pass" } else { "warn" },
        if manifest_ok {
            format!("manifest at {}", suite.manifest.display())
        } else {
            "suite manifest missing (development binary or incomplete install)".into()
        },
        Some(&suite.manifest.display().to_string()),
        Some("present manifest.json under share/moraine"),
        Some("Install with the release bundle install.sh"),
    ));

    if let Some(m) = suite.read_manifest() {
        let coherent = m.components_coherent();
        checks.push(check(
            "suite.components",
            if coherent { "pass" } else { "fail" },
            format!(
                "suite version {} (cli={}, service={}, desktop={})",
                m.version, m.components.cli, m.components.service, m.components.desktop
            ),
            Some(&format!("{:?}", m.components)),
            Some("all present components match product version"),
            (!coherent).then_some("Reinstall a coherent release bundle"),
        ));
        let match_cli = m.version == build.version;
        checks.push(check(
            "suite.cli_match",
            if match_cli { "pass" } else { "fail" },
            format!("running CLI {} vs suite {}", build.version, m.version),
            Some(&build.version),
            Some(&m.version),
            (!match_cli).then_some("Ensure PATH prefers the installed suite CLI; hash -r"),
        ));
    }

    // PATH drift — per candidate
    let path_exes = enumerate_moraine_on_path();
    let mut cargo_shadow = false;
    for (i, p) in path_exes.iter().enumerate() {
        let s = p.to_string_lossy();
        let is_cargo = s.contains(".cargo/bin");
        if is_cargo {
            cargo_shadow = true;
        }
        let is_current = fs::canonicalize(p).ok().as_ref()
            == fs::canonicalize(&current).ok().as_ref()
            || p == &current;
        let ver_out = Command::new(p)
            .args(["version", "--json"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    serde_json::from_slice::<serde_json::Value>(&o.stdout).ok()
                } else {
                    None
                }
            })
            .and_then(|v| {
                v.get("cli")
                    .and_then(|c| c.get("version"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            });
        let status = if is_cargo { "warn" } else { "pass" };
        checks.push(check(
            &format!("path.candidate.{i}"),
            status,
            format!(
                "{}{}{}",
                p.display(),
                if is_current { " (current)" } else { "" },
                ver_out
                    .as_ref()
                    .map(|v| format!(" version={v}"))
                    .unwrap_or_default()
            ),
            Some(&p.display().to_string()),
            Some("single authoritative suite CLI first on PATH"),
            is_cargo.then_some("Deprioritize ~/.cargo/bin; prefer ~/.local/bin; run: hash -r"),
        ));
    }
    checks.push(check(
        "path.count",
        if path_exes.len() <= 1 && !cargo_shadow {
            "pass"
        } else if cargo_shadow {
            "warn"
        } else {
            "info"
        },
        format!("{} moraine executable(s) on PATH", path_exes.len()),
        Some(&path_exes.len().to_string()),
        Some("1 preferred"),
        cargo_shadow.then_some("Remove or deprioritize ~/.cargo/bin on PATH"),
    ));

    // Service binary + unit ExecStart
    checks.push(check(
        "service.binary",
        if suite.service.is_file() {
            "pass"
        } else if manifest_ok {
            "fail"
        } else {
            "info"
        },
        if suite.service.is_file() {
            format!("service binary {}", suite.service.display())
        } else if manifest_ok {
            "suite manifest present but service binary missing".into()
        } else {
            "no installed service binary (ok for pure cargo dev)".into()
        },
        Some(&suite.service.display().to_string()),
        Some("libexec/moraine/moraine-service present"),
        Some("Re-run install.sh or moraine service install"),
    ));

    let unit_exists = suite.unit.is_file();
    let unit_body = unit_exists
        .then(|| fs::read_to_string(&suite.unit).ok())
        .flatten();
    let unit_points_cargo = unit_body
        .as_ref()
        .map(|s| s.contains(".cargo/bin"))
        .unwrap_or(false);
    let unit_exec = unit_body.as_ref().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("ExecStart="))
            .map(|l| l.trim_start_matches("ExecStart=").to_string())
    });
    let unit_matches_suite = unit_exec
        .as_ref()
        .map(|e| e.contains(&suite.service.display().to_string()) || e.contains("libexec/moraine"))
        .unwrap_or(false);
    checks.push(check(
        "service.unit",
        if !unit_exists {
            "warn"
        } else if unit_points_cargo {
            "fail"
        } else if unit_matches_suite {
            "pass"
        } else {
            "warn"
        },
        if !unit_exists {
            "systemd user unit not installed".into()
        } else if unit_points_cargo {
            format!(
                "unit {} points at ~/.cargo/bin (development drift)",
                suite.unit.display()
            )
        } else {
            format!(
                "unit {} ExecStart={}",
                suite.unit.display(),
                unit_exec.as_deref().unwrap_or("?")
            )
        },
        unit_exec.as_deref(),
        Some(&suite.service.display().to_string()),
        Some("moraine service install"),
    ));

    // Service online + version
    checks.push(check(
        "service.online",
        if ver.service.online { "pass" } else { "warn" },
        if ver.service.online {
            format!(
                "service online{}",
                ver.service
                    .version
                    .as_ref()
                    .map(|v| format!(" version={v}"))
                    .unwrap_or_default()
            )
        } else {
            ver.service
                .message
                .clone()
                .unwrap_or_else(|| "service not reachable on loopback diagnostics".into())
        },
        ver.service.version.as_deref(),
        Some(&build.version),
        Some("moraine service start"),
    ));

    if ver.service.online && !ver.service.compatible {
        checks.push(check(
            "service.version_compatible",
            "fail",
            "service version does not match CLI",
            ver.service.version.as_deref(),
            Some(&build.version),
            Some("Reinstall coherent suite and restart service"),
        ));
    }

    // Socket path + ownership
    let sock = default_socket_path();
    if sock.exists() {
        let meta = fs::metadata(&sock).ok();
        let mode = meta.as_ref().map(|m| m.permissions().mode() & 0o777);
        let uid_ok = meta
            .as_ref()
            .map(|m| m.uid() == libc_uid())
            .unwrap_or(false);
        checks.push(check(
            "service.socket",
            if uid_ok { "pass" } else { "warn" },
            format!(
                "socket {} mode={:o} uid_ok={uid_ok}",
                sock.display(),
                mode.unwrap_or(0)
            ),
            Some(&sock.display().to_string()),
            Some("user-owned unix socket"),
            None,
        ));
    } else {
        checks.push(check(
            "service.socket",
            "info",
            format!(
                "expected unix socket {} (created when service runs)",
                sock.display()
            ),
            None,
            Some(&sock.display().to_string()),
            Some("moraine service start"),
        ));
    }

    // Spool directory permissions
    let spool = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("moraine-service/spool");
    if spool.is_dir() {
        let mode = fs::metadata(&spool)
            .map(|m| m.permissions().mode() & 0o777)
            .unwrap_or(0);
        let restricted = mode & 0o077 == 0;
        checks.push(check(
            "service.spool_perms",
            if restricted { "pass" } else { "warn" },
            format!("spool {} mode={mode:o}", spool.display()),
            Some(&format!("{mode:o}")),
            Some("0700 preferred"),
            (!restricted).then_some("chmod 700 the spool directory"),
        ));
        if let Ok(body) = http_get_loopback(33111, "/status") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(sp) = v.get("spool") {
                    checks.push(check(
                        "service.spool_counts",
                        "info",
                        format!("spool counts {sp}"),
                        Some(&sp.to_string()),
                        None,
                        None,
                    ));
                }
            }
        }
    } else {
        checks.push(check(
            "service.spool",
            "info",
            format!("spool dir not yet created ({})", spool.display()),
            None,
            None,
            None,
        ));
    }

    // Desktop
    let desk_entry = suite.desktop_entry.is_file()
        || dirs::data_dir()
            .map(|d| d.join("applications/app.moraine.desktop").is_file())
            .unwrap_or(false);
    checks.push(check(
        "desktop.binary",
        if suite.desktop.is_file() {
            "pass"
        } else if manifest_ok {
            "warn"
        } else {
            "info"
        },
        if suite.desktop.is_file() {
            format!("desktop {}", suite.desktop.display())
        } else if manifest_ok {
            "desktop binary missing from suite".into()
        } else {
            "no installed desktop (dev mode ok)".into()
        },
        Some(&suite.desktop.display().to_string()),
        Some("lib/moraine/moraine-app"),
        Some("Install release bundle including moraine-app"),
    ));
    checks.push(check(
        "desktop.registration",
        if desk_entry || !manifest_ok {
            if desk_entry {
                "pass"
            } else {
                "info"
            }
        } else {
            "warn"
        },
        if desk_entry {
            "desktop entry present"
        } else {
            "desktop entry not found"
        },
        None,
        Some("share/applications/app.moraine.desktop"),
        Some("Re-run install.sh"),
    ));

    let project = project.map(|p| match resolve_existing_project(Some(p)) {
        Ok(r) => ProjectCheck {
            path: r.project_root.display().to_string(),
            initialized: true,
            project_id: Some(r.project_id.to_string()),
            message: None,
        },
        Err(e) => ProjectCheck {
            path: p.display().to_string(),
            initialized: false,
            project_id: None,
            message: Some(e.to_string()),
        },
    });
    if let Some(ref pc) = project {
        checks.push(check(
            "project.initialized",
            if pc.initialized { "pass" } else { "fail" },
            if pc.initialized {
                format!("project {} ready", pc.path)
            } else {
                pc.message
                    .clone()
                    .unwrap_or_else(|| "project not initialized".into())
            },
            Some(&pc.path),
            Some("initialized .moraine project"),
            (!pc.initialized).then_some("moraine project init <path>"),
        ));
    }

    let integration = integration.map(|name| {
        if name != "codex" {
            return IntegrationCheck {
                name: name.into(),
                configured: false,
                details: vec!["only codex is supported in C2".into()],
            };
        }
        let root = project
            .as_ref()
            .map(|p| PathBuf::from(&p.path))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let mut details = Vec::new();
        let codex_bin = which_on_path("codex");
        if let Some(ref c) = codex_bin {
            let ver = Command::new(c)
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            details.push(format!("codex {} ({ver})", c.display()));
            checks.push(check(
                "integration.codex.binary",
                "pass",
                format!("codex at {}", c.display()),
                Some(&c.display().to_string()),
                None,
                None,
            ));
        } else {
            details.push("codex executable not on PATH".into());
            checks.push(check(
                "integration.codex.binary",
                "warn",
                "codex not found on PATH",
                None,
                Some("codex on PATH"),
                Some("Install Codex CLI; Moraine does not install it"),
            ));
        }

        let cfg = root.join(".codex/config.toml");
        let hooks = root.join(".codex/hooks.json");
        let cfg_raw = fs::read_to_string(&cfg).unwrap_or_default();
        let cfg_ok =
            cfg.is_file() && cfg_raw.contains("[mcp_servers.moraine]") && cfg_raw.contains("mcp");
        let abs_cli = cfg_ok
            && (cfg_raw.contains(current.display().to_string().as_str())
                || cfg_raw.contains("/.local/bin/moraine")
                || cfg_raw.contains("command = \""));
        if cfg_ok {
            details.push(format!("MCP config {}", cfg.display()));
            checks.push(check(
                "integration.codex.mcp",
                if abs_cli { "pass" } else { "warn" },
                "Moraine MCP server entry present",
                Some(&cfg.display().to_string()),
                Some("absolute moraine mcp --project"),
                (!abs_cli).then_some("moraine setup codex --project <path>"),
            ));
        } else {
            details.push("missing or incomplete .codex/config.toml moraine MCP entry".into());
            checks.push(check(
                "integration.codex.mcp",
                "fail",
                "missing Moraine MCP entry",
                Some(&cfg.display().to_string()),
                Some("managed [mcp_servers.moraine]"),
                Some("moraine setup codex --project <path>"),
            ));
        }

        let hooks_raw = fs::read_to_string(&hooks).unwrap_or_default();
        let hooks_ok = hooks.is_file() && hooks_raw.contains("hook-codex");
        if hooks_ok {
            details.push(format!("hooks {}", hooks.display()));
            checks.push(check(
                "integration.codex.hooks",
                "pass",
                "hooks include hook-codex",
                Some(&hooks.display().to_string()),
                Some("managed hook-codex handlers"),
                None,
            ));
        } else {
            details.push("missing or incomplete .codex/hooks.json".into());
            checks.push(check(
                "integration.codex.hooks",
                "fail",
                "hooks missing hook-codex",
                Some(&hooks.display().to_string()),
                Some("Moraine-managed hooks"),
                Some("moraine setup codex --project <path>"),
            ));
        }

        // Best-effort MCP tools/list via installed CLI (short timeout subprocess).
        if cfg_ok {
            let tools_probe = Command::new(&current)
                .args(["mcp", "--help"])
                .output()
                .ok()
                .map(|o| o.status.success())
                .unwrap_or(false);
            checks.push(check(
                "integration.codex.mcp_cli",
                if tools_probe { "pass" } else { "warn" },
                if tools_probe {
                    "moraine mcp subcommand available"
                } else {
                    "moraine mcp help failed"
                },
                None,
                Some("installed moraine mcp"),
                None,
            ));
        }

        IntegrationCheck {
            name: "codex".into(),
            configured: cfg_ok && hooks_ok,
            details,
        }
    });
    // integration checks already pushed above
    if let Some(ref ic) = integration {
        checks.push(check(
            "integration.codex",
            if ic.configured { "pass" } else { "fail" },
            if ic.configured {
                "Codex project integration looks configured".into()
            } else {
                ic.details.join("; ")
            },
            None,
            Some("MCP + hooks managed by Moraine"),
            (!ic.configured).then_some("moraine setup codex --project <path>"),
        ));
    }

    let ok = !checks.iter().any(|c| c.status == "fail");
    DoctorReport {
        ok,
        build,
        checks,
        project,
        integration,
    }
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn libc_uid() -> u32 {
    // Avoid libc crate: read from /proc or nix; use std only.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|x| x.parse().ok())
        })
        .unwrap_or(0)
}
