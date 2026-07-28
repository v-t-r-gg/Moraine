//! Platform-dispatched delivery of already-serialized capture payloads.

#[cfg(target_os = "linux")]
mod linux_unix;

use moraine_platform::CaptureEndpoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "delivery success and temporary unavailability are Linux backend outcomes until W2"
    )
)]
pub enum CaptureDelivery {
    Delivered,
    Unavailable,
    Unsupported,
}

pub fn deliver(endpoint: &CaptureEndpoint, payload: &[u8]) -> CaptureDelivery {
    #[cfg(not(target_os = "linux"))]
    let _ = payload;
    match endpoint {
        #[cfg(target_os = "linux")]
        CaptureEndpoint::UnixSocket(path) => linux_unix::deliver(path, payload),
        #[cfg(not(target_os = "linux"))]
        CaptureEndpoint::UnixSocket(_) => CaptureDelivery::Unsupported,
        CaptureEndpoint::WindowsNamedPipe(_) | CaptureEndpoint::Unsupported => {
            CaptureDelivery::Unsupported
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_endpoint_is_never_delivered() {
        assert_eq!(
            deliver(&CaptureEndpoint::Unsupported, b"{}"),
            CaptureDelivery::Unsupported
        );
    }
}
