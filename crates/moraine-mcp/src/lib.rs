//! Local STDIO MCP transport for the Moraine agent-run protocol.
//!
//! Handlers call `moraine-core` directly; no CLI shell-out or network listener.
//! The project root is fixed for the server lifetime. Core remains authoritative
//! for persistence; this crate owns transport mapping only.

mod server;
mod tools;

pub use server::{run_stdio_server, server_instructions, SERVER_INSTRUCTIONS_MAX_BYTES};
pub use tools::{tool_names, MoraineMcp, TOOLS_LIST_MAX_BYTES};
