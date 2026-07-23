//! Setup transaction journal under ~/.local/share/moraine/setup-transactions/.

use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::Result;
use crate::suite::setup_transactions_dir;
use crate::types::SetupReceipt;

pub fn journal_path(transaction_id: Uuid) -> PathBuf {
    setup_transactions_dir().join(format!("{transaction_id}.json"))
}

pub fn write_journal(receipt: &SetupReceipt) -> Result<PathBuf> {
    let dir = setup_transactions_dir();
    fs::create_dir_all(&dir)?;
    let path = journal_path(receipt.transaction_id);
    let raw = serde_json::to_string_pretty(receipt)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, format!("{raw}\n"))?;
    fs::rename(&tmp, &path)?;
    Ok(path)
}

pub fn read_journal(transaction_id: Uuid) -> Result<SetupReceipt> {
    let path = journal_path(transaction_id);
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn write_journal_at(path: &Path, receipt: &SetupReceipt) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(receipt)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, format!("{raw}\n"))?;
    fs::rename(&tmp, path)?;
    Ok(())
}
