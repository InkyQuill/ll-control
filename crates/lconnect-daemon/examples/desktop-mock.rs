//! Isolated native desktop fixture. Never constructs a real HID backend.
#[path = "desktop-mock/archive_portal.rs"]
mod archive_portal;
#[path = "desktop-mock/cooling_modes.rs"]
mod cooling_modes;
#[path = "desktop-mock/firmware_recovery.rs"]
mod firmware_recovery;
#[path = "desktop-mock/lcd_frames.rs"]
mod lcd_frames;
#[path = "desktop-mock/tray.rs"]
mod tray;
use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, sensors::SensorCache, stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use lconnect_ipc::{server::IpcServer, SensorReading, SensorStatus, SensorUnit};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
// Fault injection belongs only to this isolated executable, never the shared backend.
#[derive(Default)]
struct FaultCounts {
    pump_rgb: AtomicU64,
    lcd: AtomicU64,
    fan_pwm: AtomicU64,
}
struct FaultBackend {
    mock: MockBackend,
    root: PathBuf,
    failures: Arc<FaultCounts>,
}
struct FaultHandle {
    inner: Box<dyn OpenHandle>,
    mock: MockBackend,
    info: DeviceInfo,
    root: PathBuf,
    failures: Arc<FaultCounts>,
}
impl HidBackend for FaultBackend {
    fn enumerate(&self) -> lconnect_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> lconnect_hid::Result<Box<dyn OpenHandle>> {
        Ok(Box::new(FaultHandle {
            inner: self.mock.open(info)?,
            mock: self.mock.clone(),
            info: info.clone(),
            root: self.root.clone(),
            failures: self.failures.clone(),
        }))
    }
}
impl OpenHandle for FaultHandle {
    fn write(&self, buf: &[u8]) -> lconnect_hid::Result<()> {
        // Exact report/command pairs prevent the same byte in another framing from faulting.
        let fault = match (buf.first(), buf.get(1)) {
            (Some(1), Some(131)) => Some(("fail-pump-rgb", &self.failures.pump_rgb)),
            (Some(2), Some(14)) => Some(("fail-lcd", &self.failures.lcd)),
            (Some(1), Some(139)) => Some(("fail-fan-pwm", &self.failures.fan_pwm)),
            _ => None,
        };
        if let Some((marker, count)) = fault {
            if self.root.join(marker).is_file() {
                count.fetch_add(1, Ordering::Relaxed);
                return Err(lconnect_hid::Error::ShortWrite {
                    written: 0,
                    expected: buf.len(),
                });
            }
        }
        self.inner.write(buf)?;
        // Queue only after the request: the transport drains stale input before
        // writing. This fixture models the PN0 gate used by saved-media playback.
        if buf == lconnect_protocol::galahad2_lcdv2::firmware::build_request().as_slice() {
            let response = lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
                134,
                b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
            );
            self.mock
                .queue_response(&self.info, response.as_bytes().to_vec());
            if std::env::var("LCONNECT_NATIVE_FIRMWARE_DATE").as_deref() == Ok("1") {
                let mut date =
                    lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
                        134,
                        b"Apr 12 2024 14:33:21",
                    )
                    .as_bytes()
                    .to_vec();
                date[4] = 1;
                self.mock.queue_response(&self.info, date);
            }
        }
        Ok(())
    }
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> lconnect_hid::Result<usize> {
        self.inner.read_timeout(buf, timeout_ms)
    }
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(std::env::var("LCONNECT_SMOKE_ROOT")?);
    anyhow::ensure!(
        root.is_absolute() && root.join("fixture-marker").is_file(),
        "isolated fixture root required"
    );
    let _archive_portal = if std::env::var("LCONNECT_NATIVE_ARCHIVE_PORTAL").as_deref() == Ok("1") {
        Some(archive_portal::start(root.clone()).await?)
    } else {
        None
    };
    let _tray = if std::env::var("LCONNECT_NATIVE_TRAY").as_deref() == Ok("1") {
        Some(tray::start(root.clone()).await?)
    } else {
        None
    };
    let store = Arc::new(PersistenceStore::new_with_root(root.join("settings")));
    let mock = MockBackend::new();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("native-smoke".into()),
        PathBuf::from("/mock/native-smoke"),
    );
    firmware_recovery::seed(&root, &store, &info).await?;
    cooling_modes::seed(&root, &store, &info)?;
    if std::env::var("LCONNECT_NATIVE_SPEC").as_deref() != Ok("1") {
        mock.add_device(info.clone());
    }
    let cache = SensorCache::default();
    let stats = Arc::new(Stats::new());
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let failures = Arc::new(FaultCounts::default());
    let manager = DeviceManager::new(
        Arc::new(FaultBackend {
            mock: mock.clone(),
            root: root.clone(),
            failures: failures.clone(),
        }),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .with_sensors(cache.clone())
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), cfg, store)
        .with_sensors(cache.clone())
        .with_system_spec(lconnect_daemon::system_spec::LinuxSpec {
            proc_root: root.join("spec/proc"),
            sys_root: root.join("spec/sys"),
            pci_ids: vec![root.join("spec/pci.ids")],
        });
    let socket = root.join("lconnectd.sock");
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&socket).await });
    let mut sample = 0;
    let export_frames = std::env::var("LCONNECT_MOCK_EXPORT_FRAMES").as_deref() == Ok("1");
    let mut frames = lcd_frames::Frames::default();
    let export_video = std::env::var("LCONNECT_MOCK_EXPORT_VIDEO").as_deref() == Ok("1");
    let mut video = lcd_frames::Video::default();
    while !root.join("stop").exists() {
        let cpu_unavailable = tokio::fs::try_exists(root.join("cpu-unavailable")).await?;
        let weather_cpu_load = tokio::fs::try_exists(root.join("weather-cpu-load")).await?;
        let cpu_hot = tokio::fs::try_exists(root.join("cpu-hot")).await?;
        // Pause only synthetic publication: actors and IPC keep running as samples age.
        if !tokio::fs::try_exists(root.join("freeze-sensors")).await? {
            cache.publish(
                Instant::now(),
                vec![
                    SensorReading {
                        id: "fixture:cpu".into(),
                        label: "Fixture CPU".into(),
                        unit: SensorUnit::Celsius,
                        value: Some(if cpu_hot {
                            95.0
                        } else {
                            40.0 + (sample % 10) as f64
                        }),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "amdgpu:2f61:utilization".into(),
                        label: "Fixture GPU A load".into(),
                        unit: SensorUnit::Percent,
                        value: Some(15.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "amdgpu:2f62:utilization".into(),
                        label: "Fixture GPU B load".into(),
                        unit: SensorUnit::Percent,
                        value: Some(35.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "intel:2f60:i915:engine:0:utilization".into(),
                        label: "Fixture Intel i915 render engine".into(),
                        unit: SensorUnit::Percent,
                        value: Some(27.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "intel-memory:2f60:i915:vram_used".into(),
                        label: "Fixture Intel i915 VRAM used".into(),
                        unit: SensorUnit::Bytes,
                        value: Some(1073741824.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "intel:2f65:xe:gt0:class0:instance0:utilization".into(),
                        label: "Fixture Intel xe GT0 render engine".into(),
                        unit: SensorUnit::Percent,
                        value: Some(47.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "intel-memory:2f65:xe:vram_used".into(),
                        label: "Fixture Intel xe VRAM used".into(),
                        unit: SensorUnit::Bytes,
                        value: Some(2147483648.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "hwmon:2f64:nct6775:fan1".into(),
                        label: "Fixture motherboard fan header".into(),
                        unit: SensorUnit::Rpm,
                        value: Some(1234.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                    SensorReading {
                        id: "hwmon:2f63:coretemp:temp1".into(),
                        label: "Fixture CPU default".into(),
                        unit: SensorUnit::Celsius,
                        value: Some(45.0),
                        status: SensorStatus::Valid,
                        observed_unix_ms: Some(sample),
                        age_ms: Some(0),
                    },
                ]
                .into_iter()
                .chain(weather_cpu_load.then(|| SensorReading {
                    id: "proc:cpu:utilization".into(),
                    label: "Fixture CPU load".into(),
                    unit: SensorUnit::Percent,
                    value: Some(45.0),
                    status: SensorStatus::Valid,
                    observed_unix_ms: Some(sample),
                    age_ms: Some(0),
                }))
                .filter(|reading| !cpu_unavailable || reading.id != "fixture:cpu")
                .collect(),
            );
        }
        let mut snapshot = serde_json::to_value(stats.snapshot(manager.list().await.len() as u32))?;
        snapshot["pump_rgb_failures"] =
            serde_json::json!(failures.pump_rgb.load(Ordering::Relaxed));
        snapshot["lcd_failures"] = serde_json::json!(failures.lcd.load(Ordering::Relaxed));
        snapshot["fan_pwm_failures"] = serde_json::json!(failures.fan_pwm.load(Ordering::Relaxed));
        let rgb_history = mock.write_history(&info);
        if export_frames {
            frames.observe(&rgb_history, &root)?;
        }
        if export_video {
            video.observe(&rgb_history, &root)?;
        }
        snapshot["firmware_queries"] = serde_json::json!(rgb_history
            .iter()
            .filter(|packet| packet.starts_with(&[1, 134]))
            .count());
        snapshot["firmware_update_writes"] = serde_json::json!(rgb_history
            .iter()
            .filter(|packet| packet.starts_with(&[2, 11]))
            .count());
        snapshot["firmware_activations"] = serde_json::json!(rgb_history
            .iter()
            .filter(|packet| packet.starts_with(&[2, 10]))
            .count());
        snapshot["last_pump_rgb_packet"] = serde_json::json!(rgb_history
            .iter()
            .rev()
            .find(|p| p.len() > 1 && p[1] == 131));
        snapshot["last_fan_rgb_packet"] = serde_json::json!(rgb_history
            .iter()
            .rev()
            .find(|p| p.len() > 1 && p[1] == 133));
        snapshot["last_pump_pwm_packet"] = serde_json::json!(rgb_history
            .iter()
            .rev()
            .find(|p| p.first() == Some(&1) && p.get(1) == Some(&138)));
        snapshot["pump_pwm_writes"] = serde_json::json!(rgb_history
            .iter()
            .filter(|p| p.first() == Some(&1) && p.get(1) == Some(&138))
            .count());
        snapshot["pump_rgb_writes"] = serde_json::json!(mock
            .write_history(&info)
            .iter()
            .filter(|packet| packet.first() == Some(&1) && packet.get(1) == Some(&131))
            .count());
        snapshot["fan_pwm_writes"] = serde_json::json!(mock
            .write_history(&info)
            .iter()
            .filter(|packet| packet.first() == Some(&1) && packet.get(1) == Some(&139))
            .count());
        snapshot["lcd_video_packets"] = serde_json::json!(mock
            .write_history(&info)
            .iter()
            .filter(|packet| packet.get(1) == Some(&13))
            .count());
        snapshot["jpeg_starts"] = serde_json::json!(mock
            .write_history(&info)
            .iter()
            .filter(|packet| packet.get(11..13) == Some(&[255, 216]))
            .count());
        std::fs::write(root.join("stats.tmp"), serde_json::to_vec(&snapshot)?)?;
        std::fs::rename(root.join("stats.tmp"), root.join("stats.json"))?;
        sample += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    manager.shutdown().await;
    server.abort();
    let _ = server.await;
    Ok(())
}
