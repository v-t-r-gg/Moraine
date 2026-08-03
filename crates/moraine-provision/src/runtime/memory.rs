//! In-memory / test service manager (no OS side effects).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{ProvisionError, Result};
use crate::types::{
    BackgroundRuntimeBackend, BackgroundRuntimeState, RuntimeRegistrationKind,
    RuntimeRegistrationSnapshot, RuntimeRegistrationState, ServiceLog,
};

#[derive(Debug, Default)]
struct Inner {
    installed: bool,
    running: bool,
    autostart: bool,
    binary: Option<PathBuf>,
    /// When set, the next start/install call fails with this message (test injection).
    fail_next: Option<String>,
    fail_after_install: Option<String>,
    fail_next_start: Option<String>,
    fail_next_stop: Option<String>,
    fail_inspect_after: Option<(u32, String)>,
    inspect_count: u32,
    install_count: u32,
    start_count: u32,
    stop_count: u32,
    /// Count of reload_registration calls (tests assert daemon-reload equivalent).
    reload_count: u32,
    endpoint_ready_override: Option<bool>,
}

/// Deterministic service manager for unit tests and non-Linux stubs.
#[derive(Debug, Default)]
pub struct MemoryRuntimeManager {
    inner: Mutex<Inner>,
    /// When set, install() writes a unit file here (hermetic registration tests).
    unit_path: Option<PathBuf>,
}

impl MemoryRuntimeManager {
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

    /// Inject an install error after the registration mutation occurred.
    pub fn fail_after_install(&self, msg: impl Into<String>) {
        self.inner.lock().unwrap().fail_after_install = Some(msg.into());
    }

    pub fn fail_next_start(&self, msg: impl Into<String>) {
        self.inner.lock().unwrap().fail_next_start = Some(msg.into());
    }

    pub fn fail_next_stop(&self, msg: impl Into<String>) {
        self.inner.lock().unwrap().fail_next_stop = Some(msg.into());
    }

    /// Fail inspect after `successful_calls` further successful inspections.
    pub fn fail_inspect_after(&self, successful_calls: u32, msg: impl Into<String>) {
        self.inner.lock().unwrap().fail_inspect_after = Some((successful_calls, msg.into()));
    }

    pub fn operation_counts(&self) -> (u32, u32, u32) {
        let inner = self.inner.lock().unwrap();
        (inner.install_count, inner.start_count, inner.stop_count)
    }

    pub fn inspect_count(&self) -> u32 {
        self.inner.lock().unwrap().inspect_count
    }

    pub fn reload_count(&self) -> u32 {
        self.inner.lock().unwrap().reload_count
    }

    pub fn set_endpoint_ready_override(&self, ready: Option<bool>) {
        self.inner.lock().unwrap().endpoint_ready_override = ready;
    }

    /// Test helper: diagnostics/capture appear ready without a product registration.
    ///
    /// Models Windows `running = task.running || diagnostics_ready` when a process
    /// answers loopback diagnostics but no Task Scheduler task exists.
    pub fn simulate_orphan_endpoint(&self, ready: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.installed = false;
        inner.running = ready;
        inner.binary = None;
        inner.endpoint_ready_override = Some(ready);
    }
}

impl super::BackgroundRuntimeManager for MemoryRuntimeManager {
    fn inspect(&self) -> Result<BackgroundRuntimeState> {
        let mut g = self.inner.lock().unwrap();
        if let Some((remaining, message)) = g.fail_inspect_after.as_mut() {
            if *remaining == 0 {
                let message = message.clone();
                g.fail_inspect_after = None;
                return Err(ProvisionError::Service(message));
            }
            *remaining -= 1;
        }
        g.inspect_count = g.inspect_count.saturating_add(1);
        let binary_present =
            g.binary.as_ref().map(|p| p.is_file()).unwrap_or(false) || g.binary.is_some();
        let unit_path = self.unit_path.as_ref().map(|p| p.display().to_string());
        // Prefer on-disk unit when configured (hermetic unit repair tests).
        let registration_present = if let Some(ref up) = self.unit_path {
            up.is_file() || g.installed
        } else {
            g.installed
        };
        let endpoint_ready = g.endpoint_ready_override.unwrap_or(g.running);
        Ok(BackgroundRuntimeState {
            backend: BackgroundRuntimeBackend::MemoryTest,
            supported: true,
            installed: registration_present,
            running: g.running,
            binary_present,
            registration_present,
            registration_valid: registration_present && binary_present,
            autostart_enabled: g.autostart,
            endpoint_ready,
            diagnostics_ready: endpoint_ready,
            capture_ready: endpoint_ready,
            binary_path: g.binary.as_ref().map(|p| p.display().to_string()),
            unit_path,
            version: None,
            last_result: None,
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
            registration: registration_present.then(|| RuntimeRegistrationState {
                kind: RuntimeRegistrationKind::SystemdUserUnit,
                location: self
                    .unit_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
                fingerprint: self
                    .unit_path
                    .as_ref()
                    .and_then(|path| crate::snapshot::file_sha256(path).ok()),
            }),
        })
    }

    fn capture_registration(&self) -> Result<RuntimeRegistrationSnapshot> {
        let path = self
            .unit_path
            .clone()
            .or_else(|| crate::suite::SuitePaths::discover().service_registration)
            .ok_or_else(|| {
                ProvisionError::Service("memory runtime registration path is not configured".into())
            })?;
        let snapshot = if path.is_file() {
            crate::snapshot::durable_backup(&path)?
        } else {
            crate::snapshot::snapshot_absent(&path)
        };
        Ok(RuntimeRegistrationSnapshot::File(snapshot))
    }

    fn registration_fingerprint(&self) -> Result<Option<String>> {
        let Some(path) = &self.unit_path else {
            return Ok(None);
        };
        Ok(if path.is_file() {
            Some(crate::snapshot::file_sha256(path)?)
        } else {
            None
        })
    }

    fn install(&self, executable: &Path) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        g.install_count = g.install_count.saturating_add(1);
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
        if let Some(msg) = g.fail_after_install.take() {
            return Err(ProvisionError::Service(msg));
        }
        Ok(())
    }

    fn restore_registration(&self, snapshot: &RuntimeRegistrationSnapshot) -> Result<()> {
        let RuntimeRegistrationSnapshot::File(snapshot) = snapshot else {
            return Err(crate::ProvisionError::Service(
                "memory runtime cannot restore a non-file registration snapshot".into(),
            ));
        };
        crate::snapshot::restore_snapshot(snapshot)?;
        let mut inner = self.inner.lock().unwrap();
        inner.reload_count = inner.reload_count.saturating_add(1);
        inner.installed = matches!(snapshot, crate::types::FileSnapshot::Existing { .. });
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
        g.start_count = g.start_count.saturating_add(1);
        if let Some(msg) = g.fail_next_start.take() {
            return Err(ProvisionError::Service(msg));
        }
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
        g.stop_count = g.stop_count.saturating_add(1);
        if let Some(msg) = g.fail_next_stop.take() {
            return Err(ProvisionError::Service(msg));
        }
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

    fn logs(&self, _limit: usize) -> Result<Vec<ServiceLog>> {
        Ok(vec![ServiceLog {
            line: "memory service manager (no real logs)".into(),
            timestamp: None,
        }])
    }
}
