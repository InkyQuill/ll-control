//! Optional NV-CONTROL voltage, independent of the nvidia-smi CSV query.
//! Explicit session configuration avoids borrowing a desktop's credentials.
use super::query_process::{query_command, reap_finished};
use super::reading;
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use tokio::process::{Child, Command};

pub(super) struct NvidiaVoltage {
    pub(super) executable: PathBuf,
    pub(super) display: String,
    pub(super) authority: PathBuf,
    pub(super) children: BTreeMap<String, Option<Child>>,
}

impl NvidiaVoltage {
    pub(super) fn from_env() -> Option<Self> {
        let display = std::env::var("LCONNECT_NVIDIA_VOLTAGE_DISPLAY").ok()?;
        let authority = PathBuf::from(std::env::var_os("LCONNECT_NVIDIA_VOLTAGE_XAUTHORITY")?);
        if display.is_empty()
            || display.len() > 256
            || display.contains('\0')
            || !authority.is_absolute()
        {
            tracing::warn!("Invalid explicit NVIDIA voltage session configuration");
            return None;
        }
        Some(Self {
            executable: "/usr/bin/nvidia-settings".into(),
            display,
            authority,
            children: BTreeMap::new(),
        })
    }

    pub(super) async fn collect(&mut self, rows: &[SensorReading], now: u64) -> Vec<SensorReading> {
        // Parent CSV parsing validates UUIDs and limits discovery to32 GPUs.
        // Keep unreaped children even if their GPU disappears: churn must not
        // permit an unbounded number of helpers stuck in driver I/O.
        let ids: BTreeSet<_> = rows
            .iter()
            .filter_map(|r| r.id.strip_prefix("nvidia:")?.strip_suffix(":temperature"))
            .collect();
        self.children
            .retain(|uuid, child| ids.contains(uuid.as_str()) || !reap_finished(child));
        for uuid in &ids {
            if self.children.len() < 32 {
                self.children.entry((*uuid).to_owned()).or_default();
            }
        }
        let mut readings = futures::future::join_all(
            self.children
                .iter_mut()
                .filter(|(uuid, _)| ids.contains(uuid.as_str()))
                .map(|(uuid, child)| {
                    let executable = &self.executable;
                    let display = &self.display;
                    let authority = &self.authority;
                    async move {
                        let result = query_command(
                            Command::new(executable)
                                .args([
                                    "--no-config",
                                    "--terse",
                                    "--ctrl-display",
                                    display,
                                    "--query",
                                    &format!("[gpu:{uuid}]/GpuUUID"),
                                    "--query",
                                    &format!("[gpu:{uuid}]/GPUCurrentCoreVoltage"),
                                ])
                                .env("DISPLAY", display)
                                .env("XAUTHORITY", authority),
                            child,
                            4096,
                        )
                        .await
                        .and_then(|bytes| parse(&bytes, uuid));
                        voltage_reading(uuid, result, now)
                    }
                }),
        )
        .await;
        for uuid in ids {
            if !self.children.contains_key(uuid) {
                readings.push(voltage_reading(uuid, Err(SensorStatus::Unavailable), now));
            }
        }
        readings
    }

    #[cfg(test)]
    async fn query(&mut self, uuid: &str) -> Result<f64, SensorStatus> {
        let rows = [reading(
            format!("nvidia:{uuid}:temperature"),
            "fixture",
            SensorUnit::Celsius,
            Ok(40.0),
            0,
        )];
        let row = self.collect(&rows, 0).await.remove(0);
        row.value.ok_or(row.status)
    }
}

fn voltage_reading(uuid: &str, value: Result<f64, SensorStatus>, now: u64) -> SensorReading {
    reading(
        format!("nvidia:{uuid}:core_voltage"),
        format!("{uuid} core voltage (NV-CONTROL)"),
        SensorUnit::Volts,
        value,
        now,
    )
}

fn parse(bytes: &[u8], uuid: &str) -> Result<f64, SensorStatus> {
    let text = std::str::from_utf8(bytes).map_err(|_| SensorStatus::Invalid)?;
    let mut lines = text.lines().map(str::trim).filter(|s| !s.is_empty());
    let returned = lines.next().ok_or(SensorStatus::Unavailable)?;
    if !returned.eq_ignore_ascii_case(uuid) {
        return Err(SensorStatus::Invalid);
    }
    let value = lines.next().ok_or(SensorStatus::Unavailable)?;
    if lines.next().is_some() {
        return Err(SensorStatus::Invalid);
    }
    if value == "N/A" {
        return Err(SensorStatus::Unavailable);
    }
    // NV-CONTROL attribute 413 is a nonnegative signed integer in microvolts.
    if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(SensorStatus::Invalid);
    }
    let uv = value.parse::<i32>().map_err(|_| SensorStatus::Invalid)?;
    Ok(f64::from(uv) / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    const GPU: &str = "GPU-12345678-1234-1234-1234-123456789abc";
    #[test]
    fn identity_units_zero_and_invalid_outputs() {
        assert_eq!(parse(format!("{GPU}\n975000\n").as_bytes(), GPU), Ok(0.975));
        assert_eq!(parse(format!("{GPU}\n0\n").as_bytes(), GPU), Ok(0.0));
        for text in [
            format!("{GPU}\n-1"),
            format!("{GPU}\nNaN"),
            format!("{GPU}\n2147483648"),
            format!("{GPU}\n1\n2"),
            "GPU-other\n900000".into(),
        ] {
            assert_eq!(parse(text.as_bytes(), GPU), Err(SensorStatus::Invalid));
        }
        assert_eq!(
            parse(format!("{GPU}\nN/A").as_bytes(), GPU),
            Err(SensorStatus::Unavailable)
        );
        assert_eq!(
            parse(format!("{GPU}\n").as_bytes(), GPU),
            Err(SensorStatus::Unavailable)
        );
    }
    fn fixture(script: &str) -> (tempfile::TempDir, NvidiaVoltage) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("query");
        std::fs::write(&executable, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let provider = NvidiaVoltage {
            executable,
            display: ":fixture".into(),
            authority: dir.path().join("authority"),
            children: BTreeMap::new(),
        };
        (dir, provider)
    }
    #[tokio::test]
    async fn query_only_explicit_session_and_uuid_then_recovery() {
        let (dir, mut provider) = fixture(&format!(
            r#"[ "$1" = --no-config ] && [ "$2" = --terse ] && [ "$3" = --ctrl-display ] && [ "$4" = :fixture ] || exit 1
[ "$5" = --query ] && [ "$6" = '[gpu:{GPU}]/GpuUUID' ] || exit 1
[ "$7" = --query ] && [ "$8" = '[gpu:{GPU}]/GPUCurrentCoreVoltage' ] && [ "$#" = 8 ] || exit 1
[ "$LC_ALL" = C ] && [ "$DISPLAY" = :fixture ] && [ "$XAUTHORITY" = "$(dirname "$0")/authority" ] || exit 1
printf '%s\n' '{GPU}' 900000"#
        ));
        assert_eq!(provider.query(GPU).await, Ok(0.9));
        std::fs::write(&provider.executable, "#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(provider.query(GPU).await, Err(SensorStatus::Unavailable));
        std::fs::write(
            dir.path().join("query"),
            format!("#!/bin/sh\nprintf '%s\\n' '{GPU}' 0\n"),
        )
        .unwrap();
        assert_eq!(provider.query(GPU).await, Ok(0.0));
    }
    #[tokio::test]
    async fn bounded_output_timeout_reaps_child() {
        let (dir, mut provider) = fixture("echo $$ > \"$0.pid\"; exec sleep 10");
        assert_eq!(provider.query(GPU).await, Err(SensorStatus::Unavailable));
        let pid = std::fs::read_to_string(dir.path().join("query.pid")).unwrap();
        assert!(!std::path::Path::new(&format!("/proc/{}", pid.trim())).exists());
        std::fs::write(
            &provider.executable,
            "#!/bin/sh\nexec head -c 4097 /dev/zero\n",
        )
        .unwrap();
        assert_eq!(provider.query(GPU).await, Err(SensorStatus::Invalid));
    }
    #[tokio::test]
    async fn disappeared_gpu_quarantine_bounds_helpers_and_recovers_new_uuid() {
        use std::time::Duration;
        let (dir, mut provider) = fixture(&format!(
            "echo started > \"$0.started\"; printf '%s\\n' '{GPU}' 975000"
        ));
        // Model32 unreaped children, all belonging to GPUs no longer discovered.
        // This is not a simulation of uninterruptible kernel I/O: admission is
        // tested with live owned children before normal SIGKILL/reap recovery.
        for index in 0..32 {
            provider.children.insert(
                format!("old-gpu-{index}"),
                Some(
                    Command::new("sleep")
                        .arg("10")
                        .kill_on_drop(true)
                        .spawn()
                        .unwrap(),
                ),
            );
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(100), provider.query(GPU))
                .await
                .unwrap(),
            Err(SensorStatus::Unavailable)
        );
        assert_eq!(provider.children.len(), 32);
        assert!(!dir.path().join("query.started").exists());
        for child in provider.children.values_mut().flatten() {
            tokio::time::timeout(Duration::from_secs(2), child.wait())
                .await
                .unwrap()
                .unwrap();
        }
        assert_eq!(provider.query(GPU).await, Ok(0.975));
        assert_eq!(provider.children.len(), 1);
        assert!(provider.children[GPU].is_none());
        assert!(dir.path().join("query.started").exists());
    }
    #[tokio::test]
    async fn quarantined_gpu_does_not_block_other_uuid_or_mix_readings() {
        let other = "GPU-abcdef12-1234-1234-1234-123456789abc";
        let (_dir, mut provider) = fixture(&format!(
            r#"case "$6" in
'[gpu:{GPU}]/GpuUUID') printf '%s\n' '{GPU}' 975000 ;;
'[gpu:{other}]/GpuUUID') printf '%s\n' '{other}' 850000 ;;
*) exit 1 ;;
esac"#
        ));
        provider.children.insert(
            GPU.into(),
            Some(
                Command::new("sleep")
                    .arg("10")
                    .kill_on_drop(true)
                    .spawn()
                    .unwrap(),
            ),
        );
        let inputs = [GPU, other].map(|uuid| {
            reading(
                format!("nvidia:{uuid}:temperature"),
                "fixture",
                SensorUnit::Celsius,
                Ok(40.0),
                1,
            )
        });
        let rows = provider.collect(&inputs, 1).await;
        assert_eq!(
            rows.iter()
                .find(|r| r.id == format!("nvidia:{GPU}:core_voltage"))
                .unwrap()
                .status,
            SensorStatus::Unavailable
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.id == format!("nvidia:{other}:core_voltage"))
                .unwrap()
                .value,
            Some(0.85)
        );
        provider
            .children
            .get_mut(GPU)
            .unwrap()
            .as_mut()
            .unwrap()
            .wait()
            .await
            .unwrap();
        let rows = provider
            .collect(&[inputs[1].clone(), inputs[0].clone()], 2)
            .await;
        assert_eq!(
            rows.iter()
                .find(|r| r.id == format!("nvidia:{GPU}:core_voltage"))
                .unwrap()
                .value,
            Some(0.975)
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.id == format!("nvidia:{other}:core_voltage"))
                .unwrap()
                .value,
            Some(0.85)
        );
    }
}
