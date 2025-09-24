pub use tempergb::Color as Rgb;

pub fn blackbody_whitepoint_kelvin(k: i32) -> Rgb {
    tempergb::rgb_from_temperature(k)
}

pub fn fill_gamma_table(buf: &mut [u16], ramp_size: usize, wp: Rgb, gamma: f64) {
    for i in 0..ramp_size {
        let val = i as f64 / (ramp_size as f64 - 1.0);

        let corrected_r = ((val * wp.r() as f64 / 255.0) as f32).powf(1.0 / gamma as f32);
        let corrected_g = ((val * wp.g() as f64 / 255.0) as f32).powf(1.0 / gamma as f32);
        let corrected_b = ((val * wp.b() as f64 / 255.0) as f32).powf(1.0 / gamma as f32);

        let rr = (corrected_r.max(0.0).min(1.0) as f64 * u16::MAX as f64).round() as u16;
        let gg = (corrected_g.max(0.0).min(1.0) as f64 * u16::MAX as f64).round() as u16;
        let bb = (corrected_b.max(0.0).min(1.0) as f64 * u16::MAX as f64).round() as u16;

        buf[i] = rr;
        buf[i + ramp_size] = gg;
        buf[i + 2 * ramp_size] = bb;
    }
}
