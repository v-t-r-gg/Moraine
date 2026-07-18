use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use moraine_core::history::HistorySource;
use moraine_core::{
    Document, DocumentId, DocumentSnapshot, FileWatcher, HistoryStore, MorainePaths, WatchEvent,
};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

pub struct AppState {
    pub paths: MorainePaths,
    pub history: HistoryStore,
    documents: Mutex<HashMap<DocumentId, Document>>,
    by_path: Mutex<HashMap<PathBuf, DocumentId>>,
    watcher: Mutex<Option<FileWatcher>>,
    /// Counter of FS events to ignore per path after our own writes.
    suppress_watch: Mutex<HashMap<PathBuf, u32>>,
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
            pending_open: Mutex::new(resolve_startup_path()),
        })
    }

    /// Path from CLI args or `MORAINE_OPEN` (consumed once by the UI).
    pub fn take_pending_open(&self) -> Option<PathBuf> {
        self.pending_open.lock().take()
    }

    pub fn open_path(&self, path: PathBuf) -> moraine_core::Result<DocumentSnapshot> {
        let abs = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(id) = self.by_path.lock().get(&abs).copied() {
            if let Some(doc) = self.documents.lock().get(&id) {
                return Ok(doc.snapshot());
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
        self.bump_suppress(&path);
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

        Ok(doc.snapshot())
    }

    pub fn reload(&self, id: DocumentId) -> moraine_core::Result<DocumentSnapshot> {
        let mut docs = self.documents.lock();
        let doc = docs
            .get_mut(&id)
            .ok_or_else(|| moraine_core::Error::other(format!("document {id} not open")))?;
        doc.reload()?;
        Ok(doc.snapshot())
    }

    pub fn close(&self, id: DocumentId) {
        if let Some(doc) = self.documents.lock().remove(&id) {
            self.by_path.lock().remove(doc.path());
        }
    }

    pub fn list_open(&self) -> Vec<DocumentSnapshot> {
        self.documents
            .lock()
            .values()
            .map(|d| d.snapshot())
            .collect()
    }

    fn bump_suppress(&self, path: &std::path::Path) {
        let mut map = self.suppress_watch.lock();
        *map.entry(path.to_path_buf()).or_insert(0) += 1;
    }

    fn should_suppress(&self, path: &std::path::Path) -> bool {
        let mut map = self.suppress_watch.lock();
        if let Some(count) = map.get_mut(path) {
            if *count > 0 {
                *count -= 1;
                if *count == 0 {
                    map.remove(path);
                }
                return true;
            }
        }
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(count) = map.get_mut(&canon) {
            if *count > 0 {
                *count -= 1;
                if *count == 0 {
                    map.remove(&canon);
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

fn spawn_watch_bridge(app: AppHandle, rx: Receiver<WatchEvent>) {
    thread::Builder::new()
        .name("moraine-watch-bridge".into())
        .spawn(move || {
            while let Ok(event) = rx.recv() {
                // Let save's suppress token land before we process the event.
                thread::sleep(Duration::from_millis(30));
                let state = app.state::<AppState>();
                if state.should_suppress(&event.path) {
                    continue;
                }

                let abs = std::fs::canonicalize(&event.path).unwrap_or(event.path.clone());
                let open_id = state.by_path.lock().get(&abs).copied();

                let payload = FileChangedPayload {
                    path: event.path.display().to_string(),
                    change: format!("{:?}", event.change).to_lowercase(),
                    document_id: open_id.map(|id| id.to_string()),
                };

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
}

fn resolve_startup_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MORAINE_OPEN") {
        let path = PathBuf::from(p.trim());
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    // Skip binary name; first non-flag arg is a file path.
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .map(PathBuf::from)
}
