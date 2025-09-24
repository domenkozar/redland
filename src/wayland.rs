use anyhow::Result;
use bytemuck;
use memmap2::MmapMut;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    protocol::{wl_output, wl_registry},
};
use wayland_protocols_wlr::gamma_control::v1::client::{
    zwlr_gamma_control_manager_v1, zwlr_gamma_control_v1,
};

use crate::color::{blackbody_whitepoint_kelvin, fill_gamma_table};

#[derive(Clone, Copy)]
pub struct OutputData {
    pub id: u32,
}

#[derive(Clone, Copy)]
pub struct GammaData {
    pub id: u32,
}

pub struct OutputState {
    pub name: Option<String>,
    pub wl_output: wl_output::WlOutput,
    pub gamma: Option<zwlr_gamma_control_v1::ZwlrGammaControlV1>,
    pub ramp_size: u32,
    pub table: Option<(File, MmapMut)>,
}

pub struct AppState {
    pub outputs: HashMap<u32, OutputState>,
    pub gamma_mgr: Option<zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1>,
    pub gamma_mgr_name: Option<u32>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            outputs: HashMap::new(),
            gamma_mgr: None,
            gamma_mgr_name: None,
        }
    }

    pub fn ensure_gamma_for(&mut self, qh: &QueueHandle<Self>, id: u32) {
        let Some(mgr) = self.gamma_mgr.clone() else {
            return;
        };
        if self
            .outputs
            .get(&id)
            .map(|o| o.gamma.is_some())
            .unwrap_or(false)
        {
            return;
        }
        let Some(wl_output) = self.outputs.get(&id).map(|o| o.wl_output.clone()) else {
            return;
        };
        let gamma = mgr.get_gamma_control(&wl_output, qh, GammaData { id });
        if let Some(output) = self.outputs.get_mut(&id) {
            output.gamma = Some(gamma);
        }
    }

    pub fn ensure_gamma_all(&mut self, qh: &QueueHandle<Self>) {
        let ids: Vec<u32> = self.outputs.keys().copied().collect();
        for id in ids {
            self.ensure_gamma_for(qh, id);
        }
    }

    pub fn remove_output(&mut self, id: u32) {
        self.outputs.remove(&id);
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface == wl_output::WlOutput::interface().name {
                    let wl_output = registry.bind::<wl_output::WlOutput, _, _>(
                        name,
                        version.min(4),
                        qh,
                        OutputData { id: name },
                    );
                    state.outputs.insert(
                        name,
                        OutputState {
                            name: None,
                            wl_output,
                            gamma: None,
                            ramp_size: 0,
                            table: None,
                        },
                    );
                    state.ensure_gamma_for(qh, name);
                } else if interface
                    == zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1::interface().name
                {
                    let mgr = registry
                        .bind::<zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1, _, _>(
                            name,
                            1,
                            qh,
                            (),
                        );
                    state.gamma_mgr = Some(mgr);
                    state.gamma_mgr_name = Some(name);
                    state.ensure_gamma_all(qh);
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                if state.gamma_mgr_name == Some(name) {
                    state.gamma_mgr = None;
                    state.gamma_mgr_name = None;
                }
                state.remove_output(name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, OutputData> for AppState {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        data: &OutputData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Name { name } => {
                if let Some(output) = state.outputs.get_mut(&data.id) {
                    output.name = Some(name);
                }
            }
            wl_output::Event::Description { description } => {
                if let Some(output) = state.outputs.get_mut(&data.id) {
                    output.name = Some(description);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<zwlr_gamma_control_v1::ZwlrGammaControlV1, GammaData> for AppState {
    fn event(
        state: &mut Self,
        _: &zwlr_gamma_control_v1::ZwlrGammaControlV1,
        event: zwlr_gamma_control_v1::Event,
        data: &GammaData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_gamma_control_v1::Event::GammaSize { size } => {
                if let Some(output) = state.outputs.get_mut(&data.id) {
                    output.ramp_size = size;
                    let table_bytes = size as usize * 3 * std::mem::size_of::<u16>();
                    match create_anonymous_file(table_bytes) {
                        Ok(file) => match unsafe { MmapMut::map_mut(&file) } {
                            Ok(mmap) => output.table = Some((file, mmap)),
                            Err(err) => {
                                eprintln!("mmap failed for output {:?}: {err}", output.name);
                                output.table = None;
                            }
                        },
                        Err(err) => {
                            eprintln!(
                                "Failed to allocate gamma table for output {:?}: {err}",
                                output.name
                            );
                            output.table = None;
                        }
                    }
                }
            }
            zwlr_gamma_control_v1::Event::Failed => {
                if let Some(output) = state.outputs.get_mut(&data.id) {
                    output.gamma = None;
                    output.table = None;
                    output.ramp_size = 0;
                }
            }
            _ => {}
        }
    }
}

delegate_noop!(AppState: ignore zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1);

pub fn create_anonymous_file(size: usize) -> Result<File> {
    let dir = "/tmp";
    let mut path = PathBuf::from(dir);
    path.push(format!("wlsunset-rs-{}", std::process::id()));
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .open(&path)?;
    f.set_len(size as u64)?;
    let _ = std::fs::remove_file(&path);
    Ok(f)
}

pub fn set_temperature_all(outputs: &mut HashMap<u32, OutputState>, kelvin: i32, gamma: f64) {
    let wp = blackbody_whitepoint_kelvin(kelvin);
    for output in outputs.values_mut() {
        let Some(ref gamma_obj) = output.gamma else {
            continue;
        };
        if output.ramp_size == 0 {
            continue;
        }
        let Some((file, mmap)) = output.table.as_mut() else {
            continue;
        };
        let ramp = output.ramp_size as usize;
        let u16_slice = bytemuck::cast_slice_mut::<u8, u16>(mmap);
        fill_gamma_table(u16_slice, ramp, wp, gamma);
        let _ = file.seek(SeekFrom::Start(0));
        eprintln!(
            "Applying gamma to output {:?} (ramp_size: {})",
            output.name, ramp
        );
        gamma_obj.set_gamma(file.as_fd());
    }
}
