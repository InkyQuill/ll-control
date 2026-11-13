//! CPU package average power from RAPL energy deltas; never writes limits.
//! <https://docs.kernel.org/power/powercap/powercap.html>
use super::{reading, text};
use llcontrol_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

pub(super) struct PackagePower {
    root: PathBuf,
    previous: BTreeMap<String, (Instant, u64, u64)>,
}
impl PackagePower {
    pub(super) fn reset_baselines(&mut self) {
        self.previous.clear();
    }

    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            root,
            previous: BTreeMap::new(),
        }
    }
    pub(super) fn collect(&mut self, now: u64, at: Instant) -> Vec<SensorReading> {
        let mut samples = BTreeMap::new();
        let mut rows = BTreeMap::new();
        let failure = |status| {
            vec![reading(
                "powercap:discovery",
                "CPU package power unavailable",
                SensorUnit::Watts,
                Err(status),
                now,
            )]
        };
        let Ok(entries) = fs::read_dir(&self.root) else {
            self.previous.clear();
            return failure(SensorStatus::Unavailable);
        };
        for (index, entry) in entries.enumerate() {
            if index >= 256 {
                self.previous.clear();
                return failure(SensorStatus::Invalid);
            }
            let Ok(entry) = entry else {
                self.previous.clear();
                return failure(SensorStatus::Unavailable);
            };
            let filename = entry.file_name();
            let Some(zone) = filename
                .to_str()
                .and_then(|s| s.strip_prefix("intel-rapl:"))
            else {
                continue;
            };
            // Only whole packages, not cores/uncore/DRAM subzones or MMIO duplicates.
            if zone.parse::<u32>().is_err() {
                continue;
            }
            let path = entry.path();
            let Ok(name) = text(&path.join("name")) else {
                continue;
            };
            let name = name.trim();
            if name
                .strip_prefix("package-")
                .and_then(|s| s.parse::<u32>().ok())
                .is_none()
            {
                continue;
            }
            let id = format!("powercap:rapl:{name}:power");
            let scalar = |file: &str| {
                text(&path.join(file))?
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| SensorStatus::Invalid)
            };
            let value = scalar("energy_uj").and_then(|energy| {
                let range = scalar("max_energy_range_uj")?;
                if range == 0 || energy > range {
                    return Err(SensorStatus::Invalid);
                }
                samples.insert(id.clone(), (at, energy, range));
                let Some((previous_at, previous_energy, previous_range)) = self.previous.get(&id)
                else {
                    return Err(SensorStatus::Unavailable);
                };
                let dt = at.saturating_duration_since(*previous_at);
                // Reset and wrap cannot be distinguished from two reads. Skip this
                // interval instead of publishing a fabricated power spike.
                if dt.is_zero() || dt > Duration::from_secs(3) || range != *previous_range {
                    return Err(SensorStatus::Unavailable);
                }
                let delta = energy
                    .checked_sub(*previous_energy)
                    .ok_or(SensorStatus::Unavailable)?;
                Ok(delta as f64 / 1_000_000.0 / dt.as_secs_f64())
            });
            let row = reading(
                &id,
                format!("CPU {name} average power"),
                SensorUnit::Watts,
                value,
                now,
            );
            if rows.insert(id.clone(), row).is_some() {
                samples.remove(&id);
                rows.insert(
                    id.clone(),
                    reading(
                        &id,
                        "Ambiguous CPU package power",
                        SensorUnit::Watts,
                        Err(SensorStatus::Invalid),
                        now,
                    ),
                );
            }
            if rows.len() > 32 {
                self.previous.clear();
                return failure(SensorStatus::Invalid);
            }
        }
        self.previous = samples;
        rows.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_sampling_gaps_and_duplicate_packages_do_not_publish_power() {
        let root = tempfile::tempdir().unwrap();
        let zone = root.path().join("intel-rapl:0");
        fs::create_dir(&zone).unwrap();
        for (name, value) in [
            ("name", "package-0"),
            ("energy_uj", "10"),
            ("max_energy_range_uj", "1000000000"),
        ] {
            fs::write(zone.join(name), value).unwrap();
        }
        let mut p = PackagePower::new(root.path().into());
        let at = Instant::now();
        p.collect(1, at);
        fs::write(zone.join("energy_uj"), "100000000").unwrap();
        assert_eq!(p.collect(2, at + Duration::from_secs(10))[0].value, None);
        let alias = root.path().join("intel-rapl:1");
        std::os::unix::fs::symlink(&zone, &alias).unwrap();
        let rows = p.collect(3, at + Duration::from_secs(11));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, SensorStatus::Invalid);
        fs::remove_file(alias).unwrap();
        assert_eq!(p.collect(4, at + Duration::from_secs(12))[0].value, None);
        assert_eq!(
            p.collect(5, at + Duration::from_secs(13))[0].value,
            Some(0.0)
        );
    }

    #[test]
    fn energy_delta_watts_reset_fault_recovery_and_zone_renumbering() {
        let root = tempfile::tempdir().unwrap();
        let mut zone = root.path().join("intel-rapl:0");
        fs::create_dir(&zone).unwrap();
        fs::write(zone.join("name"), "package-0").unwrap();
        fs::write(zone.join("max_energy_range_uj"), "1000000000").unwrap();
        fs::write(zone.join("energy_uj"), "10000000").unwrap();
        let mut p = PackagePower::new(root.path().into());
        let at = Instant::now();
        assert_eq!(p.collect(1, at)[0].status, SensorStatus::Unavailable);
        fs::write(zone.join("energy_uj"), "60000000").unwrap();
        let row = p.collect(2, at + Duration::from_secs(1)).remove(0);
        assert_eq!(row.value, Some(50.0));
        assert_eq!(row.unit, SensorUnit::Watts);
        let renamed = root.path().join("intel-rapl:9");
        fs::rename(&zone, &renamed).unwrap();
        zone = renamed;
        fs::write(zone.join("energy_uj"), "1000000").unwrap();
        let reset = p.collect(3, at + Duration::from_secs(2)).remove(0);
        assert_eq!(reset.id, row.id);
        assert_eq!(reset.value, None);
        fs::write(zone.join("energy_uj"), "21000000").unwrap();
        assert_eq!(
            p.collect(4, at + Duration::from_secs(3))[0].value,
            Some(20.0)
        );
        fs::remove_file(zone.join("energy_uj")).unwrap();
        assert_eq!(
            p.collect(5, at + Duration::from_secs(4))[0].status,
            SensorStatus::Unavailable
        );
        fs::write(zone.join("energy_uj"), "22000000").unwrap();
        assert_eq!(p.collect(6, at + Duration::from_secs(5))[0].value, None);
        fs::write(zone.join("energy_uj"), "NaN").unwrap();
        assert_eq!(
            p.collect(7, at + Duration::from_secs(6))[0].status,
            SensorStatus::Invalid
        );
        fs::remove_dir_all(zone).unwrap();
        assert!(p.collect(8, at + Duration::from_secs(7)).is_empty());
    }
    #[test]
    fn sleep_reset_requires_a_new_energy_pair() {
        let root = tempfile::tempdir().unwrap();
        let zone = root.path().join("intel-rapl:0");
        fs::create_dir(&zone).unwrap();
        fs::write(zone.join("name"), "package-0").unwrap();
        fs::write(zone.join("max_energy_range_uj"), "1000000000").unwrap();
        fs::write(zone.join("energy_uj"), "10000000").unwrap();
        let mut p = PackagePower::new(root.path().into());
        let at = Instant::now();
        p.collect(0, at);
        fs::write(zone.join("energy_uj"), "60000000").unwrap();
        assert_eq!(
            p.collect(1, at + Duration::from_secs(1))[0].value,
            Some(50.0)
        );
        p.reset_baselines();
        fs::write(zone.join("energy_uj"), "500000000").unwrap();
        assert_eq!(
            p.collect(2, at + Duration::from_secs(2))[0].status,
            SensorStatus::Unavailable
        );
        fs::write(zone.join("energy_uj"), "520000000").unwrap();
        assert_eq!(
            p.collect(3, at + Duration::from_secs(3))[0].value,
            Some(20.0)
        );
    }
}
