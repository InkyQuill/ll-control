//! Stateless procedural weather frames. All motion uses the supplied session time.
//! No device/filesystem/clock access, vendor assets or unbounded particle history.
use image::RgbImage;
use lconnect_ipc::{SensorReading, WeatherEffect, WeatherKind, WeatherParameters};

#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("{0}")]
    Configuration(#[from] lconnect_ipc::IpcError),
    #[error("weather canvas must be 480x480")]
    FrameSize,
    #[error("weather scene time cannot move backwards")]
    TimeReversed,
}

/// Owned motion state for live dynamic weather, independent of the source cache.
/// Speed changes affect future movement; no sensor transition teleports a flake.
pub struct WeatherScene {
    effect: WeatherEffect,
    last_millis: Option<u64>,
    snow_motion: u128,
    last_amount: u8,
}
impl WeatherScene {
    pub fn effect(&self) -> &WeatherEffect {
        &self.effect
    }
    pub fn new(effect: WeatherEffect) -> Result<Self, EffectError> {
        let amount = effect.parameters(&[])?.amount;
        Ok(Self {
            effect,
            last_millis: None,
            snow_motion: 0,
            last_amount: amount,
        })
    }
    /// Render with monotonic session time. Rewind is explicit: create a new scene.
    /// Invalid input does not advance state or mutate the canvas.
    pub fn render(
        &mut self,
        canvas: &mut RgbImage,
        readings: &[SensorReading],
        elapsed_millis: u64,
    ) -> Result<WeatherParameters, EffectError> {
        if canvas.dimensions() != (480, 480) {
            return Err(EffectError::FrameSize);
        }
        let parameters = self.effect.parameters(readings)?;
        let previous = self.last_millis.unwrap_or(0);
        if elapsed_millis < previous {
            return Err(EffectError::TimeReversed);
        }
        let speed = if self.last_millis.is_some() {
            self.last_amount
        } else {
            parameters.amount
        };
        let motion =
            self.snow_motion + u128::from(elapsed_millis - previous) * u128::from(speed / 10);
        draw_resolved(canvas, &self.effect, parameters, elapsed_millis, motion);
        self.snow_motion = motion;
        self.last_millis = Some(elapsed_millis);
        self.last_amount = parameters.amount;
        Ok(parameters)
    }
}

/// Draw over the caller's background; later theme layers can cover the effect.
/// Stateless preview of fixed parameters. Live dynamic rendering uses WeatherScene
/// to retain velocity history. Milliseconds permit24 positions per second.
pub fn draw_weather(
    canvas: &mut RgbImage,
    effect: &WeatherEffect,
    readings: &[SensorReading],
    elapsed_millis: u64,
) -> Result<WeatherParameters, EffectError> {
    if canvas.dimensions() != (480, 480) {
        return Err(EffectError::FrameSize);
    }
    let parameters = effect.parameters(readings)?;
    let motion = u128::from(elapsed_millis) * u128::from(parameters.amount / 10);
    draw_resolved(canvas, effect, parameters, elapsed_millis, motion);
    Ok(parameters)
}
fn draw_resolved(
    canvas: &mut RgbImage,
    effect: &WeatherEffect,
    parameters: WeatherParameters,
    elapsed_millis: u64,
    snow_motion: u128,
) {
    match effect.kind {
        WeatherKind::Rain => rain(canvas, effect.seed, parameters.amount, elapsed_millis),
        WeatherKind::Snow => snow(canvas, effect.seed, parameters.amount, snow_motion),
    }
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
fn rain(canvas: &mut RgbImage, seed: u32, rate: u8, millis: u64) {
    // Each drop refracts the immutable background rather than other drops.
    let background = canvas.clone();
    for index in 0..u32::from(rate) * 64 {
        let radius = 2 + (random(seed, index * 5) * 5.0) as i32;
        let period = 7000 + (random(seed, index * 5 + 1) * 7000.0) as u64;
        let phase =
            ((millis % period) as f64 / period as f64 + random(seed, index * 5 + 2)).fract();
        let x = (random(seed, index * 5 + 3) * 480.0) as i32;
        let y = (phase * f64::from(480 + radius * 2)) as i32 - radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let distance = dx * dx + dy * dy;
                if distance > radius * radius {
                    continue;
                }
                let px = x + dx;
                let py = y + dy;
                if !(0..480).contains(&px) || !(0..480).contains(&py) {
                    continue;
                }
                let edge = distance >= (radius - 1) * (radius - 1);
                if edge {
                    blend(
                        canvas,
                        px,
                        py,
                        if dy < 0 {
                            [225, 235, 245]
                        } else {
                            [12, 20, 30]
                        },
                        100,
                    );
                } else {
                    let sx = (px + dx / 2).clamp(0, 479) as u32;
                    let sy = (py + dy / 2).clamp(0, 479) as u32;
                    blend(canvas, px, py, background.get_pixel(sx, sy).0, 180);
                }
            }
        }
        blend(canvas, x - 1, y - radius + 1, [240, 248, 255], 170);
    }
}
fn snow(canvas: &mut RgbImage, seed: u32, speed: u8, motion: u128) {
    for index in 0..210 {
        let period = 18000 + (random(seed, index * 4) * 16000.0) as u64;
        // Bounded modular phase remains defined even for u64::MAX session time.
        let phase = ((motion % u128::from(period)) as f64 / period as f64
            + random(seed, index * 4 + 1))
        .fract();
        let radius = 2 + (random(seed, index * 4 + 2) * 4.0) as i32;
        let base_x = (random(seed, index * 4 + 3) * 480.0) as i32;
        let wind = (phase * std::f64::consts::TAU).sin() * 18.0;
        let x = base_x + wind as i32;
        let y = (phase * 520.0) as i32 - 20;
        let copies = if speed > 30 {
            3
        } else if speed > 20 {
            2
        } else {
            1
        };
        for copy in 0..copies {
            let cx = (x + copy * 73).rem_euclid(480);
            let cy = (y + copy * 137 + 20).rem_euclid(520) - 20;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dy * dy <= radius * radius {
                        blend(
                            canvas,
                            cx + dx,
                            cy + dy,
                            [248, 250, 255],
                            if copy == 0 { 160 } else { 130 },
                        );
                    }
                }
            }
        }
    }
}
