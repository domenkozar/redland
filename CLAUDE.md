# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Redland is a Rust implementation of a Wayland screen color temperature adjuster (similar to Redshift/wlsunset) with automatic day/night cycle support based on sunrise/sunset times. It includes a QML-based system tray UI for manual control.

## Build and Development Commands

Build the project:
```bash
cargo build
```

Build optimized release:
```bash
cargo build --release
```

Run the daemon:
```bash
cargo run -- --socket /tmp/redland.sock
```

Run with manual sunrise/sunset times:
```bash
cargo run -- --sunrise 06:30 --sunset 18:00 --socket /tmp/redland.sock
```

Run with specific coordinates (skips GeoClue):
```bash
cargo run -- --lat 45.0 --lon 15.0 --socket /tmp/redland.sock
```

Run the QML UI (requires quickshell):
```bash
quickshell -c redland-ui.qml
```

## Architecture

### Core Components

- **main.rs**: Main event loop coordinating Wayland events, timers, IPC, and temperature updates. Uses tokio for async runtime and nix for polling Wayland file descriptors.

- **wayland.rs**: Wayland protocol handling using `wayland-client`. Manages wlr-gamma-control-unstable-v1 protocol for setting gamma ramps. Creates anonymous memmap files for gamma tables.

- **scheduling.rs**: Sunrise/sunset calculations using the `sunrise` crate. Computes day phases (Night, Sunrise, Day, Sunset) and temperature interpolation during transitions.

- **ipc.rs**: Unix socket server (tokio-based) for JSON-line IPC. Handles commands: `get_status`, `set_mode`, `set_temperature`. Updates shared application state thread-safely with Arc<Mutex>.

- **geoclue.rs**: D-Bus client for GeoClue2 location service (using zbus blocking API).

- **color.rs**: Color temperature to RGB conversion using blackbody radiation (tempergb crate).

- **cli.rs**: Command-line argument parsing with clap.

### UI Integration

**redland-ui.qml**: Quickshell-based system tray application that:
- Spawns the daemon process with socket IPC
- Displays current mode icon (sun/moon/sunset) with superscript "A" for automatic mode
- Provides popup menu for mode selection (Auto/Day/Night/Sunset)
- Polls daemon status every second via JSON IPC
- Shows current temperature and configured range

### IPC Protocol

JSON-line protocol over Unix socket:

Commands (client → daemon):
```json
{"type": "get_status"}
{"type": "set_mode", "mode": "auto|day|night|sunset"}
{"type": "set_temperature", "low": 4000, "high": 6500}
```

Response (daemon → client):
```json
{
  "type": "status",
  "requested_mode": "auto",
  "current_mode": "day",
  "automatic_mode": "day",
  "current_temp": 6500,
  "low_temp": 4000,
  "high_temp": 6500,
  "location": [45.0, 15.0],
  "sun_times": ["06:30", "18:00"]
}
```

### Event Loop Architecture

The main loop uses tokio::select! to handle:
1. SIGUSR1 signals (force immediate update)
2. Mode changes from IPC socket
3. Timed wake-ups for sunrise/sunset transitions

Wayland events are polled non-blockingly using nix::poll with POLLIN/POLLERR/POLLHUP flags.

### Mode Override System

When user selects Day/Night/Sunset mode, an override expires at the next sunrise. Auto mode clears the override immediately. The override system uses `TrayOverride` struct with expiration timestamp.

## Key Implementation Details

- Uses Rust edition 2024
- Release profile optimized for size (opt-level = "s", lto = true)
- Temperature range typically 4000K (night) to 6500K (day)
- Transition duration (--duration) defaults to 1800 seconds (30 minutes)
- Gamma tables allocated via anonymous tmpfs files and mmap
- Uses memmap2 + bytemuck for safe gamma table manipulation