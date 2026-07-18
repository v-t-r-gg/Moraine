//! Shared foundation for the desktop app and CLI (no GUI deps).

pub mod document;
pub mod error;
pub mod history;
pub mod paths;
pub mod watcher;

pub use document::{Document, DocumentId, DocumentMeta, DocumentSnapshot};
pub use error::{Error, Result};
pub use history::{HistoryEntry, HistoryStore};
pub use paths::MorainePaths;
pub use watcher::{FileChange, FileWatcher, WatchEvent};
