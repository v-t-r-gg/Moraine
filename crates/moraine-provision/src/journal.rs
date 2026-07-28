//! Write-ahead setup transaction journal (required, fsynced).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{ProvisionError, Result};
use crate::suite::setup_transactions_dir;
use crate::types::SetupReceipt;

thread_local! {
    static JOURNAL_DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

fn journal_dir() -> PathBuf {
    JOURNAL_DIR_OVERRIDE
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(setup_transactions_dir)
}

pub fn journal_path(transaction_id: Uuid) -> PathBuf {
    journal_dir().join(format!("{transaction_id}.json"))
}

/// Runs a synchronous operation with an isolated transaction-journal directory.
#[doc(hidden)]
pub fn with_journal_dir_override<T>(path: PathBuf, operation: impl FnOnce() -> T) -> T {
    struct Reset(Option<PathBuf>);

    impl Drop for Reset {
        fn drop(&mut self) {
            JOURNAL_DIR_OVERRIDE.with(|slot| {
                *slot.borrow_mut() = self.0.take();
            });
        }
    }

    let previous = JOURNAL_DIR_OVERRIDE.with(|slot| slot.replace(Some(path)));
    let _reset = Reset(previous);
    operation()
}

/// Persist receipt with fsync. Failure aborts the transaction (not best-effort).
pub fn write_journal(receipt: &SetupReceipt) -> Result<PathBuf> {
    let path = if receipt.journal_path.is_empty() {
        journal_path(receipt.transaction_id)
    } else {
        PathBuf::from(&receipt.journal_path)
    };
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(journal_dir);
    fs::create_dir_all(&dir)?;
    write_journal_at(&path, receipt)?;
    Ok(path)
}

pub fn read_journal(transaction_id: Uuid) -> Result<SetupReceipt> {
    let path = journal_path(transaction_id);
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

/// Atomic write + fsync of journal file and parent directory (best-effort on dir).
pub fn write_journal_at(path: &Path, receipt: &SetupReceipt) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(receipt)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| ProvisionError::msg(format!("journal create {}: {e}", tmp.display())))?;
        f.write_all(format!("{raw}\n").as_bytes())?;
        f.sync_all()
            .map_err(|e| ProvisionError::msg(format!("journal fsync {}: {e}", tmp.display())))?;
    }
    fs::rename(&tmp, path)
        .map_err(|e| ProvisionError::msg(format!("journal rename {}: {e}", path.display())))?;
    // Required: fsync final file.
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| ProvisionError::msg(format!("journal final fsync {}: {e}", path.display())))?;
    // Required on Unix when supported: fsync parent directory after rename.
    if let Some(parent) = path.parent() {
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| {
                ProvisionError::msg(format!(
                    "journal parent fsync {}: {e} (required for durable recovery)",
                    parent.display()
                ))
            })?;
    }
    Ok(())
}

/// List unfinished transactions (readiness not Ready/DirectVerified/Failed after rollback).
pub fn list_unfinished() -> Result<Vec<SetupReceipt>> {
    let dir = journal_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for ent in fs::read_dir(&dir)? {
        let ent = ent?;
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(raw) = fs::read_to_string(&p) {
            if let Ok(r) = serde_json::from_str::<SetupReceipt>(&raw) {
                use crate::types::Readiness;
                match r.readiness {
                    Readiness::RollbackRequired
                    | Readiness::NotConfigured
                    | Readiness::Degraded => {
                        out.push(r);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(out)
}
