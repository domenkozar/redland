use anyhow::{Context, Result, anyhow};
use std::thread;
use std::time::{Duration, Instant};
use zbus::blocking::Connection as ZbusConnection;

pub fn geoclue_lat_lon(desktop_id: &str) -> Result<(f64, f64)> {
    let conn = ZbusConnection::system().context("connect to system bus")?;
    let manager = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.GeoClue2",
        "/org/freedesktop/GeoClue2/Manager",
        "org.freedesktop.GeoClue2.Manager",
    )?;

    let client_path: zbus::zvariant::OwnedObjectPath = manager.call("CreateClient", &())?;
    let client = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.GeoClue2",
        client_path.as_str(),
        "org.freedesktop.GeoClue2.Client",
    )?;

    client.set_property("DesktopId", desktop_id)?;
    client.set_property("RequestedAccuracyLevel", 3u32)?;
    client.call::<_, (), ()>("Start", &())?;

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut loc_path: zbus::zvariant::OwnedObjectPath = client.get_property("Location")?;
    while {
        let path = loc_path.as_str();
        path.is_empty() || path == "/"
    } {
        if Instant::now() >= deadline {
            return Err(anyhow!("GeoClue did not provide a location"));
        }
        thread::sleep(Duration::from_millis(200));
        loc_path = client.get_property("Location")?;
    }
    let location = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.GeoClue2",
        loc_path.as_str(),
        "org.freedesktop.GeoClue2.Location",
    )?;

    let lat: f64 = location.get_property("Latitude")?;
    let lon: f64 = location.get_property("Longitude")?;
    Ok((lat, lon))
}
