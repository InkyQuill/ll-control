//! GA2LCD VideoEdit CPU effects: contrast, then lightness, with a byte boundary
//! between effects. Formula/rounding verified against installed MFP64 exports.
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VideoAdjustments {
    pub contrast: u8,
    pub brightness: u8,
}
impl VideoAdjustments {
    /// Windows SIMD path uses separate f32 operations and nearest-even byte
    /// quantization. Do not fuse multiply/add or reuse the image-editor filters.
    pub fn channel_lut(self) -> [u8; 256] {
        let contrast = (f32::from(self.contrast) + 255.0) * 259.0
            / ((259.0 - f32::from(self.contrast)) * 255.0);
        let lightness = f32::from(self.brightness) / 255.0;
        std::array::from_fn(|value| {
            let contrasted = (((value as f32 - 128.0) * contrast) + 128.0)
                .round_ties_even()
                .clamp(0.0, 255.0);
            (contrasted + (255.0 - contrasted) * lightness)
                .round_ties_even()
                .clamp(0.0, 255.0) as u8
        })
    }
    /// lutrgb evaluates these expressions once to build its byte lookup table.
    /// A balanced tree avoids expression parser depth limits; no per-pixel tree
    /// traversal or dependence on FFmpeg's floating-point effect approximations.
    pub fn filter(self) -> String {
        fn branch(values: &[u8], offset: usize) -> String {
            if values.len() == 1 {
                return values[0].to_string();
            }
            let middle = values.len() / 2;
            format!(
                "if(lt(val,{}),{},{})",
                offset + middle,
                branch(&values[..middle], offset),
                branch(&values[middle..], offset + middle)
            )
        }
        let expression = branch(&self.channel_lut(), 0);
        format!("format=rgb24,lutrgb=r='{expression}':g='{expression}':b='{expression}'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    #[test]
    fn all_channel_values_match_emulated_native_simd_exports() {
        let reference = include_bytes!("../tests/fixtures/video-effect-channels.bin");
        assert_eq!(reference.len(), 2 * 256 * 256);
        for amount in 0..=255u8 {
            let contrast = VideoAdjustments {
                contrast: amount,
                brightness: 0,
            }
            .channel_lut();
            let brightness = VideoAdjustments {
                contrast: 0,
                brightness: amount,
            }
            .channel_lut();
            let offset = usize::from(amount) * 256;
            assert_eq!(
                &contrast,
                &reference[offset..offset + 256],
                "contrast {amount}"
            );
            assert_eq!(
                &brightness,
                &reference[65536 + offset..65536 + offset + 256],
                "brightness {amount}"
            );
        }
        assert!(serde_json::from_str::<VideoAdjustments>(r#"{"contrast":256}"#).is_err());
        assert!(serde_json::from_str::<VideoAdjustments>(r#"{"brightness":-1}"#).is_err());
        for invalid in [
            r#"{"contrast":1.5}"#,
            r#"{"brightness":"10"}"#,
            r#"{"gain":20}"#,
        ] {
            assert!(serde_json::from_str::<VideoAdjustments>(invalid).is_err());
        }
        let legacy: VideoAdjustments = serde_json::from_str("{}").unwrap();
        assert_eq!((legacy.contrast, legacy.brightness), (0, 0));
    }
    #[test]
    fn ffmpeg_lookup_preserves_exact_quantized_channel_results() {
        let input: Vec<u8> = (0..=255).flat_map(|value| [value; 3]).collect();
        for (contrast, brightness) in [(0, 0), (1, 1), (32, 128), (128, 32), (255, 255)] {
            let effect = VideoAdjustments {
                contrast,
                brightness,
            };
            let mut child = Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-threads",
                    "1",
                    "-filter_threads",
                    "1",
                    "-f",
                    "rawvideo",
                    "-pixel_format",
                    "rgb24",
                    "-video_size",
                    "256x1",
                    "-i",
                    "pipe:0",
                    "-vf",
                    &effect.filter(),
                    "-frames:v",
                    "1",
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&input).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let lut = effect.channel_lut();
            let expected: Vec<u8> = input.iter().map(|value| lut[usize::from(*value)]).collect();
            assert_eq!(output.stdout, expected);
        }
    }
}
