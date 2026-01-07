use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler},
    model::*,
    ErrorData as McpError,
};
use std::time::Duration;

#[derive(Debug)]
pub struct BluetoothServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for BluetoothServer {
    fn default() -> Self {
        Self::new()
    }
}

impl BluetoothServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[rmcp::tool_router]
impl BluetoothServer {
    #[rmcp::tool(description = "Scan for nearby Bluetooth Low Energy (BLE) devices")]
    pub async fn scan_ble_devices(&self) -> Result<CallToolResult, McpError> {
        let manager = Manager::new().await
            .map_err(|e| McpError::internal_error(format!("Failed to create BT manager: {}", e), None))?;

        let adapters = manager.adapters().await
            .map_err(|e| McpError::internal_error(format!("Failed to get adapters: {}", e), None))?;

        if adapters.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "Bluetooth Status:\n\nNo Bluetooth adapters found.\n"
            )]));
        }

        let mut result = String::from("Bluetooth Devices:\n\n");

        for adapter in adapters {
            let adapter_info = adapter.adapter_info().await
                .unwrap_or_else(|_| "Unknown adapter".to_string());
            result.push_str(&format!("Adapter: {}\n\n", adapter_info));

            // Start scanning
            if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
                result.push_str(&format!("  Could not scan: {}\n", e));
                continue;
            }

            // Wait a bit for devices to be discovered
            tokio::time::sleep(Duration::from_secs(3)).await;

            // Stop scanning
            let _ = adapter.stop_scan().await;

            // Get discovered peripherals
            let peripherals = adapter.peripherals().await
                .map_err(|e| McpError::internal_error(format!("Failed to get peripherals: {}", e), None))?;

            if peripherals.is_empty() {
                result.push_str("  No BLE devices found nearby.\n");
            } else {
                let mut count = 0;
                for peripheral in peripherals {
                    count += 1;

                    let properties = peripheral.properties().await
                        .ok()
                        .flatten();

                    let name = properties.as_ref()
                        .and_then(|p| p.local_name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());

                    let address = properties.as_ref()
                        .map(|p| p.address.to_string())
                        .unwrap_or_else(|| "??:??:??:??:??:??".to_string());

                    let rssi = properties.as_ref()
                        .and_then(|p| p.rssi)
                        .map(|r| format!(" ({}dBm)", r))
                        .unwrap_or_default();

                    result.push_str(&format!("  {}. {}{}\n", count, name, rssi));
                    result.push_str(&format!("     Address: {}\n", address));
                }
                result.push_str(&format!("\n  Total: {} BLE devices\n", count));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for BluetoothServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Cross-platform Bluetooth Low Energy device scanner".into()),
        }
    }
}
