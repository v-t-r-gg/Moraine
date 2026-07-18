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

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
