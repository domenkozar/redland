# Redland

A Wayland screen color temperature adjuster with automatic day/night cycle support, written in Rust. Similar to [wlsunset](https://sr.ht/~kennylevinsen/wlsunset/) and [Redshift](http://jonls.dk/redshift/), but with a modern Rust implementation and optional QML-based system tray UI.

## Features

- **Automatic color temperature adjustment** based on sunrise/sunset times
- **Multiple location methods**: GeoClue2, manual coordinates, or fixed times
- **Smooth transitions** between day and night temperatures
- **Manual mode override** (Day/Night/Sunset/Auto) with automatic expiration
- **IPC control** via Unix socket for external integration
- **System tray UI** using Quickshell (optional)
- **Wayland native** using wlr-gamma-control-unstable-v1 protocol

## Requirements

- Wayland compositor with `wlr-gamma-control-unstable-v1` support (e.g., Sway, Hyprland, river)
- Rust toolchain (edition 2024)
- GeoClue2 (optional, for automatic location detection)
- [Quickshell](https://github.com/outfoxxed/quickshell) (optional, for system tray UI)

## Building

```bash
# Using cargo directly
cargo build --release

# Or with devenv (if using Nix)
devenv shell -- cargo build --release
```

The binary will be available at `target/release/redland`.

## Usage

### Basic Usage

Run with automatic location detection (via GeoClue2):
```bash
redland --socket /tmp/redland.sock
```

### Manual Coordinates

Specify latitude and longitude to skip GeoClue:
```bash
redland --lat 45.0 --lon 15.0 --socket /tmp/redland.sock
```

### Fixed Sunrise/Sunset Times

Use manual times (HH:MM format) instead of calculated sunrise/sunset:
```bash
redland --sunrise 06:30 --sunset 18:00 --socket /tmp/redland.sock
```

### Custom Temperature Range

Adjust the temperature range (default: 4000K night, 6500K day):
```bash
redland --low 3000 --high 6500 --socket /tmp/redland.sock
```

### Transition Duration

Set transition duration around sunrise/sunset (default: 1800 seconds):
```bash
redland --duration 3600 --socket /tmp/redland.sock
```

### Start in Specific Mode

```bash
redland --mode day --socket /tmp/redland.sock    # Force day mode
redland --mode night --socket /tmp/redland.sock  # Force night mode
redland --mode auto --socket /tmp/redland.sock   # Automatic (default)
```

## System Tray UI

The included QML-based system tray provides visual mode control:

```bash
quickshell -c redland-ui.qml
```

Features:
- Icon showing current mode (☀️ day, 🌙 night, 🌅 sunset)
- Superscript "A" indicator for automatic mode
- Popup menu for mode selection
- Real-time temperature display

**Note**: Edit `redland-ui.qml` and update the daemon path to match your installation.

## IPC Protocol

Redland uses a JSON-line protocol over Unix socket for external control.

### Commands

**Get Status:**
```bash
echo '{"type":"get_status"}' | nc -U /tmp/redland.sock
```

**Set Mode:**
```bash
echo '{"type":"set_mode","mode":"night"}' | nc -U /tmp/redland.sock
```
Valid modes: `auto`, `day`, `night`, `sunset`

**Set Temperature Range:**
```bash
echo '{"type":"set_temperature","low":3000,"high":6500}' | nc -U /tmp/redland.sock
```

### Response Format

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

## Command-Line Options

```
Options:
  -o, --output <OUTPUT>        Name/description of outputs to target (can repeat)
  -t, --low <LOW_TEMP>         Low color temperature at night (K) [default: 4000]
  -T, --high <HIGH_TEMP>       High color temperature at day (K) [default: 6500]
  -l, --lat <LATITUDE>         Latitude (degrees)
  -L, --lon <LONGITUDE>        Longitude (degrees)
  -S, --sunrise <SUNRISE>      Manual sunrise time HH:MM (local)
  -s, --sunset <SUNSET>        Manual sunset time HH:MM (local)
  -d, --duration <DURATION>    Transition duration in seconds [default: 1800]
      --mode <MODE>            Operating mode [default: auto] [possible values: auto, day, night, sunset]
      --socket <SOCKET>        Enable IPC socket server (specify socket path)
  -h, --help                   Print help
  -V, --version                Print version
```

## How It Works

1. **Location Detection**: Uses GeoClue2 D-Bus service, manual coordinates, or fixed times
2. **Sunrise/Sunset Calculation**: Computes solar events using astronomical algorithms
3. **Phase Detection**: Determines current phase (Night/Sunrise/Day/Sunset)
4. **Temperature Calculation**: Interpolates between low and high temperatures during transitions
5. **Gamma Adjustment**: Applies color temperature via Wayland gamma control protocol
6. **Mode Override**: Manual mode selection expires at next sunrise, returning to automatic

## Day Phases

- **Night**: Before dawn or after dusk
- **Sunrise**: Transition period before sunrise (gradual warming)
- **Day**: Between sunrise and sunset (high temperature)
- **Sunset**: Transition period after sunset (gradual cooling)

## License

See the source code for license information.

## Acknowledgments

Inspired by [wlsunset](https://sr.ht/~kennylevinsen/wlsunset/) and [Redshift](http://jonls.dk/redshift/).