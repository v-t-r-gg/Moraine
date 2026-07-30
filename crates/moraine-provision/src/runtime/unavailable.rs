use std::path::Path;

use moraine_platform::HostPlatform;

use crate::error::{ProvisionError, Result};
use crate::types::{
    BackgroundRuntimeBackend, BackgroundRuntimeState, RuntimeRegistrationSnapshot, ServiceLog,
};

/// Truthful production fallback when an implemented host backend cannot initialize.
pub struct UnavailableRuntimeManager {
    host: HostPlatform,
    backend: BackgroundRuntimeBackend,
    reason: String,
}

impl UnavailableRuntimeManager {
    pub fn new(
        host: HostPlatform,
        backend: BackgroundRuntimeBackend,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            host,
            backend,
            reason: reason.into(),
        }
    }

    fn unavailable(&self, operation: &'static str) -> ProvisionError {
        ProvisionError::RuntimeUnavailable {
            platform: self.host,
            operation,
            detail: self.reason.clone(),
        }
    }
}

impl super::BackgroundRuntimeManager for UnavailableRuntimeManager {
    fn inspect(&self) -> Result<BackgroundRuntimeState> {
        Ok(BackgroundRuntimeState {
            backend: self.backend,
            supported: false,
            installed: false,
            binary_present: false,
            registration: None,
            registration_present: false,
            registration_valid: false,
            running: false,
            autostart_enabled: false,
            endpoint_ready: false,
            diagnostics_ready: false,
            capture_ready: false,
            binary_path: None,
            unit_path: None,
            version: None,
            last_result: None,
            status_message: format!("Background capture runtime is unavailable: {}", self.reason),
            platform: format!("{:?}", self.host).to_lowercase(),
        })
    }

    fn capture_registration(&self) -> Result<RuntimeRegistrationSnapshot> {
        Err(self.unavailable("background_runtime_capture_registration"))
    }
    fn registration_fingerprint(&self) -> Result<Option<String>> {
        Err(self.unavailable("background_runtime_registration_fingerprint"))
    }
    fn install(&self, _executable: &Path) -> Result<()> {
        Err(self.unavailable("background_runtime_install"))
    }
    fn restore_registration(&self, _snapshot: &RuntimeRegistrationSnapshot) -> Result<()> {
        Err(self.unavailable("background_runtime_restore_registration"))
    }
    fn uninstall(&self) -> Result<()> {
        Err(self.unavailable("background_runtime_uninstall"))
    }
    fn start(&self) -> Result<()> {
        Err(self.unavailable("background_runtime_start"))
    }
    fn stop(&self) -> Result<()> {
        Err(self.unavailable("background_runtime_stop"))
    }
    fn restart(&self) -> Result<()> {
        Err(self.unavailable("background_runtime_restart"))
    }
    fn enable_autostart(&self) -> Result<()> {
        Err(self.unavailable("background_runtime_enable_autostart"))
    }
    fn disable_autostart(&self) -> Result<()> {
        Err(self.unavailable("background_runtime_disable_autostart"))
    }
    fn logs(&self, _limit: usize) -> Result<Vec<ServiceLog>> {
        Err(self.unavailable("background_runtime_logs"))
    }
}
