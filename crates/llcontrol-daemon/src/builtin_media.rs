//! Original, immutable video background. No runtime FFmpeg or bootstrap writes.
use llcontrol_ipc::{H264AccessUnit, PreparedClipInfo};

/// Reserved portable key; never reuse it for different encoded content.
pub const KEY: &str = llcontrol_ipc::BUILTIN_VIDEO_KEY;
pub const FPS: u8 = 24;
static BYTES: &[u8] = include_bytes!("../assets/defaults/aurora-v1.h264");
static UNITS: &[(usize, usize, bool)] = include!("../assets/defaults/aurora-v1-index.rs");

pub fn info() -> PreparedClipInfo {
    PreparedClipInfo {
        key: KEY.to_owned(),
        name: "Сияние".to_owned(),
        fps: FPS,
        frames: UNITS.len() as u32,
        encoded_bytes: BYTES.len() as u64,
        is_default: true,
    }
}

pub fn frames() -> usize {
    UNITS.len()
}

/// Allocate just one bounded access unit, not a copy of the whole video.
pub fn read_unit(frame: usize) -> anyhow::Result<H264AccessUnit> {
    let &(start, end, keyframe) = UNITS
        .get(frame)
        .ok_or_else(|| anyhow::anyhow!("builtin video frame outside index"))?;
    let bytes = BYTES
        .get(start..end)
        .ok_or_else(|| anyhow::anyhow!("invalid builtin video index"))?;
    let unit = H264AccessUnit {
        bytes: bytes.to_vec(),
        keyframe,
    };
    llcontrol_ipc::validate_h264_access_unit(&unit, frame == 0)?;
    Ok(unit)
}

pub(crate) fn keyframe_before(frame: usize) -> anyhow::Result<usize> {
    UNITS
        .get(..=frame)
        .ok_or_else(|| anyhow::anyhow!("builtin frame outside index"))?
        .iter()
        .rposition(|unit| unit.2)
        .ok_or_else(|| anyhow::anyhow!("builtin has no preceding keyframe"))
}
