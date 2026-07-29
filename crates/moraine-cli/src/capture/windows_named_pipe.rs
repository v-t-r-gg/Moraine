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
    use std::mem::size_of;

    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::{ClientOptions, PipeMode, ServerOptions};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};

    use super::*;

    struct LocalAllocation(HLOCAL);

    impl Drop for LocalAllocation {
        fn drop(&mut self) {
            unsafe {
                let _ = LocalFree(Some(self.0));
            }
        }
    }

    fn server(
        pipe_name: &str,
        max_instances: usize,
        sddl: Option<&str>,
    ) -> (
        tokio::net::windows::named_pipe::NamedPipeServer,
        Option<LocalAllocation>,
    ) {
        let mut options = ServerOptions::new();
        options
            .access_inbound(true)
            .access_outbound(false)
            .pipe_mode(PipeMode::Byte)
            .reject_remote_clients(true)
            .first_pipe_instance(true)
            .max_instances(max_instances);
        let Some(sddl) = sddl else {
            return (options.create(pipe_name).unwrap(), None);
        };

        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &windows::core::HSTRING::from(sddl),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .unwrap();
        }
        let allocation = LocalAllocation(HLOCAL(descriptor.0));
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: BOOL(0),
        };
        let server = unsafe {
            options
                .create_with_security_attributes_raw(
                    pipe_name,
                    (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
                )
                .unwrap()
        };
        (server, Some(allocation))
    }

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn write_only_delivery_preserves_exact_bytes() {
        let pipe_name = format!(r"\\.\pipe\moraine.capture.client.{}", uuid::Uuid::new_v4());
        let (server, _security) = server(&pipe_name, 1, None);
        let reader = tokio::spawn(async move {
            server.connect().await.unwrap();
            let mut bytes = Vec::new();
            server.take(64).read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        let expected = b"{\"eventId\":\"client-exact\"}";
        assert_eq!(deliver(&pipe_name, expected), CaptureDelivery::Delivered);
        assert_eq!(reader.await.unwrap(), expected);
    }

    #[test]
    fn protected_pipe_access_denial_is_distinct() {
        let pipe_name = format!(r"\\.\pipe\moraine.capture.denied.{}", uuid::Uuid::new_v4());
        let (_server, _security) = server(&pipe_name, 1, Some("D:P(A;;FA;;;SY)"));
        assert_eq!(
            deliver(&pipe_name, b"denied"),
            CaptureDelivery::AccessDenied
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn busy_pipe_retries_only_within_the_delivery_budget() {
        let pipe_name = format!(r"\\.\pipe\moraine.capture.busy.{}", uuid::Uuid::new_v4());
        let (server, _security) = server(&pipe_name, 1, None);
        let mut client_options = ClientOptions::new();
        client_options.read(false).write(true);
        let _occupying_client = client_options.open(&pipe_name).unwrap();
        server.connect().await.unwrap();

        let started = Instant::now();
        assert_eq!(deliver(&pipe_name, b"busy"), CaptureDelivery::Unavailable);
        assert!(started.elapsed() >= DELIVERY_BUDGET - Duration::from_millis(100));
        assert!(started.elapsed() < DELIVERY_BUDGET + Duration::from_secs(1));
    }
}
