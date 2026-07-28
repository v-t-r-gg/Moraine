use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use super::CaptureDelivery;

pub(super) fn deliver(socket_path: &Path, payload: &[u8]) -> CaptureDelivery {
    let Ok(mut stream) = UnixStream::connect(socket_path) else {
        return CaptureDelivery::Unavailable;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    if stream.write_all(payload).is_err() || stream.flush().is_err() {
        return CaptureDelivery::Unavailable;
    }
    CaptureDelivery::Delivered
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn delivers_to_unix_listener() {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("capture.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let receiver = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut body = Vec::new();
            stream.read_to_end(&mut body).unwrap();
            body
        });

        assert_eq!(
            deliver(&socket, b"{\"ok\":true}"),
            CaptureDelivery::Delivered
        );
        assert_eq!(receiver.join().unwrap(), b"{\"ok\":true}");
    }

    #[test]
    fn connection_refusal_is_unavailable() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            deliver(&temp.path().join("missing.sock"), b"{}"),
            CaptureDelivery::Unavailable
        );
    }
}
