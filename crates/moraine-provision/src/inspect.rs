//! System state inspection.

use std::path::{Path, PathBuf};

use moraine_core::resolve_existing_project;

use crate::agent::all_adapters;
use crate::error::Result;
use crate::service::ServiceManager;
use crate::suite::{SuitePaths, SuiteState};
use crate::types::{
    AgentKind, DetectedAgent, ProjectCandidate, Readiness, ServiceState, SystemState,
};

/// Inspect suite, service, agents, and optional project candidates under scan roots.
pub fn inspect(service: &dyn ServiceManager, scan_roots: &[PathBuf]) -> Result<SystemState> {
    let suite_paths = SuitePaths::discover();
    let suite = inspect_suite(&suite_paths);
    let service_state = service.inspect()?;
    let agents = inspect_agents()?;
    let mut projects = Vec::new();
    for root in scan_roots {
        projects.extend(scan_project_candidates(root, 3)?);
    }
    let platform = moraine_platform::PlatformCapabilities::current();
    let readiness =
        derive_platform_readiness(&platform, &suite, &service_state, &agents, &projects);
    Ok(SystemState {
        platform,
        suite,
        service: service_state,
        agents,
        projects,
        readiness,
    })
}

/// Convenience: default service manager + no scan roots.
pub fn inspect_default() -> Result<SystemState> {
    let svc = crate::service::default_service_manager();
    let roots = moraine_core::registered_project_roots().unwrap_or_default();
    inspect(svc.as_ref(), &roots)
}

pub fn inspect_suite(paths: &SuitePaths) -> SuiteState {
    let manifest = paths.read_manifest();
    let version = manifest.as_ref().map(|m| m.version.clone());
    let coherent = manifest
        .as_ref()
        .map(|m| m.components_coherent())
        .unwrap_or(true);
    SuiteState {
        prefix: paths.prefix.display().to_string(),
        cli_path: paths.absolute_cli().display().to_string(),
        cli_present: paths.cli.is_file() || std::env::current_exe().is_ok(),
        service_path: paths.service.display().to_string(),
        service_present: paths.absolute_service().is_some(),
        desktop_path: paths.desktop.display().to_string(),
        desktop_present: paths.desktop.is_file(),
        manifest_path: paths.manifest.display().to_string(),
        manifest_present: paths.manifest.is_file(),
        version,
        components_coherent: coherent,
    }
}

fn inspect_agents() -> Result<Vec<DetectedAgent>> {
    let mut out = Vec::new();
    for adapter in all_adapters() {
        let d = adapter.detect()?;
        out.push(DetectedAgent {
            kind: d.kind,
            id: adapter.id().into(),
            display_name: adapter.display_name().into(),
            detected: d.detected,
            executable: d.executable,
            version: d.version,
            status: d.status,
            status_message: d.status_message,
        });
    }
    Ok(out)
}

fn scan_project_candidates(root: &Path, max_depth: usize) -> Result<Vec<ProjectCandidate>> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    // Direct root itself.
    push_candidate(&mut out, root);
    if max_depth == 0 {
        return Ok(out);
    }
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for ent in entries.flatten() {
        let p = ent.path();
        if !p.is_dir() {
            continue;
        }
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if p.join(".git").exists() || p.join(".moraine").is_dir() {
            push_candidate(&mut out, &p);
        }
    }
    Ok(out)
}

fn push_candidate(out: &mut Vec<ProjectCandidate>, path: &Path) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let initialized = resolve_existing_project(Some(path)).is_ok();
    let (integration_configured, integration_needs_repair) = if initialized {
        let mut configured = false;
        let mut needs_repair = false;
        for adapter in crate::agent::all_adapters() {
            match adapter.inspect(path) {
                Ok(state) => {
                    if state.configured {
                        configured = true;
                    }
                    // Surface repair when any adapter is partial or drifted.
                    needs_repair = needs_repair || state.needs_repair;
                }
                Err(_) => needs_repair = true,
            }
        }
        (configured, needs_repair)
    } else {
        (false, false)
    };
    let is_git = path.join(".git").exists();
    // Avoid duplicates
    let s = path.display().to_string();
    if out.iter().any(|c| c.path == s) {
        return;
    }
    out.push(ProjectCandidate {
        path: s,
        name,
        initialized,
        is_git,
        integration_configured,
        integration_needs_repair,
    });
}

fn derive_platform_readiness(
    platform: &moraine_platform::PlatformCapabilities,
    suite: &SuiteState,
    service: &ServiceState,
    agents: &[DetectedAgent],
    projects: &[ProjectCandidate],
) -> Readiness {
    if !platform.product_ready_supported() {
        return Readiness::NotConfigured;
    }
    derive_readiness(suite, service, agents, projects)
}

fn derive_readiness(
    suite: &SuiteState,
    service: &ServiceState,
    agents: &[DetectedAgent],
    projects: &[ProjectCandidate],
) -> Readiness {
    if !suite.cli_present && !suite.manifest_present {
        return Readiness::NotConfigured;
    }
    if service.registration_valid
        && service.running
        && service.diagnostics_ready
        && service.capture_ready
        && agents.iter().any(|a| a.detected)
        && projects.iter().any(|project| {
            project.initialized
                && project.integration_configured
                && !project.integration_needs_repair
        })
    {
        return Readiness::Ready;
    }
    if service.installed || suite.service_present {
        return Readiness::Degraded;
    }
    Readiness::NotConfigured
}

/// Build DetectedAgent list for a single kind (used by wizard).
pub fn detect_agent(kind: AgentKind) -> Result<DetectedAgent> {
    let adapter = crate::agent::adapter_for(kind);
    let d = adapter.detect()?;
    Ok(DetectedAgent {
        kind: d.kind,
        id: adapter.id().into(),
        display_name: adapter.display_name().into(),
        detected: d.detected,
        executable: d.executable,
        version: d.version,
        status: d.status,
        status_message: d.status_message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_environment() -> (SuiteState, ServiceState, Vec<DetectedAgent>) {
        (
            SuiteState {
                prefix: "/tmp".into(),
                cli_path: "/tmp/moraine".into(),
                cli_present: true,
                service_path: "/tmp/moraine-service".into(),
                service_present: true,
                desktop_path: "/tmp/moraine-app".into(),
                desktop_present: true,
                manifest_path: "/tmp/manifest.json".into(),
                manifest_present: true,
                version: Some("0.1.0".into()),
                components_coherent: true,
            },
            ServiceState {
                backend: crate::types::BackgroundRuntimeBackend::MemoryTest,
                supported: true,
                installed: true,
                binary_present: true,
                registration_present: true,
                registration_valid: true,
                running: true,
                autostart_enabled: true,
                endpoint_ready: true,
                diagnostics_ready: true,
                capture_ready: true,
                binary_path: Some("/tmp/moraine-service".into()),
                unit_path: Some("/tmp/moraine-service.service".into()),
                version: Some("0.1.0".into()),
                last_result: None,
                status_message: "ready".into(),
                platform: "test".into(),
                registration: None,
            },
            vec![DetectedAgent {
                kind: AgentKind::Codex,
                id: "codex".into(),
                display_name: "Codex".into(),
                detected: true,
                executable: Some("/tmp/codex".into()),
                version: Some("test".into()),
                status: "readyToConnect".into(),
                status_message: "detected".into(),
            }],
        )
    }

    #[test]
    fn initialized_registered_project_without_codex_config_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        moraine_core::init_project(Some(dir.path())).unwrap();
        let mut projects = Vec::new();
        push_candidate(&mut projects, dir.path());
        assert_eq!(projects.len(), 1);
        assert!(projects[0].initialized);
        assert!(!projects[0].integration_configured);

        let (suite, service, agents) = ready_environment();
        assert_eq!(
            derive_readiness(&suite, &service, &agents, &projects),
            Readiness::Degraded
        );
    }

    #[test]
    fn unsupported_platform_cannot_report_ready() {
        let (suite, service, agents) = ready_environment();
        let projects = vec![ProjectCandidate {
            path: "/tmp/project".into(),
            name: "project".into(),
            initialized: true,
            is_git: true,
            integration_configured: true,
            integration_needs_repair: false,
        }];
        let platform =
            moraine_platform::PlatformCapabilities::for_host(moraine_platform::HostPlatform::MacOs);

        assert_eq!(
            derive_platform_readiness(&platform, &suite, &service, &agents, &projects),
            Readiness::NotConfigured
        );
    }
}
