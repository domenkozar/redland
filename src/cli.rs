use clap::{ArgAction, Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum ModeArg {
    Auto,
    Day,
    Night,
    Sunset,
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "wlsunset-rs",
    version,
    about = "Wayland screen temperature with sunrise/sunset + GeoClue"
)]
pub struct Opts {
    /// Name/description of outputs to target (can repeat). If omitted, all.
    #[arg(short = 'o', long = "output", action = ArgAction::Append)]
    pub outputs: Vec<String>,

    /// Low color temperature at night (K)
    #[arg(short = 't', long = "low", default_value_t = 4000)]
    pub low_temp: i32,

    /// High color temperature at day (K)
    #[arg(short = 'T', long = "high", default_value_t = 6500)]
    pub high_temp: i32,

    /// Latitude (degrees). If omitted, will try GeoClue if --geoclue is set.
    #[arg(short = 'l', long = "lat")]
    pub latitude: Option<f64>,

    /// Longitude (degrees). If omitted, will try GeoClue if --geoclue is set.
    #[arg(short = 'L', long = "lon")]
    pub longitude: Option<f64>,

    /// Manual sunrise time HH:MM (local). Disables lat/lon usage.
    #[arg(short = 'S', long = "sunrise")]
    pub sunrise: Option<String>,

    /// Manual sunset time HH:MM (local). Disables lat/lon usage.
    #[arg(short = 's', long = "sunset")]
    pub sunset: Option<String>,

    /// Transition duration in seconds around sunrise/sunset
    #[arg(short = 'd', long = "duration", default_value_t = 1800)]
    pub duration: i64,

    /// Operating mode override (auto/day/night)
    #[arg(long = "mode", value_enum, default_value_t = ModeArg::Auto)]
    pub mode: ModeArg,

    /// Enable IPC socket server for external control (specify socket path)
    #[arg(long = "socket")]
    pub socket: Option<PathBuf>,
}
