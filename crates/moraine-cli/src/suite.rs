//! Installed suite paths, manifest, and PATH drift helpers (C2).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use moraine_core::{BuildIdentity, SuiteManifest};
use serde::Serialize;

/// Default user-scoped install prefix.
pub fn default_prefix() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // layout fields reserved for install/uninstall tooling
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
}

/// All `moraine` executables found by scanning PATH entries.
pub fn enumerate_moraine_on_path() -> Vec<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let mut out = Vec::new();
    for dir in env::split_paths(&path) {
        let cand = dir.join("moraine");
        if cand.is_file() {
            if let Ok(canon) = fs::canonicalize(&cand) {
                if !out.iter().any(|p| p == &canon) {
                    out.push(canon);
                }
            } else if !out.contains(&cand) {
                out.push(cand);
            }
        }
    }
    out
}

pub fn current_exe_path() -> PathBuf {
    env::current_exe().unwrap_or_else(|_| PathBuf::from("moraine"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionReport {
    pub ok: bool,
    pub cli: ComponentVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<SuiteInfo>,
    pub service: ServiceVersionInfo,
    pub desktop: DesktopVersionInfo,
    pub path_executables: Vec<String>,
    pub build: BuildIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentVersion {
    pub path: String,
    pub version: String,
    pub git_commit: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteInfo {
    pub manifest_path: String,
    pub version: String,
    pub components_coherent: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceVersionInfo {
    pub installed: bool,
    pub online: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopVersionInfo {
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub compatible: bool,
}

pub fn collect_version_report() -> VersionReport {
    let build = BuildIdentity::current();
    let suite_paths = SuitePaths::discover();
    let mut warnings = Vec::new();
    let path_exes = enumerate_moraine_on_path();
    if path_exes.len() > 1 {
        warnings.push(format!(
            "multiple moraine executables on PATH ({}); prefer the installed suite CLI",
            path_exes.len()
        ));
    }
    for p in &path_exes {
        let s = p.to_string_lossy();
        if s.contains(".cargo/bin") {
            warnings.push(
                "PATH includes ~/.cargo/bin/moraine which may shadow the installed suite".into(),
            );
            break;
        }
    }

    let suite = suite_paths.read_manifest().map(|m| SuiteInfo {
        manifest_path: suite_paths.manifest.display().to_string(),
        version: m.version.clone(),
        components_coherent: m.components_coherent(),
    });

    let service_installed = suite_paths.service.is_file();
    let (online, service_version, svc_msg) = probe_service_status();
    let service_compatible = service_version
        .as_ref()
        .map(|v| v == &build.version)
        .unwrap_or(!online);

    let desktop_installed = suite_paths.desktop.is_file();
    let desktop_version = suite
        .as_ref()
        .map(|s| s.version.clone())
        .or_else(|| desktop_installed.then(|| build.version.clone()));

    VersionReport {
        // ok reflects hard suite coherence only; PATH drift is advisory (see warnings).
        ok: suite
            .as_ref()
            .map(|s| s.components_coherent)
            .unwrap_or(true),
        cli: ComponentVersion {
            path: current_exe_path().display().to_string(),
            version: build.version.clone(),
            git_commit: build.git_commit.clone(),
        },
        suite,
        service: ServiceVersionInfo {
            installed: service_installed,
            online,
            path: service_installed.then(|| suite_paths.service.display().to_string()),
            version: service_version,
            compatible: service_compatible,
            message: svc_msg,
        },
        desktop: DesktopVersionInfo {
            installed: desktop_installed,
            path: desktop_installed.then(|| suite_paths.desktop.display().to_string()),
            version: desktop_version,
            compatible: true,
        },
        path_executables: path_exes.iter().map(|p| p.display().to_string()).collect(),
        build,
        warnings: (!warnings.is_empty()).then_some(warnings),
    }
}

fn probe_service_status() -> (bool, Option<String>, Option<String>) {
    // Loopback diagnostics only; bounded timeout via curl-free native TCP+HTTP/1.0.
    match http_get_loopback(33111, "/status") {
        Ok(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let ver = v
                .get("version")
                .or_else(|| v.get("productVersion"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let online = v
                .get("online")
                .and_then(|x| x.as_bool())
                .or_else(|| v.get("status").and_then(|s| s.as_str()).map(|s| s == "ok"))
                .unwrap_or(true);
            (online, ver, None)
        }
        Err(e) => (false, None, Some(e)),
    }
}

/// Minimal loopback HTTP/1.1 GET without external curl (shared with doctor).
pub fn http_get_loopback(port: u16, path: &str) -> Result<String, String> {
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

pub fn systemctl_user(args: &[&str]) -> Result<std::process::ExitStatus, String> {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .map_err(|e| e.to_string())
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
    // systemd ExecStart: quote if spaces
    let s = p.display().to_string();
    if s.contains(' ') || s.contains('\\') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s
    }
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
