mod config;
mod error;
mod grok;
mod server;
mod tools;

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Config;
use crate::server::GrokSearchServer;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting Grok Search MCP Server v{}", env!("CARGO_PKG_VERSION"));

    let config = Config::load()?;
    tracing::debug!("Configuration loaded: model={}", config.model);

    let server = GrokSearchServer::new(config);
    let service = server.serve(stdio()).await?;

    tokio::select! {
        result = service.waiting() => {
            if let Err(e) = result {
                tracing::warn!("Service ended with error: {}", e);
            }
        }
        _ = shutdown_signal() => {
            tracing::info!("Shutdown signal received");
        }
        _ = parent_process_exited() => {
            tracing::info!("Parent process exited");
        }
    }

    tracing::info!("Grok Search MCP Server stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn parent_process_exited() {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use tokio::time::{interval, Duration};
        use windows_sys::Win32::Foundation::{
            GetLastError, ERROR_BROKEN_PIPE, ERROR_INVALID_HANDLE, ERROR_NO_DATA,
        };
        use windows_sys::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_PIPE};

        let stdin_handle = std::io::stdin().as_raw_handle() as *mut std::ffi::c_void;

        // Only monitor if stdin is a pipe
        let file_type = unsafe { GetFileType(stdin_handle) };
        if file_type != FILE_TYPE_PIPE {
            tracing::debug!("Stdin is not a pipe, skipping parent process monitor");
            return std::future::pending::<()>().await;
        }

        let mut check = interval(Duration::from_millis(500));
        loop {
            check.tick().await;
            let mut available: u32 = 0;
            let result = unsafe {
                windows_sys::Win32::System::Pipes::PeekNamedPipe(
                    stdin_handle,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if result == 0 {
                let err = unsafe { GetLastError() };
                match err {
                    ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_INVALID_HANDLE => break,
                    _ => {}
                }
            }
        }
    }

    #[cfg(not(windows))]
    std::future::pending::<()>().await
}
