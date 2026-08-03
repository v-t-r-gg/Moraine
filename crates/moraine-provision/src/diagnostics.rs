//! Shared loopback diagnostics parsing and endpoint resolution.

use moraine_core::SERVICE_PROTOCOL_VERSION;
use moraine_platform::CaptureEndpoint;

use crate::suite::http_get_loopback;

pub use moraine_core::SERVICE_PRODUCT_ID;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsStatus {
    pub online: bool,
    pub capture_ready: bool,
    pub version: Option<String>,
    pub product: Option<String>,
    pub protocol_version: Option<u32>,
    pub scope_id: Option<String>,
    pub capture_endpoint: Option<CaptureEndpoint>,
}

/// Optional identity that a `/status` response must match.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosticsExpectation {
    pub capture_endpoint: Option<CaptureEndpoint>,
    pub scope_id: Option<String>,
}

impl DiagnosticsExpectation {
    pub fn from_runtime_layout(layout: &moraine_platform::RuntimeLayout) -> Self {
        let capture_endpoint = match &layout.capture_endpoint {
            CaptureEndpoint::Unsupported => None,
            endpoint => Some(endpoint.clone()),
        };
        let scope_id = match &layout.capture_endpoint {
            CaptureEndpoint::WindowsNamedPipe(name) => scope_id_from_pipe_name(name),
            _ => None,
        };
        Self {
            capture_endpoint,
            scope_id,
        }
    }
}

fn scope_id_from_pipe_name(name: &str) -> Option<String> {
    // \\.\pipe\moraine.capture.v1.<scope_id>
    let base = name.rsplit(['\\', '/']).next().unwrap_or(name);
    base.strip_prefix("moraine.capture.v1.")
        .filter(|scope| !scope.is_empty())
        .map(str::to_owned)
}

/// Parse a `/status` body. Unknown or non-Moraine responders fail closed
/// (`online = false`) even when they set generic JSON fields.
pub fn parse_status(body: &str) -> DiagnosticsStatus {
    parse_status_with_expectation(body, None)
}

pub fn parse_status_with_expectation(
    body: &str,
    expected: Option<&DiagnosticsExpectation>,
) -> DiagnosticsStatus {
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return DiagnosticsStatus::offline(),
    };

    let product = value
        .get("product")
        .and_then(|field| field.as_str())
        .map(str::to_owned);
    let protocol_version = value
        .get("serviceProtocolVersion")
        .or_else(|| value.get("protocolVersion"))
        .and_then(|field| field.as_u64())
        .and_then(|version| u32::try_from(version).ok());
    let version = value
        .get("version")
        .or_else(|| value.get("productVersion"))
        .and_then(|field| field.as_str())
        .map(str::to_owned);
    let scope_id = value
        .get("scopeId")
        .and_then(|field| field.as_str())
        .map(str::to_owned);
    let capture_endpoint = value
        .get("captureEndpoint")
        .and_then(|field| serde_json::from_value::<CaptureEndpoint>(field.clone()).ok());
    let claimed_online = value
        .get("online")
        .and_then(|field| field.as_bool())
        .unwrap_or(false);
    let capture_ready = value
        .get("captureReady")
        .and_then(|field| field.as_bool())
        .unwrap_or(false);

    let identity_ok = product.as_deref() == Some(SERVICE_PRODUCT_ID)
        && protocol_version == Some(SERVICE_PROTOCOL_VERSION);

    let expectation_ok = match expected {
        None => true,
        Some(expectation) => {
            let endpoint_ok = match (&expectation.capture_endpoint, &capture_endpoint) {
                (Some(expected_endpoint), Some(actual)) => expected_endpoint == actual,
                // When we know the expected capture endpoint, require the service
                // to report it. Missing or mismatched endpoints fail closed.
                (Some(_), None) => false,
                (None, _) => true,
            };
            let scope_ok = match (&expectation.scope_id, &scope_id) {
                (Some(expected_scope), Some(actual)) => expected_scope == actual,
                (Some(_), None) => false,
                (None, _) => true,
            };
            endpoint_ok && scope_ok
        }
    };

    let online = claimed_online && identity_ok && expectation_ok;
    DiagnosticsStatus {
        online,
        // Capture readiness is only meaningful for an authenticated Moraine
        // diagnostics responder.
        capture_ready: online && capture_ready,
        version: if identity_ok { version } else { None },
        product,
        protocol_version,
        scope_id,
        capture_endpoint,
    }
}

impl DiagnosticsStatus {
    fn offline() -> Self {
        Self {
            online: false,
            capture_ready: false,
            version: None,
            product: None,
            protocol_version: None,
            scope_id: None,
            capture_endpoint: None,
        }
    }
}

pub fn probe_default() -> std::result::Result<DiagnosticsStatus, String> {
    let layout = moraine_platform::RuntimeLayout::discover();
    let expected = DiagnosticsExpectation::from_runtime_layout(&layout);
    http_get_loopback(layout.diagnostics_endpoint.port(), "/status")
        .map(|body| parse_status_with_expectation(&body, Some(&expected)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use moraine_platform::CaptureEndpoint;

    fn valid_ready_body() -> String {
        serde_json::json!({
            "product": SERVICE_PRODUCT_ID,
            "serviceProtocolVersion": SERVICE_PROTOCOL_VERSION,
            "online": true,
            "captureReady": true,
            "version": "0.1.0",
            "captureEndpoint": {
                "kind": "unix_socket",
                "value": "/tmp/moraine.sock"
            }
        })
        .to_string()
    }

    #[test]
    fn online_without_capture_is_not_capture_ready() {
        let body = serde_json::json!({
            "product": SERVICE_PRODUCT_ID,
            "serviceProtocolVersion": SERVICE_PROTOCOL_VERSION,
            "online": true,
            "captureReady": false
        })
        .to_string();
        let status = parse_status(&body);
        assert!(status.online);
        assert!(!status.capture_ready);
    }

    #[test]
    fn online_with_bound_capture_is_ready() {
        let status = parse_status(&valid_ready_body());
        assert!(status.online && status.capture_ready);
        assert_eq!(status.version.as_deref(), Some("0.1.0"));
        assert_eq!(status.product.as_deref(), Some(SERVICE_PRODUCT_ID));
        assert_eq!(status.protocol_version, Some(SERVICE_PROTOCOL_VERSION));
    }

    #[test]
    fn missing_capture_field_fails_closed() {
        let body = serde_json::json!({
            "product": SERVICE_PRODUCT_ID,
            "serviceProtocolVersion": SERVICE_PROTOCOL_VERSION,
            "online": true
        })
        .to_string();
        assert!(!parse_status(&body).capture_ready);
    }

    #[test]
    fn generic_json_without_product_identity_is_offline() {
        let status = parse_status(r#"{"online":true,"captureReady":true,"version":"0.1.0"}"#);
        assert!(!status.online);
        assert!(!status.capture_ready);
        assert_eq!(status.version, None);
    }

    #[test]
    fn wrong_protocol_version_is_offline() {
        let body = serde_json::json!({
            "product": SERVICE_PRODUCT_ID,
            "serviceProtocolVersion": SERVICE_PROTOCOL_VERSION + 1,
            "online": true,
            "captureReady": true
        })
        .to_string();
        let status = parse_status(&body);
        assert!(!status.online);
        assert!(!status.capture_ready);
    }

    #[test]
    fn expected_capture_endpoint_must_match() {
        let expected = DiagnosticsExpectation {
            capture_endpoint: Some(CaptureEndpoint::UnixSocket("/tmp/moraine.sock".into())),
            scope_id: None,
        };
        let ok = parse_status_with_expectation(&valid_ready_body(), Some(&expected));
        assert!(ok.online && ok.capture_ready);

        let mismatch = DiagnosticsExpectation {
            capture_endpoint: Some(CaptureEndpoint::UnixSocket("/tmp/other.sock".into())),
            scope_id: None,
        };
        let bad = parse_status_with_expectation(&valid_ready_body(), Some(&mismatch));
        assert!(!bad.online);
        assert!(!bad.capture_ready);
    }

    #[test]
    fn expected_scope_id_must_match_when_present() {
        let body = serde_json::json!({
            "product": SERVICE_PRODUCT_ID,
            "serviceProtocolVersion": SERVICE_PROTOCOL_VERSION,
            "online": true,
            "captureReady": true,
            "scopeId": "d07be4ed3160",
            "captureEndpoint": {
                "kind": "windows_named_pipe",
                "value": r"\\.\pipe\moraine.capture.v1.d07be4ed3160"
            }
        })
        .to_string();
        let expected = DiagnosticsExpectation {
            capture_endpoint: Some(CaptureEndpoint::WindowsNamedPipe(
                r"\\.\pipe\moraine.capture.v1.d07be4ed3160".into(),
            )),
            scope_id: Some("d07be4ed3160".into()),
        };
        assert!(parse_status_with_expectation(&body, Some(&expected)).online);

        let wrong_scope = DiagnosticsExpectation {
            capture_endpoint: expected.capture_endpoint.clone(),
            scope_id: Some("deadbeefcafe".into()),
        };
        assert!(!parse_status_with_expectation(&body, Some(&wrong_scope)).online);
    }

    #[test]
    fn scope_id_is_derived_from_named_pipe_layout() {
        let layout_endpoint = CaptureEndpoint::WindowsNamedPipe(
            r"\\.\pipe\moraine.capture.v1.d07be4ed3160".into(),
        );
        let expectation = DiagnosticsExpectation {
            capture_endpoint: Some(layout_endpoint),
            scope_id: scope_id_from_pipe_name(r"\\.\pipe\moraine.capture.v1.d07be4ed3160"),
        };
        assert_eq!(expectation.scope_id.as_deref(), Some("d07be4ed3160"));
    }
}
