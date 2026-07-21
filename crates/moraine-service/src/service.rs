use anyhow::Result;
use moraine_core::{
    provisional_run_ensure, run_checkpoint, run_ready, run_resume, run_start, session_observe,
    CheckpointInput, ProvisionalRunRequest, RunStartRequest, SessionObserveRequest,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MORAINE_DIR: &str = ".moraine";
/// Maximum accepted event payload (bytes).
pub const MAX_EVENT_BYTES: usize = 1024 * 1024;
/// Soft cap on pending event files in the spool root.
pub const MAX_PENDING_EVENTS: usize = 1000;

/// Durable processed-event markers under `spool/seen/`. Survive service restart.
fn seen_dir(spool_dir: &Path) -> PathBuf {
    spool_dir.join("seen")
}

fn seen_marker_path(spool_dir: &Path, event_id: &str) -> PathBuf {
    seen_dir(spool_dir).join(format!("{}.seen", sanitize_id(event_id)))
}

pub fn event_already_seen(spool_dir: &Path, event_id: &str) -> bool {
    if event_id.trim().is_empty() {
        return false;
    }
    seen_marker_path(spool_dir, event_id).exists()
}

pub fn mark_event_seen(spool_dir: &Path, event_id: &str) -> Result<()> {
    let dir = seen_dir(spool_dir);
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let path = seen_marker_path(spool_dir, event_id);
    let tmp = dir.join(format!(".{}.tmp", sanitize_id(event_id)));
    std::fs::write(&tmp, b"1")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

pub async fn write_spooled_payload(spool_dir: &Path, buf: &[u8]) -> Result<std::path::PathBuf> {
    if buf.len() > MAX_EVENT_BYTES {
        return Err(anyhow::anyhow!(
            "event exceeds MAX_EVENT_BYTES ({MAX_EVENT_BYTES})"
        ));
    }
    std::fs::create_dir_all(spool_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(spool_dir, std::fs::Permissions::from_mode(0o700));
    }
    for sub in ["processed", "failed", "seen", "quarantine"] {
        let p = spool_dir.join(sub);
        std::fs::create_dir_all(&p)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700));
        }
    }

    let event_id = extract_event_id(buf);
    if let Some(ref id) = event_id {
        if event_already_seen(spool_dir, id) {
            // Durable dedupe: already applied; do not re-queue.
            return Ok(seen_marker_path(spool_dir, id));
        }
    }

    let file_stem = match &event_id {
        Some(id) if !id.is_empty() => format!("event-id-{}", sanitize_id(id)),
        _ => content_hash_name(buf),
    };
    let file_name = format!("{file_stem}.json");
    let path = spool_dir.join(&file_name);
    let processed = spool_dir.join("processed").join(&file_name);
    let failed = spool_dir.join("failed").join(&file_name);
    let quarantine = spool_dir.join("quarantine").join(&file_name);
    if path.exists() || processed.exists() || failed.exists() || quarantine.exists() {
        return Ok(path);
    }
    // Atomic create: temp then rename.
    let tmp = spool_dir.join(format!(".{file_name}.tmp"));
    tokio::fs::write(&tmp, buf).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    tokio::fs::rename(&tmp, &path).await?;
    Ok(path)
}

fn extract_event_id(buf: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(buf).ok()?;
    v.get("eventId")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn content_hash_name(buf: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(buf);
    format!("event-{}", hex::encode(hasher.finalize()))
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(128)
        .collect()
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MechanicalEvent {
    pub schema_version: u32,
    pub event_id: String,
    pub kind: String,
    pub session_id: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub integration: Option<String>,
    #[serde(default)]
    pub occurred_at: Option<String>,
    #[serde(default)]
    pub payload: Option<Value>,
}

/// Legacy MCP-shaped spool event (compatibility).
#[derive(serde::Deserialize, Debug)]
pub struct Event {
    pub kind: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub expected_hash: Option<String>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

pub async fn process_spool_file(
    path: &std::path::Path,
    processed_dir: &Path,
    failed_dir: &Path,
) -> Result<()> {
    let spool_dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("spool file has no parent"))?;
    let data = tokio::fs::read(path).await?;
    if data.len() > MAX_EVENT_BYTES {
        quarantine_file(path, spool_dir).await?;
        return Err(anyhow::anyhow!("event too large; quarantined"));
    }

    let value: Value = match serde_json::from_slice(&data) {
        Ok(v) => v,
        Err(e) => {
            quarantine_file(path, spool_dir).await?;
            return Err(anyhow::anyhow!("invalid json: {e}; quarantined"));
        }
    };

    let event_id = value
        .get("eventId")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());

    // Restart-safe dedupe: if already applied, acknowledge without re-running side effects.
    if let Some(ref id) = event_id {
        if event_already_seen(spool_dir, id) {
            let dest = processed_dir.join(path.file_name().unwrap());
            let _ = tokio::fs::rename(path, &dest).await;
            return Ok(());
        }
    }

    let res = if value.get("schemaVersion").is_some() || value.get("eventId").is_some() {
        process_mechanical_value(&value)
    } else {
        process_legacy_value(&value)
    };

    match res {
        Ok(_) => {
            if let Some(ref id) = event_id {
                mark_event_seen(spool_dir, id)?;
            }
            let dest = processed_dir.join(path.file_name().unwrap());
            tokio::fs::rename(path, &dest).await?;
            Ok(())
        }
        Err(e) => {
            // Permanent failure → failed/ (not retried). Corrupt → quarantine.
            let dest = failed_dir.join(path.file_name().unwrap());
            let _ = tokio::fs::rename(path, &dest).await;
            Err(e)
        }
    }
}

async fn quarantine_file(path: &Path, spool_dir: &Path) -> Result<()> {
    let q = spool_dir.join("quarantine");
    tokio::fs::create_dir_all(&q).await.ok();
    let dest = q.join(path.file_name().unwrap());
    let _ = tokio::fs::rename(path, &dest).await;
    Ok(())
}

fn process_mechanical_value(value: &Value) -> Result<()> {
    let event: MechanicalEvent = serde_json::from_value(value.clone())
        .map_err(|e| anyhow::anyhow!("invalid mechanical event: {e}"))?;
    validate_mechanical(&event)?;

    let project = event.project.as_deref().map(PathBuf::from);
    let integration = event.integration.clone().unwrap_or_else(|| "codex".into());
    let kind = event.kind.as_str();

    match kind {
        "session_start" => {
            let source = event
                .payload
                .as_ref()
                .and_then(|p| p.get("source"))
                .and_then(|s| s.as_str())
                .unwrap_or("startup");
            session_observe(SessionObserveRequest {
                session_id: event.session_id.clone(),
                integration,
                project,
                source: source.into(),
                initial_task: None,
                ended: false,
                confine_existing_project: true,
            })
            .map(|_| ())
            .map_err(core_err)
        }
        "user_prompt" => {
            let prompt = event
                .payload
                .as_ref()
                .and_then(|p| {
                    p.get("prompt")
                        .or_else(|| p.get("text"))
                        .or_else(|| p.get("initialTask"))
                })
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let observed = session_observe(SessionObserveRequest {
                session_id: event.session_id.clone(),
                integration: integration.clone(),
                project: project.clone(),
                source: "user_prompt".into(),
                initial_task: prompt.clone(),
                ended: false,
                confine_existing_project: true,
            })
            .map_err(core_err)?;
            // First substantive prompt only → provisional. Later prompts are context.
            if observed.should_ensure_provisional {
                provisional_run_ensure(ProvisionalRunRequest {
                    session_id: event.session_id,
                    project,
                    objective: prompt,
                    idempotency_key: None,
                })
                .map(|_| ())
                .map_err(core_err)?;
            }
            Ok(())
        }
        "session_stop" => {
            // Envelope close only — never mutates run lifecycle.
            session_observe(SessionObserveRequest {
                session_id: event.session_id,
                integration,
                project,
                source: "stop".into(),
                initial_task: None,
                ended: true,
                confine_existing_project: true,
            })
            .map(|_| ())
            .map_err(core_err)
        }
        "command_started" | "command_finished" | "tool_started" | "tool_finished"
        | "artifact_observed" => {
            let payload_obj = event.payload.as_ref();
            let tool = payload_obj
                .and_then(|p| p.get("tool"))
                .and_then(|s| s.as_str())
                .unwrap_or("unknown")
                .to_string();
            let command = payload_obj
                .and_then(|p| p.get("command"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let working_directory = payload_obj
                .and_then(|p| p.get("workingDirectory"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let call_id = payload_obj
                .and_then(|p| p.get("callId"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let exit_code = payload_obj
                .and_then(|p| p.get("exitCode"))
                .and_then(|v| v.as_i64())
                .map(|n| n as i32);
            let output_text = payload_obj
                .and_then(|p| p.get("output"))
                .and_then(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if v.is_object() || v.is_array() {
                        Some(v.to_string())
                    } else {
                        None
                    }
                });

            let observed = session_observe(SessionObserveRequest {
                session_id: event.session_id.clone(),
                integration: integration.clone(),
                project,
                source: kind.to_string(),
                initial_task: None,
                ended: false,
                confine_existing_project: true,
            })
            .map_err(core_err)?;

            moraine_core::record_mechanical_evidence(
                &observed.project_root,
                moraine_core::MechanicalEvidenceRequest {
                    session_key: observed.session_key,
                    integration: Some(integration),
                    event_kind: kind.to_string(),
                    tool,
                    command,
                    working_directory,
                    call_id,
                    started_at: None,
                    finished_at: None,
                    exit_code,
                    output_text,
                    event_id: event.event_id,
                },
            )
            .map(|_| ())
            .map_err(core_err)
        }
        other => Err(anyhow::anyhow!(
            "unsupported mechanical event kind: {other}"
        )),
    }
}

fn process_legacy_value(value: &Value) -> Result<()> {
    let event: Event = serde_json::from_value(value.clone())
        .map_err(|e| anyhow::anyhow!("invalid legacy event: {e}"))?;
    validate_event(&event)?;

    let kind = event.kind.as_str();
    match kind {
        "start" => {
            let objective = event.objective.as_deref().unwrap_or("");
            let idempotency = event.idempotency_key.as_deref().unwrap_or("default");
            let project = event.project.as_deref().map(PathBuf::from);
            let req = RunStartRequest {
                objective: objective.to_string(),
                idempotency_key: idempotency.to_string(),
                project,
                session_id: event.session_id.clone(),
            };
            run_start(req).map(|_| ()).map_err(core_err)
        }
        "checkpoint" => {
            let run_id = event
                .run_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("missing run_id"))?;
            let run_uuid = Uuid::parse_str(run_id)?;
            let expected = event.expected_hash.as_deref().unwrap_or("");
            let idempotency = event.idempotency_key.as_deref().unwrap_or("default");
            let input_val = event.input.clone().unwrap_or(Value::Null);
            let input: CheckpointInput = serde_json::from_value(input_val)
                .map_err(|e| anyhow::anyhow!("invalid checkpoint input: {}", e))?;
            run_checkpoint(None, run_uuid, expected, idempotency, input)
                .map(|_| ())
                .map_err(core_err)
        }
        "ready" => {
            let run_id = event
                .run_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("missing run_id"))?;
            let run_uuid = Uuid::parse_str(run_id)?;
            let expected = event.expected_hash.as_deref().unwrap_or("");
            let idempotency = event.idempotency_key.as_deref().unwrap_or("default");
            let summary = event.summary.clone();
            run_ready(None, run_uuid, expected, idempotency, summary)
                .map(|_| ())
                .map_err(core_err)
        }
        "resume" => {
            let run_id = event
                .run_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("missing run_id"))?;
            let run_uuid = Uuid::parse_str(run_id)?;
            let expected = event.expected_hash.as_deref().unwrap_or("");
            let idempotency = event.idempotency_key.as_deref().unwrap_or("default");
            let reason = event.reason.clone();
            run_resume(None, run_uuid, expected, idempotency, reason)
                .map(|_| ())
                .map_err(core_err)
        }
        other => Err(anyhow::anyhow!("unsupported event kind: {other}")),
    }
}

fn core_err(e: moraine_core::Error) -> anyhow::Error {
    anyhow::anyhow!("moraine_core error: {}", e)
}

fn validate_mechanical(ev: &MechanicalEvent) -> Result<()> {
    if ev.schema_version != 1 {
        return Err(anyhow::anyhow!(
            "unsupported mechanical schemaVersion: {}",
            ev.schema_version
        ));
    }
    if ev.event_id.trim().is_empty() {
        return Err(anyhow::anyhow!("mechanical event requires eventId"));
    }
    if ev.session_id.trim().is_empty() {
        return Err(anyhow::anyhow!("mechanical event requires sessionId"));
    }
    match ev.kind.as_str() {
        "session_start"
        | "user_prompt"
        | "session_stop"
        | "command_started"
        | "command_finished"
        | "tool_started"
        | "tool_finished"
        | "artifact_observed" => Ok(()),
        other => Err(anyhow::anyhow!(
            "unsupported mechanical event kind: {other}"
        )),
    }
}

fn validate_event(ev: &Event) -> Result<(), anyhow::Error> {
    match ev.kind.as_str() {
        "start" => {
            let obj = ev.objective.as_deref().unwrap_or("").trim();
            if obj.is_empty() {
                return Err(anyhow::anyhow!("start event requires non-empty objective"));
            }
            if ev
                .idempotency_key
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!("start event requires idempotency_key"));
            }
            Ok(())
        }
        "checkpoint" => {
            if ev
                .run_id
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!("checkpoint event requires run_id"));
            }
            if let Some(Value::Object(map)) = &ev.input {
                if let Some(v) = map.get("summary") {
                    if v.as_str().map(|s| s.trim().is_empty()).unwrap_or(true) {
                        return Err(anyhow::anyhow!(
                            "checkpoint input.summary must be a non-empty string"
                        ));
                    }
                } else {
                    return Err(anyhow::anyhow!("checkpoint input missing summary"));
                }
            } else {
                return Err(anyhow::anyhow!("checkpoint input missing or not an object"));
            }
            Ok(())
        }
        "ready" | "resume" => {
            if ev
                .run_id
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!("{} event requires run_id", ev.kind));
            }
            if ev
                .idempotency_key
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!(
                    "{} event requires idempotency_key",
                    ev.kind
                ));
            }
            Ok(())
        }
        _ => Err(anyhow::anyhow!("unsupported event kind: {}", ev.kind)),
    }
}

pub async fn rebuild_index(
    base: std::path::PathBuf,
    out_file: std::path::PathBuf,
    max_depth: usize,
) -> Result<()> {
    use serde_json::json;
    use std::fs;

    let mut projects = vec![];
    let mut stack = vec![(base.clone(), 0usize)];
    while let Some((cur, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        if cur.join(MORAINE_DIR).is_dir() {
            let proj = moraine_core::resolve_existing_project(Some(&cur)).ok();
            let runs = cur.join(MORAINE_DIR).join("runs");
            let run_count = count_run_records(&runs);
            projects.push(json!({
                "root": cur.display().to_string(),
                "run_count": run_count,
                "meta": proj.map(|m| json!({"id": m.project_id, "created": m.created}))
            }));
            continue;
        }
        if let Ok(entries) = fs::read_dir(&cur) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push((p, depth + 1));
                }
            }
        }
    }

    let doc = json!({"projects": projects, "scanned_at": chrono::Utc::now()});
    let raw = serde_json::to_vec_pretty(&doc)?;
    let tmp = out_file.with_extension("json.tmp");
    tokio::fs::write(&tmp, &raw).await?;
    tokio::fs::rename(&tmp, &out_file).await?;
    Ok(())
}

/// Count canonical run Markdown records, excluding sidecars and lock files.
pub fn count_run_records(runs_dir: &Path) -> usize {
    if !runs_dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(runs_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    let path = entry.path();
                    path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
                })
                .count()
        })
        .unwrap_or(0)
}

pub async fn spool_counts(spool_dir: &Path) -> Result<(usize, usize, usize)> {
    let pending = count_event_files(spool_dir).await?;
    let processed = count_event_files(&spool_dir.join("processed")).await?;
    let failed = count_event_files(&spool_dir.join("failed")).await?;
    Ok((pending, processed, failed))
}

async fn count_event_files(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut n = 0usize;
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Ok(Some(ent)) = entries.next_entry().await {
        let p = ent.path();
        if p.is_file()
            && p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("event-") && n.ends_with(".json"))
                .unwrap_or(false)
        {
            n += 1;
        }
    }
    Ok(n)
}

pub fn read_index_projects(spool_dir: &Path) -> Option<Value> {
    let path = spool_dir.join("index.json");
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}
