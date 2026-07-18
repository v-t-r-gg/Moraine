use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::paths::MorainePaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub label: Option<String>,
    pub content: String,
    pub content_hash: u64,
    pub source: HistorySource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HistorySource {
    Manual,
    AutoSave,
    External,
    Open,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryLog {
    document_path: PathBuf,
    entries: Vec<HistoryEntry>,
    max_entries: usize,
}

#[derive(Debug)]
pub struct HistoryStore {
    paths: MorainePaths,
    max_entries: usize,
}

impl HistoryStore {
    pub fn new(paths: MorainePaths) -> Self {
        Self {
            paths,
            max_entries: 100,
        }
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max.max(1);
        self
    }

    /// Append a snapshot; skips if content hash matches the latest entry.
    pub fn push(
        &self,
        document_path: &Path,
        content: &str,
        source: HistorySource,
        label: Option<String>,
    ) -> Result<HistoryEntry> {
        let abs = canonicalize_or_keep(document_path);
        let mut log = self.load_or_create(&abs)?;

        let hash = content_hash(content);
        if let Some(last) = log.entries.last() {
            if last.content_hash == hash {
                return Ok(last.clone());
            }
        }

        let entry = HistoryEntry {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            label,
            content: content.to_owned(),
            content_hash: hash,
            source,
        };

        log.entries.push(entry.clone());
        if log.entries.len() > log.max_entries {
            let excess = log.entries.len() - log.max_entries;
            log.entries.drain(0..excess);
        }

        self.persist(&log)?;
        Ok(entry)
    }

    pub fn list(&self, document_path: &Path) -> Result<Vec<HistoryEntry>> {
        let abs = canonicalize_or_keep(document_path);
        let mut log = self.load_or_create(&abs)?;
        log.entries.reverse();
        Ok(log.entries)
    }

    pub fn list_meta(&self, document_path: &Path) -> Result<Vec<HistoryEntryMeta>> {
        Ok(self
            .list(document_path)?
            .into_iter()
            .map(HistoryEntryMeta::from)
            .collect())
    }

    pub fn get(&self, document_path: &Path, entry_id: Uuid) -> Result<HistoryEntry> {
        self.list(document_path)?
            .into_iter()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| Error::History(format!("history entry {entry_id} not found")))
    }

    pub fn restore_content(&self, document_path: &Path, entry_id: Uuid) -> Result<String> {
        Ok(self.get(document_path, entry_id)?.content)
    }

    fn history_path(&self, abs_document: &Path) -> PathBuf {
        self.paths.history_file_for(abs_document)
    }

    fn load_or_create(&self, abs: &Path) -> Result<HistoryLog> {
        let path = self.history_path(abs);
        if path.exists() {
            let raw = fs::read_to_string(&path)?;
            let log: HistoryLog = serde_json::from_str(&raw)?;
            Ok(log)
        } else {
            Ok(HistoryLog {
                document_path: abs.to_path_buf(),
                entries: Vec::new(),
                max_entries: self.max_entries,
            })
        }
    }

    fn persist(&self, log: &HistoryLog) -> Result<()> {
        let path = self.history_path(&log.document_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(log)?;
        fs::write(path, raw)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryMeta {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub label: Option<String>,
    pub content_hash: u64,
    pub source: HistorySource,
    pub byte_len: usize,
}

impl From<HistoryEntry> for HistoryEntryMeta {
    fn from(e: HistoryEntry) -> Self {
        Self {
            id: e.id,
            created_at: e.created_at,
            label: e.label,
            content_hash: e.content_hash,
            source: e.source,
            byte_len: e.content.len(),
        }
    }
}

fn content_hash(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    content.hash(&mut h);
    h.finish()
}

fn canonicalize_or_keep(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn store(tmp: &tempfile::TempDir) -> HistoryStore {
        let data = tmp.path().join("data");
        let history = data.join("history");
        let config = tmp.path().join("config");
        fs::create_dir_all(&history).unwrap();
        fs::create_dir_all(&config).unwrap();
        HistoryStore::new(MorainePaths {
            data_dir: data,
            history_dir: history,
            config_dir: config,
        })
        .with_max_entries(5)
    }

    #[test]
    fn dedupes_identical_pushes() {
        let tmp = tempdir().unwrap();
        let s = store(&tmp);
        let path = tmp.path().join("doc.md");
        fs::write(&path, "a").unwrap();

        let e1 = s
            .push(&path, "hello", HistorySource::AutoSave, None)
            .unwrap();
        let e2 = s
            .push(&path, "hello", HistorySource::AutoSave, None)
            .unwrap();
        assert_eq!(e1.id, e2.id);
        assert_eq!(s.list(&path).unwrap().len(), 1);
    }

    #[test]
    fn caps_entries() {
        let tmp = tempdir().unwrap();
        let s = store(&tmp);
        let path = tmp.path().join("doc.md");
        fs::write(&path, "a").unwrap();

        for i in 0..10 {
            s.push(&path, &format!("v{i}"), HistorySource::Manual, None)
                .unwrap();
        }
        assert_eq!(s.list(&path).unwrap().len(), 5);
    }
}
