use anyhow::{Result, anyhow};
use chrono::Duration;
use sunrise::{Coordinates, SolarDay, SolarEvent};

use crate::cli::ModeArg;

#[derive(Copy, Clone, Debug)]
pub struct DayStops {
    pub dawn: i64,
    pub sunrise: i64,
    pub sunset: i64,
    pub night: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayPhase {
    Night,
    Sunrise,
    Day,
    Sunset,
}

pub struct TrayOverride {
    pub mode: ModeArg,
    pub expires_at: i64,
}

pub fn parse_hhmm(s: &str) -> Result<i64> {
    let parts: Vec<_> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("invalid time format"));
    }
    let h: i64 = parts[0].parse()?;
    let m: i64 = parts[1].parse()?;
    Ok(h * 3600 + m * 60)
}

pub fn compute_day_stops(
    now: i64,
    lat: f64,
    lon: f64,
    duration: i64,
    manual: Option<(i64, i64)>,
) -> Result<DayStops> {
    let midnight = now - (now % 86400);
    if let Some((sunrise_s, sunset_s)) = manual {
        let dawn = sunrise_s - duration;
        let night = sunset_s + duration;
        return Ok(DayStops {
            dawn: midnight + dawn,
            sunrise: midnight + sunrise_s,
            sunset: midnight + sunset_s,
            night: midnight + night,
        });
    }
    let coords = Coordinates::new(lat, lon).ok_or_else(|| anyhow!("invalid coordinates"))?;
    let date = chrono::DateTime::from_timestamp(now, 0)
        .ok_or_else(|| anyhow!("invalid timestamp"))?
        .date_naive();
    let solar_day = SolarDay::new(coords, date);
    let sunrise_ts = solar_day
        .event_time(SolarEvent::Sunrise)
        .timestamp();
    let sunset_ts = solar_day
        .event_time(SolarEvent::Sunset)
        .timestamp();
    Ok(DayStops {
        dawn: sunrise_ts - duration,
        sunrise: sunrise_ts,
        sunset: sunset_ts,
        night: sunset_ts + duration,
    })
}

pub fn next_sunrise_timestamp(
    now: i64,
    current: DayStops,
    lat: f64,
    lon: f64,
    duration: i64,
    manual: Option<(i64, i64)>,
) -> Result<i64> {
    if now < current.sunrise {
        return Ok(current.sunrise);
    }
    // Calculate next day's stops properly using chrono to handle DST
    let current_dt = chrono::DateTime::from_timestamp(now, 0)
        .ok_or_else(|| anyhow!("invalid timestamp"))?;
    let tomorrow_dt = current_dt + Duration::days(1);
    let tomorrow = tomorrow_dt.timestamp();
    let next = compute_day_stops(tomorrow, lat, lon, duration, manual)?;
    Ok(next.sunrise)
}

pub fn interpolate(now: i64, start: i64, stop: i64, a: i32, b: i32) -> i32 {
    if start == stop {
        return b;
    }
    let t = ((now - start) as f64 / (stop - start) as f64).clamp(0.0, 1.0);
    let v = a as f64 + (b - a) as f64 * t;
    v.round() as i32
}

pub fn temperature_for(now: i64, stops: DayStops, low: i32, high: i32) -> i32 {
    if now < stops.dawn {
        low
    } else if now < stops.sunrise {
        interpolate(now, stops.dawn, stops.sunrise, low, high)
    } else if now < stops.sunset {
        high
    } else if now < stops.night {
        interpolate(now, stops.sunset, stops.night, high, low)
    } else {
        low
    }
}

pub fn phase_for(now: i64, stops: DayStops) -> DayPhase {
    if now < stops.dawn {
        DayPhase::Night
    } else if now < stops.sunrise {
        DayPhase::Sunrise
    } else if now < stops.sunset {
        DayPhase::Day
    } else if now < stops.night {
        DayPhase::Sunset
    } else {
        DayPhase::Night
    }
}
