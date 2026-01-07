use display_info::DisplayInfo;
use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler},
    model::*,
    ErrorData as McpError,
};

#[derive(Debug)]
pub struct DisplayServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for DisplayServer {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn format_display_info(displays: &[DisplayInfo]) -> String {
        let mut result = String::from("Display Information:\n\n");

        if displays.is_empty() {
            result.push_str("No displays detected.\n");
            return result;
        }

        for (i, d) in displays.iter().enumerate() {
            // Header with name and primary indicator
            let primary = if d.is_primary { " (primary)" } else { "" };
            result.push_str(&format!(
                "Display {}: {}{}\n",
                i + 1,
                if d.friendly_name.is_empty() { &d.name } else { &d.friendly_name },
                primary
            ));

            // Resolution and position
            result.push_str(&format!("  Resolution: {}x{}\n", d.width, d.height));
            result.push_str(&format!("  Position: ({}, {})\n", d.x, d.y));

            // Physical size if available
            if d.width_mm > 0 && d.height_mm > 0 {
                // Calculate diagonal in inches
                let diag_mm = ((d.width_mm.pow(2) + d.height_mm.pow(2)) as f32).sqrt();
                let diag_inches = diag_mm / 25.4;
                result.push_str(&format!(
                    "  Physical: {}mm x {}mm (~{:.1}\")\n",
                    d.width_mm, d.height_mm, diag_inches
                ));
            }

            // Refresh rate
            if d.frequency > 0.0 {
                result.push_str(&format!("  Refresh: {:.0}Hz\n", d.frequency));
            }

            // Scale factor
            if d.scale_factor != 1.0 {
                result.push_str(&format!("  Scale: {:.0}%\n", d.scale_factor * 100.0));
            }

            // Rotation
            if d.rotation != 0.0 {
                result.push_str(&format!("  Rotation: {}°\n", d.rotation as i32));
            }

            result.push('\n');
        }

        result.push_str(&format!("Total displays: {}\n", displays.len()));
        result
    }
}

#[rmcp::tool_router]
impl DisplayServer {
    #[rmcp::tool(description = "Get display/monitor information (connected displays, resolutions, physical sizes)")]
    pub async fn get_display_info(&self) -> Result<CallToolResult, McpError> {
        let displays = DisplayInfo::all()
            .map_err(|e| McpError::internal_error(format!("Failed to get display info: {}", e), None))?;

        let formatted = Self::format_display_info(&displays);

        Ok(CallToolResult::success(vec![Content::text(formatted)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for DisplayServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Cross-platform display/monitor information server".into()),
        }
    }
}
