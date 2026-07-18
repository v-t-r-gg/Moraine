//! Deterministic Markdown projection for agent runs.
//!
//! Uses ordinary ATX headings and lists so Tiptap (`html: false`) round-trips
//! preserve structure. Human-authored free text lives only under
//! `## Human notes` and is preserved byte-for-byte between mutations.

use chrono::{DateTime, Utc};

use super::git::GitContextSummary;
use super::types::{AgentRunState, CheckpointRecord, LifecycleEvent, RunLifecycle};
use crate::error::{Error, Result};

pub const HUMAN_NOTES_HEADING: &str = "## Human notes";
const HUMAN_MARKER: &str = "\n## Human notes\n";

/// Extract human notes body (may be empty). Errors if the heading is missing or duplicated.
pub fn extract_human_notes(markdown: &str) -> Result<String> {
    let count = markdown
        .lines()
        .filter(|l| l.trim_end() == HUMAN_NOTES_HEADING)
        .count();
    if count == 0 {
        return Err(Error::RunRecordStructureInvalid {
            message: "missing required heading '## Human notes'".into(),
        });
    }
    if count > 1 {
        return Err(Error::RunRecordStructureInvalid {
            message: "duplicate '## Human notes' headings".into(),
        });
    }
    if let Some(idx) = markdown.find(HUMAN_MARKER) {
        let after = &markdown[idx + HUMAN_MARKER.len()..];
        return Ok(after.to_string());
    }
    // Heading at start of file
    if markdown.starts_with(HUMAN_NOTES_HEADING) {
        let rest = markdown[HUMAN_NOTES_HEADING.len()..]
            .strip_prefix('\n')
            .unwrap_or("");
        return Ok(rest.to_string());
    }
    // Heading without preceding newline (unlikely)
    if let Some(pos) = markdown.find(HUMAN_NOTES_HEADING) {
        let after = &markdown[pos + HUMAN_NOTES_HEADING.len()..];
        let after = after.strip_prefix('\n').unwrap_or(after);
        return Ok(after.to_string());
    }
    Err(Error::RunRecordStructureInvalid {
        message: "could not locate Human notes section body".into(),
    })
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
    out.push_str(&format!("- **Run ID:** `{run_id}`\n"));
    out.push_str(&format!(
        "- **Lifecycle:** `{}`\n",
        agent.lifecycle.as_str()
    ));
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
    // Preserve human notes exactly (may be empty).
    if !human_notes.is_empty() {
        out.push_str(human_notes);
        if !human_notes.ends_with('\n') {
            out.push('\n');
        }
    } else {
        out.push('\n');
    }
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
            completed_ops: vec![],
            incomplete_op: None,
            risks: vec![],
            open_questions: vec![],
        }
    }

    #[test]
    fn human_notes_roundtrip() {
        let md = render_run_markdown_with_id(Uuid::nil(), &sample_agent(), "Keep me\nexactly\n");
        let notes = extract_human_notes(&md).unwrap();
        assert_eq!(notes, "Keep me\nexactly\n");
        let md2 = render_run_markdown_with_id(Uuid::nil(), &sample_agent(), &notes);
        assert_eq!(extract_human_notes(&md2).unwrap(), "Keep me\nexactly\n");
    }

    #[test]
    fn missing_human_notes_errors() {
        let err = extract_human_notes("# x\n").unwrap_err();
        assert!(matches!(err, Error::RunRecordStructureInvalid { .. }));
    }
}
