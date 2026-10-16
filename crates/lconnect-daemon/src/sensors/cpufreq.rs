//! Read-only CPUFreq policy frequencies. Never changes governors or limits.
//! ABI: <https://docs.kernel.org/admin-guide/pm/cpufreq.html#policy-interface-in-sysfs>
use super::{number, reading, text};
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{collections::BTreeSet, fs, path::PathBuf};

const MAX_POLICIES: usize = 128;
const MAX_CPUS: usize = 128;

pub(super) struct CpuFrequency {
    root: PathBuf,
}

impl CpuFrequency {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(super) fn collect(&self, now: u64) -> Vec<SensorReading> {
        let failure = |status| {
            vec![reading(
                "cpufreq:discovery",
                "CPU frequency unavailable",
                SensorUnit::Hertz,
                Err(status),
                now,
            )]
        };
        let Ok(entries) = fs::read_dir(&self.root) else {
            return failure(SensorStatus::Unavailable);
        };
        let mut rows = Vec::new();
        let mut seen_cpus = BTreeSet::new();
        for (position, entry) in entries.enumerate() {
            // Bound enumeration as well as the returned catalog.
            if position == MAX_POLICIES {
                return failure(SensorStatus::Invalid);
            }
            let Ok(entry) = entry else {
                return failure(SensorStatus::Unavailable);
            };
            let name = entry.file_name();
            let Some(suffix) = name.to_str().and_then(|name| name.strip_prefix("policy")) else {
                continue;
            };
            if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let path = entry.path();
            let cpus = match text(&path.join("related_cpus")).and_then(|s| related_cpus(&s)) {
                Ok(cpus) => cpus,
                Err(status) => return failure(status),
            };
            // Overlapping policies are ambiguous (or changed during discovery).
            if cpus.iter().any(|cpu| !seen_cpus.insert(*cpu)) {
                return failure(SensorStatus::Invalid);
            }
            let identity = cpus
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(".");
            let label = if cpus.len() == 1 {
                format!("CPU {}", cpus.first().expect("nonempty CPU set"))
            } else {
                format!(
                    "CPU group {} ({} CPUs)",
                    cpus.first().expect("nonempty CPU set"),
                    cpus.len()
                )
            };
            for (file, description) in [
                ("cpuinfo_cur_freq", "hardware current"),
                ("cpuinfo_avg_freq", "hardware average"),
                ("scaling_cur_freq", "driver reported"),
            ] {
                // Keep these sources distinct; never use the requested P-state
                // as a fallback for unavailable hardware feedback.
                let value = number(&path.join(file)).and_then(|khz| {
                    if khz < 0 {
                        Err(SensorStatus::Invalid)
                    } else {
                        Ok(khz as f64 * 1000.0)
                    }
                });
                rows.push(reading(
                    format!("cpufreq:cpus-{identity}:{file}"),
                    format!("{label}: {description}"),
                    SensorUnit::Hertz,
                    value,
                    now,
                ));
            }
        }
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        rows
    }
}

fn related_cpus(input: &str) -> Result<BTreeSet<u16>, SensorStatus> {
    let mut cpus = BTreeSet::new();
    for (index, token) in input.split_whitespace().enumerate() {
        if index == MAX_CPUS || !token.bytes().all(|b| b.is_ascii_digit()) {
            return Err(SensorStatus::Invalid);
        }
        let cpu = token.parse::<u16>().map_err(|_| SensorStatus::Invalid)?;
        if !cpus.insert(cpu) {
            return Err(SensorStatus::Invalid);
        }
    }
    if cpus.is_empty() {
        return Err(SensorStatus::Invalid);
    }
    Ok(cpus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, CpuFrequency) {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("policy0")).unwrap();
        fs::write(dir.path().join("policy0/related_cpus"), "0 1\n").unwrap();
        let provider = CpuFrequency::new(dir.path().to_owned());
        (dir, provider)
    }

    #[test]
    fn source_identity_units_renumber_and_topology_change_are_explicit() {
        let (dir, provider) = fixture();
        for (name, value) in [
            ("cpuinfo_cur_freq", "2300000"),
            ("cpuinfo_avg_freq", "2100000"),
            ("scaling_cur_freq", "3000000"),
        ] {
            fs::write(dir.path().join("policy0").join(name), value).unwrap();
        }
        let first = provider.collect(1);
        assert_eq!(first.len(), 3);
        let hardware = first
            .iter()
            .find(|r| r.id.ends_with(":cpuinfo_cur_freq"))
            .unwrap();
        assert_eq!(hardware.value, Some(2_300_000_000.0));
        assert_eq!(hardware.unit, SensorUnit::Hertz);
        assert_eq!(
            lconnect_ipc::theme_sensor_text(
                &first,
                &hardware.id,
                SensorUnit::Hertz,
                lconnect_ipc::TemperatureUnit::Fahrenheit,
                1
            ),
            "2300.0 MHz"
        );
        fs::rename(dir.path().join("policy0"), dir.path().join("policy7")).unwrap();
        fs::write(dir.path().join("policy7/related_cpus"), "1 0").unwrap();
        fs::write(dir.path().join("policy7/affected_cpus"), "0").unwrap();
        assert_eq!(
            provider.collect(1),
            first,
            "offline CPUs must not change identity"
        );
        fs::remove_file(dir.path().join("policy7/cpuinfo_cur_freq")).unwrap();
        let missing = provider.collect(2);
        let row = missing.iter().find(|r| r.id == hardware.id).unwrap();
        assert_eq!(row.status, SensorStatus::Unavailable);
        assert_eq!(
            row.value, None,
            "driver frequency must not substitute hardware"
        );
        assert_eq!(
            missing
                .iter()
                .find(|r| r.id.ends_with(":scaling_cur_freq"))
                .unwrap()
                .value,
            Some(3_000_000_000.0)
        );
        fs::write(dir.path().join("policy7/related_cpus"), "2 3").unwrap();
        assert!(!provider.collect(3).iter().any(|r| r.id == hardware.id));
        fs::remove_dir_all(dir.path().join("policy7")).unwrap();
        assert!(provider.collect(4).is_empty());
    }

    #[test]
    fn failures_zero_recovery_and_ambiguous_policies_are_explicit() {
        let (dir, provider) = fixture();
        for (value, status) in [
            ("-1", SensorStatus::Invalid),
            ("NaN", SensorStatus::Invalid),
            ("9223372036854775808", SensorStatus::Invalid),
            ("0", SensorStatus::Valid),
        ] {
            fs::write(dir.path().join("policy0/scaling_cur_freq"), value).unwrap();
            let row = provider
                .collect(1)
                .into_iter()
                .find(|r| r.id.ends_with(":scaling_cur_freq"))
                .unwrap();
            assert_eq!(row.status, status);
            assert_eq!(
                row.value,
                if status == SensorStatus::Valid {
                    Some(0.0)
                } else {
                    None
                }
            );
        }
        fs::create_dir(dir.path().join("policy1")).unwrap();
        fs::write(dir.path().join("policy1/related_cpus"), "1 2").unwrap();
        let ambiguous = provider.collect(1);
        assert_eq!(ambiguous.len(), 1);
        assert_eq!(ambiguous[0].id, "cpufreq:discovery");
        assert_eq!(ambiguous[0].status, SensorStatus::Invalid);
        fs::remove_dir_all(dir.path().join("policy1")).unwrap();
        assert_eq!(provider.collect(2).len(), 3);
        fs::remove_file(dir.path().join("policy0/related_cpus")).unwrap();
        assert_eq!(provider.collect(3)[0].status, SensorStatus::Unavailable);
    }

    #[test]
    fn malformed_and_oversized_cpu_sets_or_catalogs_are_rejected() {
        for input in ["", "0 0", "-1", "+1", "0-3", "65536", "0x1"] {
            assert!(related_cpus(input).is_err(), "{input}");
        }
        assert!(related_cpus(
            &(0..=MAX_CPUS)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )
        .is_err());
        let (dir, provider) = fixture();
        for n in 1..=MAX_POLICIES {
            fs::create_dir(dir.path().join(format!("policy{n}"))).unwrap();
            fs::write(
                dir.path().join(format!("policy{n}/related_cpus")),
                (n + 1).to_string(),
            )
            .unwrap();
        }
        assert_eq!(provider.collect(1)[0].status, SensorStatus::Invalid);
        let missing = CpuFrequency::new(dir.path().join("missing"));
        assert_eq!(missing.collect(1)[0].status, SensorStatus::Unavailable);
    }
}
