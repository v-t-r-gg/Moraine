use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use tokio::{io::AsyncReadExt, net::UnixListener, sync::Notify};
use tracing::{error, info};

pub(crate) struct UnixCaptureListener {
    listener: UnixListener,
    socket_path: PathBuf,
}

struct SocketCleanup(PathBuf);

impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl UnixCaptureListener {
    pub(super) async fn bind(socket_path: &Path) -> Result<Self> {
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::remove_file(socket_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(socket_path)?;
        info!(socket=%socket_path.display(), "unix socket bound");
        Ok(Self {
            listener,
            socket_path: socket_path.to_path_buf(),
        })
    }

    pub(super) async fn run(self, spool_dir: PathBuf, shutdown: Arc<Notify>) -> Result<()> {
        let _cleanup = SocketCleanup(self.socket_path.clone());
        tokio::fs::create_dir_all(spool_dir.join("processed")).await?;
        tokio::fs::create_dir_all(spool_dir.join("failed")).await?;

        loop {
            tokio::select! {
                accepted = self.listener.accept() => {
                    let (stream, _) = accepted?;
                    let mut buf = Vec::new();
                    let mut limited = stream.take((moraine_service::MAX_EVENT_BYTES + 1) as u64);
                    match limited.read_to_end(&mut buf).await {
                        Ok(_) => match moraine_service::write_spooled_payload(&spool_dir, &buf).await {
                            Ok(path) => info!(file=%path.display(), "spooled event"),
                            Err(error) => error!(%error, "failed to spool payload"),
                        },
                        Err(error) => error!(%error, "failed to read unix socket payload"),
                    }
                }
                _ = shutdown.notified() => {
                    info!(socket=%self.socket_path.display(), "shutting down unix listener");
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn bound_listener_spools_and_removes_socket_on_shutdown() {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("capture.sock");
        let spool = temp.path().join("spool");
        let listener = UnixCaptureListener::bind(&socket).await.unwrap();
        let shutdown = Arc::new(Notify::new());
        let task = tokio::spawn(listener.run(spool.clone(), shutdown.clone()));

        let mut stream = tokio::net::UnixStream::connect(&socket).await.unwrap();
        stream
            .write_all(br#"{"eventId":"capture-test"}"#)
            .await
            .unwrap();
        drop(stream);

        for _ in 0..50 {
            if spool.join("event-id-capture-test.json").exists() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(spool.join("event-id-capture-test.json").exists());

        shutdown.notify_one();
        task.await.unwrap().unwrap();
        assert!(!socket.exists());
    }
}
