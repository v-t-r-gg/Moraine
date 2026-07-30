//! Platform-neutral background runtime lifecycle.

pub mod linux_systemd;
mod memory;
mod unsupported;
#[cfg(target_os = "windows")]
pub mod windows_task_scheduler;

pub use linux_systemd::LinuxSystemdUserRuntime;
pub use memory::MemoryRuntimeManager;
pub use unsupported::UnsupportedRuntimeManager;
#[cfg(target_os = "windows")]
pub use windows_task_scheduler::WindowsTaskSchedulerRuntime;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::Result;
use crate::types::{BackgroundRuntimeState, RuntimeRegistrationSnapshot, ServiceLog};
use moraine_platform::{CaptureEndpoint, HostPlatform};

#[derive(Debug, Clone)]
pub struct RuntimeInstallSpec {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub capture_endpoint: CaptureEndpoint,
    pub diagnostics_endpoint: SocketAddr,
    pub spool_dir: PathBuf,
    pub log_dir: Option<PathBuf>,
}

impl RuntimeInstallSpec {
    pub fn try_discover(executable: impl Into<PathBuf>) -> Result<Self> {
        let layout = moraine_platform::RuntimeLayout::try_discover()
            .map_err(|error| crate::ProvisionError::Service(error.to_string()))?;
        let suite = crate::suite::SuitePaths::discover();
        Ok(Self {
            executable: executable.into(),
            working_directory: suite.prefix,
            capture_endpoint: layout.capture_endpoint,
            diagnostics_endpoint: layout.diagnostics_endpoint,
            spool_dir: layout.spool_dir,
            log_dir: (moraine_platform::HostPlatform::current()
                == moraine_platform::HostPlatform::Windows)
                .then_some(layout.log_dir),
        })
    }

    pub fn discover(executable: impl Into<PathBuf>) -> Self {
        let executable = executable.into();
        Self::try_discover(executable.clone()).unwrap_or_else(|_| {
            let layout = moraine_platform::RuntimeLayout::discover();
            let suite = crate::suite::SuitePaths::discover();
            Self {
                executable,
                working_directory: suite.prefix,
                capture_endpoint: layout.capture_endpoint,
                diagnostics_endpoint: layout.diagnostics_endpoint,
                spool_dir: layout.spool_dir,
                log_dir: None,
            }
        })
    }
}

/// Background capture lifecycle. Implementations hide OS terminology from the UI.
pub trait BackgroundRuntimeManager: Send + Sync {
    fn inspect(&self) -> Result<BackgroundRuntimeState>;
    fn capture_registration(&self) -> Result<RuntimeRegistrationSnapshot>;
    fn registration_fingerprint(&self) -> Result<Option<String>>;
    /// Compatibility entry point for existing injected managers.
    fn install(&self, executable: &Path) -> Result<()>;
    fn install_runtime(&self, spec: &RuntimeInstallSpec) -> Result<()> {
        self.install(&spec.executable)
    }
    fn restore_registration(&self, snapshot: &RuntimeRegistrationSnapshot) -> Result<()>;
    fn uninstall(&self) -> Result<()>;
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn restart(&self) -> Result<()>;
    fn enable_autostart(&self) -> Result<()>;
    fn disable_autostart(&self) -> Result<()>;
    fn logs(&self, limit: usize) -> Result<Vec<ServiceLog>>;
}

pub fn background_runtime_manager_for_host(
    host: HostPlatform,
) -> Arc<dyn BackgroundRuntimeManager> {
    match host {
        HostPlatform::Linux => Arc::new(LinuxSystemdUserRuntime::new()),
        #[cfg(target_os = "windows")]
        HostPlatform::Windows => WindowsTaskSchedulerRuntime::new()
            .map(|runtime| Arc::new(runtime) as Arc<dyn BackgroundRuntimeManager>)
            .unwrap_or_else(|_| Arc::new(UnsupportedRuntimeManager::new(host))),
        #[cfg(not(target_os = "windows"))]
        HostPlatform::Windows => Arc::new(UnsupportedRuntimeManager::new(host)),
        HostPlatform::MacOs | HostPlatform::Other => Arc::new(UnsupportedRuntimeManager::new(host)),
    }
}

pub fn default_background_runtime_manager() -> Arc<dyn BackgroundRuntimeManager> {
    background_runtime_manager_for_host(HostPlatform::current())
}

// Source-compatible names for callers of the former service abstraction. New code uses
// the platform-neutral runtime vocabulary above.
pub use default_background_runtime_manager as default_service_manager;
pub use BackgroundRuntimeManager as ServiceManager;
pub use LinuxSystemdUserRuntime as LinuxSystemdUserService;
pub use MemoryRuntimeManager as MemoryServiceManager;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BackgroundRuntimeBackend;

    #[test]
    fn production_factory_never_uses_memory_for_unsupported_hosts() {
        for host in [HostPlatform::MacOs, HostPlatform::Other] {
            let runtime = background_runtime_manager_for_host(host);
            let state = runtime.inspect().unwrap();
            assert_eq!(state.backend, BackgroundRuntimeBackend::Unsupported);
            assert!(!state.supported);
            assert!(matches!(
                runtime.start(),
                Err(crate::ProvisionError::UnsupportedPlatform { .. })
            ));
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn modeled_windows_host_remains_unsupported_off_windows() {
        let runtime = background_runtime_manager_for_host(HostPlatform::Windows);
        assert_eq!(
            runtime.inspect().unwrap().backend,
            BackgroundRuntimeBackend::Unsupported
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_host_selects_task_scheduler_without_promoting_capabilities() {
        let runtime = background_runtime_manager_for_host(HostPlatform::Windows);
        let state = runtime.inspect().unwrap();
        assert_eq!(
            state.backend,
            BackgroundRuntimeBackend::WindowsTaskScheduler
        );
        assert!(!state.supported);
        assert!(
            !moraine_platform::PlatformCapabilities::for_host(HostPlatform::Windows)
                .runtime_capture_supported()
        );
    }

    #[test]
    fn linux_host_selects_systemd_backend() {
        let runtime = background_runtime_manager_for_host(HostPlatform::Linux);
        assert_eq!(
            runtime.inspect().unwrap().backend,
            BackgroundRuntimeBackend::LinuxSystemdUser
        );
    }
}
