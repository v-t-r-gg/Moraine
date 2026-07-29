//! Platform-dispatched capture listener binding.

#[cfg(target_os = "linux")]
pub mod linux_unix;
#[cfg(target_os = "windows")]
pub mod windows_named_pipe;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use moraine_platform::CaptureEndpoint;
use tokio::sync::Notify;

pub enum BoundCaptureListener {
    #[cfg(target_os = "linux")]
    Unix(linux_unix::UnixCaptureListener),
    #[cfg(target_os = "windows")]
    WindowsNamedPipe(windows_named_pipe::WindowsNamedPipeCaptureListener),
    #[allow(
        dead_code,
        reason = "explicit non-Linux backend slot; bind fails before constructing it until W2"
    )]
    Unsupported,
}

pub async fn bind(endpoint: &CaptureEndpoint) -> Result<BoundCaptureListener> {
    match endpoint {
        #[cfg(target_os = "linux")]
        CaptureEndpoint::UnixSocket(path) => Ok(BoundCaptureListener::Unix(
            linux_unix::UnixCaptureListener::bind(path).await?,
        )),
        #[cfg(target_os = "windows")]
        CaptureEndpoint::WindowsNamedPipe(name) => Ok(BoundCaptureListener::WindowsNamedPipe(
            windows_named_pipe::WindowsNamedPipeCaptureListener::bind(name)?,
        )),
        endpoint => anyhow::bail!("unsupported capture endpoint for moraine-service: {endpoint:?}"),
    }
}

impl BoundCaptureListener {
    pub async fn run(self, spool_dir: PathBuf, shutdown: Arc<Notify>) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Unix(listener) => listener.run(spool_dir, shutdown).await,
            #[cfg(target_os = "windows")]
            Self::WindowsNamedPipe(listener) => listener.run(spool_dir, shutdown).await,
            Self::Unsupported => {
                let _ = (spool_dir, shutdown);
                anyhow::bail!("unsupported capture endpoint for moraine-service")
            }
        }
    }
}
