//! rmcp-usb: Cross-platform MCP server for USB device information
//!
//! Run with: `rmcp-usb` (serves on stdio)

use rmcp::ServiceExt;
use rmcp_usb::UsbServer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting rmcp-usb server");

    let server = UsbServer::new();
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    tracing::info!("rmcp-usb server stopped");
    Ok(())
}
