//! Point colour operations derived from the GA2LCD editor's shader contract.
//! See docs/development/parity/ga2lcd-editor.md; this is not WPF raster qualification.
use image::RgbaImage;

/// Windows image-editor UI values; each starts at zero.
#[derive(Debug, Clone, Copy, Default)]
pub struct ColorFilters {
    /// Hue shift in degrees, 0..=360.
    pub hue: f32,
    /// Saturation offset, -25..=75 (factor = offset/25 + 1).
    pub saturation: f32,
    /// Brightness offset, -100..=100.
    pub brightness: f32,
    /// Contrast offset, -100..=100 (factor = offset/100 + 1).
    pub contrast: f32,
    /// Diagonal detail amount, 0..=10 (shader amount = value/5).
    pub sharpen: f32,
    /// Two-pass blur amount, 0..=10 (sample spacing = value/5).
    pub blur: f32,
}
impl ColorFilters {
    /// Reject non-finite and out-of-range controls instead of silently clamping.
    pub fn validate(&self) -> crate::Result<()> {
        for (value, min, max) in [
            (self.hue, 0.0, 360.0),
            (self.saturation, -25.0, 75.0),
            (self.brightness, -100.0, 100.0),
            (self.contrast, -100.0, 100.0),
            (self.sharpen, 0.0, 10.0),
            (self.blur, 0.0, 10.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(crate::Error::InvalidEdit(
                    "colour filter is outside its range",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn is_neutral(self) -> bool {
        (self.hue == 0.0 || self.hue == 360.0)
            && self.saturation == 0.0
            && self.brightness == 0.0
            && self.contrast == 0.0
            && self.sharpen == 0.0
            && self.blur == 0.0
    }

    /// Apply hue → saturation → sharpen → brightness/contrast → blur. Shader stages operate on
    /// premultiplied channels and render to an 8-bit target between operations.
    /// Blur filters alpha as well; other stages preserve it. Final transparent
    /// pixels are canonicalized to zero RGB.
    pub fn apply(self, pixels: &mut RgbaImage) -> crate::Result<()> {
        self.apply_with_input_size(pixels, pixels.dimensions())
    }

    pub(crate) fn apply_with_input_size(
        self,
        pixels: &mut RgbaImage,
        input: (u32, u32),
    ) -> crate::Result<()> {
        self.validate()?;
        if input.0 == 0 || input.1 == 0 {
            return Err(crate::Error::InvalidEdit("empty filter input"));
        }
        if self.is_neutral() {
            return Ok(());
        }
        for pixel in pixels.pixels_mut() {
            let alpha = f32::from(pixel[3]) / 255.0;
            if alpha == 0.0 {
                pixel.0 = [0; 4];
                continue;
            }
            let mut rgb =
                [pixel[0], pixel[1], pixel[2]].map(|v| quantize(f32::from(v) / 255.0 * alpha));
            if self.hue != 0.0 && self.hue != 360.0 {
                rgb = shift_hue(rgb, self.hue).map(quantize);
            }
            if self.saturation != 0.0 {
                let luma = rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114;
                let factor = self.saturation / 25.0 + 1.0;
                rgb = rgb.map(|c| quantize(factor * c + (1.0 - factor) * luma));
            }
            for (channel, value) in pixel.0[..3].iter_mut().zip(rgb) {
                *channel = (value * 255.0).round() as u8;
            }
        }
        if self.sharpen != 0.0 {
            crate::editor_spatial::sharpen(pixels, self.sharpen / 5.0, input);
        }
        // Windows runs this stage even with neutral controls. After sharpen,
        // zero-alpha pixels can contain RGB from neighbours; clear those before
        // blur can spread them into visible pixels.
        if self.brightness != 0.0 || self.contrast != 0.0 || self.sharpen != 0.0 {
            let contrast = self.contrast / 100.0 + 1.0;
            let brightness = self.brightness / 100.0;
            for pixel in pixels.pixels_mut() {
                let alpha = f32::from(pixel[3]) / 255.0;
                if alpha == 0.0 {
                    pixel.0 = [0; 4];
                    continue;
                }
                for c in &mut pixel.0[..3] {
                    *c = (quantize(
                        ((f32::from(*c) / 255.0 / alpha - 0.5) * contrast + brightness + 0.5)
                            * alpha,
                    ) * 255.0)
                        .round() as u8;
                }
            }
        }
        if self.blur != 0.0 {
            crate::editor_spatial::blur(pixels, self.blur / 5.0, input);
        }
        for pixel in pixels.pixels_mut() {
            let alpha = f32::from(pixel[3]);
            if alpha == 0.0 {
                pixel.0 = [0; 4];
            } else {
                for c in &mut pixel.0[..3] {
                    *c = (f32::from(*c) / alpha * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        Ok(())
    }
}
fn quantize(value: f32) -> f32 {
    (value.clamp(0.0, 1.0) * 255.0).round() / 255.0
}
fn shift_hue(rgb: [f32; 3], degrees: f32) -> [f32; 3] {
    let [red, green, blue] = rgb;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    if delta == 0.0 {
        return rgb;
    }
    let base = if max == red {
        (green - blue) * delta.recip()
    } else if max == green {
        (blue - red) * delta.recip() + 2.0
    } else {
        (red - green) * delta.recip() + 4.0
    };
    // The shader multiplies by its f32 reciprocal-of-60 constant. Division
    // rounds differently at half-byte boundaries, amplified by saturation.
    let hue = (base + degrees * (1.0_f32 / 60.0)).rem_euclid(6.0);
    let fraction = hue.fract();
    let saturation = delta * max.recip();
    let down = max * (1.0 - saturation * fraction);
    let up = max * (1.0 - saturation * (1.0 - fraction));
    let min = max * (1.0 - saturation);
    match hue.floor() as u8 {
        0 => [max, up, min],
        1 => [down, max, min],
        2 => [min, max, up],
        3 => [min, down, max],
        4 => [up, min, max],
        _ => [max, min, down],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_stages_retain_original_dimensions_after_rotation() {
        let source = [
            [44, 120, 180, 84],
            [37, 100, 180, 74],
            [30, 80, 180, 64],
            [74, 120, 160, 109],
            [67, 100, 160, 99],
            [60, 80, 160, 89],
            [104, 120, 140, 134],
            [97, 100, 140, 124],
            [90, 80, 140, 114],
            [134, 120, 120, 159],
            [127, 100, 120, 149],
            [120, 80, 120, 139],
            [164, 120, 100, 184],
            [157, 100, 100, 174],
            [150, 80, 100, 164],
        ];
        let expected = [
            [43, 3, 255, 96],
            [62, 3, 255, 87],
            [74, 0, 255, 79],
            [44, 11, 234, 111],
            [60, 13, 255, 102],
            [74, 8, 239, 93],
            [63, 31, 205, 133],
            [78, 31, 220, 124],
            [84, 16, 202, 115],
            [81, 70, 171, 154],
            [100, 74, 186, 145],
            [103, 51, 167, 136],
            [100, 95, 151, 169],
            [118, 100, 164, 160],
            [118, 68, 142, 151],
        ];
        let mut pixels = RgbaImage::from_fn(3, 5, |x, y| image::Rgba(source[(y * 3 + x) as usize]));
        ColorFilters {
            hue: 60.,
            saturation: 10.,
            sharpen: 4.,
            brightness: 10.,
            contrast: 20.,
            blur: 2.,
        }
        .apply_with_input_size(&mut pixels, (5, 3))
        .unwrap();
        for (pixel, expected) in pixels.pixels().zip(expected) {
            for (actual, wanted) in pixel.0.into_iter().zip(expected) {
                assert!(actual.abs_diff(wanted) <= 1, "{pixel:?} != {expected:?}");
            }
        }
    }
}
