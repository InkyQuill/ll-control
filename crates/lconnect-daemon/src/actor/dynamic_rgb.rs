//! Windows 2.1.28 Galahad2VisionController:499–568 and HSVColor integer arithmetic.
use lconnect_ipc::{DynamicRgbConfig, DynamicRgbSample, Rgb, SensorReading, SensorStatus};

/// Invalid/unavailable samples are explicit: the owner restores its ordinary base.
pub fn evaluate(config: &DynamicRgbConfig, rows: &[SensorReading]) -> Option<DynamicRgbSample> {
    config.validate().ok()?;
    let row = rows
        .iter()
        .find(|r| r.id == config.sensor_id && r.unit == config.unit)?;
    if row.status != SensorStatus::Valid || row.age_ms.is_none_or(|age| age > 3000) {
        return None;
    }
    let maximum = if config.unit == lconnect_ipc::SensorUnit::Rpm {
        3600
    } else {
        100
    };
    let value = (row.value.filter(|v| v.is_finite())?.trunc() as i32).clamp(0, maximum);
    let low = i32::from(config.low_value);
    let high = i32::from(config.high_value);
    // Windows tests >= high before <= low. Equal thresholds form a step,
    // never divide by zero in the interpolation branch.
    let ratio = if value >= high {
        1.0
    } else if value <= low {
        0.0
    } else {
        f64::from(value - low) / f64::from(high - low)
    };
    let colors = config
        .low_colors
        .iter()
        .zip(&config.high_colors)
        .map(|(&low, &high)| {
            if ratio == 0.0 {
                low
            } else if ratio == 1.0 {
                high
            } else {
                interpolate(ratio, low, high)
            }
        })
        .collect();
    let speed_percent = config
        .low_speed_percent
        .zip(config.high_speed_percent)
        .map(|(low, high)| map(ratio, i32::from(low), i32::from(high)) as u8);
    Some(DynamicRgbSample {
        value,
        colors,
        speed_percent,
    })
}
fn map(ratio: f64, low: i32, high: i32) -> i32 {
    (ratio * f64::from(high - low) + f64::from(low)) as i32
}
fn hsv(rgb: Rgb) -> [i32; 3] {
    let r = f64::from(rgb.r) / 255.0;
    let g = f64::from(rgb.g) / 255.0;
    let b = f64::from(rgb.b) / 255.0;
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        (g - b) / delta % 6.0
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    // Reference Hue setter clamps a negative remainder instead of wrapping it.
    // Preserve that source-derived behavior; endpoint colors remain exact RGB.
    [
        ((60.0 * hue).floor() as i32 % 360).clamp(0, 360),
        (if max == 0.0 { 0.0 } else { delta / max * 100.0 }).floor() as i32,
        (max * 100.0).floor() as i32,
    ]
}
fn interpolate(ratio: f64, low: Rgb, high: Rgb) -> Rgb {
    let a = hsv(low);
    let b = hsv(high);
    let h = map(ratio, a[0], b[0]);
    let saturation = f64::from(map(ratio, a[1], b[1])) / 100.0;
    let value = f64::from(map(ratio, a[2], b[2])) / 100.0;
    let c = value * saturation;
    let x = c * (1.0 - (f64::from(h) / 60.0 % 2.0 - 1.0).abs());
    let (r, g, b) = match h / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    Rgb {
        r: ((r + m) * 255.0) as u8,
        g: ((g + m) * 255.0) as u8,
        b: ((b + m) * 255.0) as u8,
    }
}
