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
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND};
use windows::Win32::Security::{DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION};
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
            if restored.xml != xml || restored.sddl != current.sddl {
                return Err(ProvisionError::Service(
                    "Task Scheduler trigger update changed unrelated registration state".into(),
                ));
            }
            validate_registration(&identity, &restored, None)?;
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
        if let Some(task) = self.get(identity)? {
            unsafe {
                let _ = task.Stop(0);
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
    let xml = &registration.xml;
    for required in [
        "<Author>Moraine</Author>",
        "<Source>Moraine</Source>",
        "<LogonType>InteractiveToken</LogonType>",
        "<RunLevel>LeastPrivilege</RunLevel>",
        "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
        "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
        "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
        "<StartWhenAvailable>true</StartWhenAvailable>",
        "<RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>",
        "<AllowStartOnDemand>true</AllowStartOnDemand>",
        "<Hidden>false</Hidden>",
        "<RunOnlyIfIdle>false</RunOnlyIfIdle>",
        "<WakeToRun>false</WakeToRun>",
        "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>",
        "<Interval>PT1M</Interval>",
        "<Count>3</Count>",
    ] {
        if !xml.contains(required) {
            return Err(ProvisionError::Service(format!(
                "Task Scheduler registration is missing required contract {required}"
            )));
        }
    }
    if !xml.contains(&xml_escape(&identity.account_sid))
        || !xml.contains(&xml_escape(&identity.task_uri))
    {
        return Err(ProvisionError::Service(
            "Task Scheduler identity does not match the current account".into(),
        ));
    }
    if xml.matches("<LogonTrigger").count() != 1 {
        return Err(ProvisionError::Service(
            "Task Scheduler registration must contain exactly one logon trigger".into(),
        ));
    }
    validate_sddl(identity, &registration.sddl)?;
    if let Some(spec) = spec {
        let expected = render_task_xml(identity, spec)?;
        for section in ["<Command>", "<Arguments>", "<WorkingDirectory>"] {
            let expected_value = element_value(&expected, section)?;
            let actual_value = element_value(xml, section)?;
            if expected_value != actual_value {
                return Err(ProvisionError::Service(format!(
                    "Task Scheduler action {section} does not match the install specification"
                )));
            }
        }
    }
    Ok(())
}

fn validate_sddl(identity: &WindowsTaskIdentity, sddl: &str) -> Result<()> {
    let administrator_alias = identity.account_sid.ends_with("-500");
    let owns_current = sddl.contains(&format!("O:{}", identity.account_sid))
        || (administrator_alias && sddl.contains("O:LA"));
    let allows_current = sddl.contains(&format!(";;;{})", identity.account_sid))
        || (administrator_alias && sddl.contains(";;;LA)"));
    if !owns_current || !allows_current || !sddl.contains(";;;SY)") {
        return Err(ProvisionError::Service(
            "Task Scheduler ACL does not grant the current account and LocalSystem".into(),
        ));
    }
    for broad in ["WD", "AN", "BU", "AU"] {
        if sddl.contains(&format!(";;;{broad})")) {
            return Err(ProvisionError::Service(format!(
                "Task Scheduler ACL unexpectedly grants broad principal {broad}"
            )));
        }
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
    let trigger_start = xml
        .find("<LogonTrigger")
        .ok_or_else(|| ProvisionError::Service("missing Moraine logon trigger".into()))?;
    let trigger_end = xml[trigger_start..]
        .find("</LogonTrigger>")
        .map(|offset| trigger_start + offset)
        .ok_or_else(|| ProvisionError::Service("unclosed Moraine logon trigger".into()))?;
    let trigger = &xml[trigger_start..trigger_end];
    match element_value(trigger, "<Enabled>")? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(ProvisionError::Service(format!(
            "invalid logon trigger enabled value {value}"
        ))),
    }
}

fn set_logon_trigger_enabled(xml: &str, enabled: bool) -> Result<String> {
    let trigger_start = xml
        .find("<LogonTrigger")
        .ok_or_else(|| ProvisionError::Service("missing Moraine logon trigger".into()))?;
    let trigger_end = xml[trigger_start..]
        .find("</LogonTrigger>")
        .map(|offset| trigger_start + offset)
        .ok_or_else(|| ProvisionError::Service("unclosed Moraine logon trigger".into()))?;
    let relative = xml[trigger_start..trigger_end]
        .find("<Enabled>")
        .ok_or_else(|| ProvisionError::Service("logon trigger has no enabled state".into()))?;
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
                    security_descriptor,
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
            if let Some(task) = session.get(&identity)? {
                unsafe {
                    let _ = task.Stop(0);
                }
            }
            Ok(())
        })
    }

    fn restart(&self) -> Result<()> {
        let identity = self.task_identity.clone();
        self.run_com_operation("restart", move |session| {
            let task = session.get(&identity)?.ok_or_else(|| {
                ProvisionError::Service(format!("task {} is absent", identity.task_path))
            })?;
            unsafe {
                let _ = task.Stop(0);
            }
            let deadline = Instant::now() + STOP_BUDGET;
            while unsafe {
                task.State()
                    .map_err(task_error("read stopped task state"))?
            } == TASK_STATE_RUNNING
            {
                if Instant::now() >= deadline {
                    return Err(ProvisionError::Service(
                        "Task Scheduler runtime did not stop within five seconds".into(),
                    ));
                }
                thread::sleep(STOP_POLL);
            }
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
        assert!(xml.contains("&quot;Quoted&quot;"));
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
