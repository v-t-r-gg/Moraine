use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use super::CaptureDelivery;

const DELIVERY_BUDGET: Duration = Duration::from_secs(2);
const BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(25);
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_BROKEN_PIPE: i32 = 109;
const ERROR_SEM_TIMEOUT: i32 = 121;
const ERROR_PIPE_BUSY: i32 = 231;

pub(super) fn deliver(pipe_name: &str, payload: &[u8]) -> CaptureDelivery {
    let deadline = Instant::now() + DELIVERY_BUDGET;
    loop {
        match OpenOptions::new().write(true).open(pipe_name) {
            Ok(mut pipe) => {
                return match pipe.write_all(payload).and_then(|_| pipe.flush()) {
                    Ok(()) => CaptureDelivery::Delivered,
                    Err(error) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
                        CaptureDelivery::AccessDenied
                    }
                    Err(_) => CaptureDelivery::Unavailable,
                };
            }
            Err(error) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
                return CaptureDelivery::AccessDenied;
            }
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                let now = Instant::now();
                if now >= deadline {
                    return CaptureDelivery::Unavailable;
                }
                thread::sleep(BUSY_RETRY_INTERVAL.min(deadline.saturating_duration_since(now)));
            }
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(ERROR_FILE_NOT_FOUND) | Some(ERROR_SEM_TIMEOUT) | Some(ERROR_BROKEN_PIPE)
                ) =>
            {
                return CaptureDelivery::Unavailable;
            }
            Err(_) => return CaptureDelivery::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_pipe_is_unavailable_without_an_unbounded_wait() {
        let started = Instant::now();
        assert_eq!(
            deliver(
                &format!(
                    r"\\.\pipe\moraine.capture.test.missing.{}",
                    uuid::Uuid::new_v4()
                ),
                b"{}"
            ),
            CaptureDelivery::Unavailable
        );
        assert!(started.elapsed() < DELIVERY_BUDGET);
    }
}
