#![cfg(target_os = "windows")]

use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use moraine_platform::{CaptureEndpoint, HostPlatform, RuntimeLayout, UserPaths};
use moraine_provision::{
    BackgroundRuntimeBackend, BackgroundRuntimeManager, RuntimeInstallSpec,
    RuntimeRegistrationSnapshot, SuitePaths, WindowsTaskIdentity, WindowsTaskSchedulerRuntime,
    WindowsTaskSnapshotState,
};
use uuid::Uuid;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

struct StaApartment;

impl StaApartment {
    fn initialize() -> windows::core::Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
        Ok(Self)
    }
}

impl Drop for StaApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct Cleanup(Arc<WindowsTaskSchedulerRuntime>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.0.uninstall();
    }
}

fn disposable_runtime() -> moraine_provision::Result<(
    tempfile::TempDir,
    Arc<WindowsTaskSchedulerRuntime>,
    RuntimeInstallSpec,
)> {
    let temp = tempfile::tempdir()?;
    let identity = moraine_platform::current_windows_user_identity()
        .map_err(|error| moraine_provision::ProvisionError::Service(error.to_string()))?;
    let scope = Uuid::new_v4().simple().to_string()[..12].to_owned();
    let task_identity = WindowsTaskIdentity::for_scope(identity.sid, scope.clone());
    let users = UserPaths {
        data_dir: temp.path().join("data"),
        config_dir: temp.path().join("config"),
        cache_dir: temp.path().join("cache"),
        runtime_dir: temp.path().join("runtime"),
    };
    let suite = SuitePaths::for_host(HostPlatform::Windows, temp.path(), &users);
    fs::write(&suite.service, b"disposable task action fixture")?;
    let runtime_layout =
        RuntimeLayout::for_host_with_scope(HostPlatform::Windows, &users, Some(&scope));
    let spec = RuntimeInstallSpec {
        executable: suite.service.clone(),
        working_directory: suite.prefix.clone(),
        capture_endpoint: runtime_layout.capture_endpoint.clone(),
        diagnostics_endpoint: runtime_layout.diagnostics_endpoint,
        spool_dir: runtime_layout.spool_dir.clone(),
        log_dir: Some(runtime_layout.log_dir.clone()),
    };
    let runtime = Arc::new(WindowsTaskSchedulerRuntime::with_layouts(
        suite,
        runtime_layout,
        task_identity,
    ));
    Ok((temp, runtime, spec))
}

#[test]
fn sta_caller_uses_production_mta_backend_for_exact_registration_restore(
) -> moraine_provision::Result<()> {
    let _caller = StaApartment::initialize()
        .map_err(|error| moraine_provision::ProvisionError::Service(error.to_string()))?;
    let (_temp, runtime, spec) = disposable_runtime()?;
    let _cleanup = Cleanup(runtime.clone());

    let absent = runtime.capture_registration()?;
    assert!(matches!(
        absent,
        RuntimeRegistrationSnapshot::WindowsTask(moraine_provision::WindowsTaskSnapshot {
            state: WindowsTaskSnapshotState::Absent,
            ..
        })
    ));
    runtime.install_runtime(&spec)?;
    let installed = runtime.inspect()?;
    assert_eq!(
        installed.backend,
        BackgroundRuntimeBackend::WindowsTaskScheduler
    );
    assert!(installed.supported);
    assert!(installed.registration_present);
    assert!(installed.registration_valid);
    assert!(!installed.autostart_enabled);
    assert!(installed.unit_path.is_none());
    assert!(matches!(
        installed.registration.as_ref().map(|value| value.kind),
        Some(moraine_provision::RuntimeRegistrationKind::WindowsTaskSchedulerTask)
    ));

    let original = runtime.capture_registration()?;
    let original_fingerprint = runtime.registration_fingerprint()?.unwrap();
    let RuntimeRegistrationSnapshot::WindowsTask(snapshot) = &original else {
        panic!("production Windows backend returned a file snapshot");
    };
    assert_eq!(snapshot.task_path, runtime.identity().task_path);
    let WindowsTaskSnapshotState::Existing {
        xml,
        security_descriptor,
        fingerprint,
    } = &snapshot.state
    else {
        panic!("installed task snapshot reported absence");
    };
    assert_eq!(fingerprint, &original_fingerprint);
    assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
    assert!(!xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
    assert!(security_descriptor.contains("SY"));
    for broad in ["WD", "AN", "BU", "AU"] {
        assert!(!security_descriptor.contains(&format!(";;;{broad})")));
    }

    runtime.enable_autostart()?;
    assert!(runtime.inspect()?.autostart_enabled);
    runtime.disable_autostart()?;
    assert!(!runtime.inspect()?.autostart_enabled);
    runtime.restore_registration(&original)?;
    assert_eq!(
        runtime.registration_fingerprint()?.as_deref(),
        Some(original_fingerprint.as_str())
    );

    runtime.restore_registration(&absent)?;
    assert!(!runtime.inspect()?.registration_present);
    runtime.restore_registration(&original)?;
    assert_eq!(
        runtime.registration_fingerprint()?.as_deref(),
        Some(original_fingerprint.as_str())
    );
    runtime.uninstall()?;
    runtime.uninstall()?;
    Ok(())
}

#[test]
fn foreign_and_corrupt_snapshots_are_rejected() -> moraine_provision::Result<()> {
    let (_temp, runtime, spec) = disposable_runtime()?;
    let _cleanup = Cleanup(runtime.clone());
    runtime.install_runtime(&spec)?;
    let RuntimeRegistrationSnapshot::WindowsTask(mut snapshot) = runtime.capture_registration()?
    else {
        panic!("expected Windows task snapshot");
    };

    snapshot.task_path.push_str("-foreign");
    assert!(runtime
        .restore_registration(&RuntimeRegistrationSnapshot::WindowsTask(snapshot.clone()))
        .is_err());

    snapshot.task_path = runtime.identity().task_path.clone();
    if let WindowsTaskSnapshotState::Existing { fingerprint, .. } = &mut snapshot.state {
        *fingerprint = "corrupt".into();
    }
    assert!(runtime
        .restore_registration(&RuntimeRegistrationSnapshot::WindowsTask(snapshot))
        .is_err());
    Ok(())
}

#[test]
fn operation_lock_serializes_production_inspection() -> moraine_provision::Result<()> {
    let (_temp, runtime, _spec) = disposable_runtime()?;
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let runtime = runtime.clone();
            thread::spawn(move || runtime.inspect())
        })
        .collect();
    for worker in threads {
        assert_eq!(
            worker.join().expect("inspection worker panicked")?.backend,
            BackgroundRuntimeBackend::WindowsTaskScheduler
        );
    }
    Ok(())
}

#[test]
fn scheduled_real_service_accepts_a_real_hook_and_writes_logs() -> moraine_provision::Result<()> {
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let target = repo.join("target/debug");
    let service_binary = target.join("moraine-service.exe");
    let cli_binary = target.join("moraine.exe");
    assert!(
        service_binary.is_file() && cli_binary.is_file(),
        "Windows CI must build the real CLI & service binaries before this smoke"
    );

    let temp = tempfile::tempdir()?;
    let identity = moraine_platform::current_windows_user_identity()
        .map_err(|error| moraine_provision::ProvisionError::Service(error.to_string()))?;
    let scope = Uuid::new_v4().simple().to_string()[..12].to_owned();
    let task_identity = WindowsTaskIdentity::for_scope(identity.sid, scope.clone());
    let users = UserPaths {
        data_dir: temp.path().join("data"),
        config_dir: temp.path().join("config"),
        cache_dir: temp.path().join("cache"),
        runtime_dir: temp.path().join("runtime"),
    };
    let mut runtime_layout =
        RuntimeLayout::for_host_with_scope(HostPlatform::Windows, &users, Some(&scope));
    let port_probe = TcpListener::bind("127.0.0.1:0")?;
    runtime_layout.diagnostics_endpoint = port_probe.local_addr()?;
    drop(port_probe);
    let suite = SuitePaths {
        prefix: repo.clone(),
        cli: cli_binary.clone(),
        service: service_binary.clone(),
        desktop: target.join("moraine-app.exe"),
        share: repo.join("share/moraine"),
        manifest: repo.join("share/moraine/manifest.json"),
        service_registration: None,
        desktop_registration: None,
    };
    let spec = RuntimeInstallSpec {
        executable: service_binary,
        working_directory: repo.clone(),
        capture_endpoint: runtime_layout.capture_endpoint.clone(),
        diagnostics_endpoint: runtime_layout.diagnostics_endpoint,
        spool_dir: runtime_layout.spool_dir.clone(),
        log_dir: Some(runtime_layout.log_dir.clone()),
    };
    let runtime = Arc::new(WindowsTaskSchedulerRuntime::with_layouts(
        suite,
        runtime_layout.clone(),
        task_identity,
    ));
    let _cleanup = Cleanup(runtime.clone());
    runtime.install_runtime(&spec)?;
    runtime.start()?;
    wait_for_status(runtime_layout.diagnostics_endpoint.port(), true)?;

    let session = format!("windows-task-smoke-{}", Uuid::new_v4());
    let payload = serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": session,
        "cwd": temp.path(),
        "model": "w2-task-smoke"
    });
    let pipe = match &runtime_layout.capture_endpoint {
        CaptureEndpoint::WindowsNamedPipe(pipe) => pipe,
        _ => panic!("Windows smoke runtime has no named pipe"),
    };
    let mut hook = Command::new(cli_binary)
        .args([
            "hook-codex",
            "--named-pipe",
            pipe,
            "--spool-dir",
            &runtime_layout.spool_dir.display().to_string(),
        ])
        .stdin(Stdio::piped())
        .spawn()?;
    hook.stdin
        .take()
        .expect("hook stdin")
        .write_all(payload.to_string().as_bytes())?;
    assert!(hook.wait()?.success());

    let mut captured = false;
    for _ in 0..100 {
        captured = walk_contains(&runtime_layout.spool_dir, session.as_bytes());
        if captured {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        captured,
        "scheduled runtime did not spool the real hook event"
    );
    let log = runtime_layout.log_dir.join("moraine-service.log");
    for _ in 0..100 {
        if fs::read_to_string(&log).is_ok_and(|text| text.contains("starting moraine-service")) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(fs::read_to_string(&log)
        .unwrap_or_default()
        .contains("starting moraine-service"));

    let running_snapshot = runtime.capture_registration()?;
    let running_fingerprint = runtime.registration_fingerprint()?.unwrap();
    runtime.restore_registration(&running_snapshot)?;
    wait_for_status(runtime_layout.diagnostics_endpoint.port(), false)?;
    assert_eq!(
        runtime.registration_fingerprint()?.as_deref(),
        Some(running_fingerprint.as_str())
    );

    runtime.start()?;
    wait_for_status(runtime_layout.diagnostics_endpoint.port(), true)?;
    runtime.stop()?;
    wait_for_status(runtime_layout.diagnostics_endpoint.port(), false)?;
    runtime.enable_autostart()?;
    assert!(runtime.inspect()?.autostart_enabled);
    runtime.disable_autostart()?;
    assert!(!runtime.inspect()?.autostart_enabled);
    runtime.start()?;
    wait_for_status(runtime_layout.diagnostics_endpoint.port(), true)?;
    runtime.uninstall()?;
    wait_for_status(runtime_layout.diagnostics_endpoint.port(), false)?;
    assert!(!runtime.inspect()?.registration_present);
    Ok(())
}

fn wait_for_status(port: u16, expected_online: bool) -> moraine_provision::Result<()> {
    for _ in 0..100 {
        let online = moraine_provision::suite::http_get_loopback(port, "/status")
            .map(|body| moraine_provision::diagnostics::parse_status(&body).capture_ready)
            .unwrap_or(false);
        if online == expected_online {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(moraine_provision::ProvisionError::Service(format!(
        "scheduled runtime did not reach expected online state {expected_online}"
    )))
}

fn walk_contains(root: &std::path::Path, needle: &[u8]) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && walk_contains(&path, needle) {
            return true;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("json")
            && fs::read(&path)
                .is_ok_and(|bytes| bytes.windows(needle.len()).any(|part| part == needle))
        {
            return true;
        }
    }
    false
}
