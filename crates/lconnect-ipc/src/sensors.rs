//! Host sensor catalog shared by cooling and display consumers.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorUnit {
    Celsius,
    Percent,
    Bytes,
    Rpm,
    Watts,
    Volts,
    Hertz,
    BytesPerSecond,
    FramesPerSecond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorStatus {
    Valid,
    Unavailable,
    Invalid,
    Stale,
}

/// Only Valid readings have usable values. IDs describe a host topology,
/// not a portable identity across hardware replacement or bus relocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorReading {
    pub id: String,
    pub label: String,
    pub unit: SensorUnit,
    pub value: Option<f64>,
    pub status: SensorStatus,
    pub observed_unix_ms: Option<u64>,
    pub age_ms: Option<u64>,
}
