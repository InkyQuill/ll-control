//! Per-interface RX/TX byte rates from Linux sysfs counters; no network writes.
//! <https://docs.kernel.org/networking/statistics.html>
use super::{number, reading, text, STALE_AFTER};
use llcontrol_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

type Sample = (u32, Instant, u64);
pub(super) struct Network {
    root: PathBuf,
    boot_id: PathBuf,
    previous: BTreeMap<String, Sample>,
}
impl Network {
    pub(super) fn reset_baselines(&mut self) {
        self.previous.clear();
    }

    pub(super) fn new(root: PathBuf, boot_id: PathBuf) -> Self {
        Self {
            root,
            boot_id,
            previous: BTreeMap::new(),
        }
    }
    pub(super) fn collect(&mut self, now: u64, at: Instant) -> Vec<SensorReading> {
        match self.collect_inner(now, at) {
            Ok((rows, samples)) => {
                self.previous = samples;
                rows
            }
            Err(status) => {
                self.previous.clear();
                vec![reading(
                    "network:discovery",
                    "Network discovery",
                    SensorUnit::BytesPerSecond,
                    Err(status),
                    now,
                )]
            }
        }
    }
    fn collect_inner(
        &self,
        now: u64,
        at: Instant,
    ) -> Result<(Vec<SensorReading>, BTreeMap<String, Sample>), SensorStatus> {
        let entries = fs::read_dir(&self.root).map_err(|_| SensorStatus::Unavailable)?;
        let mut rows = Vec::new();
        let mut samples = BTreeMap::new();
        let mut identities = BTreeSet::new();
        for (index, entry) in entries.enumerate() {
            if index == 128 {
                return Err(SensorStatus::Invalid);
            }
            let entry = entry.map_err(|_| SensorStatus::Unavailable)?;
            let path = entry.path();
            let ifindex = u32::try_from(number(&path.join("ifindex"))?)
                .ok()
                .filter(|n| *n > 0)
                .ok_or(SensorStatus::Invalid)?;
            let id = self.identity(&path, ifindex)?;
            if !identities.insert(id.clone()) {
                return Err(SensorStatus::Invalid);
            }
            let label = entry.file_name().to_string_lossy().into_owned();
            for (file, direction) in [("rx_bytes", "RX"), ("tx_bytes", "TX")] {
                let id = format!("{id}:{file}_per_second");
                let current = text(&path.join("statistics").join(file))
                    .and_then(|v| v.trim().parse::<u64>().map_err(|_| SensorStatus::Invalid));
                let value = current.and_then(|counter| {
                    samples.insert(id.clone(), (ifindex, at, counter));
                    let (old_index, old_at, old_counter) =
                        self.previous.get(&id).ok_or(SensorStatus::Unavailable)?;
                    let elapsed = at
                        .checked_duration_since(*old_at)
                        .ok_or(SensorStatus::Unavailable)?;
                    if old_index != &ifindex || elapsed.is_zero() || elapsed > STALE_AFTER {
                        return Err(SensorStatus::Unavailable);
                    }
                    let delta = counter
                        .checked_sub(*old_counter)
                        .ok_or(SensorStatus::Unavailable)?;
                    Ok(delta as f64 / elapsed.as_secs_f64())
                });
                rows.push(reading(
                    id,
                    format!("{label}: {direction}"),
                    SensorUnit::BytesPerSecond,
                    value,
                    now,
                ));
            }
            // Do not combine two different interfaces if the name was reused mid-read.
            if number(&path.join("ifindex"))? != i64::from(ifindex) {
                return Err(SensorStatus::Unavailable);
            }
        }
        for (file, label) in [("rx_bytes", "RX"), ("tx_bytes", "TX")] {
            let suffix = format!(":{file}_per_second");
            let direction: Vec<_> = rows.iter().filter(|r| r.id.ends_with(&suffix)).collect();
            let total = if direction.is_empty() {
                Err(SensorStatus::Unavailable)
            } else {
                direction
                    .iter()
                    .try_fold(0.0, |sum, row| match (row.status, row.value) {
                        (SensorStatus::Valid, Some(value)) => Ok(sum + value),
                        (status, _) => Err(status),
                    })
            };
            rows.push(reading(
                format!("network:total:{file}_per_second"),
                format!("All interfaces {label} (includes virtual traffic)"),
                SensorUnit::BytesPerSecond,
                total,
                now,
            ));
        }
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok((rows, samples))
    }
    fn identity(&self, path: &Path, ifindex: u32) -> Result<String, SensorStatus> {
        // A permanent hardware address plus physical path survives rename/reboot.
        // Randomized/software interfaces get boot-scoped IDs instead of pretending
        // that an interface name is persistent hardware identity.
        if number(&path.join("addr_assign_type")) == Ok(0) {
            if let Ok(device) = fs::canonicalize(path.join("device")) {
                let address = text(&path.join("address"))?.trim().to_ascii_lowercase();
                if !(17..=95).contains(&address.len())
                    || !address
                        .split(':')
                        .all(|b| b.len() == 2 && b.bytes().all(|b| b.is_ascii_hexdigit()))
                    || !address.bytes().any(|b| b != b'0' && b != b':')
                {
                    return Err(SensorStatus::Invalid);
                }
                use std::os::unix::ffi::OsStrExt;
                let physical = device
                    .as_os_str()
                    .as_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                return Ok(format!("network:{physical}:{address}"));
            }
        }
        let boot = text(&self.boot_id)?;
        let boot = boot.trim();
        if boot.len() != 36
            || !boot.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                }
            })
        {
            return Err(SensorStatus::Invalid);
        }
        Ok(format!(
            "network:boot-{}:if-{ifindex}",
            boot.to_ascii_lowercase()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::fs::symlink, time::Duration};
    use tempfile::TempDir;
    fn fixture() -> (TempDir, Network) {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("net/eth0/statistics")).unwrap();
        fs::create_dir(dir.path().join("device")).unwrap();
        symlink(
            dir.path().join("device"),
            dir.path().join("net/eth0/device"),
        )
        .unwrap();
        for (file, value) in [
            ("ifindex", "2"),
            ("addr_assign_type", "0"),
            ("address", "aa:bb:cc:dd:ee:ff"),
            ("statistics/rx_bytes", "100"),
            ("statistics/tx_bytes", "200"),
        ] {
            fs::write(dir.path().join("net/eth0").join(file), value).unwrap();
        }
        fs::write(
            dir.path().join("boot_id"),
            "01234567-1234-1234-1234-123456789abc",
        )
        .unwrap();
        let provider = Network::new(dir.path().join("net"), dir.path().join("boot_id"));
        (dir, provider)
    }
    #[test]
    fn rates_units_rename_reconnect_and_stale_intervals() {
        let (dir, mut provider) = fixture();
        let at = Instant::now();
        let warm = provider.collect(0, at);
        assert!(warm.iter().all(|r| r.status == SensorStatus::Unavailable));
        fs::write(dir.path().join("net/eth0/statistics/rx_bytes"), "2100").unwrap();
        let rows = provider.collect(1, at + Duration::from_secs(2));
        assert_eq!(rows[0].value, Some(1000.0));
        assert_eq!(rows[1].value, Some(0.0));
        assert_eq!(
            llcontrol_ipc::theme_sensor_text(
                &rows,
                &rows[0].id,
                SensorUnit::BytesPerSecond,
                llcontrol_ipc::TemperatureUnit::Fahrenheit,
                1
            ),
            "1000.0 B/s"
        );
        fs::rename(dir.path().join("net/eth0"), dir.path().join("net/renamed")).unwrap();
        let renamed = provider.collect(2, at + Duration::from_secs(3));
        assert_eq!(renamed[0].id, rows[0].id);
        assert_eq!(renamed[0].value, Some(0.0));
        let stale = provider.collect(3, at + Duration::from_secs(7));
        assert_eq!(stale[0].status, SensorStatus::Unavailable);
        assert_eq!(
            provider.collect(4, at + Duration::from_secs(8))[0].value,
            Some(0.0)
        );
        fs::write(dir.path().join("net/renamed/ifindex"), "7").unwrap();
        let reconnected = provider.collect(5, at + Duration::from_secs(9));
        assert_eq!(reconnected[0].id, rows[0].id);
        assert_eq!(reconnected[0].status, SensorStatus::Unavailable);
        fs::remove_dir_all(dir.path().join("net/renamed")).unwrap();
        let removed = provider.collect(6, at + Duration::from_secs(10));
        assert_eq!(removed.len(), 2);
        assert!(removed
            .iter()
            .all(|r| r.id.starts_with("network:total:") && r.status == SensorStatus::Unavailable));
    }
    #[test]
    fn resets_errors_and_recovery_do_not_corrupt_other_direction() {
        let (dir, mut provider) = fixture();
        let at = Instant::now();
        provider.collect(0, at);
        for (step, value, expected) in [
            (1, "99", SensorStatus::Unavailable),
            (2, "NaN", SensorStatus::Invalid),
            (3, "300", SensorStatus::Unavailable),
            (4, "300", SensorStatus::Valid),
        ] {
            fs::write(dir.path().join("net/eth0/statistics/rx_bytes"), value).unwrap();
            let rows = provider.collect(step, at + Duration::from_secs(step));
            assert_eq!(rows[0].status, expected);
            assert_eq!(rows[1].value, Some(0.0));
        }
        fs::remove_file(dir.path().join("net/eth0/statistics/rx_bytes")).unwrap();
        assert_eq!(
            provider.collect(5, at + Duration::from_secs(5))[0].status,
            SensorStatus::Unavailable
        );
    }
    #[test]
    fn virtual_identity_is_boot_scoped_and_ambiguous_devices_are_rejected() {
        let (dir, mut provider) = fixture();
        let at = Instant::now();
        fs::remove_file(dir.path().join("net/eth0/device")).unwrap();
        let old = provider.collect(0, at);
        assert!(old[0].id.contains("boot-"));
        fs::write(
            dir.path().join("boot_id"),
            "abcdef12-1234-1234-1234-123456789abc",
        )
        .unwrap();
        let new = provider.collect(1, at + Duration::from_secs(1));
        assert_ne!(old[0].id, new[0].id);
        symlink(
            dir.path().join("net/eth0"),
            dir.path().join("net/duplicate"),
        )
        .unwrap();
        let bad = provider.collect(2, at + Duration::from_secs(2));
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].status, SensorStatus::Invalid);
    }
    #[test]
    fn zero_interval_bad_identity_and_catalog_bound_are_explicit() {
        let (dir, mut provider) = fixture();
        let at = Instant::now();
        provider.collect(0, at);
        assert_eq!(provider.collect(0, at)[0].status, SensorStatus::Unavailable);
        fs::write(dir.path().join("net/eth0/address"), "aa").unwrap();
        assert_eq!(
            provider.collect(1, at + Duration::from_secs(1))[0].status,
            SensorStatus::Invalid
        );
        fs::write(dir.path().join("net/eth0/address"), "aa:bb:cc:dd:ee:ff").unwrap();
        fs::remove_dir_all(dir.path().join("net/eth0")).unwrap();
        for i in 0..129 {
            let path = dir.path().join(format!("net/v{i}"));
            fs::create_dir_all(path.join("statistics")).unwrap();
            fs::write(path.join("ifindex"), (i + 1).to_string()).unwrap();
            fs::write(path.join("statistics/rx_bytes"), "0").unwrap();
            fs::write(path.join("statistics/tx_bytes"), "0").unwrap();
        }
        let rows = provider.collect(2, at + Duration::from_secs(2));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, SensorStatus::Invalid);
        fs::remove_dir_all(dir.path().join("net")).unwrap();
        assert_eq!(
            provider.collect(3, at + Duration::from_secs(3))[0].status,
            SensorStatus::Unavailable
        );
    }
    #[test]
    fn totals_sum_interface_rates_and_reject_partial_measurements() {
        let (dir, mut provider) = fixture();
        let extra = dir.path().join("net/vpn0");
        fs::create_dir_all(extra.join("statistics")).unwrap();
        fs::write(extra.join("ifindex"), "3").unwrap();
        fs::write(extra.join("statistics/rx_bytes"), "100").unwrap();
        fs::write(extra.join("statistics/tx_bytes"), "200").unwrap();
        let at = Instant::now();
        provider.collect(0, at);
        fs::write(extra.join("statistics/rx_bytes"), "2100").unwrap();
        fs::write(dir.path().join("net/eth0/statistics/rx_bytes"), "1100").unwrap();
        let rows = provider.collect(1, at + Duration::from_secs(2));
        let total = |rows: Vec<SensorReading>| {
            rows.into_iter()
                .find(|r| r.id == "network:total:rx_bytes_per_second")
                .unwrap()
        };
        assert_eq!(total(rows).value, Some(1500.0));
        fs::remove_file(extra.join("statistics/rx_bytes")).unwrap();
        assert_eq!(
            total(provider.collect(2, at + Duration::from_secs(3))).status,
            SensorStatus::Unavailable
        );
        fs::remove_dir_all(extra).unwrap();
        assert_eq!(
            total(provider.collect(3, at + Duration::from_secs(4))).value,
            Some(0.0)
        );
    }
    #[test]
    fn sleep_reset_requires_new_interface_and_total_rate_pairs() {
        let (dir, mut provider) = fixture();
        let at = Instant::now();
        provider.collect(0, at);
        let rx = dir.path().join("net/eth0/statistics/rx_bytes");
        fs::write(&rx, "1100").unwrap();
        assert_eq!(
            provider.collect(1, at + Duration::from_secs(1))[0].value,
            Some(1000.0)
        );
        provider.reset_baselines();
        fs::write(&rx, "9000000").unwrap();
        assert!(provider
            .collect(2, at + Duration::from_secs(2))
            .iter()
            .all(|r| r.status == SensorStatus::Unavailable));
        fs::write(&rx, "9000500").unwrap();
        let rows = provider.collect(3, at + Duration::from_secs(3));
        assert_eq!(rows[0].value, Some(500.0));
        assert_eq!(
            rows.iter()
                .find(|r| r.id == "network:total:rx_bytes_per_second")
                .unwrap()
                .value,
            Some(500.0)
        );
    }
}
