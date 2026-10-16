//! Clockwise rotation of the complete composed frame, without resampling.
use image::RgbImage;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FrameRotation {
    #[default]
    None,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}
impl TryFrom<u16> for FrameRotation {
    type Error = crate::Error;
    fn try_from(degrees: u16) -> crate::Result<Self> {
        match degrees {
            0 => Ok(Self::None),
            90 => Ok(Self::Clockwise90),
            180 => Ok(Self::Clockwise180),
            270 => Ok(Self::Clockwise270),
            _ => Err(crate::Error::InvalidRotation(degrees)),
        }
    }
}
impl FrameRotation {
    pub fn degrees(self) -> u16 {
        match self {
            Self::None => 0,
            Self::Clockwise90 => 90,
            Self::Clockwise180 => 180,
            Self::Clockwise270 => 270,
        }
    }
    pub fn apply(self, frame: RgbImage) -> RgbImage {
        match self {
            Self::None => frame,
            Self::Clockwise90 => image::imageops::rotate90(&frame),
            Self::Clockwise180 => image::imageops::rotate180(&frame),
            Self::Clockwise270 => image::imageops::rotate270(&frame),
        }
    }
}
