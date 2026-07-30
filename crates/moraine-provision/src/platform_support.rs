//! Shared fail-closed guards for platform-dependent product operations.

use moraine_platform::PlatformCapabilities;

use crate::error::{ProvisionError, Result};
use crate::types::BackgroundRuntimeState;

pub fn ensure_product_capture_supported(
    capabilities: &PlatformCapabilities,
    operation: &'static str,
) -> Result<()> {
    if capabilities.product_ready_supported() {
        return Ok(());
    }
    Err(ProvisionError::UnsupportedPlatform {
        platform: capabilities.host,
        operation,
    })
}

pub fn ensure_background_runtime_available(
    state: &BackgroundRuntimeState,
    platform: moraine_platform::HostPlatform,
    operation: &'static str,
) -> Result<()> {
    if state.supported {
        return Ok(());
    }
    Err(ProvisionError::RuntimeUnavailable {
        platform,
        operation,
        detail: state.status_message.clone(),
    })
}
