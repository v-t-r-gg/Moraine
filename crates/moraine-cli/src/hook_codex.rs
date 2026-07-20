//! Codex lifecycle hook adapter: stdin JSON → Moraine local service IPC / spool.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Read a Codex hook payload from stdin, map to a Moraine mechanical event, deliver.
pub fn run_hook_codex(socket: Option<PathBuf>, spool_dir: Option<PathBuf>) -> Result<i32> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .context("read Codex hook stdin")?;
    let payload: Value = if raw.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&raw).context("parse Codex hook JSON")?
    };

    let Some(event) = map_codex_hook(&payload)? else {
        // Unhandled event kinds: succeed quietly so Codex is not disrupted.
        return Ok(0);
    };

    let body = serde_json::to_vec(&event)?;
    let socket_path = socket.unwrap_or_else(default_socket_path);
    let spool = spool_dir.unwrap_or_else(default_spool_dir);

    if deliver_unix(&socket_path, &body).is_ok() {
        return Ok(0);
    }

    // Service unavailable: spool locally and exit 0 so the agent continues.
    std::fs::create_dir_all(&spool).ok();
    std::fs::create_dir_all(spool.join("processed")).ok();
    std::fs::create_dir_all(spool.join("failed")).ok();
    write_spooled(&spool, &body)?;
    Ok(0)
}

fn default_socket_path() -> PathBuf {
    std::env::var_os("MORAINE_SOCKET")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(|d| PathBuf::from(d).join("moraine-service.sock"))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("moraine-service.sock"))
}

fn default_spool_dir() -> PathBuf {
    std::env::var_os("MORAINE_SPOOL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("moraine-service/spool")
        })
}

fn map_codex_hook(payload: &Value) -> Result<Option<Value>> {
    let hook_event = payload
        .get("hook_event_name")
        .or_else(|| payload.get("hookEventName"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let session_id = payload
        .get("session_id")
        .or_else(|| payload.get("sessionId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if session_id.is_empty() {
        // Cannot bind without a session id.
        return Ok(None);
    }

    let cwd = payload
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let kind = match hook_event {
        "SessionStart" => "session_start",
        "UserPromptSubmit" => "user_prompt",
        "Stop" => "session_stop",
        _ => return Ok(None),
    };

    let mut inner = json!({});
    if hook_event == "SessionStart" {
        if let Some(source) = payload.get("source").and_then(|v| v.as_str()) {
            inner["source"] = json!(source);
        } else {
            inner["source"] = json!("startup");
        }
    }
    if hook_event == "UserPromptSubmit" {
        let prompt = payload
            .get("prompt")
            .or_else(|| payload.get("user_prompt"))
            .or_else(|| payload.pointer("/hookSpecificOutput/prompt"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // Bound and flatten newlines for objective safety.
        let bounded: String = prompt
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .take(2000)
            .collect();
        inner["prompt"] = json!(bounded.trim());
    }

    let event_id = stable_event_id(hook_event, &session_id, payload);

    Ok(Some(json!({
        "schemaVersion": 1,
        "eventId": event_id,
        "kind": kind,
        "sessionId": session_id,
        "project": cwd,
        "integration": "codex",
        "occurredAt": chrono::Utc::now().to_rfc3339(),
        "payload": inner,
    })))
}

fn stable_event_id(hook_event: &str, session_id: &str, payload: &Value) -> String {
    if let Some(id) = payload
        .get("event_id")
        .or_else(|| payload.get("eventId"))
        .and_then(|v| v.as_str())
    {
        if !id.trim().is_empty() {
            return id.to_string();
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(hook_event.as_bytes());
    hasher.update(b"|");
    hasher.update(session_id.as_bytes());
    hasher.update(b"|");
    if let Some(s) = payload.get("source").and_then(|v| v.as_str()) {
        hasher.update(s.as_bytes());
    }
    if let Some(t) = payload
        .get("triggered_at")
        .or_else(|| payload.get("triggeredAt"))
    {
        hasher.update(t.to_string().as_bytes());
    } else if let Some(p) = payload.get("prompt").and_then(|v| v.as_str()) {
        hasher.update(p.as_bytes());
    }
    format!("codex-{}", &hex::encode(hasher.finalize())[..24])
}

fn deliver_unix(socket_path: &Path, body: &[u8]) -> Result<()> {
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connect {}", socket_path.display()))?;
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    stream.write_all(body)?;
    let _ = stream.flush();
    Ok(())
}

fn write_spooled(spool_dir: &Path, buf: &[u8]) -> Result<PathBuf> {
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
                        .take(128)
                        .collect();
                    format!("event-id-{safe}")
                } else {
                    content_hash_name(buf)
                }
            } else {
                content_hash_name(buf)
            }
        }
        Err(_) => content_hash_name(buf),
    };
    let file_name = format!("{file_stem}.json");
    let path = spool_dir.join(&file_name);
    let processed = spool_dir.join("processed").join(&file_name);
    let failed = spool_dir.join("failed").join(&file_name);
    if path.exists() || processed.exists() || failed.exists() {
        return Ok(path);
    }
    // Atomic-ish: write temp then rename.
    let tmp = spool_dir.join(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, buf)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

fn content_hash_name(buf: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(buf);
    format!("event-{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_session_start() {
        let payload = json!({
            "hook_event_name": "SessionStart",
            "session_id": "abc",
            "cwd": "/tmp/proj",
            "source": "startup",
        });
        let ev = map_codex_hook(&payload).unwrap().unwrap();
        assert_eq!(ev["kind"], "session_start");
        assert_eq!(ev["sessionId"], "abc");
        assert_eq!(ev["integration"], "codex");
    }

    #[test]
    fn maps_user_prompt() {
        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "abc",
            "cwd": "/tmp/proj",
            "prompt": "Fix the bug\nplease",
        });
        let ev = map_codex_hook(&payload).unwrap().unwrap();
        assert_eq!(ev["kind"], "user_prompt");
        assert_eq!(ev["payload"]["prompt"], "Fix the bug please");
    }

    #[test]
    fn ignores_unknown() {
        let payload = json!({
            "hook_event_name": "PreToolUse",
            "session_id": "abc",
        });
        assert!(map_codex_hook(&payload).unwrap().is_none());
    }
}
