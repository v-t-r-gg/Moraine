//! Durable file snapshots for write-ahead recovery.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{ProvisionError, Result};
use crate::types::FileSnapshot;

pub fn file_sha256(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let mut h = Sha256::new();
    h.update(&data);
    Ok(hex::encode(h.finalize()))
}

pub fn optional_file_sha256(path: &Path) -> Option<String> {
    if path.is_file() {
        file_sha256(path).ok()
    } else {
        None
    }
}

/// Create a durable on-disk backup of an existing file (fsync data + parent dir).
pub fn durable_backup(source: &Path) -> Result<FileSnapshot> {
    if !source.is_file() {
        return Err(ProvisionError::msg(format!(
            "cannot backup missing file {}",
            source.display()
        )));
    }
    let hash = file_sha256(source)?;
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S%.3f");
    let bak = source.with_extension(format!("bak.{ts}"));
    fs::copy(source, &bak)?;
    {
        let f = File::open(&bak)
            .map_err(|e| ProvisionError::msg(format!("open backup {}: {e}", bak.display())))?;
        f.sync_all()
            .map_err(|e| ProvisionError::msg(format!("fsync backup {}: {e}", bak.display())))?;
    }
    if let Some(parent) = bak.parent() {
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| {
                ProvisionError::msg(format!("fsync backup parent {}: {e}", parent.display()))
            })?;
    }
    Ok(FileSnapshot::Existing {
        path: source.display().to_string(),
        backup_path: bak.display().to_string(),
        original_hash: hash,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn snapshot_absent(path: &Path) -> FileSnapshot {
    FileSnapshot::Absent {
        path: path.display().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Apply a snapshot during rollback (restore existing or remove created).
pub fn restore_snapshot(snap: &FileSnapshot) -> Result<()> {
    match snap {
        FileSnapshot::Existing {
            path, backup_path, ..
        } => {
            let original = PathBuf::from(path);
            let backup = PathBuf::from(backup_path);
            if !backup.is_file() {
                return Err(ProvisionError::msg(format!(
                    "backup missing for rollback: {}",
                    backup.display()
                )));
            }
            if let Some(parent) = original.parent() {
                fs::create_dir_all(parent)?;
            }
            // Durable restore: copy to temp then rename.
            let tmp = original.with_extension("rollback.tmp");
            fs::copy(&backup, &tmp)?;
            {
                let f = File::open(&tmp)?;
                f.sync_all()?;
            }
            fs::rename(&tmp, &original)?;
            if let Ok(f) = File::open(&original) {
                let _ = f.sync_all();
            }
            Ok(())
        }
        FileSnapshot::Absent { path, .. } => {
            let p = PathBuf::from(path);
            if p.is_file() {
                fs::remove_file(&p)?;
            }
            // Best-effort: remove empty parent .codex if we emptied it.
            if let Some(parent) = p.parent() {
                if parent.file_name().and_then(|n| n.to_str()) == Some(".codex") {
                    let _ = fs::remove_dir(parent); // only if empty
                }
            }
            Ok(())
        }
    }
}

/// Atomic write with fsync of temp file before rename.
pub fn atomic_write_durable(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("moraine")
    ));
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| ProvisionError::msg(format!("fsync write {}: {e}", path.display())))?;
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| {
            ProvisionError::msg(format!("fsync write parent {}: {e}", parent.display()))
        })?;
    Ok(())
}
