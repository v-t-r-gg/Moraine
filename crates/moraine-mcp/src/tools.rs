//! MCP tool surface: run_start, run_show, run_checkpoint, run_ready, run_resume.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use moraine_core::{
    run_checkpoint, run_ready, run_resume, run_show, run_start, CheckpointInput,
    Error as CoreError, EvidenceItem, EvidenceKind, EvidenceProvenance, RationalItem,
    RunShowOptions, RunStartRequest,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// Soft budget for serialized tools/list (bytes, not provider tokens).
pub const TOOLS_LIST_MAX_BYTES: usize = 12 * 1024;

#[derive(Clone)]
pub struct MoraineMcp {
    project_root: Arc<PathBuf>,
    tool_router: ToolRouter<MoraineMcp>,
}

impl MoraineMcp {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root: Arc::new(project_root),
            tool_router: Self::tool_router(),
        }
    }

    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    pub fn list_tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect()
    }

    pub fn tools_list_json_bytes(&self) -> usize {
        let tools = self.tool_router.list_all();
        serde_json::to_vec(&tools).map(|v| v.len()).unwrap_or(0)
    }
}

/// Public tool names for tests (stable set).
pub fn tool_names() -> &'static [&'static str] {
    &[
        "run_start",
        "run_show",
        "run_checkpoint",
        "run_ready",
        "run_resume",
    ]
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunStartArgs {
    pub objective: String,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunShowArgs {
    pub run_id: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunCheckpointArgs {
    pub run_id: String,
    pub expected_hash: String,
    pub idempotency_key: String,
    pub summary: String,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub rationales: Vec<RationaleArg>,
    #[serde(default)]
    pub evidence: Vec<EvidenceArg>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RationaleArg {
    pub choice: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceArg {
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunReadyArgs {
    pub run_id: String,
    pub expected_hash: String,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunResumeArgs {
    pub run_id: String,
    pub expected_hash: String,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[tool_router]
impl MoraineMcp {
    #[tool(
        description = "Start a Moraine run. Args: objective, idempotencyKey. Returns runId, contentHash, state. No full Markdown."
    )]
    async fn run_start(
        &self,
        Parameters(args): Parameters<RunStartArgs>,
    ) -> Result<CallToolResult, McpError> {
        match run_start(RunStartRequest {
            objective: args.objective,
            idempotency_key: args.idempotency_key,
            project: Some(self.project_root.as_ref().clone()),
        }) {
            Ok(r) => ok_json(json!({
                "runId": r.run_id.to_string(),
                "state": r.state.as_str(),
                "recordPath": r.record_path,
                "contentHash": r.content_hash,
                "recordRevision": r.record_revision,
                "projectId": r.project_id.map(|u| u.to_string()),
                "idempotentReplay": r.idempotent_replay,
                "git": compact_git(r.git.as_ref()),
            })),
            Err(e) => core_err(e),
        }
    }

    #[tool(
        description = "Read-only compact run status: state, hash, recent checkpoints, risk totals. No full Markdown."
    )]
    async fn run_show(
        &self,
        Parameters(args): Parameters<RunShowArgs>,
    ) -> Result<CallToolResult, McpError> {
        let run_id = parse_uuid(&args.run_id)?;
        match run_show(
            Some(self.project_root.as_ref()),
            run_id,
            RunShowOptions {
                include_markdown: false,
                ..Default::default()
            },
        ) {
            Ok(p) => ok_json(json!({
                "runId": p.run_id.to_string(),
                "projectId": p.project_id.map(|u| u.to_string()),
                "recordPath": p.record_path,
                "state": p.state.as_str(),
                "contentHash": p.content_hash,
                "recordRevision": p.record_revision,
                "objective": p.objective,
                "checkpointCount": p.checkpoint_count,
                "recentCheckpoints": p.recent_checkpoints,
                "risks": p.risks,
                "openQuestions": p.open_questions,
                "annotations": p.annotations,
                "reviewState": p.review_state,
                "decisionCurrent": p.decision_current,
                "incompleteOperation": p.incomplete_operation,
                "git": compact_git(p.current_git.as_ref()),
            })),
            Err(e) => core_err(e),
        }
    }

    #[tool(
        description = "Append a structured checkpoint. Requires runId, expectedHash, idempotencyKey, summary. Evidence is agent-reported."
    )]
    async fn run_checkpoint(
        &self,
        Parameters(args): Parameters<RunCheckpointArgs>,
    ) -> Result<CallToolResult, McpError> {
        let run_id = parse_uuid(&args.run_id)?;
        let expected_hash = args.expected_hash.clone();
        let idempotency_key = args.idempotency_key.clone();
        let input = match map_checkpoint(args) {
            Ok(i) => i,
            Err(msg) => return core_err(CoreError::InvalidCheckpoint { message: msg }),
        };
        match run_checkpoint(
            Some(self.project_root.as_ref()),
            run_id,
            &expected_hash,
            &idempotency_key,
            input,
        ) {
            Ok(r) => ok_json(op_result_json(&r)),
            Err(e) => core_err(e),
        }
    }

    #[tool(
        description = "Mark run ready for human review (not approval). Requires runId, expectedHash, idempotencyKey."
    )]
    async fn run_ready(
        &self,
        Parameters(args): Parameters<RunReadyArgs>,
    ) -> Result<CallToolResult, McpError> {
        let run_id = parse_uuid(&args.run_id)?;
        match run_ready(
            Some(self.project_root.as_ref()),
            run_id,
            &args.expected_hash,
            &args.idempotency_key,
            args.summary,
        ) {
            Ok(r) => ok_json(op_result_json(&r)),
            Err(e) => core_err(e),
        }
    }

    #[tool(
        description = "Resume a ready_for_review run to active. Requires runId, expectedHash, idempotencyKey. Optional reason."
    )]
    async fn run_resume(
        &self,
        Parameters(args): Parameters<RunResumeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let run_id = parse_uuid(&args.run_id)?;
        match run_resume(
            Some(self.project_root.as_ref()),
            run_id,
            &args.expected_hash,
            &args.idempotency_key,
            args.reason,
        ) {
            Ok(r) => ok_json(op_result_json(&r)),
            Err(e) => core_err(e),
        }
    }
}

#[tool_handler]
impl ServerHandler for MoraineMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("moraine", env!("CARGO_PKG_VERSION"))
                    .with_title("Moraine")
                    .with_description(
                        "Local-first review layer for coding-agent work (agent-run protocol)",
                    )
                    .with_website_url("https://github.com/v-t-r-gg/Moraine"),
            )
            .with_instructions(crate::server::server_instructions())
    }
}

fn op_result_json(r: &moraine_core::AgentOpResult) -> serde_json::Value {
    json!({
        "runId": r.run_id.to_string(),
        "state": r.state.as_str(),
        "recordPath": r.record_path,
        "contentHash": r.content_hash,
        "recordRevision": r.record_revision,
        "opId": r.op_id.map(|u| u.to_string()),
        "idempotentReplay": r.idempotent_replay,
        "reviewState": r.review_state,
        "decisionCurrent": r.decision_current,
        "git": compact_git(r.git.as_ref()),
    })
}

fn compact_git(g: Option<&moraine_core::GitContextSummary>) -> serde_json::Value {
    let Some(g) = g else {
        return json!(null);
    };
    json!({
        "available": g.available,
        "branch": g.branch,
        "head": g.head.as_ref().map(|h| {
            if h.len() > 12 { h[..12].to_string() } else { h.clone() }
        }),
        "workingTree": g.working_tree,
        "changedFileCount": g.changed_file_count,
    })
}

fn map_checkpoint(args: RunCheckpointArgs) -> Result<CheckpointInput, String> {
    let mut evidence = Vec::new();
    for e in args.evidence {
        let kind = match e.kind.as_str() {
            "command_result" => EvidenceKind::CommandResult,
            "path" => EvidenceKind::Path,
            "url" => EvidenceKind::Url,
            "note" => EvidenceKind::Note,
            other => return Err(format!("unknown evidence kind: {other}")),
        };
        evidence.push(EvidenceItem {
            kind,
            label: e.label,
            command: e.command,
            exit_code: e.exit_code,
            path: e.path,
            url: e.url,
            // Core validation also forces agent_reported; set honestly here.
            provenance: EvidenceProvenance::AgentReported,
        });
    }
    Ok(CheckpointInput {
        summary: args.summary,
        actions: args.actions,
        rationales: args
            .rationales
            .into_iter()
            .map(|r| RationalItem {
                choice: r.choice,
                reason: r.reason,
            })
            .collect(),
        evidence,
        risks: args.risks,
        open_questions: args.open_questions,
    })
}

fn parse_uuid(s: &str) -> Result<Uuid, McpError> {
    Uuid::from_str(s.trim()).map_err(|_| {
        McpError::invalid_params(
            "invalid runId",
            Some(json!({ "code": "run_not_found", "runId": s })),
        )
    })
}

fn ok_json(value: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(
        value.to_string(),
    )]))
}

fn core_err(err: CoreError) -> Result<CallToolResult, McpError> {
    let details = err.to_json_value();
    let code = details
        .get("code")
        .and_then(|c| c.as_str())
        .unwrap_or(err.protocol_code())
        .to_string();
    let message = details
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or(&err.to_string())
        .to_string();
    // Prefer structured error payload without absolute paths when present.
    let mut safe = details.clone();
    if let Some(obj) = safe.as_object_mut() {
        obj.remove("path");
    }
    Ok(CallToolResult::error(vec![ContentBlock::text(
        json!({
            "ok": false,
            "error": {
                "code": code,
                "message": message,
                "details": safe,
            }
        })
        .to_string(),
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use moraine_core::init_project;
    use tempfile::tempdir;

    #[test]
    fn tool_set_is_exact_and_excludes_human_decisions() {
        let dir = tempdir().unwrap();
        let project = init_project(Some(dir.path())).unwrap();
        let mcp = MoraineMcp::new(project.project_root);
        let mut names = mcp.list_tool_names();
        names.sort();
        let mut expected: Vec<_> = tool_names().iter().map(|s| s.to_string()).collect();
        expected.sort();
        assert_eq!(names, expected);
        for forbidden in [
            "decide",
            "approved",
            "changes_requested",
            "rejected",
            "run_open",
            "project_init",
        ] {
            assert!(
                !names.iter().any(|n| n.contains(forbidden)),
                "forbidden tool fragment {forbidden} in {names:?}"
            );
        }
    }

    #[test]
    fn tools_list_under_budget() {
        let dir = tempdir().unwrap();
        let project = init_project(Some(dir.path())).unwrap();
        let mcp = MoraineMcp::new(project.project_root);
        let n = mcp.tools_list_json_bytes();
        assert!(
            n < TOOLS_LIST_MAX_BYTES,
            "tools list {n} bytes exceeds {TOOLS_LIST_MAX_BYTES}"
        );
    }

    #[test]
    fn instructions_budget_and_prefix() {
        let instr = crate::server::server_instructions();
        assert!(instr.len() <= crate::server::SERVER_INSTRUCTIONS_MAX_BYTES);
        let head: String = instr.chars().take(512).collect();
        assert!(head.contains("run_start"));
        assert!(head.contains("run_ready"));
        assert!(head.contains("human approval") || head.contains("Never record"));
    }
}
