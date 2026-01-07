use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler, wrapper::Parameters},
    model::*,
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::Deserialize;
use user_idle::UserIdle;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IdleThresholdParams {
    #[schemars(description = "Threshold in seconds to check against (default: 300)")]
    pub threshold_seconds: u64,
}


#[derive(Debug)]
pub struct IdleServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for IdleServer {
    fn default() -> Self {
        Self::new()
    }
}

impl IdleServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn format_duration(seconds: u64) -> String {
        if seconds < 60 {
            format!("{}s", seconds)
        } else if seconds < 3600 {
            let mins = seconds / 60;
            let secs = seconds % 60;
            if secs == 0 {
                format!("{}m", mins)
            } else {
                format!("{}m {}s", mins, secs)
            }
        } else {
            let hours = seconds / 3600;
            let mins = (seconds % 3600) / 60;
            if mins == 0 {
                format!("{}h", hours)
            } else {
                format!("{}h {}m", hours, mins)
            }
        }
    }
}

#[rmcp::tool_router]
impl IdleServer {
    #[rmcp::tool(description = "Get user idle time (how long since last keyboard/mouse input)")]
    pub async fn get_idle_time(&self) -> Result<CallToolResult, McpError> {
        let idle = UserIdle::get_time()
            .map_err(|e| McpError::internal_error(format!("Failed to get idle time: {}", e), None))?;

        let seconds = idle.as_seconds();
        let formatted = Self::format_duration(seconds);

        let result = format!(
            "User Idle Time:\n\n  Raw: {} seconds\n  Formatted: {}\n",
            seconds, formatted
        );

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[rmcp::tool(description = "Check if user has been idle longer than specified seconds")]
    pub async fn is_idle_for(
        &self,
        Parameters(params): Parameters<IdleThresholdParams>,
    ) -> Result<CallToolResult, McpError> {
        let idle = UserIdle::get_time()
            .map_err(|e| McpError::internal_error(format!("Failed to get idle time: {}", e), None))?;

        let seconds = idle.as_seconds();
        let threshold = params.threshold_seconds;
        let is_idle = seconds >= threshold;

        let result = format!(
            "Idle Check:\n\n  Current idle: {} ({})\n  Threshold: {} ({})\n  Is idle: {}\n",
            seconds,
            Self::format_duration(seconds),
            threshold,
            Self::format_duration(threshold),
            if is_idle { "YES" } else { "NO" }
        );

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for IdleServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Cross-platform user idle time detection server".into()),
        }
    }
}
