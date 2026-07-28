//! Platform-neutral background runtime lifecycle.

pub mod linux_systemd;
mod memory;
mod unsupported;

pub use linux_systemd::LinuxSystemdUserRuntime;
pub use memory::MemoryRuntimeManager;
pub use unsupported::UnsupportedRuntimeManager;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::Result;
use crate::types::{BackgroundRuntimeState, RuntimeRegistrationSnapshot, ServiceLog};
use moraine_platform::{CaptureEndpoint, HostPlatform};

#[derive(Debug, Clone)]
pub struct RuntimeInstallSpec {
    pub executable: PathBuf,
    pub capture_endpoint: CaptureEndpoint,
    pub diagnostics_endpoint: SocketAddr,
    pub spool_dir: PathBuf,
}

impl RuntimeInstallSpec {
    pub fn discover(executable: impl Into<PathBuf>) -> Self {
        let layout = moraine_platform::RuntimeLayout::discover();
        Self {
            executable: executable.into(),
            capture_endpoint: layout.capture_endpoint,
            diagnostics_endpoint: layout.diagnostics_endpoint,
            spool_dir: layout.spool_dir,
        }
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
        HostPlatform::Windows | HostPlatform::MacOs | HostPlatform::Other => {
            Arc::new(UnsupportedRuntimeManager::new(host))
        }
    }
}

pub fn default_background_runtime_manager() -> Arc<dyn BackgroundRuntimeManager> {
    background_runtime_manager_for_host(HostPlatform::current())
}

// Source-compatible names for persisted C3 callers. New implementation code uses
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
        for host in [
            HostPlatform::Windows,
            HostPlatform::MacOs,
            HostPlatform::Other,
        ] {
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

    #[test]
    fn linux_host_selects_systemd_backend() {
        let runtime = background_runtime_manager_for_host(HostPlatform::Linux);
        assert_eq!(
            runtime.inspect().unwrap().backend,
            BackgroundRuntimeBackend::LinuxSystemdUser
        );
    }
}
