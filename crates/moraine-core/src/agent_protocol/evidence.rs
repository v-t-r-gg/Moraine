//! Mechanical evidence capture, redaction, storage, and run association.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::project::MORAINE_DIR;
use super::session::load_session;
use super::types::{EvidenceKind, EvidenceProvenance, EvidenceSummary};
use crate::atomic::{write_atomic, SidecarLock};
use crate::error::Result;
use crate::run_meta::{moraine_sidecar_path, RunMeta};

pub const EVIDENCE_DIR: &str = "evidence";
pub const MAX_COMMAND_LEN: usize = 4_096;
pub const MAX_OUTPUT_BYTES: usize = 16_384;
pub const EVIDENCE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutputMetadata {
    pub captured: bool,
    pub truncated: bool,
    pub byte_count: usize,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRecord {
    pub schema_version: u32,
    pub evidence_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    pub session_key: String,
    pub kind: EvidenceKind,
    pub provenance: EvidenceProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration: Option<String>,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MechanicalEvidenceRequest {
    pub session_key: String,
    pub integration: Option<String>,
    pub event_kind: String,
    pub tool: String,
    pub command: Option<String>,
    pub working_directory: Option<String>,
    pub call_id: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub output_text: Option<String>,
    pub event_id: String,
}

/// Redact secret-like patterns (API keys, bearer tokens, passwords) from text without regex dependencies.
pub fn redact_secrets(text: &str) -> String {
    let mut result = text.to_string();

    // 1. Redact API tokens (e.g. sk-..., ghp_..., gho_..., glpat-..., xox...)
    for prefix in &[
        "sk-", "ghp_", "gho_", "glpat-", "xoxb-", "xoxp-", "xoxa-", "xoxr-", "xoxs-",
    ] {
        let mut start_idx = 0;
        while let Some(idx) = result[start_idx..].find(prefix) {
            let real_idx = start_idx + idx;
            let tail = &result[real_idx + prefix.len()..];
            let end_len = tail
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(tail.len());
            let secret_len = prefix.len() + end_len;
            if secret_len >= 16 {
                result.replace_range(real_idx..real_idx + secret_len, "[REDACTED]");
                start_idx = real_idx + 10;
            } else {
                start_idx = real_idx + prefix.len();
            }
            if start_idx >= result.len() {
                break;
            }
        }
    }

    // 2. Redact Bearer tokens
    let mut search_pos = 0;
    while search_pos < result.len() {
        let lower = result[search_pos..].to_lowercase();
        if let Some(rel) = lower.find("bearer ") {
            let idx = search_pos + rel;
            let val_start = idx + 7;
            let tail = &result[val_start..];
            let token_len = tail
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ';')
                .unwrap_or(tail.len());
            if token_len > 0 {
                result.replace_range(val_start..val_start + token_len, "[REDACTED]");
                search_pos = val_start + 10;
            } else {
                search_pos = val_start;
            }
        } else {
            break;
        }
    }

    // 3. Redact --password, --token, --secret, --api-key, -p flags
    for flag in &["--password", "--token", "--secret", "--api-key"] {
        let mut pos = 0;
        while pos < result.len() {
            let lower = result[pos..].to_lowercase();
            if let Some(rel) = lower.find(flag) {
                let idx = pos + rel;
                let after_flag = &result[idx + flag.len()..];
                if after_flag.starts_with(' ') || after_flag.starts_with('=') {
                    let skip = if after_flag.starts_with('=') {
                        1
                    } else {
                        after_flag
                            .chars()
                            .take_while(|c| c.is_whitespace())
                            .map(|c| c.len_utf8())
                            .sum()
                    };
                    let val_start = idx + flag.len() + skip;
                    let val_tail = &result[val_start..];
                    let val_len = val_tail
                        .find(|c: char| {
                            c.is_whitespace() || c == '&' || c == ';' || c == '"' || c == '\''
                        })
                        .unwrap_or(val_tail.len());
                    if val_len > 0 {
                        result.replace_range(val_start..val_start + val_len, "[REDACTED]");
                        pos = val_start + 10;
                        continue;
                    }
                }
                pos = idx + flag.len();
            } else {
                break;
            }
        }
    }

    // 4. Redact key-value secrets: password=..., secret=..., token=..., api_key=...
    for key in &[
        "password=",
        "passwd=",
        "secret=",
        "token=",
        "api_key=",
        "access_token=",
        "authorization=",
    ] {
        let mut pos = 0;
        while pos < result.len() {
            let lower = result[pos..].to_lowercase();
            if let Some(rel) = lower.find(key) {
                let idx = pos + rel;
                let val_start = idx + key.len();
                let val_tail = &result[val_start..];
                let val_len = val_tail
                    .find(|c: char| {
                        c.is_whitespace() || c == '&' || c == ';' || c == '"' || c == '\''
                    })
                    .unwrap_or(val_tail.len());
                if val_len > 0 {
                    result.replace_range(val_start..val_start + val_len, "[REDACTED]");
                    pos = val_start + 10;
                } else {
                    pos = val_start;
                }
            } else {
                break;
            }
        }
    }

    // 5. Redact PEM private keys
    if let (Some(start), Some(end)) = (result.find("-----BEGIN "), result.find("-----END ")) {
        if let Some(end_line) = result[end..].find('\n') {
            result.replace_range(start..end + end_line, "[REDACTED_PRIVATE_KEY]");
        }
    }

    result
}

fn apply_redaction(text: &str) -> String {
    redact_secrets(text)
}

pub fn evidence_root_dir(project_root: &Path) -> PathBuf {
    project_root.join(MORAINE_DIR).join(EVIDENCE_DIR)
}

pub fn evidence_folder(project_root: &Path, run_id: Option<Uuid>) -> PathBuf {
    match run_id {
        Some(id) => evidence_root_dir(project_root).join(id.to_string()),
        None => evidence_root_dir(project_root).join("unassigned"),
    }
}

pub fn evidence_file_path(project_root: &Path, run_id: Option<Uuid>, evidence_id: Uuid) -> PathBuf {
    evidence_folder(project_root, run_id).join(format!("{evidence_id}.json"))
}

pub fn evidence_excerpt_path(
    project_root: &Path,
    run_id: Option<Uuid>,
    evidence_id: Uuid,
) -> PathBuf {
    evidence_folder(project_root, run_id).join(format!("{evidence_id}-output.txt"))
}

pub fn load_evidence_record(
    project_root: &Path,
    run_id: Option<Uuid>,
    evidence_id: Uuid,
) -> Result<Option<EvidenceRecord>> {
    let path = evidence_file_path(project_root, run_id, evidence_id);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    let rec: EvidenceRecord = serde_json::from_str(&raw)?;
    Ok(Some(rec))
}

pub fn record_mechanical_evidence(
    project_root: &Path,
    req: MechanicalEvidenceRequest,
) -> Result<EvidenceRecord> {
    let session = load_session(project_root, &req.session_key)?;
    // Active run for session: prefer capture_active_run_id, fallback to active_provisional_run_id
    let target_run_id = session
        .as_ref()
        .and_then(|s| s.capture_active_run_id.or(s.active_provisional_run_id));

    let folder = evidence_folder(project_root, target_run_id);
    fs::create_dir_all(&folder)?;

    let is_finish = matches!(
        req.event_kind.as_str(),
        "command_finished" | "tool_finished"
    );

    let (provenance, kind) = match req.event_kind.as_str() {
        "command_started" => (
            EvidenceProvenance::InvocationObserved,
            EvidenceKind::CommandResult,
        ),
        "tool_started" => (
            EvidenceProvenance::InvocationObserved,
            EvidenceKind::ToolResult,
        ),
        "command_finished" => (
            EvidenceProvenance::MoraineCaptured,
            EvidenceKind::CommandResult,
        ),
        "tool_finished" => (
            EvidenceProvenance::MoraineCaptured,
            EvidenceKind::ToolResult,
        ),
        "artifact_observed" => (EvidenceProvenance::MoraineCaptured, EvidenceKind::Artifact),
        _ => (
            EvidenceProvenance::MoraineCaptured,
            EvidenceKind::CommandResult,
        ),
    };

    // Command redaction and length bounding
    let command = req.command.as_deref().map(|cmd| {
        let redacted = apply_redaction(cmd);
        if redacted.len() > MAX_COMMAND_LEN {
            format!("{}…", &redacted[..MAX_COMMAND_LEN - 1])
        } else {
            redacted
        }
    });

    let working_directory = req.working_directory.as_deref().map(|w| w.to_string());
    let now = Utc::now();
    let started_at = req.started_at.unwrap_or(now);
    let finished_at = if is_finish {
        Some(req.finished_at.unwrap_or(now))
    } else {
        req.finished_at
    };

    // Check if there is a matching invocation by call_id in folder
    let mut existing_evidence: Option<EvidenceRecord> = None;
    if let Some(ref cid) = req.call_id {
        if !cid.trim().is_empty() {
            if let Ok(entries) = fs::read_dir(&folder) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Ok(raw) = fs::read_to_string(&path) {
                            if let Ok(rec) = serde_json::from_str::<EvidenceRecord>(&raw) {
                                if rec.call_id.as_deref() == Some(cid.as_str()) {
                                    existing_evidence = Some(rec);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let evidence_id = existing_evidence
        .as_ref()
        .map(|e| e.evidence_id)
        .unwrap_or_else(|| Uuid::parse_str(&req.event_id).unwrap_or_else(|_| Uuid::new_v4()));

    // Output processing
    let output_meta = if let Some(raw_output) = &req.output_text {
        let redacted = apply_redaction(raw_output);
        let bytes = redacted.as_bytes();
        let byte_count = bytes.len();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sha256 = hex::encode(hasher.finalize());

        let (excerpt_bytes, truncated) = if byte_count > MAX_OUTPUT_BYTES {
            (&bytes[..MAX_OUTPUT_BYTES], true)
        } else {
            (bytes, false)
        };

        let excerpt_file = evidence_excerpt_path(project_root, target_run_id, evidence_id);
        write_atomic(&excerpt_file, excerpt_bytes)?;

        let rel_path = format!(
            ".moraine/evidence/{}/{}",
            target_run_id
                .map(|i| i.to_string())
                .unwrap_or_else(|| "unassigned".into()),
            excerpt_file.file_name().unwrap().to_str().unwrap()
        );

        Some(OutputMetadata {
            captured: true,
            truncated,
            byte_count,
            sha256,
            excerpt_path: Some(rel_path),
        })
    } else {
        existing_evidence.as_ref().and_then(|e| e.output.clone())
    };

    let record = EvidenceRecord {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        evidence_id,
        run_id: target_run_id,
        session_key: req.session_key.clone(),
        kind,
        provenance,
        integration: req.integration,
        tool: req.tool.clone(),
        command: command.clone(),
        working_directory,
        started_at: existing_evidence
            .as_ref()
            .map(|e| e.started_at)
            .unwrap_or(started_at),
        finished_at,
        exit_code: req.exit_code,
        output: output_meta,
        call_id: req.call_id,
    };

    let json_path = evidence_file_path(project_root, target_run_id, evidence_id);
    let json_raw = serde_json::to_string_pretty(&record)?;
    write_atomic(&json_path, format!("{json_raw}\n").as_bytes())?;

    // If attached to a run, update sidecar and Markdown projection
    if let Some(run_id) = target_run_id {
        attach_evidence_to_run(project_root, run_id, &record)?;
    }

    Ok(record)
}

fn attach_evidence_to_run(
    project_root: &Path,
    run_id: Uuid,
    record: &EvidenceRecord,
) -> Result<()> {
    let runs_dir = project_root.join(MORAINE_DIR).join("runs");
    if !runs_dir.exists() {
        return Ok(());
    }

    // Locate the run file with matching run_id
    let mut target_md: Option<PathBuf> = None;
    if let Ok(entries) = fs::read_dir(&runs_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("md") {
                let sidecar_p = moraine_sidecar_path(&p);
                if sidecar_p.exists() {
                    if let Ok(raw) = fs::read_to_string(&sidecar_p) {
                        if let Ok(meta) = serde_json::from_str::<RunMeta>(&raw) {
                            if meta.run.id == run_id {
                                target_md = Some(p);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let Some(md_path) = target_md else {
        return Ok(());
    };
    let sidecar_path = moraine_sidecar_path(&md_path);
    let _lock = SidecarLock::acquire(&sidecar_path)?;

    let raw_meta = fs::read_to_string(&sidecar_path)?;
    let mut meta: RunMeta = serde_json::from_str(&raw_meta)?;

    if meta.agent.is_none() {
        return Ok(());
    }

    let cmd_str = record.command.as_deref().unwrap_or("").to_string();
    let summary_str = match record.kind {
        EvidenceKind::CommandResult => format!(
            "{} (exit {})",
            if cmd_str.is_empty() {
                record.tool.as_str()
            } else {
                cmd_str.as_str()
            },
            record.exit_code.unwrap_or(0)
        ),
        _ => format!("{} ({})", record.tool, record.provenance.as_str()),
    };

    let summary = EvidenceSummary {
        evidence_id: record.evidence_id,
        kind: record.kind,
        provenance: record.provenance,
        tool: record.tool.clone(),
        command: record.command.clone(),
        exit_code: record.exit_code,
        summary: summary_str,
        occurred_at: record.finished_at.unwrap_or(record.started_at),
    };

    let agent = meta.agent.as_mut().unwrap();

    // Upsert evidence summary in agent state
    if let Some(pos) = agent
        .evidence
        .iter()
        .position(|e| e.evidence_id == record.evidence_id)
    {
        agent.evidence[pos] = summary;
    } else {
        agent.evidence.push(summary);
    }

    meta.touch();

    // Re-render Markdown
    let raw_md = fs::read_to_string(&md_path)?;
    let human_notes = super::markdown::extract_human_notes(&raw_md)?;
    let updated_md = super::markdown::render_run_markdown_with_id(
        run_id,
        meta.agent.as_ref().unwrap(),
        &human_notes,
    );

    write_atomic(
        &sidecar_path,
        format!("{}\n", serde_json::to_string_pretty(&meta)?).as_bytes(),
    )?;
    write_atomic(&md_path, updated_md.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_values() {
        let raw = "cargo test --password mysecret123 --token sk-1234567890123456789012345";
        let redacted = redact_secrets(raw);
        assert!(!redacted.contains("mysecret123"));
        assert!(!redacted.contains("sk-1234567890123456789012345"));
        assert!(redacted.contains("[REDACTED]"));
    }
}
