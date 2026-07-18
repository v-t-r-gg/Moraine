//! Share / collab URL helpers (pure; no network).

use std::path::Path;

use serde::Serialize;

use crate::room::room_id_for_path;

pub const DEFAULT_RELAY_HTTP: &str = "http://127.0.0.1:3099";
pub const DEFAULT_RELAY_WS: &str = "ws://127.0.0.1:3099";
pub const DEFAULT_UI: &str = "http://localhost:1420";
pub const DEFAULT_RELAY_BIND: &str = "127.0.0.1:3099";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShareLinks {
    pub path: String,
    pub room: String,
    pub url: String,
    pub ws: String,
    pub server: String,
}

pub fn trim_base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

pub fn http_to_ws(server_http: &str) -> String {
    let s = trim_base(server_http);
    if let Some(rest) = s.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = s.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if s.starts_with("ws://") || s.starts_with("wss://") {
        s
    } else {
        format!("ws://{s}")
    }
}

/// Join URL for the web UI (`?room=` enables the default relay on the client).
pub fn share_links(path: &Path, ui: &str, server_http: &str) -> ShareLinks {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let room = room_id_for_path(&abs);
    let server = trim_base(server_http);
    let ui = trim_base(ui);
    let ws_base = http_to_ws(&server);
    ShareLinks {
        path: abs.display().to_string(),
        room: room.clone(),
        url: format!("{ui}/?room={room}"),
        ws: format!("{ws_base}/ws/{room}"),
        server,
    }
}

/// Host:port for `--bind` from an HTTP base like `http://127.0.0.1:3099`.
pub fn bind_from_http(server_http: &str) -> Option<String> {
    let s = trim_base(server_http);
    let rest = s
        .strip_prefix("http://")
        .or_else(|| s.strip_prefix("https://"))?;
    let hostport = rest.split('/').next().unwrap_or(rest);
    if hostport.contains(':') {
        Some(hostport.to_string())
    } else {
        let port = if s.starts_with("https://") { 443 } else { 80 };
        Some(format!("{hostport}:{port}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn http_ws_mapping() {
        assert_eq!(http_to_ws("http://127.0.0.1:3099"), "ws://127.0.0.1:3099");
        assert_eq!(http_to_ws("https://ex.com/"), "wss://ex.com");
        assert_eq!(http_to_ws("ws://x:1"), "ws://x:1");
    }

    #[test]
    fn share_links_stable_room() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("n.md");
        fs::write(&path, "hi").unwrap();
        let a = share_links(&path, DEFAULT_UI, DEFAULT_RELAY_HTTP);
        let b = share_links(&path, DEFAULT_UI, DEFAULT_RELAY_HTTP);
        assert_eq!(a.room, b.room);
        assert!(a.url.contains(&format!("room={}", a.room)));
        assert!(a.ws.ends_with(&format!("/ws/{}", a.room)));
        assert_eq!(
            bind_from_http(DEFAULT_RELAY_HTTP).as_deref(),
            Some(DEFAULT_RELAY_BIND)
        );
    }
}
