//! Structured health checks with optional repair actions (doctor → UI).

use std::path::{Path, PathBuf};

use moraine_core::resolve_existing_project;

use crate::agent::adapter_for;
use crate::error::Result;
use crate::service::ServiceManager;
use crate::suite::SuitePaths;
use crate::types::{
    AgentKind, HealthCheck, HealthReport, HealthStatus, Readiness, RepairAction, RepairKind,
    RepairResult, SetupIntent,
};

pub fn health(
    service: &dyn ServiceManager,
    project: Option<&Path>,
    agent: Option<AgentKind>,
) -> Result<HealthReport> {
    let suite = SuitePaths::discover();
    let mut checks = Vec::new();

    // Suite CLI
    let cli = suite.absolute_cli();
    checks.push(HealthCheck {
        id: "suite.cli".into(),
        status: if cli.is_file() || std::env::current_exe().is_ok() {
            HealthStatus::Pass
        } else {
            HealthStatus::Fail
        },
        user_message: "Moraine program is available".into(),
        technical_detail: cli.display().to_string(),
        repair: None,
    });

    // Service
    let svc = service.inspect()?;
    if !svc.supported {
        checks.push(HealthCheck {
            id: "service.supported".into(),
            status: HealthStatus::Fail,
            user_message: if svc.backend == crate::types::BackgroundRuntimeBackend::Unsupported {
                "Background capture is not available on this platform".into()
            } else {
                "Background capture runtime is unavailable".into()
            },
            technical_detail: svc.status_message.clone(),
            repair: None,
        });
    } else if !svc.registration_present || !svc.registration_valid {
        // Registration is authoritative. A live diagnostics responder without a
        // valid product registration is not healthy (manual launch / port
        // collision must not report Ready).
        checks.push(HealthCheck {
            id: "service.installed".into(),
            status: HealthStatus::Fail,
            user_message: if !svc.registration_present {
                if svc.binary_present {
                    "Background capture is not registered".into()
                } else {
                    "Background capture is not set up".into()
                }
            } else {
                "Background capture registration needs repair".into()
            },
            technical_detail: svc.status_message.clone(),
            repair: Some(RepairAction {
                id: "repair.install_service".into(),
                label: "Fix".into(),
                kind: RepairKind::InstallService,
                project: None,
                agent: None,
            }),
        });
    } else if svc.running && svc.diagnostics_ready && svc.capture_ready {
        checks.push(HealthCheck {
            id: "service.running".into(),
            status: HealthStatus::Pass,
            user_message: "Background capture is running".into(),
            technical_detail: svc.status_message.clone(),
            repair: None,
        });
    } else if svc.running {
        checks.push(HealthCheck {
            id: "service.capture".into(),
            status: HealthStatus::Fail,
            user_message: "Background capture is running but unhealthy".into(),
            technical_detail: svc.status_message.clone(),
            repair: Some(RepairAction {
                id: "repair.restart_service".into(),
                label: "Fix".into(),
                kind: RepairKind::RestartService,
                project: None,
                agent: None,
            }),
        });
    } else {
        // Valid registration but stopped → Start.
        checks.push(HealthCheck {
            id: "service.running".into(),
            status: HealthStatus::Fail,
            user_message: "Background capture is not running".into(),
            technical_detail: svc.status_message.clone(),
            repair: Some(RepairAction {
                id: "repair.start_service".into(),
                label: "Fix".into(),
                kind: RepairKind::StartService,
                project: None,
                agent: None,
            }),
        });
    }

    // Project
    if let Some(proj) = project {
        let init = resolve_existing_project(Some(proj)).is_ok();
        checks.push(HealthCheck {
            id: "project.initialized".into(),
            status: if init {
                HealthStatus::Pass
            } else {
                HealthStatus::Fail
            },
            user_message: if init {
                "Project ledger is healthy".into()
            } else {
                "Project is not set up for Moraine yet".into()
            },
            technical_detail: proj.display().to_string(),
            repair: (!init).then(|| RepairAction {
                id: "repair.init_project".into(),
                label: "Fix".into(),
                kind: RepairKind::InitProject,
                project: Some(proj.to_path_buf()),
                agent: None,
            }),
        });

        let kind = agent.unwrap_or(AgentKind::Codex);
        let adapter = adapter_for(kind);
        if let Ok(state) = adapter.inspect(proj) {
            if state.configured && !state.needs_repair {
                checks.push(HealthCheck {
                    id: "agent.integration".into(),
                    status: HealthStatus::Pass,
                    user_message: format!("{} integration is healthy", adapter.display_name()),
                    technical_detail: state.details.join("; "),
                    repair: None,
                });
            } else {
                checks.push(HealthCheck {
                    id: "agent.integration".into(),
                    status: HealthStatus::Fail,
                    user_message: format!("{} integration needs repair", adapter.display_name()),
                    technical_detail: state.details.join("; "),
                    repair: svc.supported.then(|| RepairAction {
                        id: "repair.agent".into(),
                        label: "Fix".into(),
                        kind: RepairKind::RepairAgentIntegration,
                        project: Some(proj.to_path_buf()),
                        agent: Some(kind),
                    }),
                });
            }
        }
    }

    let has_fail = checks.iter().any(|c| c.status == HealthStatus::Fail);
    let ok = !has_fail;
    Ok(HealthReport {
        ok,
        checks,
        readiness: if ok {
            Readiness::Ready
        } else {
            Readiness::Degraded
        },
    })
}

pub fn health_default(project: Option<&Path>, agent: Option<AgentKind>) -> Result<HealthReport> {
    let svc = crate::service::default_service_manager();
    health(svc.as_ref(), project, agent)
}

pub fn repair(action: &RepairAction, service: &dyn ServiceManager) -> Result<RepairResult> {
    if matches!(action.kind, RepairKind::InitProject) {
        let path = action.project.clone().unwrap_or_else(|| PathBuf::from("."));
        return match moraine_core::init_project(Some(&path)).and_then(|result| {
            moraine_core::register_project_root(&result.project_root)?;
            Ok(result)
        }) {
            Ok(_) => Ok(RepairResult {
                ok: true,
                action_id: action.id.clone(),
                user_message: "Project is ready".into(),
                technical_detail: None,
            }),
            Err(e) => Ok(RepairResult {
                ok: false,
                action_id: action.id.clone(),
                user_message: "Could not prepare project".into(),
                technical_detail: Some(e.to_string()),
            }),
        };
    }

    let runtime_state = service.inspect()?;
    if !runtime_state.supported {
        return Ok(RepairResult {
            ok: false,
            action_id: action.id.clone(),
            user_message: "Background capture is not available on this platform".into(),
            technical_detail: Some(
                if runtime_state.backend == crate::types::BackgroundRuntimeBackend::Unsupported {
                    "unsupported_platform"
                } else {
                    "runtime_unavailable"
                }
                .into(),
            ),
        });
    }
    match action.kind {
        RepairKind::StartService => {
            match service.start().and_then(|_| require_runtime_ready(service)) {
                Ok(()) => Ok(RepairResult {
                    ok: true,
                    action_id: action.id.clone(),
                    user_message: "Background capture started".into(),
                    technical_detail: None,
                }),
                Err(e) => Ok(RepairResult {
                    ok: false,
                    action_id: action.id.clone(),
                    user_message: "Could not start background capture".into(),
                    technical_detail: Some(e.to_string()),
                }),
            }
        }
        RepairKind::InstallService => {
            let suite = SuitePaths::discover();
            let bin = suite.absolute_service().or_else(|| {
                std::env::current_exe().ok().and_then(|e| {
                    e.parent()
                        .map(|p| {
                            p.join(moraine_platform::executable_name(
                                moraine_platform::HostPlatform::current(),
                                moraine_platform::SuiteComponent::Service,
                            ))
                        })
                        .filter(|p| p.is_file())
                })
            });
            match bin {
                Some(b) => match repair_runtime_registration(service, &b) {
                    Ok(()) => Ok(RepairResult {
                        ok: true,
                        action_id: action.id.clone(),
                        user_message: "Background capture installed and started".into(),
                        technical_detail: None,
                    }),
                    Err(e) => Ok(RepairResult {
                        ok: false,
                        action_id: action.id.clone(),
                        user_message: if e.to_string().contains("manual restoration required") {
                            "Background capture repair requires manual restoration".into()
                        } else {
                            "Could not install background capture".into()
                        },
                        technical_detail: Some(e.to_string()),
                    }),
                },
                None => Ok(RepairResult {
                    ok: false,
                    action_id: action.id.clone(),
                    user_message: "Moraine service program is missing".into(),
                    technical_detail: Some("no service binary in suite".into()),
                }),
            }
        }
        RepairKind::RestartService => match service
            .restart()
            .and_then(|_| require_runtime_ready(service))
        {
            Ok(()) => Ok(RepairResult {
                ok: true,
                action_id: action.id.clone(),
                user_message: "Background capture restarted".into(),
                technical_detail: None,
            }),
            Err(e) => Ok(RepairResult {
                ok: false,
                action_id: action.id.clone(),
                user_message: "Could not restart background capture".into(),
                technical_detail: Some(e.to_string()),
            }),
        },
        RepairKind::InitProject => {
            unreachable!("portable project repair returns before inspection")
        }
        RepairKind::RepairAgentIntegration => {
            let path = action.project.clone().unwrap_or_else(|| PathBuf::from("."));
            let kind = action.agent.unwrap_or(AgentKind::Codex);
            let intent = SetupIntent {
                project: path,
                agent: kind,
                enable_autostart: false,
                skip_service: true,
            };
            match crate::plan::plan(intent.clone(), service) {
                Ok(p) => {
                    // Only run configure_agent (+ init if needed).
                    let filtered = crate::types::SetupPlan {
                        plan_id: p.plan_id,
                        intent: p.intent,
                        operations: p
                            .operations
                            .into_iter()
                            .filter(|o| {
                                matches!(
                                    o.kind,
                                    crate::types::ProvisionOpKind::InitializeProject
                                        | crate::types::ProvisionOpKind::RegisterProject
                                        | crate::types::ProvisionOpKind::ConfigureAgent
                                )
                            })
                            .collect(),
                        warnings: p.warnings,
                        absolute_cli: p.absolute_cli,
                        product_summary: p.product_summary,
                        state_witness: p.state_witness,
                    };
                    match crate::apply::apply(filtered, service) {
                        Ok(outcome) if outcome.is_success() => Ok(RepairResult {
                            ok: true,
                            action_id: action.id.clone(),
                            user_message: "Agent connection repaired".into(),
                            technical_detail: None,
                        }),
                        Ok(outcome) => Ok(RepairResult {
                            ok: false,
                            action_id: action.id.clone(),
                            user_message: "Agent repair incomplete".into(),
                            technical_detail: outcome.receipt().error.clone(),
                        }),
                        Err(e) => Ok(RepairResult {
                            ok: false,
                            action_id: action.id.clone(),
                            user_message: "Agent repair failed".into(),
                            technical_detail: Some(e.to_string()),
                        }),
                    }
                }
                Err(e) => Ok(RepairResult {
                    ok: false,
                    action_id: action.id.clone(),
                    user_message: "Could not plan agent repair".into(),
                    technical_detail: Some(e.to_string()),
                }),
            }
        }
    }
}

fn repair_runtime_registration(service: &dyn ServiceManager, executable: &Path) -> Result<()> {
    let probe = crate::service_ready::default_service_probe();
    repair_runtime_registration_with_probe(service, executable, probe.as_ref())
}

fn repair_runtime_registration_with_probe(
    service: &dyn ServiceManager,
    executable: &Path,
    probe: &dyn crate::service_ready::ServiceProbe,
) -> Result<()> {
    let prestate = crate::runtime::capture_runtime_prestate(service)?;
    let result = (|| {
        let spec = crate::runtime::RuntimeInstallSpec::try_discover(executable)?;
        service.install_runtime(&spec)?;
        if prestate.state.autostart_enabled {
            service.enable_autostart()?;
        }
        service.start()?;
        require_runtime_ready_with_probe(service, probe)
    })();

    match result {
        Ok(()) => Ok(()),
        Err(error) => match crate::runtime::restore_runtime_prestate(service, &prestate) {
            Ok(()) => Err(error),
            Err(restoration) => Err(crate::ProvisionError::Service(format!(
                "{error}; manual restoration required: {restoration}"
            ))),
        },
    }
}

fn require_runtime_ready(service: &dyn ServiceManager) -> Result<()> {
    let probe = crate::service_ready::default_service_probe();
    require_runtime_ready_with_probe(service, probe.as_ref())
}

fn require_runtime_ready_with_probe(
    service: &dyn ServiceManager,
    probe: &dyn crate::service_ready::ServiceProbe,
) -> Result<()> {
    let state = service.inspect()?;
    if state.diagnostics_ready && state.capture_ready {
        return Ok(());
    }
    let readiness = probe.wait_ready(crate::service_ready::default_service_ready_timeout_ms());
    if readiness.ready {
        Ok(())
    } else {
        Err(crate::ProvisionError::Service(readiness.message))
    }
}

pub fn repair_default(action: &RepairAction) -> Result<RepairResult> {
    let svc = crate::service::default_service_manager();
    repair(action, svc.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::MemoryRuntimeManager;
    use crate::service_ready::AlwaysOfflineProbe;

    #[test]
    fn failed_registration_repair_restores_exact_prestate() {
        let temp = tempfile::tempdir().unwrap();
        let unit = temp.path().join("moraine.service");
        let executable = temp.path().join("moraine-service");
        std::fs::write(&executable, b"service").unwrap();
        let runtime = MemoryRuntimeManager::with_unit_path(unit.clone());
        runtime.install(&executable).unwrap();
        runtime.enable_autostart().unwrap();
        runtime.stop().unwrap();
        runtime.set_endpoint_ready_override(Some(false));
        std::fs::write(&unit, b"exact registration before repair").unwrap();
        let fingerprint = runtime.registration_fingerprint().unwrap();

        let error =
            repair_runtime_registration_with_probe(&runtime, &executable, &AlwaysOfflineProbe)
                .unwrap_err();

        assert!(error.to_string().contains("not ready"));
        assert_eq!(runtime.registration_fingerprint().unwrap(), fingerprint);
        let restored = runtime.inspect().unwrap();
        assert!(!restored.running);
        assert!(restored.autostart_enabled);
    }

    #[test]
    fn project_initialization_does_not_inspect_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let runtime = MemoryRuntimeManager::new();
        runtime.fail_inspect_after(0, "runtime must not be inspected");
        let action = RepairAction {
            id: "init".into(),
            label: "Fix".into(),
            kind: RepairKind::InitProject,
            project: Some(project.clone()),
            agent: None,
        };

        let result = moraine_core::project_registry::with_project_registry_path_override(
            temp.path().join("projects.json"),
            || repair(&action, &runtime).unwrap(),
        );

        assert!(result.ok);
        assert_eq!(runtime.inspect_count(), 0);
        assert!(project.join(".moraine").is_dir());
    }

    #[test]
    fn health_fails_when_diagnostics_ready_without_registration() {
        let runtime = MemoryRuntimeManager::new();
        // Simulate a live diagnostics responder (manual launch / port collision)
        // without product Task Scheduler / systemd registration.
        runtime.simulate_orphan_endpoint(true);

        let report = health(&runtime, None, None).unwrap();
        let service = report
            .checks
            .iter()
            .find(|check| check.id.starts_with("service."))
            .expect("service health check");

        assert_eq!(service.status, HealthStatus::Fail);
        assert_eq!(service.id, "service.installed");
        let repair = service.repair.as_ref().expect("install repair");
        assert_eq!(repair.kind, RepairKind::InstallService);
        assert!(
            !report
                .checks
                .iter()
                .any(|check| check.id == "service.running" && check.status == HealthStatus::Pass),
            "orphan diagnostics must not report healthy running capture"
        );
    }
}
