//! STDIO MCP server bootstrap.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use moraine_core::{resolve_or_init_project, resolve_existing_project};
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

use crate::tools::MoraineMcp;

/// Soft budget for server instructions (bytes).
pub const SERVER_INSTRUCTIONS_MAX_BYTES: usize = 1800;

/// Server instructions for MCP clients (first 512 chars must be self-sufficient).
pub fn server_instructions() -> String {
    // Keep the first ~512 characters independently useful.
    let s = "Use Moraine for every bounded coding task in this project. Call run_start before \
substantive work, record only meaningful checkpoints, and call run_ready after \
validation. Keep normal runs to roughly 3–8 checkpoints. Reuse the returned run \
ID and content hash. Never record or imply human approval. Do not submit secrets, \
private reasoning, full transcripts, or large logs.\n\n\
Moraine captures Git branch/HEAD/working-tree facts mechanically—do not restate them. \
Supply objective, rationales, risks, open questions, and concise agent-reported evidence. \
On revision_conflict, call run_show and retry with the new contentHash. \
Agent-reported evidence is not independently verified. Human decisions are not available through MCP.";
    debug_assert!(s.len() <= SERVER_INSTRUCTIONS_MAX_BYTES);
    s.to_string()
}

/// Resolve and fix the project root for the lifetime of the process.
pub fn resolve_project_root(project: Option<&Path>) -> Result<PathBuf> {
    // Prefer existing project; otherwise init minimal structure (start may need it).
    let result = match resolve_existing_project(project) {
        Ok(r) => r,
        Err(_) => resolve_or_init_project(project).context("resolve or init Moraine project")?,
    };
    let root = result.project_root;
    if !root.is_dir() {
        bail!("project root is not a directory: {}", root.display());
    }
    Ok(root)
}

/// Run the STDIO MCP server (protocol frames on stdout; logs on stderr).
pub async fn run_stdio_server(project: Option<PathBuf>) -> Result<()> {
    // Diagnostics only on stderr — never pollute MCP stdout frames.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();

    let root = resolve_project_root(project.as_deref())?;
    tracing::info!(project = %root.display(), "moraine mcp starting (stdio)");

    let service = MoraineMcp::new(root)
        .serve(stdio())
        .await
        .context("MCP serve(stdio) failed")?;

    service.waiting().await.context("MCP service wait")?;
    Ok(())
}
