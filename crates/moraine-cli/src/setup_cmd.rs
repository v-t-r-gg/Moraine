//! Post-install `moraine setup` entry point (C2 §15).
//!
//! CLI automation path; normal users should use the desktop Enable Moraine wizard.
//! Prefer `moraine enable --project <path>` for a strict transactional setup.

use anyhow::Result;
use serde_json::json;

use crate::doctor;
use crate::service_cmd;
use crate::suite::{collect_version_report, SuitePaths};

/// Inspect suite, repair/install user unit, start service, report next steps.
pub fn setup_post_install(json: bool) -> Result<i32> {
    let suite = SuitePaths::discover();
    let ver = collect_version_report();
    let mut actions = Vec::new();
    let mut warnings = Vec::new();

    // Install/repair unit when suite service binary exists
    if suite.service.is_file() {
        match service_cmd::service_install(false) {
            Ok(()) => actions.push(format!("service unit → {}", suite.unit.display())),
            Err(e) => warnings.push(format!("service install: {e:#}")),
        }
        match service_cmd::service_start(false) {
            Ok(()) => actions.push("service start requested".into()),
            Err(e) => warnings.push(format!("service start: {e:#}")),
        }
    } else {
        warnings.push(format!(
            "suite service binary missing at {}; install a release bundle first",
            suite.service.display()
        ));
    }

    let doctor_report = doctor::run_doctor(None, None);
    let service_online =
        ver.service.online || crate::suite::http_get_loopback(33111, "/status").is_ok();

    // Structured inspect via shared control plane (same data desktop uses).
    let system = moraine_provision::inspect_default().ok();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": doctor_report.ok || service_online,
                "cli": {
                    "path": ver.cli.path,
                    "version": ver.cli.version,
                    "status": "healthy",
                },
                "service": {
                    "path": suite.service.display().to_string(),
                    "installed": suite.service.is_file(),
                    "online": service_online,
                    "unit": suite.unit.display().to_string(),
                },
                "desktop": {
                    "installed": suite.desktop.is_file(),
                    "path": suite.desktop.display().to_string(),
                },
                "suite": {
                    "manifest": suite.manifest.display().to_string(),
                    "share": suite.share.display().to_string(),
                },
                "system": system,
                "actions": actions,
                "warnings": warnings,
                "doctorOk": doctor_report.ok,
                "next": [
                    "Open Moraine desktop → Enable Moraine",
                    "or: moraine enable --project /path/to/repo --json",
                    "or: moraine self-test --project /path/to/repo --json",
                ],
            }))?
        );
    } else {
        println!("Moraine is installed.\n");
        println!("CLI:       {} ({})", ver.cli.version, ver.cli.path);
        println!(
            "Service:   {}",
            if service_online {
                "running"
            } else if suite.service.is_file() {
                "installed (not reachable yet — try: moraine service start)"
            } else {
                "missing"
            }
        );
        println!(
            "Desktop:   {}",
            if suite.desktop.is_file() {
                "installed"
            } else {
                "not in suite"
            }
        );
        println!("Data:      {}", suite.share.display());
        println!(
            "Runtime:   {}",
            crate::suite::default_socket_path().display()
        );
        for a in &actions {
            println!("  · {a}");
        }
        for w in &warnings {
            eprintln!("warning: {w}");
        }
        println!(
            "\nNext (normal path):\n  Open Moraine desktop → Enable Moraine\n\
Automation:\n  moraine enable --project /path/to/repo\n  moraine self-test --project /path/to/repo"
        );
    }

    // Setup is advisory-success when suite is present even if service is momentarily offline.
    Ok(if suite.cli.is_file() || ver.cli.version != "0.0.0" {
        0
    } else {
        1
    })
}
