//! Offline rockets/bursts inspection; no daemon or device access.
use image::{imageops, Rgb, RgbImage};
use lconnect_image::fireworks::draw_fireworks;
use lconnect_ipc::FireworksEffect;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("provide an output PNG path")?;
    let mut sheet = RgbImage::new(960, 480);
    for (column, millis) in [3000, 3042].into_iter().enumerate() {
        let mut frame = RgbImage::from_pixel(480, 480, Rgb([12, 18, 30]));
        draw_fireworks(&mut frame, &FireworksEffect::default(), &[], millis)?;
        imageops::overlay(&mut sheet, &frame, column as i64 * 480, 0);
    }
    sheet.save(path)?;
    Ok(())
}
