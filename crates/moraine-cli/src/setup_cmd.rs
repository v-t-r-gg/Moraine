//! Post-install `moraine setup` entry point.
//!
//! CLI automation path; normal users should use the desktop Enable Moraine wizard.
//! Prefer `moraine enable --project <path>` for a strict transactional setup.

use anyhow::Result;
use moraine_provision::BackgroundRuntimeManager;
use serde_json::json;

use crate::doctor;
use crate::suite::{collect_version_report, SuitePaths};

/// Inspect suite, repair/install user unit, start service, report next steps.
pub fn setup_post_install(json: bool) -> Result<i32> {
    let capabilities = moraine_platform::PlatformCapabilities::current();
    if !capabilities.product_ready_supported() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "code": "unsupported_platform",
                    "platform": capabilities.host,
                    "operation": "product_setup",
                    "message": format!(
                        "Moraine background capture is not supported on {:?} yet",
                        capabilities.host
                    ),
                    "capabilities": capabilities,
                }))?
            );
        } else {
            eprintln!(
                "unsupported_platform: Moraine background capture is not supported on {:?} yet",
                capabilities.host
            );
        }
        return Ok(1);
    }

    let runtime = moraine_provision::default_background_runtime_manager();
    let initial_state = runtime.inspect()?;
    if let Err(error) = moraine_provision::ensure_background_runtime_available(
        &initial_state,
        capabilities.host,
        "product_setup",
    ) {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "code": "runtime_unavailable",
                    "platform": capabilities.host,
                    "operation": "product_setup",
                    "message": error.to_string(),
                    "capabilities": capabilities,
                }))?
            );
        } else {
            eprintln!("{error}");
        }
        return Ok(1);
    }

    let suite = SuitePaths::discover();
    let ver = collect_version_report();
    let mut actions = Vec::new();
    let mut warnings = Vec::new();

    // Install/repair unit when suite service binary exists
    if suite.service.is_file() {
        let registration = runtime.capture_registration()?;
        let spec = moraine_provision::RuntimeInstallSpec::try_discover(suite.service.clone())?;
        let setup_result = (|| -> moraine_provision::Result<()> {
            runtime.install_runtime(&spec)?;
            runtime.start()?;
            let readiness = moraine_provision::default_service_probe()
                .wait_ready(moraine_provision::default_service_ready_timeout_ms());
            if !readiness.ready {
                return Err(moraine_provision::ProvisionError::Service(
                    readiness.message,
                ));
            }
            Ok(())
        })();
        match setup_result {
            Ok(()) => actions.push(
                suite
                    .service_registration
                    .as_ref()
                    .map(|path| format!("service unit → {}", path.display()))
                    .unwrap_or_else(|| "background runtime registration installed".into()),
            ),
            Err(error) => {
                let restoration =
                    restore_runtime_prestate(runtime.as_ref(), &registration, &initial_state);
                warnings.push(format!("background runtime setup: {error}"));
                if let Err(restore_error) = restoration {
                    warnings.push(format!(
                        "runtime restoration requires attention: {restore_error}"
                    ));
                }
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "ok": false,
                            "code": "runtime_setup_failed",
                            "operation": "product_setup",
                            "message": error.to_string(),
                            "warnings": warnings,
                        }))?
                    );
                } else {
                    eprintln!("Background runtime setup failed: {error}");
                }
                return Ok(1);
            }
        }
        actions.push("background capture is ready".into());
    } else {
        warnings.push(format!(
            "suite service binary missing at {}; install a release bundle first",
            suite.service.display()
        ));
    }

    let doctor_report = doctor::run_doctor(None, None);
    let service_online = moraine_provision::default_background_runtime_manager()
        .inspect()
        .map(|state| state.diagnostics_ready && state.capture_ready)
        .unwrap_or(false);

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
                    "unit": suite.service_registration.as_ref().map(|path| path.display().to_string()),
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
        let runtime = match crate::suite::capture_endpoint() {
            moraine_platform::CaptureEndpoint::UnixSocket(path) => path.display().to_string(),
            moraine_platform::CaptureEndpoint::WindowsNamedPipe(name) => name,
            moraine_platform::CaptureEndpoint::Unsupported => "unsupported".into(),
        };
        println!("Runtime:   {runtime}");
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

fn restore_runtime_prestate(
    runtime: &dyn BackgroundRuntimeManager,
    registration: &moraine_provision::RuntimeRegistrationSnapshot,
    state: &moraine_provision::BackgroundRuntimeState,
) -> moraine_provision::Result<()> {
    runtime.stop()?;
    runtime.restore_registration(registration)?;
    if state.autostart_enabled {
        runtime.enable_autostart()?;
    } else if state.registration_present {
        runtime.disable_autostart()?;
    }
    if state.running {
        runtime.start()?;
    }
    Ok(())
}
