//! Diagnostic H.264 clip types and validation for M1 LCD playback.

use crate::{IpcError, Result};
use serde::{Deserialize, Serialize};

const MAX_ACCESS_UNITS: usize = 240;
const MAX_ACCESS_UNIT_BYTES: usize = 128 * 1024;
const MAX_CLIP_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct H264AccessUnit {
    pub bytes: Vec<u8>,
    pub keyframe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct H264Clip {
    pub fps: u8,
    pub units: Vec<H264AccessUnit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdPlaybackStats {
    pub generation: u64,
    pub sent_frames: u64,
    pub sent_bytes: u64,
    pub read_timeouts: u64,
    pub failed_transactions: u64,
    pub pending_frames: u32,
    pub elapsed_ms: u64,
}

/// Validate the deliberately narrow diagnostic clip format accepted by M1.
///
/// This only recognizes Annex-B boundaries and NAL unit types needed to
/// establish a safe diagnostic stream. It is not a general H.264 decoder.
pub fn validate_h264_clip(clip: &H264Clip, loops: u16) -> Result<()> {
    if clip.fps != 24 {
        return invalid(format!("M1 clips require 24 fps, got {}", clip.fps));
    }
    if !(1..=900).contains(&loops) {
        return invalid(format!("clip loops must be in 1..=900, got {loops}"));
    }
    if !(1..=MAX_ACCESS_UNITS).contains(&clip.units.len()) {
        return invalid(format!(
            "clip must contain 1..={MAX_ACCESS_UNITS} access units"
        ));
    }

    let mut total = 0usize;
    for (index, unit) in clip.units.iter().enumerate() {
        if unit.bytes.is_empty() || unit.bytes.len() > MAX_ACCESS_UNIT_BYTES {
            return invalid(format!(
                "access unit {index} must contain 1..={MAX_ACCESS_UNIT_BYTES} bytes"
            ));
        }
        total = total
            .checked_add(unit.bytes.len())
            .ok_or_else(|| IpcError::InvalidArgument("clip byte size overflow".into()))?;
        if total > MAX_CLIP_BYTES {
            return invalid(format!("clip exceeds {MAX_CLIP_BYTES} bytes"));
        }

        validate_h264_access_unit(unit, index == 0).map_err(|reason| {
            IpcError::InvalidArgument(format!("access unit {index}: {reason}"))
        })?;
    }
    Ok(())
}

/// Validate one bounded Annex-B AU, independently of diagnostic clip duration.
/// This recognizes framing/IDR/SPS/PPS markers, not full decoder conformance.
pub fn validate_h264_access_unit(unit: &H264AccessUnit, first: bool) -> Result<()> {
    if unit.bytes.is_empty() || unit.bytes.len() > MAX_ACCESS_UNIT_BYTES {
        return invalid(format!(
            "access unit must contain 1..={MAX_ACCESS_UNIT_BYTES} bytes"
        ));
    }
    let types = annex_b_nal_types(&unit.bytes)
        .map_err(|reason| IpcError::InvalidArgument(reason.into()))?;
    if unit.keyframe != types.contains(&5) {
        return invalid("keyframe marker does not match IDR NAL");
    }
    if first && (!unit.keyframe || !types.contains(&7) || !types.contains(&8)) {
        return invalid("first access unit must be a keyframe containing SPS, PPS, and IDR NALs");
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(IpcError::InvalidArgument(message.into()))
}

fn annex_b_nal_types(bytes: &[u8]) -> std::result::Result<Vec<u8>, &'static str> {
    let mut offset = 0;
    let mut types = Vec::new();
    while offset < bytes.len() {
        let start_len = if bytes[offset..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if bytes[offset..].starts_with(&[0, 0, 1]) {
            3
        } else {
            return Err("expected Annex-B start code");
        };
        let header = offset + start_len;
        if header >= bytes.len() {
            return Err("truncated Annex-B start code");
        }
        types.push(bytes[header] & 0x1f);

        let mut next = header + 1;
        while next < bytes.len()
            && !bytes[next..].starts_with(&[0, 0, 1])
            && !bytes[next..].starts_with(&[0, 0, 0, 1])
        {
            next += 1;
        }
        if next == bytes.len() {
            break;
        }
        offset = next;
    }
    if types.is_empty() {
        Err("access unit contains no NAL units")
    } else {
        Ok(types)
    }
}

/// Metadata for a daemon-managed prepared clip; key is portable, never a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedClipInfo {
    /// Immutable bundled video; omitted by older servers.
    #[serde(default)]
    pub is_default: bool,
    pub key: String,
    pub name: String,
    pub fps: u8,
    pub frames: u32,
    pub encoded_bytes: u64,
}
