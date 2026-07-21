//! Deterministic Markdown projection for agent runs.
//!
//! Uses ordinary ATX headings and lists so Tiptap (`html: false`) round-trips
//! preserve structure.
//!
//! **Authority model A:** Moraine-managed sections are projections of structured
//! sidecar state. Human-authored free text that survives agent mutations lives
//! only under `## Human notes`. Edits outside that region are not preserved on
//! the next protocol operation.

use chrono::{DateTime, Utc};

use super::git::GitContextSummary;
use super::types::{AgentRunState, CheckpointRecord, LifecycleEvent, RunLifecycle};
use crate::error::{Error, Result};

pub const HUMAN_NOTES_HEADING: &str = "## Human notes";

/// Byte offset where the Human notes body begins (after the delimiter's line ending).
///
/// Only the **first** line that is exactly `## Human notes` (no leading/trailing
/// spaces on the line content) is the managed delimiter. Later identical lines
/// inside the body (including fenced code) are human content.
pub fn human_notes_body_start(markdown: &str) -> Result<usize> {
    let bytes = markdown.as_bytes();
    let heading = HUMAN_NOTES_HEADING.as_bytes();
    let mut line_start = 0usize;
    let mut i = 0usize;
    while i <= bytes.len() {
        if i == bytes.len() || bytes[i] == b'\n' {
            let mut content_end = i;
            if content_end > line_start && bytes[content_end - 1] == b'\r' {
                content_end -= 1;
            }
            if &bytes[line_start..content_end] == heading {
                // Body starts after the original line ending (LF or after CR of CRLF).
                let body_start = if i < bytes.len() {
                    i + 1 // skip '\n'
                } else {
                    i // delimiter was last line without trailing newline → empty body
                };
                return Ok(body_start);
            }
            if i == bytes.len() {
                break;
            }
            line_start = i + 1;
        }
        i += 1;
    }
    Err(Error::RunRecordStructureInvalid {
        message: "missing required heading '## Human notes'".into(),
    })
}

/// Extract human notes body **byte-for-byte** (LF/CRLF, trailing blanks, no rejoin).
pub fn extract_human_notes(markdown: &str) -> Result<String> {
    let start = human_notes_body_start(markdown)?;
    Ok(markdown[start..].to_string())
}

/// Render with explicit run id (stored on RunMeta.run.id).
pub fn render_run_markdown_with_id(
    run_id: uuid::Uuid,
    agent: &AgentRunState,
    human_notes: &str,
) -> String {
    let mut out = String::with_capacity(8192);
    out.push_str("# Moraine run record\n\n");

    out.push_str("## Objective\n\n");
    out.push_str(agent.objective.trim());
    out.push_str("\n\n");

    out.push_str("## Protocol status\n\n");
    out.push_str(
        "> **Managed regions:** Everything above `## Human notes` is regenerated from Moraine structured state. Human free-form edits and accepted suggestion text outside Human notes are **not** preserved on the next agent operation. Review managed content with comments / request-changes; put free-form notes only under Human notes.\n\n",
    );
    out.push_str(&format!("- **Run ID:** `{run_id}`\n"));
    out.push_str(&format!(
        "- **Lifecycle:** `{}`\n",
        agent.lifecycle.as_str()
    ));
    out.push_str(&format!(
        "- **Capture coverage:** `{}`\n",
        agent.capture_coverage.as_str()
    ));
    if agent.provisional {
        out.push_str("- **Provisional:** `true` (awaiting semantic `run_start`)\n");
    }
    if let Some(sid) = &agent.session_id {
        out.push_str(&format!("- **Session ID:** `{sid}`\n"));
    }
    out.push_str(&format!(
        "- **Record revision:** `{}`\n",
        agent.record_revision
    ));
    out.push_str(&format!("- **Record path:** `{}`\n", agent.record_path));
    if let Some(pid) = agent.project_id {
        out.push_str(&format!("- **Project ID:** `{pid}`\n"));
    }
    out.push('\n');

    out.push_str("## Starting Git context\n\n");
    out.push_str(&format_git(agent.starting_git.as_ref()));
    out.push('\n');

    out.push_str("## Current Git context\n\n");
    out.push_str(&format_git(agent.current_git.as_ref()));
    out.push('\n');

    out.push_str("## Checkpoints\n\n");
    if agent.checkpoints.is_empty() {
        out.push_str("_No checkpoints yet._\n\n");
    } else {
        for (i, cp) in agent.checkpoints.iter().enumerate() {
            out.push_str(&format_checkpoint(i + 1, cp));
            out.push('\n');
        }
    }

    out.push_str("## Risks\n\n");
    if agent.risks.is_empty() {
        out.push_str("_None recorded._\n\n");
    } else {
        for r in &agent.risks {
            out.push_str(&format!("- {}\n", escape_list_item(r)));
        }
        out.push('\n');
    }

    out.push_str("## Open questions\n\n");
    if agent.open_questions.is_empty() {
        out.push_str("_None recorded._\n\n");
    } else {
        for q in &agent.open_questions {
            out.push_str(&format!("- {}\n", escape_list_item(q)));
        }
        out.push('\n');
    }

    out.push_str("## Lifecycle events\n\n");
    if agent.lifecycle_events.is_empty() {
        out.push_str("_None yet._\n\n");
    } else {
        for ev in &agent.lifecycle_events {
            out.push_str(&format_lifecycle(ev));
        }
        out.push('\n');
    }

    out.push_str("## Evidence\n\n");
    if agent.evidence.is_empty() {
        out.push_str("_None recorded._\n\n");
    } else {
        for ev in &agent.evidence {
            let prov = ev.provenance.as_str();
            let tool = &ev.tool;
            let cmd = ev.command.as_deref().unwrap_or("").trim();
            if !cmd.is_empty() {
                out.push_str(&format!(
                    "- `[{prov}]` **{tool}**: `{cmd}`{}\n",
                    ev.exit_code
                        .map(|code| format!(" (exit {code})"))
                        .unwrap_or_default()
                ));
            } else {
                out.push_str(&format!("- `[{prov}]` **{tool}**: {}\n", ev.summary.trim()));
            }
        }
        out.push('\n');
    }

    if agent.lifecycle == RunLifecycle::ReadyForReview {
        out.push_str("## Ready for review\n\n");
        out.push_str("This run is **ready for human review**. Human decisions use `moraine decide` and are separate from agent lifecycle.\n\n");
        if let Some(s) = &agent.ready_summary {
            out.push_str("**Outcome summary:** ");
            out.push_str(s.trim());
            out.push_str("\n\n");
        }
    }

    out.push_str("---\n\n");
    out.push_str(HUMAN_NOTES_HEADING);
    out.push('\n');
    // Append Human notes body exactly as stored (may be empty, CRLF, no final NL).
    out.push_str(human_notes);
    out
}

fn format_git(git: Option<&GitContextSummary>) -> String {
    let Some(g) = git else {
        return "_Git context not available._\n".into();
    };
    if !g.available {
        return "_Not a Git repository (or Git unavailable)._\n".into();
    }
    let mut s = String::new();
    if let Some(r) = &g.repository_root {
        s.push_str(&format!("- **Repository root:** `{r}`\n"));
    }
    if let Some(true) = g.detached {
        s.push_str("- **HEAD:** detached\n");
    } else if let Some(b) = &g.branch {
        s.push_str(&format!("- **Branch:** `{b}`\n"));
    }
    if let Some(h) = &g.head {
        s.push_str(&format!("- **HEAD:** `{h}`\n"));
    }
    if let Some(u) = &g.upstream {
        s.push_str(&format!("- **Upstream:** `{u}`\n"));
    }
    if let Some(w) = &g.working_tree {
        s.push_str(&format!("- **Working tree:** `{w}`\n"));
    }
    if let Some(c) = g.changed_file_count {
        s.push_str(&format!("- **Changed files:** {c}\n"));
        for f in &g.changed_files {
            s.push_str(&format!("  - `{f}`\n"));
        }
    }
    if s.is_empty() {
        s.push_str("_Git available; no summary fields._\n");
    }
    s
}

fn format_checkpoint(n: usize, cp: &CheckpointRecord) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "### Checkpoint {n} — {}\n\n",
        format_ts(cp.created_at)
    ));
    s.push_str(&format!("- **Op ID:** `{}`\n", cp.op_id));
    s.push_str(&format!("- **Summary:** {}\n\n", cp.summary.trim()));
    if !cp.actions.is_empty() {
        s.push_str("#### Actions\n\n");
        for a in &cp.actions {
            s.push_str(&format!("- {}\n", escape_list_item(a)));
        }
        s.push('\n');
    }
    if !cp.rationales.is_empty() {
        s.push_str("#### Rationales\n\n");
        for r in &cp.rationales {
            s.push_str(&format!(
                "- **{}:** {}\n",
                escape_list_item(&r.choice),
                r.reason.trim()
            ));
        }
        s.push('\n');
    }
    if !cp.evidence.is_empty() {
        s.push_str("#### Evidence\n\n");
        for e in &cp.evidence {
            s.push_str(&format!(
                "- [{} | {}] {}",
                e.kind.as_str(),
                e.provenance.as_str(),
                e.label.trim()
            ));
            if let Some(cmd) = &e.command {
                s.push_str(&format!(" — `{cmd}`"));
            }
            if let Some(code) = e.exit_code {
                s.push_str(&format!(" (exit {code})"));
            }
            if let Some(p) = &e.path {
                s.push_str(&format!(" path=`{p}`"));
            }
            if let Some(u) = &e.url {
                s.push_str(&format!(" url=`{u}`"));
            }
            s.push('\n');
        }
        s.push('\n');
    }
    if let Some(g) = &cp.git {
        s.push_str("#### Git at checkpoint\n\n");
        s.push_str(&format_git(Some(g)));
        s.push('\n');
    }
    s
}

fn format_lifecycle(ev: &LifecycleEvent) -> String {
    let mut s = format!(
        "- **{}** at {} (op `{}`)",
        ev.kind,
        format_ts(ev.created_at),
        ev.op_id
    );
    if let Some(n) = &ev.note {
        s.push_str(&format!(" — {}", n.trim()));
    }
    s.push('\n');
    s
}

fn format_ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn escape_list_item(s: &str) -> String {
    s.replace('\n', " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_protocol::types::{AgentRunState, RunLifecycle};
    use uuid::Uuid;

    fn sample_agent() -> AgentRunState {
        AgentRunState {
            lifecycle: RunLifecycle::Active,
            record_revision: 1,
            objective: "Test objective".into(),
            record_path: ".moraine/runs/x.md".into(),
            project_id: Some(Uuid::nil()),
            start_idempotency_key: "k".into(),
            starting_git: None,
            current_git: None,
            checkpoints: vec![],
            lifecycle_events: vec![],
            ready_summary: None,
            idempotency: Default::default(),
            incomplete_op: None,
            risks: vec![],
            open_questions: vec![],
            capture_coverage: Default::default(),
            session_id: None,
            provisional: false,
            evidence: vec![],
        }
    }

    #[test]
    fn missing_human_notes_errors() {
        let err = extract_human_notes("# x\n").unwrap_err();
        assert!(matches!(err, Error::RunRecordStructureInvalid { .. }));
    }

    #[test]
    fn preserves_lf_body_exactly() {
        let md = "# t\n\n## Human notes\nline1\nline2\n";
        assert_eq!(extract_human_notes(md).unwrap(), "line1\nline2\n");
    }

    #[test]
    fn preserves_crlf_body_exactly() {
        let md = "# t\r\n\r\n## Human notes\r\nline1\r\nline2\r\n";
        assert_eq!(extract_human_notes(md).unwrap(), "line1\r\nline2\r\n");
    }

    #[test]
    fn preserves_no_final_newline() {
        let md = "## Human notes\nhello";
        assert_eq!(extract_human_notes(md).unwrap(), "hello");
    }

    #[test]
    fn preserves_one_trailing_blank_line() {
        let md = "## Human notes\nbody\n\n";
        assert_eq!(extract_human_notes(md).unwrap(), "body\n\n");
    }

    #[test]
    fn preserves_multiple_trailing_blank_lines() {
        let md = "## Human notes\nbody\n\n\n\n";
        assert_eq!(extract_human_notes(md).unwrap(), "body\n\n\n\n");
    }

    #[test]
    fn fenced_code_with_human_notes_is_content_not_delimiter() {
        let md = "## Human notes\n```\n## Human notes\ninside fence\n```\n";
        assert_eq!(
            extract_human_notes(md).unwrap(),
            "```\n## Human notes\ninside fence\n```\n"
        );
    }

    #[test]
    fn preserves_unicode() {
        let md = "## Human notes\ncafé 日本語 🎉\n";
        assert_eq!(extract_human_notes(md).unwrap(), "café 日本語 🎉\n");
    }

    #[test]
    fn roundtrip_append_preserves_notes_bytes() {
        let notes = "Keep me\r\nexactly\r\n\r\n";
        let md = render_run_markdown_with_id(Uuid::nil(), &sample_agent(), notes);
        assert_eq!(extract_human_notes(&md).unwrap(), notes);
    }
}
