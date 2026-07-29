use std::thread;
use std::time::{Duration, Instant};

use super::CaptureDelivery;
use windows::core::{HRESULT, HSTRING};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING,
    ERROR_OPERATION_ABORTED, ERROR_PIPE_BUSY, ERROR_SEM_TIMEOUT, GENERIC_WRITE, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_MODE, OPEN_EXISTING,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

const DELIVERY_BUDGET: Duration = Duration::from_secs(2);
const BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn deliver(pipe_name: &str, payload: &[u8]) -> CaptureDelivery {
    let deadline = Instant::now() + DELIVERY_BUDGET;
    loop {
        match open_overlapped(pipe_name) {
            Ok(pipe) => {
                let result = write_before_deadline(pipe.0, payload, deadline);
                drop(pipe);
                return result;
            }
            Err(error) if is_win32_error(&error, ERROR_ACCESS_DENIED.0) => {
                return CaptureDelivery::AccessDenied;
            }
            Err(error) if is_win32_error(&error, ERROR_PIPE_BUSY.0) => {
                let now = Instant::now();
                if now >= deadline {
                    return CaptureDelivery::Unavailable;
                }
                thread::sleep(BUSY_RETRY_INTERVAL.min(deadline.saturating_duration_since(now)));
            }
            Err(error)
                if matches!(
                    win32_error_code(&error),
                    Some(code)
                        if code == ERROR_FILE_NOT_FOUND.0
                            || code == ERROR_SEM_TIMEOUT.0
                            || code == ERROR_BROKEN_PIPE.0
                ) =>
            {
                return CaptureDelivery::Unavailable;
            }
            Err(_) => return CaptureDelivery::Unavailable,
        }
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn open_overlapped(pipe_name: &str) -> windows::core::Result<OwnedHandle> {
    let name = HSTRING::from(pipe_name);
    let handle = unsafe {
        CreateFileW(
            &name,
            GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )?
    };
    Ok(OwnedHandle(handle))
}

fn write_before_deadline(handle: HANDLE, payload: &[u8], deadline: Instant) -> CaptureDelivery {
    let event = match unsafe { CreateEventW(None, true, false, None) } {
        Ok(event) => OwnedHandle(event),
        Err(_) => return CaptureDelivery::Unavailable,
    };
    let mut overlapped = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
    };

    let write = unsafe { WriteFile(handle, Some(payload), None, Some(&mut overlapped)) };
    if let Err(error) = write {
        if is_win32_error(&error, ERROR_ACCESS_DENIED.0) {
            return CaptureDelivery::AccessDenied;
        }
        if !is_win32_error(&error, ERROR_IO_PENDING.0) {
            return CaptureDelivery::Unavailable;
        }
    }

    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        cancel_and_wait(handle, &overlapped);
        return CaptureDelivery::Unavailable;
    }
    let wait_millis = remaining.as_millis().clamp(1, u32::MAX as u128) as u32;
    let wait = unsafe { WaitForSingleObject(event.0, wait_millis) };
    if wait != windows::Win32::Foundation::WAIT_OBJECT_0 {
        cancel_and_wait(handle, &overlapped);
        return CaptureDelivery::Unavailable;
    }

    let mut written = 0;
    match unsafe { GetOverlappedResult(handle, &overlapped, &mut written, false) } {
        Ok(()) if written as usize == payload.len() => CaptureDelivery::Delivered,
        Err(error) if is_win32_error(&error, ERROR_ACCESS_DENIED.0) => {
            CaptureDelivery::AccessDenied
        }
        _ => CaptureDelivery::Unavailable,
    }
}

fn cancel_and_wait(handle: HANDLE, overlapped: &OVERLAPPED) {
    let cancel = unsafe { CancelIoEx(handle, Some(overlapped)) };
    if let Err(error) = cancel {
        debug_assert!(
            is_win32_error(&error, windows::Win32::Foundation::ERROR_NOT_FOUND.0),
            "unexpected overlapped cancellation error: {error}"
        );
    }

    // The payload buffer and OVERLAPPED storage must remain alive until Windows
    // confirms that the write completed or cancellation took effect.
    let mut transferred = 0;
    let completion = unsafe { GetOverlappedResult(handle, overlapped, &mut transferred, true) };
    if let Err(error) = completion {
        debug_assert!(
            is_win32_error(&error, ERROR_OPERATION_ABORTED.0),
            "unexpected overlapped cancellation error: {error}"
        );
    }
}

fn is_win32_error(error: &windows::core::Error, code: u32) -> bool {
    error.code() == HRESULT::from_win32(code)
}

fn win32_error_code(error: &windows::core::Error) -> Option<u32> {
    [
        ERROR_FILE_NOT_FOUND.0,
        ERROR_SEM_TIMEOUT.0,
        ERROR_BROKEN_PIPE.0,
    ]
    .into_iter()
    .find(|code| is_win32_error(error, *code))
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

    #[tokio::test]
    async fn protected_pipe_access_denial_is_distinct() {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stalled_server_cannot_block_delivery_past_the_budget() {
        let pipe_name = format!(r"\\.\pipe\moraine.capture.stalled.{}", uuid::Uuid::new_v4());
        let mut options = ServerOptions::new();
        options
            .access_inbound(true)
            .access_outbound(false)
            .pipe_mode(PipeMode::Byte)
            .reject_remote_clients(true)
            .first_pipe_instance(true)
            .max_instances(1)
            .in_buffer_size(64);
        let server = options.create(&pipe_name).unwrap();
        let payload = vec![b'x'; 1024 * 1024];
        let delivery_pipe = pipe_name.clone();
        let started = Instant::now();
        let delivery = tokio::task::spawn_blocking(move || deliver(&delivery_pipe, &payload));
        server.connect().await.unwrap();

        assert_eq!(
            tokio::time::timeout(DELIVERY_BUDGET + Duration::from_secs(1), delivery)
                .await
                .expect("delivery exceeded its bounded cancellation budget")
                .unwrap(),
            CaptureDelivery::Unavailable
        );
        assert!(started.elapsed() >= DELIVERY_BUDGET - Duration::from_millis(100));
        assert!(started.elapsed() < DELIVERY_BUDGET + Duration::from_secs(1));
    }
}
