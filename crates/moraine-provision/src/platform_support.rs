//! Shared fail-closed guards for platform-dependent product operations.

use moraine_platform::PlatformCapabilities;

use crate::error::{ProvisionError, Result};

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
