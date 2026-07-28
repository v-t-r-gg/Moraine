//! Linux systemd --user background runtime backend.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::{ProvisionError, Result};
use crate::runtime::RuntimeInstallSpec;
use crate::suite::{http_get_loopback, SuitePaths};
use crate::types::{
    BackgroundRuntimeBackend, BackgroundRuntimeState, RuntimeRegistrationKind,
    RuntimeRegistrationState, ServiceLog,
};

pub struct LinuxSystemdUserRuntime {
    suite: SuitePaths,
}

pub fn render_systemd_unit(spec: &RuntimeInstallSpec) -> Result<String> {
    let socket = match &spec.capture_endpoint {
        moraine_platform::CaptureEndpoint::UnixSocket(path) => path,
        endpoint => {
            return Err(ProvisionError::Service(format!(
                "Linux runtime requires a Unix socket endpoint, got {endpoint:?}"
            )))
        }
    };
    let exec = shell_escape_path(&spec.executable);
    Ok(format!(
        r#"[Unit]
Description=Moraine local integration runtime (per-user)
After=network.target

[Service]
Type=simple
ExecStart={exec} --http {http} --unix-socket {socket} --spool-dir {spool}
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
"#,
        http = spec.diagnostics_endpoint,
        socket = socket.display(),
        spool = shell_escape_path(&spec.spool_dir),
    ))
}

fn shell_escape_path(path: &Path) -> String {
    let value = path.display().to_string();
    if value.contains(' ') || value.contains('\\') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value
    }
}

impl LinuxSystemdUserRuntime {
    pub fn new() -> Self {
        Self {
            suite: SuitePaths::discover(),
        }
    }

    pub fn with_suite(suite: SuitePaths) -> Self {
        Self { suite }
    }

    fn systemctl(args: &[&str]) -> std::result::Result<std::process::ExitStatus, String> {
        Command::new("systemctl")
            .arg("--user")
            .args(args)
            .status()
            .map_err(|e| e.to_string())
    }

    fn unit_active() -> Option<String> {
        Command::new("systemctl")
            .args(["--user", "is-active", "moraine-service.service"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }

    fn unit_enabled() -> Option<String> {
        Command::new("systemctl")
            .args(["--user", "is-enabled", "moraine-service.service"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }
}

impl Default for LinuxSystemdUserRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl super::BackgroundRuntimeManager for LinuxSystemdUserRuntime {
    fn inspect(&self) -> Result<BackgroundRuntimeState> {
        let binary = self.suite.absolute_service();
        let binary_present = binary.as_ref().map(|p| p.is_file()).unwrap_or(false);
        let registration_present = self.suite.unit.is_file();
        let registration_valid =
            registration_present && unit_exec_matches_suite(&self.suite.unit, binary.as_deref());
        let active = Self::unit_active();
        let running_unit = active.as_deref() == Some("active");
        let (http_online, version) = match http_get_loopback(33111, "/status") {
            Ok(body) => {
                let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                let ver = v
                    .get("version")
                    .or_else(|| v.get("productVersion"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                (true, ver)
            }
            Err(_) => (false, None),
        };
        let running = running_unit || http_online;
        let status_message = if running {
            "Background capture is running".into()
        } else if registration_present && !binary_present {
            "Background capture is registered but its program is missing".into()
        } else if binary_present && !registration_present {
            "Background capture program is present but not registered".into()
        } else if registration_present {
            "Background capture is installed but not running".into()
        } else {
            "Background capture is not set up".into()
        };
        let autostart_enabled = Self::unit_enabled().as_deref() == Some("enabled");
        Ok(BackgroundRuntimeState {
            backend: BackgroundRuntimeBackend::LinuxSystemdUser,
            supported: true,
            // "Installed" means registered for start — not binary-only.
            installed: registration_present,
            binary_present,
            registration_present,
            registration_valid,
            running,
            autostart_enabled,
            endpoint_ready: http_online,
            diagnostics_ready: http_online,
            capture_ready: http_online,
            binary_path: binary.map(|p| p.display().to_string()),
            unit_path: Some(self.suite.unit.display().to_string()),
            version,
            status_message,
            platform: "linux".into(),
            registration: registration_present.then(|| RuntimeRegistrationState {
                kind: RuntimeRegistrationKind::SystemdUserUnit,
                location: Some(self.suite.unit.display().to_string()),
                fingerprint: crate::snapshot::file_sha256(&self.suite.unit).ok(),
            }),
        })
    }

    fn install(&self, executable: &Path) -> Result<()> {
        let layout = moraine_platform::RuntimeLayout::discover();
        self.install_runtime(&RuntimeInstallSpec {
            executable: executable.to_path_buf(),
            capture_endpoint: layout.capture_endpoint,
            diagnostics_endpoint: layout.diagnostics_endpoint,
            spool_dir: layout.spool_dir,
        })
    }

    fn install_runtime(&self, spec: &RuntimeInstallSpec) -> Result<()> {
        if !cfg!(target_os = "linux") {
            return Err(ProvisionError::Service(
                "Linux service install is only supported on Linux".into(),
            ));
        }
        if !spec.executable.is_file() {
            return Err(ProvisionError::Service(format!(
                "service binary not found at {}",
                spec.executable.display()
            )));
        }
        let unit = render_systemd_unit(spec)?;
        if let Some(parent) = self.suite.unit.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.suite.unit, &unit)?;
        let _ = Self::systemctl(&["daemon-reload"]);
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let _ = Self::systemctl(&["stop", "moraine-service.service"]);
        let _ = Self::systemctl(&["disable", "moraine-service.service"]);
        if self.suite.unit.is_file() {
            fs::remove_file(&self.suite.unit)?;
        }
        let _ = Self::systemctl(&["daemon-reload"]);
        Ok(())
    }

    fn start(&self) -> Result<()> {
        let st = Self::systemctl(&["start", "moraine-service.service"])
            .map_err(ProvisionError::Service)?;
        if !st.success() {
            return Err(ProvisionError::Service(
                "failed to start background capture".into(),
            ));
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let _ = Self::systemctl(&["stop", "moraine-service.service"]);
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        let st = Self::systemctl(&["restart", "moraine-service.service"])
            .map_err(ProvisionError::Service)?;
        if !st.success() {
            return Err(ProvisionError::Service(
                "failed to restart background capture".into(),
            ));
        }
        Ok(())
    }

    fn enable_autostart(&self) -> Result<()> {
        let st = Self::systemctl(&["enable", "moraine-service.service"])
            .map_err(ProvisionError::Service)?;
        if !st.success() {
            return Err(ProvisionError::Service(
                "failed to enable background capture at login".into(),
            ));
        }
        Ok(())
    }

    fn disable_autostart(&self) -> Result<()> {
        let st = Self::systemctl(&["disable", "moraine-service.service"])
            .map_err(ProvisionError::Service)?;
        if !st.success() {
            return Err(ProvisionError::Service(
                "failed to disable background capture at login".into(),
            ));
        }
        Ok(())
    }

    fn reload_registration(&self) -> Result<()> {
        // Required after unit file restore so systemd picks up prior ExecStart.
        let st = Self::systemctl(&["daemon-reload"]).map_err(ProvisionError::Service)?;
        if !st.success() {
            return Err(ProvisionError::Service(
                "failed to reload background capture registration".into(),
            ));
        }
        Ok(())
    }

    fn logs(&self, limit: usize) -> Result<Vec<ServiceLog>> {
        let n = limit.to_string();
        let output = Command::new("journalctl")
            .args([
                "--user",
                "-u",
                "moraine-service.service",
                "-n",
                &n,
                "--no-pager",
                "-o",
                "cat",
            ])
            .output()
            .map_err(|e| ProvisionError::Service(e.to_string()))?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .lines()
            .map(|line| ServiceLog {
                line: line.to_string(),
                timestamp: None,
            })
            .collect())
    }
}

/// True when unit ExecStart canonicalizes to the suite service binary.
fn unit_exec_matches_suite(unit_path: &Path, suite_service: Option<&Path>) -> bool {
    let Some(suite) = suite_service else {
        return false;
    };
    let Ok(unit) = fs::read_to_string(unit_path) else {
        return false;
    };
    let exec = unit.lines().find_map(|l| {
        let t = l.trim();
        t.strip_prefix("ExecStart=")
            .map(|s| s.trim().trim_matches('"').to_string())
    });
    let Some(exec_line) = exec else {
        return false;
    };
    let bin = exec_line.split_whitespace().next().unwrap_or("");
    if bin.is_empty() {
        return false;
    }
    let exec_path = Path::new(bin);
    match (fs::canonicalize(exec_path), fs::canonicalize(suite)) {
        (Ok(a), Ok(b)) => a == b,
        _ => exec_path == suite,
    }
}
