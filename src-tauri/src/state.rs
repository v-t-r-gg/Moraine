use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use moraine_core::history::HistorySource;
use moraine_core::{
    content_hash, Document, DocumentId, DocumentSnapshot, FileWatcher, HistoryStore, MorainePaths,
    WatchEvent,
};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info, warn};

pub struct AppState {
    pub paths: MorainePaths,
    pub history: HistoryStore,
    documents: Mutex<HashMap<DocumentId, Document>>,
    by_path: Mutex<HashMap<PathBuf, DocumentId>>,
    watcher: Mutex<Option<FileWatcher>>,
    /// Soft suppress for FS bursts after our own writes (optimization only).
    suppress_watch: Mutex<HashMap<PathBuf, u32>>,
    /// Last known persisted Markdown content hash per path (authoritative after open/save/reload).
    known_content_hash: Mutex<HashMap<PathBuf, String>>,
    /// Last external hash we already emitted for this path (dedupe).
    last_emitted_external_hash: Mutex<HashMap<PathBuf, String>>,
    pending_open: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> moraine_core::Result<Self> {
        let paths = MorainePaths::default_ensure()?;
        let history = HistoryStore::new(paths.clone()).with_max_entries(100);
        Ok(Self {
            paths,
            history,
            documents: Mutex::new(HashMap::new()),
            by_path: Mutex::new(HashMap::new()),
            watcher: Mutex::new(None),
            suppress_watch: Mutex::new(HashMap::new()),
            known_content_hash: Mutex::new(HashMap::new()),
            last_emitted_external_hash: Mutex::new(HashMap::new()),
            pending_open: Mutex::new(resolve_startup_path()),
        })
    }

    pub fn take_pending_open(&self) -> Option<PathBuf> {
        self.pending_open.lock().take()
    }

    fn remember_hash(&self, path: &Path, hash: &str) {
        let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.known_content_hash.lock().insert(abs, hash.to_string());
    }

    pub fn open_path(&self, path: PathBuf) -> moraine_core::Result<DocumentSnapshot> {
        let abs = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(id) = self.by_path.lock().get(&abs).copied() {
            if let Some(doc) = self.documents.lock().get(&id) {
                let snap = doc.snapshot();
                self.remember_hash(doc.path(), &snap.content_hash);
                return Ok(snap);
            }
        }

        let doc = if abs.exists() {
            Document::open(&abs)?
        } else {
            Document::create(&abs, "# New document\n\n")?
        };

        let snap = doc.snapshot();
        let id = doc.id();
        let path = doc.path().to_path_buf();
        self.remember_hash(&path, &snap.content_hash);

        self.history
            .push(&path, doc.content(), HistorySource::Open, None)?;

        self.by_path.lock().insert(path.clone(), id);
        self.documents.lock().insert(id, doc);

        if let Some(w) = self.watcher.lock().as_ref() {
            if let Err(e) = w.watch(&path) {
                warn!("failed to watch {}: {e}", path.display());
            }
        }

        Ok(snap)
    }

    pub fn get_snapshot(&self, id: DocumentId) -> moraine_core::Result<DocumentSnapshot> {
        self.documents
            .lock()
            .get(&id)
            .map(|d| d.snapshot())
            .ok_or_else(|| moraine_core::Error::other(format!("document {id} not open")))
    }

    pub fn set_content(&self, id: DocumentId, content: String) -> moraine_core::Result<()> {
        let mut docs = self.documents.lock();
        let doc = docs
            .get_mut(&id)
            .ok_or_else(|| moraine_core::Error::other(format!("document {id} not open")))?;
        doc.set_content(content);
        Ok(())
    }

    pub fn save(
        &self,
        id: DocumentId,
        content: Option<String>,
        record_history: bool,
        expected_content_hash: Option<String>,
    ) -> moraine_core::Result<DocumentSnapshot> {
        let mut docs = self.documents.lock();
        let doc = docs
            .get_mut(&id)
            .ok_or_else(|| moraine_core::Error::other(format!("document {id} not open")))?;

        if let Some(c) = content {
            doc.set_content(c);
        }

        let path = doc.path().to_path_buf();
        // Soft suppress a burst of FS events from atomic replace (not the correctness boundary).
        self.bump_suppress(&path, 8);
        if let Some(expected) = expected_content_hash.as_deref() {
            if !expected.is_empty() {
                doc.save_if_base_matches(expected)?;
            } else {
                doc.save()?;
            }
        } else {
            doc.save()?;
        }

        if record_history {
            self.history
                .push(&path, doc.content(), HistorySource::AutoSave, None)?;
        }

        let snap = doc.snapshot();
        // Establish new base hash before watcher classification can race.
        self.remember_hash(&path, &snap.content_hash);
        debug!(
            path = %path.display(),
            hash = %&snap.content_hash[..12.min(snap.content_hash.len())],
            "save completed; known hash updated"
        );
        Ok(snap)
    }

    pub fn reload(&self, id: DocumentId) -> moraine_core::Result<DocumentSnapshot> {
        let mut docs = self.documents.lock();
        let doc = docs
            .get_mut(&id)
            .ok_or_else(|| moraine_core::Error::other(format!("document {id} not open")))?;
        doc.reload()?;
        let snap = doc.snapshot();
        self.remember_hash(doc.path(), &snap.content_hash);
        Ok(snap)
    }

    pub fn close(&self, id: DocumentId) {
        if let Some(doc) = self.documents.lock().remove(&id) {
            let path = doc.path().to_path_buf();
            self.by_path.lock().remove(&path);
            self.known_content_hash.lock().remove(&path);
            self.last_emitted_external_hash.lock().remove(&path);
        }
    }

    pub fn list_open(&self) -> Vec<DocumentSnapshot> {
        self.documents
            .lock()
            .values()
            .map(|d| d.snapshot())
            .collect()
    }

    fn bump_suppress(&self, path: &Path, n: u32) {
        let mut map = self.suppress_watch.lock();
        let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        *map.entry(abs).or_insert(0) += n;
    }

    fn should_suppress(&self, path: &Path) -> bool {
        let mut map = self.suppress_watch.lock();
        let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(count) = map.get_mut(&abs) {
            if *count > 0 {
                *count -= 1;
                if *count == 0 {
                    map.remove(&abs);
                }
                return true;
            }
        }
        false
    }

    pub fn start_watcher(app: AppHandle) {
        let state = app.state::<AppState>();
        match FileWatcher::start() {
            Ok((watcher, rx)) => {
                *state.watcher.lock() = Some(watcher);
                spawn_watch_bridge(app.clone(), rx);
                info!("file watcher started");
            }
            Err(e) => {
                error!("failed to start file watcher: {e}");
            }
        }
    }
}

fn is_sidecar_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.ends_with(".moraine.json")
        || s.ends_with(".moraine.json.lock")
        || s.ends_with(".comments.json")
        || s.ends_with(".comments.json.migrated")
        || s.contains(".tmp")
}

fn spawn_watch_bridge(app: AppHandle, rx: Receiver<WatchEvent>) {
    thread::Builder::new()
        .name("moraine-watch-bridge".into())
        .spawn(move || {
            while let Ok(event) = rx.recv() {
                // Soft delay so save's suppress tokens and known-hash update can land.
                thread::sleep(Duration::from_millis(30));
                let state = app.state::<AppState>();

                if is_sidecar_path(&event.path) {
                    debug!(path = %event.path.display(), "ignore sidecar watch event");
                    continue;
                }

                let soft_suppressed = state.should_suppress(&event.path);

                let abs = std::fs::canonicalize(&event.path).unwrap_or(event.path.clone());
                let open_id = state.by_path.lock().get(&abs).copied();

                // Only open Markdown documents participate in content classification.
                let disk_hash = match Document::read_file(&abs) {
                    Ok(body) => content_hash(&body),
                    Err(e) => {
                        debug!(path = %abs.display(), error = %e, "watch: cannot read path");
                        continue;
                    }
                };

                let known = state.known_content_hash.lock().get(&abs).cloned();
                let content_changed = known.as_ref() != Some(&disk_hash);

                if !content_changed {
                    debug!(
                        path = %abs.display(),
                        hash = %&disk_hash[..12.min(disk_hash.len())],
                        soft_suppressed,
                        "watch: same hash as known; ignore"
                    );
                    continue;
                }

                if let Some(prev) = state.last_emitted_external_hash.lock().get(&abs) {
                    if prev == &disk_hash {
                        debug!(path = %abs.display(), "watch: duplicate external hash; ignore");
                        continue;
                    }
                }

                state
                    .last_emitted_external_hash
                    .lock()
                    .insert(abs.clone(), disk_hash.clone());

                let payload = FileChangedPayload {
                    path: event.path.display().to_string(),
                    change: format!("{:?}", event.change).to_lowercase(),
                    document_id: open_id.map(|id| id.to_string()),
                    disk_content_hash: Some(disk_hash.clone()),
                    known_content_hash: known,
                    content_changed: true,
                };

                debug!(
                    path = %payload.path,
                    content_changed = payload.content_changed,
                    soft_suppressed,
                    "watch: emit revision change"
                );

                if let Err(e) = app.emit("file-changed", &payload) {
                    warn!("emit file-changed failed: {e}");
                }
            }
        })
        .expect("spawn watch bridge");
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FileChangedPayload {
    path: String,
    change: String,
    document_id: Option<String>,
    disk_content_hash: Option<String>,
    known_content_hash: Option<String>,
    content_changed: bool,
}

fn resolve_startup_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MORAINE_OPEN") {
        let path = PathBuf::from(p.trim());
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .map(PathBuf::from)
}
