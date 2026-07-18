use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic::write_atomic;
use crate::error::{Error, Result};

pub type DocumentId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMeta {
    pub id: DocumentId,
    pub path: PathBuf,
    pub title: String,
    pub dirty: bool,
    pub last_saved_at: Option<DateTime<Utc>>,
    pub last_modified_on_disk: Option<DateTime<Utc>>,
    pub byte_len: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSnapshot {
    pub meta: DocumentMeta,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct Document {
    meta: DocumentMeta,
    content: String,
}

impl Document {
    pub fn new_untitled(content: impl Into<String>) -> Self {
        let content = content.into();
        let id = Uuid::new_v4();
        Self {
            meta: DocumentMeta {
                id,
                path: PathBuf::from("untitled.md"),
                title: "untitled.md".into(),
                dirty: true,
                last_saved_at: None,
                last_modified_on_disk: None,
                byte_len: content.len() as u64,
            },
            content,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = fs::canonicalize(path.as_ref()).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound(path.as_ref().to_path_buf())
            } else {
                Error::Io(e)
            }
        })?;

        if !path.is_file() {
            return Err(Error::NotAFile(path));
        }

        let bytes = fs::read(&path)?;
        let content = String::from_utf8(bytes).map_err(|_| Error::InvalidUtf8(path.clone()))?;
        let mtime = file_mtime(&path)?;
        let title = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document.md".into());

        Ok(Self {
            meta: DocumentMeta {
                id: Uuid::new_v4(),
                path,
                title,
                dirty: false,
                last_saved_at: Some(Utc::now()),
                last_modified_on_disk: mtime,
                byte_len: content.len() as u64,
            },
            content,
        })
    }

    pub fn create(path: impl AsRef<Path>, content: impl Into<String>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let content = content.into();
        fs::write(path, &content)?;
        Self::open(path)
    }

    pub fn id(&self) -> DocumentId {
        self.meta.id
    }

    pub fn path(&self) -> &Path {
        &self.meta.path
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn meta(&self) -> &DocumentMeta {
        &self.meta
    }

    pub fn is_dirty(&self) -> bool {
        self.meta.dirty
    }

    pub fn set_content(&mut self, content: impl Into<String>) {
        let content = content.into();
        if content != self.content {
            self.content = content;
            self.meta.byte_len = self.content.len() as u64;
            self.meta.dirty = true;
        }
    }

    pub fn save(&mut self) -> Result<()> {
        write_atomic(&self.meta.path, self.content.as_bytes())?;
        self.meta.dirty = false;
        self.meta.last_saved_at = Some(Utc::now());
        self.meta.last_modified_on_disk = file_mtime(&self.meta.path)?;
        self.meta.byte_len = self.content.len() as u64;
        Ok(())
    }

    /// Save only if on-disk Markdown hash still equals `expected_disk_hash`.
    pub fn save_if_base_matches(&mut self, expected_disk_hash: &str) -> Result<()> {
        if self.meta.path.exists() {
            crate::run_meta::assert_disk_revision(&self.meta.path, expected_disk_hash)?;
        }
        self.save()
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        write_atomic(path, self.content.as_bytes())?;
        let path = fs::canonicalize(path)?;
        self.meta.path = path;
        self.meta.title = self
            .meta
            .path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document.md".into());
        self.meta.dirty = false;
        self.meta.last_saved_at = Some(Utc::now());
        self.meta.last_modified_on_disk = file_mtime(&self.meta.path)?;
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        let bytes = fs::read(&self.meta.path)?;
        let content =
            String::from_utf8(bytes).map_err(|_| Error::InvalidUtf8(self.meta.path.clone()))?;
        self.content = content;
        self.meta.dirty = false;
        self.meta.last_saved_at = Some(Utc::now());
        self.meta.last_modified_on_disk = file_mtime(&self.meta.path)?;
        self.meta.byte_len = self.content.len() as u64;
        Ok(())
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            meta: self.meta.clone(),
            content: self.content.clone(),
        }
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<String> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound(path.to_path_buf())
            } else {
                Error::Io(e)
            }
        })?;
        String::from_utf8(bytes).map_err(|_| Error::InvalidUtf8(path.to_path_buf()))
    }

    /// Write via unique same-directory temp file + safe replace (no truncate fallback).
    pub fn write_file(path: impl AsRef<Path>, content: &str) -> Result<()> {
        write_atomic(path.as_ref(), content.as_bytes())
    }
}

fn file_mtime(path: &Path) -> Result<Option<DateTime<Utc>>> {
    let meta = fs::metadata(path)?;
    let modified = meta.modified().ok();
    Ok(modified.map(system_time_to_utc))
}

fn system_time_to_utc(t: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn open_save_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "# Hello\n").unwrap();

        let mut doc = Document::open(&path).unwrap();
        assert_eq!(doc.content(), "# Hello\n");
        assert!(!doc.is_dirty());

        doc.set_content("# Hello world\n");
        assert!(doc.is_dirty());
        doc.save().unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "# Hello world\n");
    }

    #[test]
    fn write_file_atomic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.md");
        Document::write_file(&path, "alpha").unwrap();
        assert_eq!(Document::read_file(&path).unwrap(), "alpha");
        Document::write_file(&path, "beta").unwrap();
        assert_eq!(Document::read_file(&path).unwrap(), "beta");
    }
}
