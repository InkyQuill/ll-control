//! Intel i915/xe read-only GT clock telemetry, independent of PMU permissions.
//! Source paths and semantics: docs/development/parity/sensor-provider-inventory.md.
use super::{number, reading};
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const MAX_ENTRIES: usize = 256;
const MAX_DEVICES: usize = 32;
const MAX_ROWS: usize = 512;

pub(super) struct IntelClocks {
    root: PathBuf,
}
impl IntelClocks {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub(super) fn collect(&self, now: u64) -> Vec<SensorReading> {
        self.collect_inner(now).unwrap_or_else(|status| {
            vec![reading(
                "intel-clocks:discovery",
                "Intel GPU clock discovery",
                SensorUnit::Hertz,
                Err(status),
                now,
            )]
        })
    }
    fn collect_inner(&self, now: u64) -> Result<Vec<SensorReading>, SensorStatus> {
        let mut seen = BTreeSet::new();
        let mut rows = Vec::new();
        for card in children(&self.root, "card")? {
            let device =
                fs::canonicalize(card.join("device")).map_err(|_| SensorStatus::Unavailable)?;
            let Ok(driver_path) = fs::canonicalize(device.join("driver")) else {
                continue;
            };
            let Some(driver) = driver_path
                .file_name()
                .and_then(|s| s.to_str())
                .filter(|s| matches!(*s, "i915" | "xe"))
            else {
                continue;
            };
            if !seen.insert(device.clone()) || seen.len() > MAX_DEVICES {
                return Err(SensorStatus::Invalid);
            }
            use std::os::unix::ffi::OsStrExt;
            let physical = device
                .as_os_str()
                .as_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let prefix = format!("intel-clocks:{physical}:{driver}");
            if driver == "i915" {
                match fs::symlink_metadata(card.join("gt")) {
                    Ok(_) => {
                        for gt in children(&card.join("gt"), "gt")? {
                            let domain = gt.file_name().unwrap().to_string_lossy();
                            append(
                                &mut rows,
                                &gt,
                                &format!("{prefix}:{domain}"),
                                &format!("i915 {domain}"),
                                ["rps_act_freq_mhz", "rps_cur_freq_mhz"],
                                now,
                            )?;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // Older card-wide ABI has different identity/semantics.
                        append(
                            &mut rows,
                            &card,
                            &format!("{prefix}:legacy"),
                            "i915 legacy card",
                            ["gt_act_freq_mhz", "gt_cur_freq_mhz"],
                            now,
                        )?;
                    }
                    Err(_) => return Err(SensorStatus::Unavailable),
                }
            } else {
                for tile in children(&device, "tile")? {
                    for gt in children(&tile, "gt")? {
                        let domain = format!(
                            "{}:{}",
                            tile.file_name().unwrap().to_string_lossy(),
                            gt.file_name().unwrap().to_string_lossy()
                        );
                        append(
                            &mut rows,
                            &gt.join("freq0"),
                            &format!("{prefix}:{domain}"),
                            &format!("xe {domain}"),
                            ["act_freq", "cur_freq"],
                            now,
                        )?;
                    }
                }
            }
        }
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(rows)
    }
}

fn children(root: &Path, prefix: &str) -> Result<Vec<PathBuf>, SensorStatus> {
    let entries = fs::read_dir(root).map_err(|_| SensorStatus::Unavailable)?;
    let mut paths = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index == MAX_ENTRIES {
            return Err(SensorStatus::Invalid);
        }
        let entry = entry.map_err(|_| SensorStatus::Unavailable)?;
        let name = entry.file_name();
        let Some(suffix) = name.to_str().and_then(|s| s.strip_prefix(prefix)) else {
            continue;
        };
        // Ignore connectors/render nodes and noncanonical/ambiguous indices.
        if suffix
            .parse::<u16>()
            .ok()
            .is_some_and(|n| n.to_string() == suffix)
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn append(
    rows: &mut Vec<SensorReading>,
    path: &Path,
    prefix: &str,
    label: &str,
    files: [&str; 2],
    now: u64,
) -> Result<(), SensorStatus> {
    if rows.len() + 2 > MAX_ROWS {
        return Err(SensorStatus::Invalid);
    }
    for (file, metric, description) in [
        (files[0], "actual_frequency", "actual frequency"),
        (files[1], "requested_frequency", "requested frequency"),
    ] {
        let value = number(&path.join(file)).and_then(|mhz| {
            // Both kernel ABIs emit unsigned 32-bit MHz; zero can indicate RC6.
            u32::try_from(mhz)
                .map(|mhz| f64::from(mhz) * 1_000_000.0)
                .map_err(|_| SensorStatus::Invalid)
        });
        rows.push(reading(
            format!("{prefix}:{metric}"),
            format!("{label}: {description}"),
            SensorUnit::Hertz,
            value,
            now,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;
    fn device(dir: &Path, card: &str, driver: &str) -> PathBuf {
        let physical = dir.join("devices").join(card);
        fs::create_dir_all(&physical).unwrap();
        fs::create_dir_all(dir.join("drivers").join(driver)).unwrap();
        symlink(dir.join("drivers").join(driver), physical.join("driver")).unwrap();
        fs::create_dir_all(dir.join("drm").join(card)).unwrap();
        symlink(&physical, dir.join("drm").join(card).join("device")).unwrap();
        physical
    }
    fn clock(path: &Path, files: [&str; 2]) {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join(files[0]), "1200").unwrap();
        fs::write(path.join(files[1]), "1400").unwrap();
    }
    #[test]
    fn both_drivers_multiple_domains_units_and_card_renumber() {
        let dir = TempDir::new().unwrap();
        device(dir.path(), "card0", "i915");
        let xe = device(dir.path(), "card1", "xe");
        device(dir.path(), "card2", "amdgpu");
        for gt in ["gt0", "gt1"] {
            clock(
                &dir.path().join("drm/card0/gt").join(gt),
                ["rps_act_freq_mhz", "rps_cur_freq_mhz"],
            );
        }
        for tile in ["tile0", "tile1"] {
            clock(&xe.join(tile).join("gt0/freq0"), ["act_freq", "cur_freq"]);
        }
        let provider = IntelClocks::new(dir.path().join("drm"));
        let rows = provider.collect(1);
        assert_eq!(rows.len(), 8);
        let ids = rows.iter().map(|r| &r.id).collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), 8);
        for row in &rows {
            assert_eq!(row.unit, SensorUnit::Hertz);
            let expected = if row.id.ends_with(":actual_frequency") {
                "1200.0 MHz"
            } else {
                "1400.0 MHz"
            };
            assert_eq!(
                lconnect_ipc::theme_sensor_text(
                    &rows,
                    &row.id,
                    SensorUnit::Hertz,
                    lconnect_ipc::TemperatureUnit::Fahrenheit,
                    1
                ),
                expected
            );
        }
        fs::rename(dir.path().join("drm/card0"), dir.path().join("drm/card9")).unwrap();
        assert_eq!(provider.collect(1), rows);
        fs::remove_dir_all(dir.path().join("drm/card9/gt/gt1")).unwrap();
        let remaining = provider.collect(2);
        assert_eq!(remaining.len(), 6);
        assert!(!remaining.iter().any(|r| r.id.contains(":i915:gt1:")));
    }
    #[test]
    fn errors_zero_and_missing_actual_never_substitute_requested() {
        let dir = TempDir::new().unwrap();
        let dev = device(dir.path(), "card0", "xe");
        let path = dev.join("tile0/gt0/freq0");
        clock(&path, ["act_freq", "cur_freq"]);
        let provider = IntelClocks::new(dir.path().join("drm"));
        for (value, status) in [
            ("-1", SensorStatus::Invalid),
            ("NaN", SensorStatus::Invalid),
            ("4294967296", SensorStatus::Invalid),
            ("0", SensorStatus::Valid),
        ] {
            fs::write(path.join("act_freq"), value).unwrap();
            let rows = provider.collect(1);
            assert_eq!(rows[0].status, status);
            assert_eq!(
                rows[0].value,
                if status == SensorStatus::Valid {
                    Some(0.0)
                } else {
                    None
                }
            );
            assert_eq!(rows[1].value, Some(1_400_000_000.0));
        }
        fs::remove_file(path.join("act_freq")).unwrap();
        assert_eq!(provider.collect(2)[0].status, SensorStatus::Unavailable);
        fs::write(path.join("act_freq"), "1000").unwrap();
        assert_eq!(provider.collect(3)[0].value, Some(1_000_000_000.0));
    }
    #[test]
    fn legacy_and_modern_are_not_silently_interchanged() {
        let dir = TempDir::new().unwrap();
        device(dir.path(), "card0", "i915");
        let card = dir.path().join("drm/card0");
        clock(&card, ["gt_act_freq_mhz", "gt_cur_freq_mhz"]);
        let provider = IntelClocks::new(dir.path().join("drm"));
        let legacy = provider.collect(1);
        assert!(legacy.iter().all(|r| r.id.contains(":legacy:")));
        fs::create_dir_all(card.join("gt/gt0")).unwrap();
        let modern = provider.collect(2);
        assert!(modern
            .iter()
            .all(|r| r.id.contains(":gt0:") && r.status == SensorStatus::Unavailable));
        assert!(!modern
            .iter()
            .any(|r| legacy.iter().any(|old| old.id == r.id)));
    }
    #[test]
    fn duplicate_devices_and_catalog_overflow_fail_explicitly() {
        let dir = TempDir::new().unwrap();
        device(dir.path(), "card0", "i915");
        let provider = IntelClocks::new(dir.path().join("drm"));
        symlink(dir.path().join("drm/card0"), dir.path().join("drm/card1")).unwrap();
        assert_eq!(provider.collect(1)[0].status, SensorStatus::Invalid);
        fs::remove_file(dir.path().join("drm/card1")).unwrap();
        for n in 0..MAX_ENTRIES {
            fs::write(dir.path().join(format!("drm/unrelated{n}")), "").unwrap();
        }
        assert_eq!(provider.collect(2)[0].status, SensorStatus::Invalid);
        assert_eq!(
            IntelClocks::new(dir.path().join("missing")).collect(3)[0].status,
            SensorStatus::Unavailable
        );
    }
}
