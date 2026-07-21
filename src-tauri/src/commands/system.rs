use tauri::State;

use crate::state::AppState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub git_commit: String,
    pub data_dir: String,
    pub history_dir: String,
    pub config_dir: String,
    /// Installed suite / diagnostics (C2 About surface).
    pub service_online: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    pub service_compatible: bool,
    pub doctor_hint: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    let build = moraine_core::BuildIdentity::current();
    let (service_online, service_version, service_compatible) = probe_service(&build.version);
    AppInfo {
        name: "Moraine".into(),
        version: build.version.clone(),
        git_commit: build.git_commit.clone(),
        data_dir: state.paths.data_dir.display().to_string(),
        history_dir: state.paths.history_dir.display().to_string(),
        config_dir: state.paths.config_dir.display().to_string(),
        service_online,
        service_version,
        service_compatible,
        doctor_hint: "moraine doctor --json".into(),
    }
}

fn probe_service(cli_version: &str) -> (bool, Option<String>, bool) {
    // Reuse discovery native client path (loopback only).
    match crate::commands::discovery::discovery_status() {
        Ok(st) if st.online => {
            // status DTO may not include version; best-effort HTTP via discovery is enough for online.
            let ver = std::net::TcpStream::connect_timeout(
                &"127.0.0.1:33111".parse().unwrap(),
                std::time::Duration::from_millis(80),
            )
            .ok()
            .and_then(|_| fetch_service_version());
            let compatible = ver.as_ref().map(|v| v == cli_version).unwrap_or(true);
            (true, ver, compatible)
        }
        _ => (false, None, true),
    }
}

fn fetch_service_version() -> Option<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;
    let mut stream =
        TcpStream::connect_timeout(&"127.0.0.1:33111".parse().ok()?, Duration::from_millis(400))
            .ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    stream
        .write_all(b"GET /status HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let raw = String::from_utf8_lossy(&buf);
    let body = raw.split("\r\n\r\n").nth(1)?;
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("productVersion")
        .or_else(|| v.get("version"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

#[tauri::command]
pub fn take_startup_path(state: State<'_, AppState>) -> Option<String> {
    state.take_pending_open().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Moraine is ready, {name}!")
}
