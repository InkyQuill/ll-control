//! Seeded perspective snowfall over caller-owned backgrounds; no vendor sprite.
use image::RgbImage;
use lconnect_ipc::Snow3dEffect;
#[derive(Debug, thiserror::Error)]
pub enum Snow3dError {
    #[error("{0}")]
    Configuration(#[from] lconnect_ipc::IpcError),
    #[error("Snow3D canvas must be 480x480")]
    FrameSize,
}
pub fn draw_snow3d(
    canvas: &mut RgbImage,
    effect: &Snow3dEffect,
    millis: u64,
) -> Result<(), Snow3dError> {
    if canvas.dimensions() != (480, 480) {
        return Err(Snow3dError::FrameSize);
    }
    let parameters = effect.parameters()?;
    let camera = 480.0 / 1.8;
    let focal = 240.0 / (37.5f64.to_radians().tan());
    // Native uses requestAnimationFrame rather than a fixed simulation FPS.
    // Encoding remains24fps; nominal60Hz motion needs Windows timing qualification.
    let ticks = millis as f64 * 0.06;
    for index in 0..u32::from(parameters.particles) {
        let angle = (-65.0 + (random(effect.seed, index * 5) * 131.0).floor()).to_radians();
        let phi = (random(effect.seed, index * 5 + 1) * 270.0)
            .floor()
            .to_radians();
        let vy = -f64::from(parameters.speed) / 50.0 * angle.cos();
        let vx = -vy * phi.sin() * angle.sin();
        let vz = -phi.sin() * vx;
        let wrap = |position: f64| (position + camera).rem_euclid(2.0 * camera) - camera;
        let x = wrap((random(effect.seed, index * 5 + 2) * 960.0).floor() - 480.0 + vx * ticks);
        let y = wrap((random(effect.seed, index * 5 + 3) * 960.0).floor() - 480.0 + vy * ticks);
        let z = wrap((random(effect.seed, index * 5 + 4) * camera).floor() + vz * ticks);
        // Native shader uses homogeneous w=.9 and perspective FOV75°.
        let depth = 0.9 * camera - z;
        if depth < 1.0 {
            continue;
        }
        let distance = (x * x + y * y + depth * depth).sqrt();
        let radius = 3200.0 / distance;
        let cx = 240.0 + focal * x / depth;
        let cy = 240.0 - focal * y / depth;
        let camera_distance = (x * x + y * y + (camera - z).powi(2)).sqrt();
        let opacity = (1.0 - camera_distance / 1500.0).max(0.0).powi(2);
        sprite(canvas, cx, cy, radius, opacity);
    }
    Ok(())
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
fn sprite(canvas: &mut RgbImage, cx: f64, cy: f64, radius: f64, opacity: f64) {
    // Clip before looping, so close particles never allocate/iterate giant sprites.
    let left = (cx - radius).floor().max(0.0) as i32;
    let right = (cx + radius).ceil().min(479.0) as i32;
    let top = (cy - radius).floor().max(0.0) as i32;
    let bottom = (cy + radius).ceil().min(479.0) as i32;
    for y in top..=bottom {
        for x in left..=right {
            let dx = (f64::from(x) - cx) / radius;
            let dy = (f64::from(y) - cy) / radius;
            let r2 = dx * dx + dy * dy;
            if r2 >= 1.0 {
                continue;
            }
            // Owned soft white snow sprite. Vendor texture/raster comparison remains open.
            let alpha = ((1.0 - r2).powi(3) * opacity * 255.0).round() as u32;
            let pixel = canvas.get_pixel_mut(x as u32, y as u32);
            for channel in &mut pixel.0 {
                *channel = ((u32::from(*channel) * (255 - alpha) + 255 * alpha + 127) / 255) as u8;
            }
        }
    }
}
