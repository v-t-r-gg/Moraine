//! Installed suite path layout (shared by CLI and desktop).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use moraine_core::SuiteManifest;
use serde::{Deserialize, Serialize};

/// Default user-scoped install prefix (`~/.local`).
pub fn default_prefix() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
}

#[derive(Debug, Clone)]
pub struct SuitePaths {
    pub prefix: PathBuf,
    pub cli: PathBuf,
    pub service: PathBuf,
    pub desktop: PathBuf,
    pub share: PathBuf,
    pub manifest: PathBuf,
    pub unit: PathBuf,
    pub desktop_entry: PathBuf,
}

impl SuitePaths {
    pub fn from_prefix(prefix: impl AsRef<Path>) -> Self {
        let prefix = prefix.as_ref().to_path_buf();
        let share = prefix.join("share/moraine");
        Self {
            cli: prefix.join("bin/moraine"),
            service: prefix.join("libexec/moraine/moraine-service"),
            desktop: prefix.join("lib/moraine/moraine-app"),
            manifest: share.join("manifest.json"),
            share,
            unit: dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("~/.config"))
                .join("systemd/user/moraine-service.service"),
            desktop_entry: prefix.join("share/applications/app.moraine.desktop"),
            prefix,
        }
    }

    pub fn default() -> Self {
        Self::from_prefix(default_prefix())
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
                let sibling = parent.join("moraine");
                if sibling.is_file() {
                    return fs::canonicalize(&sibling).unwrap_or(sibling);
                }
                // Installed layout: …/lib/moraine/moraine-app → …/bin/moraine
                if let Some(lib) = parent.parent() {
                    let bin = lib.join("bin/moraine");
                    if bin.is_file() {
                        return fs::canonicalize(&bin).unwrap_or(bin);
                    }
                    // …/lib/moraine → prefix/bin/moraine
                    if let Some(prefix) = lib.parent() {
                        let bin = prefix.join("bin/moraine");
                        if bin.is_file() {
                            return fs::canonicalize(&bin).unwrap_or(bin);
                        }
                    }
                }
            }
            // Only accept current_exe when it *is* the CLI.
            if exe.file_name().and_then(|n| n.to_str()) == Some("moraine") && exe.is_file() {
                return fs::canonicalize(&exe).unwrap_or(exe);
            }
        }
        // Dev: cargo workspace target/{debug,release}/moraine from any crate manifest.
        if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
            let base = PathBuf::from(manifest);
            for rel in [
                "target/debug/moraine",
                "target/release/moraine",
                "../target/debug/moraine",
                "../target/release/moraine",
                "../../target/debug/moraine",
                "../../target/release/moraine",
            ] {
                let p = base.join(rel);
                if p.is_file() {
                    return fs::canonicalize(&p).unwrap_or(p);
                }
            }
        }
        PathBuf::from("moraine")
    }

    /// Absolute path to the suite service binary when present.
    pub fn absolute_service(&self) -> Option<PathBuf> {
        if self.service.is_file() {
            return Some(
                fs::canonicalize(&self.service).unwrap_or_else(|_| self.service.clone()),
            );
        }
        if let Ok(exe) = env::current_exe() {
            if let Some(parent) = exe.parent() {
                let sibling = parent.join("moraine-service");
                if sibling.is_file() {
                    return Some(fs::canonicalize(&sibling).unwrap_or(sibling));
                }
            }
        }
        None
    }
}

/// Directory for setup transaction journals.
pub fn setup_transactions_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/share")
        })
        .join("moraine/setup-transactions")
}

pub fn default_socket_path() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("moraine-service.sock")
}

pub fn default_http_addr() -> &'static str {
    "127.0.0.1:33111"
}

pub fn default_http_port() -> u16 {
    33111
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

/// Render systemd user unit with absolute ExecStart.
pub fn render_systemd_unit(service_bin: &Path, http: &str, socket: &str) -> String {
    format!(
        r#"[Unit]
Description=Moraine local integration runtime (per-user)
After=network.target

[Service]
Type=simple
ExecStart={exec} --http {http} --unix-socket {socket}
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
"#,
        exec = shell_escape_path(service_bin),
        http = http,
        socket = socket,
    )
}

fn shell_escape_path(p: &Path) -> String {
    let s = p.display().to_string();
    if s.contains(' ') || s.contains('\\') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s
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
