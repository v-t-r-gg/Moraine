//! Provisioning errors.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProvisionError {
    #[error("{0}")]
    Message(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("core: {0}")]
    Core(#[from] moraine_core::Error),

    #[error("operation {op_id} failed: {message}")]
    OperationFailed { op_id: String, message: String },

    #[error("project path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("unsupported agent: {0}")]
    UnsupportedAgent(String),

    #[error("service manager: {0}")]
    Service(String),

    #[error("rollback required: {0}")]
    RollbackRequired(String),
}

pub type Result<T> = std::result::Result<T, ProvisionError>;

impl ProvisionError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}
