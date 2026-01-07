//! rmcp-git: Cross-platform MCP server for Git repository information
//!
//! Run with: `rmcp-git` (serves on stdio)

use rmcp::ServiceExt;
use rmcp_git::GitServer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting rmcp-git server");

    let server = GitServer::new();
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    tracing::info!("rmcp-git server stopped");
    Ok(())
}
