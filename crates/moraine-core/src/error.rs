use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("path not found: {0}")]
    NotFound(PathBuf),

    #[error("not a file: {0}")]
    NotAFile(PathBuf),

    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("invalid UTF-8 in file: {0}")]
    InvalidUtf8(PathBuf),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("watcher error: {0}")]
    Watcher(String),

    #[error("history error: {0}")]
    History(String),

    /// Markdown on disk no longer matches the revision the caller expected.
    #[error("revision conflict: expected content hash {expected}, actual {actual}")]
    RevisionConflict { expected: String, actual: String },

    /// Another process holds the ledger lock, or the lock could not be taken.
    #[error("ledger busy: {0}")]
    LedgerBusy(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::NotFound(_) => "not_found",
            Self::NotAFile(_) => "not_a_file",
            Self::NotADirectory(_) => "not_a_directory",
            Self::InvalidUtf8(_) => "invalid_utf8",
            Self::Serde(_) => "serde",
            Self::Watcher(_) => "watcher",
            Self::History(_) => "history",
            Self::RevisionConflict { .. } => "revision_conflict",
            Self::LedgerBusy(_) => "ledger_busy",
            Self::Other(_) => "error",
        }
    }

    /// Structured JSON object suitable for CLI/Tauri error payloads.
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::RevisionConflict { expected, actual } => serde_json::json!({
                "kind": "revision_conflict",
                "expectedContentHash": expected,
                "actualContentHash": actual,
                "message": "The Markdown file changed before the operation completed.",
            }),
            Self::LedgerBusy(msg) => serde_json::json!({
                "kind": "ledger_busy",
                "message": msg,
            }),
            other => serde_json::json!({
                "kind": other.kind_str(),
                "message": other.to_string(),
            }),
        }
    }
}
