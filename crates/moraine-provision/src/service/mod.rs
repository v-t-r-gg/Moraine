//! Platform-abstracted background-service management.

mod linux_systemd;
mod memory;

pub use linux_systemd::LinuxSystemdUserService;
pub use memory::MemoryServiceManager;

use std::path::Path;
use std::sync::Arc;

use crate::error::Result;
use crate::types::{ServiceLog, ServiceState};

/// Background capture lifecycle. Implementations hide OS terminology from the UI.
pub trait ServiceManager: Send + Sync {
    fn inspect(&self) -> Result<ServiceState>;
    fn install(&self, executable: &Path) -> Result<()>;
    fn uninstall(&self) -> Result<()>;
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn restart(&self) -> Result<()>;
    fn enable_autostart(&self) -> Result<()>;
    fn disable_autostart(&self) -> Result<()>;
    fn logs(&self, limit: usize) -> Result<Vec<ServiceLog>>;
}

/// Default platform service manager for the host OS.
pub fn default_service_manager() -> Arc<dyn ServiceManager> {
    #[cfg(target_os = "linux")]
    {
        Arc::new(LinuxSystemdUserService::new())
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Stub until Windows/macOS implementations land.
        Arc::new(MemoryServiceManager::new())
    }
}
