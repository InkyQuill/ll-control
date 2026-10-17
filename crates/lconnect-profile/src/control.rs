//! A complete normalized FanAndLighting draft, independent of its eventual target.
use crate::{
    cooling::inspect_windows_cooling,
    lighting::inspect_windows_lighting,
    linux_cooling,
    linux_lighting::{self, PreparedLighting, SensorBindings},
    Error, LCDV2_TYPE_ID,
};
use lconnect_ipc::{CoolingConfig, CoolingModeBanks};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedControl {
    pub version: u8,
    pub source_backup_sha256: String,
    pub source_profile_sha256: String,
    pub record_index: usize,
    pub cooling: CoolingConfig,
    pub cooling_banks: CoolingModeBanks,
    pub lighting: PreparedLighting,
    /// Typed source report without Windows hardware Metadata. Historical only.
    pub lighting_source_json: String,
}
/// Read a single consistent ZIP/record twice through the existing bounded codecs.
/// Every applicable controller field must be present; Screen data stays unconverted.
pub fn prepare<R: Read + Seek>(
    mut reader: R,
    record: usize,
    bindings: &SensorBindings,
    hysteresis: f64,
    fallback: u8,
) -> Result<PreparedControl, Error> {
    let cooling = inspect_windows_cooling(&mut reader, record)?;
    reader.seek(SeekFrom::Start(0))?;
    let lighting = inspect_windows_lighting(&mut reader, record)?;
    if cooling.archive.backup_sha256 != lighting.archive.backup_sha256 {
        return Err(Error::Invalid("Windows archive changed during preparation"));
    }
    let cpu = bindings
        .get("CPUTemperature")
        .ok_or(Error::Invalid("explicit CPU temperature binding required"))?;
    let normalized_cooling =
        linux_cooling::prepare(&cooling, &cpu.sensor_id, hysteresis, fallback)?;
    let normalized_lighting = linux_lighting::prepare(&lighting, bindings)?;
    let fingerprint = cooling
        .archive
        .controllers
        .iter()
        .find(|c| c.record_index == record && c.controller_type == LCDV2_TYPE_ID)
        .ok_or(Error::Invalid("missing selected source record"))?
        .profile_sha256
        .clone();
    Ok(PreparedControl {
        version: 1,
        source_backup_sha256: cooling.archive.backup_sha256,
        source_profile_sha256: fingerprint,
        record_index: record,
        cooling: normalized_cooling.cooling_config,
        cooling_banks: normalized_cooling.cooling_banks,
        lighting: normalized_lighting,
        lighting_source_json: serde_json::to_string(&lighting)?,
    })
}
/// Bounded, strict payload reader for later IPC admission. Semantic validation is
/// still performed by the target group builder before any persistent mutation.
pub fn parse(bytes: &[u8]) -> Result<PreparedControl, Error> {
    if bytes.len() > 256 * 1024 {
        return Err(Error::Invalid("Windows control draft exceeds256KiB"));
    }
    Ok(serde_json::from_value(crate::strict_json::parse(bytes)?)?)
}
