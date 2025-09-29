use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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

fn format_status_response(state: &SharedAppState) -> IpcResponse {
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

pub async fn handle_stdin_commands(
    shared_state: Arc<Mutex<SharedAppState>>,
    mode_tx: tokio::sync::mpsc::UnboundedSender<ModeArg>,
) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let response = match serde_json::from_str::<IpcCommand>(&line.trim()) {
                    Ok(IpcCommand::GetStatus) => {
                        let state = shared_state.lock().unwrap();
                        format_status_response(&state)
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
                        }

                        format_status_response(&state)
                    }
                    Ok(IpcCommand::SetTemperature { low, high }) => {
                        eprintln!("Setting temperature: {} - {}", low, high);
                        let mut state = shared_state.lock().unwrap();
                        state.low_temp = low;
                        state.high_temp = high;
                        state.current_temp = (low + high) / 2;
                        format_status_response(&state)
                    }
                    Err(e) => IpcResponse::Error {
                        message: format!("Invalid command: {}", e),
                    },
                };

                let response_json = serde_json::to_string(&response)?;
                stdout.write_all(response_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
            Err(e) => {
                eprintln!("Error reading from stdin: {}", e);
                break;
            }
        }
    }
    Ok(())
}

