//! Post-install `moraine setup` entry point.
//!
//! CLI automation path; normal users should use the desktop Enable Moraine wizard.
//! Prefer `moraine enable --project <path>` for a strict transactional setup.

use anyhow::Result;
use moraine_provision::BackgroundRuntimeManager;
use serde_json::json;
use std::path::Path;

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
        match setup_runtime(runtime.as_ref(), &suite.service) {
            Ok(registration_changed) => {
                if registration_changed {
                    actions.push(
                        suite
                            .service_registration
                            .as_ref()
                            .map(|path| format!("service unit → {}", path.display()))
                            .unwrap_or_else(|| "background runtime registration installed".into()),
                    );
                } else {
                    actions.push("background runtime registration already valid".into());
                }
            }
            Err(error) => {
                warnings.push(format!("background runtime setup: {error}"));
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

fn setup_runtime(
    runtime: &dyn BackgroundRuntimeManager,
    executable: &Path,
) -> moraine_provision::Result<bool> {
    let probe = moraine_provision::default_service_probe();
    setup_runtime_with_probe(runtime, executable, probe.as_ref())
}

fn setup_runtime_with_probe(
    runtime: &dyn BackgroundRuntimeManager,
    executable: &Path,
    probe: &dyn moraine_provision::ServiceProbe,
) -> moraine_provision::Result<bool> {
    let prestate = moraine_provision::capture_runtime_prestate(runtime)?;
    let registration_changed = !prestate.state.registration_valid;
    let result = (|| {
        if registration_changed {
            let spec = moraine_provision::RuntimeInstallSpec::try_discover(executable)?;
            runtime.install_runtime(&spec)?;
            if prestate.state.autostart_enabled {
                runtime.enable_autostart()?;
            }
        }

        if prestate.state.running {
            if !(prestate.state.diagnostics_ready && prestate.state.capture_ready) {
                runtime.restart()?;
            }
        } else {
            runtime.start()?;
        }

        let readiness = probe.wait_ready(moraine_provision::default_service_ready_timeout_ms());
        if !readiness.ready {
            return Err(moraine_provision::ProvisionError::Service(
                readiness.message,
            ));
        }
        Ok(())
    })();

    match result {
        Ok(()) => Ok(registration_changed),
        Err(error) => match moraine_provision::restore_runtime_prestate(runtime, &prestate) {
            Ok(()) => Err(error),
            Err(restoration) => Err(moraine_provision::ProvisionError::Service(format!(
                "{error}; manual restoration required: {restoration}"
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moraine_provision::{
        AlwaysOfflineProbe, AlwaysReadyProbe, BackgroundRuntimeManager, MemoryRuntimeManager,
    };

    #[test]
    fn setup_keeps_valid_registration_and_enabled_autostart() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("moraine-service");
        std::fs::write(&executable, b"service").unwrap();
        let unit = temp.path().join("moraine.service");
        let runtime = MemoryRuntimeManager::with_unit_path(unit);
        runtime.install(&executable).unwrap();
        runtime.enable_autostart().unwrap();
        runtime.start().unwrap();
        let before = runtime.registration_fingerprint().unwrap();
        let counts_before = runtime.operation_counts();

        let changed =
            setup_runtime_with_probe(&runtime, &executable, &AlwaysReadyProbe { version: None })
                .unwrap();

        assert!(!changed);
        assert!(runtime.inspect().unwrap().autostart_enabled);
        assert_eq!(runtime.registration_fingerprint().unwrap(), before);
        assert_eq!(runtime.operation_counts().0, counts_before.0);
    }

    #[test]
    fn failed_setup_restores_exact_registration_absence() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("moraine-service");
        std::fs::write(&executable, b"service").unwrap();
        let unit = temp.path().join("moraine.service");
        let runtime = MemoryRuntimeManager::with_unit_path(unit.clone());

        let error =
            setup_runtime_with_probe(&runtime, &executable, &AlwaysOfflineProbe).unwrap_err();

        assert!(error.to_string().contains("not ready"));
        assert!(!unit.exists());
        assert_eq!(runtime.registration_fingerprint().unwrap(), None);
        assert!(!runtime.inspect().unwrap().running);
    }
}
