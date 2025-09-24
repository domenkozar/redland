mod cli;
mod color;
mod geoclue;
mod ipc;
mod scheduling;
mod wayland;

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use clap::Parser;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use std::sync::{Arc, Mutex};
use tokio::signal::unix::{SignalKind, signal};
use wayland_client::Connection;

use cli::{ModeArg, Opts};
use geoclue::geoclue_lat_lon;
use ipc::{SharedAppState, start_socket_server};
use scheduling::{
    DayPhase, TrayOverride, compute_day_stops, next_sunrise_timestamp, parse_hhmm, phase_for,
    temperature_for,
};
use wayland::{AppState, set_temperature_all};

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let startup_mode: ModeArg = opts.mode;

    if opts.high_temp <= opts.low_temp {
        return Err(anyhow!("--high must be > --low"));
    }

    let manual = match (&opts.sunrise, &opts.sunset) {
        (Some(a), Some(b)) => Some((parse_hhmm(a)?, parse_hhmm(b)?)),
        (None, None) => None,
        _ => return Err(anyhow!("Provide both --sunrise and --sunset or neither")),
    };

    let (lat, lon) = match manual {
        Some(_) => (0.0, 0.0),
        None => {
            let lat = opts.latitude;
            let lon = opts.longitude;
            match (lat, lon) {
                (Some(a), Some(b)) => (a, b),
                _ => {
                    eprintln!("Resolving location via GeoClue...");
                    geoclue_lat_lon("wlsunset-rs.desktop").context("GeoClue failed")?
                }
            }
        }
    };

    let shared_state = Arc::new(Mutex::new(SharedAppState::new(
        opts.low_temp,
        opts.high_temp,
    )));
    {
        let mut state = shared_state.lock().unwrap();
        state.requested_mode = startup_mode;
    }

    let (mode_tx, mut mode_rx) = tokio::sync::mpsc::unbounded_channel::<ModeArg>();

    if let Some(socket_path) = &opts.socket {
        let shared_state_clone = Arc::clone(&shared_state);
        let socket_path = socket_path.clone();
        tokio::spawn(async move {
            if let Err(e) = start_socket_server(shared_state_clone, mode_tx, &socket_path).await {
                eprintln!("Socket server error: {}", e);
            }
        });
    }

    let mut tray_override: Option<TrayOverride> = None;
    let mut initial_override_pending = if matches!(startup_mode, ModeArg::Day | ModeArg::Night) {
        Some(startup_mode)
    } else {
        None
    };

    let conn = Connection::connect_to_env().context("connect wayland display")?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    conn.display().get_registry(&qh, ());

    let mut state = AppState::new();
    event_queue
        .roundtrip(&mut state)
        .context("initial wayland roundtrip")?;
    if state.gamma_mgr.is_none() {
        return Err(anyhow!("Compositor lacks wlr-gamma-control-unstable-v1"));
    }
    state.ensure_gamma_all(&qh);
    event_queue
        .roundtrip(&mut state)
        .context("gamma setup roundtrip")?;

    let mut sigusr1 = signal(SignalKind::user_defined1()).context("setup SIGUSR1 handler")?;

    loop {
        event_queue
            .dispatch_pending(&mut state)
            .context("dispatch pending")?;

        let now = Local::now().timestamp();
        let stops = compute_day_stops(now, lat, lon, opts.duration, manual)?;
        let mut temp = temperature_for(now, stops, opts.low_temp, opts.high_temp);
        let natural_phase = phase_for(now, stops);
        let mut applied_phase = natural_phase;

        if let Some(mode) = initial_override_pending.take() {
            let expires_at = next_sunrise_timestamp(now, stops, lat, lon, opts.duration, manual)?;
            tray_override = Some(TrayOverride { mode, expires_at });
        }


        let override_expired = tray_override
            .as_ref()
            .map_or(false, |state| now >= state.expires_at);
        if override_expired {
            tray_override = None;
        }

        if let Some(state) = tray_override.as_ref() {
            match state.mode {
                ModeArg::Auto => {}
                ModeArg::Day => {
                    applied_phase = DayPhase::Day;
                    temp = opts.high_temp;
                }
                ModeArg::Night => {
                    applied_phase = DayPhase::Night;
                    temp = opts.low_temp;
                }
                ModeArg::Sunset => {
                    applied_phase = DayPhase::Sunset;
                    temp = (opts.low_temp + opts.high_temp) / 2;
                }
            }
        }

        // Update shared state with current and automatic phases
        {
            let mut shared = shared_state.lock().unwrap();
            shared.current_mode = applied_phase;
            shared.automatic_mode = natural_phase;
            shared.current_temp = temp;
        }

        set_temperature_all(&mut state.outputs, temp, 1.0);
        conn.flush().context("flush wayland connection")?;

        let next = if now < stops.dawn {
            stops.dawn
        } else if now < stops.sunrise {
            now + 10
        } else if now < stops.sunset {
            stops.sunset
        } else if now < stops.night {
            now + 10
        } else {
            ((now / 86400) + 1) * 86400
        };
        let wait_ms = ((next - now).max(1) * 1000) as i64;

        tokio::select! {
            _ = sigusr1.recv() => {
                // Signal received, continue loop
            }
            Some(mode) = mode_rx.recv() => {
                // Mode change received, process immediately
                eprintln!("★ Received mode change from socket: {:?}", mode);
                match mode {
                    ModeArg::Auto => {
                        tray_override = None;
                    }
                    ModeArg::Day | ModeArg::Night | ModeArg::Sunset => {
                        let expires_at =
                            next_sunrise_timestamp(now, stops, lat, lon, opts.duration, manual)?;
                        tray_override = Some(TrayOverride { mode, expires_at });
                    }
                }
                // Restart loop immediately to apply the new temperature
                continue;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms as u64)) => {
                // Timeout, continue loop
            }
        }

        // Check for wayland events after potential signal/timeout
        if let Some(guard) = event_queue.prepare_read() {
            let conn_fd = guard.connection_fd();
            let mut fds = [PollFd::new(
                conn_fd,
                PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP,
            )];
            match poll(&mut fds, PollTimeout::ZERO) {
                Ok(0) => {
                    // no events, drop guard to cancel read
                }
                Ok(_) => {
                    let conn_ready = fds[0].revents().map_or(false, |flags| {
                        flags
                            .intersects(PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP)
                    });
                    if conn_ready {
                        if let Err(err) = guard.read() {
                            eprintln!("Failed to read wayland events: {err}");
                        }
                    } else {
                        drop(guard);
                    }
                }
                Err(err) => {
                    if err == nix::errno::Errno::EINTR {
                        drop(guard);
                    } else {
                        return Err(err.into());
                    }
                }
            }
        }
    }
}
