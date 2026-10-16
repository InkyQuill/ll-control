//! Owned bounded MeteorShower scene, independent of vendor scripts/assets.
use image::RgbImage;
use lconnect_ipc::{MeteorEffect, MeteorParameters, SensorReading};
#[derive(Debug, thiserror::Error)]
pub enum MeteorError {
    #[error("{0}")]
    Configuration(#[from] lconnect_ipc::IpcError),
    #[error("meteor canvas must be 480x480")]
    FrameSize,
    #[error("meteor scene time cannot move backwards")]
    TimeReversed,
}
/// One scene per live theme. Speed changes affect future motion rather than
/// recomputing old positions; preview owns a separate stateless timeline.
pub struct MeteorScene {
    effect: MeteorEffect,
    last_millis: Option<u64>,
    motion: u128,
    last_speed: u8,
}
impl MeteorScene {
    pub fn new(effect: MeteorEffect) -> Result<Self, MeteorError> {
        let speed = effect.parameters(&[])?.speed;
        Ok(Self {
            effect,
            last_millis: None,
            motion: 0,
            last_speed: speed,
        })
    }
    pub fn effect(&self) -> &MeteorEffect {
        &self.effect
    }
    pub fn render(
        &mut self,
        canvas: &mut RgbImage,
        readings: &[SensorReading],
        elapsed_millis: u64,
    ) -> Result<MeteorParameters, MeteorError> {
        if canvas.dimensions() != (480, 480) {
            return Err(MeteorError::FrameSize);
        }
        let parameters = self.effect.parameters(readings)?;
        let previous = self.last_millis.unwrap_or(0);
        if elapsed_millis < previous {
            return Err(MeteorError::TimeReversed);
        }
        let speed = if self.last_millis.is_some() {
            self.last_speed
        } else {
            parameters.speed
        };
        let motion = self.motion + u128::from(elapsed_millis - previous) * u128::from(speed);
        draw_resolved(canvas, self.effect.seed, parameters, motion);
        self.last_millis = Some(elapsed_millis);
        self.last_speed = parameters.speed;
        self.motion = motion;
        Ok(parameters)
    }
}
pub fn draw_meteor(
    canvas: &mut RgbImage,
    effect: &MeteorEffect,
    readings: &[SensorReading],
    elapsed_millis: u64,
) -> Result<MeteorParameters, MeteorError> {
    MeteorScene::new(effect.clone())?.render(canvas, readings, elapsed_millis)
}
fn random(seed: u32, index: u32) -> f64 {
    let mut value = seed.wrapping_add(index.wrapping_mul(0x9e3779b9));
    value ^= value >> 16;
    value = value.wrapping_mul(0x85ebca6b);
    value ^= value >> 13;
    value = value.wrapping_mul(0xc2b2ae35);
    value ^= value >> 16;
    f64::from(value) / f64::from(u32::MAX)
}
pub(crate) fn hsb_rgb([hue, saturation, brightness]: [f64; 3]) -> [u8; 3] {
    let hue = hue.rem_euclid(360.0) / 60.0;
    let s = saturation / 100.0;
    let v = brightness / 100.0;
    let chroma = v * s;
    let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let m = v - chroma;
    let rgb = match hue as u8 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    rgb.map(|channel| ((channel + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}
fn draw_resolved(canvas: &mut RgbImage, seed: u32, parameters: MeteorParameters, motion: u128) {
    let colors = parameters.hsb.map(hsb_rgb);
    // Native uses500 falling streaks, lengths85..115 and angle−π/8. This
    // deterministic scene uses bounded analytic acceleration/reset cycles;
    // vendor stochastic trails still require separate visual qualification.
    let angle = -std::f64::consts::PI / 8.0;
    for index in 0..500u32 {
        let period = 10000 + (random(seed, index * 5) * 10000.0) as u128;
        let phase =
            ((motion % period) as f64 / period as f64 + random(seed, index * 5 + 1)).fract();
        let distance = phase * phase * 2000.0;
        let x = -480.0 + random(seed, index * 5 + 2) * 1440.0 + distance * angle.sin();
        let y = -1200.0 + random(seed, index * 5 + 3) * 960.0 + distance * angle.cos();
        let length = 85 + (random(seed, index * 5 + 4) * 30.0) as i32;
        for step in 0..=length {
            let px = (x + f64::from(step) * angle.sin()).round() as i32;
            let py = (y + f64::from(step) * angle.cos()).round() as i32;
            for offset in 0..2 {
                blend(canvas, px + offset, py, colors[index as usize % 2], 190);
            }
        }
    }
}
fn blend(canvas: &mut RgbImage, x: i32, y: i32, color: [u8; 3], alpha: u8) {
    if !(0..480).contains(&x) || !(0..480).contains(&y) {
        return;
    }
    let pixel = canvas.get_pixel_mut(x as u32, y as u32);
    for (channel, color) in pixel.0.iter_mut().zip(color) {
        *channel = ((u32::from(*channel) * (255 - u32::from(alpha))
            + u32::from(color) * u32::from(alpha)
            + 127)
            / 255) as u8;
    }
}
