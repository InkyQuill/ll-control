//! GA2LCD VideoEdit's continuous single range. VideoCut's 180s recording limit
//! and millisecond DTO are separate contracts.
use crate::media_prepare::SourceMetadata;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoEditRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEditBounds {
    pub maximum_seconds: f64,
    pub minimum_span_seconds: f64,
}
impl VideoEditBounds {
    /// Prefer the demuxer's average rate; preserve the raw rates in metadata so
    /// this Linux choice is not mistaken for proof of Windows SDK/VFR equivalence.
    pub fn from_metadata(metadata: &SourceMetadata) -> Result<Self> {
        let duration = metadata
            .duration_seconds
            .context("VideoEdit requires a known source duration")?;
        let rate = metadata
            .average_frame_rate
            .or(metadata.nominal_frame_rate)
            .context("VideoEdit requires a known source frame rate")?;
        if !duration.is_finite()
            || duration <= 0.0
            || duration > 3600.0
            || rate.numerator == 0
            || rate.denominator == 0
        {
            bail!("Invalid VideoEdit source timing");
        }
        let tick = rate.frame_seconds();
        let bounds = Self {
            maximum_seconds: duration - tick,
            minimum_span_seconds: 2.0 * tick,
        };
        if bounds.maximum_seconds < bounds.minimum_span_seconds {
            bail!("Video is too short for the VideoEdit range");
        }
        Ok(bounds)
    }
}
impl VideoEditRange {
    pub fn validate(self) -> Result<()> {
        if !self.start_seconds.is_finite()
            || !self.end_seconds.is_finite()
            || self.start_seconds < 0.0
            || self.end_seconds <= self.start_seconds
            || self.end_seconds > 3600.0
        {
            bail!("VideoEdit range must have 0 <= start < end within one hour");
        }
        Ok(())
    }
    pub fn validate_source(self, metadata: &SourceMetadata) -> Result<()> {
        self.validate()?;
        let bounds = VideoEditBounds::from_metadata(metadata)?;
        // Only absorb double arithmetic noise, not a millisecond or frame.
        if self.end_seconds > bounds.maximum_seconds + 1e-9
            || self.end_seconds - self.start_seconds + 1e-9 < bounds.minimum_span_seconds
        {
            bail!(
                "VideoEdit range exceeds the last-frame limit or is shorter than two source frames"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_prepare::SourceFrameRate;
    fn metadata(numerator: u32, denominator: u32) -> SourceMetadata {
        SourceMetadata {
            width: 32,
            height: 32,
            duration_seconds: Some(10.0),
            average_frame_rate: Some(SourceFrameRate {
                numerator,
                denominator,
            }),
            nominal_frame_rate: None,
        }
    }
    #[test]
    fn continuous_range_uses_source_ticks_without_snapping_or_video_cut_limit() {
        for (n, d) in [(24, 1), (30000, 1001)] {
            let metadata = metadata(n, d);
            let bounds = VideoEditBounds::from_metadata(&metadata).unwrap();
            let tick = f64::from(d) / f64::from(n);
            assert_eq!(bounds.maximum_seconds, 10.0 - tick);
            VideoEditRange {
                start_seconds: 0.12345,
                end_seconds: 0.12345 + 2.0 * tick,
            }
            .validate_source(&metadata)
            .unwrap();
            assert!(VideoEditRange {
                start_seconds: 0.0,
                end_seconds: 2.0 * tick - 0.000001
            }
            .validate_source(&metadata)
            .is_err());
            assert!(VideoEditRange {
                start_seconds: 0.0,
                end_seconds: bounds.maximum_seconds + 0.000001
            }
            .validate_source(&metadata)
            .is_err());
        }
        let mut long = metadata(24, 1);
        long.duration_seconds = Some(300.0);
        VideoEditRange {
            start_seconds: 0.0,
            end_seconds: 250.123456,
        }
        .validate_source(&long)
        .unwrap();
    }
    #[test]
    fn unavailable_invalid_and_too_short_sources_do_not_get_fake_24fps() {
        let mut source = metadata(24, 1);
        source.average_frame_rate = None;
        assert!(VideoEditBounds::from_metadata(&source).is_err());
        source.nominal_frame_rate = Some(SourceFrameRate {
            numerator: 30,
            denominator: 1,
        });
        assert!(VideoEditBounds::from_metadata(&source).is_ok());
        for duration in [None, Some(f64::NAN), Some(-1.0), Some(0.05), Some(3601.0)] {
            source.duration_seconds = duration;
            assert!(VideoEditBounds::from_metadata(&source).is_err());
        }
        assert!(serde_json::from_str::<VideoEditRange>(
            r#"{"startSeconds":0,"endSeconds":1,"fps":24}"#
        )
        .is_err());
    }
}
