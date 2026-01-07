//! rmcp-bluetooth: Cross-platform MCP server for Bluetooth device scanning
//!
//! Run with: `rmcp-bluetooth` (serves on stdio)

use rmcp::ServiceExt;
use rmcp_bluetooth::BluetoothServer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting rmcp-bluetooth server");

    let server = BluetoothServer::new();
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    tracing::info!("rmcp-bluetooth server stopped");
    Ok(())
}
