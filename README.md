# rmcp-sensors

[![Crates.io](https://img.shields.io/crates/v/rmcp-sensors.svg)](https://crates.io/crates/rmcp-sensors)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

Cross-platform environmental awareness for AI assistants. A suite of MCP servers that let any MCP-compatible AI perceive your local environment.

## What This Is

A collection of lightweight MCP (Model Context Protocol) servers that expose system information to AI assistants. Each server is standalone, cross-platform, and built in Rust.

Works with Claude, ChatGPT, or any MCP-compatible client.

## Installation

### Unified Binary (Recommended)

```bash
cargo install rmcp-sensors
```

This gives you **all 14+ tools in a single binary**.

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
| **rmcp-display** | `get_display_info`, `get_display_at_point`, `get_display_by_name` | Monitors, resolutions, refresh rates, physical sizes |
| **rmcp-idle** | `get_idle_time`, `is_idle_for` | Time since last keyboard/mouse input |
| **rmcp-network** | `get_interfaces` | Network interfaces, IPs, MACs |
| **rmcp-usb** | `get_usb_devices` | Connected USB devices with vendor/product info |
| **rmcp-battery** | `get_battery_status` | Charge level, power state, health, temperature |
| **rmcp-bluetooth** | `scan_ble_devices` | Nearby Bluetooth Low Energy devices |
| **rmcp-git** | `get_status`, `get_log`, `get_branches`, `get_remotes`, `get_tags`, `get_stash_list`, `get_diff_summary`, `get_current_branch` | Full repo awareness |
| **rmcp-sysinfo** | `get_system_info`, `get_disk_info`, `get_top_processes`, `get_network_stats`, `get_component_temps`, `get_users` | CPU, memory, disk, uptime, processes, temps |
| **rmcp-weather** | `get_weather`, `get_forecast` | Current conditions and multi-day forecast |

## Sample Output

Here's what your AI sees when using these sensors:

### System Info
```
System Information:

CPU: Intel(R) Core(TM) i5-6500 CPU @ 3.20GHz (4 cores)
CPU Usage: 12.3%

Memory: 5.2 GB / 46.8 GB (11%)
Swap: 0 B / 0 B

Disk: 105.2 GB / 333.5 GB free

Uptime: 35h 25m
Load Average: 1.80 2.04 1.83 (1m 5m 15m)
```

### Idle Time
```
User Idle Time:

  Raw: 847 seconds
  Formatted: 14m 7s
```

### USB Devices
```
USB Devices:

1. USB Optical Mouse
   Manufacturer: Logitech
   Vendor ID: 046d, Product ID: c077
   Bus: 1, Device: 27

2. Xbox Series X Controller
   Manufacturer: BDA
   Vendor ID: 20d6, Product ID: 2001
   Bus: 1, Device: 7

3. Huion Tablet_H1161
   Manufacturer: HUION
   Vendor ID: 256c, Product ID: 0064
   Bus: 1, Device: 6

Total: 10 USB devices
```

### Weather
```
Weather for Portland, Maine:
Conditions: Partly cloudy
Temperature: 28°F / -2°C
Feels like: 21°F / -6°C
Humidity: 65%
Wind: 8 mph NW
UV Index: 1
```

## Configuration

Add to your Claude Code config (`~/.claude.json`) or any MCP client config:

```json
{
  "mcpServers": {
    "sensors": {
      "type": "stdio",
      "command": "rmcp-sensors",
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
- Check the weather before you head out

It's the foundation for **proactive AI** — assistants that exist in your space, not just your chat window.

## Tech Stack

- **Pure Rust** — Single static binaries, no runtime dependencies
- **Cross-platform** — Linux, macOS, Windows
- **MCP Protocol** — Via the [`rmcp`](https://crates.io/crates/rmcp) crate
- **Minimal footprint** — ~5MB unified binary with all sensors
- **Release-optimized** — LTO, single codegen unit, stripped symbols

### Platform Crates Used

| Sensor | Crate | Notes |
|--------|-------|-------|
| Display | [`display-info`](https://crates.io/crates/display-info) | X11, Wayland, Win32, macOS |
| Idle | [`user-idle`](https://crates.io/crates/user-idle) | X11, Win32, macOS |
| Network | [`network-interface`](https://crates.io/crates/network-interface) | All platforms |
| USB | [`nusb`](https://crates.io/crates/nusb) | Pure Rust, no libusb |
| Battery | [`battery`](https://crates.io/crates/battery) | All platforms |
| Bluetooth | [`btleplug`](https://crates.io/crates/btleplug) | BLE on all platforms |
| Git | [`git2`](https://crates.io/crates/git2) | libgit2 bindings |
| System | [`sysinfo`](https://crates.io/crates/sysinfo) | All platforms |
| Weather | [`reqwest`](https://crates.io/crates/reqwest) | wttr.in API |

## Building from Source

```bash
git clone https://github.com/sqrew/rmcp-sensors
cd rmcp-sensors
cargo build --release

# Unified binary at target/release/rmcp-sensors
# Individual binaries in crates/*/target/release/
```

## Related Projects

- [`rmcp`](https://crates.io/crates/rmcp) — The Rust MCP framework these servers are built on

## License

MIT

---

Built by [sqrew](https://github.com/sqrew).
