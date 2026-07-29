use std::{
    os::unix::{
        fs::{FileTypeExt, MetadataExt},
        net::UnixStream as StdUnixStream,
    },
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use tokio::{io::AsyncReadExt, net::UnixListener, sync::Notify};
use tracing::{error, info};

#[derive(Debug)]
pub struct UnixCaptureListener {
    listener: UnixListener,
    socket_path: PathBuf,
    _cleanup: SocketCleanup,
}

#[derive(Debug)]
struct SocketCleanup {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.file_type().is_socket()
            && metadata.dev() == self.device
            && metadata.ino() == self.inode
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl UnixCaptureListener {
    pub(super) async fn bind(socket_path: &Path) -> Result<Self> {
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::symlink_metadata(socket_path) {
            Ok(metadata) if !metadata.file_type().is_socket() => {
                anyhow::bail!(
                    "refusing to replace non-socket capture endpoint {}",
                    socket_path.display()
                );
            }
            Ok(_) => match StdUnixStream::connect(socket_path) {
                Ok(_) => {
                    anyhow::bail!(
                        "capture endpoint already active at {}",
                        socket_path.display()
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(socket_path)?;
                }
                Err(error) => {
                    return Err(anyhow::anyhow!(
                        "could not confirm stale capture endpoint {}: {error}",
                        socket_path.display()
                    ));
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(socket_path)?;
        let metadata = std::fs::symlink_metadata(socket_path)?;
        let cleanup = SocketCleanup {
            path: socket_path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        info!(socket=%socket_path.display(), "unix socket bound");
        Ok(Self {
            listener,
            socket_path: socket_path.to_path_buf(),
            _cleanup: cleanup,
        })
    }

    pub(super) async fn run(self, spool_dir: PathBuf, shutdown: Arc<Notify>) -> Result<()> {
        tokio::fs::create_dir_all(spool_dir.join("processed")).await?;
        tokio::fs::create_dir_all(spool_dir.join("failed")).await?;

        loop {
            tokio::select! {
                accepted = self.listener.accept() => {
                    let (stream, _) = accepted?;
                    let mut buf = Vec::new();
                    let mut limited = stream.take((crate::MAX_EVENT_BYTES + 1) as u64);
                    match limited.read_to_end(&mut buf).await {
                        Ok(_) => match crate::write_spooled_payload(&spool_dir, &buf).await {
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

        let spooled = spool.join("event-id-capture-test.json");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !spooled.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("listener should materialize the payload before shutdown");
        assert!(spooled.exists());

        shutdown.notify_one();
        task.await.unwrap().unwrap();
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn active_socket_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("capture.sock");
        let first = UnixCaptureListener::bind(&socket).await.unwrap();
        let error = UnixCaptureListener::bind(&socket).await.unwrap_err();
        assert!(error.to_string().contains("already active"));
        assert!(tokio::net::UnixStream::connect(&socket).await.is_ok());
        drop(first);
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn regular_file_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("capture.sock");
        std::fs::write(&socket, b"do not remove").unwrap();
        let error = UnixCaptureListener::bind(&socket).await.unwrap_err();
        assert!(error.to_string().contains("non-socket"));
        assert_eq!(std::fs::read(&socket).unwrap(), b"do not remove");
    }

    #[tokio::test]
    async fn confirmed_stale_socket_is_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("capture.sock");
        drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
        assert!(socket.exists());
        let replacement = UnixCaptureListener::bind(&socket).await.unwrap();
        assert!(tokio::net::UnixStream::connect(&socket).await.is_ok());
        drop(replacement);
        assert!(!socket.exists());
    }
}
