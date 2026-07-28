//! Shared loopback diagnostics parsing and endpoint resolution.

use crate::suite::http_get_loopback;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsStatus {
    pub online: bool,
    pub capture_ready: bool,
    pub version: Option<String>,
}

pub fn parse_status(body: &str) -> DiagnosticsStatus {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    DiagnosticsStatus {
        online: value
            .get("online")
            .and_then(|field| field.as_bool())
            .unwrap_or(false),
        capture_ready: value
            .get("captureReady")
            .and_then(|field| field.as_bool())
            .unwrap_or(false),
        version: value
            .get("version")
            .or_else(|| value.get("productVersion"))
            .and_then(|field| field.as_str())
            .map(str::to_owned),
    }
}

pub fn probe_default() -> std::result::Result<DiagnosticsStatus, String> {
    let endpoint = moraine_platform::RuntimeLayout::discover().diagnostics_endpoint;
    http_get_loopback(endpoint.port(), "/status").map(|body| parse_status(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_without_capture_is_not_capture_ready() {
        let status = parse_status(r#"{"online":true,"captureReady":false}"#);
        assert!(status.online);
        assert!(!status.capture_ready);
    }

    #[test]
    fn online_with_bound_capture_is_ready() {
        let status = parse_status(r#"{"online":true,"captureReady":true,"version":"0.1.0"}"#);
        assert!(status.online && status.capture_ready);
        assert_eq!(status.version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn missing_capture_field_fails_closed() {
        assert!(!parse_status(r#"{"online":true}"#).capture_ready);
    }
}
