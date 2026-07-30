//! Current-user Task Scheduler 2.0 background runtime for Windows.
//!
//! COM objects are created, used & released on a dedicated MTA worker for every
//! operation. The manager stores only owned Rust data & serializes mutations.

use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use sha2::{Digest, Sha256};
use windows::core::{BSTR, HRESULT};
use windows::Win32::Foundation::{
    LocalFree, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, GENERIC_ALL, HLOCAL,
    SCHED_E_TASK_NOT_RUNNING,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, ConvertStringSidToSidW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    AclSizeInformation, EqualSid, GetAce, GetAclInformation, GetSecurityDescriptorControl,
    GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, ACL_SIZE_INFORMATION,
    DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SE_DACL_PROTECTED,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::TaskScheduler::{
    IRegisteredTask, ITaskFolder, ITaskService, TaskScheduler, TASK_CREATE_OR_UPDATE,
    TASK_LOGON_INTERACTIVE_TOKEN, TASK_STATE_RUNNING,
};
use windows::Win32::System::Variant::VARIANT;

use crate::error::{ProvisionError, Result};
use crate::runtime::{BackgroundRuntimeManager, RuntimeInstallSpec};
use crate::suite::SuitePaths;
use crate::types::{
    BackgroundRuntimeBackend, BackgroundRuntimeState, RuntimeRegistrationKind,
    RuntimeRegistrationSnapshot, RuntimeRegistrationState, ServiceLog, WindowsTaskSnapshot,
    WindowsTaskSnapshotState,
};
use moraine_platform::{CaptureEndpoint, RuntimeLayout};

const STOP_BUDGET: Duration = Duration::from_secs(5);
const STOP_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTaskIdentity {
    pub account_sid: String,
    pub scope_id: String,
    pub task_name: String,
    pub task_path: String,
    pub task_uri: String,
}

impl WindowsTaskIdentity {
    pub fn current() -> Result<Self> {
        let identity = moraine_platform::current_windows_user_identity()
            .map_err(|error| ProvisionError::Service(error.to_string()))?;
        Ok(Self::for_scope(identity.sid, identity.scope_id))
    }

    pub fn for_scope(account_sid: String, scope_id: String) -> Self {
        let task_name = format!("Moraine Background Capture ({scope_id})");
        Self {
            task_path: format!(r"\{task_name}"),
            task_uri: format!(r"\Moraine\{scope_id}\BackgroundCapture"),
            account_sid,
            scope_id,
            task_name,
        }
    }
}

pub struct WindowsTaskSchedulerRuntime {
    suite: SuitePaths,
    runtime_layout: RuntimeLayout,
    task_identity: WindowsTaskIdentity,
    operation_lock: Mutex<()>,
}

impl WindowsTaskSchedulerRuntime {
    pub fn new() -> Result<Self> {
        Ok(Self {
            suite: SuitePaths::discover(),
            runtime_layout: RuntimeLayout::try_discover()
                .map_err(|error| ProvisionError::Service(error.to_string()))?,
            task_identity: WindowsTaskIdentity::current()?,
            operation_lock: Mutex::new(()),
        })
    }

    /// Explicit layouts & identity are used by disposable production-backed tests.
    pub fn with_layouts(
        suite: SuitePaths,
        runtime_layout: RuntimeLayout,
        task_identity: WindowsTaskIdentity,
    ) -> Self {
        Self {
            suite,
            runtime_layout,
            task_identity,
            operation_lock: Mutex::new(()),
        }
    }

    pub fn identity(&self) -> &WindowsTaskIdentity {
        &self.task_identity
    }

    fn run_com_operation<T, F>(&self, operation: &'static str, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(TaskSchedulerSession) -> Result<T> + Send + 'static,
    {
        let _lock = self.operation_lock.lock().map_err(|_| {
            ProvisionError::Service(format!("{operation}: runtime operation lock is poisoned"))
        })?;
        let worker = thread::Builder::new()
            .name(format!("moraine-task-scheduler-{operation}"))
            .spawn(move || {
                let _com = ComApartment::initialize().map_err(|error| {
                    ProvisionError::Service(format!("{operation}: initialize MTA COM: {error}"))
                })?;
                let session = TaskSchedulerSession::connect().map_err(|error| {
                    ProvisionError::Service(format!("{operation}: connect Task Scheduler: {error}"))
                })?;
                work(session)
            })
            .map_err(|error| {
                ProvisionError::Service(format!("{operation}: start COM worker: {error}"))
            })?;
        worker.join().map_err(|_| {
            ProvisionError::Service(format!("{operation}: Task Scheduler COM worker panicked"))
        })?
    }

    fn read_task(&self, operation: &'static str) -> Result<Option<TaskRegistration>> {
        let identity = self.task_identity.clone();
        self.run_com_operation(operation, move |session| session.read(&identity))
    }

    fn expected_install_spec(&self) -> RuntimeInstallSpec {
        RuntimeInstallSpec {
            executable: self.suite.service.clone(),
            working_directory: self.suite.prefix.clone(),
            capture_endpoint: self.runtime_layout.capture_endpoint.clone(),
            diagnostics_endpoint: self.runtime_layout.diagnostics_endpoint,
            spool_dir: self.runtime_layout.spool_dir.clone(),
            log_dir: Some(self.runtime_layout.log_dir.clone()),
        }
    }

    fn mutate_trigger(&self, enabled: bool) -> Result<()> {
        let identity = self.task_identity.clone();
        let expected_spec = self.expected_install_spec();
        self.run_com_operation("set_autostart", move |session| {
            let Some(current) = session.read(&identity)? else {
                return Err(ProvisionError::Service(format!(
                    "Task Scheduler registration {} is absent",
                    identity.task_path
                )));
            };
            let xml = set_logon_trigger_enabled(&current.xml, enabled)?;
            session.register(&identity, &xml, None)?;
            let restored = session
                .read(&identity)?
                .ok_or_else(|| ProvisionError::Service("task disappeared after update".into()))?;
            if restored.sddl != current.sddl {
                return Err(ProvisionError::Service(
                    "Task Scheduler trigger update changed the registration ACL".into(),
                ));
            }
            validate_registration(&identity, &restored, Some(&expected_spec))?;
            if logon_trigger_enabled(&restored.xml)? != enabled {
                return Err(ProvisionError::Service(
                    "Task Scheduler did not preserve requested autostart state".into(),
                ));
            }
            Ok(())
        })
    }
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> windows::core::Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct TaskSchedulerSession {
    _service: ITaskService,
    root: ITaskFolder,
}

#[derive(Debug, Clone)]
struct TaskRegistration {
    xml: String,
    sddl: String,
    running: bool,
    last_result: Option<i32>,
}

impl TaskSchedulerSession {
    fn connect() -> windows::core::Result<Self> {
        unsafe {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let empty = VARIANT::default();
            service.Connect(&empty, &empty, &empty, &empty)?;
            let root = service.GetFolder(&BSTR::from(r"\"))?;
            Ok(Self {
                _service: service,
                root,
            })
        }
    }

    fn get(&self, identity: &WindowsTaskIdentity) -> Result<Option<IRegisteredTask>> {
        let task = unsafe { self.root.GetTask(&BSTR::from(&identity.task_path)) };
        match task {
            Ok(task) => Ok(Some(task)),
            Err(error)
                if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
                    || error.code() == HRESULT::from_win32(ERROR_PATH_NOT_FOUND.0) =>
            {
                Ok(None)
            }
            Err(error) => Err(ProvisionError::Service(format!(
                "read Task Scheduler registration {}: {error}",
                identity.task_path
            ))),
        }
    }

    fn read(&self, identity: &WindowsTaskIdentity) -> Result<Option<TaskRegistration>> {
        let Some(task) = self.get(identity)? else {
            return Ok(None);
        };
        let security_information =
            (OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION).0 as i32;
        unsafe {
            Ok(Some(TaskRegistration {
                xml: task
                    .Xml()
                    .map_err(task_error("read registered task XML"))?
                    .to_string(),
                sddl: task
                    .GetSecurityDescriptor(security_information)
                    .map_err(task_error("read registered task security descriptor"))?
                    .to_string(),
                running: task.State().map_err(task_error("read task state"))? == TASK_STATE_RUNNING,
                last_result: task.LastTaskResult().ok(),
            }))
        }
    }

    fn register(
        &self,
        identity: &WindowsTaskIdentity,
        xml: &str,
        sddl: Option<&str>,
    ) -> Result<IRegisteredTask> {
        let security = sddl
            .map(|value| VARIANT::from(BSTR::from(value)))
            .unwrap_or_default();
        unsafe {
            self.root
                .RegisterTask(
                    &BSTR::from(&identity.task_name),
                    &BSTR::from(xml),
                    TASK_CREATE_OR_UPDATE.0,
                    &VARIANT::from(BSTR::from(&identity.account_sid)),
                    &VARIANT::default(),
                    TASK_LOGON_INTERACTIVE_TOKEN,
                    &security,
                )
                .map_err(task_error("register current-user Task Scheduler task"))
        }
    }

    fn delete(&self, identity: &WindowsTaskIdentity) -> Result<()> {
        if self.get(identity)?.is_some() {
            self.stop_and_wait(identity, STOP_BUDGET)?;
            unsafe {
                self.root
                    .DeleteTask(&BSTR::from(&identity.task_name), 0)
                    .map_err(task_error("delete Task Scheduler registration"))?;
            }
        }
        if self.get(identity)?.is_some() {
            return Err(ProvisionError::Service(
                "Task Scheduler registration still exists after deletion".into(),
            ));
        }
        Ok(())
    }

    fn stop_and_wait(&self, identity: &WindowsTaskIdentity, budget: Duration) -> Result<()> {
        let Some(task) = self.get(identity)? else {
            return Ok(());
        };
        let stop = unsafe { task.Stop(0) };
        if let Err(error) = stop {
            if error.code() != SCHED_E_TASK_NOT_RUNNING {
                return Err(ProvisionError::Service(format!(
                    "stop Task Scheduler runtime {}: {error}",
                    identity.task_path
                )));
            }
        }
        let deadline = Instant::now() + budget;
        loop {
            let running = unsafe {
                task.GetInstances(0)
                    .and_then(|instances| instances.Count())
                    .map_err(task_error("inspect running Task Scheduler instances"))?
            };
            if running == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ProvisionError::Service(format!(
                    "Task Scheduler runtime {} did not stop within five seconds",
                    identity.task_path
                )));
            }
            thread::sleep(STOP_POLL);
        }
    }
}

fn task_error(context: &'static str) -> impl FnOnce(windows::core::Error) -> ProvisionError {
    move |error| ProvisionError::Service(format!("{context}: {error}"))
}

pub fn registration_fingerprint(xml: &str, sddl: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(xml.as_bytes());
    digest.update([0]);
    digest.update(sddl.as_bytes());
    hex::encode(digest.finalize())
}

fn task_sddl(identity: &WindowsTaskIdentity) -> String {
    format!(
        "O:{sid}G:{sid}D:P(A;;FA;;;SY)(A;;FA;;;{sid})",
        sid = identity.account_sid
    )
}

pub fn render_task_xml(
    identity: &WindowsTaskIdentity,
    spec: &RuntimeInstallSpec,
) -> Result<String> {
    let CaptureEndpoint::WindowsNamedPipe(pipe) = &spec.capture_endpoint else {
        return Err(ProvisionError::Service(
            "Windows runtime requires a named-pipe capture endpoint".into(),
        ));
    };
    let log_dir = spec.log_dir.as_ref().ok_or_else(|| {
        ProvisionError::Service("Windows runtime requires an application log directory".into())
    })?;
    let arguments = [
        "--http".to_owned(),
        spec.diagnostics_endpoint.to_string(),
        "--named-pipe".to_owned(),
        pipe.clone(),
        "--spool-dir".to_owned(),
        spec.spool_dir.display().to_string(),
        "--log-dir".to_owned(),
        log_dir.display().to_string(),
    ]
    .into_iter()
    .map(|value| quote_windows_argument(&value))
    .collect::<Vec<_>>()
    .join(" ");
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Moraine</Author>
    <Source>Moraine</Source>
    <Description>Runs local Moraine capture for the current Windows user.</Description>
    <URI>{uri}</URI>
    <Version>1</Version>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>false</Enabled>
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
      <Command>{command}</Command>
      <Arguments>{arguments}</Arguments>
      <WorkingDirectory>{working}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>"#,
        uri = xml_escape(&identity.task_uri),
        sid = xml_escape(&identity.account_sid),
        command = xml_escape(&spec.executable.display().to_string()),
        arguments = xml_escape(&arguments),
        working = xml_escape(&spec.working_directory.display().to_string()),
    ))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn quote_windows_argument(value: &str) -> String {
    if !value.is_empty()
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character == '"')
    {
        return value.to_owned();
    }
    let mut result = String::from("\"");
    let mut backslashes = 0;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                result.push_str(&"\\".repeat(backslashes * 2 + 1));
                result.push('"');
                backslashes = 0;
            }
            _ => {
                result.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                result.push(character);
            }
        }
    }
    result.push_str(&"\\".repeat(backslashes * 2));
    result.push('"');
    result
}

fn validate_registration(
    identity: &WindowsTaskIdentity,
    registration: &TaskRegistration,
    spec: Option<&RuntimeInstallSpec>,
) -> Result<()> {
    let document = roxmltree::Document::parse(&registration.xml).map_err(|error| {
        ProvisionError::Service(format!("parse Task Scheduler registration XML: {error}"))
    })?;
    let task = document.root_element();
    if task.tag_name().name() != "Task" {
        return Err(ProvisionError::Service(
            "Task Scheduler registration root must be Task".into(),
        ));
    }

    let registration_info = single_child(task, "RegistrationInfo")?;
    expect_text(registration_info, "Author", "Moraine")?;
    expect_text(registration_info, "Source", "Moraine")?;

    let principals = single_child(task, "Principals")?;
    let principal = only_element_child(principals, "Principal")?;
    expect_text(principal, "UserId", &identity.account_sid)?;
    expect_text(principal, "LogonType", "InteractiveToken")?;
    expect_optional_text(principal, "RunLevel", "LeastPrivilege")?;

    let triggers = single_child(task, "Triggers")?;
    let trigger = only_element_child(triggers, "LogonTrigger")?;
    expect_text(trigger, "UserId", &identity.account_sid)?;
    expect_text(trigger, "Delay", "PT5S")?;
    expect_optional_boolean(trigger, "Enabled", true)?;

    let actions = single_child(task, "Actions")?;
    let action = only_element_child(actions, "Exec")?;
    if let Some(spec) = spec {
        let expected = render_task_xml(identity, spec)?;
        let expected_document = roxmltree::Document::parse(&expected).map_err(|error| {
            ProvisionError::Service(format!("parse expected Task Scheduler XML: {error}"))
        })?;
        let expected_action = only_element_child(
            single_child(expected_document.root_element(), "Actions")?,
            "Exec",
        )?;
        for field in ["Command", "Arguments", "WorkingDirectory"] {
            expect_text(action, field, child_text(expected_action, field)?)?;
        }
    }

    let settings = single_child(task, "Settings")?;
    for (name, value) in [
        ("MultipleInstancesPolicy", "IgnoreNew"),
        ("DisallowStartIfOnBatteries", "false"),
        ("StopIfGoingOnBatteries", "false"),
        ("StartWhenAvailable", "true"),
        ("ExecutionTimeLimit", "PT0S"),
    ] {
        expect_text(settings, name, value)?;
    }
    for (name, default) in [
        ("RunOnlyIfNetworkAvailable", false),
        ("AllowStartOnDemand", true),
        ("Enabled", true),
        ("Hidden", false),
        ("RunOnlyIfIdle", false),
        ("WakeToRun", false),
    ] {
        expect_optional_boolean(settings, name, default)?;
    }
    let restart = single_child(settings, "RestartOnFailure")?;
    expect_text(restart, "Interval", "PT1M")?;
    expect_text(restart, "Count", "3")?;

    validate_security_descriptor(identity, &registration.sddl)?;
    Ok(())
}

fn single_child<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Result<roxmltree::Node<'a, 'input>> {
    let matches: Vec<_> = parent
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == name)
        .collect();
    if matches.len() != 1 {
        return Err(ProvisionError::Service(format!(
            "Task Scheduler registration requires exactly one {name}"
        )));
    }
    Ok(matches[0])
}

fn only_element_child<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    expected_name: &str,
) -> Result<roxmltree::Node<'a, 'input>> {
    let children: Vec<_> = parent.children().filter(|node| node.is_element()).collect();
    if children.len() != 1 || children[0].tag_name().name() != expected_name {
        return Err(ProvisionError::Service(format!(
            "Task Scheduler {} must contain exactly one {expected_name}",
            parent.tag_name().name()
        )));
    }
    Ok(children[0])
}

fn child_text<'a, 'input>(parent: roxmltree::Node<'a, 'input>, name: &str) -> Result<&'a str> {
    single_child(parent, name)?
        .text()
        .ok_or_else(|| ProvisionError::Service(format!("Task Scheduler {name} value is missing")))
}

fn expect_text(parent: roxmltree::Node<'_, '_>, name: &str, expected: &str) -> Result<()> {
    let actual = child_text(parent, name)?;
    if actual != expected {
        return Err(ProvisionError::Service(format!(
            "Task Scheduler {name} differs: expected {expected:?}, got {actual:?}"
        )));
    }
    Ok(())
}

fn expect_optional_text(parent: roxmltree::Node<'_, '_>, name: &str, expected: &str) -> Result<()> {
    let matches: Vec<_> = parent
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == name)
        .collect();
    match matches.as_slice() {
        [] => Ok(()),
        [node] if node.text() == Some(expected) => Ok(()),
        _ => Err(ProvisionError::Service(format!(
            "Task Scheduler {name} must be absent or {expected}"
        ))),
    }
}

fn expect_optional_boolean(
    parent: roxmltree::Node<'_, '_>,
    name: &str,
    default: bool,
) -> Result<()> {
    expect_optional_text(parent, name, if default { "true" } else { "false" })
}

struct LocalAllocation(HLOCAL);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(self.0));
        }
    }
}

fn sid_from_string(value: &str) -> Result<(PSID, LocalAllocation)> {
    let mut sid = PSID::default();
    unsafe {
        ConvertStringSidToSidW(&windows::core::HSTRING::from(value), &mut sid)
            .map_err(task_error("parse Task Scheduler SID"))?;
    }
    Ok((sid, LocalAllocation(HLOCAL(sid.0))))
}

fn validate_security_descriptor(identity: &WindowsTaskIdentity, sddl: &str) -> Result<()> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            &windows::core::HSTRING::from(sddl),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .map_err(task_error("parse Task Scheduler security descriptor"))?;
    }
    let _descriptor = LocalAllocation(HLOCAL(descriptor.0));
    let (current_sid, _current_sid) = sid_from_string(&identity.account_sid)?;
    let (system_sid, _system_sid) = sid_from_string("S-1-5-18")?;

    let mut owner = PSID::default();
    let mut _owner_defaulted = windows::core::BOOL::default();
    unsafe {
        GetSecurityDescriptorOwner(descriptor, &mut owner, &mut _owner_defaulted)
            .map_err(task_error("read Task Scheduler descriptor owner"))?;
    }
    if owner.0.is_null() || unsafe { EqualSid(owner, current_sid) }.is_err() {
        return Err(ProvisionError::Service(
            "Task Scheduler descriptor owner is not the current account".into(),
        ));
    }

    let mut control = 0u16;
    let mut _revision = 0u32;
    unsafe {
        GetSecurityDescriptorControl(descriptor, &mut control, &mut _revision)
            .map_err(task_error("read Task Scheduler descriptor control"))?;
    }
    if control & SE_DACL_PROTECTED.0 == 0 {
        return Err(ProvisionError::Service(
            "Task Scheduler descriptor DACL is not protected".into(),
        ));
    }

    let mut dacl_present = windows::core::BOOL::default();
    let mut dacl = std::ptr::null_mut();
    let mut _dacl_defaulted = windows::core::BOOL::default();
    unsafe {
        GetSecurityDescriptorDacl(
            descriptor,
            &mut dacl_present,
            &mut dacl,
            &mut _dacl_defaulted,
        )
        .map_err(task_error("read Task Scheduler descriptor DACL"))?;
    }
    if !dacl_present.as_bool() || dacl.is_null() {
        return Err(ProvisionError::Service(
            "Task Scheduler descriptor has no DACL".into(),
        ));
    }

    let mut information = ACL_SIZE_INFORMATION::default();
    unsafe {
        GetAclInformation(
            dacl,
            (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
        .map_err(task_error("inspect Task Scheduler descriptor DACL"))?;
    }
    if information.AceCount != 2 {
        return Err(ProvisionError::Service(format!(
            "Task Scheduler descriptor must contain exactly two ACEs, found {}",
            information.AceCount
        )));
    }

    let mut current_allowed = false;
    let mut system_allowed = false;
    for index in 0..information.AceCount {
        let mut raw_ace = std::ptr::null_mut();
        unsafe {
            GetAce(dacl, index, &mut raw_ace)
                .map_err(task_error("read Task Scheduler descriptor ACE"))?;
        }
        let ace = raw_ace.cast::<windows::Win32::Security::ACCESS_ALLOWED_ACE>();
        let header = unsafe { &(*ace).Header };
        if header.AceType != 0 {
            return Err(ProvisionError::Service(
                "Task Scheduler descriptor contains a non-allow ACE".into(),
            ));
        }
        if unsafe { (*ace).Mask } != GENERIC_ALL.0 {
            return Err(ProvisionError::Service(
                "Task Scheduler descriptor ACE does not grant full access".into(),
            ));
        }
        let sid = PSID(unsafe { std::ptr::addr_of_mut!((*ace).SidStart).cast() });
        if unsafe { EqualSid(sid, current_sid) }.is_ok() {
            if current_allowed {
                return Err(ProvisionError::Service(
                    "Task Scheduler descriptor duplicates the current-account ACE".into(),
                ));
            }
            current_allowed = true;
        } else if unsafe { EqualSid(sid, system_sid) }.is_ok() {
            if system_allowed {
                return Err(ProvisionError::Service(
                    "Task Scheduler descriptor duplicates the LocalSystem ACE".into(),
                ));
            }
            system_allowed = true;
        } else {
            return Err(ProvisionError::Service(
                "Task Scheduler descriptor grants an additional principal".into(),
            ));
        }
    }
    if !current_allowed || !system_allowed {
        return Err(ProvisionError::Service(
            "Task Scheduler descriptor must allow exactly the current account and LocalSystem"
                .into(),
        ));
    }
    Ok(())
}

fn element_value<'a>(xml: &'a str, opening: &str) -> Result<&'a str> {
    let start = xml
        .find(opening)
        .ok_or_else(|| ProvisionError::Service(format!("missing XML element {opening}")))?
        + opening.len();
    let closing = format!("</{}", &opening[1..]);
    let end = xml[start..]
        .find(&closing)
        .map(|offset| start + offset)
        .ok_or_else(|| ProvisionError::Service(format!("unclosed XML element {opening}")))?;
    Ok(&xml[start..end])
}

fn logon_trigger_enabled(xml: &str) -> Result<bool> {
    let (trigger_start, trigger_end) = logon_trigger_range(xml)?;
    let trigger = &xml[trigger_start..trigger_end];
    let Some(enabled_start) = trigger.find("<Enabled>") else {
        return Ok(true);
    };
    match element_value(&trigger[enabled_start..], "<Enabled>")? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(ProvisionError::Service(format!(
            "invalid logon trigger enabled value {value}"
        ))),
    }
}

fn set_logon_trigger_enabled(xml: &str, enabled: bool) -> Result<String> {
    let (trigger_start, trigger_end) = logon_trigger_range(xml)?;
    let Some(relative) = xml[trigger_start..trigger_end].find("<Enabled>") else {
        let opening_end = xml[trigger_start..trigger_end]
            .find('>')
            .map(|offset| trigger_start + offset + 1)
            .ok_or_else(|| ProvisionError::Service("invalid Moraine logon trigger".into()))?;
        let mut updated = xml.to_owned();
        updated.insert_str(
            opening_end,
            if enabled {
                "<Enabled>true</Enabled>"
            } else {
                "<Enabled>false</Enabled>"
            },
        );
        return Ok(updated);
    };
    let value_start = trigger_start + relative + "<Enabled>".len();
    let value_end = xml[value_start..]
        .find("</Enabled>")
        .map(|offset| value_start + offset)
        .ok_or_else(|| ProvisionError::Service("unclosed trigger enabled state".into()))?;
    let mut updated = xml.to_owned();
    updated.replace_range(
        value_start..value_end,
        if enabled { "true" } else { "false" },
    );
    Ok(updated)
}

fn logon_trigger_range(xml: &str) -> Result<(usize, usize)> {
    let start = xml
        .find("<LogonTrigger")
        .ok_or_else(|| ProvisionError::Service("missing Moraine logon trigger".into()))?;
    let end = xml[start..]
        .find("</LogonTrigger>")
        .map(|offset| start + offset)
        .ok_or_else(|| ProvisionError::Service("unclosed Moraine logon trigger".into()))?;
    Ok((start, end))
}

fn read_application_logs(log_dir: &Path, limit: usize) -> Result<Vec<ServiceLog>> {
    if limit == 0 || !log_dir.exists() {
        return Ok(Vec::new());
    }
    let mut lines = Vec::new();
    for name in [
        "moraine-service.log.3",
        "moraine-service.log.2",
        "moraine-service.log.1",
        "moraine-service.log",
    ] {
        let path = log_dir.join(name);
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        lines.extend(String::from_utf8_lossy(&bytes).lines().map(|line| {
            ServiceLog {
                timestamp: line
                    .split_once(' ')
                    .filter(|(value, _)| chrono::DateTime::parse_from_rfc3339(value).is_ok())
                    .map(|(value, _)| value.to_owned()),
                line: line.to_owned(),
            }
        }));
    }
    if lines.len() > limit {
        lines.drain(..lines.len() - limit);
    }
    Ok(lines)
}

impl BackgroundRuntimeManager for WindowsTaskSchedulerRuntime {
    fn inspect(&self) -> Result<BackgroundRuntimeState> {
        let registration = self.read_task("inspect")?;
        let expected_spec = self.expected_install_spec();
        let binary = self.suite.absolute_service();
        let binary_present = binary.as_ref().is_some_and(|path| path.is_file());
        let (diagnostics_ready, capture_ready, version) = match crate::diagnostics::probe_default()
        {
            Ok(status) => (status.online, status.capture_ready, status.version),
            Err(_) => (false, false, None),
        };
        let (
            registration_present,
            registration_valid,
            running,
            autostart_enabled,
            last_result,
            state,
        ) = match registration {
            Some(task) => {
                let valid =
                    validate_registration(&self.task_identity, &task, Some(&expected_spec)).is_ok();
                let fingerprint = registration_fingerprint(&task.xml, &task.sddl);
                let _last_result = task.last_result;
                (
                    true,
                    valid,
                    task.running || diagnostics_ready,
                    logon_trigger_enabled(&task.xml).unwrap_or(false),
                    task.last_result,
                    Some(RuntimeRegistrationState {
                        kind: RuntimeRegistrationKind::WindowsTaskSchedulerTask,
                        location: Some(self.task_identity.task_path.clone()),
                        fingerprint: Some(fingerprint),
                    }),
                )
            }
            None => (false, false, diagnostics_ready, false, None, None),
        };
        Ok(BackgroundRuntimeState {
            backend: BackgroundRuntimeBackend::WindowsTaskScheduler,
            supported: false,
            installed: registration_present,
            binary_present,
            registration_present,
            registration_valid,
            running,
            autostart_enabled,
            endpoint_ready: diagnostics_ready && capture_ready,
            diagnostics_ready,
            capture_ready,
            binary_path: binary.map(|path| path.display().to_string()),
            unit_path: None,
            version,
            last_result,
            status_message: if running {
                "Background capture is running in an unsupported Windows preview".into()
            } else if registration_present {
                "Background capture is registered but Windows product setup remains unsupported"
                    .into()
            } else {
                "Background capture is not registered; Windows product setup remains unsupported"
                    .into()
            },
            platform: "windows".into(),
            registration: state,
        })
    }

    fn capture_registration(&self) -> Result<RuntimeRegistrationSnapshot> {
        let registration = self.read_task("capture_registration")?;
        Ok(RuntimeRegistrationSnapshot::WindowsTask(
            WindowsTaskSnapshot {
                task_path: self.task_identity.task_path.clone(),
                captured_at: Utc::now().to_rfc3339(),
                state: match registration {
                    Some(task) => WindowsTaskSnapshotState::Existing {
                        fingerprint: registration_fingerprint(&task.xml, &task.sddl),
                        xml: task.xml,
                        security_descriptor: task.sddl,
                    },
                    None => WindowsTaskSnapshotState::Absent,
                },
            },
        ))
    }

    fn registration_fingerprint(&self) -> Result<Option<String>> {
        Ok(self
            .read_task("registration_fingerprint")?
            .map(|task| registration_fingerprint(&task.xml, &task.sddl)))
    }

    fn install(&self, executable: &Path) -> Result<()> {
        self.install_runtime(&RuntimeInstallSpec::try_discover(executable)?)
    }

    fn install_runtime(&self, spec: &RuntimeInstallSpec) -> Result<()> {
        if !spec.executable.is_absolute() || !spec.executable.is_file() {
            return Err(ProvisionError::Service(format!(
                "Windows service executable must be an existing absolute file: {}",
                spec.executable.display()
            )));
        }
        if !spec.working_directory.is_absolute() || !spec.working_directory.is_dir() {
            return Err(ProvisionError::Service(
                "Windows runtime working directory must be an existing absolute directory".into(),
            ));
        }
        let expected_pipe = match &self.runtime_layout.capture_endpoint {
            CaptureEndpoint::WindowsNamedPipe(pipe) => pipe,
            _ => {
                return Err(ProvisionError::Service(
                    "Windows runtime layout has no named-pipe endpoint".into(),
                ))
            }
        };
        if spec.capture_endpoint != CaptureEndpoint::WindowsNamedPipe(expected_pipe.clone()) {
            return Err(ProvisionError::Service(
                "Windows task endpoint does not match the current account".into(),
            ));
        }
        let expected = self.expected_install_spec();
        if !paths_equal(&spec.executable, &expected.executable)
            || !paths_equal(&spec.working_directory, &expected.working_directory)
            || spec.diagnostics_endpoint != expected.diagnostics_endpoint
            || spec.spool_dir != expected.spool_dir
            || spec.log_dir != expected.log_dir
        {
            return Err(ProvisionError::Service(
                "Windows runtime install specification differs from the authoritative suite layout"
                    .into(),
            ));
        }
        let xml = render_task_xml(&self.task_identity, spec)?;
        let sddl = task_sddl(&self.task_identity);
        let identity = self.task_identity.clone();
        let spec = spec.clone();
        self.run_com_operation("install", move |session| {
            session.register(&identity, &xml, Some(&sddl))?;
            let installed = session
                .read(&identity)?
                .ok_or_else(|| ProvisionError::Service("task absent after registration".into()))?;
            validate_registration(&identity, &installed, Some(&spec))?;
            if logon_trigger_enabled(&installed.xml)? {
                return Err(ProvisionError::Service(
                    "new Task Scheduler registration must start with autostart disabled".into(),
                ));
            }
            Ok(())
        })
    }

    fn restore_registration(&self, snapshot: &RuntimeRegistrationSnapshot) -> Result<()> {
        let RuntimeRegistrationSnapshot::WindowsTask(snapshot) = snapshot else {
            return Err(ProvisionError::RollbackRequired(
                "Windows runtime cannot restore a file registration snapshot".into(),
            ));
        };
        if snapshot.task_path != self.task_identity.task_path {
            return Err(ProvisionError::RollbackRequired(format!(
                "snapshot task {} does not match {}",
                snapshot.task_path, self.task_identity.task_path
            )));
        }
        let identity = self.task_identity.clone();
        let state = snapshot.state.clone();
        if let WindowsTaskSnapshotState::Existing {
            xml,
            security_descriptor,
            fingerprint,
        } = &state
        {
            if registration_fingerprint(xml, security_descriptor) != *fingerprint {
                return Err(ProvisionError::RollbackRequired(
                    "Windows task snapshot fingerprint is corrupt".into(),
                ));
            }
        }
        let restored = self.run_com_operation("restore_registration", move |session| {
            session.delete(&identity)?;
            match state {
                WindowsTaskSnapshotState::Absent => Ok(()),
                WindowsTaskSnapshotState::Existing {
                    xml,
                    security_descriptor: _,
                    fingerprint,
                } => {
                    // Returned Task Scheduler XML embeds the normalized descriptor.
                    // Reapplying a separate SDDL changes the returned XML.
                    session.register(&identity, &xml, None)?;
                    let restored = session.read(&identity)?.ok_or_else(|| {
                        ProvisionError::RollbackRequired(
                            "restored Windows task is absent".into(),
                        )
                    })?;
                    let actual = registration_fingerprint(&restored.xml, &restored.sddl);
                    if actual != fingerprint {
                        return Err(ProvisionError::RollbackRequired(format!(
                            "restored Windows task fingerprint differs: expected {fingerprint}, got {actual}"
                        )));
                    }
                    Ok(())
                }
            }
        });
        restored.map_err(|error| match error {
            already @ ProvisionError::RollbackRequired(_) => already,
            other => ProvisionError::RollbackRequired(other.to_string()),
        })
    }

    fn uninstall(&self) -> Result<()> {
        let identity = self.task_identity.clone();
        self.run_com_operation("uninstall", move |session| session.delete(&identity))
    }

    fn start(&self) -> Result<()> {
        let identity = self.task_identity.clone();
        self.run_com_operation("start", move |session| {
            let task = session.get(&identity)?.ok_or_else(|| {
                ProvisionError::Service(format!("task {} is absent", identity.task_path))
            })?;
            unsafe {
                task.Run(&VARIANT::default())
                    .map_err(task_error("start Task Scheduler runtime"))?;
            }
            Ok(())
        })
    }

    fn stop(&self) -> Result<()> {
        let identity = self.task_identity.clone();
        self.run_com_operation("stop", move |session| {
            session.stop_and_wait(&identity, STOP_BUDGET)
        })
    }

    fn restart(&self) -> Result<()> {
        let identity = self.task_identity.clone();
        self.run_com_operation("restart", move |session| {
            session.stop_and_wait(&identity, STOP_BUDGET)?;
            let task = session.get(&identity)?.ok_or_else(|| {
                ProvisionError::Service(format!("task {} is absent", identity.task_path))
            })?;
            unsafe {
                task.Run(&VARIANT::default())
                    .map_err(task_error("restart Task Scheduler runtime"))?;
            }
            Ok(())
        })
    }

    fn enable_autostart(&self) -> Result<()> {
        self.mutate_trigger(true)
    }

    fn disable_autostart(&self) -> Result<()> {
        self.mutate_trigger(false)
    }

    fn logs(&self, limit: usize) -> Result<Vec<ServiceLog>> {
        read_application_logs(&self.runtime_layout.log_dir, limit)
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn identity() -> WindowsTaskIdentity {
        WindowsTaskIdentity::for_scope("S-1-5-21-1000-2000-3000-1001".into(), "d07be4ed3160".into())
    }

    fn spec() -> RuntimeInstallSpec {
        RuntimeInstallSpec {
            executable: PathBuf::from(r"C:\Program Files\Moraine\moraine-service.exe"),
            working_directory: PathBuf::from(r"C:\Program Files\Moraine"),
            capture_endpoint: CaptureEndpoint::WindowsNamedPipe(
                r"\\.\pipe\moraine.capture.v1.d07be4ed3160".into(),
            ),
            diagnostics_endpoint: "127.0.0.1:33111".parse().unwrap(),
            spool_dir: PathBuf::from(r"C:\Users\A B\AppData\Local\Moraine\spool"),
            log_dir: Some(PathBuf::from(
                r#"C:\Users\A "Quoted"\AppData\Local\Moraine\logs\"#,
            )),
        }
    }

    fn registration(xml: String) -> TaskRegistration {
        let identity = identity();
        TaskRegistration {
            xml,
            sddl: task_sddl(&identity),
            running: false,
            last_result: None,
        }
    }

    #[test]
    fn identity_and_rendering_preserve_scheduler_contracts() {
        let identity = identity();
        assert_eq!(
            identity.task_path,
            r"\Moraine Background Capture (d07be4ed3160)"
        );
        let xml = render_task_xml(&identity, &spec()).unwrap();
        for contract in [
            "<Source>Moraine</Source>",
            "<Enabled>false</Enabled>",
            "<LogonType>InteractiveToken</LogonType>",
            "<RunLevel>LeastPrivilege</RunLevel>",
            "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
            "<AllowStartOnDemand>true</AllowStartOnDemand>",
            "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>",
            "<RestartOnFailure><Interval>PT1M</Interval><Count>3</Count>",
        ] {
            assert!(xml.contains(contract), "missing {contract}");
        }
        assert!(xml.contains("<Command>C:\\Program Files\\Moraine\\moraine-service.exe</Command>"));
        assert!(xml.contains("&quot;"));
        assert!(xml.contains("Quoted"));
        assert!(!xml.contains("powershell"));
        assert!(!xml.contains("cmd.exe"));
    }

    #[test]
    fn windows_argument_quoting_handles_quotes_and_trailing_backslashes() {
        assert_eq!(quote_windows_argument("plain"), "plain");
        assert_eq!(quote_windows_argument("two words"), r#""two words""#);
        assert_eq!(quote_windows_argument(r#"a"b"#), r#""a\"b""#);
        assert_eq!(
            quote_windows_argument(r#"C:\two words\"#),
            r#""C:\two words\\""#
        );
    }

    #[test]
    fn normalized_default_omissions_remain_valid_but_inverse_values_fail() {
        let identity = identity();
        let mut xml = render_task_xml(&identity, &spec()).unwrap();
        for omitted in [
            "<RunLevel>LeastPrivilege</RunLevel>",
            "<RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>",
            "<AllowStartOnDemand>true</AllowStartOnDemand>",
            "<Hidden>false</Hidden>",
            "<RunOnlyIfIdle>false</RunOnlyIfIdle>",
            "<WakeToRun>false</WakeToRun>",
        ] {
            xml = xml.replace(omitted, "");
        }
        let registration = registration(xml.clone());
        validate_registration(&identity, &registration, Some(&spec())).unwrap();

        let elevated = TaskRegistration {
            xml: xml.replace(
                "</Principal>",
                "<RunLevel>HighestAvailable</RunLevel></Principal>",
            ),
            ..registration
        };
        assert!(validate_registration(&identity, &elevated, Some(&spec())).is_err());
    }

    #[test]
    fn complete_task_ownership_rejects_extra_structure_and_wrong_sids() {
        let identity = identity();
        let xml = render_task_xml(&identity, &spec()).unwrap();
        let extra_principal = xml.replace(
            "</Principals>",
            "<Principal><UserId>S-1-5-18</UserId><LogonType>InteractiveToken</LogonType></Principal></Principals>",
        );
        assert!(
            validate_registration(&identity, &registration(extra_principal), Some(&spec()))
                .is_err()
        );

        let extra_trigger = xml.replace(
            "</Triggers>",
            "<TimeTrigger><StartBoundary>2026-01-01T00:00:00</StartBoundary></TimeTrigger></Triggers>",
        );
        assert!(
            validate_registration(&identity, &registration(extra_trigger), Some(&spec())).is_err()
        );

        let extra_action = xml.replace(
            "</Actions>",
            "<Exec><Command>other.exe</Command></Exec></Actions>",
        );
        assert!(
            validate_registration(&identity, &registration(extra_action), Some(&spec())).is_err()
        );

        let wrong_principal = xml.replacen(
            &format!("<UserId>{}</UserId>", identity.account_sid),
            "<UserId>S-1-5-18</UserId>",
            1,
        );
        assert!(
            validate_registration(&identity, &registration(wrong_principal), Some(&spec()))
                .is_err()
        );

        let trigger_marker = format!(
            "<LogonTrigger>\n      <Enabled>false</Enabled>\n      <UserId>{}</UserId>",
            identity.account_sid
        );
        let wrong_trigger = xml.replace(
            &trigger_marker,
            "<LogonTrigger>\n      <Enabled>false</Enabled>\n      <UserId>S-1-5-18</UserId>",
        );
        assert!(
            validate_registration(&identity, &registration(wrong_trigger), Some(&spec())).is_err()
        );
    }

    #[test]
    fn effective_acl_rejects_extra_unprotected_and_non_full_entries() {
        let identity = identity();
        let xml = render_task_xml(&identity, &spec()).unwrap();
        for sddl in [
            format!(
                "O:{sid}D:P(A;;FA;;;SY)(A;;FA;;;{sid})(A;;FA;;;S-1-5-32-545)",
                sid = identity.account_sid
            ),
            format!(
                "O:{sid}D:(A;;FA;;;SY)(A;;FA;;;{sid})",
                sid = identity.account_sid
            ),
            format!(
                "O:{sid}D:P(A;;FR;;;SY)(A;;FA;;;{sid})",
                sid = identity.account_sid
            ),
            format!(
                "O:{sid}D:P(D;;FA;;;SY)(A;;FA;;;{sid})",
                sid = identity.account_sid
            ),
        ] {
            let mut candidate = registration(xml.clone());
            candidate.sddl = sddl;
            assert!(validate_registration(&identity, &candidate, Some(&spec())).is_err());
        }
    }

    #[test]
    fn fingerprint_uses_xml_nul_sddl() {
        let expected = hex::encode(Sha256::digest(b"<Task />\0O:SY"));
        assert_eq!(registration_fingerprint("<Task />", "O:SY"), expected);
    }

    #[test]
    fn log_reader_is_chronological_bounded_and_lossy() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("moraine-service.log.2"),
            b"2026-07-29T00:00:00Z oldest\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("moraine-service.log"),
            b"2026-07-29T00:00:02Z current\nbad \xff\n",
        )
        .unwrap();
        let logs = read_application_logs(temp.path(), 2).unwrap();
        assert_eq!(logs.len(), 2);
        assert!(logs[0].line.contains("current"));
        assert!(logs[1].line.contains('\u{fffd}'));
        assert!(read_application_logs(temp.path(), 0).unwrap().is_empty());
        assert!(read_application_logs(&temp.path().join("missing"), 10)
            .unwrap()
            .is_empty());
    }
}
