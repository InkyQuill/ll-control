//! Process-isolated Intel dedicated memory, never on the HID/IPC queue.
use super::query_process::QueryProcess;
use super::reading;
use llcontrol_intel_memory::{Device, QueryError, MAX_DEVICES};
use llcontrol_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{collections::BTreeSet, path::PathBuf};
const LIMIT: usize = 65_536;
pub(super) struct IntelMemory {
    process: QueryProcess,
    previous: Vec<SensorReading>,
}
impl IntelMemory {
    pub(super) fn new(executable: PathBuf) -> Self {
        Self {
            process: QueryProcess::new(executable, "--query-intel-memory", LIMIT),
            previous: Vec::new(),
        }
    }
    pub(super) async fn collect(&mut self, now: u64) -> Vec<SensorReading> {
        match self
            .process
            .query()
            .await
            .and_then(|bytes| parse(&bytes, now))
        {
            Ok(rows) => {
                self.previous = rows.clone();
                rows
            }
            Err(status) => {
                let mut rows: Vec<_> = self
                    .previous
                    .iter()
                    .map(|r| reading(&r.id, &r.label, r.unit, Err(status), now))
                    .collect();
                rows.push(reading(
                    "intel-memory:discovery",
                    "Intel VRAM query unavailable",
                    SensorUnit::Bytes,
                    Err(status),
                    now,
                ));
                rows
            }
        }
    }
}

fn parse(bytes: &[u8], now: u64) -> Result<Vec<SensorReading>, SensorStatus> {
    if bytes.len() > LIMIT {
        return Err(SensorStatus::Invalid);
    }
    let devices: Vec<Device> = serde_json::from_slice(bytes).map_err(|_| SensorStatus::Invalid)?;
    if devices.len() > MAX_DEVICES {
        return Err(SensorStatus::Invalid);
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for device in devices {
        let physical = &device.physical;
        if !physical.starts_with("2f")
            || physical.len() > 900
            || !physical.len().is_multiple_of(2)
            || !physical
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !seen.insert(physical.clone())
        {
            return Err(SensorStatus::Invalid);
        }
        let memory = device.memory.map_err(|e| match e {
            QueryError::Unavailable => SensorStatus::Unavailable,
            QueryError::Invalid => SensorStatus::Invalid,
        });
        if memory
            .as_ref()
            .is_ok_and(|m| m.total == 0 || m.used.is_some_and(|u| u == 0 || u > m.total))
        {
            return Err(SensorStatus::Invalid);
        }
        let total = memory.as_ref().map(|m| m.total as f64).map_err(|s| *s);
        let used = memory
            .as_ref()
            .map_err(|s| *s)
            .and_then(|m| m.used.map(|u| u as f64).ok_or(SensorStatus::Unavailable));
        let percent = used.and_then(|u| total.map(|t| 100.0 * u / t));
        let prefix = format!("intel-memory:{physical}:{}", device.driver.name());
        for (metric, unit, value) in [
            ("vram_total", SensorUnit::Bytes, total),
            ("vram_used", SensorUnit::Bytes, used),
            ("vram_used_percent", SensorUnit::Percent, percent),
        ] {
            rows.push(reading(
                format!("{prefix}:{metric}"),
                format!("{} dedicated memory: {metric}", device.driver.name()),
                unit,
                value,
                now,
            ));
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use llcontrol_intel_memory::{Driver, Memory};
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;
    use tokio::process::Command;
    fn rows() -> Vec<Device> {
        vec![Device {
            physical: "2f7379732f67707531".into(),
            driver: Driver::Xe,
            memory: Ok(Memory {
                total: 4294967296,
                used: Some(2147483648),
            }),
        }]
    }
    fn encoded(rows: &[Device]) -> Vec<u8> {
        serde_json::to_vec(rows).unwrap()
    }
    fn fixture(body: &str) -> (tempfile::TempDir, IntelMemory) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("helper");
        std::fs::write(&exe, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        (dir, IntelMemory::new(exe))
    }
    fn success() -> String {
        format!(
            "[ \"$*\" = --query-intel-memory ] || exit 1\nprintf '%s' '{}'",
            String::from_utf8(encoded(&rows())).unwrap()
        )
    }
    #[test]
    fn usage_identity_selection_and_unavailable_without_fake_zero() {
        let readings = parse(&encoded(&rows()), 1).unwrap();
        assert_eq!(
            readings
                .iter()
                .find(|r| r.id.ends_with(":vram_used"))
                .unwrap()
                .value,
            Some(2147483648.0)
        );
        let candidates = crate::sensor_selection::candidates(&readings);
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].role,
            llcontrol_ipc::DefaultSensorRole::GpuMemoryUsed
        );
        assert_eq!(
            candidates[0].gpu_id.as_deref(),
            Some("sysfs:2f7379732f67707531:xe")
        );
        let defaults = crate::sensor_selection::recommended(&readings, 0);
        crate::sensor_selection::validate_selection(&defaults, &Default::default(), &readings)
            .unwrap();
        let mut devices = rows();
        devices[0].memory.as_mut().unwrap().used = None;
        let unknown = parse(&encoded(&devices), 2).unwrap();
        assert_eq!(unknown.iter().filter(|r| r.value.is_some()).count(), 1);
        assert!(unknown
            .iter()
            .filter(|r| !r.id.ends_with(":vram_total"))
            .all(|r| r.status == SensorStatus::Unavailable && r.observed_unix_ms.is_none()));
        assert_eq!(
            readings.iter().map(|r| &r.id).collect::<Vec<_>>(),
            unknown.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
        devices[0].memory = Err(QueryError::Unavailable);
        assert!(parse(&encoded(&devices), 3)
            .unwrap()
            .iter()
            .all(|r| r.value.is_none()));
    }
    #[test]
    fn helper_contract_rejects_duplicate_invalid_and_oversized_data() {
        let mut duplicate = rows();
        duplicate.extend(rows());
        assert!(parse(&encoded(&duplicate), 1).is_err());
        for memory in [
            Memory {
                total: 0,
                used: None,
            },
            Memory {
                total: 100,
                used: Some(101),
            },
            Memory {
                total: 100,
                used: Some(0),
            },
        ] {
            let mut devices = rows();
            devices[0].memory = Ok(memory);
            assert!(parse(&encoded(&devices), 1).is_err());
        }
        assert!(parse(&vec![b' '; LIMIT + 1], 1).is_err());
        assert!(parse(b"null", 1).is_err());
        let mut devices = rows();
        devices[0].physical = "card0".into();
        assert!(parse(&encoded(&devices), 1).is_err());
    }
    #[tokio::test]
    async fn helper_failure_revokes_prior_values_then_same_identity_recovers() {
        let (dir, mut provider) = fixture(&success());
        let first = provider.collect(1).await;
        assert_eq!(first.len(), 3);
        assert!(first.iter().all(|r| r.status == SensorStatus::Valid));
        std::fs::write(dir.path().join("helper"), "#!/bin/sh\nexit 1\n").unwrap();
        assert!(provider.collect(2).await.iter().all(|r| r.value.is_none()));
        std::fs::write(
            dir.path().join("helper"),
            format!("#!/bin/sh\n{}\n", success()),
        )
        .unwrap();
        let recovered = provider.collect(3).await;
        assert_eq!(
            first.iter().map(|r| &r.id).collect::<Vec<_>>(),
            recovered.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
        assert!(recovered.iter().all(|r| r.status == SensorStatus::Valid));
        std::fs::write(dir.path().join("helper"), "#!/bin/sh\nprintf '[]'\n").unwrap();
        assert!(provider.collect(4).await.is_empty());
    }
    #[tokio::test]
    async fn helper_timeout_reaps_and_quarantined_child_prevents_respawn() {
        let (dir, mut provider) = fixture("echo $$ > \"$0.pid\"; exec sleep 10");
        let at = std::time::Instant::now();
        assert_eq!(
            provider.collect(1).await[0].status,
            SensorStatus::Unavailable
        );
        assert!(at.elapsed() < Duration::from_secs(2));
        let pid = std::fs::read_to_string(dir.path().join("helper.pid")).unwrap();
        assert!(!std::path::Path::new(&format!("/proc/{}", pid.trim())).exists());
        std::fs::write(
            dir.path().join("helper"),
            "#!/bin/sh\necho started > \"$0.started\"\n",
        )
        .unwrap();
        provider.process.draining = Some(
            Command::new("sleep")
                .arg("10")
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
        );
        assert_eq!(
            provider.collect(2).await[0].status,
            SensorStatus::Unavailable
        );
        assert!(!dir.path().join("helper.started").exists());
        // Admission already requests termination of the retained helper.
        provider
            .process
            .draining
            .take()
            .unwrap()
            .wait()
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn memory_reaches_cache_and_lcd_without_host_access() {
        use super::super::{spawn, LinuxSensors, SensorCache};
        let (dir, memory) = fixture(&success());
        let mut provider = LinuxSensors::new(dir.path().join("hwmon"), dir.path().join("proc"));
        provider.intel_memory = Some(memory);
        let cache = SensorCache::default();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let task = spawn(provider, cache.clone(), rx);
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let readings = cache.readings();
                if let Some(row) = readings.iter().find(|r| r.id.ends_with(":vram_used")) {
                    assert_eq!(
                        llcontrol_ipc::theme_sensor_text(
                            &readings,
                            &row.id,
                            SensorUnit::Bytes,
                            llcontrol_ipc::TemperatureUnit::Celsius,
                            1
                        ),
                        "2.0 GiB"
                    );
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();

        // RPC reads only the published cache, even after the helper disappears.
        std::fs::remove_file(dir.path().join("helper")).unwrap();
        use crate::{
            config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
            persistence::PersistenceStore, stats::Stats,
        };
        use llcontrol_ipc::{client::IpcClient, server::IpcServer, transport_memory::mem_pair};
        use std::sync::Arc;
        let store = Arc::new(PersistenceStore::new_with_root(dir.path().join("settings")));
        let stats = Arc::new(Stats::new());
        let manager = DeviceManager::new(
            Arc::new(llcontrol_hid::MockBackend::new()),
            store.clone(),
            Config::default(),
            stats.clone(),
        )
        .spawn()
        .await;
        let rpc = IpcServerImpl::new(manager.clone(), stats, Config::default(), store)
            .with_sensors(cache.clone());
        let (a, b) = mem_pair();
        let server =
            tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        let readings: Vec<SensorReading> = serde_json::from_value(
            client
                .call("list_sensors", serde_json::json!({}))
                .await
                .unwrap(),
        )
        .unwrap();
        let row = readings
            .iter()
            .find(|r| r.id.ends_with(":vram_used"))
            .unwrap();
        assert_eq!(row.value, Some(2147483648.0));
        let theme = serde_json::json!({"version":1,"name":"Intel memory","background":[0,0,0],"layers":[{
            "kind":"sensor","sensor_id":row.id,"label":"VRAM","unit":"bytes","temperature_unit":"celsius","decimals":1,
            "style":{"x":240,"y":240,"size":25,"color":[255,255,255],"align":"center"}
        }]});
        let preview = client
            .call(
                "preview_lcd_theme",
                serde_json::json!({"dev":null,"theme":theme}),
            )
            .await
            .unwrap();
        let jpeg: Vec<u8> = serde_json::from_value(preview.clone()).unwrap();
        assert!(jpeg.starts_with(&[255, 216]));
        cache.publish(std::time::Instant::now() - Duration::from_secs(4), readings);
        let stale: Vec<SensorReading> = serde_json::from_value(
            client
                .call("list_sensors", serde_json::json!({}))
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(stale
            .iter()
            .filter(|r| r.id.starts_with("intel-memory:"))
            .all(|r| r.status == SensorStatus::Stale && r.value.is_none()));
        let unavailable = client
            .call(
                "preview_lcd_theme",
                serde_json::json!({"dev":null,"theme":theme}),
            )
            .await
            .unwrap();
        assert_ne!(
            preview, unavailable,
            "stale VRAM must not render the old measurement"
        );
        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
    }
}
