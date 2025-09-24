use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::cli::ModeArg;
use crate::scheduling::DayPhase;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcCommand {
    #[serde(rename = "set_mode")]
    SetMode { mode: String },
    #[serde(rename = "get_status")]
    GetStatus,
    #[serde(rename = "set_temperature")]
    SetTemperature { low: i32, high: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    #[serde(rename = "status")]
    Status {
        requested_mode: String,
        current_mode: String,
        automatic_mode: String,
        current_temp: i32,
        low_temp: i32,
        high_temp: i32,
        location: Option<(f64, f64)>,
        sun_times: Option<(String, String)>,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct SharedAppState {
    pub requested_mode: ModeArg,
    pub current_mode: DayPhase,
    pub automatic_mode: DayPhase,
    pub current_temp: i32,
    pub low_temp: i32,
    pub high_temp: i32,
    pub location: Option<(f64, f64)>,
    pub sun_times: Option<(String, String)>,
}

impl SharedAppState {
    pub fn new(low_temp: i32, high_temp: i32) -> Self {
        Self {
            requested_mode: ModeArg::Auto,
            current_mode: DayPhase::Day,
            automatic_mode: DayPhase::Day,
            current_temp: (low_temp + high_temp) / 2,
            low_temp,
            high_temp,
            location: None,
            sun_times: None,
        }
    }
}

async fn handle_client(
    stream: UnixStream,
    shared_state: Arc<Mutex<SharedAppState>>,
    mode_tx: tokio::sync::mpsc::UnboundedSender<ModeArg>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let response = match serde_json::from_str::<IpcCommand>(&line.trim()) {
                    Ok(IpcCommand::GetStatus) => {
                        let state = shared_state.lock().unwrap();
                        let current = match state.current_mode {
                            DayPhase::Night => "night",
                            DayPhase::Sunrise => "sunrise",
                            DayPhase::Day => "day",
                            DayPhase::Sunset => "sunset",
                        };
                        let automatic = match state.automatic_mode {
                            DayPhase::Night => "night",
                            DayPhase::Sunrise => "sunrise",
                            DayPhase::Day => "day",
                            DayPhase::Sunset => "sunset",
                        };
                        IpcResponse::Status {
                            requested_mode: format!("{:?}", state.requested_mode).to_lowercase(),
                            current_mode: current.to_string(),
                            automatic_mode: automatic.to_string(),
                            current_temp: state.current_temp,
                            low_temp: state.low_temp,
                            high_temp: state.high_temp,
                            location: state.location,
                            sun_times: state.sun_times.clone(),
                        }
                    }
                    Ok(IpcCommand::SetMode { mode }) => {
                        eprintln!("Setting mode to: {}", mode);
                        let mut state = shared_state.lock().unwrap();
                        let new_mode = match mode.as_str() {
                            "auto" => ModeArg::Auto,
                            "day" => ModeArg::Day,
                            "night" => ModeArg::Night,
                            "sunset" => ModeArg::Sunset,
                            _ => state.requested_mode,
                        };
                        state.requested_mode = new_mode;

                        if let Err(e) = mode_tx.send(new_mode) {
                            eprintln!("Failed to send mode change: {}", e);
                        } else {
                            eprintln!("Sent mode change to main loop: {:?}", new_mode);
                        }

                        let current = match state.current_mode {
                            DayPhase::Night => "night",
                            DayPhase::Sunrise => "sunrise",
                            DayPhase::Day => "day",
                            DayPhase::Sunset => "sunset",
                        };
                        let automatic = match state.automatic_mode {
                            DayPhase::Night => "night",
                            DayPhase::Sunrise => "sunrise",
                            DayPhase::Day => "day",
                            DayPhase::Sunset => "sunset",
                        };
                        IpcResponse::Status {
                            requested_mode: format!("{:?}", state.requested_mode).to_lowercase(),
                            current_mode: current.to_string(),
                            automatic_mode: automatic.to_string(),
                            current_temp: state.current_temp,
                            low_temp: state.low_temp,
                            high_temp: state.high_temp,
                            location: state.location,
                            sun_times: state.sun_times.clone(),
                        }
                    }
                    Ok(IpcCommand::SetTemperature { low, high }) => {
                        eprintln!("Setting temperature: {} - {}", low, high);
                        let mut state = shared_state.lock().unwrap();
                        state.low_temp = low;
                        state.high_temp = high;
                        state.current_temp = (low + high) / 2;
                        let current = match state.current_mode {
                            DayPhase::Night => "night",
                            DayPhase::Sunrise => "sunrise",
                            DayPhase::Day => "day",
                            DayPhase::Sunset => "sunset",
                        };
                        let automatic = match state.automatic_mode {
                            DayPhase::Night => "night",
                            DayPhase::Sunrise => "sunrise",
                            DayPhase::Day => "day",
                            DayPhase::Sunset => "sunset",
                        };
                        IpcResponse::Status {
                            requested_mode: format!("{:?}", state.requested_mode).to_lowercase(),
                            current_mode: current.to_string(),
                            automatic_mode: automatic.to_string(),
                            current_temp: state.current_temp,
                            low_temp: state.low_temp,
                            high_temp: state.high_temp,
                            location: state.location,
                            sun_times: state.sun_times.clone(),
                        }
                    }
                    Err(e) => IpcResponse::Error {
                        message: format!("Invalid command: {}", e),
                    },
                };

                let response_json = serde_json::to_string(&response)?;
                writer.write_all(response_json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            Err(e) => {
                eprintln!("Error reading from client: {}", e);
                break;
            }
        }
    }
    Ok(())
}

pub async fn start_socket_server(
    shared_state: Arc<Mutex<SharedAppState>>,
    mode_tx: tokio::sync::mpsc::UnboundedSender<ModeArg>,
    socket_path: &Path,
) -> Result<()> {

    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    eprintln!("IPC server listening on {}", socket_path.display());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state_clone = Arc::clone(&shared_state);
                let tx_clone = mode_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, state_clone, tx_clone).await {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}
