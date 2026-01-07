//! claude-sensors - Cross-platform environmental awareness for AI assistants
//!
//! A unified MCP server that exposes all sensor tools in one binary.

use btleplug::api::{Central, Manager as BtManager, Peripheral as _, ScanFilter};
use btleplug::platform::Manager as BluetoothManager;
use display_info::DisplayInfo;
use git2::{Repository, StatusOptions};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use nusb::list_devices;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters, ServerHandler},
    model::*,
    transport::stdio,
    ErrorData as McpError,
    ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
use user_idle::UserIdle;

// ============================================================================
// Parameter structs
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IdleThresholdParams {
    #[schemars(description = "Threshold in seconds to check against (default: 300)")]
    pub threshold_seconds: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoPathParams {
    #[schemars(description = "Path to the git repository (defaults to current directory)")]
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TopProcessesParams {
    #[schemars(description = "Number of top processes to show (default 10)")]
    #[serde(default)]
    pub count: Option<usize>,
    #[schemars(description = "Sort by: 'cpu' or 'memory' (default 'cpu')")]
    #[serde(default)]
    pub sort_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LocationParams {
    #[schemars(description = "Location to get weather for (city name, zip code, or 'lat,lon')")]
    pub location: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ForecastParams {
    #[schemars(description = "Location to get forecast for")]
    pub location: String,
    #[schemars(description = "Number of days (1-3, default 3)")]
    #[serde(default)]
    pub days: Option<u8>,
}

// ============================================================================
// Weather API response structs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct WttrResponse {
    pub current_condition: Vec<CurrentCondition>,
    pub nearest_area: Vec<NearestArea>,
    pub weather: Vec<WeatherDay>,
}

#[derive(Debug, Deserialize)]
pub struct CurrentCondition {
    pub temp_F: String,
    pub temp_C: String,
    #[serde(rename = "FeelsLikeF")]
    pub feels_like_f: String,
    #[serde(rename = "FeelsLikeC")]
    pub feels_like_c: String,
    pub humidity: String,
    pub weatherDesc: Vec<WeatherDesc>,
    pub windspeedMiles: String,
    pub windspeedKmph: String,
    pub winddir16Point: String,
    pub precipMM: String,
    pub visibility: String,
    pub pressure: String,
    pub uvIndex: String,
}

#[derive(Debug, Deserialize)]
pub struct WeatherDesc {
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct NearestArea {
    pub areaName: Vec<AreaValue>,
    pub region: Vec<AreaValue>,
    pub country: Vec<AreaValue>,
}

#[derive(Debug, Deserialize)]
pub struct AreaValue {
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct WeatherDay {
    pub date: String,
    pub maxtempF: String,
    pub maxtempC: String,
    pub mintempF: String,
    pub mintempC: String,
    pub hourly: Vec<HourlyForecast>,
}

#[derive(Debug, Deserialize)]
pub struct HourlyForecast {
    pub time: String,
    pub tempF: String,
    pub tempC: String,
    pub weatherDesc: Vec<WeatherDesc>,
    pub chanceofrain: String,
}

// ============================================================================
// Unified Sensors Server
// ============================================================================

#[derive(Debug)]
pub struct SensorsServer {
    pub tool_router: ToolRouter<Self>,
    http_client: reqwest::Client,
}

impl Default for SensorsServer {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorsServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            http_client: reqwest::Client::new(),
        }
    }

    // Helper functions
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

    fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    fn get_repo(path: Option<String>) -> Result<Repository, McpError> {
        let repo_path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        Repository::discover(&repo_path)
            .map_err(|e| McpError::internal_error(format!("Not a git repository: {}", e), None))
    }

    fn battery_state_to_string(state: battery::State) -> &'static str {
        match state {
            battery::State::Charging => "Charging",
            battery::State::Discharging => "Discharging",
            battery::State::Empty => "Empty",
            battery::State::Full => "Full",
            battery::State::Unknown => "Unknown",
            _ => "Unknown",
        }
    }

    async fn fetch_weather(&self, location: &str) -> Result<WttrResponse, McpError> {
        let url = format!(
            "https://wttr.in/{}?format=j1",
            urlencoding::encode(location)
        );

        let response = self
            .http_client
            .get(&url)
            .header("User-Agent", "claude-sensors/0.1.0")
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("HTTP request failed: {}", e), None))?;

        if !response.status().is_success() {
            return Err(McpError::internal_error(
                format!("Weather API returned status: {}", response.status()),
                None,
            ));
        }

        response
            .json::<WttrResponse>()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to parse weather data: {}", e), None))
    }
}

#[rmcp::tool_router]
impl SensorsServer {
    // ========================================================================
    // DISPLAY
    // ========================================================================

    #[rmcp::tool(description = "Get display/monitor information (connected displays, resolutions, physical sizes)")]
    pub async fn get_display_info(&self) -> Result<CallToolResult, McpError> {
        let displays = DisplayInfo::all()
            .map_err(|e| McpError::internal_error(format!("Failed to get display info: {}", e), None))?;

        let mut result = String::from("Display Information:\n\n");

        if displays.is_empty() {
            result.push_str("No displays detected.\n");
        } else {
            for (i, d) in displays.iter().enumerate() {
                let primary = if d.is_primary { " (primary)" } else { "" };
                result.push_str(&format!(
                    "Display {}: {}{}\n",
                    i + 1,
                    if d.friendly_name.is_empty() { &d.name } else { &d.friendly_name },
                    primary
                ));
                result.push_str(&format!("  Resolution: {}x{}\n", d.width, d.height));
                result.push_str(&format!("  Position: ({}, {})\n", d.x, d.y));

                if d.width_mm > 0 && d.height_mm > 0 {
                    let diag_mm = ((d.width_mm.pow(2) + d.height_mm.pow(2)) as f32).sqrt();
                    let diag_inches = diag_mm / 25.4;
                    result.push_str(&format!(
                        "  Physical: {}mm x {}mm (~{:.1}\")\n",
                        d.width_mm, d.height_mm, diag_inches
                    ));
                }

                if d.frequency > 0.0 {
                    result.push_str(&format!("  Refresh: {:.0}Hz\n", d.frequency));
                }

                if d.scale_factor != 1.0 {
                    result.push_str(&format!("  Scale: {:.0}%\n", d.scale_factor * 100.0));
                }

                result.push('\n');
            }
            result.push_str(&format!("Total displays: {}\n", displays.len()));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // IDLE
    // ========================================================================

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

    // ========================================================================
    // NETWORK
    // ========================================================================

    #[rmcp::tool(description = "List all network interfaces with their IP addresses and MAC addresses")]
    pub async fn get_interfaces(&self) -> Result<CallToolResult, McpError> {
        let interfaces = NetworkInterface::show()
            .map_err(|e| McpError::internal_error(format!("Failed to get network interfaces: {}", e), None))?;

        let mut result = String::from("Network Interfaces:\n\n");

        if interfaces.is_empty() {
            result.push_str("No network interfaces found.\n");
        } else {
            for iface in &interfaces {
                let is_loopback = iface.addr.iter().any(|a| match a {
                    Addr::V4(v4) => v4.ip.is_loopback(),
                    Addr::V6(v6) => v6.ip.is_loopback(),
                });

                result.push_str(&format!("{}", iface.name));
                if is_loopback {
                    result.push_str(" (loopback)");
                }
                result.push('\n');

                if let Some(ref mac) = iface.mac_addr {
                    if !mac.is_empty() && mac != "00:00:00:00:00:00" {
                        result.push_str(&format!("  MAC: {}\n", mac));
                    }
                }

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
                            if !v6.ip.to_string().starts_with("fe80") {
                                result.push_str(&format!("  IPv6: {}\n", v6.ip));
                            }
                        }
                    }
                }
                result.push('\n');
            }

            let active_count = interfaces.iter().filter(|i| !i.addr.is_empty()).count();
            result.push_str(&format!(
                "Total interfaces: {} ({} with addresses)\n",
                interfaces.len(),
                active_count
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // USB
    // ========================================================================

    #[rmcp::tool(description = "List all connected USB devices with vendor/product info")]
    pub async fn get_usb_devices(&self) -> Result<CallToolResult, McpError> {
        let devices = list_devices()
            .map_err(|e| McpError::internal_error(format!("Failed to list USB devices: {}", e), None))?;

        let mut result = String::from("USB Devices:\n\n");
        let mut count = 0;

        for device in devices {
            count += 1;

            let manufacturer = device.manufacturer_string().unwrap_or_default();
            let product = device.product_string().unwrap_or_default();
            let serial = device.serial_number().unwrap_or_default();

            let display_name = if !product.is_empty() {
                product.to_string()
            } else {
                format!("Device {:04x}:{:04x}", device.vendor_id(), device.product_id())
            };

            result.push_str(&format!("{}. {}\n", count, display_name));

            if !manufacturer.is_empty() {
                result.push_str(&format!("   Manufacturer: {}\n", manufacturer));
            }

            result.push_str(&format!(
                "   Vendor ID: {:04x}, Product ID: {:04x}\n",
                device.vendor_id(),
                device.product_id()
            ));

            if !serial.is_empty() {
                result.push_str(&format!("   Serial: {}\n", serial));
            }

            result.push_str(&format!(
                "   Bus: {}, Device: {}\n\n",
                device.bus_number(),
                device.device_address()
            ));
        }

        if count == 0 {
            result.push_str("No USB devices found.\n");
        } else {
            result.push_str(&format!("Total: {} USB devices\n", count));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // BATTERY
    // ========================================================================

    #[rmcp::tool(description = "Get battery/power status (charge level, charging state, time remaining)")]
    pub async fn get_battery_status(&self) -> Result<CallToolResult, McpError> {
        let manager = battery::Manager::new()
            .map_err(|e| McpError::internal_error(format!("Failed to create battery manager: {}", e), None))?;

        let batteries: Vec<_> = manager
            .batteries()
            .map_err(|e| McpError::internal_error(format!("Failed to get batteries: {}", e), None))?
            .filter_map(|b| b.ok())
            .collect();

        let mut result = String::from("Battery Status:\n\n");

        if batteries.is_empty() {
            result.push_str("No batteries detected.\n");
            result.push_str("(This is normal for desktop computers without UPS)\n");
        } else {
            for (i, battery) in batteries.iter().enumerate() {
                result.push_str(&format!("Battery {}:\n", i + 1));

                let percentage = battery
                    .state_of_charge()
                    .get::<battery::units::ratio::percent>();
                result.push_str(&format!("  Charge: {:.1}%\n", percentage));

                result.push_str(&format!(
                    "  State: {}\n",
                    Self::battery_state_to_string(battery.state())
                ));

                let energy = battery.energy().get::<battery::units::energy::watt_hour>();
                let energy_full = battery
                    .energy_full()
                    .get::<battery::units::energy::watt_hour>();
                result.push_str(&format!("  Energy: {:.1} / {:.1} Wh\n", energy, energy_full));

                if let Some(time) = battery.time_to_full() {
                    let minutes = time.get::<battery::units::time::minute>();
                    result.push_str(&format!("  Time to full: {:.0} minutes\n", minutes));
                }
                if let Some(time) = battery.time_to_empty() {
                    let minutes = time.get::<battery::units::time::minute>();
                    result.push_str(&format!("  Time to empty: {:.0} minutes\n", minutes));
                }

                let health = battery
                    .state_of_health()
                    .get::<battery::units::ratio::percent>();
                result.push_str(&format!("  Health: {:.1}%\n", health));

                if let Some(temp) = battery.temperature() {
                    let celsius = temp.get::<battery::units::thermodynamic_temperature::degree_celsius>();
                    result.push_str(&format!("  Temperature: {:.1}°C\n", celsius));
                }

                result.push('\n');
            }
            result.push_str(&format!("Total batteries: {}\n", batteries.len()));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // BLUETOOTH
    // ========================================================================

    #[rmcp::tool(description = "Scan for nearby Bluetooth Low Energy (BLE) devices")]
    pub async fn scan_ble_devices(&self) -> Result<CallToolResult, McpError> {
        let manager = BluetoothManager::new()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to create BT manager: {}", e), None))?;

        let adapters = manager
            .adapters()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to get adapters: {}", e), None))?;

        if adapters.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "Bluetooth Status:\n\nNo Bluetooth adapters found.\n",
            )]));
        }

        let mut result = String::from("Bluetooth Devices:\n\n");

        for adapter in adapters {
            let adapter_info = adapter
                .adapter_info()
                .await
                .unwrap_or_else(|_| "Unknown adapter".to_string());
            result.push_str(&format!("Adapter: {}\n\n", adapter_info));

            if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
                result.push_str(&format!("  Could not scan: {}\n", e));
                continue;
            }

            tokio::time::sleep(Duration::from_secs(3)).await;
            let _ = adapter.stop_scan().await;

            let peripherals = adapter
                .peripherals()
                .await
                .map_err(|e| McpError::internal_error(format!("Failed to get peripherals: {}", e), None))?;

            if peripherals.is_empty() {
                result.push_str("  No BLE devices found nearby.\n");
            } else {
                let mut count = 0;
                for peripheral in peripherals {
                    count += 1;

                    let properties = peripheral.properties().await.ok().flatten();

                    let name = properties
                        .as_ref()
                        .and_then(|p| p.local_name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());

                    let address = properties
                        .as_ref()
                        .map(|p| p.address.to_string())
                        .unwrap_or_else(|| "??:??:??:??:??:??".to_string());

                    let rssi = properties
                        .as_ref()
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

    // ========================================================================
    // GIT
    // ========================================================================

    #[rmcp::tool(description = "Get git repository status (branch, uncommitted changes, last commit)")]
    pub async fn get_status(
        &self,
        Parameters(params): Parameters<RepoPathParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = Self::get_repo(params.path)?;
        let mut result = String::from("Git Repository Status:\n\n");

        if let Some(workdir) = repo.workdir() {
            result.push_str(&format!("Repository: {}\n", workdir.display()));
        }

        match repo.head() {
            Ok(head) => {
                if let Some(name) = head.shorthand() {
                    result.push_str(&format!("Branch: {}\n", name));
                }

                if let Ok(commit) = head.peel_to_commit() {
                    let id = commit.id();
                    let short_id = &id.to_string()[..7];
                    let summary = commit.summary().unwrap_or("(no message)");
                    let time = commit.time();
                    let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    result.push_str("\nLast Commit:\n");
                    result.push_str(&format!("  {} - {}\n", short_id, summary));
                    result.push_str(&format!(
                        "  Author: {}\n",
                        commit.author().name().unwrap_or("unknown")
                    ));
                    result.push_str(&format!("  Date: {}\n", timestamp));
                }
            }
            Err(_) => {
                result.push_str("Branch: (no commits yet)\n");
            }
        }

        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);

        match repo.statuses(Some(&mut opts)) {
            Ok(statuses) => {
                let mut staged = Vec::new();
                let mut modified = Vec::new();
                let mut untracked = Vec::new();

                for entry in statuses.iter() {
                    let path = entry.path().unwrap_or("?");
                    let status = entry.status();

                    if status.is_index_new() || status.is_index_modified() || status.is_index_deleted()
                    {
                        staged.push(path.to_string());
                    }
                    if status.is_wt_modified() || status.is_wt_deleted() {
                        modified.push(path.to_string());
                    }
                    if status.is_wt_new() {
                        untracked.push(path.to_string());
                    }
                }

                result.push_str("\nWorking Tree:\n");

                if staged.is_empty() && modified.is_empty() && untracked.is_empty() {
                    result.push_str("  Clean - nothing to commit\n");
                } else {
                    if !staged.is_empty() {
                        result.push_str(&format!("  Staged: {} file(s)\n", staged.len()));
                        for f in staged.iter().take(5) {
                            result.push_str(&format!("    + {}\n", f));
                        }
                        if staged.len() > 5 {
                            result.push_str(&format!("    ... and {} more\n", staged.len() - 5));
                        }
                    }
                    if !modified.is_empty() {
                        result.push_str(&format!("  Modified: {} file(s)\n", modified.len()));
                        for f in modified.iter().take(5) {
                            result.push_str(&format!("    M {}\n", f));
                        }
                        if modified.len() > 5 {
                            result.push_str(&format!("    ... and {} more\n", modified.len() - 5));
                        }
                    }
                    if !untracked.is_empty() {
                        result.push_str(&format!("  Untracked: {} file(s)\n", untracked.len()));
                        for f in untracked.iter().take(5) {
                            result.push_str(&format!("    ? {}\n", f));
                        }
                        if untracked.len() > 5 {
                            result.push_str(&format!("    ... and {} more\n", untracked.len() - 5));
                        }
                    }
                }
            }
            Err(e) => {
                result.push_str(&format!("\nCould not get status: {}\n", e));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[rmcp::tool(description = "Get recent git commits (last 10)")]
    pub async fn get_log(
        &self,
        Parameters(params): Parameters<RepoPathParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = Self::get_repo(params.path)?;
        let mut result = String::from("Recent Commits:\n\n");

        let head = repo
            .head()
            .map_err(|e| McpError::internal_error(format!("No HEAD: {}", e), None))?;

        let oid = head
            .target()
            .ok_or_else(|| McpError::internal_error("HEAD has no target", None))?;

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| McpError::internal_error(format!("Failed to create revwalk: {}", e), None))?;

        revwalk
            .push(oid)
            .map_err(|e| McpError::internal_error(format!("Failed to push HEAD: {}", e), None))?;

        let mut count = 0;
        for oid in revwalk.take(10) {
            if let Ok(oid) = oid {
                if let Ok(commit) = repo.find_commit(oid) {
                    count += 1;
                    let id_str = oid.to_string();
                    let short_id = &id_str[..7];
                    let summary = commit.summary().unwrap_or("(no message)").to_string();
                    let author = commit.author();
                    let author_name = author.name().unwrap_or("unknown");

                    result.push_str(&format!("{} {} - {}\n", short_id, author_name, summary));
                }
            }
        }

        if count == 0 {
            result.push_str("No commits found.\n");
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // SYSINFO
    // ========================================================================

    #[rmcp::tool(description = "Get system overview: CPU usage, memory, disk space, uptime")]
    pub async fn get_system_info(&self) -> Result<CallToolResult, McpError> {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu_all();

        let disks = Disks::new_with_refreshed_list();

        let cpu_count = sys.cpus().len();
        let cpu_usage: f32 =
            sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32;
        let cpu_name = sys.cpus().first().map(|c| c.brand()).unwrap_or("Unknown");

        let total_mem = sys.total_memory();
        let used_mem = sys.used_memory();
        let mem_percent = (used_mem as f64 / total_mem as f64 * 100.0) as u64;

        let total_swap = sys.total_swap();
        let used_swap = sys.used_swap();

        let mut total_disk: u64 = 0;
        let mut free_disk: u64 = 0;
        for disk in disks.iter() {
            total_disk += disk.total_space();
            free_disk += disk.available_space();
        }

        let uptime_secs = System::uptime();
        let uptime_hours = uptime_secs / 3600;
        let uptime_mins = (uptime_secs % 3600) / 60;

        let load = System::load_average();

        let output = format!(
            "System Information:\n\
             \n\
             CPU: {} ({} cores)\n\
             CPU Usage: {:.1}%\n\
             \n\
             Memory: {} / {} ({:.0}%)\n\
             Swap: {} / {}\n\
             \n\
             Disk: {} / {} free\n\
             \n\
             Uptime: {}h {}m\n\
             Load Average: {:.2} {:.2} {:.2} (1m 5m 15m)",
            cpu_name,
            cpu_count,
            cpu_usage,
            Self::format_bytes(used_mem),
            Self::format_bytes(total_mem),
            mem_percent,
            Self::format_bytes(used_swap),
            Self::format_bytes(total_swap),
            Self::format_bytes(free_disk),
            Self::format_bytes(total_disk),
            uptime_hours,
            uptime_mins,
            load.one,
            load.five,
            load.fifteen
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[rmcp::tool(description = "Get detailed disk usage for all mounted filesystems")]
    pub async fn get_disk_info(&self) -> Result<CallToolResult, McpError> {
        let disks = Disks::new_with_refreshed_list();

        let mut output = String::from("Disk Usage:\n\n");

        for disk in disks.iter() {
            let total = disk.total_space();
            let free = disk.available_space();
            let used = total - free;
            let percent = if total > 0 {
                (used as f64 / total as f64 * 100.0) as u64
            } else {
                0
            };

            output.push_str(&format!(
                "{} ({})\n  {} / {} ({:.0}% used)\n  Mount: {}\n\n",
                disk.name().to_string_lossy(),
                disk.file_system().to_string_lossy(),
                Self::format_bytes(used),
                Self::format_bytes(total),
                percent,
                disk.mount_point().display()
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[rmcp::tool(description = "Get top processes by CPU or memory usage")]
    pub async fn get_top_processes(
        &self,
        Parameters(params): Parameters<TopProcessesParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut sys = System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_all();

        let count = params.count.unwrap_or(10);
        let sort_by = params.sort_by.unwrap_or_else(|| "cpu".to_string());

        let mut processes: Vec<_> = sys.processes().values().collect();

        match sort_by.as_str() {
            "memory" | "mem" => {
                processes.sort_by(|a, b| b.memory().cmp(&a.memory()));
            }
            _ => {
                processes.sort_by(|a, b| {
                    b.cpu_usage()
                        .partial_cmp(&a.cpu_usage())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        let mut output = format!("Top {} processes by {}:\n\n", count, sort_by);
        output.push_str(&format!(
            "{:<8} {:<10} {:<10} {}\n",
            "PID", "CPU%", "Memory", "Name"
        ));
        output.push_str(&format!("{:-<50}\n", ""));

        for proc in processes.iter().take(count) {
            output.push_str(&format!(
                "{:<8} {:<10.1} {:<10} {}\n",
                proc.pid(),
                proc.cpu_usage(),
                Self::format_bytes(proc.memory()),
                proc.name().to_string_lossy()
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    // ========================================================================
    // WEATHER
    // ========================================================================

    #[rmcp::tool(description = "Get current weather conditions for a location")]
    pub async fn get_weather(
        &self,
        Parameters(params): Parameters<LocationParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self.fetch_weather(&params.location).await?;

        let current = data
            .current_condition
            .first()
            .ok_or_else(|| McpError::internal_error("No current conditions", None))?;

        let area = data
            .nearest_area
            .first()
            .map(|a| {
                format!(
                    "{}, {}",
                    a.areaName.first().map(|v| v.value.as_str()).unwrap_or("Unknown"),
                    a.region.first().map(|v| v.value.as_str()).unwrap_or("")
                )
            })
            .unwrap_or_else(|| params.location.clone());

        let desc = current
            .weatherDesc
            .first()
            .map(|d| d.value.as_str())
            .unwrap_or("Unknown");

        let output = format!(
            "Weather for {}:\n\
             Conditions: {}\n\
             Temperature: {}°F / {}°C\n\
             Feels like: {}°F / {}°C\n\
             Humidity: {}%\n\
             Wind: {} mph {} ({})\n\
             Visibility: {} miles\n\
             Pressure: {} mb\n\
             UV Index: {}",
            area,
            desc,
            current.temp_F,
            current.temp_C,
            current.feels_like_f,
            current.feels_like_c,
            current.humidity,
            current.windspeedMiles,
            current.winddir16Point,
            current.windspeedKmph,
            current.visibility,
            current.pressure,
            current.uvIndex
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[rmcp::tool(description = "Get weather forecast for upcoming days")]
    pub async fn get_forecast(
        &self,
        Parameters(params): Parameters<ForecastParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self.fetch_weather(&params.location).await?;
        let days = params.days.unwrap_or(3).min(3) as usize;

        let area = data
            .nearest_area
            .first()
            .map(|a| {
                format!(
                    "{}, {}",
                    a.areaName.first().map(|v| v.value.as_str()).unwrap_or("Unknown"),
                    a.region.first().map(|v| v.value.as_str()).unwrap_or("")
                )
            })
            .unwrap_or_else(|| params.location.clone());

        let mut output = format!("Forecast for {} ({} days):\n\n", area, days);

        for day in data.weather.iter().take(days) {
            output.push_str(&format!(
                "{}:\n  High: {}°F / {}°C | Low: {}°F / {}°C\n",
                day.date, day.maxtempF, day.maxtempC, day.mintempF, day.mintempC
            ));

            for hour in day.hourly.iter().step_by(3) {
                let time_hr = hour.time.parse::<u32>().unwrap_or(0) / 100;
                let desc = hour
                    .weatherDesc
                    .first()
                    .map(|d| d.value.as_str())
                    .unwrap_or("?");
                output.push_str(&format!(
                    "  {:02}:00 - {}°F, {}, {}% rain\n",
                    time_hr, hour.tempF, desc, hour.chanceofrain
                ));
            }
            output.push('\n');
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for SensorsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Claude Sensors - Cross-platform environmental awareness for AI assistants. \
                 Provides display, idle, network, USB, battery, bluetooth, git, system, and weather information."
                    .into(),
            ),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let server = SensorsServer::new();
    let transport = stdio();

    tracing::info!("claude-sensors starting...");

    server.serve(transport).await?;

    Ok(())
}
