//! Linux systemd --user implementation of ServiceManager.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::{ProvisionError, Result};
use crate::suite::{
    default_http_addr, default_socket_path, http_get_loopback, render_systemd_unit, SuitePaths,
};
use crate::types::{ServiceLog, ServiceState};

pub struct LinuxSystemdUserService {
    suite: SuitePaths,
}

impl LinuxSystemdUserService {
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
}

impl Default for LinuxSystemdUserService {
    fn default() -> Self {
        Self::new()
    }
}

impl super::ServiceManager for LinuxSystemdUserService {
    fn inspect(&self) -> Result<ServiceState> {
        let binary = self.suite.absolute_service();
        let binary_present = binary.as_ref().map(|p| p.is_file()).unwrap_or(false);
        let registration_present = self.suite.unit.is_file();
        let registration_valid = registration_present
            && (binary_present
                || self
                    .suite
                    .unit
                    .is_file()
                    .then(|| std::fs::read_to_string(&self.suite.unit).ok())
                    .flatten()
                    .map(|u| u.contains("ExecStart="))
                    .unwrap_or(false));
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
        Ok(ServiceState {
            // "Installed" means registered for start — not binary-only.
            installed: registration_present,
            binary_present,
            registration_present,
            registration_valid,
            running,
            endpoint_ready: http_online,
            binary_path: binary.map(|p| p.display().to_string()),
            unit_path: Some(self.suite.unit.display().to_string()),
            version,
            status_message,
            platform: "linux".into(),
        })
    }

    fn install(&self, executable: &Path) -> Result<()> {
        if !cfg!(target_os = "linux") {
            return Err(ProvisionError::Service(
                "Linux service install is only supported on Linux".into(),
            ));
        }
        if !executable.is_file() {
            return Err(ProvisionError::Service(format!(
                "service binary not found at {}",
                executable.display()
            )));
        }
        let socket = default_socket_path();
        let unit = render_systemd_unit(
            executable,
            default_http_addr(),
            &socket.display().to_string(),
        );
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


