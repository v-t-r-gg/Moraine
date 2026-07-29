//! Platform-dispatched delivery of already-serialized capture payloads.

#[cfg(target_os = "linux")]
mod linux_unix;
#[cfg(target_os = "windows")]
mod windows_named_pipe;

use moraine_platform::CaptureEndpoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDelivery {
    Delivered,
    Unavailable,
    #[cfg_attr(
        not(target_os = "windows"),
        allow(dead_code, reason = "Windows security outcome")
    )]
    AccessDenied,
    Unsupported,
}

pub fn deliver(endpoint: &CaptureEndpoint, payload: &[u8]) -> CaptureDelivery {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = payload;
    match endpoint {
        #[cfg(target_os = "linux")]
        CaptureEndpoint::UnixSocket(path) => linux_unix::deliver(path, payload),
        #[cfg(not(target_os = "linux"))]
        CaptureEndpoint::UnixSocket(_) => CaptureDelivery::Unsupported,
        #[cfg(target_os = "windows")]
        CaptureEndpoint::WindowsNamedPipe(name) => windows_named_pipe::deliver(name, payload),
        #[cfg(not(target_os = "windows"))]
        CaptureEndpoint::WindowsNamedPipe(_) => CaptureDelivery::Unsupported,
        CaptureEndpoint::Unsupported => CaptureDelivery::Unsupported,
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
