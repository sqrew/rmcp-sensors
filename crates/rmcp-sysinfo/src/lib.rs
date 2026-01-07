use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler, wrapper::Parameters},
    model::*,
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sysinfo::{System, Disks, CpuRefreshKind, MemoryRefreshKind, RefreshKind};

#[derive(Debug)]
pub struct SysinfoServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for SysinfoServer {
    fn default() -> Self {
        Self::new()
    }
}

impl SysinfoServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

// Tool parameter structs
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
pub struct FindProcessParams {
    #[schemars(description = "Process name to search for (case-insensitive, partial match)")]
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProcessIdParams {
    #[schemars(description = "Process ID (PID) to get details for")]
    pub pid: u32,
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

#[rmcp::tool_router]
impl SysinfoServer {
    #[rmcp::tool(description = "Get system overview: CPU usage, memory, disk space, uptime")]
    pub async fn get_system_info(&self) -> Result<CallToolResult, McpError> {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );

        // Need to wait a bit for CPU measurement
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu_all();

        let disks = Disks::new_with_refreshed_list();

        // CPU info
        let cpu_count = sys.cpus().len();
        let cpu_usage: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32;
        let cpu_name = sys.cpus().first().map(|c| c.brand()).unwrap_or("Unknown");

        // Memory info
        let total_mem = sys.total_memory();
        let used_mem = sys.used_memory();
        let mem_percent = (used_mem as f64 / total_mem as f64 * 100.0) as u64;

        // Swap info
        let total_swap = sys.total_swap();
        let used_swap = sys.used_swap();

        // Disk info (aggregate)
        let mut total_disk: u64 = 0;
        let mut free_disk: u64 = 0;
        for disk in disks.iter() {
            total_disk += disk.total_space();
            free_disk += disk.available_space();
        }

        // Uptime
        let uptime_secs = System::uptime();
        let uptime_hours = uptime_secs / 3600;
        let uptime_mins = (uptime_secs % 3600) / 60;

        // Load average (Unix only)
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
            cpu_name, cpu_count,
            cpu_usage,
            format_bytes(used_mem), format_bytes(total_mem), mem_percent,
            format_bytes(used_swap), format_bytes(total_swap),
            format_bytes(free_disk), format_bytes(total_disk),
            uptime_hours, uptime_mins,
            load.one, load.five, load.fifteen
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
            let percent = if total > 0 { (used as f64 / total as f64 * 100.0) as u64 } else { 0 };

            output.push_str(&format!(
                "{} ({})\n  {} / {} ({:.0}% used)\n  Mount: {}\n\n",
                disk.name().to_string_lossy(),
                disk.file_system().to_string_lossy(),
                format_bytes(used),
                format_bytes(total),
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
                processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        let mut output = format!("Top {} processes by {}:\n\n", count, sort_by);
        output.push_str(&format!("{:<8} {:<10} {:<10} {}\n", "PID", "CPU%", "Memory", "Name"));
        output.push_str(&format!("{:-<50}\n", ""));

        for proc in processes.iter().take(count) {
            output.push_str(&format!(
                "{:<8} {:<10.1} {:<10} {}\n",
                proc.pid(),
                proc.cpu_usage(),
                format_bytes(proc.memory()),
                proc.name().to_string_lossy()
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[rmcp::tool(description = "Find processes by name (case-insensitive, partial match)")]
    pub async fn find_process(
        &self,
        Parameters(params): Parameters<FindProcessParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut sys = System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_all();

        let search = params.name.to_lowercase();
        let mut matches: Vec<_> = sys
            .processes()
            .values()
            .filter(|p| p.name().to_string_lossy().to_lowercase().contains(&search))
            .collect();

        matches.sort_by(|a, b| {
            b.cpu_usage()
                .partial_cmp(&a.cpu_usage())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut output = format!("Processes matching '{}':\n\n", params.name);

        if matches.is_empty() {
            output.push_str("No matching processes found.\n");
        } else {
            output.push_str(&format!(
                "{:<8} {:<10} {:<10} {}\n",
                "PID", "CPU%", "Memory", "Name"
            ));
            output.push_str(&format!("{:-<50}\n", ""));

            for proc in matches.iter().take(20) {
                output.push_str(&format!(
                    "{:<8} {:<10.1} {:<10} {}\n",
                    proc.pid(),
                    proc.cpu_usage(),
                    format_bytes(proc.memory()),
                    proc.name().to_string_lossy()
                ));
            }

            if matches.len() > 20 {
                output.push_str(&format!("\n... and {} more matches\n", matches.len() - 20));
            }

            output.push_str(&format!("\nTotal matches: {}\n", matches.len()));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[rmcp::tool(description = "Get detailed information about a specific process by PID")]
    pub async fn get_process_details(
        &self,
        Parameters(params): Parameters<ProcessIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut sys = System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_all();

        let pid = sysinfo::Pid::from_u32(params.pid);

        let proc = sys.process(pid).ok_or_else(|| {
            McpError::internal_error(format!("Process {} not found", params.pid), None)
        })?;

        let mut output = format!("Process Details (PID {}):\n\n", params.pid);

        output.push_str(&format!("Name: {}\n", proc.name().to_string_lossy()));
        output.push_str(&format!("Status: {:?}\n", proc.status()));
        output.push_str(&format!("CPU Usage: {:.1}%\n", proc.cpu_usage()));
        output.push_str(&format!("Memory: {}\n", format_bytes(proc.memory())));
        output.push_str(&format!("Virtual Memory: {}\n", format_bytes(proc.virtual_memory())));

        if let Some(parent) = proc.parent() {
            output.push_str(&format!("Parent PID: {}\n", parent));
        }

        let run_time = proc.run_time();
        output.push_str(&format!("Running for: {}\n", format_duration(run_time)));

        if let Some(exe) = proc.exe() {
            output.push_str(&format!("Executable: {}\n", exe.display()));
        }

        if let Some(cwd) = proc.cwd() {
            output.push_str(&format!("Working Dir: {}\n", cwd.display()));
        }

        let cmd = proc.cmd();
        if !cmd.is_empty() {
            let cmd_str: Vec<_> = cmd.iter().map(|s| s.to_string_lossy()).collect();
            let cmd_display = cmd_str.join(" ");
            if cmd_display.len() > 200 {
                output.push_str(&format!("Command: {}...\n", &cmd_display[..200]));
            } else {
                output.push_str(&format!("Command: {}\n", cmd_display));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[rmcp::tool(description = "List all running processes (sorted by CPU usage)")]
    pub async fn list_processes(&self) -> Result<CallToolResult, McpError> {
        let mut sys = System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_all();

        let mut processes: Vec<_> = sys.processes().values().collect();
        processes.sort_by(|a, b| {
            b.cpu_usage()
                .partial_cmp(&a.cpu_usage())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut output = String::from("All Running Processes:\n\n");
        output.push_str(&format!(
            "{:<8} {:<10} {:<10} {}\n",
            "PID", "CPU%", "Memory", "Name"
        ));
        output.push_str(&format!("{:-<60}\n", ""));

        for proc in processes.iter().take(50) {
            output.push_str(&format!(
                "{:<8} {:<10.1} {:<10} {}\n",
                proc.pid(),
                proc.cpu_usage(),
                format_bytes(proc.memory()),
                proc.name().to_string_lossy()
            ));
        }

        if processes.len() > 50 {
            output.push_str(&format!("\n... and {} more processes\n", processes.len() - 50));
        }

        output.push_str(&format!("\nTotal processes: {}\n", processes.len()));

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for SysinfoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("System information server - CPU, memory, disk, processes".into()),
        }
    }
}
