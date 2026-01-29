mod config;
mod error;
mod grok;
mod server;
mod tools;

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Config;
use crate::server::GrokSearchServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to stderr (stdout reserved for MCP protocol)
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting Grok Search MCP Server v{}", env!("CARGO_PKG_VERSION"));

    // Load and validate configuration
    let config = Config::load()?;
    tracing::debug!("Configuration loaded: model={}", config.model);

    // Create server and start serving
    let server = GrokSearchServer::new(config);
    let service = server.serve(stdio()).await?;

    // Wait for shutdown (handles SIGINT/SIGTERM on Unix, Ctrl+C on Windows)
    service.waiting().await?;

    tracing::info!("Grok Search MCP Server stopped");
    Ok(())
}
