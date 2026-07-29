#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;

use anyhow::Context;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, PipeMode, ServerOptions};
use uuid::Uuid;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
    SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenUser, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const MAX_EVENT_BYTES: usize = 1024 * 1024;

struct LocalAllocation(HLOCAL);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(self.0));
        }
    }
}

fn current_account_sid() -> windows::core::Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;

        let mut required = 0;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut required);
        let mut buffer = vec![0u8; required as usize];
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            required,
            &mut required,
        )?;
        CloseHandle(token)?;

        let token_user = &*(buffer.as_ptr().cast::<TOKEN_USER>());
        let mut sid = PWSTR::null();
        ConvertSidToStringSidW(token_user.User.Sid, &mut sid)?;
        let value = sid.to_string()?;
        LocalFree(Some(HLOCAL(sid.0.cast())));
        Ok(value)
    }
}

struct SecurityDescriptor {
    descriptor: PSECURITY_DESCRIPTOR,
    _allocation: LocalAllocation,
}

impl SecurityDescriptor {
    fn protected_for(sid: &str) -> windows::core::Result<Self> {
        let sddl = format!("O:{sid}G:{sid}D:P(A;;FA;;;SY)(A;;FA;;;{sid})");
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &windows::core::HSTRING::from(sddl),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )?;
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
        .max_instances(16)
        .in_buffer_size((MAX_EVENT_BYTES + 1) as u32)
        .out_buffer_size(1);
    options
}

fn sddl_names_current_account(sddl: &str, sid: &str) -> bool {
    sddl.contains(sid)
        || (sid.ends_with("-500") && (sddl.contains("O:LA") || sddl.contains(";;;LA)")))
}

unsafe fn create_server(
    pipe_name: &str,
    first: bool,
    attributes: &mut SECURITY_ATTRIBUTES,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    server_options(first).create_with_security_attributes_raw(
        pipe_name,
        (attributes as *mut SECURITY_ATTRIBUTES).cast(),
    )
}

fn pipe_sddl(
    server: &tokio::net::windows::named_pipe::NamedPipeServer,
) -> windows::core::Result<String> {
    unsafe {
        let handle = HANDLE(server.as_raw_handle());
        let information = OWNER_SECURITY_INFORMATION
            | DACL_SECURITY_INFORMATION
            | PROTECTED_DACL_SECURITY_INFORMATION;
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let status = GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            information,
            None,
            None,
            None,
            None,
            Some(&mut descriptor),
        );
        status.ok()?;
        let _descriptor_allocation = LocalAllocation(HLOCAL(descriptor.0));

        let mut text = PWSTR::null();
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            information,
            &mut text,
            None,
        )?;
        let _text_allocation = LocalAllocation(HLOCAL(text.0.cast()));
        Ok(text.to_string()?)
    }
}

async fn read_one_event(
    server: tokio::net::windows::named_pipe::NamedPipeServer,
) -> std::io::Result<Vec<u8>> {
    server.connect().await?;
    let mut payload = Vec::new();
    server
        .take((MAX_EVENT_BYTES + 1) as u64)
        .read_to_end(&mut payload)
        .await?;
    if payload.len() > MAX_EVENT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "capture event exceeds maximum size",
        ));
    }
    Ok(payload)
}

#[tokio::test]
async fn secured_pipe_preserves_framing_size_and_first_instance() -> anyhow::Result<()> {
    let sid = current_account_sid()?;
    let pipe_name = format!(r"\\.\pipe\moraine.w2.contract.{}", Uuid::new_v4());
    let mut security = SecurityDescriptor::protected_for(&sid)?;
    let mut attributes = security.attributes();

    let server = unsafe { create_server(&pipe_name, true, &mut attributes)? };
    let second = unsafe { create_server(&pipe_name, true, &mut attributes) };
    assert!(second.is_err(), "a second first instance claimed the pipe");

    let sddl = pipe_sddl(&server)?;
    assert!(
        sddl_names_current_account(&sddl, &sid),
        "current SID {sid} is absent from pipe SDDL {sddl}"
    );
    assert!(sddl.contains("SY"));
    for broad in ["WD", "AN", "BU", "AU"] {
        assert!(
            !sddl.contains(&format!(";;;{broad})")),
            "broad principal {broad} appears in {sddl}"
        );
    }

    let expected = vec![0x5a; MAX_EVENT_BYTES];
    let reader = tokio::spawn(read_one_event(server));
    let mut client = ClientOptions::new()
        .write(true)
        .open(&pipe_name)
        .context("open maximum-size payload client")?;
    client.write_all(&expected).await?;
    client.flush().await?;
    drop(client);
    assert_eq!(reader.await??, expected);

    let mut attributes = security.attributes();
    let oversized_server = unsafe { create_server(&pipe_name, true, &mut attributes)? };
    let reader = tokio::spawn(read_one_event(oversized_server));
    let mut client = ClientOptions::new()
        .write(true)
        .open(&pipe_name)
        .context("open oversized payload client")?;
    client.write_all(&vec![0x7b; MAX_EVENT_BYTES + 1]).await?;
    client.flush().await?;
    drop(client);
    let error = reader.await?.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    Ok(())
}

#[tokio::test]
async fn next_listener_exists_before_connected_event_is_processed() -> anyhow::Result<()> {
    let sid = current_account_sid()?;
    let pipe_name = format!(r"\\.\pipe\moraine.w2.concurrent.{}", Uuid::new_v4());
    let mut security = SecurityDescriptor::protected_for(&sid)?;
    let mut attributes = security.attributes();

    let first = unsafe { create_server(&pipe_name, true, &mut attributes) }
        .context("create first concurrent server instance")?;
    let mut client_one = ClientOptions::new()
        .write(true)
        .open(&pipe_name)
        .context("open first concurrent client")?;
    first
        .connect()
        .await
        .context("connect first concurrent server")?;

    let next = unsafe { create_server(&pipe_name, false, &mut attributes) }
        .context("create next concurrent server instance")?;
    let mut client_two = ClientOptions::new()
        .write(true)
        .open(&pipe_name)
        .context("open second concurrent client")?;
    next.connect()
        .await
        .context("connect second concurrent server")?;

    client_one.write_all(b"one").await?;
    client_two.write_all(b"two").await?;
    drop(client_one);
    drop(client_two);

    let (mut first_bytes, mut next_bytes) = (Vec::new(), Vec::new());
    first.take(4).read_to_end(&mut first_bytes).await?;
    next.take(4).read_to_end(&mut next_bytes).await?;
    assert_eq!(first_bytes, b"one");
    assert_eq!(next_bytes, b"two");
    Ok(())
}
