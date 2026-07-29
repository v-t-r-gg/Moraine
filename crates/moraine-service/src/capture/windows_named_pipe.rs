use std::mem::size_of;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tracing::{error, info};
use windows::core::BOOL;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};

const MAX_PIPE_INSTANCES: usize = 16;
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

struct LocalAllocation(HLOCAL);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(self.0));
        }
    }
}

struct SecurityDescriptor {
    descriptor: PSECURITY_DESCRIPTOR,
    _allocation: LocalAllocation,
}

impl SecurityDescriptor {
    fn protected_for(sid: &str) -> Result<Self> {
        let sddl = format!("O:{sid}G:{sid}D:P(A;;FA;;;SY)(A;;FA;;;{sid})");
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &windows::core::HSTRING::from(sddl),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .context("build protected named-pipe security descriptor")?;
        }
        Ok(Self {
            descriptor,
            _allocation: LocalAllocation(HLOCAL(descriptor.0)),
        })
    }

    fn attributes(&mut self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor.0,
            bInheritHandle: BOOL(0),
        }
    }
}

fn server_options(first: bool) -> ServerOptions {
    let mut options = ServerOptions::new();
    options
        .access_inbound(true)
        .access_outbound(false)
        .pipe_mode(PipeMode::Byte)
        .reject_remote_clients(true)
        .first_pipe_instance(first)
        .max_instances(MAX_PIPE_INSTANCES as u32)
        .in_buffer_size((crate::MAX_EVENT_BYTES + 1) as u32)
        .out_buffer_size(1);
    options
}

fn create_server(pipe_name: &str, sid: &str, first: bool) -> Result<NamedPipeServer> {
    let mut security = SecurityDescriptor::protected_for(sid)?;
    let mut attributes = security.attributes();
    unsafe {
        server_options(first)
            .create_with_security_attributes_raw(
                pipe_name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
            .with_context(|| {
                if first {
                    format!(
                        "could not claim first instance of Windows capture endpoint {pipe_name}"
                    )
                } else {
                    format!("could not create the next Windows capture listener at {pipe_name}")
                }
            })
    }
}

pub struct WindowsNamedPipeCaptureListener {
    pipe_name: String,
    sid: String,
    initial: NamedPipeServer,
}

impl WindowsNamedPipeCaptureListener {
    pub(super) fn bind(pipe_name: &str) -> Result<Self> {
        let identity = moraine_platform::current_windows_user_identity()
            .context("resolve current Windows identity for capture ownership")?;
        let initial = create_server(pipe_name, &identity.sid, true)?;
        info!(
            pipe = pipe_name,
            "Windows named-pipe capture endpoint bound"
        );
        Ok(Self {
            pipe_name: pipe_name.to_owned(),
            sid: identity.sid,
            initial,
        })
    }

    pub(super) async fn run(
        self,
        spool_dir: std::path::PathBuf,
        shutdown: Arc<Notify>,
    ) -> Result<()> {
        tokio::fs::create_dir_all(spool_dir.join("processed")).await?;
        tokio::fs::create_dir_all(spool_dir.join("failed")).await?;

        let mut pending = self.initial;
        let mut clients = JoinSet::new();
        loop {
            if clients.len() >= MAX_PIPE_INSTANCES - 1 {
                tokio::select! {
                    joined = clients.join_next() => {
                        supervise_client(joined)?;
                    }
                    _ = shutdown.notified() => break,
                }
                continue;
            }

            tokio::select! {
                connected = pending.connect() => {
                    connected.with_context(|| {
                        format!("Windows capture accept failed at {}", self.pipe_name)
                    })?;
                    let next = create_server(&self.pipe_name, &self.sid, false)?;
                    let connected = std::mem::replace(&mut pending, next);
                    let spool = spool_dir.clone();
                    clients.spawn(async move { process_client(connected, spool).await });
                }
                joined = clients.join_next(), if !clients.is_empty() => {
                    supervise_client(joined)?;
                }
                _ = shutdown.notified() => break,
            }
        }

        drop(pending);
        let drain = async {
            while let Some(joined) = clients.join_next().await {
                supervise_client(Some(joined))?;
            }
            Ok::<(), anyhow::Error>(())
        };
        if tokio::time::timeout(SHUTDOWN_GRACE, drain).await.is_err() {
            clients.abort_all();
            while clients.join_next().await.is_some() {}
        }
        info!(
            pipe = self.pipe_name,
            "Windows named-pipe capture endpoint stopped"
        );
        Ok(())
    }
}

fn supervise_client(joined: Option<Result<Result<()>, tokio::task::JoinError>>) -> Result<()> {
    let Some(joined) = joined else {
        return Ok(());
    };
    joined.context("Windows capture client task failed")??;
    Ok(())
}

async fn process_client(server: NamedPipeServer, spool_dir: std::path::PathBuf) -> Result<()> {
    let mut payload = Vec::new();
    server
        .take((crate::MAX_EVENT_BYTES + 1) as u64)
        .read_to_end(&mut payload)
        .await
        .context("read Windows named-pipe capture payload")?;
    if payload.len() > crate::MAX_EVENT_BYTES {
        error!(
            bytes = payload.len(),
            "rejected oversized Windows capture payload"
        );
        return Ok(());
    }
    let path = crate::write_spooled_payload(&spool_dir, &payload).await?;
    info!(file = %path.display(), "spooled Windows capture event");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::windows::io::AsRawHandle;

    use tokio::io::AsyncWriteExt;
    use tokio::net::windows::named_pipe::ClientOptions;
    use uuid::Uuid;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SE_KERNEL_OBJECT,
    };
    use windows::Win32::Security::{
        DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    };

    use super::*;

    fn write_only_client(
        pipe_name: &str,
    ) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
        let mut options = ClientOptions::new();
        options.read(false).write(true);
        options.open(pipe_name)
    }

    fn pipe_sddl(server: &NamedPipeServer) -> windows::core::Result<String> {
        unsafe {
            let information = OWNER_SECURITY_INFORMATION
                | DACL_SECURITY_INFORMATION
                | PROTECTED_DACL_SECURITY_INFORMATION;
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            GetSecurityInfo(
                HANDLE(server.as_raw_handle()),
                SE_KERNEL_OBJECT,
                information,
                None,
                None,
                None,
                None,
                Some(&mut descriptor),
            )
            .ok()?;
            let _descriptor = LocalAllocation(HLOCAL(descriptor.0));
            let mut text = PWSTR::null();
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                information,
                &mut text,
                None,
            )?;
            let _text = LocalAllocation(HLOCAL(text.0.cast()));
            text.to_string()
        }
    }

    fn sddl_names_current_account(sddl: &str, sid: &str) -> bool {
        sddl.contains(sid)
            || (sid.ends_with("-500") && (sddl.contains("O:LA") || sddl.contains(";;;LA)")))
    }

    #[tokio::test]
    async fn production_server_enforces_acl_first_instance_size_and_framing() -> Result<()> {
        let identity = moraine_platform::current_windows_user_identity()?;
        let pipe_name = format!(r"\\.\pipe\moraine.capture.test.{}", Uuid::new_v4());
        let first = create_server(&pipe_name, &identity.sid, true)?;
        assert!(create_server(&pipe_name, &identity.sid, true).is_err());

        let sddl = pipe_sddl(&first)?;
        assert!(sddl_names_current_account(&sddl, &identity.sid));
        assert!(sddl.contains("SY"));
        assert!(sddl.contains("D:P"));
        for broad in ["WD", "AN", "BU", "AU"] {
            assert!(!sddl.contains(&format!(";;;{broad})")));
        }

        let spool = tempfile::tempdir()?;
        let expected = vec![0x5a; crate::MAX_EVENT_BYTES];
        let spool_path = spool.path().to_path_buf();
        let reader = tokio::spawn(async move {
            first.connect().await?;
            process_client(first, spool_path).await
        });
        let mut client = write_only_client(&pipe_name)?;
        client.write_all(&expected).await?;
        client.flush().await?;
        drop(client);
        reader.await??;
        let pending: Vec<_> = std::fs::read_dir(spool.path())?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            })
            .collect();
        assert_eq!(pending.len(), 1);
        assert_eq!(std::fs::read(pending[0].path())?, expected);

        let oversized = create_server(&pipe_name, &identity.sid, true)?;
        let spool_path = spool.path().to_path_buf();
        let reader = tokio::spawn(async move {
            oversized.connect().await?;
            process_client(oversized, spool_path).await
        });
        let mut client = write_only_client(&pipe_name)?;
        client
            .write_all(&vec![0x7b; crate::MAX_EVENT_BYTES + 1])
            .await?;
        drop(client);
        reader.await??;
        let pending_after = std::fs::read_dir(spool.path())?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            })
            .count();
        assert_eq!(pending_after, 1);
        Ok(())
    }

    #[tokio::test]
    async fn production_listener_keeps_next_instance_ready_and_shuts_down() -> Result<()> {
        let pipe_name = format!(r"\\.\pipe\moraine.capture.test.{}", Uuid::new_v4());
        let listener = WindowsNamedPipeCaptureListener::bind(&pipe_name)?;
        let spool = tempfile::tempdir()?;
        let shutdown = Arc::new(Notify::new());
        let task = tokio::spawn(listener.run(spool.path().to_path_buf(), shutdown.clone()));

        let mut first = write_only_client(&pipe_name)?;
        first
            .write_all(br#"{"eventId":"windows-production-one"}"#)
            .await?;
        drop(first);

        let mut second = None;
        for _ in 0..100 {
            match write_only_client(&pipe_name) {
                Ok(client) => {
                    second = Some(client);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
        let mut second = second.context("next listener was not available")?;
        second
            .write_all(br#"{"eventId":"windows-production-two"}"#)
            .await?;
        drop(second);

        for _ in 0..200 {
            if spool
                .path()
                .join("event-id-windows-production-one.json")
                .exists()
                && spool
                    .path()
                    .join("event-id-windows-production-two.json")
                    .exists()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(spool
            .path()
            .join("event-id-windows-production-one.json")
            .exists());
        assert!(spool
            .path()
            .join("event-id-windows-production-two.json")
            .exists());

        shutdown.notify_waiters();
        task.await??;
        assert!(write_only_client(&pipe_name).is_err());
        Ok(())
    }
}
