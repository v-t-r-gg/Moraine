//! Safe same-directory temporary write + replace.
//!
//! Guarantees:
//! * Full payload is written and flushed to a unique temp file before the destination is touched.
//! * Destination is never truncated and rewritten in place as a fallback.
//! * On replace failure, the original destination is left intact (when still present) and the temp
//!   file is retained next to it for recovery inspection.
//!
//! Durability boundary:
//! * Temp file contents are `sync_all`'d before replace.
//! * Directory fsync after replace is best-effort on Unix and not claimed on all platforms.
//! * Same-filesystem atomic rename on Unix; on Windows dest is moved aside then replaced, with
//!   restore of the aside file if the final rename fails.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{Error, Result};

/// Write `bytes` to `path` via a unique same-directory temp file and safe replace.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let tmp = parent.join(format!(".{base}.{}.tmp", Uuid::new_v4()));

    {
        let mut f = File::create(&tmp).map_err(Error::Io)?;
        f.write_all(bytes).map_err(Error::Io)?;
        f.sync_all().map_err(Error::Io)?;
    }

    match replace_file(&tmp, path) {
        Ok(()) => {
            let _ = sync_dir(parent);
            Ok(())
        }
        Err(e) => {
            // Keep tmp for recovery; do not destroy destination.
            Err(Error::other(format!(
                "failed to replace {} (temp retained at {}): {e}",
                path.display(),
                tmp.display()
            )))
        }
    }
}

fn replace_file(tmp: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        // Atomic replace when same filesystem.
        fs::rename(tmp, dest)?;
        Ok(())
    }

    #[cfg(windows)]
    {
        if !dest.exists() {
            return fs::rename(tmp, dest);
        }
        let parent = dest.parent().unwrap_or_else(|| Path::new("."));
        let bak = parent.join(format!(
            ".{}.{}.bak",
            dest.file_name().and_then(|s| s.to_str()).unwrap_or("file"),
            Uuid::new_v4()
        ));
        fs::rename(dest, &bak)?;
        match fs::rename(tmp, dest) {
            Ok(()) => {
                let _ = fs::remove_file(&bak);
                Ok(())
            }
            Err(e) => {
                let _ = fs::rename(&bak, dest);
                Err(e)
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        if dest.exists() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "atomic replace not implemented on this platform",
            ));
        }
        fs::rename(tmp, dest)
    }
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let f = OpenOptions::new().read(true).open(dir)?;
        f.sync_all()?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Path of the exclusive lock file for a ledger sidecar destination.
pub fn lock_path_for(sidecar: &Path) -> PathBuf {
    let mut s = sidecar.as_os_str().to_os_string();
    s.push(".lock");
    PathBuf::from(s)
}

/// Hold an exclusive cross-process lock on the per-sidecar lock file.
#[derive(Debug)]
pub struct SidecarLock {
    file: File,
    path: PathBuf,
}

impl SidecarLock {
    /// Block until the lock is acquired (or return error).
    pub fn acquire(sidecar: &Path) -> Result<Self> {
        let path = lock_path_for(sidecar);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)?;
        use fs4::fs_std::FileExt;
        file.lock_exclusive()
            .map_err(|e| Error::LedgerBusy(format!("could not lock {}: {e}", path.display())))?;
        Ok(Self { file, path })
    }

    /// Non-blocking acquire; returns `LedgerBusy` if held.
    pub fn try_acquire(sidecar: &Path) -> Result<Self> {
        let path = lock_path_for(sidecar);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)?;
        use fs4::fs_std::FileExt;
        match file.try_lock_exclusive() {
            Ok(true) => Ok(Self { file, path }),
            Ok(false) => Err(Error::LedgerBusy(format!(
                "ledger busy (lock held): {}",
                path.display()
            ))),
            Err(e) => Err(Error::LedgerBusy(format!(
                "could not lock {}: {e}",
                path.display()
            ))),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SidecarLock {
    fn drop(&mut self) {
        // Exclusive flock is released when the file handle closes (Unix/Windows).
        let _ = &self.file;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn write_creates_and_replaces() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("x.json");
        write_atomic(&path, b"one").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one");
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
    }

    #[test]
    fn no_tmp_left_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("y.json");
        write_atomic(&path, b"ok").unwrap();
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn exclusive_lock_blocks_try() {
        let dir = tempdir().unwrap();
        let side = dir.path().join("a.md.moraine.json");
        let _a = SidecarLock::acquire(&side).unwrap();
        let err = SidecarLock::try_acquire(&side).unwrap_err();
        assert!(matches!(err, Error::LedgerBusy(_)));
    }

    #[test]
    fn concurrent_locked_writes_preserve_both_updates() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("c.md.moraine.json");
        write_atomic(&path, b"0").unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let path1 = path.clone();
        let path2 = path.clone();
        let b1 = barrier.clone();
        let b2 = barrier;

        let t1 = thread::spawn(move || {
            let _lock = SidecarLock::acquire(&path1).unwrap();
            b1.wait();
            let cur = fs::read_to_string(&path1).unwrap();
            let n: u32 = cur.parse().unwrap();
            // Hold briefly so the other waiter serializes.
            thread::sleep(std::time::Duration::from_millis(30));
            write_atomic(&path1, (n + 1).to_string().as_bytes()).unwrap();
        });
        let t2 = thread::spawn(move || {
            b2.wait();
            let _lock = SidecarLock::acquire(&path2).unwrap();
            let cur = fs::read_to_string(&path2).unwrap();
            let n: u32 = cur.parse().unwrap();
            write_atomic(&path2, (n + 1).to_string().as_bytes()).unwrap();
        });
        t1.join().unwrap();
        t2.join().unwrap();
        let final_v: u32 = fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(final_v, 2);
    }
}
