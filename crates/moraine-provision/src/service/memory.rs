//! In-memory / test service manager (no OS side effects).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{ProvisionError, Result};
use crate::types::{ServiceLog, ServiceState};

#[derive(Debug, Default)]
struct Inner {
    installed: bool,
    running: bool,
    autostart: bool,
    binary: Option<PathBuf>,
    /// When set, the next start/install call fails with this message (test injection).
    fail_next: Option<String>,
    /// Count of reload_registration calls (tests assert daemon-reload equivalent).
    reload_count: u32,
}

/// Deterministic service manager for unit tests and non-Linux stubs.
#[derive(Debug, Default)]
pub struct MemoryServiceManager {
    inner: Mutex<Inner>,
    /// When set, install() writes a unit file here (hermetic registration tests).
    unit_path: Option<PathBuf>,
}

impl MemoryServiceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_unit_path(unit_path: PathBuf) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            unit_path: Some(unit_path),
        }
    }

    pub fn fail_next(&self, msg: impl Into<String>) {
        self.inner.lock().unwrap().fail_next = Some(msg.into());
    }

    pub fn reload_count(&self) -> u32 {
        self.inner.lock().unwrap().reload_count
    }
}

impl super::ServiceManager for MemoryServiceManager {
    fn inspect(&self) -> Result<ServiceState> {
        let g = self.inner.lock().unwrap();
        let binary_present =
            g.binary.as_ref().map(|p| p.is_file()).unwrap_or(false) || g.binary.is_some();
        let unit_path = self.unit_path.as_ref().map(|p| p.display().to_string());
        // Prefer on-disk unit when configured (hermetic unit repair tests).
        let registration_present = if let Some(ref up) = self.unit_path {
            up.is_file() || g.installed
        } else {
            g.installed
        };
        Ok(ServiceState {
            installed: registration_present,
            running: g.running,
            binary_present,
            registration_present,
            registration_valid: registration_present && binary_present,
            autostart_enabled: g.autostart,
            endpoint_ready: g.running,
            binary_path: g.binary.as_ref().map(|p| p.display().to_string()),
            unit_path,
            version: None,
            status_message: if g.running {
                "Background capture is running".into()
            } else if registration_present {
                "Background capture is installed but not running".into()
            } else if binary_present {
                "Background capture program is present but not registered".into()
            } else {
                "Background capture is not set up".into()
            },
            platform: "memory".into(),
        })
    }

    fn install(&self, executable: &Path) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        if let Some(msg) = g.fail_next.take() {
            return Err(ProvisionError::Service(msg));
        }
        g.binary = Some(executable.to_path_buf());
        g.installed = true;
        if let Some(ref unit) = self.unit_path {
            if let Some(parent) = unit.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Overwrite registration (mirrors systemd unit rewrite on repair).
            std::fs::write(
                unit,
                format!("# memory unit\nExecStart={}\n", executable.display()),
            )?;
        }
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.installed = false;
        g.running = false;
        g.autostart = false;
        g.binary = None;
        if let Some(ref unit) = self.unit_path {
            let _ = std::fs::remove_file(unit);
        }
        Ok(())
    }

    fn start(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        if let Some(msg) = g.fail_next.take() {
            return Err(ProvisionError::Service(msg));
        }
        if !g.installed {
            return Err(ProvisionError::Service(
                "cannot start: background capture is not installed".into(),
            ));
        }
        g.running = true;
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.running = false;
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        self.stop()?;
        self.start()
    }

    fn enable_autostart(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        if !g.installed {
            return Err(ProvisionError::Service(
                "cannot enable autostart: not installed".into(),
            ));
        }
        g.autostart = true;
        Ok(())
    }

    fn disable_autostart(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.autostart = false;
        Ok(())
    }

    fn reload_registration(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.reload_count = g.reload_count.saturating_add(1);
        Ok(())
    }

    fn logs(&self, _limit: usize) -> Result<Vec<ServiceLog>> {
        Ok(vec![ServiceLog {
            line: "memory service manager (no real logs)".into(),
            timestamp: None,
        }])
    }
}
