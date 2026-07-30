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
    } else if svc.running && svc.diagnostics_ready && svc.capture_ready {
        checks.push(HealthCheck {
            id: "service.running".into(),
            status: HealthStatus::Pass,
            user_message: "Background capture is running".into(),
            technical_detail: svc.status_message.clone(),
            repair: None,
        });
    } else if !svc.registration_present || !svc.registration_valid {
        // Missing or invalid registration → Install/repair (not Start).
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
    let runtime_state = service.inspect()?;
    if !matches!(action.kind, RepairKind::InitProject) && !runtime_state.supported {
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
                Some(b) => match crate::runtime::RuntimeInstallSpec::try_discover(b)
                    .and_then(|spec| service.install_runtime(&spec))
                    .and_then(|_| service.start())
                    .and_then(|_| require_runtime_ready(service))
                {
                    Ok(()) => Ok(RepairResult {
                        ok: true,
                        action_id: action.id.clone(),
                        user_message: "Background capture installed and started".into(),
                        technical_detail: None,
                    }),
                    Err(e) => Ok(RepairResult {
                        ok: false,
                        action_id: action.id.clone(),
                        user_message: "Could not install background capture".into(),
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
            let path = action.project.clone().unwrap_or_else(|| PathBuf::from("."));
            match moraine_core::init_project(Some(&path)).and_then(|result| {
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
            }
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

fn require_runtime_ready(service: &dyn ServiceManager) -> Result<()> {
    let state = service.inspect()?;
    if state.diagnostics_ready && state.capture_ready {
        return Ok(());
    }
    let readiness = crate::service_ready::default_service_probe()
        .wait_ready(crate::service_ready::default_service_ready_timeout_ms());
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
