//! GA2LCD spatial stages on premultiplied RGBA8, before final unpremultiply.
//! Nearest/clamp follows WPF's software Auto sampler; exact Windows raster
//! coordinates/rounding still require comparison with the installed baseline.
use image::RgbaImage;

fn byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub(crate) fn sharpen(pixels: &mut RgbaImage, amount: f32, input: (u32, u32)) {
    if pixels.width() == 0 || pixels.height() == 0 {
        return;
    }
    let source = pixels.clone();
    let dx = 1.0 / input.0 as f32;
    let dy = 1.0 / input.1 as f32;
    for (x, y, pixel) in pixels.enumerate_pixels_mut() {
        let before = source.get_pixel(
            sample_coordinate(x, -1.0, dx, source.width()),
            sample_coordinate(y, -1.0, dy, source.height()),
        );
        let after = source.get_pixel(
            sample_coordinate(x, 1.0, dx, source.width()),
            sample_coordinate(y, 1.0, dy, source.height()),
        );
        for c in 0..3 {
            // The baseline shader is asymmetric: centre + amount*NW - amount*SE.
            // It is not an unsharp mask or a Laplacian convolution.
            pixel[c] = byte(
                f32::from(pixel[c]) + amount * f32::from(before[c]) - amount * f32::from(after[c]),
            );
        }
    }
}

pub(crate) fn blur(pixels: &mut RgbaImage, spacing: f32, input: (u32, u32)) {
    if pixels.width() == 0 || pixels.height() == 0 {
        return;
    }
    // Shader coefficients and evaluation order, not a normalized Gaussian.
    // Each pass writes an 8-bit target, including the filtered alpha channel.
    const TAPS: [(f32, f32); 13] = [
        (-5.0, 0.008764),
        (-6.0, 0.002216),
        (-4.0, 0.026995),
        (-3.0, 0.064759),
        (-2.0, 0.120985),
        (-1.0, 0.176033),
        (0.0, 0.199471),
        (1.0, 0.176033),
        (2.0, 0.120985),
        (3.0, 0.064759),
        (4.0, 0.026995),
        (5.0, 0.008764),
        (6.0, 0.002216),
    ];
    let dx = spacing * 1.0 / input.0 as f32;
    let dy = spacing * 1.0 / input.1 as f32;
    let mut scratch = pixels.clone();
    for horizontal in [true, false] {
        for (x, y, output) in scratch.enumerate_pixels_mut() {
            let mut sum = [0.0; 4];
            for (offset, weight) in TAPS {
                let (sx, sy) = if horizontal {
                    (sample_coordinate(x, offset, dx, pixels.width()), y)
                } else {
                    (x, sample_coordinate(y, offset, dy, pixels.height()))
                };
                let sample = pixels.get_pixel(sx, sy);
                for c in 0..4 {
                    sum[c] += f32::from(sample[c]) * weight;
                }
            }
            output.0 = sum.map(byte);
        }
        std::mem::swap(pixels, &mut scratch);
    }
}

fn sample_coordinate(origin: u32, offset: f32, step: f32, size: u32) -> u32 {
    // Keep normalized UV arithmetic: simplifying it to pixel-space offsets
    // changes half-texel decisions after f32 rounding (notably blur 2.5).
    let uv = offset.mul_add(step, origin as f32 / size as f32);
    (uv * size as f32)
        .round_ties_even()
        .clamp(0.0, (size - 1) as f32) as u32
}
