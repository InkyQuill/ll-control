//! Offline inspection: rows Rain/Snow, columns low at0ms, high at0ms/high at42ms.
use image::{imageops, Rgb, RgbImage};
use llcontrol_image::effects::draw_weather;
use llcontrol_ipc::{WeatherEffect, WeatherKind};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("provide an output PNG path")?;
    let mut sheet = RgbImage::new(480 * 3, 480 * 2);
    for (row, kind) in [WeatherKind::Rain, WeatherKind::Snow]
        .into_iter()
        .enumerate()
    {
        for (column, (intensity, millis)) in [(1, 0), (3, 0), (3, 42)].into_iter().enumerate() {
            let mut canvas = RgbImage::from_fn(480, 480, |x, y| {
                let shade = if (x / 60 + y / 60) % 2 == 0 { 20 } else { 40 };
                Rgb([shade, shade + (x / 8) as u8, shade + (y / 8) as u8])
            });
            draw_weather(
                &mut canvas,
                &WeatherEffect {
                    retained_sensors: Vec::new(),
                    presets: Vec::new(),
                    kind,
                    intensity,
                    sensor: None,
                    dynamic: false,
                    seed: 42,
                },
                &[],
                millis,
            )?;
            imageops::overlay(&mut sheet, &canvas, column as i64 * 480, row as i64 * 480);
        }
    }
    sheet.save(path)?;
    Ok(())
}
