//! Bounded, read-only NVIDIA query in a separate killable process.
//! ABI: <https://docs.nvidia.com/deploy/nvidia-smi/index.html>
use super::query_process::query_command;
use super::reading;
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{collections::BTreeSet, path::PathBuf};
use tokio::process::{Child, Command};

const LIMIT: usize = 65_536;
const MAX_GPUS: usize = 32;

pub(super) struct NvidiaSensors {
    executable: PathBuf,
    previous: Vec<SensorReading>,
    child: Option<Child>,
    voltage: Option<super::nvidia_voltage::NvidiaVoltage>,
}
impl NvidiaSensors {
    pub(super) fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            previous: Vec::new(),
            child: None,
            voltage: None,
        }
    }

    pub(super) fn with_voltage_from_env(mut self) -> Self {
        self.voltage = super::nvidia_voltage::NvidiaVoltage::from_env();
        self
    }

    pub(super) async fn collect(&mut self, now: u64) -> Vec<SensorReading> {
        match self.query().await.and_then(|bytes| parse(&bytes, now)) {
            Ok(mut rows) => {
                if let Some(voltage) = &mut self.voltage {
                    rows.extend(voltage.collect(&rows, now).await);
                }
                self.previous = rows.clone();
                rows
            }
            Err(status) => {
                let mut rows = self
                    .previous
                    .iter()
                    .map(|row| reading(&row.id, &row.label, row.unit, Err(status), now))
                    .collect::<Vec<_>>();
                rows.push(reading(
                    "nvidia:discovery",
                    "NVIDIA query unavailable or invalid",
                    SensorUnit::Percent,
                    Err(status),
                    now,
                ));
                rows
            }
        }
    }

    async fn query(&mut self) -> Result<Vec<u8>, SensorStatus> {
        query_command(Command::new(&self.executable).args([
            "--query-gpu=uuid,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw,clocks.current.graphics,clocks.current.memory",
            "--format=csv,noheader,nounits",
        ]), &mut self.child, LIMIT).await
    }
}

fn scalar(value: &str, min: f64, max: f64) -> Result<f64, SensorStatus> {
    if value == "N/A" || value == "[Not Supported]" {
        return Err(SensorStatus::Unavailable);
    }
    let n = value.parse::<f64>().map_err(|_| SensorStatus::Invalid)?;
    if n.is_finite() && (min..=max).contains(&n) {
        Ok(n)
    } else {
        Err(SensorStatus::Invalid)
    }
}
fn parse(bytes: &[u8], now: u64) -> Result<Vec<SensorReading>, SensorStatus> {
    if bytes.len() > LIMIT {
        return Err(SensorStatus::Invalid);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SensorStatus::Invalid)?;
    let mut ids = BTreeSet::new();
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if fields.len() != 8 || ids.len() >= MAX_GPUS {
            return Err(SensorStatus::Invalid);
        }
        let uuid = fields[0]
            .strip_prefix("GPU-")
            .ok_or(SensorStatus::Invalid)?;
        if uuid.len() != 36
            || !uuid.bytes().enumerate().all(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c == b'-'
                } else {
                    c.is_ascii_hexdigit()
                }
            })
            || !ids.insert(uuid.to_ascii_lowercase())
        {
            return Err(SensorStatus::Invalid);
        }
        let id = format!("nvidia:GPU-{}", uuid.to_ascii_lowercase());
        let temperature = scalar(fields[1], -273.15, 300.0);
        let load = scalar(fields[2], 0.0, 100.0);
        // Legacy power.draw semantics depend on GPU/driver; do not label it instantaneous.
        let power = scalar(fields[5], 0.0, f64::MAX);
        // NVIDIA reports current clocks in MHz; the shared catalog uses Hz.
        let clock =
            |field: &str| scalar(field, 0.0, f64::MAX / 1_000_000.0).map(|mhz| mhz * 1_000_000.0);
        let total = scalar(fields[4], 0.0, 8_589_934_592.0).map(|v| v * 1_048_576.0);
        let used = scalar(fields[3], 0.0, 8_589_934_592.0)
            .map(|v| v * 1_048_576.0)
            .and_then(|used| match total {
                Ok(total) if used > total => Err(SensorStatus::Invalid),
                _ => Ok(used),
            });
        let percent = used.and_then(|used| {
            total.and_then(|total| {
                if total > 0.0 {
                    Ok(100.0 * used / total)
                } else {
                    Err(SensorStatus::Unavailable)
                }
            })
        });
        for (metric, unit, value) in [
            ("temperature", SensorUnit::Celsius, temperature),
            ("utilization", SensorUnit::Percent, load),
            ("vram_used", SensorUnit::Bytes, used),
            ("vram_total", SensorUnit::Bytes, total),
            ("vram_utilization", SensorUnit::Percent, percent),
            ("power_draw", SensorUnit::Watts, power),
            ("graphics_clock", SensorUnit::Hertz, clock(fields[6])),
            ("memory_clock", SensorUnit::Hertz, clock(fields[7])),
        ] {
            rows.push(reading(
                format!("{id}:{metric}"),
                format!("{id} {metric}"),
                unit,
                value,
                now,
            ));
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{process::Stdio, time::Duration};
    const GPU: &str = "GPU-12345678-1234-1234-1234-123456789abc";
    #[test]
    fn csv_units_identity_unavailable_and_invalid_are_explicit() {
        let rows = parse(
            format!("{GPU}, 42, 37, 2048, 8192, 125.50, 1800, 7000\n").as_bytes(),
            10,
        )
        .unwrap();
        assert_eq!(
            rows.iter().map(|r| r.value).collect::<Vec<_>>(),
            vec![
                Some(42.0),
                Some(37.0),
                Some(2147483648.0),
                Some(8589934592.0),
                Some(25.0),
                Some(125.5),
                Some(1_800_000_000.0),
                Some(7_000_000_000.0)
            ]
        );
        assert_eq!(rows[0].id, format!("nvidia:{GPU}:temperature"));
        let rows = parse(
            format!("{GPU}, N/A, NaN, 9, 8, N/A, N/A, N/A").as_bytes(),
            10,
        )
        .unwrap();
        assert_eq!(rows[0].status, SensorStatus::Unavailable);
        for i in [1, 2, 4] {
            assert_eq!(rows[i].status, SensorStatus::Invalid);
            assert_eq!(rows[i].value, None);
        }
        assert!(parse(
            format!("{GPU},1,2,3,4,5,6,7\n{GPU},1,2,3,4,5,6,7").as_bytes(),
            10
        )
        .is_err());
        assert!(parse(b"0,1,2,3,4,5,6,7", 10).is_err());
        assert!(parse(&vec![b' '; LIMIT + 1], 10).is_err());
    }
    #[test]
    fn enumeration_order_does_not_change_bindings_and_missing_gpu_disappears() {
        let second = "GPU-abcdef12-1234-1234-1234-123456789abc";
        let one = format!("{GPU},42,10,1,4,0,0,0");
        let two = format!("{second},65,90,3,4,250.25,2200,8000");
        let a = parse(format!("{one}\n{two}").as_bytes(), 1).unwrap();
        let b = parse(format!("{two}\n{one}").as_bytes(), 1).unwrap();
        for row in &a {
            assert_eq!(b.iter().find(|r| r.id == row.id).unwrap(), row);
        }
        let remaining = parse(two.as_bytes(), 1).unwrap();
        assert_eq!(remaining.len(), 8);
        assert!(remaining.iter().all(|r| r.id.contains(second)));
        assert!(parse("".as_bytes(), 1).unwrap().is_empty());
    }

    #[test]
    fn power_is_typed_watts_and_missing_invalid_zero_are_distinct() {
        use lconnect_ipc::theme::{theme_sensor_text, TemperatureUnit};
        for (value, status, expected) in [
            ("42.25", SensorStatus::Valid, "42.25 W"),
            ("0", SensorStatus::Valid, "0.00 W"),
            ("N/A", SensorStatus::Unavailable, "—"),
            ("[Not Supported]", SensorStatus::Unavailable, "—"),
            ("-1", SensorStatus::Invalid, "—"),
            ("NaN", SensorStatus::Invalid, "—"),
            ("inf", SensorStatus::Invalid, "—"),
        ] {
            let rows = parse(format!("{GPU},42,10,1,4,{value},1800,7000").as_bytes(), 10).unwrap();
            let row = rows
                .iter()
                .find(|row| row.unit == SensorUnit::Watts)
                .unwrap();
            assert_eq!(row.id, format!("nvidia:{GPU}:power_draw"));
            assert_eq!(row.status, status);
            assert_eq!(
                theme_sensor_text(
                    &rows,
                    &row.id,
                    SensorUnit::Watts,
                    TemperatureUnit::Celsius,
                    2
                ),
                expected
            );
            assert_eq!(row.value.is_some(), status == SensorStatus::Valid);
            assert_eq!(
                rows[0].status,
                SensorStatus::Valid,
                "power failure cannot invalidate temperature"
            );
        }
        assert!(
            parse(format!("{GPU},42,10,1,4").as_bytes(), 10).is_err(),
            "old layout cannot silently shift columns"
        );
    }

    #[test]
    fn clocks_keep_domains_units_and_errors_independent() {
        use lconnect_ipc::{theme_sensor_text, TemperatureUnit};
        for (value, status, expected) in [
            ("1800.5", SensorStatus::Valid, "1800.50 MHz"),
            ("0", SensorStatus::Valid, "0.00 MHz"),
            ("N/A", SensorStatus::Unavailable, "—"),
            ("[Not Supported]", SensorStatus::Unavailable, "—"),
            ("-1", SensorStatus::Invalid, "—"),
            ("NaN", SensorStatus::Invalid, "—"),
            ("inf", SensorStatus::Invalid, "—"),
            ("1e308", SensorStatus::Invalid, "—"),
            ("1.797693134862316e302", SensorStatus::Invalid, "—"),
        ] {
            for (graphics, memory, metric, other) in [
                (value, "7000", "graphics_clock", "memory_clock"),
                ("7000", value, "memory_clock", "graphics_clock"),
            ] {
                let rows = parse(
                    format!("{GPU},42,10,1,4,100,{graphics},{memory}").as_bytes(),
                    1,
                )
                .unwrap();
                let id = format!("nvidia:{GPU}:{metric}");
                let row = rows.iter().find(|r| r.id == id).unwrap();
                assert_eq!(row.unit, SensorUnit::Hertz);
                assert_eq!(row.status, status);
                assert_eq!(
                    theme_sensor_text(
                        &rows,
                        &id,
                        SensorUnit::Hertz,
                        TemperatureUnit::Fahrenheit,
                        2
                    ),
                    expected
                );
                assert_eq!(
                    rows.iter().find(|r| r.id.ends_with(other)).unwrap().value,
                    Some(7_000_000_000.0)
                );
                assert_eq!(rows[0].value, Some(42.0));
                assert_eq!(rows[5].value, Some(100.0));
            }
        }
        assert!(
            parse(format!("{GPU},42,10,1,4,100").as_bytes(), 1).is_err(),
            "previous six-column layout must not be accepted"
        );
    }

    #[tokio::test]
    async fn query_requests_power_and_clocks_with_bounded_readonly_arguments() {
        let (_directory, mut provider) = fixture(&format!(
            "[ \"$1\" = '--query-gpu=uuid,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw,clocks.current.graphics,clocks.current.memory' ] || exit 1\n[ \"$2\" = '--format=csv,noheader,nounits' ] || exit 1\n[ \"$LC_ALL\" = C ] || exit 1\nprintf '%s\\n' '{GPU},42,10,1,4,123.75,1800,7000'"
        ));
        let rows = provider.collect(1).await;
        assert_eq!(
            rows.iter()
                .find(|row| row.unit == SensorUnit::Watts)
                .unwrap()
                .value,
            Some(123.75)
        );
    }

    #[tokio::test]
    async fn voltage_uuid_binding_failure_isolation_cache_and_recovery() {
        use super::super::{nvidia_voltage::NvidiaVoltage, SensorCache};
        use crate::sensor_selection;
        use lconnect_ipc::{theme_sensor_text, DefaultSensorRole, TemperatureUnit};
        use std::time::Instant;
        let second = "GPU-abcdef12-1234-1234-1234-123456789abc";
        let (smi_dir, mut provider) = fixture(&format!(
            "printf '%s\\n' '{GPU},42,10,1,4,100,1800,7000' '{second},43,20,2,4,110,1900,8000'"
        ));
        let (voltage_dir, voltage_fixture) = fixture(&format!(
            r#"case "$6" in
'[gpu:{GPU}]/GpuUUID') printf '%s\n' '{GPU}' 975000 ;;
'[gpu:{second}]/GpuUUID') exit 1 ;;
*) exit 1 ;;
esac"#
        ));
        provider.voltage = Some(NvidiaVoltage {
            executable: voltage_fixture.executable,
            display: ":fixture".into(),
            authority: voltage_dir.path().join("auth"),
            children: Default::default(),
        });
        let rows = provider.collect(1).await;
        let id = format!("nvidia:{GPU}:core_voltage");
        let other = format!("nvidia:{second}:core_voltage");
        assert_eq!(rows.iter().find(|r| r.id == id).unwrap().value, Some(0.975));
        assert_eq!(
            rows.iter().find(|r| r.id == other).unwrap().status,
            SensorStatus::Unavailable
        );
        assert!(rows
            .iter()
            .filter(|r| r.unit != SensorUnit::Volts)
            .all(|r| r.status == SensorStatus::Valid));
        let choice = sensor_selection::candidates(&rows)
            .into_iter()
            .find(|c| c.reading.id == id)
            .unwrap();
        assert_eq!(choice.role, DefaultSensorRole::GpuVoltage);
        let defaults = sensor_selection::recommended(&rows, 0);
        sensor_selection::validate_selection(&defaults, &Default::default(), &rows).unwrap();
        let cache = SensorCache::default();
        cache.publish(Instant::now(), rows.clone());
        assert_eq!(
            theme_sensor_text(
                &cache.readings(),
                &id,
                SensorUnit::Volts,
                TemperatureUnit::Celsius,
                2
            ),
            "0.97 V"
        );
        cache.publish(Instant::now() - Duration::from_secs(4), rows);
        assert_eq!(
            theme_sensor_text(
                &cache.readings(),
                &id,
                SensorUnit::Volts,
                TemperatureUnit::Celsius,
                2
            ),
            "—"
        );
        // Reordering discovery cannot bind the successful rail to the other GPU.
        std::fs::write(smi_dir.path().join("query"), format!("#!/bin/sh\nprintf '%s\\n' '{second},43,20,2,4,110,1900,8000' '{GPU},42,10,1,4,100,1800,7000'\n")).unwrap();
        assert_eq!(
            provider
                .collect(2)
                .await
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .value,
            Some(0.975)
        );
        std::fs::write(voltage_dir.path().join("query"), "#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(
            provider
                .collect(3)
                .await
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .value,
            None
        );
        std::fs::write(
            voltage_dir.path().join("query"),
            format!("#!/bin/sh\nprintf '%s\\n' '{GPU}' 0\n"),
        )
        .unwrap();
        assert_eq!(
            provider
                .collect(4)
                .await
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .value,
            Some(0.0)
        );
        std::fs::write(smi_dir.path().join("query"), "#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(
            provider
                .collect(5)
                .await
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .status,
            SensorStatus::Unavailable
        );
        std::fs::write(smi_dir.path().join("query"), "#!/bin/sh\nexit 0\n").unwrap();
        assert!(provider.collect(6).await.is_empty());
    }

    fn fixture(script: &str) -> (tempfile::TempDir, NvidiaSensors) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("query");
        std::fs::write(&exe, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        let provider = NvidiaSensors::new(exe);
        (dir, provider)
    }
    #[tokio::test]
    async fn process_failure_retains_ids_without_old_values_and_can_recover() {
        let (dir, mut provider) = fixture(&format!(
            "printf '%s\\n' '{GPU}, 42, 37, 2048, 8192, 125.50, 1800, 7000'"
        ));
        let first = provider.collect(1).await;
        assert_eq!(first.len(), 8);
        std::fs::write(dir.path().join("query"), "#!/bin/sh\nexit 1\n").unwrap();
        let failed = provider.collect(2).await;
        assert_eq!(failed.len(), 9);
        assert!(failed
            .iter()
            .all(|r| r.status == SensorStatus::Unavailable && r.value.is_none()));
        assert_eq!(failed[0].id, first[0].id);
        std::fs::write(
            dir.path().join("query"),
            format!("#!/bin/sh\nprintf '%s\\n' '{GPU}, 43, 0, 0, 8192, 0, 0, 0'\n"),
        )
        .unwrap();
        assert_eq!(provider.collect(3).await[0].value, Some(43.0));
    }
    #[tokio::test]
    async fn timeout_and_oversized_output_are_bounded_and_child_reaped() {
        let (dir, mut provider) = fixture("exec sleep 10");
        // Capture the PID from spawn, not a file the child may never get CPU time to write.
        let child = Command::new(&provider.executable)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        let start = std::time::Instant::now();
        assert_eq!(
            super::super::query_process::read_output(&mut Some(child), LIMIT).await,
            Err(SensorStatus::Unavailable)
        );
        assert!(start.elapsed() >= Duration::from_millis(750));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        std::fs::write(
            dir.path().join("query"),
            "#!/bin/sh\nexec head -c 65537 /dev/zero\n",
        )
        .unwrap();
        assert_eq!(provider.collect(2).await[0].status, SensorStatus::Invalid);
    }
}
