# Claude Sensors

Cross-platform environmental awareness for AI assistants. A suite of MCP servers that let Claude (or any MCP-compatible AI) perceive your local environment.

## What This Is

A collection of lightweight MCP (Model Context Protocol) servers that expose system information to AI assistants. Each server is standalone, cross-platform, and built in Rust.

Together, they transform Claude from "chatbot in a terminal" to "ambient presence aware of your environment."

## Installation

### Unified Binary (Recommended)

```bash
cargo install claude-sensors
```

This gives you **all 14 tools in a single binary**.

### Individual Sensors

If you only need specific sensors:

```bash
cargo install rmcp-idle      # User idle time detection
cargo install rmcp-display   # Monitor/display info
cargo install rmcp-network   # Network interfaces
cargo install rmcp-usb       # USB devices
cargo install rmcp-battery   # Battery/power status
cargo install rmcp-bluetooth # BLE device scanner
cargo install rmcp-git       # Git repository info
cargo install rmcp-sysinfo   # CPU, memory, disk, processes
cargo install rmcp-weather   # Weather conditions and forecast
```

## The Suite

| Server | Tools | What It Sees |
|--------|-------|--------------|
| **rmcp-display** | `get_display_info` | Monitors, resolutions, refresh rates, physical sizes |
| **rmcp-idle** | `get_idle_time`, `is_idle_for` | Time since last keyboard/mouse input |
| **rmcp-network** | `get_interfaces` | Network interfaces, IPs, MACs |
| **rmcp-usb** | `get_usb_devices` | Connected USB devices with vendor/product info |
| **rmcp-battery** | `get_battery_status` | Charge level, power state, health, temperature |
| **rmcp-bluetooth** | `scan_ble_devices` | Nearby Bluetooth Low Energy devices |
| **rmcp-git** | `get_status`, `get_log` | Repo branch, commits, uncommitted changes |
| **rmcp-sysinfo** | `get_system_info`, `get_disk_info`, `get_top_processes` | CPU, memory, disk, uptime, processes |
| **rmcp-weather** | `get_weather`, `get_forecast` | Current conditions and multi-day forecast |

## Configuration

Add to your Claude config (`~/.claude.json` or equivalent):

```json
{
  "mcpServers": {
    "sensors": {
      "type": "stdio",
      "command": "claude-sensors",
      "args": [],
      "env": {}
    }
  }
}
```

Or for individual sensors:

```json
{
  "mcpServers": {
    "idle": {
      "type": "stdio",
      "command": "rmcp-idle",
      "args": [],
      "env": {}
    }
  }
}
```

## Why This Exists

AI assistants are blind. They don't know if you're at your computer or away. They can't see your network, your devices, or your environment. They respond when prompted and sit idle otherwise.

These sensors change that. An AI with environmental awareness can:
- Notice you've been idle for 3 hours and check in
- See you're on battery at 10% and suggest saving work
- Know your git repo has uncommitted changes before you close the terminal
- Detect when you return and greet you

It's the foundation for proactive AI — assistants that exist in your space, not just your chat window.

## Tech Stack

- Pure Rust, single binaries
- Cross-platform (Linux, macOS, Windows)
- MCP protocol via [`rmcp`](https://crates.io/crates/rmcp) crate
- Minimal dependencies
- Release-optimized (LTO, stripped)

### Platform Crates Used

| Sensor | Crate |
|--------|-------|
| Display | `display-info` |
| Idle | `user-idle` |
| Network | `network-interface` |
| USB | `nusb` |
| Battery | `battery` |
| Bluetooth | `btleplug` |
| Git | `git2` |
| System | `sysinfo` |
| Weather | `reqwest` (wttr.in API) |

## Building from Source

```bash
git clone https://github.com/sqrew/claude-sensors
cd claude-sensors
cargo build --release

# Unified binary at target/release/claude-sensors
# Individual binaries in crates/*/target/release/
```

## License

MIT

---

Part of the Claude Ambient Suite. Built with Claude.
