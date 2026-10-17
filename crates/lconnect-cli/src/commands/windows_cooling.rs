//! Local policy conversion only. Full-group storage and publication are separate.
use anyhow::ensure;
use serde_json::json;
use std::path::PathBuf;

pub fn run(
    file: PathBuf,
    record: usize,
    sensor: &str,
    hysteresis: f64,
    fallback: u8,
) -> anyhow::Result<()> {
    ensure!(
        !sensor.trim().is_empty() && !sensor.chars().any(char::is_control),
        "CPU sensor binding must be explicit and contain no control characters"
    );
    // Check preparation options even if every profile in a future bank is fixed.
    ensure!(
        sensor.len() <= 4096
            && hysteresis.is_finite()
            && (0.0..=10.0).contains(&hysteresis)
            && (20..=100).contains(&fallback),
        "invalid cooling binding, hysteresis or fallback (20..=100)"
    );
    let prepared =
        lconnect_profile::cooling::inspect_windows_cooling(std::fs::File::open(file)?, record)?;
    let normalized =
        lconnect_profile::linux_cooling::prepare(&prepared, sensor, hysteresis, fallback)?;
    let report = json!({
        "policy_conversion_only": true,
        "group_import_supported": false,
        "apply_supported": false,
        "sensor_binding_verified": false,
        "source_presentation_metadata_validated": false,
        "cpu_sensor": sensor,
        "archive": prepared.archive,
        "record_index": prepared.record_index,
        "unconverted_controller_fields": prepared.unconverted_controller_fields,
        "fan": normalized.fan,
        "pump": normalized.pump,
        "cooling_banks": normalized.cooling_banks,
        "cooling_config": normalized.cooling_config,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
