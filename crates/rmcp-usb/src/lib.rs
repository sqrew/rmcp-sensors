use nusb::list_devices;
use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler},
    model::*,
    ErrorData as McpError,
};

#[derive(Debug)]
pub struct UsbServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for UsbServer {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[rmcp::tool_router]
impl UsbServer {
    #[rmcp::tool(description = "List all connected USB devices with vendor/product info")]
    pub async fn get_usb_devices(&self) -> Result<CallToolResult, McpError> {
        let devices = list_devices()
            .map_err(|e| McpError::internal_error(format!("Failed to list USB devices: {}", e), None))?;

        let mut result = String::from("USB Devices:\n\n");
        let mut count = 0;

        for device in devices {
            count += 1;

            // Get manufacturer and product strings if available
            let manufacturer = device.manufacturer_string().unwrap_or_default();
            let product = device.product_string().unwrap_or_default();
            let serial = device.serial_number().unwrap_or_default();

            // Display name: prefer product name, fall back to vendor:product IDs
            let display_name = if !product.is_empty() {
                product.to_string()
            } else {
                format!("Device {:04x}:{:04x}", device.vendor_id(), device.product_id())
            };

            result.push_str(&format!("{}. {}\n", count, display_name));

            if !manufacturer.is_empty() {
                result.push_str(&format!("   Manufacturer: {}\n", manufacturer));
            }

            result.push_str(&format!("   Vendor ID: {:04x}, Product ID: {:04x}\n",
                device.vendor_id(), device.product_id()));

            if !serial.is_empty() {
                result.push_str(&format!("   Serial: {}\n", serial));
            }

            // Bus and device info
            result.push_str(&format!("   Bus: {}, Device: {}\n",
                device.bus_number(), device.device_address()));

            result.push('\n');
        }

        if count == 0 {
            result.push_str("No USB devices found.\n");
        } else {
            result.push_str(&format!("Total: {} USB devices\n", count));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for UsbServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Cross-platform USB device information server".into()),
        }
    }
}
