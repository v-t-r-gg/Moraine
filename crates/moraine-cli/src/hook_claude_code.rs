//! Claude Code lifecycle hook adapter: stdin JSON → Moraine capture IPC / spool.
//!
//! Privacy: full prompts and assistant messages are not persisted by default.
//! Session IDs are namespaced as `claude-code:<raw>` so they cannot collide with Codex.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use moraine_platform::CaptureEndpoint;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::capture::{self, CaptureDelivery};

const MAX_EVENT_BYTES: usize = 1024 * 1024;
const INTEGRATION: &str = "claude-code";

/// Read a Claude Code hook payload from stdin, map to a Moraine mechanical event, deliver.
pub fn run_hook_claude_code(
    socket: Option<PathBuf>,
    named_pipe: Option<String>,
    spool_dir: Option<PathBuf>,
) -> Result<i32> {
    let endpoint = select_capture_endpoint(socket, named_pipe)?;
    let mut raw = Vec::new();
    // Bound allocation: stop after MAX+1 bytes so oversized input fails closed.
    let mut stdin = io::stdin().lock();
    let mut buf = [0u8; 8192];
    loop {
        let n = stdin
            .read(&mut buf)
            .context("read Claude Code hook stdin")?;
        if n == 0 {
            break;
        }
        if raw.len() + n > MAX_EVENT_BYTES {
            anyhow::bail!("hook event exceeds {MAX_EVENT_BYTES} bytes");
        }
        raw.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&raw);
    let payload: Value = if text.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(text.trim()).context("parse Claude Code hook JSON")?
    };

    let Some(event) = map_claude_code_hook(&payload)? else {
        // Unhandled event kinds: succeed quietly so Claude Code is not disrupted.
        return Ok(0);
    };

    let body = serde_json::to_vec(&event)?;
    if body.len() > MAX_EVENT_BYTES {
        anyhow::bail!("mapped hook event exceeds {MAX_EVENT_BYTES} bytes");
    }
    let spool = spool_dir.unwrap_or_else(default_spool_dir);
    handle_delivery(capture::deliver(&endpoint, &body), &endpoint, &spool, &body)?;
    Ok(0)
}

fn handle_delivery(
    delivery: CaptureDelivery,
    endpoint: &CaptureEndpoint,
    spool: &Path,
    body: &[u8],
) -> Result<()> {
    match delivery {
        CaptureDelivery::Delivered => {}
        CaptureDelivery::Unavailable => {
            write_spooled(spool, body)?;
        }
        CaptureDelivery::AccessDenied => {
            write_spooled(spool, body)?;
            eprintln!(
                "{}",
                json!({
                    "ok": false,
                    "code": "capture_access_denied",
                    "operation": "capture_delivery",
                    "endpoint": endpoint,
                    "fallback": "spool",
                })
            );
        }
        CaptureDelivery::Unsupported => {
            eprintln!(
                "{}",
                json!({
                    "ok": false,
                    "code": "unsupported_platform",
                    "operation": "capture_delivery",
                    "endpoint": endpoint,
                })
            );
        }
    }
    Ok(())
}

fn select_capture_endpoint(
    socket: Option<PathBuf>,
    named_pipe: Option<String>,
) -> Result<CaptureEndpoint> {
    if socket.is_some() && named_pipe.is_some() {
        anyhow::bail!("--socket and --named-pipe are mutually exclusive");
    }
    if let Some(path) = socket {
        if moraine_platform::HostPlatform::current() != moraine_platform::HostPlatform::Linux {
            anyhow::bail!("--socket is supported only on Linux");
        }
        return Ok(CaptureEndpoint::UnixSocket(path));
    }
    if let Some(name) = named_pipe {
        if moraine_platform::HostPlatform::current() != moraine_platform::HostPlatform::Windows {
            anyhow::bail!("--named-pipe is supported only on Windows");
        }
        return Ok(CaptureEndpoint::WindowsNamedPipe(name));
    }
    Ok(moraine_platform::RuntimeLayout::try_discover()?.capture_endpoint)
}

fn default_spool_dir() -> PathBuf {
    std::env::var_os("MORAINE_SPOOL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| moraine_platform::RuntimeLayout::discover().spool_dir)
}

/// Namespace Claude Code session IDs so they cannot collide with Codex.
pub fn namespace_claude_session(raw: &str) -> String {
    if raw.starts_with("claude-code:") {
        raw.to_string()
    } else {
        format!("claude-code:{raw}")
    }
}

pub fn map_claude_code_hook(payload: &Value) -> Result<Option<Value>> {
    let hook_event = payload
        .get("hook_event_name")
        .or_else(|| payload.get("hookEventName"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let raw_session = payload
        .get("session_id")
        .or_else(|| payload.get("sessionId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if raw_session.is_empty() {
        return Ok(None);
    }
    let session_id = namespace_claude_session(&raw_session);

    let cwd = payload
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Never follow transcript_path — observational only.
    let _transcript = payload
        .get("transcript_path")
        .or_else(|| payload.get("transcriptPath"));

    let mut inner = json!({});
    let kind = match hook_event {
        "SessionStart" => {
            if let Some(source) = payload.get("source").and_then(|v| v.as_str()) {
                inner["source"] = json!(source);
            } else {
                inner["source"] = json!("startup");
            }
            "session_start"
        }
        "UserPromptSubmit" => {
            let prompt = payload
                .get("prompt")
                .or_else(|| payload.get("user_prompt"))
                .and_then(|v| v.as_str());
            if let Some(p) = prompt {
                inner["promptPresent"] = json!(true);
                inner["promptCharCount"] = json!(p.chars().count());
                // Self-test / explicit Moraine markers only — not full user prompts.
                if p.starts_with("Moraine self-test") {
                    let bounded: String = p
                        .chars()
                        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                        .take(500)
                        .collect();
                    inner["objectiveHint"] = json!(bounded.trim());
                }
            } else {
                inner["promptPresent"] = json!(false);
            }
            "user_prompt"
        }
        "Stop" => {
            // Do not store last_assistant_message.
            if payload
                .get("stop_hook_active")
                .or_else(|| payload.get("stopHookActive"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                inner["stopHookActive"] = json!(true);
            }
            "session_stop"
        }
        _ => return Ok(None),
    };

    let event_id = payload
        .get("event_id")
        .or_else(|| payload.get("eventId"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| stable_event_id(hook_event, &session_id, payload));

    Ok(Some(json!({
        "schemaVersion": 1,
        "eventId": event_id,
        "kind": kind,
        "sessionId": session_id,
        "project": cwd,
        "integration": INTEGRATION,
        "occurredAt": chrono::Utc::now().to_rfc3339(),
        "payload": inner,
    })))
}

fn stable_event_id(hook_event: &str, session_id: &str, payload: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(hook_event.as_bytes());
    hasher.update(b"|");
    hasher.update(session_id.as_bytes());
    hasher.update(b"|");
    if let Some(s) = payload.get("source").and_then(|v| v.as_str()) {
        hasher.update(s.as_bytes());
    }
    if let Some(p) = payload.get("prompt").and_then(|v| v.as_str()) {
        // Hash only — never embed the prompt body in the event id payload.
        hasher.update(p.as_bytes());
    }
    format!("claude-code-{}", &hex::encode(hasher.finalize())[..24])
}

fn write_spooled(spool_dir: &Path, buf: &[u8]) -> Result<PathBuf> {
    if buf.len() > MAX_EVENT_BYTES {
        anyhow::bail!("hook event exceeds {MAX_EVENT_BYTES} bytes");
    }
    std::fs::create_dir_all(spool_dir)?;
    set_private_dir(spool_dir);
    for sub in ["processed", "failed", "seen", "quarantine"] {
        let dir = spool_dir.join(sub);
        std::fs::create_dir_all(&dir)?;
        set_private_dir(&dir);
    }
    let file_stem = match serde_json::from_slice::<Value>(buf) {
        Ok(v) => {
            if let Some(id) = v.get("eventId").and_then(|x| x.as_str()) {
                let id = id.trim();
                if !id.is_empty() {
                    let safe: String = id
                        .chars()
                        .map(|c| {
                            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                                c
                            } else {
                                '_'
                            }
                        })
                        .take(80)
                        .collect();
                    format!("event-id-{safe}")
                } else {
                    format!("event-{}", uuid::Uuid::new_v4())
                }
            } else {
                format!("event-{}", uuid::Uuid::new_v4())
            }
        }
        Err(_) => format!("event-{}", uuid::Uuid::new_v4()),
    };
    let path = spool_dir.join(format!("{file_stem}.json"));
    std::fs::write(&path, buf)?;
    Ok(path)
}

#[cfg(unix)]
fn set_private_dir(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn set_private_dir(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaces_session_ids() {
        assert_eq!(namespace_claude_session("abc"), "claude-code:abc");
        assert_eq!(
            namespace_claude_session("claude-code:abc"),
            "claude-code:abc"
        );
    }

    #[test]
    fn maps_session_start_without_optional_fields() {
        let payload = json!({
            "hook_event_name": "SessionStart",
            "session_id": "s1",
            "cwd": "/tmp/p"
        });
        let event = map_claude_code_hook(&payload).unwrap().unwrap();
        assert_eq!(event["kind"], "session_start");
        assert_eq!(event["sessionId"], "claude-code:s1");
        assert_eq!(event["integration"], "claude-code");
    }

    #[test]
    fn does_not_persist_full_prompt_by_default() {
        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "s1",
            "cwd": "/tmp/p",
            "prompt": "secret user request about payroll"
        });
        let event = map_claude_code_hook(&payload).unwrap().unwrap();
        let p = &event["payload"];
        assert_eq!(p["promptPresent"], true);
        assert!(p.get("prompt").is_none());
        assert!(p.get("objectiveHint").is_none());
    }

    #[test]
    fn self_test_prompt_becomes_objective_hint() {
        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "s1",
            "cwd": "/tmp/p",
            "prompt": "Moraine self-test verification_id=abc"
        });
        let event = map_claude_code_hook(&payload).unwrap().unwrap();
        assert_eq!(
            event["payload"]["objectiveHint"],
            "Moraine self-test verification_id=abc"
        );
    }

    #[test]
    fn stop_does_not_store_assistant_message() {
        let payload = json!({
            "hook_event_name": "Stop",
            "session_id": "s1",
            "last_assistant_message": "should not be stored"
        });
        let event = map_claude_code_hook(&payload).unwrap().unwrap();
        assert!(event["payload"].get("last_assistant_message").is_none());
        assert!(event["payload"].get("lastAssistantMessage").is_none());
    }

    #[test]
    fn missing_session_is_ignored() {
        let payload = json!({ "hook_event_name": "SessionStart" });
        assert!(map_claude_code_hook(&payload).unwrap().is_none());
    }
}
