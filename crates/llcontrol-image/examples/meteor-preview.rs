//! Offline preview of owned MeteorShower strokes; no daemon or device access.
use image::{imageops, Rgb, RgbImage};
use llcontrol_image::meteor::draw_meteor;
use llcontrol_ipc::MeteorEffect;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("provide an output PNG path")?;
    let mut sheet = RgbImage::new(960, 480);
    for (column, millis) in [500, 542].into_iter().enumerate() {
        let mut frame = RgbImage::from_pixel(480, 480, Rgb([12, 18, 30]));
        draw_meteor(&mut frame, &MeteorEffect::default(), &[], millis)?;
        imageops::overlay(&mut sheet, &frame, column as i64 * 480, 0);
    }
    sheet.save(path)?;
    Ok(())
}
