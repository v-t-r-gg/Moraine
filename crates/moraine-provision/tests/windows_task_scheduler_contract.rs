#![cfg(target_os = "windows")]

use std::fs;
use std::sync::Arc;
use std::thread;

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
    assert!(!installed.supported);
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
    assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
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
