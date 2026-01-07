use network_interface::{NetworkInterface, NetworkInterfaceConfig, Addr};
use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler},
    model::*,
    ErrorData as McpError,
};

#[derive(Debug)]
pub struct NetworkServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for NetworkServer {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn format_interfaces(interfaces: &[NetworkInterface]) -> String {
        let mut result = String::from("Network Interfaces:\n\n");

        if interfaces.is_empty() {
            result.push_str("No network interfaces found.\n");
            return result;
        }

        for iface in interfaces {
            // Skip loopback for cleaner output (optional)
            let is_loopback = iface.addr.iter().any(|a| match a {
                Addr::V4(v4) => v4.ip.is_loopback(),
                Addr::V6(v6) => v6.ip.is_loopback(),
            });

            result.push_str(&format!("{}", iface.name));
            if is_loopback {
                result.push_str(" (loopback)");
            }
            result.push('\n');

            // MAC address if available
            if let Some(ref mac) = iface.mac_addr {
                if !mac.is_empty() && mac != "00:00:00:00:00:00" {
                    result.push_str(&format!("  MAC: {}\n", mac));
                }
            }

            // IP addresses
            for addr in &iface.addr {
                match addr {
                    Addr::V4(v4) => {
                        result.push_str(&format!("  IPv4: {}", v4.ip));
                        if let Some(netmask) = &v4.netmask {
                            result.push_str(&format!(" / {}", netmask));
                        }
                        result.push('\n');
                    }
                    Addr::V6(v6) => {
                        // Skip link-local IPv6 for cleaner output
                        if !v6.ip.to_string().starts_with("fe80") {
                            result.push_str(&format!("  IPv6: {}\n", v6.ip));
                        }
                    }
                }
            }

            result.push('\n');
        }

        // Summary
        let active_count = interfaces.iter()
            .filter(|i| !i.addr.is_empty())
            .count();
        result.push_str(&format!("Total interfaces: {} ({} with addresses)\n",
            interfaces.len(), active_count));

        result
    }
}

#[rmcp::tool_router]
impl NetworkServer {
    #[rmcp::tool(description = "List all network interfaces with their IP addresses and MAC addresses")]
    pub async fn get_interfaces(&self) -> Result<CallToolResult, McpError> {
        let interfaces = NetworkInterface::show()
            .map_err(|e| McpError::internal_error(format!("Failed to get network interfaces: {}", e), None))?;

        let formatted = Self::format_interfaces(&interfaces);

        Ok(CallToolResult::success(vec![Content::text(formatted)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for NetworkServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Cross-platform network interface information server".into()),
        }
    }
}
