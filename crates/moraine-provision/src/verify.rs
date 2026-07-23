//! Product vs direct verification.
//!
//! ProductCapture Ready requires adapter/hook delivery of a **unique** session/event
//! and a discoverable run bound to that session — never a stale self-test run.
//! Direct core APIs are never used as a fallback for product Ready.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use moraine_core::{
    list_run_summaries, provisional_run_ensure, resolve_existing_project, run_start,
    session_observe, ProvisionalRunRequest, RunStartRequest, SessionObserveRequest,
};
use serde_json::json;
use uuid::Uuid;

use crate::agent::adapter_for;
use crate::error::Result;
use crate::service_ready::{
    default_service_probe, default_service_ready_timeout_ms, ServiceProbe,
};
use crate::suite::SuitePaths;
use crate::types::{
    Readiness, SetupIntent, VerificationMode, VerificationReport, VerificationStep,
};

/// Options for verification (capture + service probe injectables for tests).
#[derive(Clone)]
pub struct VerifyOptions {
    pub mode: VerificationMode,
    pub capture: Option<Arc<dyn EventCapture>>,
    pub service_probe: Option<Arc<dyn ServiceProbe>>,
}

/// Injectable capture delivery (real hook-codex or test double).
pub trait EventCapture: Send + Sync {
    /// Deliver synthetic hooks for `session_id` carrying `event_id`.
    /// On success, returns a run_id that is discoverable **and** bound to this session
    /// (and ideally references `event_id` in session/run metadata or objective).
    fn deliver_and_materialize(
        &self,
        project: &Path,
        cli: &Path,
        session_id: &str,
        event_id: &str,
    ) -> std::result::Result<Uuid, String>;
}

/// Real product path: invoke suite CLI hook-codex; poll **session-bound** runs only.
pub struct HookCodexCapture {
    pub poll_timeout: Duration,
}

impl Default for HookCodexCapture {
    fn default() -> Self {
        Self {
            poll_timeout: Duration::from_secs(8),
        }
    }
}

impl EventCapture for HookCodexCapture {
    fn deliver_and_materialize(
        &self,
        project: &Path,
        cli: &Path,
        session_id: &str,
        event_id: &str,
    ) -> std::result::Result<Uuid, String> {
        if !cli.is_file() || !is_moraine_cli_binary(cli) {
            return Err(format!(
                "suite CLI not available for hook delivery: {}",
                cli.display()
            ));
        }
        // Structural event_id + prompt marker (never accept stale runs).
        let prompt = format!("Moraine self-test event_id={event_id}");
        invoke_hook_codex(cli, project, session_id, event_id, "SessionStart", None)?;
        invoke_hook_codex(
            cli,
            project,
            session_id,
            event_id,
            "UserPromptSubmit",
            Some(&prompt),
        )?;

        let resolved = resolve_existing_project(Some(project))
            .map_err(|e| format!("project resolve after hook: {e}"))?;
        let deadline = Instant::now() + self.poll_timeout;
        let mut delay = Duration::from_millis(100);
        while Instant::now() < deadline {
            // ONLY accept runs bound to this unique session_id AND event_id marker.
            if let Ok(Some(run_id)) = find_session_run(&resolved.project_root, session_id) {
                let runs = list_run_summaries(&resolved.project_root, resolved.project_id);
                if let Some(r) = runs.iter().find(|r| r.run_id == run_id) {
                    if r.objective.contains(event_id) {
                        return Ok(run_id);
                    }
                    // Keep polling until objective reflects the unique event_id.
                }
            }
            std::thread::sleep(delay);
            delay = (delay * 2).min(Duration::from_millis(800));
        }
        Err(format!(
            "no session-bound run for session={session_id} event_id={event_id} within {:?}",
            self.poll_timeout
        ))
    }
}

/// Test double: fails or materializes a run **bound to session_id + event_id**.
pub struct ControlledCapture {
    pub fail_delivery: bool,
    pub materialize_run: bool,
}

impl EventCapture for ControlledCapture {
    fn deliver_and_materialize(
        &self,
        project: &Path,
        _cli: &Path,
        session_id: &str,
        event_id: &str,
    ) -> std::result::Result<Uuid, String> {
        if self.fail_delivery {
            return Err(format!(
                "controlled capture: hook delivery failed for event_id={event_id}"
            ));
        }
        if !self.materialize_run {
            return Err(format!(
                "controlled capture: delivery ok but no materialization for event_id={event_id}"
            ));
        }
        // Simulate service processing this unique session/event (test double only).
        let run_id = direct_core_capture_with_marker(project, session_id, event_id)?;
        // Must be discoverable via session binding.
        let resolved = resolve_existing_project(Some(project)).map_err(|e| e.to_string())?;
        let bound = find_session_run(&resolved.project_root, session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("session {session_id} has no bound run after materialize"))?;
        if bound != run_id {
            return Err(format!(
                "session-bound run {bound} != materialized {run_id}"
            ));
        }
        Ok(run_id)
    }
}

/// Default verify: product when !skip_service else direct.
pub fn verify(intent: &SetupIntent) -> Result<VerificationReport> {
    if intent.skip_service {
        verify_with_options(
            intent,
            VerifyOptions {
                mode: VerificationMode::DirectCoreTest,
                capture: None,
                service_probe: None,
            },
        )
    } else {
        verify_with_options(
            intent,
            VerifyOptions {
                mode: VerificationMode::ProductCapture,
                capture: Some(Arc::new(HookCodexCapture::default())),
                service_probe: Some(default_service_probe()),
            },
        )
    }
}

/// Convenience wrapper.
pub fn verify_with(
    intent: &SetupIntent,
    mode: VerificationMode,
    capture: Option<Arc<dyn EventCapture>>,
) -> Result<VerificationReport> {
    verify_with_options(
        intent,
        VerifyOptions {
            mode,
            capture,
            service_probe: None,
        },
    )
}

pub fn verify_with_options(
    intent: &SetupIntent,
    opts: VerifyOptions,
) -> Result<VerificationReport> {
    match opts.mode {
        VerificationMode::ProductCapture => verify_product(intent, opts),
        VerificationMode::DirectCoreTest => verify_direct(intent),
    }
}

fn verify_product(intent: &SetupIntent, opts: VerifyOptions) -> Result<VerificationReport> {
    let mut steps = Vec::new();
    let project = &intent.project;
    let suite = SuitePaths::discover();
    let suite_cli = suite.absolute_cli();
    let adapter = adapter_for(intent.agent);

    let resolved = match resolve_existing_project(Some(project)) {
        Ok(r) => {
            steps.push(ok_step(
                "project.initialized",
                "Project is prepared",
                format!("project_id={}", r.project_id),
            ));
            r
        }
        Err(e) => {
            steps.push(fail_step(
                "project.initialized",
                "Project is prepared",
                e.to_string(),
            ));
            return Ok(fail_report(steps, project.display().to_string()));
        }
    };

    let det = adapter.detect()?;
    steps.push(step(
        "agent.detected",
        "Coding agent is installed",
        det.detected,
        if det.detected {
            det.status_message.clone()
        } else {
            "Supported coding agent was not found".into()
        },
        None,
    ));
    if !det.detected {
        return Ok(fail_report(steps, project.display().to_string()));
    }

    let integ = adapter.inspect(project)?;
    let mcp_msg: String = if integ.mcp_present {
        "connection present".into()
    } else {
        "connection configuration missing".into()
    };
    steps.push(step(
        "agent.mcp",
        "Agent connection is configured",
        integ.mcp_present,
        mcp_msg,
        None,
    ));
    let hooks_msg: String = if integ.hooks_present {
        "hooks present".into()
    } else {
        "capture hooks missing".into()
    };
    steps.push(step(
        "agent.hooks",
        "Capture hooks are configured",
        integ.hooks_present,
        hooks_msg,
        None,
    ));
    if !integ.mcp_present || !integ.hooks_present {
        return Ok(fail_report(steps, project.display().to_string()));
    }

    // Strict suite CLI identity: must match SuitePaths::absolute_cli (not any moraine on disk).
    let cli_ok = integ
        .absolute_cli
        .as_ref()
        .map(|c| {
            let p = Path::new(c);
            p.is_absolute()
                && is_moraine_cli_binary(p)
                && p.is_file()
                && paths_equal(c, &suite_cli)
        })
        .unwrap_or(false);
    steps.push(step(
        "agent.absolute_cli",
        "Agent uses the suite Moraine path",
        cli_ok,
        format!(
            "configured={} suite={}",
            integ
                .absolute_cli
                .clone()
                .unwrap_or_else(|| "(missing)".into()),
            suite_cli.display()
        ),
        None,
    ));
    if !cli_ok {
        return Ok(fail_report(steps, project.display().to_string()));
    }

    let probe = opts
        .service_probe
        .unwrap_or_else(default_service_probe);
    let ready = probe.wait_ready(default_service_ready_timeout_ms());
    steps.push(step(
        "service.reachable",
        "Background capture is reachable",
        ready.ready,
        ready.message.clone(),
        Some(format!(
            "attempts={} waited_ms={}",
            ready.attempts, ready.waited_ms
        )),
    ));
    if !ready.ready {
        return Ok(fail_report(steps, project.display().to_string()));
    }

    let capture = opts
        .capture
        .unwrap_or_else(|| Arc::new(HookCodexCapture::default()));
    let session_id = format!("self-test-{}", Uuid::new_v4());
    let event_id = format!("evt-{}", Uuid::new_v4());
    let run_id = match capture.deliver_and_materialize(
        &resolved.project_root,
        &suite_cli,
        &session_id,
        &event_id,
    ) {
        Ok(id) => {
            steps.push(ok_step(
                "capture.adapter_event",
                "Created a test capture event",
                format!("session={session_id} event_id={event_id} run_id={id}"),
            ));
            id
        }
        Err(e) => {
            steps.push(fail_step(
                "capture.adapter_event",
                "Created a test capture event",
                e,
            ));
            return Ok(fail_report(steps, project.display().to_string()));
        }
    };

    // Re-validate binding: discovery + session must agree for this run_id.
    let runs = list_run_summaries(&resolved.project_root, resolved.project_id);
    let found = runs.iter().any(|r| r.run_id == run_id);
    let session_bound = find_session_run(&resolved.project_root, &session_id)
        .ok()
        .flatten()
        == Some(run_id);
    let bound_ok = found && session_bound;
    steps.push(step(
        "discovery.run_visible",
        "Test run is discoverable",
        bound_ok,
        if bound_ok {
            format!("found session-bound run {run_id} for event_id={event_id}")
        } else {
            format!(
                "run {run_id} not session-bound (found={found} session_bound={session_bound})"
            )
        },
        Some(format!("session={session_id} event_id={event_id}")),
    ));
    if !bound_ok {
        return Ok(fail_report(steps, project.display().to_string()));
    }

    let detail_ok = runs
        .iter()
        .find(|r| r.run_id == run_id)
        .map(|r| !r.absolute_path.is_empty() || !r.record_path.is_empty())
        .unwrap_or(false);
    steps.push(step(
        "discovery.run_readable",
        "Desktop can read the test run",
        detail_ok,
        "record path present",
        None,
    ));
    if !detail_ok {
        return Ok(fail_report(steps, project.display().to_string()));
    }

    let all_passed = steps.iter().all(|s| s.passed);
    Ok(VerificationReport {
        ok: all_passed,
        readiness: if all_passed {
            Readiness::Ready
        } else {
            Readiness::Failed
        },
        steps,
        run_id: Some(run_id.to_string()),
        project_path: Some(resolved.project_root.display().to_string()),
        user_message: if all_passed {
            "Moraine is ready — capture works end-to-end".into()
        } else {
            "Verification failed — Moraine is not ready yet".into()
        },
    })
}

fn verify_direct(intent: &SetupIntent) -> Result<VerificationReport> {
    let mut steps = Vec::new();
    let project = &intent.project;

    let resolved = match resolve_existing_project(Some(project)) {
        Ok(r) => {
            steps.push(ok_step(
                "project.initialized",
                "Project is prepared",
                format!("project_id={}", r.project_id),
            ));
            r
        }
        Err(e) => {
            steps.push(fail_step(
                "project.initialized",
                "Project is prepared",
                e.to_string(),
            ));
            return Ok(VerificationReport {
                ok: false,
                readiness: Readiness::Failed,
                steps,
                run_id: None,
                project_path: Some(project.display().to_string()),
                user_message: "Direct verification failed".into(),
            });
        }
    };

    let adapter = adapter_for(intent.agent);
    let integ = adapter.inspect(project)?;
    let agent_ok = integ.mcp_present && integ.hooks_present;
    steps.push(step(
        "agent.configured",
        "Agent configuration present",
        agent_ok,
        integ.details.join("; "),
        None,
    ));
    if !agent_ok {
        return Ok(VerificationReport {
            ok: false,
            readiness: Readiness::Failed,
            steps,
            run_id: None,
            project_path: Some(project.display().to_string()),
            user_message: "Direct verification failed — agent not fully configured".into(),
        });
    }

    let session_id = format!("direct-test-{}", Uuid::new_v4());
    let event_id = format!("direct-evt-{}", Uuid::new_v4());
    let run_id = match direct_core_capture_with_marker(&resolved.project_root, &session_id, &event_id)
    {
        Ok(id) => {
            steps.push(ok_step(
                "capture.direct_core",
                "Direct test capture created",
                format!("run_id={id}"),
            ));
            id
        }
        Err(e) => {
            steps.push(fail_step(
                "capture.direct_core",
                "Direct test capture created",
                e,
            ));
            return Ok(VerificationReport {
                ok: false,
                readiness: Readiness::Failed,
                steps,
                run_id: None,
                project_path: Some(project.display().to_string()),
                user_message: "Direct verification failed".into(),
            });
        }
    };

    let runs = list_run_summaries(&resolved.project_root, resolved.project_id);
    let found = runs.iter().any(|r| r.run_id == run_id);
    steps.push(step(
        "discovery.run_visible",
        "Test run is discoverable",
        found,
        format!("run {run_id}"),
        None,
    ));

    let all = steps.iter().all(|s| s.passed);
    Ok(VerificationReport {
        ok: all,
        readiness: if all {
            Readiness::DirectVerified
        } else {
            Readiness::Failed
        },
        steps,
        run_id: Some(run_id.to_string()),
        project_path: Some(resolved.project_root.display().to_string()),
        user_message: if all {
            "Direct verification passed (development path — not product Ready)".into()
        } else {
            "Direct verification failed".into()
        },
    })
}

fn direct_core_capture_with_marker(
    project: &Path,
    session_id: &str,
    event_id: &str,
) -> std::result::Result<Uuid, String> {
    let objective = format!("Moraine self-test event_id={event_id}");
    session_observe(SessionObserveRequest {
        session_id: session_id.to_string(),
        integration: "codex".into(),
        project: Some(project.to_path_buf()),
        source: "self_test".into(),
        initial_task: Some(objective.clone()),
        ended: false,
        confine_existing_project: true,
    })
    .map_err(|e| e.to_string())?;

    provisional_run_ensure(ProvisionalRunRequest {
        session_id: session_id.to_string(),
        project: Some(project.to_path_buf()),
        objective: Some(objective.clone()),
        idempotency_key: Some(format!("prov-{event_id}")),
    })
    .map_err(|e| e.to_string())?;

    let confirmed = run_start(RunStartRequest {
        objective,
        idempotency_key: format!("confirm-{event_id}"),
        project: Some(project.to_path_buf()),
        session_id: Some(session_id.to_string()),
    })
    .map_err(|e| e.to_string())?;
    Ok(confirmed.run_id)
}

fn find_session_run(project: &Path, session_id: &str) -> moraine_core::Result<Option<Uuid>> {
    let sessions = project.join(".moraine/sessions");
    if !sessions.is_dir() {
        return Ok(None);
    }
    for ent in std::fs::read_dir(&sessions)? {
        let ent = ent?;
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&p)?;
        // Require external session id match, not mere substring of UUID-like noise.
        if !raw.contains(&format!("\"{session_id}\""))
            && !raw.contains(session_id)
        {
            continue;
        }
        // Prefer structured parse
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            let ext = v
                .get("externalSessionId")
                .or_else(|| v.get("external_session_id"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let key = v
                .get("sessionKey")
                .or_else(|| v.get("session_key"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if ext != session_id && !key.ends_with(session_id) && !key.contains(session_id) {
                continue;
            }
            if let Some(id) = v
                .get("activeProvisionalRunId")
                .or_else(|| v.get("captureActiveRunId"))
                .and_then(|x| x.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                return Ok(Some(id));
            }
            if let Some(arr) = v.get("runIds").and_then(|a| a.as_array()) {
                if let Some(id) = arr
                    .last()
                    .and_then(|x| x.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                {
                    return Ok(Some(id));
                }
            }
        }
    }
    Ok(None)
}

fn invoke_hook_codex(
    cli: &Path,
    project: &Path,
    session_id: &str,
    event_id: &str,
    hook_event: &str,
    prompt: Option<&str>,
) -> std::result::Result<(), String> {
    let mut payload = json!({
        "hook_event_name": hook_event,
        "session_id": session_id,
        "event_id": event_id,
        "cwd": project.display().to_string(),
    });
    if let Some(p) = prompt {
        payload["prompt"] = json!(p);
    }
    let mut child = Command::new(cli)
        .arg("hook-codex")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn hook-codex: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let raw = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        stdin.write_all(&raw).map_err(|e| e.to_string())?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("wait hook-codex: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "hook-codex exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn is_moraine_cli_binary(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()) == Some("moraine")
}

fn paths_equal(a: &str, b: &Path) -> bool {
    let pa = Path::new(a);
    if let (Ok(ca), Ok(cb)) = (std::fs::canonicalize(pa), std::fs::canonicalize(b)) {
        return ca == cb;
    }
    pa == b
}

fn step(
    id: &str,
    product_label: &str,
    passed: bool,
    message: impl Into<String>,
    technical: Option<String>,
) -> VerificationStep {
    VerificationStep {
        id: id.into(),
        product_label: product_label.into(),
        passed,
        message: message.into(),
        technical_detail: technical,
    }
}

fn ok_step(id: &str, label: &str, message: impl Into<String>) -> VerificationStep {
    step(id, label, true, message, None)
}

fn fail_step(id: &str, label: &str, message: impl Into<String>) -> VerificationStep {
    step(id, label, false, message, None)
}

fn fail_report(steps: Vec<VerificationStep>, project: String) -> VerificationReport {
    VerificationReport {
        ok: false,
        readiness: Readiness::Failed,
        steps,
        run_id: None,
        project_path: Some(project),
        user_message: "Verification failed — Moraine is not ready yet".into(),
    }
}
