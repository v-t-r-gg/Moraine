//! Installed suite path layout (shared by CLI and desktop).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use moraine_core::SuiteManifest;
use moraine_platform::{
    executable_name, CaptureEndpoint, HostPlatform, RuntimeLayout, SuiteComponent, SuiteLayout,
    UserPaths, DIAGNOSTICS_PORT,
};
use serde::{Deserialize, Serialize};

/// Default user-scoped install prefix (`~/.local`).
pub fn default_prefix() -> PathBuf {
    moraine_platform::default_prefix(HostPlatform::current())
}

#[derive(Debug, Clone)]
pub struct SuitePaths {
    pub prefix: PathBuf,
    pub cli: PathBuf,
    pub service: PathBuf,
    pub desktop: PathBuf,
    pub share: PathBuf,
    pub manifest: PathBuf,
    pub service_registration: Option<PathBuf>,
    pub desktop_registration: Option<PathBuf>,
}

impl SuitePaths {
    pub fn from_prefix(prefix: impl AsRef<Path>) -> Self {
        Self::for_host(
            HostPlatform::current(),
            prefix.as_ref(),
            &UserPaths::discover(),
        )
    }

    pub fn for_host(host: HostPlatform, prefix: &Path, users: &UserPaths) -> Self {
        let layout = SuiteLayout::from_prefix(host, prefix, users);
        Self {
            prefix: layout.prefix,
            cli: layout.cli,
            service: layout.service,
            desktop: layout.desktop,
            share: layout.share,
            manifest: layout.manifest,
            service_registration: layout.service_registration,
            desktop_registration: layout.desktop_registration,
        }
    }

    /// Resolve suite from env `MORAINE_PREFIX` or default XDG layout.
    pub fn discover() -> Self {
        if let Ok(p) = env::var("MORAINE_PREFIX") {
            return Self::from_prefix(p);
        }
        Self::default()
    }

    pub fn read_manifest(&self) -> Option<SuiteManifest> {
        let raw = fs::read_to_string(&self.manifest).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Absolute path to the suite-owned CLI binary named `moraine`.
    ///
    /// Never returns `moraine-app` / `moraine-service` (desktop/service hosts).
    /// Does not depend on shell PATH for product correctness.
    pub fn absolute_cli(&self) -> PathBuf {
        let host = HostPlatform::current();
        let cli_name = executable_name(host, SuiteComponent::Cli);
        if self.cli.is_file() {
            return fs::canonicalize(&self.cli).unwrap_or_else(|_| self.cli.clone());
        }
        // Explicit override for tests / advanced installs.
        if let Ok(over) = env::var("MORAINE_CLI") {
            let p = PathBuf::from(over);
            if p.is_file() {
                return fs::canonicalize(&p).unwrap_or(p);
            }
        }
        if let Ok(exe) = env::current_exe() {
            if let Some(parent) = exe.parent() {
                // Sibling `moraine` next to moraine-app / test binary / cargo target.
                let sibling = parent.join(cli_name);
                if sibling.is_file() {
                    return fs::canonicalize(&sibling).unwrap_or(sibling);
                }
                // Installed layout: …/lib/moraine/moraine-app → …/bin/moraine
                if host == HostPlatform::Linux {
                    if let Some(lib) = parent.parent() {
                        let bin = lib.join("bin").join(cli_name);
                        if bin.is_file() {
                            return fs::canonicalize(&bin).unwrap_or(bin);
                        }
                        // …/lib/moraine → prefix/bin/moraine
                        if let Some(prefix) = lib.parent() {
                            let bin = prefix.join("bin").join(cli_name);
                            if bin.is_file() {
                                return fs::canonicalize(&bin).unwrap_or(bin);
                            }
                        }
                    }
                }
            }
            // Only accept current_exe when it *is* the CLI.
            if exe.file_name().and_then(|n| n.to_str()) == Some(cli_name) && exe.is_file() {
                return fs::canonicalize(&exe).unwrap_or(exe);
            }
        }
        // Dev: cargo workspace target/{debug,release}/moraine from any crate manifest.
        if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
            let base = PathBuf::from(manifest);
            for ancestor in ["", "..", "../.."] {
                for profile in ["debug", "release"] {
                    let p = base
                        .join(ancestor)
                        .join("target")
                        .join(profile)
                        .join(cli_name);
                    if p.is_file() {
                        return fs::canonicalize(&p).unwrap_or(p);
                    }
                }
            }
        }
        PathBuf::from(cli_name)
    }

    /// Absolute path to the suite service binary when present.
    pub fn absolute_service(&self) -> Option<PathBuf> {
        let service_name = executable_name(HostPlatform::current(), SuiteComponent::Service);
        if self.service.is_file() {
            return Some(fs::canonicalize(&self.service).unwrap_or_else(|_| self.service.clone()));
        }
        if let Ok(over) = env::var("MORAINE_SERVICE_BIN") {
            let p = PathBuf::from(over);
            if p.is_file() {
                return Some(fs::canonicalize(&p).unwrap_or(p));
            }
        }
        if let Ok(exe) = env::current_exe() {
            if let Some(parent) = exe.parent() {
                let sibling = parent.join(service_name);
                if sibling.is_file() {
                    return Some(fs::canonicalize(&sibling).unwrap_or(sibling));
                }
            }
        }
        let cli = self.absolute_cli();
        if let Some(parent) = cli.parent() {
            let sibling = parent.join(service_name);
            if sibling.is_file() {
                return Some(fs::canonicalize(&sibling).unwrap_or(sibling));
            }
        }
        None
    }
}

impl Default for SuitePaths {
    fn default() -> Self {
        Self::from_prefix(default_prefix())
    }
}

/// Directory for setup transaction journals.
pub fn setup_transactions_dir() -> PathBuf {
    RuntimeLayout::discover().transaction_journals
}

pub fn capture_endpoint() -> CaptureEndpoint {
    RuntimeLayout::discover().capture_endpoint
}

pub fn unix_capture_socket() -> Option<PathBuf> {
    match capture_endpoint() {
        CaptureEndpoint::UnixSocket(path) => Some(path),
        CaptureEndpoint::WindowsNamedPipe(_) | CaptureEndpoint::Unsupported => None,
    }
}

pub fn default_http_addr() -> String {
    moraine_platform::RuntimeLayout::discover()
        .diagnostics_endpoint
        .to_string()
}

pub fn default_http_port() -> u16 {
    DIAGNOSTICS_PORT
}

/// Minimal loopback HTTP/1.1 GET without external curl.
pub fn http_get_loopback(port: u16, path: &str) -> std::result::Result<String, String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let mut stream =
        TcpStream::connect_timeout(&addr, Duration::from_millis(400)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&buf);
    if let Some(idx) = raw.find("\r\n\r\n") {
        Ok(raw[idx + 4..].to_string())
    } else {
        Err("invalid HTTP response".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteState {
    pub prefix: String,
    pub cli_path: String,
    pub cli_present: bool,
    pub service_path: String,
    pub service_present: bool,
    pub desktop_path: String,
    pub desktop_present: bool,
    pub manifest_path: String,
    pub manifest_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub components_coherent: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users(root: &Path) -> UserPaths {
        UserPaths {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            runtime_dir: root.join("runtime"),
        }
    }

    #[test]
    fn linux_wrapper_preserves_registration_paths() {
        let root = tempfile::tempdir().unwrap();
        let users = users(root.path());
        let prefix = root.path().join(".local");
        let paths = SuitePaths::for_host(HostPlatform::Linux, &prefix, &users);
        assert_eq!(
            paths.service_registration,
            Some(
                users
                    .config_dir
                    .join("systemd/user/moraine-service.service")
            )
        );
        assert_eq!(
            paths.desktop_registration,
            Some(prefix.join("share/applications/app.moraine.desktop"))
        );
        assert_eq!(paths.cli, prefix.join("bin/moraine"));
        assert_eq!(
            paths.service,
            prefix.join("libexec/moraine/moraine-service")
        );
    }

    #[test]
    fn windows_wrapper_has_executable_names_without_registration_sentinels() {
        let root = tempfile::tempdir().unwrap();
        let users = users(root.path());
        let paths = SuitePaths::for_host(HostPlatform::Windows, root.path(), &users);
        assert!(paths.service_registration.is_none());
        assert!(paths.desktop_registration.is_none());
        assert_eq!(
            paths.cli.file_name().unwrap(),
            executable_name(HostPlatform::Windows, SuiteComponent::Cli)
        );
        assert_eq!(
            paths.service.file_name().unwrap(),
            executable_name(HostPlatform::Windows, SuiteComponent::Service)
        );
        assert_eq!(
            paths.desktop.file_name().unwrap(),
            executable_name(HostPlatform::Windows, SuiteComponent::Desktop)
        );
    }
}
