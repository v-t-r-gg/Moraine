//! `moraine doctor` health report (C2).

use std::fs;
use std::path::{Path, PathBuf};

use moraine_core::{resolve_existing_project, BuildIdentity};
use serde::Serialize;

use crate::suite::{collect_version_report, enumerate_moraine_on_path, SuitePaths};

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
    pub status: String,
    pub message: String,
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

fn check(id: &str, ok: bool, message: impl Into<String>, remediation: Option<&str>) -> DoctorCheck {
    DoctorCheck {
        id: id.into(),
        status: if ok { "ok" } else { "fail" }.into(),
        message: message.into(),
        remediation: remediation.map(|s| s.into()),
    }
}

pub fn run_doctor(project: Option<&Path>, integration: Option<&str>) -> DoctorReport {
    let build = BuildIdentity::current();
    let suite = SuitePaths::discover();
    let ver = collect_version_report();
    let mut checks = Vec::new();

    // Suite
    let manifest_ok = suite.manifest.is_file();
    checks.push(check(
        "suite.manifest",
        manifest_ok,
        if manifest_ok {
            format!("manifest at {}", suite.manifest.display())
        } else {
            "suite manifest missing (development binary or incomplete install)".into()
        },
        Some("Install with the release bundle install.sh or moraine setup"),
    ));

    if let Some(m) = suite.read_manifest() {
        checks.push(check(
            "suite.components",
            m.components_coherent(),
            format!(
                "suite version {} (cli={}, service={}, desktop={})",
                m.version, m.components.cli, m.components.service, m.components.desktop
            ),
            (!m.components_coherent()).then_some("Reinstall a coherent release bundle"),
        ));
        checks.push(check(
            "suite.cli_match",
            m.version == build.version,
            format!("running CLI {} vs suite {}", build.version, m.version),
            (m.version != build.version).then_some("Ensure PATH prefers ~/.local/bin/moraine"),
        ));
    }

    // PATH drift
    let path_exes = enumerate_moraine_on_path();
    let cargo_shadow = path_exes
        .iter()
        .any(|p| p.to_string_lossy().contains(".cargo/bin"));
    checks.push(check(
        "path.count",
        path_exes.len() <= 1 || !cargo_shadow,
        format!(
            "{} moraine executable(s) on PATH: {}",
            path_exes.len(),
            path_exes
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        cargo_shadow
            .then_some("Remove or deprioritize ~/.cargo/bin on PATH; use the installed suite CLI"),
    ));

    // Service binary + unit
    checks.push(check(
        "service.binary",
        suite.service.is_file() || !manifest_ok,
        if suite.service.is_file() {
            format!("service binary {}", suite.service.display())
        } else if manifest_ok {
            "suite manifest present but service binary missing".into()
        } else {
            "no installed service binary (ok for pure cargo dev)".into()
        },
        Some("Re-run install.sh or moraine service install"),
    ));

    let unit_exists = suite.unit.is_file();
    let unit_points_cargo = unit_exists
        && fs::read_to_string(&suite.unit)
            .map(|s| s.contains(".cargo/bin"))
            .unwrap_or(false);
    checks.push(check(
        "service.unit",
        unit_exists && !unit_points_cargo,
        if !unit_exists {
            "systemd user unit not installed".into()
        } else if unit_points_cargo {
            format!(
                "unit {} points at ~/.cargo/bin (development drift)",
                suite.unit.display()
            )
        } else {
            format!("unit {}", suite.unit.display())
        },
        Some("moraine service install"),
    ));

    // Service online
    checks.push(check(
        "service.online",
        ver.service.online,
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
        Some("moraine service start"),
    ));

    if ver.service.online && !ver.service.compatible {
        checks.push(check(
            "service.version_compatible",
            false,
            "service version does not match CLI",
            Some("Reinstall coherent suite and restart service"),
        ));
    }

    // Desktop
    checks.push(check(
        "desktop.binary",
        suite.desktop.is_file() || !manifest_ok,
        if suite.desktop.is_file() {
            format!("desktop {}", suite.desktop.display())
        } else if manifest_ok {
            "desktop binary missing from suite".into()
        } else {
            "no installed desktop (dev mode ok)".into()
        },
        Some("Install release bundle including moraine-app"),
    ));

    // Socket / spool dirs (informational)
    let sock = crate::suite::default_socket_path();
    checks.push(check(
        "service.socket_path",
        true,
        format!("expected unix socket {}", sock.display()),
        None,
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
            pc.initialized,
            if pc.initialized {
                format!("project {} ready", pc.path)
            } else {
                pc.message
                    .clone()
                    .unwrap_or_else(|| "project not initialized".into())
            },
            (!pc.initialized).then_some("moraine project init"),
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
        let cfg = root.join(".codex/config.toml");
        let hooks = root.join(".codex/hooks.json");
        let cfg_ok = cfg.is_file()
            && fs::read_to_string(&cfg)
                .map(|s| s.contains("moraine") && s.contains("mcp"))
                .unwrap_or(false);
        let hooks_ok = hooks.is_file()
            && fs::read_to_string(&hooks)
                .map(|s| s.contains("hook-codex"))
                .unwrap_or(false);
        if cfg_ok {
            details.push(format!("MCP config {}", cfg.display()));
        } else {
            details.push("missing or incomplete .codex/config.toml moraine MCP entry".into());
        }
        if hooks_ok {
            details.push(format!("hooks {}", hooks.display()));
        } else {
            details.push("missing or incomplete .codex/hooks.json".into());
        }
        IntegrationCheck {
            name: "codex".into(),
            configured: cfg_ok && hooks_ok,
            details,
        }
    });
    if let Some(ref ic) = integration {
        checks.push(check(
            "integration.codex",
            ic.configured,
            if ic.configured {
                "Codex project integration looks configured".into()
            } else {
                ic.details.join("; ")
            },
            (!ic.configured).then_some("moraine setup codex --project <path>"),
        ));
    }

    let ok = checks.iter().all(|c| c.status == "ok");
    DoctorReport {
        ok,
        build,
        checks,
        project,
        integration,
    }
}
