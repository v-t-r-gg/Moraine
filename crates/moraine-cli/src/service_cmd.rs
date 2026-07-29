//! `moraine service` lifecycle through the shared background-runtime backend.

use anyhow::{bail, Context, Result};
use moraine_provision::{
    default_background_runtime_manager, BackgroundRuntimeManager, RuntimeInstallSpec,
};
use serde_json::json;

use crate::suite::SuitePaths;

fn runtime() -> std::sync::Arc<dyn BackgroundRuntimeManager> {
    default_background_runtime_manager()
}

fn ensure_mutation_supported(operation: &'static str) -> Result<()> {
    moraine_provision::ensure_product_capture_supported(
        &moraine_platform::PlatformCapabilities::current(),
        operation,
    )
    .map_err(Into::into)
}

fn service_binary() -> Result<std::path::PathBuf> {
    let suite = SuitePaths::discover();
    if suite.service.is_file() {
        return Ok(suite.service);
    }
    let sibling = std::env::current_exe()
        .context("current_exe")?
        .parent()
        .map(|path| {
            path.join(moraine_platform::executable_name(
                moraine_platform::HostPlatform::current(),
                moraine_platform::SuiteComponent::Service,
            ))
        })
        .filter(|path| path.is_file());
    sibling.ok_or_else(|| {
        anyhow::anyhow!(
            "service binary not found at {} (install the release suite first)",
            suite.service.display()
        )
    })
}

pub fn service_install(json: bool) -> Result<()> {
    ensure_mutation_supported("background_runtime_install")?;
    let executable = service_binary()?;
    runtime().install_runtime(&RuntimeInstallSpec::discover(executable))?;
    print_state(json, "install")
}

pub fn service_start(json: bool) -> Result<()> {
    ensure_mutation_supported("background_runtime_start")?;
    runtime().start()?;
    print_state(json, "start")
}

pub fn service_stop(json: bool) -> Result<()> {
    ensure_mutation_supported("background_runtime_stop")?;
    runtime().stop()?;
    print_state(json, "stop")
}

pub fn service_restart(json: bool) -> Result<()> {
    ensure_mutation_supported("background_runtime_restart")?;
    runtime().restart()?;
    print_state(json, "restart")
}

pub fn service_status(json: bool) -> Result<()> {
    print_state(json, "status")
}

pub fn service_logs(json_output: bool) -> Result<()> {
    let logs = runtime().logs(80)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "action": "logs",
                "logs": logs,
            }))?
        );
    } else {
        for entry in logs {
            println!("{}", entry.line);
        }
    }
    Ok(())
}

pub fn service_uninstall(json: bool) -> Result<()> {
    ensure_mutation_supported("background_runtime_uninstall")?;
    runtime().uninstall()?;
    print_state(json, "uninstall")
}

fn print_state(json_output: bool, action: &str) -> Result<()> {
    let state = runtime().inspect()?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "action": action,
                "service": state,
            }))?
        );
    } else {
        println!("{}", state.status_message);
        if !state.supported {
            bail!("{}", state.status_message);
        }
    }
    Ok(())
}
