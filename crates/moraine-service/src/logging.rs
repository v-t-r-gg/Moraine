//! Size-bounded UTF-8 application logs for the Windows background runtime.
#![cfg(any(target_os = "windows", test))]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};

pub const WINDOWS_LOG_FILE: &str = "moraine-service.log";
pub const WINDOWS_LOG_ROTATION_BYTES: u64 = 5 * 1024 * 1024;
pub const WINDOWS_LOG_ARCHIVES: usize = 3;

#[derive(Clone)]
#[cfg(target_os = "windows")]
struct RotatingMakeWriter {
    inner: Arc<Mutex<RotatingFile>>,
}

#[cfg(target_os = "windows")]
struct RotatingWriter {
    inner: Arc<Mutex<RotatingFile>>,
}

#[cfg(target_os = "windows")]
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingMakeWriter {
    type Writer = RotatingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        RotatingWriter {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(target_os = "windows")]
impl Write for RotatingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.inner
            .lock()
            .map_err(|_| io::Error::other("Windows log writer lock poisoned"))?
            .write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner
            .lock()
            .map_err(|_| io::Error::other("Windows log writer lock poisoned"))?
            .flush()
    }
}

struct RotatingFile {
    directory: PathBuf,
    file: File,
    length: u64,
    threshold: u64,
}

impl RotatingFile {
    fn open(directory: impl Into<PathBuf>, threshold: u64) -> io::Result<Self> {
        let directory = directory.into();
        fs::create_dir_all(&directory)?;
        let path = directory.join(WINDOWS_LOG_FILE);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let length = file.metadata()?.len();
        Ok(Self {
            directory,
            file,
            length,
            threshold,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let oldest = archive_path(&self.directory, WINDOWS_LOG_ARCHIVES);
        if oldest.exists() {
            fs::remove_file(oldest)?;
        }
        for archive in (1..WINDOWS_LOG_ARCHIVES).rev() {
            let from = archive_path(&self.directory, archive);
            if from.exists() {
                fs::rename(from, archive_path(&self.directory, archive + 1))?;
            }
        }
        let current = self.directory.join(WINDOWS_LOG_FILE);
        if current.exists() {
            fs::rename(&current, archive_path(&self.directory, 1))?;
        }
        self.file = OpenOptions::new().create(true).append(true).open(current)?;
        self.length = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.length > 0 && self.length.saturating_add(buffer.len() as u64) > self.threshold {
            self.rotate()?;
        }
        let written = self.file.write(buffer)?;
        self.length = self.length.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn archive_path(directory: &Path, archive: usize) -> PathBuf {
    directory.join(format!("{WINDOWS_LOG_FILE}.{archive}"))
}

#[cfg(target_os = "windows")]
pub fn init_windows_file_logging(directory: &Path) -> anyhow::Result<()> {
    let writer = RotatingMakeWriter {
        inner: Arc::new(Mutex::new(RotatingFile::open(
            directory,
            WINDOWS_LOG_ROTATION_BYTES,
        )?)),
    };
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(writer)
        .try_init()
        .map_err(|error| anyhow::anyhow!("initialize Windows runtime logging: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_retains_three_archives_and_current_file() {
        let temp = tempfile::tempdir().unwrap();
        let mut writer = RotatingFile::open(temp.path(), 8).unwrap();
        for value in ["one\n", "two\n", "three\n", "four\n", "five\n"] {
            writer.write_all(value.as_bytes()).unwrap();
        }
        writer.flush().unwrap();

        assert_eq!(
            fs::read_to_string(temp.path().join(WINDOWS_LOG_FILE)).unwrap(),
            "five\n"
        );
        assert_eq!(
            fs::read_to_string(archive_path(temp.path(), 1)).unwrap(),
            "four\n"
        );
        assert_eq!(
            fs::read_to_string(archive_path(temp.path(), 2)).unwrap(),
            "three\n"
        );
        assert_eq!(
            fs::read_to_string(archive_path(temp.path(), 3)).unwrap(),
            "one\ntwo\n"
        );
    }
}
