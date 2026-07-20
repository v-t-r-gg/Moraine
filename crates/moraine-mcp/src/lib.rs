//! Local STDIO MCP transport for the Moraine agent-run protocol.
//!
//! Handlers call `moraine-core` directly (no CLI shell-out). Project root is
//! fixed for the lifetime of the server process.

mod server;
mod tools;

pub use server::{run_stdio_server, server_instructions, SERVER_INSTRUCTIONS_MAX_BYTES};
pub use tools::{tool_names, MoraineMcp, TOOLS_LIST_MAX_BYTES};
