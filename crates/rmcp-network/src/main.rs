//! rmcp-network: Cross-platform MCP server for network interface information
//!
//! Run with: `rmcp-network` (serves on stdio)

use rmcp::ServiceExt;
use rmcp_network::NetworkServer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (to stderr so it doesn't interfere with stdio transport)
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting rmcp-network server");

    // Create server and serve on stdio
    let server = NetworkServer::new();
    let service = server.serve(rmcp::transport::stdio()).await?;

    // Wait for shutdown
    service.waiting().await?;

    tracing::info!("rmcp-network server stopped");
    Ok(())
}
