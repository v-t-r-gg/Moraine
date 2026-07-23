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
}

/// Deterministic service manager for unit tests and non-Linux stubs.
#[derive(Debug, Default)]
pub struct MemoryServiceManager {
    inner: Mutex<Inner>,
}

impl MemoryServiceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_next(&self, msg: impl Into<String>) {
        self.inner.lock().unwrap().fail_next = Some(msg.into());
    }
}

impl super::ServiceManager for MemoryServiceManager {
    fn inspect(&self) -> Result<ServiceState> {
        let g = self.inner.lock().unwrap();
        Ok(ServiceState {
            installed: g.installed,
            running: g.running,
            binary_present: g.binary.as_ref().map(|p| p.is_file()).unwrap_or(false)
                || g.binary.is_some(),
            binary_path: g.binary.as_ref().map(|p| p.display().to_string()),
            unit_path: None,
            version: None,
            status_message: if g.running {
                "Background capture is running".into()
            } else if g.installed {
                "Background capture is installed but not running".into()
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
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.installed = false;
        g.running = false;
        g.autostart = false;
        g.binary = None;
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

    fn logs(&self, _limit: usize) -> Result<Vec<ServiceLog>> {
        Ok(vec![ServiceLog {
            line: "memory service manager (no real logs)".into(),
            timestamp: None,
        }])
    }
}
