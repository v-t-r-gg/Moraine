//! Shared library for CLI, desktop, and tests. No Tauri / Axum / UI deps.
//!
//! One markdown file = one collab room. No multi-file workspace in core.

pub mod comments;
pub mod document;
pub mod error;
pub mod history;
pub mod paths;
pub mod room;
pub mod share;
pub mod watcher;

pub use comments::{
    comments_sidecar_path, merge_comments, read_comments_sidecar, write_comments_sidecar,
    CommentRecord, CommentsFile,
};
pub use document::{Document, DocumentId, DocumentMeta, DocumentSnapshot};
pub use error::{Error, Result};
pub use history::{HistoryEntry, HistoryStore};
pub use paths::MorainePaths;
pub use room::{room_id_for_path, room_id_for_str};
pub use share::{
    bind_from_http, http_to_ws, share_links, ShareLinks, DEFAULT_RELAY_BIND, DEFAULT_RELAY_HTTP,
    DEFAULT_RELAY_WS, DEFAULT_UI,
};
pub use watcher::{FileChange, FileWatcher, WatchEvent};
