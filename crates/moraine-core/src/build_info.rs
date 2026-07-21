//! Product version and schema compatibility constants for installed-suite diagnostics.
//!
//! One source of truth for CLI, service, desktop, and release manifests.

use serde::{Deserialize, Serialize};

/// Product marketing name.
pub const PRODUCT_NAME: &str = "Moraine";

/// Service diagnostics / query protocol major version (loopback HTTP surface).
pub const SERVICE_PROTOCOL_VERSION: u32 = 1;

/// MCP tool/schema implementation version for doctor compatibility checks.
pub const MCP_IMPLEMENTATION_VERSION: u32 = 1;

/// Minimum sidecar schema version this build will load (with migration).
pub const SCHEMA_MINIMUM_READABLE: u32 = 3;

/// Maximum sidecar schema version this build will load (rejects newer).
pub const SCHEMA_MAXIMUM_READABLE: u32 = crate::run_meta::SCHEMA_VERSION;

/// Schema version written on new/updated sidecars.
pub const SCHEMA_CURRENT_WRITABLE: u32 = crate::run_meta::SCHEMA_VERSION;

/// Package version from Cargo.
pub fn product_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Git commit embedded at compile time when available (`VERGEN`/`git` not required).
/// Falls back to `"unknown"` for plain `cargo build` without env injection.
pub fn git_commit() -> &'static str {
    option_env!("MORAINE_GIT_COMMIT").unwrap_or("unknown")
}

/// Target triple when set by release packaging.
pub fn build_target() -> &'static str {
    option_env!("MORAINE_TARGET_TRIPLE").unwrap_or(option_env!("TARGET").unwrap_or("unknown"))
}

/// Build profile name when set by packaging (`release` / `debug`).
pub fn build_profile() -> &'static str {
    option_env!("MORAINE_BUILD_PROFILE").unwrap_or(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCompat {
    pub minimum_readable: u32,
    pub maximum_readable: u32,
    pub current_writable: u32,
}

impl SchemaCompat {
    pub fn current() -> Self {
        Self {
            minimum_readable: SCHEMA_MINIMUM_READABLE,
            maximum_readable: SCHEMA_MAXIMUM_READABLE,
            current_writable: SCHEMA_CURRENT_WRITABLE,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BuildIdentity {
    pub product: String,
    pub version: String,
    pub git_commit: String,
    pub target: String,
    pub profile: String,
    pub schema: SchemaCompat,
    pub service_protocol_version: u32,
    pub mcp_implementation_version: u32,
}

impl BuildIdentity {
    pub fn current() -> Self {
        Self {
            product: PRODUCT_NAME.into(),
            version: product_version().into(),
            git_commit: git_commit().into(),
            target: build_target().into(),
            profile: build_profile().into(),
            schema: SchemaCompat::current(),
            service_protocol_version: SERVICE_PROTOCOL_VERSION,
            mcp_implementation_version: MCP_IMPLEMENTATION_VERSION,
        }
    }
}

/// Suite install manifest written by the installer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiteManifest {
    pub product: String,
    pub version: String,
    pub git_commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_timestamp: Option<String>,
    pub target: String,
    pub profile: String,
    pub schema: SchemaCompat,
    pub service_protocol_version: u32,
    pub mcp_implementation_version: u32,
    pub components: SuiteComponents,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiteComponents {
    pub cli: String,
    pub service: String,
    pub desktop: String,
}

impl SuiteManifest {
    pub fn from_build(prefix: Option<&str>) -> Self {
        let id = BuildIdentity::current();
        let v = id.version.clone();
        Self {
            product: id.product,
            version: v.clone(),
            git_commit: id.git_commit,
            build_timestamp: Some(chrono::Utc::now().to_rfc3339()),
            target: id.target,
            profile: id.profile,
            schema: id.schema,
            service_protocol_version: id.service_protocol_version,
            mcp_implementation_version: id.mcp_implementation_version,
            components: SuiteComponents {
                cli: v.clone(),
                service: v.clone(),
                desktop: v,
            },
            prefix: prefix.map(|s| s.to_string()),
        }
    }

    pub fn components_coherent(&self) -> bool {
        let cli_ok = self.components.cli == self.version;
        let svc_ok = self.components.service == self.version;
        // Desktop may be omitted from headless/CLI+service bundles.
        let desk_ok = self.components.desktop == self.version
            || self.components.desktop == "missing"
            || self.components.desktop.is_empty();
        cli_ok && svc_ok && desk_ok
    }
}
