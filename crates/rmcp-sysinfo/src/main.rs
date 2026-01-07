//! rmcp-sysinfo: MCP server for system information
//!
//! Run with: `rmcp-sysinfo` (serves on stdio)

use rmcp::ServiceExt;
use rmcp_sysinfo::SysinfoServer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (to stderr so it doesn't interfere with stdio transport)
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting rmcp-sysinfo server");

    // Create server and serve on stdio
    let server = SysinfoServer::new();
    let service = server.serve(rmcp::transport::stdio()).await?;

    // Wait for shutdown
    service.waiting().await?;

    tracing::info!("rmcp-sysinfo server stopped");
    Ok(())
}
