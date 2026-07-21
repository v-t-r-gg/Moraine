//! `moraine service` lifecycle (systemd --user).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::json;

use crate::suite::{
    default_http_addr, default_socket_path, http_get_loopback, render_systemd_unit, systemctl_user,
    SuitePaths,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCmdResult {
    pub ok: bool,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

pub fn service_install(json: bool) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("service install is only supported on Linux/systemd");
    }
    let suite = SuitePaths::discover();
    let service_bin = if suite.service.is_file() {
        suite.service.clone()
    } else {
        // Fall back to same-directory as CLI for dev bundles
        let exe = std::env::current_exe().context("current_exe")?;
        let sibling = exe
            .parent()
            .map(|p| p.join("moraine-service"))
            .filter(|p| p.is_file());
        sibling.unwrap_or(suite.service.clone())
    };
    if !service_bin.is_file() {
        let r = ServiceCmdResult {
            ok: false,
            action: "install".into(),
            message: Some(format!(
                "service binary not found at {} (install the release suite first)",
                service_bin.display()
            )),
            unit_path: None,
            exec_start: None,
            code: Some("service_binary_missing".into()),
        };
        print_result(json, &r);
        bail!("{}", r.message.unwrap_or_default());
    }
    let socket = default_socket_path();
    let unit = render_systemd_unit(
        &service_bin,
        default_http_addr(),
        &socket.display().to_string(),
    );
    if let Some(parent) = suite.unit.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&suite.unit, &unit)?;
    let _ = systemctl_user(&["daemon-reload"]);
    let r = ServiceCmdResult {
        ok: true,
        action: "install".into(),
        message: Some(format!("wrote {}", suite.unit.display())),
        unit_path: Some(suite.unit.display().to_string()),
        exec_start: Some(service_bin.display().to_string()),
        code: None,
    };
    print_result(json, &r);
    Ok(())
}

pub fn service_start(json: bool) -> Result<()> {
    let st =
        systemctl_user(&["start", "moraine-service.service"]).map_err(|e| anyhow::anyhow!(e))?;
    let r = ServiceCmdResult {
        ok: st.success(),
        action: "start".into(),
        message: Some(format!("systemctl --user start → {st}")),
        unit_path: None,
        exec_start: None,
        code: (!st.success()).then(|| "systemctl_failed".into()),
    };
    print_result(json, &r);
    if !st.success() {
        bail!("service start failed");
    }
    Ok(())
}

pub fn service_stop(json: bool) -> Result<()> {
    let st =
        systemctl_user(&["stop", "moraine-service.service"]).map_err(|e| anyhow::anyhow!(e))?;
    let r = ServiceCmdResult {
        ok: st.success(),
        action: "stop".into(),
        message: Some(format!("systemctl --user stop → {st}")),
        unit_path: None,
        exec_start: None,
        code: (!st.success()).then(|| "systemctl_failed".into()),
    };
    print_result(json, &r);
    Ok(())
}

pub fn service_restart(json: bool) -> Result<()> {
    let st =
        systemctl_user(&["restart", "moraine-service.service"]).map_err(|e| anyhow::anyhow!(e))?;
    let r = ServiceCmdResult {
        ok: st.success(),
        action: "restart".into(),
        message: Some(format!("systemctl --user restart → {st}")),
        unit_path: None,
        exec_start: None,
        code: (!st.success()).then(|| "systemctl_failed".into()),
    };
    print_result(json, &r);
    if !st.success() {
        bail!("service restart failed");
    }
    Ok(())
}

pub fn service_status(json: bool) -> Result<()> {
    let suite = SuitePaths::discover();
    let unit_active = Command::new("systemctl")
        .args(["--user", "is-active", "moraine-service.service"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string());
    let diag = http_get_loopback(33111, "/status").ok();
    let body = json!({
        "ok": true,
        "action": "status",
        "unitPath": suite.unit.display().to_string(),
        "unitActive": unit_active,
        "serviceBinary": suite.service.display().to_string(),
        "serviceBinaryPresent": suite.service.is_file(),
        "diagnostics": diag.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        println!(
            "unit: {} ({})",
            suite.unit.display(),
            unit_active.as_deref().unwrap_or("unknown")
        );
        println!(
            "binary: {} ({})",
            suite.service.display(),
            if suite.service.is_file() {
                "present"
            } else {
                "missing"
            }
        );
        if let Some(d) = body.get("diagnostics") {
            println!("diagnostics: {d}");
        } else {
            println!("diagnostics: offline");
        }
    }
    Ok(())
}

pub fn service_logs(json: bool) -> Result<()> {
    if json {
        // journalctl text is not structured; return exit status only
        let st = Command::new("journalctl")
            .args([
                "--user",
                "-u",
                "moraine-service.service",
                "-n",
                "50",
                "--no-pager",
            ])
            .status()
            .context("journalctl")?;
        println!(
            "{}",
            json!({"ok": st.success(), "action": "logs", "note": "logs printed to stderr/stdout by journalctl"})
        );
    } else {
        let st = Command::new("journalctl")
            .args([
                "--user",
                "-u",
                "moraine-service.service",
                "-n",
                "80",
                "--no-pager",
            ])
            .status()
            .context("journalctl")?;
        if !st.success() {
            bail!("journalctl failed: {st}");
        }
    }
    Ok(())
}

pub fn service_uninstall(json: bool) -> Result<()> {
    let suite = SuitePaths::discover();
    let _ = systemctl_user(&["stop", "moraine-service.service"]);
    let _ = systemctl_user(&["disable", "moraine-service.service"]);
    if suite.unit.is_file() {
        fs::remove_file(&suite.unit)?;
    }
    let _ = systemctl_user(&["daemon-reload"]);
    let r = ServiceCmdResult {
        ok: true,
        action: "uninstall".into(),
        message: Some("removed user unit; project ledgers and spool data retained".into()),
        unit_path: Some(suite.unit.display().to_string()),
        exec_start: None,
        code: None,
    };
    print_result(json, &r);
    Ok(())
}

fn print_result(json: bool, r: &ServiceCmdResult) {
    if json {
        println!("{}", serde_json::to_string_pretty(r).unwrap_or_default());
    } else if let Some(m) = &r.message {
        println!("{m}");
    }
}

#[allow(dead_code)]
pub fn unit_path() -> PathBuf {
    SuitePaths::discover().unit
}
