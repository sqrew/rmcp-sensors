use battery::{Manager, State};
use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler},
    model::*,
    ErrorData as McpError,
};

#[derive(Debug)]
pub struct BatteryServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for BatteryServer {
    fn default() -> Self {
        Self::new()
    }
}

impl BatteryServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn state_to_string(state: State) -> &'static str {
        match state {
            State::Charging => "Charging",
            State::Discharging => "Discharging",
            State::Empty => "Empty",
            State::Full => "Full",
            State::Unknown => "Unknown",
            _ => "Unknown",
        }
    }
}

#[rmcp::tool_router]
impl BatteryServer {
    #[rmcp::tool(description = "Get battery/power status (charge level, charging state, time remaining)")]
    pub async fn get_battery_status(&self) -> Result<CallToolResult, McpError> {
        let manager = Manager::new()
            .map_err(|e| McpError::internal_error(format!("Failed to create battery manager: {}", e), None))?;

        let batteries: Vec<_> = manager.batteries()
            .map_err(|e| McpError::internal_error(format!("Failed to get batteries: {}", e), None))?
            .filter_map(|b| b.ok())
            .collect();

        let mut result = String::from("Battery Status:\n\n");

        if batteries.is_empty() {
            result.push_str("No batteries detected.\n");
            result.push_str("(This is normal for desktop computers without UPS)\n");
            return Ok(CallToolResult::success(vec![Content::text(result)]));
        }

        for (i, battery) in batteries.iter().enumerate() {
            result.push_str(&format!("Battery {}:\n", i + 1));

            // State of charge (percentage)
            let percentage = battery.state_of_charge().get::<battery::units::ratio::percent>();
            result.push_str(&format!("  Charge: {:.1}%\n", percentage));

            // State (charging, discharging, etc.)
            result.push_str(&format!("  State: {}\n", Self::state_to_string(battery.state())));

            // Energy info
            let energy = battery.energy().get::<battery::units::energy::watt_hour>();
            let energy_full = battery.energy_full().get::<battery::units::energy::watt_hour>();
            result.push_str(&format!("  Energy: {:.1} / {:.1} Wh\n", energy, energy_full));

            // Time remaining (if available)
            if let Some(time) = battery.time_to_full() {
                let minutes = time.get::<battery::units::time::minute>();
                result.push_str(&format!("  Time to full: {:.0} minutes\n", minutes));
            }
            if let Some(time) = battery.time_to_empty() {
                let minutes = time.get::<battery::units::time::minute>();
                result.push_str(&format!("  Time to empty: {:.0} minutes\n", minutes));
            }

            // Health
            let health = battery.state_of_health().get::<battery::units::ratio::percent>();
            result.push_str(&format!("  Health: {:.1}%\n", health));

            // Temperature if available
            if let Some(temp) = battery.temperature() {
                let celsius = temp.get::<battery::units::thermodynamic_temperature::degree_celsius>();
                result.push_str(&format!("  Temperature: {:.1}°C\n", celsius));
            }

            result.push('\n');
        }

        result.push_str(&format!("Total batteries: {}\n", batteries.len()));

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for BatteryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Cross-platform battery/power status server".into()),
        }
    }
}
