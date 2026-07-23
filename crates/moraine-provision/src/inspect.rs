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
    let readiness = derive_readiness(&suite, &service_state, &agents);
    Ok(SystemState {
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
    inspect(svc.as_ref(), &[])
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
    });
}

fn derive_readiness(
    suite: &SuiteState,
    service: &ServiceState,
    agents: &[DetectedAgent],
) -> Readiness {
    if !suite.cli_present && !suite.manifest_present {
        return Readiness::NotConfigured;
    }
    if service.running && agents.iter().any(|a| a.detected) {
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
