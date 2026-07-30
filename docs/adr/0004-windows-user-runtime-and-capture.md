# ADR 0004; Windows user runtime & capture

Status: accepted for W2 implementation

Date: 2026-07-29

## Context

Moraine needs a Windows runtime that preserves its current product boundaries:

* one trusted local user;
* no administrator requirement;
* background capture while that user is logged in;
* direct lifecycle inspection & repair;
* exact transactional restoration;
* local IPC with no network fallback;
* unchanged event, run & ledger formats.

W1 made capture transport & background-runtime lifecycle replaceable. It did
not select the Windows mechanisms or claim Windows runtime support.

## Decision

Windows will use:

* Task Scheduler 2.0 for the current-user background runtime;
* Task Scheduler COM interfaces through Microsoft's `windows` crate;
* a current-user logon trigger with `TASK_LOGON_INTERACTIVE_TOKEN`;
* `TASK_RUNLEVEL_LUA`;
* one SID-qualified task in the Task Scheduler root;
* one SID-qualified Windows named pipe;
* explicit task & pipe security descriptors;
* one pipe connection per serialized capture event;
* the existing durable spool when direct delivery is unavailable.

W2-A records these contracts & validates the risky host assumptions. Production
backends remain unsupported until later W2 slices implement & accept them.

## Account scope

Both runtime & capture identity derive from the current account SID. Resolve it
from the process access token with `OpenProcessToken` &
`GetTokenInformation(TokenUser)`.

Convert the SID to its string form, hash that UTF-8 string with SHA-256 & use
the first 12 lowercase hexadecimal characters:

```text
scope_id = sha256(account_sid_string)[0..12]
```

The SID is stable when an account is renamed. Usernames, project paths,
installation paths, process IDs & random startup values are not stable security
identities & must not enter either endpoint name.

This is an account boundary, not a logon-session boundary. Separate local
sessions for the same account share Moraine's trust boundary. A process already
running as that user is trusted by the current product model.

## Task Scheduler runtime

### Identity & metadata

Register one task in the Task Scheduler root:

```text
\Moraine Background Capture (<scope_id>)
```

Use:

```text
Author: Moraine
Source: Moraine
Description: Runs local Moraine capture for the current Windows user.
URI: \Moraine\<scope_id>\BackgroundCapture
Version: 1
```

The root avoids relying on standard-user permission to create a custom folder.
The SID suffix prevents collisions between accounts on one machine.

### Principal

The task principal is fixed:

```text
UserId: current account SID
LogonType: TASK_LOGON_INTERACTIVE_TOKEN
RunLevel: TASK_RUNLEVEL_LUA
```

Moraine must not use password, S4U, service-account or highest-runlevel task
principals. It runs in the same ordinary user context as the coding agent.

### Trigger & autostart

Register exactly one logon trigger:

```text
Type: LogonTrigger
UserId: current account SID
Delay: PT5S
Enabled: Moraine autostart state
```

Keep the task itself enabled. Autostart is the logon trigger's enabled state:

```text
task present + trigger enabled   = autostart enabled
task present + trigger disabled  = demand start only
task absent                      = runtime not installed
```

Disabling autostart must not prevent `IRegisteredTask::Run`.

### Action

Register one direct executable action:

```text
Path: <absolute suite path>\moraine-service.exe
Arguments:
  --http 127.0.0.1:33111
  --named-pipe <resolved pipe name>
  --spool-dir <absolute spool path>
  --log-dir <absolute log path>
WorkingDirectory: <absolute suite prefix>
```

Do not invoke PowerShell, `cmd.exe`, a wrapper script or `schtasks.exe`.
Validity compares the registered action with the authoritative suite & runtime
layouts.

### Settings

Use:

```text
Enabled: true
AllowDemandStart: true
ExecutionTimeLimit: PT0S
MultipleInstancesPolicy: IgnoreNew
StartWhenAvailable: true
RestartCount: 3
RestartInterval: PT1M
DisallowStartIfOnBatteries: false
StopIfGoingOnBatteries: false
RunOnlyIfIdle: false
RunOnlyIfNetworkAvailable: false
WakeToRun: false
Hidden: false
```

`IgnoreNew` prevents duplicate runtime processes. `PT0S` permits an indefinite
run. The task remains visible to the user.

### Task security

Supply an explicit security descriptor when registering the task:

```text
Owner:
  current account SID

Allow:
  current account SID; full task management
  LocalSystem; full task management
```

Do not grant Everyone, Anonymous, Builtin Users or Authenticated Users. Validate
the effective descriptor after registration. LocalSystem retains access because
Task Scheduler requires it for reliable task management.

### COM ownership

Production management uses Task Scheduler 2.0 COM interfaces through the
`windows` crate. `WindowsTaskSchedulerRuntime` stores only ordinary Rust data:

```rust
pub struct WindowsTaskSchedulerRuntime {
    suite: SuitePaths,
    task_identity: WindowsTaskIdentity,
    operation_lock: Mutex<()>,
}
```

It must not retain COM interface pointers. Each manager method locks
`operation_lock`, starts a dedicated worker thread, initializes that worker as
MTA with `COINIT_MULTITHREADED`, performs one bounded COM transaction & joins
the worker before returning. Every Task Scheduler interface is created, used &
released on that worker. Successful COM initialization is balanced with
`CoUninitialize`.

This boundary avoids depending on the caller's apartment. In particular, Tauri
may call the synchronous runtime trait from a desktop thread already initialized
as STA; initializing that same thread as MTA can fail with
`RPC_E_CHANGED_MODE`. A persistent worker is unnecessary because Task Scheduler
operations are infrequent.

COM pointers must never cross threads. Worker startup failure, panic & HRESULT
failure become structured provisioning errors. The ordinary Rust fields plus
the operation mutex preserve the runtime manager's `Send + Sync` contract.

### Lifecycle mapping

The Windows backend will add:

```text
BackgroundRuntimeBackend::WindowsTaskScheduler
RuntimeRegistrationKind::WindowsTaskSchedulerTask
```

Operations map as follows:

* `inspect`; read registration, task state, trigger, action, principal, settings,
  ACL, diagnostics & capture readiness.
* `capture_registration`; capture Task Scheduler-returned XML plus SDDL.
* `registration_fingerprint`; hash returned XML plus SDDL.
* `install_runtime`; register or replace the exact definition.
* `restore_registration`; restore exact prior XML plus ACL or exact absence.
* `uninstall`; stop & delete only the SID-qualified Moraine task.
* `start`; call `IRegisteredTask::Run`.
* `stop`; stop all running instances of that task.
* `restart`; stop, wait for stopped state & run.
* `enable_autostart`; enable only the Moraine logon trigger.
* `disable_autostart`; disable only that trigger.
* `logs`; read Moraine's user-scoped application log.

Windows must replace only the Windows branch of the production runtime factory.
Memory remains test-only.

## Transactional restoration

Windows registration is not a file. Do not read or write
`C:\Windows\System32\Tasks`.

Extend the untagged registration snapshot with:

```rust
pub struct WindowsTaskSnapshot {
    pub task_path: String,
    pub captured_at: String,
    pub state: WindowsTaskSnapshotState,
}

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WindowsTaskSnapshotState {
    Existing {
        xml: String,
        security_descriptor: String,
        fingerprint: String,
    },
    Absent,
}

#[serde(untagged)]
pub enum RuntimeRegistrationSnapshot {
    File(FileSnapshot),
    WindowsTask(WindowsTaskSnapshot),
}
```

The outer enum stays untagged so existing Linux journals remain readable.

Fingerprint:

```text
SHA-256(UTF-8(returned XML) + NUL + UTF-8(returned SDDL))
```

Always re-read the registered task before hashing; Task Scheduler may normalize
the supplied definition. In particular, returned XML may omit
`RunLevel=LeastPrivilege` & false-valued settings when they are Task Scheduler
defaults. Validation accepts those omissions, requires every non-default
contract value & rejects inverse values that would elevate or constrain the
runtime.

Task Scheduler may also omit the optional registration URI from returned XML.
Moraine still supplies the URI as metadata, but returned-state identity is
proved by the exact SID-qualified task path, current-account principal & task
ACL rather than by URI preservation.

Registration validity parses the returned task definition & effective security
descriptor. A valid Moraine registration has exactly one current-account
interactive-token principal, one current-account logon trigger & one direct
executable action. Its protected DACL has exactly two full-access allow ACEs;
one for the current account & one for LocalSystem. Deny ACEs, inherited access
& additional principals are invalid.

Restoration must stop & delete the current registration, then register the
captured Task Scheduler-returned XML without separately overriding its embedded
security descriptor. Passing both captured XML & SDDL makes Task Scheduler
rewrite the XML representation even when the effective ACL is unchanged.
Re-read XML & SDDL, verify the combined fingerprint, then restore prior
autostart & running state. Exact absence is restored by deletion. Any unproven
restoration yields `RollbackRequired`.

Stopping is a bounded proof, not a best-effort request. Stop, restart, delete,
uninstall & restoration all request termination of the exact task then poll its
running-instance collection until empty. Registration deletion or replacement
does not begin until termination is proven.

## Named-pipe capture

### Identity

Use:

```text
\\.\pipe\moraine.capture.v1.<scope_id>
```

The CLI, service, platform layout, provisioning verification & desktop health
must resolve the same account-SID-derived name.

The service is the only pipe server. Hooks are clients. Provisioning & desktop
code may inspect the endpoint but must not create a substitute server.

### Security descriptor

Never pass null security attributes. Build a protected descriptor before the
first pipe instance:

```text
Owner:
  current account SID

Allow:
  current account SID; full pipe access
  LocalSystem; full pipe access
```

Do not grant Everyone, Anonymous, Builtin Users or Authenticated Users. Set
`bInheritHandle = FALSE` & pass the descriptor through `SECURITY_ATTRIBUTES`.
The effective owner & DACL are part of verification.

This descriptor does not defend against a malicious same-account process; that
limitation follows Moraine's single-trusted-user threat model.

### Server ownership & options

Use Tokio's Windows named-pipe support with:

```text
Direction: inbound only
Mode: byte
Remote clients: rejected
First instance: required for the initial instance
Maximum instances: 16
Input buffer: MAX_EVENT_BYTES + 1
Output buffer: minimal
```

The first instance uses `FILE_FLAG_FIRST_PIPE_INSTANCE`. If another server owns
the name, startup fails; Moraine does not connect to, replace or report that
server as ready. Later instances created by the same process omit the flag.

Create the next listening instance before handing off a connected one so
concurrent clients do not observe a listener gap. `PIPE_REJECT_REMOTE_CLIENTS`
must remain set on every instance.

### Framing

Preserve Unix capture semantics:

```text
one client connection = one serialized event
```

The client opens the pipe write-only, writes the complete payload, flushes &
closes. Tokio clients default to read-write; the Windows backend must explicitly
disable client reads because the server is inbound-only. The server accepts,
creates the next listener, reads to EOF with a
`MAX_EVENT_BYTES + 1` cap, writes through the existing spool path & closes the
connected instance.

Do not add headers, acknowledgements, bidirectional commands, authentication
tokens, JSON-RPC or schema changes.

### Failure behavior

Use a bounded connection budget of about two seconds. File-not-found, pipe-busy
after bounded retry, semaphore-timeout & broken-pipe errors mean temporary
unavailability. Access denial is a security/configuration failure internally.

In every delivery failure:

* preserve the existing spool fallback where it can be processed;
* emit local diagnostic context;
* do not disrupt the coding agent;
* do not claim successful capture.

A listener bind or supervision failure prevents capture readiness & terminates
the runtime so Task Scheduler restart policy can recover it.

## Capability separation

Runtime support & distribution support are independent:

```rust
runtime_capture_supported =
    capture_transport == Supported &&
    background_runtime == Supported

desktop_runtime_supported =
    runtime_capture_supported &&
    desktop_host == Supported

distribution_supported =
    user_installation == Supported
```

W2 may validate a manually staged Windows suite when runtime capabilities are
implemented. It must not claim a supported installer. W3 changes Windows
installation support after signed distribution is accepted.

Current Windows capability values remain unsupported throughout W2-A.

## Logging

Task Scheduler is not the application log. The Windows runtime will write a
user-scoped rolling log:

```text
%LOCALAPPDATA%\Moraine\logs\moraine-service.log
```

`logs(limit)` reads this file. Task state, last result & run times may supplement
diagnostics but Task Scheduler event text is not presented as Moraine logs.

The service must use the Windows GUI subsystem so a login start does not show a
console window.

## Rejected alternatives

* Windows Service; machine-scoped, usually elevated & wrong for current-user
  installation.
* HKCU Run key; launch-only with inadequate lifecycle, inspection & rollback.
* Startup folder; launch-only with the same lifecycle gap.
* PowerShell or `schtasks.exe`; shell quoting, locale-specific output & external
  command dependency.
* TCP capture; broadens the capture transport boundary.
* Machine-wide, password-backed, S4U, service-account or elevated tasks; wrong
  security context.
* Default pipe ACL; permits broader access than Moraine's trust model.

## Validation boundary

Windows CI must prove, with UUID-qualified disposable resources:

* current-account interactive-token task registration through COM;
* low run level & no stored password;
* explicit task ACL readback;
* demand start & stop;
* trigger-only autostart toggling;
* exact XML plus ACL capture, mutation & restoration;
* exact absence restoration;
* current-user named-pipe connection;
* first-instance exclusion;
* explicit owner & DACL;
* remote-client rejection flags;
* byte-preserving, one-connection framing;
* size enforcement & concurrent listener continuity.

Cleanup guards must remove test resources. GitHub's hosted Windows runner uses
an administrator account; it cannot prove the no-admin user experience. CI
evidence is not real Windows 11 graphical acceptance. A separate standard-user
VM must prove non-administrator registration & cross-account denial before
runtime support is claimed.

## Consequences

W2 implementation has no remaining mechanism choice. It adds a Windows
named-pipe backend & a Task Scheduler backend without changing shared product
or persistence protocols.

W3 owns installation, signing & WinGet. Current public architecture remains
unchanged until production backends & real-session acceptance are complete.
