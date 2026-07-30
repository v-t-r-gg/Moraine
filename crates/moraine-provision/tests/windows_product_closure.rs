#![cfg(target_os = "windows")]

use std::fs;
use std::sync::Arc;

use moraine_platform::{HostPlatform, RuntimeLayout, UserPaths};
use moraine_provision::{
    enable_project, AgentKind, ApplyOutcome, BackgroundRuntimeManager, Readiness, SetupIntent,
    SuitePaths, WindowsTaskIdentity, WindowsTaskSchedulerRuntime,
};
use uuid::Uuid;

struct Cleanup(Arc<WindowsTaskSchedulerRuntime>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.0.uninstall();
    }
}

#[test]
fn manually_staged_windows_suite_reaches_product_ready() -> moraine_provision::Result<()> {
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let built = repo.join("target/debug");
    let built_cli = built.join("moraine.exe");
    let built_service = built.join("moraine-service.exe");
    assert!(built_cli.is_file() && built_service.is_file());

    let temp = tempfile::tempdir()?;
    let prefix = temp.path().join("Moraine Suite");
    fs::create_dir_all(&prefix)?;
    let users = UserPaths {
        data_dir: temp.path().join("data"),
        config_dir: temp.path().join("config"),
        cache_dir: temp.path().join("cache"),
        runtime_dir: temp.path().join("runtime"),
    };
    let suite = SuitePaths::for_host(HostPlatform::Windows, &prefix, &users);
    fs::copy(&built_cli, &suite.cli)?;
    fs::copy(&built_service, &suite.service)?;

    let user = moraine_platform::current_windows_user_identity()
        .map_err(|error| moraine_provision::ProvisionError::Service(error.to_string()))?;
    let task_scope = Uuid::new_v4().simple().to_string()[..12].to_owned();
    let task_identity = WindowsTaskIdentity::for_scope(user.sid, task_scope);
    let runtime_layout = RuntimeLayout::try_discover()
        .map_err(|error| moraine_provision::ProvisionError::Service(error.to_string()))?;
    let runtime = Arc::new(WindowsTaskSchedulerRuntime::with_layouts(
        suite.clone(),
        runtime_layout,
        task_identity,
    ));
    let _cleanup = Cleanup(runtime.clone());

    let project = temp.path().join("Project With Spaces");
    fs::create_dir_all(&project)?;
    // Detection only requires an executable named codex; the product capture
    // verification itself always invokes the real staged Moraine CLI.
    let codex_fixture = prefix.join("codex.exe");
    fs::copy(&built_cli, &codex_fixture)?;

    let prior_path = std::env::var_os("PATH");
    let prior_prefix = std::env::var_os("MORAINE_PREFIX");
    let prior_cli = std::env::var_os("MORAINE_CLI");
    unsafe {
        std::env::set_var(
            "PATH",
            format!(
                "{};{}",
                prefix.display(),
                prior_path.as_deref().unwrap_or_default().to_string_lossy()
            ),
        );
        std::env::set_var("MORAINE_PREFIX", &prefix);
        std::env::set_var("MORAINE_CLI", &suite.cli);
    }

    let outcome = enable_project(
        SetupIntent {
            project: project.clone(),
            agent: AgentKind::Codex,
            enable_autostart: true,
            skip_service: false,
        },
        runtime.as_ref(),
    );

    unsafe {
        match prior_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
        match prior_prefix {
            Some(value) => std::env::set_var("MORAINE_PREFIX", value),
            None => std::env::remove_var("MORAINE_PREFIX"),
        }
        match prior_cli {
            Some(value) => std::env::set_var("MORAINE_CLI", value),
            None => std::env::remove_var("MORAINE_CLI"),
        }
    }

    let outcome = outcome?;
    assert!(matches!(outcome, ApplyOutcome::Ready { .. }));
    assert_eq!(outcome.receipt().readiness, Readiness::Ready);
    assert!(outcome.receipt().service_prestate.is_some());
    assert!(project.join(".moraine").is_dir());
    let state = runtime.inspect()?;
    assert!(state.supported);
    assert!(state.registration_valid);
    assert!(state.autostart_enabled);
    assert!(state.diagnostics_ready);
    assert!(state.capture_ready);
    Ok(())
}
