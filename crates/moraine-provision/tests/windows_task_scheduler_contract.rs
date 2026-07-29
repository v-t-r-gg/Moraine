#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::thread;
use std::time::Duration;

use uuid::Uuid;
use windows::core::{BSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, TokenUser, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::TaskScheduler::{
    ITaskFolder, ITaskService, TaskScheduler, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN,
    TASK_RUNLEVEL_LUA, TASK_STATE_RUNNING,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::System::Variant::VARIANT;

struct ComApartment;

impl ComApartment {
    fn initialize() -> windows::core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct TaskCleanup {
    folder: ITaskFolder,
    name: BSTR,
}

impl Drop for TaskCleanup {
    fn drop(&mut self) {
        unsafe {
            let _ = self.folder.DeleteTask(&self.name, 0);
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

fn connect_scheduler() -> windows::core::Result<(ITaskService, ITaskFolder)> {
    unsafe {
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        let empty = VARIANT::default();
        service.Connect(&empty, &empty, &empty, &empty)?;
        let root = service.GetFolder(&BSTR::from(r"\"))?;
        Ok((service, root))
    }
}

fn task_xml(sid: &str, enabled: bool, description: &str) -> String {
    let enabled = if enabled { "true" } else { "false" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Moraine</Author>
    <Source>Moraine W2 contract probe</Source>
    <Description>{description}</Description>
    <URI>\Moraine\Tests\RuntimeContract</URI>
    <Version>1</Version>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>{enabled}</Enabled>
      <UserId>{sid}</UserId>
      <Delay>PT5S</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure><Interval>PT1M</Interval><Count>3</Count></RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>%SystemRoot%\System32\timeout.exe</Command>
      <Arguments>/T 30 /NOBREAK</Arguments>
    </Exec>
  </Actions>
</Task>"#
    )
}

fn register(
    folder: &ITaskFolder,
    name: &BSTR,
    sid: &str,
    xml: &str,
    sddl: &str,
) -> windows::core::Result<windows::Win32::System::TaskScheduler::IRegisteredTask> {
    unsafe {
        folder.RegisterTask(
            name,
            &BSTR::from(xml),
            TASK_CREATE_OR_UPDATE.0,
            &VARIANT::from(BSTR::from(sid)),
            &VARIANT::default(),
            TASK_LOGON_INTERACTIVE_TOKEN,
            &VARIANT::from(BSTR::from(sddl)),
        )
    }
}

fn read_registration(
    task: &windows::Win32::System::TaskScheduler::IRegisteredTask,
) -> windows::core::Result<(String, String)> {
    let security_information = (OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION).0 as i32;
    unsafe {
        Ok((
            task.Xml()?.to_string(),
            task.GetSecurityDescriptor(security_information)?
                .to_string(),
        ))
    }
}

fn sddl_names_current_account(sddl: &str, sid: &str) -> bool {
    sddl.contains(sid)
        // Task Scheduler canonicalizes the built-in local Administrator
        // account SID (RID 500) to the well-known SDDL alias `LA`.
        || (sid.ends_with("-500")
            && (sddl.contains("O:LA") || sddl.contains(";;;LA)")))
}

#[test]
fn current_user_task_can_run_restore_and_preserve_demand_start() -> windows::core::Result<()> {
    let _com = ComApartment::initialize()?;
    let sid = current_account_sid()?;
    let (_service, folder) = connect_scheduler()?;
    let name = BSTR::from(format!("Moraine W2 Contract {}", Uuid::new_v4()));
    let _cleanup = TaskCleanup {
        folder: folder.clone(),
        name: name.clone(),
    };
    let sddl = format!("O:{sid}G:{sid}D:P(A;;FA;;;SY)(A;;FA;;;{sid})");

    let original = register(
        &folder,
        &name,
        &sid,
        &task_xml(&sid, false, "original"),
        &sddl,
    )?;
    let (original_xml, original_sddl) = read_registration(&original)?;
    assert!(original_xml.contains("<LogonType>InteractiveToken</LogonType>"));
    assert!(original_xml.contains("<Enabled>false</Enabled>"));
    assert!(
        sddl_names_current_account(&original_sddl, &sid),
        "current SID {sid} is absent from returned task SDDL {original_sddl}"
    );
    assert!(
        original_sddl.contains("SY"),
        "LocalSystem is absent from returned task SDDL {original_sddl}"
    );
    unsafe {
        let principal = original.Definition()?.Principal()?;
        let mut run_level = TASK_RUNLEVEL_LUA;
        principal.RunLevel(&mut run_level)?;
        assert_eq!(run_level, TASK_RUNLEVEL_LUA);
    }

    unsafe {
        let running = original.Run(&VARIANT::default())?;
        for _ in 0..20 {
            if original.State()? == TASK_STATE_RUNNING {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(original.State()?, TASK_STATE_RUNNING);
        running.Stop()?;
        original.Stop(0)?;
    }

    let mutated = register(
        &folder,
        &name,
        &sid,
        &task_xml(&sid, true, "mutated"),
        &sddl,
    )?;
    assert_ne!(read_registration(&mutated)?.0, original_xml);

    let restored = register(&folder, &name, &sid, &original_xml, &original_sddl)?;
    let (restored_xml, restored_sddl) = read_registration(&restored)?;
    assert_eq!(restored_xml, original_xml);
    assert_eq!(restored_sddl, original_sddl);

    unsafe {
        folder.DeleteTask(&name, 0)?;
        assert!(folder.GetTask(&name).is_err());
    }
    Ok(())
}
