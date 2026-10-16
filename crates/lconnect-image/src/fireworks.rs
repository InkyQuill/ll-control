//! Bounded seeded rocket/burst simulation, with colors captured at launch.
use image::RgbImage;
use lconnect_ipc::{FireworksEffect, FireworksParameters, SensorReading};
#[derive(Debug, thiserror::Error)]
pub enum FireworksError {
    #[error("{0}")]
    Configuration(#[from] lconnect_ipc::IpcError),
    #[error("fireworks canvas must be 480x480")]
    FrameSize,
    #[error("fireworks scene time cannot move backwards")]
    TimeReversed,
}
#[derive(Clone)]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    color: [u8; 3],
    size: f64,
    life: f64,
    initial_life: f64,
}
pub struct FireworksScene {
    effect: FireworksEffect,
    last_millis: Option<u64>,
    last_parameters: FireworksParameters,
    tick: u128,
    rockets: Vec<Particle>,
    sparks: Vec<Particle>,
}
impl FireworksScene {
    pub fn new(effect: FireworksEffect) -> Result<Self, FireworksError> {
        let parameters = effect.parameters(&[])?;
        Ok(Self {
            effect,
            last_millis: None,
            last_parameters: parameters,
            tick: 0,
            rockets: Vec::new(),
            sparks: Vec::new(),
        })
    }
    pub fn effect(&self) -> &FireworksEffect {
        &self.effect
    }
    pub fn render(
        &mut self,
        canvas: &mut RgbImage,
        readings: &[SensorReading],
        millis: u64,
    ) -> Result<FireworksParameters, FireworksError> {
        if canvas.dimensions() != (480, 480) {
            return Err(FireworksError::FrameSize);
        }
        let parameters = self.effect.parameters(readings)?;
        if self.last_millis.is_some_and(|last| millis < last) {
            return Err(FireworksError::TimeReversed);
        }
        let target = u128::from(millis) * 60 / 1000;
        let previous = if self.last_millis.is_some() {
            self.last_parameters
        } else {
            parameters
        };
        // A paused/lost renderer may skip years. Bound catch-up to10s, clearing
        // old particles and warming a deterministic recent interval. No huge loop.
        if target - self.tick > 600 {
            self.rockets.clear();
            self.sparks.clear();
            self.tick = target - 600;
        }
        while self.tick < target {
            self.tick += 1;
            self.step(previous);
        }
        for particle in self.rockets.iter().chain(&self.sparks) {
            draw_particle(canvas, particle);
        }
        self.last_millis = Some(millis);
        self.last_parameters = parameters;
        Ok(parameters)
    }
    fn step(&mut self, parameters: FireworksParameters) {
        if self.tick % 6001 == 0 {
            self.rockets.clear();
            self.sparks.clear();
        }
        let frame = (self.tick as u64) ^ ((self.tick >> 64) as u64);
        let seed = self.effect.seed ^ frame as u32 ^ ((frame >> 32) as u32).rotate_left(13);
        if random(seed, 0) < 0.7 * parameters.rate {
            let color = crate::meteor::hsb_rgb(parameters.hsb[(random(seed, 1) * 6.0) as usize]);
            self.rockets.push(Particle {
                x: random(seed, 2) * 480.0,
                y: 480.0,
                vx: random(seed, 3) * 2.0 - 1.0,
                vy: -(480.0 * 0.04f64).sqrt() * (1.0 + random(seed, 4) * (1.95f64.sqrt() - 1.0)),
                color,
                size: 7.0 + random(seed, 5) * 3.0,
                life: 1.0,
                initial_life: 1.0,
            });
        }
        for spark in &mut self.sparks {
            spark.x += spark.vx;
            spark.y += spark.vy;
            spark.vy += 0.04;
            spark.life -= 0.7;
        }
        self.sparks.retain(|spark| spark.life > 0.0);
        for (index, rocket) in self.rockets.iter_mut().enumerate() {
            rocket.x += rocket.vx;
            rocket.y += rocket.vy;
            rocket.vy += 0.04;
            if rocket.vy >= 0.0 {
                for burst in 0..40u32 {
                    let key = burst * 6 + (index as u32) * 257;
                    let angle = random(seed, key + 6) * std::f64::consts::TAU;
                    let speed = 2.0 + random(seed, key + 7) * 3.0;
                    let life =
                        (40.0 + random(seed, key + 8) * 20.0) * (0.7 + random(seed, key + 9) * 0.6);
                    self.sparks.push(Particle {
                        x: rocket.x,
                        y: rocket.y,
                        vx: angle.cos() * speed,
                        vy: angle.sin() * speed,
                        color: rocket.color,
                        size: 2.0 + random(seed, key + 10) * 2.0,
                        life,
                        initial_life: life,
                    });
                }
            }
        }
        self.rockets.retain(|rocket| rocket.vy < 0.0);
        // Native clears overflowing pools rather than growing without a bound.
        if self.sparks.len() > 450 {
            self.sparks.clear();
        }
        if self.rockets.len() > 250 {
            self.rockets.clear();
        }
    }
}
pub fn draw_fireworks(
    canvas: &mut RgbImage,
    effect: &FireworksEffect,
    readings: &[SensorReading],
    millis: u64,
) -> Result<FireworksParameters, FireworksError> {
    FireworksScene::new(effect.clone())?.render(canvas, readings, millis)
}
fn random(seed: u32, index: u32) -> f64 {
    let mut value = seed.wrapping_add(index.wrapping_mul(0x9e3779b9));
    value ^= value >> 16;
    value = value.wrapping_mul(0x85ebca6b);
    value ^= value >> 13;
    value = value.wrapping_mul(0xc2b2ae35);
    value ^= value >> 16;
    f64::from(value) / 4294967296.0
}
fn draw_particle(canvas: &mut RgbImage, particle: &Particle) {
    let radius = particle.size * (particle.life / particle.initial_life) / 2.0;
    let extent = radius + 12.0;
    let left = (particle.x - extent).floor().max(0.0) as i32;
    let right = (particle.x + extent).ceil().min(479.0) as i32;
    let top = (particle.y - extent).floor().max(0.0) as i32;
    let bottom = (particle.y + extent).ceil().min(479.0) as i32;
    for y in top..=bottom {
        for x in left..=right {
            let distance =
                ((f64::from(x) - particle.x).powi(2) + (f64::from(y) - particle.y).powi(2)).sqrt();
            let alpha = if distance <= radius {
                255
            } else {
                ((1.0 - (distance - radius) / 12.0).max(0.0).powi(3) * 80.0).round() as u32
            };
            let pixel = canvas.get_pixel_mut(x as u32, y as u32);
            for (channel, color) in pixel.0.iter_mut().zip(particle.color) {
                *channel = ((u32::from(*channel) * (255 - alpha) + u32::from(color) * alpha + 127)
                    / 255) as u8;
            }
        }
    }
}
