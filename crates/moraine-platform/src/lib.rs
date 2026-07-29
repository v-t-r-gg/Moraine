//! Host identity, capabilities, and filesystem layout for Moraine.
//!
//! This foundational crate owns descriptions only; it has no Moraine domain,
//! IPC implementation, runtime manager, provisioning or desktop dependency.
//! Concrete capture & background-runtime backends remain in product crates.

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod windows_identity;

#[cfg(target_os = "windows")]
pub use windows_identity::current_windows_user_identity;
pub use windows_identity::{named_pipe_name_from_scope, scope_id_from_sid, WindowsUserIdentity};

pub const DIAGNOSTICS_PORT: u16 = 33111;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    pub code: &'static str,
    pub message: String,
}

impl PlatformError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PlatformError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostPlatform {
    Linux,
    Windows,
    MacOs,
    Other,
}

impl HostPlatform {
    pub const fn current() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Supported,
    Unsupported,
    Unavailable,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub host: HostPlatform,
    pub user_paths: CapabilityStatus,
    pub suite_layout: CapabilityStatus,
    pub capture_transport: CapabilityStatus,
    pub background_runtime: CapabilityStatus,
    pub desktop_host: CapabilityStatus,
    pub user_installation: CapabilityStatus,
}

impl PlatformCapabilities {
    pub const fn for_host(host: HostPlatform) -> Self {
        match host {
            HostPlatform::Linux => Self {
                host,
                user_paths: CapabilityStatus::Supported,
                suite_layout: CapabilityStatus::Supported,
                capture_transport: CapabilityStatus::Supported,
                background_runtime: CapabilityStatus::Supported,
                desktop_host: CapabilityStatus::Supported,
                user_installation: CapabilityStatus::Supported,
            },
            HostPlatform::Windows => Self {
                host,
                user_paths: CapabilityStatus::Supported,
                suite_layout: CapabilityStatus::Supported,
                capture_transport: CapabilityStatus::Unsupported,
                background_runtime: CapabilityStatus::Unsupported,
                desktop_host: CapabilityStatus::Unsupported,
                user_installation: CapabilityStatus::Unsupported,
            },
            HostPlatform::MacOs | HostPlatform::Other => Self {
                host,
                user_paths: CapabilityStatus::Unsupported,
                suite_layout: CapabilityStatus::Unsupported,
                capture_transport: CapabilityStatus::Unsupported,
                background_runtime: CapabilityStatus::Unsupported,
                desktop_host: CapabilityStatus::Unsupported,
                user_installation: CapabilityStatus::Unsupported,
            },
        }
    }

    pub const fn current() -> Self {
        Self::for_host(HostPlatform::current())
    }

    /// Whether this host has product capture transport & runtime backends.
    ///
    /// Distribution is deliberately separate; W2 can validate a manually
    /// staged runtime without claiming the W3 installer exists.
    pub const fn runtime_capture_supported(&self) -> bool {
        matches!(self.capture_transport, CapabilityStatus::Supported)
            && matches!(self.background_runtime, CapabilityStatus::Supported)
    }

    pub const fn desktop_runtime_supported(&self) -> bool {
        self.runtime_capture_supported() && matches!(self.desktop_host, CapabilityStatus::Supported)
    }

    pub const fn distribution_supported(&self) -> bool {
        matches!(self.user_installation, CapabilityStatus::Supported)
    }

    /// Compatibility name for callers deciding whether ProductCapture can be
    /// ready. Installation provenance is not a runtime readiness condition.
    pub const fn product_ready_supported(&self) -> bool {
        self.runtime_capture_supported()
    }

    /// Compatibility name for native desktop runtime support.
    pub const fn desktop_product_supported(&self) -> bool {
        self.desktop_runtime_supported()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPaths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
}

impl UserPaths {
    pub fn discover() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            data_dir: dirs::data_dir().unwrap_or_else(|| home.join(".local/share")),
            config_dir: dirs::config_dir().unwrap_or_else(|| home.join(".config")),
            cache_dir: dirs::cache_dir().unwrap_or_else(|| home.join(".cache")),
            runtime_dir: env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(env::temp_dir),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CaptureEndpoint {
    UnixSocket(PathBuf),
    WindowsNamedPipe(String),
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteLayout {
    pub prefix: PathBuf,
    pub cli: PathBuf,
    pub service: PathBuf,
    pub desktop: PathBuf,
    pub share: PathBuf,
    pub manifest: PathBuf,
    pub service_registration: Option<PathBuf>,
    pub desktop_registration: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutOverrides {
    pub prefix: Option<PathBuf>,
    pub cli: Option<PathBuf>,
    pub service: Option<PathBuf>,
    pub capture_socket: Option<PathBuf>,
    pub spool_dir: Option<PathBuf>,
    pub project_registry: Option<PathBuf>,
}

impl LayoutOverrides {
    pub fn from_env() -> Self {
        Self {
            prefix: env::var_os("MORAINE_PREFIX").map(PathBuf::from),
            cli: env::var_os("MORAINE_CLI").map(PathBuf::from),
            service: env::var_os("MORAINE_SERVICE_BIN").map(PathBuf::from),
            capture_socket: env::var_os("MORAINE_SOCKET").map(PathBuf::from),
            spool_dir: env::var_os("MORAINE_SPOOL_DIR").map(PathBuf::from),
            project_registry: env::var_os("MORAINE_PROJECT_REGISTRY").map(PathBuf::from),
        }
    }
}

impl SuiteLayout {
    pub fn from_prefix(host: HostPlatform, prefix: impl AsRef<Path>, users: &UserPaths) -> Self {
        let prefix = prefix.as_ref().to_path_buf();
        let share = prefix.join("share/moraine");
        let (cli, service, desktop, service_registration, desktop_registration) = match host {
            HostPlatform::Windows => (
                prefix.join("moraine.exe"),
                prefix.join("moraine-service.exe"),
                prefix.join("moraine-app.exe"),
                None,
                None,
            ),
            HostPlatform::Linux => (
                prefix.join("bin/moraine"),
                prefix.join("libexec/moraine/moraine-service"),
                prefix.join("lib/moraine/moraine-app"),
                Some(
                    users
                        .config_dir
                        .join("systemd/user/moraine-service.service"),
                ),
                Some(prefix.join("share/applications/app.moraine.desktop")),
            ),
            HostPlatform::MacOs | HostPlatform::Other => (
                prefix.join("bin/moraine"),
                prefix.join("bin/moraine-service"),
                prefix.join("bin/moraine-app"),
                None,
                None,
            ),
        };
        Self {
            prefix,
            cli,
            service,
            desktop,
            manifest: share.join("manifest.json"),
            share,
            service_registration,
            desktop_registration,
        }
    }

    pub fn discover() -> Self {
        let host = HostPlatform::current();
        let users = UserPaths::discover();
        Self::with_overrides(host, &users, &LayoutOverrides::from_env())
    }

    pub fn with_overrides(
        host: HostPlatform,
        users: &UserPaths,
        overrides: &LayoutOverrides,
    ) -> Self {
        let prefix = overrides
            .prefix
            .clone()
            .unwrap_or_else(|| default_prefix(host));
        let mut layout = Self::from_prefix(host, prefix, users);
        if let Some(cli) = &overrides.cli {
            layout.cli = cli.clone();
        }
        if let Some(service) = &overrides.service {
            layout.service = service.clone();
        }
        layout
    }
}

pub fn default_prefix(host: HostPlatform) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    match host {
        HostPlatform::Windows => dirs::data_local_dir()
            .unwrap_or_else(|| home.join("AppData/Local"))
            .join("Moraine"),
        HostPlatform::Linux | HostPlatform::MacOs | HostPlatform::Other => home.join(".local"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLayout {
    pub spool_dir: PathBuf,
    pub project_registry: PathBuf,
    pub transaction_journals: PathBuf,
    pub diagnostics_endpoint: SocketAddr,
    pub capture_endpoint: CaptureEndpoint,
}

impl RuntimeLayout {
    pub fn for_host(host: HostPlatform, users: &UserPaths) -> Self {
        Self::for_host_with_scope(host, users, None)
    }

    pub fn for_host_with_scope(
        host: HostPlatform,
        users: &UserPaths,
        windows_scope_id: Option<&str>,
    ) -> Self {
        let capture_endpoint = match host {
            HostPlatform::Linux => {
                CaptureEndpoint::UnixSocket(users.runtime_dir.join("moraine-service.sock"))
            }
            HostPlatform::Windows => windows_scope_id
                .map(named_pipe_name_from_scope)
                .map(CaptureEndpoint::WindowsNamedPipe)
                .unwrap_or(CaptureEndpoint::Unsupported),
            HostPlatform::MacOs | HostPlatform::Other => CaptureEndpoint::Unsupported,
        };
        Self {
            spool_dir: users.cache_dir.join("moraine-service/spool"),
            project_registry: users.data_dir.join("moraine/projects.json"),
            transaction_journals: users.data_dir.join("moraine/setup-transactions"),
            diagnostics_endpoint: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                DIAGNOSTICS_PORT,
            ),
            capture_endpoint,
        }
    }

    pub fn try_discover() -> Result<Self, PlatformError> {
        let host = HostPlatform::current();
        let users = UserPaths::discover();
        let overrides = LayoutOverrides::from_env();
        #[cfg(target_os = "windows")]
        let windows_scope = Some(current_windows_user_identity()?.scope_id);
        #[cfg(not(target_os = "windows"))]
        let windows_scope: Option<String> = None;

        let mut layout = Self::for_host_with_scope(host, &users, windows_scope.as_deref());
        Self::apply_overrides(&mut layout, host, &overrides);
        Ok(layout)
    }

    pub fn discover() -> Self {
        Self::try_discover().unwrap_or_else(|_| {
            Self::with_overrides(
                HostPlatform::current(),
                &UserPaths::discover(),
                &LayoutOverrides::from_env(),
            )
        })
    }

    pub fn with_overrides(
        host: HostPlatform,
        users: &UserPaths,
        overrides: &LayoutOverrides,
    ) -> Self {
        let mut layout = Self::for_host(host, users);
        Self::apply_overrides(&mut layout, host, overrides);
        layout
    }

    fn apply_overrides(layout: &mut Self, host: HostPlatform, overrides: &LayoutOverrides) {
        if let Some(socket) = &overrides.capture_socket {
            layout.capture_endpoint = if host == HostPlatform::Linux {
                CaptureEndpoint::UnixSocket(socket.clone())
            } else {
                CaptureEndpoint::Unsupported
            };
        }
        if let Some(spool) = &overrides.spool_dir {
            layout.spool_dir = spool.clone();
        }
        if let Some(registry) = &overrides.project_registry {
            layout.project_registry = registry.clone();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuiteComponent {
    Cli,
    Service,
    Desktop,
}

pub const fn executable_name(host: HostPlatform, component: SuiteComponent) -> &'static str {
    match (host, component) {
        (HostPlatform::Windows, SuiteComponent::Cli) => "moraine.exe",
        (HostPlatform::Windows, SuiteComponent::Service) => "moraine-service.exe",
        (HostPlatform::Windows, SuiteComponent::Desktop) => "moraine-app.exe",
        (_, SuiteComponent::Cli) => "moraine",
        (_, SuiteComponent::Service) => "moraine-service",
        (_, SuiteComponent::Desktop) => "moraine-app",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PlatformContractFixture {
        capabilities: PlatformCapabilities,
        service_registration: Option<String>,
        desktop_registration: Option<String>,
        capture_endpoint: CaptureEndpoint,
    }

    fn users(root: &Path) -> UserPaths {
        UserPaths {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            runtime_dir: root.join("runtime"),
        }
    }

    #[test]
    fn linux_layout_preserves_c3_paths() {
        let root = tempfile::tempdir().unwrap();
        let prefix = root.path().join(".local");
        let users = users(root.path());
        let suite = SuiteLayout::from_prefix(HostPlatform::Linux, &prefix, &users);
        let runtime = RuntimeLayout::for_host(HostPlatform::Linux, &users);

        assert_eq!(suite.prefix, prefix);
        assert_eq!(suite.cli, prefix.join("bin/moraine"));
        assert_eq!(
            suite.service,
            prefix.join("libexec/moraine/moraine-service")
        );
        assert_eq!(suite.desktop, prefix.join("lib/moraine/moraine-app"));
        assert_eq!(suite.manifest, prefix.join("share/moraine/manifest.json"));
        assert_eq!(
            suite.service_registration,
            Some(
                users
                    .config_dir
                    .join("systemd/user/moraine-service.service")
            )
        );
        assert_eq!(
            suite.desktop_registration,
            Some(prefix.join("share/applications/app.moraine.desktop"))
        );
        assert_eq!(
            runtime.project_registry,
            users.data_dir.join("moraine/projects.json")
        );
        assert_eq!(
            runtime.transaction_journals,
            users.data_dir.join("moraine/setup-transactions")
        );
        assert_eq!(
            runtime.spool_dir,
            users.cache_dir.join("moraine-service/spool")
        );
        assert_eq!(
            runtime.capture_endpoint,
            CaptureEndpoint::UnixSocket(users.runtime_dir.join("moraine-service.sock"))
        );
        assert_eq!(
            runtime.diagnostics_endpoint,
            "127.0.0.1:33111".parse().unwrap()
        );
    }

    #[test]
    fn windows_layout_has_executables_without_linux_registrations() {
        let root = tempfile::tempdir().unwrap();
        let prefix = root.path().join("Moraine");
        let users = users(root.path());
        let suite = SuiteLayout::from_prefix(HostPlatform::Windows, &prefix, &users);
        let runtime = RuntimeLayout::for_host(HostPlatform::Windows, &users);

        assert_eq!(suite.cli, prefix.join("moraine.exe"));
        assert_eq!(suite.service, prefix.join("moraine-service.exe"));
        assert_eq!(suite.desktop, prefix.join("moraine-app.exe"));
        assert_eq!(suite.service_registration, None);
        assert_eq!(suite.desktop_registration, None);
        assert_eq!(runtime.capture_endpoint, CaptureEndpoint::Unsupported);
        for path in [&suite.cli, &suite.service, &suite.desktop] {
            let relative = path.strip_prefix(&prefix).unwrap().display().to_string();
            assert!(!relative.contains(".local"));
            assert!(!relative.contains("libexec"));
            assert!(!relative.contains("systemd"));
            assert!(!relative.contains(".desktop"));
            assert!(!relative.contains("moraine-service.sock"));
        }
        assert!(!PlatformCapabilities::for_host(HostPlatform::Windows).product_ready_supported());
    }

    #[test]
    fn windows_runtime_layout_requires_an_explicit_scope() {
        let root = tempfile::tempdir().unwrap();
        let users = users(root.path());
        assert_eq!(
            RuntimeLayout::for_host(HostPlatform::Windows, &users).capture_endpoint,
            CaptureEndpoint::Unsupported
        );
        assert_eq!(
            RuntimeLayout::for_host_with_scope(HostPlatform::Windows, &users, Some("0123456789ab"))
                .capture_endpoint,
            CaptureEndpoint::WindowsNamedPipe(r"\\.\pipe\moraine.capture.v1.0123456789ab".into())
        );
        assert_eq!(
            PlatformCapabilities::for_host(HostPlatform::Windows).capture_transport,
            CapabilityStatus::Unsupported
        );
    }

    #[test]
    fn unknown_hosts_fail_closed() {
        let capabilities = PlatformCapabilities::for_host(HostPlatform::Other);
        assert!(!capabilities.product_ready_supported());
        assert_eq!(
            capabilities.background_runtime,
            CapabilityStatus::Unsupported
        );
    }

    #[test]
    fn runtime_readiness_does_not_claim_distribution_support() {
        let capabilities = PlatformCapabilities {
            host: HostPlatform::Windows,
            user_paths: CapabilityStatus::Supported,
            suite_layout: CapabilityStatus::Supported,
            capture_transport: CapabilityStatus::Supported,
            background_runtime: CapabilityStatus::Supported,
            desktop_host: CapabilityStatus::Supported,
            user_installation: CapabilityStatus::Unsupported,
        };

        assert!(capabilities.runtime_capture_supported());
        assert!(capabilities.desktop_runtime_supported());
        assert!(capabilities.product_ready_supported());
        assert!(capabilities.desktop_product_supported());
        assert!(!capabilities.distribution_supported());
    }

    #[test]
    fn runtime_and_desktop_capabilities_fail_closed_independently() {
        let mut capabilities = PlatformCapabilities::for_host(HostPlatform::Linux);
        capabilities.capture_transport = CapabilityStatus::Degraded;
        assert!(!capabilities.runtime_capture_supported());
        assert!(!capabilities.desktop_runtime_supported());
        assert!(capabilities.distribution_supported());

        capabilities.capture_transport = CapabilityStatus::Supported;
        capabilities.desktop_host = CapabilityStatus::Unavailable;
        assert!(capabilities.runtime_capture_supported());
        assert!(!capabilities.desktop_runtime_supported());
    }

    #[test]
    fn shared_platform_contract_fixture_deserializes_with_stable_enums() {
        let raw = include_str!("../../../src/shared/api/platform.contract.fixture.json");
        let states: Vec<PlatformContractFixture> = serde_json::from_str(raw).unwrap();

        assert_eq!(states[0].capabilities.host, HostPlatform::Linux);
        assert!(states[0].service_registration.is_some());
        assert!(states[0].desktop_registration.is_some());
        assert!(matches!(
            states[0].capture_endpoint,
            CaptureEndpoint::UnixSocket(_)
        ));
        assert_eq!(states[1].capabilities.host, HostPlatform::Windows);
        assert_eq!(states[1].service_registration, None);
        assert_eq!(states[1].capture_endpoint, CaptureEndpoint::Unsupported);
        assert!(!states[1].capabilities.product_ready_supported());
        assert_eq!(states[2].capabilities.host, HostPlatform::Other);
    }

    #[test]
    fn linux_overrides_are_applied_without_process_environment_mutation() {
        let root = tempfile::tempdir().unwrap();
        let users = users(root.path());
        let overrides = LayoutOverrides {
            prefix: Some(root.path().join("prefix")),
            cli: Some(root.path().join("custom/moraine")),
            service: Some(root.path().join("custom/moraine-service")),
            capture_socket: Some(root.path().join("custom/moraine.sock")),
            spool_dir: Some(root.path().join("custom/spool")),
            project_registry: Some(root.path().join("custom/projects.json")),
        };
        let suite = SuiteLayout::with_overrides(HostPlatform::Linux, &users, &overrides);
        let runtime = RuntimeLayout::with_overrides(HostPlatform::Linux, &users, &overrides);

        assert_eq!(suite.prefix, root.path().join("prefix"));
        assert_eq!(suite.cli, root.path().join("custom/moraine"));
        assert_eq!(suite.service, root.path().join("custom/moraine-service"));
        assert_eq!(
            runtime.capture_endpoint,
            CaptureEndpoint::UnixSocket(root.path().join("custom/moraine.sock"))
        );
        assert_eq!(runtime.spool_dir, root.path().join("custom/spool"));
        assert_eq!(
            runtime.project_registry,
            root.path().join("custom/projects.json")
        );
    }
}
