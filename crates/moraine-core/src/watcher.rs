use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileChange {
    Create,
    Modify,
    Remove,
    Rename,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEvent {
    pub path: PathBuf,
    pub change: FileChange,
}

pub struct FileWatcher {
    inner: Mutex<Option<RecommendedWatcher>>,
    watched: Mutex<Vec<PathBuf>>,
    tx: Sender<WatchEvent>,
}

impl FileWatcher {
    /// Spawns a background thread that debounces notify bursts (~150ms).
    pub fn start() -> Result<(Self, Receiver<WatchEvent>)> {
        let (out_tx, out_rx) = mpsc::channel::<WatchEvent>();
        let (raw_tx, raw_rx) = mpsc::channel::<notify::Result<Event>>();

        let watcher = notify::recommended_watcher(move |res| {
            let _ = raw_tx.send(res);
        })
        .map_err(|e| Error::Watcher(e.to_string()))?;

        let out_tx_thread = out_tx.clone();
        thread::Builder::new()
            .name("moraine-watcher".into())
            .spawn(move || debounce_loop(raw_rx, out_tx_thread))
            .map_err(|e| Error::Watcher(e.to_string()))?;

        Ok((
            Self {
                inner: Mutex::new(Some(watcher)),
                watched: Mutex::new(Vec::new()),
                tx: out_tx,
            },
            out_rx,
        ))
    }

    pub fn watch(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        let abs = std::fs::canonicalize(&path).unwrap_or(path);

        let mode = if abs.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        let mut inner = self.inner.lock();
        let watcher = inner
            .as_mut()
            .ok_or_else(|| Error::Watcher("watcher stopped".into()))?;
        watcher
            .watch(&abs, mode)
            .map_err(|e| Error::Watcher(e.to_string()))?;

        self.watched.lock().push(abs);
        Ok(())
    }

    pub fn unwatch(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        let abs = std::fs::canonicalize(&path).unwrap_or(path);

        let mut inner = self.inner.lock();
        if let Some(watcher) = inner.as_mut() {
            let _ = watcher.unwatch(&abs);
        }
        self.watched.lock().retain(|p| p != &abs);
        Ok(())
    }

    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched.lock().clone()
    }

    pub fn inject(&self, event: WatchEvent) {
        let _ = self.tx.send(event);
    }
}

fn debounce_loop(raw_rx: Receiver<notify::Result<Event>>, out_tx: Sender<WatchEvent>) {
    let debounce = Duration::from_millis(150);
    let mut pending: Vec<WatchEvent> = Vec::new();

    loop {
        let first = match raw_rx.recv() {
            Ok(Ok(ev)) => map_event(ev),
            Ok(Err(_)) => continue,
            Err(_) => break,
        };
        pending.extend(first);

        let deadline = std::time::Instant::now() + debounce;
        while let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) {
            match raw_rx.recv_timeout(remaining) {
                Ok(Ok(ev)) => pending.extend(map_event(ev)),
                Ok(Err(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    flush(&out_tx, &mut pending);
                    return;
                }
            }
        }

        flush(&out_tx, &mut pending);
    }
}

fn flush(tx: &Sender<WatchEvent>, pending: &mut Vec<WatchEvent>) {
    let mut map = std::collections::BTreeMap::<PathBuf, FileChange>::new();
    for ev in pending.drain(..) {
        if is_markdown_or_dir(&ev.path) {
            map.insert(ev.path, ev.change);
        }
    }
    for (path, change) in map {
        if tx.send(WatchEvent { path, change }).is_err() {
            return;
        }
    }
}

fn map_event(event: Event) -> Vec<WatchEvent> {
    let change = match event.kind {
        EventKind::Create(_) => FileChange::Create,
        EventKind::Modify(_) => FileChange::Modify,
        EventKind::Remove(_) => FileChange::Remove,
        EventKind::Any | EventKind::Access(_) | EventKind::Other => FileChange::Other,
    };

    event
        .paths
        .into_iter()
        .map(|path| WatchEvent { path, change })
        .collect()
}

fn is_markdown_or_dir(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            matches!(ext.as_str(), "md" | "markdown" | "mdx" | "mdown")
        }
        // Editors often emit rename events with no extension mid-swap.
        None => true,
    }
}
