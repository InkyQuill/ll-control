use lconnect_daemon::sensors::{LinuxSensors, SensorCache};
use lconnect_ipc::{SensorStatus, SensorUnit};
use std::{
    fs,
    os::unix::fs::symlink,
    time::{Duration, Instant},
};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
}
impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: TempDir::new().unwrap(),
        };
        fs::create_dir_all(f.dir.path().join("hwmon/hwmon0")).unwrap();
        fs::create_dir_all(f.dir.path().join("devices/cpu")).unwrap();
        fs::create_dir_all(f.dir.path().join("proc")).unwrap();
        symlink(
            f.dir.path().join("devices/cpu"),
            f.dir.path().join("hwmon/hwmon0/device"),
        )
        .unwrap();
        f.write("hwmon/hwmon0/name", "k10temp\n");
        f.write("hwmon/hwmon0/temp1_input", "42000\n");
        f.write("hwmon/hwmon0/temp1_label", "Tctl\n");
        f.write("proc/stat", "cpu 10 0 10 80 0 0 0 0 5 0\n");
        f.write("proc/meminfo", "MemTotal: 1000 kB\nMemAvailable: 250 kB\n");
        f
    }
    fn write(&self, p: &str, v: &str) {
        fs::write(self.dir.path().join(p), v).unwrap();
    }
    fn provider(&self) -> LinuxSensors {
        LinuxSensors::new(self.dir.path().join("hwmon"), self.dir.path().join("proc"))
    }
}

#[test]
fn physical_identity_survives_hwmon_renumbering_and_disappearance_is_not_zero() {
    let f = Fixture::new();
    let mut p = f.provider();
    let first = p
        .collect()
        .into_iter()
        .find(|r| r.unit == SensorUnit::Celsius)
        .unwrap();
    assert_eq!(first.value, Some(42.0));
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon37"),
    )
    .unwrap();
    let second = p
        .collect()
        .into_iter()
        .find(|r| r.unit == SensorUnit::Celsius)
        .unwrap();
    assert_eq!(first.id, second.id);
    fs::remove_file(f.dir.path().join("hwmon/hwmon37/temp1_input")).unwrap();
    assert!(!p.collect().iter().any(|r| r.id == first.id));
}

#[test]
fn failed_faulted_and_disabled_temperatures_never_return_old_or_zero_values() {
    let f = Fixture::new();
    let mut p = f.provider();
    for (input, flag, expected) in [
        ("not-a-number", None, SensorStatus::Invalid),
        ("42000", Some(("fault", "1")), SensorStatus::Unavailable),
        ("42000", Some(("enable", "0")), SensorStatus::Unavailable),
    ] {
        for suffix in ["fault", "enable"] {
            let _ = fs::remove_file(f.dir.path().join(format!("hwmon/hwmon0/temp1_{suffix}")));
        }
        f.write("hwmon/hwmon0/temp1_input", input);
        if let Some((suffix, value)) = flag {
            f.write(&format!("hwmon/hwmon0/temp1_{suffix}"), value);
        }
        let row = p
            .collect()
            .into_iter()
            .find(|r| r.unit == SensorUnit::Celsius)
            .unwrap();
        assert_eq!(row.status, expected);
        assert_eq!(row.value, None);
        assert_eq!(row.observed_unix_ms, None);
    }
}

#[test]
fn utilization_uses_counter_deltas_without_guest_double_count_and_resets_warmup() {
    let f = Fixture::new();
    let mut p = f.provider();
    let get = |rows: Vec<lconnect_ipc::SensorReading>| {
        rows.into_iter()
            .find(|r| r.id == "proc:cpu:utilization")
            .unwrap()
    };
    assert_eq!(get(p.collect()).status, SensorStatus::Unavailable);
    f.write("proc/stat", "cpu 30 0 10 160 0 0 0 0 25 0\n");
    assert_eq!(get(p.collect()).value, Some(20.0));
    f.write("proc/stat", "cpu 1 0 0 0 0 0 0 0\n");
    assert_eq!(get(p.collect()).value, None);
    f.write("proc/stat", "broken\n");
    assert_eq!(get(p.collect()).status, SensorStatus::Invalid);
    f.write("proc/stat", "cpu 30 0 10 160 0 0 0 0\n");
    assert_eq!(get(p.collect()).status, SensorStatus::Unavailable);
}

#[test]
fn memory_units_and_malformed_available_are_explicit() {
    let f = Fixture::new();
    let mut p = f.provider();
    let get = |rows: Vec<lconnect_ipc::SensorReading>| {
        rows.into_iter()
            .find(|r| r.id == "proc:memory:used")
            .unwrap()
    };
    assert_eq!(get(p.collect()).value, Some(750.0 * 1024.0));
    for value in [
        "MemTotal: 1000 kB\n",
        "MemTotal: 1000 kB\nMemAvailable: 1001 kB\n",
        "MemTotal: 1000 MB\nMemAvailable: 250 kB\n",
    ] {
        f.write("proc/meminfo", value);
        let row = get(p.collect());
        assert_eq!(row.status, SensorStatus::Invalid);
        assert_eq!(row.value, None);
    }
}

#[test]
fn stale_cache_keeps_identity_and_timestamp_but_revokes_value() {
    let f = Fixture::new();
    let rows = f.provider().collect();
    let cache = SensorCache::default();
    cache.publish(Instant::now() - Duration::from_secs(4), rows.clone());
    let cached = cache.readings();
    for original in rows.iter().filter(|r| r.status == SensorStatus::Valid) {
        let row = cached.iter().find(|r| r.id == original.id).unwrap();
        assert_eq!(row.status, SensorStatus::Stale);
        assert_eq!(row.value, None);
        assert_eq!(row.observed_unix_ms, original.observed_unix_ms);
        assert!(row.age_ms.unwrap() >= 4000);
    }
}

#[tokio::test]
async fn collector_publishes_and_joins_when_stop_sender_is_dropped() {
    let f = Fixture::new();
    let cache = SensorCache::default();
    let (tx, rx) = tokio::sync::watch::channel(false);
    let task = lconnect_daemon::sensors::spawn(f.provider(), cache.clone(), rx);
    tokio::time::timeout(Duration::from_secs(2), async {
        while cache.readings().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(tx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn catalog_roundtrip_without_any_usb_device_uses_cache() {
    use lconnect_daemon::{
        config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
        persistence::PersistenceStore, stats::Stats,
    };
    use lconnect_ipc::{client::IpcClient, server::IpcServer, transport_memory::mem_pair};
    use std::sync::Arc;
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "amdgpu");
    f.write("hwmon/hwmon0/power1_input", "125500000");
    f.write("hwmon/hwmon0/in0_input", "975");
    f.write("hwmon/hwmon0/freq1_input", "1500000000");
    let cpu_root = f.dir.path().join("cpufreq");
    fs::create_dir_all(cpu_root.join("policy4")).unwrap();
    fs::write(cpu_root.join("policy4/related_cpus"), "4 5").unwrap();
    fs::write(cpu_root.join("policy4/cpuinfo_cur_freq"), "4200000").unwrap();
    let drm = f.dir.path().join("drm");
    let gpu = f.dir.path().join("devices/intel-gpu");
    let driver = f.dir.path().join("drivers/i915");
    fs::create_dir_all(&gpu).unwrap();
    fs::create_dir_all(&driver).unwrap();
    fs::create_dir_all(drm.join("card0/gt/gt0")).unwrap();
    symlink(&gpu, drm.join("card0/device")).unwrap();
    symlink(driver, gpu.join("driver")).unwrap();
    fs::write(drm.join("card0/gt/gt0/rps_act_freq_mhz"), "900").unwrap();
    fs::write(drm.join("card0/gt/gt0/rps_cur_freq_mhz"), "1100").unwrap();
    for (index, driver, channel) in [
        (1, "i915", "in0"),
        (2, "xe", "in1"),
        (3, "asusec", "in0"),
        (4, "asus_wmi_sensors", "in4"),
        (5, "nct6798", "in0"),
        (6, "nouveau", "in0"),
        (7, "zenpower", "in1"),
    ] {
        let chip = f.dir.path().join(format!("hwmon/hwmon{index}"));
        let physical = f.dir.path().join(format!("devices/voltage-{driver}"));
        fs::create_dir_all(&chip).unwrap();
        fs::create_dir_all(&physical).unwrap();
        symlink(physical, chip.join("device")).unwrap();
        fs::write(chip.join("name"), driver).unwrap();
        fs::write(chip.join(format!("{channel}_input")), "900").unwrap();
        if driver == "zenpower" {
            fs::write(chip.join("in1_label"), "SVI2_Core").unwrap();
        }
        if driver == "asus_wmi_sensors" {
            fs::write(chip.join("in4_label"), "CPU Core Voltage").unwrap();
        }
    }
    let log = f.dir.path().join("game.csv");
    fs::write(&log,"fps,frametime,cpu_load,cpu_power,gpu_load,cpu_temp,gpu_temp,gpu_core_clock,gpu_mem_clock,gpu_vram_used,gpu_power,ram_used,swap_used,process_rss,cpu_mhz,elapsed\n").unwrap();
    let mut provider = f
        .provider()
        .with_asus_wmi_voltage()
        .with_cpufreq_root(cpu_root)
        .with_intel_frequency_root(drm)
        .with_mangohud_per_frame_log(&log);
    provider.collect();
    {
        use std::io::Write;
        let mut file = fs::OpenOptions::new().append(true).open(&log).unwrap();
        writeln!(file, "100,10,0,0,0,0,0,0,0,0,0,0,0,0,0,10000000").unwrap();
        writeln!(file, "33.333333,30,0,0,0,0,0,0,0,0,0,0,0,0,0,40000000").unwrap();
    }
    let cache = SensorCache::default();
    cache.publish(Instant::now(), provider.collect());
    let stats = Arc::new(Stats::new());
    let store = Arc::new(PersistenceStore::new_with_root(
        f.dir.path().join("settings"),
    ));
    let manager = DeviceManager::new(
        Arc::new(lconnect_hid::MockBackend::new()),
        store.clone(),
        Config::default(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, Config::default(), store)
        .with_sensors(cache.clone());
    // Removing files after publishing proves the RPC does not perform I/O.
    fs::remove_dir_all(f.dir.path().join("proc")).unwrap();
    let (a, b) = mem_pair();
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let value = client
        .call("list_sensors", serde_json::json!({}))
        .await
        .unwrap();
    let rows: Vec<lconnect_ipc::SensorReading> = serde_json::from_value(value).unwrap();
    assert!(rows
        .iter()
        .any(|r| r.id == "proc:memory:used" && r.value == Some(768000.0)));
    let cpu_clock = rows
        .iter()
        .find(|r| r.id == "cpufreq:cpus-4.5:cpuinfo_cur_freq")
        .unwrap();
    assert_eq!(cpu_clock.value, Some(4_200_000_000.0));
    assert_eq!(
        lconnect_ipc::theme_sensor_text(
            &rows,
            &cpu_clock.id,
            SensorUnit::Hertz,
            lconnect_ipc::TemperatureUnit::Celsius,
            1
        ),
        "4200.0 MHz"
    );
    let mut voltage_previews = Vec::new();
    for (suffix, unit, expected) in [
        (":amdgpu:in0", SensorUnit::Volts, "0.97 V"),
        (":i915:in0", SensorUnit::Volts, "0.90 V"),
        (":xe:in1", SensorUnit::Volts, "0.90 V"),
        (":asusec:in0", SensorUnit::Volts, "0.90 V"),
        (":nct6798:in0", SensorUnit::Volts, "0.90 V"),
        (":zenpower:in1", SensorUnit::Volts, "0.90 V"),
        (
            ":logged_frame_mean",
            SensorUnit::FramesPerSecond,
            "50.00 FPS",
        ),
        (":nouveau:in0", SensorUnit::Volts, "0.90 V"),
        (
            ":asus_wmi_sensors:cpu_core_voltage",
            SensorUnit::Volts,
            "0.90 V",
        ),
        (":amdgpu:freq1", SensorUnit::Hertz, "1500.00 MHz"),
        (
            ":i915:gt0:actual_frequency",
            SensorUnit::Hertz,
            "900.00 MHz",
        ),
        (
            ":i915:gt0:requested_frequency",
            SensorUnit::Hertz,
            "1100.00 MHz",
        ),
        (
            "cpufreq:cpus-4.5:cpuinfo_cur_freq",
            SensorUnit::Hertz,
            "4200.00 MHz",
        ),
    ] {
        let row = rows.iter().find(|r| r.id.ends_with(suffix)).unwrap();
        assert_eq!(
            lconnect_ipc::theme::theme_sensor_text(
                &rows,
                &row.id,
                unit,
                lconnect_ipc::theme::TemperatureUnit::Fahrenheit,
                2
            ),
            expected
        );
        let theme = serde_json::json!({"version":1,"name":"GPU telemetry","background":[0,0,0],"layers":[{
            "kind":"sensor","sensor_id":row.id,"label":"GPU","unit":unit,"temperature_unit":"fahrenheit","decimals":2,
            "style":{"x":240,"y":240,"size":25,"color":[255,255,255],"align":"center"}
        }]});
        let preview = client
            .call(
                "preview_lcd_theme",
                serde_json::json!({"dev":null,"theme":&theme}),
            )
            .await
            .unwrap();
        let bytes: Vec<u8> = serde_json::from_value(preview.clone()).unwrap();
        if matches!(
            suffix,
            ":asus_wmi_sensors:cpu_core_voltage" | ":zenpower:in1" | ":logged_frame_mean"
        ) {
            assert_eq!(
                row.value,
                if suffix == ":logged_frame_mean" {
                    Some(50.0)
                } else {
                    Some(0.9)
                }
            );
            voltage_previews.push((theme, preview));
        }
        assert!(bytes.starts_with(&[255, 216]));
    }
    let power = rows
        .iter()
        .find(|row| row.unit == SensorUnit::Watts)
        .unwrap();
    assert_eq!(
        lconnect_ipc::theme::theme_sensor_text(
            &rows,
            &power.id,
            SensorUnit::Watts,
            lconnect_ipc::theme::TemperatureUnit::Celsius,
            1
        ),
        "125.5 W"
    );
    let theme = serde_json::json!({"version":1,"name":"GPU power","background":[0,0,0],"layers":[{
        "kind":"sensor","sensor_id":power.id,"label":"GPU","unit":"watts","temperature_unit":"celsius","decimals":1,
        "style":{"x":240,"y":240,"size":40,"color":[255,255,255],"align":"center"}
    }]});
    let preview = client
        .call(
            "preview_lcd_theme",
            serde_json::json!({"dev":null,"theme":theme}),
        )
        .await
        .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(preview.clone()).unwrap();
    assert!(bytes.starts_with(&[255, 216]));
    cache.publish(Instant::now() - Duration::from_secs(4), rows.clone());
    let stale = client
        .call("list_sensors", serde_json::json!({}))
        .await
        .unwrap();
    let stale: Vec<lconnect_ipc::SensorReading> = serde_json::from_value(stale).unwrap();
    for suffix in [
        ":i915:in0",
        ":xe:in1",
        ":asus_wmi_sensors:cpu_core_voltage",
        ":zenpower:in1",
    ] {
        let row = stale.iter().find(|r| r.id.ends_with(suffix)).unwrap();
        assert_eq!(
            lconnect_ipc::theme_sensor_text(
                &stale,
                &row.id,
                SensorUnit::Volts,
                lconnect_ipc::TemperatureUnit::Celsius,
                2
            ),
            "—"
        );
    }
    assert_eq!(
        lconnect_ipc::theme_sensor_text(
            &stale,
            "cpufreq:cpus-4.5:cpuinfo_cur_freq",
            SensorUnit::Hertz,
            lconnect_ipc::TemperatureUnit::Celsius,
            1
        ),
        "—"
    );
    let power = stale
        .iter()
        .find(|row| row.unit == SensorUnit::Watts)
        .unwrap();
    assert_eq!(
        lconnect_ipc::theme::theme_sensor_text(
            &stale,
            &power.id,
            SensorUnit::Watts,
            lconnect_ipc::theme::TemperatureUnit::Celsius,
            1
        ),
        "—"
    );
    let unavailable = client
        .call(
            "preview_lcd_theme",
            serde_json::json!({"dev":null,"theme":theme}),
        )
        .await
        .unwrap();
    assert_ne!(
        preview, unavailable,
        "fresh power and unavailable must render differently"
    );
    assert_eq!(
        voltage_previews.len(),
        3,
        "CPU voltages and captured frame mean rendered through IPC"
    );
    for (theme, fresh) in voltage_previews {
        cache.publish(Instant::now() - Duration::from_secs(4), rows.clone());
        let stale = client
            .call(
                "preview_lcd_theme",
                serde_json::json!({"dev":null,"theme":&theme}),
            )
            .await
            .unwrap();
        assert_ne!(
            fresh, stale,
            "stale voltage must not retain its value on LCD"
        );
        cache.publish(Instant::now(), rows.clone());
        let recovered = client
            .call(
                "preview_lcd_theme",
                serde_json::json!({"dev":null,"theme":theme}),
            )
            .await
            .unwrap();
        assert_eq!(
            fresh, recovered,
            "same binding recovers without editing the theme"
        );
    }
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}

#[test]
fn large_proc_stat_tail_does_not_invalidate_aggregate_cpu_line() {
    let f = Fixture::new();
    let mut p = f.provider();
    p.collect();
    f.write(
        "proc/stat",
        &format!("cpu 30 0 10 160 0 0 0 0\n{}", "intr 0 ".repeat(12000)),
    );
    let row = p
        .collect()
        .into_iter()
        .find(|r| r.id == "proc:cpu:utilization")
        .unwrap();
    assert_eq!(row.value, Some(20.0));
}

#[test]
fn many_non_input_hwmon_attributes_do_not_consume_sensor_budget() {
    let f = Fixture::new();
    for i in 2..=100 {
        f.write(&format!("hwmon/hwmon0/temp{i}_input"), "42000");
        for suffix in ["max", "crit", "crit_alarm", "label"] {
            f.write(&format!("hwmon/hwmon0/temp{i}_{suffix}"), "100");
        }
    }
    let rows = f.provider().collect();
    assert_eq!(
        rows.iter()
            .filter(|r| r.unit == SensorUnit::Celsius)
            .count(),
        100
    );
}

#[test]
fn catalog_overflow_is_explicit_and_never_publishes_an_arbitrary_subset() {
    let f = Fixture::new();
    for i in 2..=254 {
        f.write(&format!("hwmon/hwmon0/temp{i}_input"), "42000");
    }
    let rows = f.provider().collect();
    assert_eq!(rows.len(), 4);
    let row = rows.iter().find(|r| r.id == "hwmon:overflow").unwrap();
    assert_eq!(row.status, SensorStatus::Invalid);
    assert_eq!(row.value, None);
    assert!(!rows
        .iter()
        .any(|r| r.unit == SensorUnit::Celsius && r.status == SensorStatus::Valid));
}

#[test]
fn device_bindings_use_stable_key_and_invalid_coolant_does_not_hide_real_rpm() {
    let mut snapshot = lconnect_ipc::SensorSnapshot::default();
    snapshot.last_seen_unix_ms = 42;
    snapshot.temp_valid = false;
    snapshot.pump_rpm = 2400;
    let rows = lconnect_daemon::sensors::device_readings("0416-7395-serial-test", &snapshot);
    assert_eq!(rows[0].id, "lcdv2:0416-7395-serial-test:coolant");
    assert_eq!(rows[0].value, None);
    assert_eq!(rows[0].status, SensorStatus::Invalid);
    assert_eq!(rows[2].unit, SensorUnit::Rpm);
    assert_eq!(rows[2].value, Some(2400.0));
    let cache = SensorCache::default();
    cache.publish(Instant::now() - Duration::from_secs(4), rows);
    assert_eq!(cache.readings()[2].status, SensorStatus::Stale);
    assert_eq!(cache.readings()[2].value, None);
}

#[test]
fn amdgpu_load_and_vram_keep_physical_binding_and_recover_after_missing_files() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "amdgpu");
    f.write("devices/cpu/gpu_busy_percent", "37");
    f.write("devices/cpu/mem_info_vram_total", "8589934592");
    f.write("devices/cpu/mem_info_vram_used", "2147483648");
    let mut p = f.provider();
    let gpu = |rows: Vec<lconnect_ipc::SensorReading>| {
        rows.into_iter()
            .filter(|r| r.id.starts_with("amdgpu:"))
            .collect::<Vec<_>>()
    };
    let first = gpu(p.collect());
    let metric = |suffix: &str| first.iter().find(|r| r.id.ends_with(suffix)).unwrap();
    assert_eq!(metric(":utilization").value, Some(37.0));
    assert_eq!(metric(":vram_used").unit, SensorUnit::Bytes);
    assert_eq!(metric(":vram_used").value, Some(2147483648.0));
    assert_eq!(metric(":vram_utilization").value, Some(25.0));
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon28"),
    )
    .unwrap();
    assert_eq!(
        gpu(p.collect()).iter().map(|r| &r.id).collect::<Vec<_>>(),
        first.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    fs::remove_file(f.dir.path().join("devices/cpu/gpu_busy_percent")).unwrap();
    let missing = gpu(p.collect());
    let load = missing
        .iter()
        .find(|r| r.id == metric(":utilization").id)
        .unwrap();
    assert_eq!(load.status, SensorStatus::Unavailable);
    assert_eq!(load.value, None);
    assert_eq!(load.observed_unix_ms, None);
    f.write("devices/cpu/gpu_busy_percent", "0");
    assert_eq!(
        gpu(p.collect())
            .iter()
            .find(|r| r.id == load.id)
            .unwrap()
            .value,
        Some(0.0)
    );
    fs::remove_dir_all(f.dir.path().join("hwmon/hwmon28")).unwrap();
    assert!(gpu(p.collect()).is_empty());
}

#[test]
fn amdgpu_rejects_impossible_load_memory_and_exposes_unsupported_metrics() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "amdgpu");
    let mut p = f.provider();
    let rows = p.collect();
    assert!(rows
        .iter()
        .filter(|r| r.id.starts_with("amdgpu:"))
        .all(|r| r.status == SensorStatus::Unavailable && r.value.is_none()));
    for load in ["-1", "101", "NaN", "999999999999999999999999"] {
        f.write("devices/cpu/gpu_busy_percent", load);
        let rows = p.collect();
        let row = rows
            .iter()
            .find(|r| r.id.ends_with(":utilization") && r.id.starts_with("amdgpu:"))
            .unwrap();
        assert_eq!(row.status, SensorStatus::Invalid);
        assert_eq!(row.value, None);
    }
    f.write("devices/cpu/mem_info_vram_total", "100");
    for used in ["101", "-1", "bad"] {
        f.write("devices/cpu/mem_info_vram_used", used);
        for row in p
            .collect()
            .iter()
            .filter(|r| r.id.ends_with(":vram_used") || r.id.ends_with(":vram_utilization"))
        {
            assert_eq!(row.status, SensorStatus::Invalid);
            assert_eq!(row.value, None);
        }
    }
    f.write("devices/cpu/mem_info_vram_total", "0");
    f.write("devices/cpu/mem_info_vram_used", "0");
    let rows = p.collect();
    assert_eq!(
        rows.iter()
            .find(|r| r.id.ends_with(":vram_utilization"))
            .unwrap()
            .status,
        SensorStatus::Unavailable
    );
}

#[test]
fn amdgpu_multiple_cards_are_distinct_and_duplicate_identity_is_invalid() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "amdgpu");
    f.write("devices/cpu/gpu_busy_percent", "10");
    fs::create_dir_all(f.dir.path().join("hwmon/hwmon1")).unwrap();
    fs::create_dir_all(f.dir.path().join("devices/gpu2")).unwrap();
    symlink(
        f.dir.path().join("devices/gpu2"),
        f.dir.path().join("hwmon/hwmon1/device"),
    )
    .unwrap();
    f.write("hwmon/hwmon1/name", "amdgpu");
    f.write("devices/gpu2/gpu_busy_percent", "90");
    let mut p = f.provider();
    let load = |rows: Vec<lconnect_ipc::SensorReading>| {
        rows.into_iter()
            .filter(|r| r.id.starts_with("amdgpu:") && r.id.ends_with(":utilization"))
            .collect::<Vec<_>>()
    };
    let rows = load(p.collect());
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].id, rows[1].id);
    assert!(rows.iter().any(|r| r.value == Some(10.0)));
    assert!(rows.iter().any(|r| r.value == Some(90.0)));
    fs::remove_file(f.dir.path().join("hwmon/hwmon1/device")).unwrap();
    symlink(
        f.dir.path().join("devices/cpu"),
        f.dir.path().join("hwmon/hwmon1/device"),
    )
    .unwrap();
    let rows = load(p.collect());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, SensorStatus::Invalid);
    assert_eq!(rows[0].value, None);
}

#[test]
fn gpu_power_input_and_average_keep_distinct_bindings_through_renumbering() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "amdgpu");
    f.write("hwmon/hwmon0/power1_input", "125500000");
    f.write("hwmon/hwmon0/power1_average", "100250000");
    f.write("hwmon/hwmon0/power1_label", "PPT");
    let mut provider = f.provider();
    let readings: Vec<_> = provider
        .collect()
        .into_iter()
        .filter(|r| r.unit == SensorUnit::Watts)
        .collect();
    assert_eq!(readings.len(), 2);
    let input = readings
        .iter()
        .find(|r| r.id.ends_with(":power1_input"))
        .unwrap();
    let average = readings
        .iter()
        .find(|r| r.id.ends_with(":power1_average"))
        .unwrap();
    assert_eq!(input.value, Some(125.5));
    assert_eq!(average.value, Some(100.25));
    assert!(input.label.contains("PPT (input)"));
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon37"),
    )
    .unwrap();
    let next = provider.collect();
    assert!(next
        .iter()
        .any(|r| r.id == input.id && r.value == input.value));
    fs::remove_file(f.dir.path().join("hwmon/hwmon37/power1_input")).unwrap();
    let next = provider.collect();
    assert!(!next.iter().any(|r| r.id == input.id));
    assert!(next
        .iter()
        .any(|r| r.id == average.id && r.value == average.value));
}

#[test]
fn gpu_power_errors_disable_recovery_and_zero_are_explicit() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "nouveau");
    let mut provider = f.provider();
    for (value, enabled, status, expected) in [
        ("25000000", "1", SensorStatus::Valid, Some(25.0)),
        ("-1", "1", SensorStatus::Invalid, None),
        ("NaN", "1", SensorStatus::Invalid, None),
        ("25000000", "0", SensorStatus::Unavailable, None),
        ("25000000", "2", SensorStatus::Invalid, None),
        ("0", "1", SensorStatus::Valid, Some(0.0)),
    ] {
        f.write("hwmon/hwmon0/power1_input", value);
        f.write("hwmon/hwmon0/power1_enable", enabled);
        let row = provider
            .collect()
            .into_iter()
            .find(|r| r.unit == SensorUnit::Watts)
            .unwrap();
        assert_eq!((row.status, row.value), (status, expected));
        assert_eq!(
            row.observed_unix_ms.is_some(),
            status == SensorStatus::Valid
        );
    }
    f.write("hwmon/hwmon0/name", "unknown-driver");
    assert!(!provider
        .collect()
        .iter()
        .any(|r| r.unit == SensorUnit::Watts));
}

#[test]
fn amdgpu_voltage_and_clocks_keep_units_rails_and_physical_identity() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "amdgpu");
    for (file, value) in [
        ("in0_input", "975"),
        ("in1_input", "1100"),
        ("freq1_input", "1500000000"),
        ("freq2_input", "875000000"),
        ("in2_input", "3300"),
        ("freq0_input", "100"),
        ("in0_max", "2000"),
    ] {
        f.write(&format!("hwmon/hwmon0/{file}"), value);
    }
    let mut provider = f.provider();
    let readings = |rows: Vec<lconnect_ipc::SensorReading>| {
        rows.into_iter()
            .filter(|r| matches!(r.unit, SensorUnit::Volts | SensorUnit::Hertz))
            .collect::<Vec<_>>()
    };
    let first = readings(provider.collect());
    assert_eq!(first.len(), 4);
    for (suffix, unit, value, label) in [
        (":in0", SensorUnit::Volts, 0.975, "GPU voltage"),
        (":in1", SensorUnit::Volts, 1.1, "Northbridge voltage"),
        (":freq1", SensorUnit::Hertz, 1_500_000_000.0, "GFX clock"),
        (":freq2", SensorUnit::Hertz, 875_000_000.0, "Memory clock"),
    ] {
        let row = first.iter().find(|r| r.id.ends_with(suffix)).unwrap();
        assert_eq!(row.unit, unit);
        assert_eq!(row.value, Some(value));
        assert!(row.label.ends_with(label));
    }
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon9"),
    )
    .unwrap();
    let renamed = readings(provider.collect());
    assert_eq!(
        first.iter().map(|r| &r.id).collect::<Vec<_>>(),
        renamed.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    fs::remove_file(f.dir.path().join("hwmon/hwmon9/in0_input")).unwrap();
    let missing = readings(provider.collect());
    assert_eq!(missing.len(), 3);
    assert!(!missing.iter().any(|r| r.id.ends_with(":in0")));
    f.write("hwmon/hwmon9/name", "coretemp");
    assert!(
        readings(provider.collect()).is_empty(),
        "do not invent CPU voltage from unrelated channels"
    );
}

#[test]
fn amdgpu_voltage_and_frequency_errors_do_not_fake_zero() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "amdgpu");
    let mut provider = f.provider();
    for prefix in ["in0", "freq1"] {
        for (value, enable, status) in [
            ("-1", "1", SensorStatus::Invalid),
            ("NaN", "1", SensorStatus::Invalid),
            ("975", "0", SensorStatus::Unavailable),
            ("975", "2", SensorStatus::Invalid),
            ("0", "1", SensorStatus::Valid),
        ] {
            f.write(&format!("hwmon/hwmon0/{prefix}_input"), value);
            f.write(&format!("hwmon/hwmon0/{prefix}_enable"), enable);
            let row = provider
                .collect()
                .into_iter()
                .find(|r| r.id.ends_with(&format!(":{prefix}")))
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
    }
}

#[test]
fn hwmon_fans_use_rpm_without_guessing_cpu_role_or_scaling_twice() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "nct6775");
    f.write("hwmon/hwmon0/fan1_input", "1234");
    f.write("hwmon/hwmon0/fan1_label", "CPU Fan");
    f.write("hwmon/hwmon0/fan1_div", "8");
    f.write("hwmon/hwmon0/fan1_pulses", "4");
    f.write("hwmon/hwmon0/fan1_target", "2000");
    f.write("hwmon/hwmon0/fan1_alarm", "1");
    f.write("hwmon/hwmon0/fan0_input", "999");
    f.write("hwmon/hwmon0/fan+2_input", "999");
    f.write("hwmon/hwmon0/in0_input", "1000");
    let rows = f.provider().collect();
    let fans: Vec<_> = rows.iter().filter(|r| r.unit == SensorUnit::Rpm).collect();
    assert_eq!(fans.len(), 1);
    assert_eq!(fans[0].value, Some(1234.0));
    assert!(fans[0].label.contains("CPU Fan"));
    // Eligible for explicit assignment, but the CPU label cannot auto-select it.
    assert!(lconnect_daemon::sensor_selection::recommended(&rows, 0)
        .cpu
        .fan
        .is_none());
    assert!(!rows
        .iter()
        .any(|r| matches!(r.unit, SensorUnit::Celsius | SensorUnit::Volts)));
}

#[test]
fn hwmon_fan_zero_fault_enable_and_invalid_input_are_distinct() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "nct6775");
    let mut provider = f.provider();
    for (input, flag, status, value) in [
        ("0", None, SensorStatus::Valid, Some(0.0)),
        (
            "1200",
            Some(("fault", "1")),
            SensorStatus::Unavailable,
            None,
        ),
        ("1200", Some(("fault", "2")), SensorStatus::Invalid, None),
        (
            "1200",
            Some(("enable", "0")),
            SensorStatus::Unavailable,
            None,
        ),
        ("1200", Some(("enable", "2")), SensorStatus::Invalid, None),
        ("-1", None, SensorStatus::Invalid, None),
        ("NaN", None, SensorStatus::Invalid, None),
        ("9223372036854775808", None, SensorStatus::Invalid, None),
        ("1200", None, SensorStatus::Valid, Some(1200.0)),
    ] {
        for suffix in ["fault", "enable"] {
            let _ = fs::remove_file(f.dir.path().join(format!("hwmon/hwmon0/fan1_{suffix}")));
        }
        f.write("hwmon/hwmon0/fan1_input", input);
        if let Some((suffix, flag)) = flag {
            f.write(&format!("hwmon/hwmon0/fan1_{suffix}"), flag);
        }
        let row = provider
            .collect()
            .into_iter()
            .find(|r| r.unit == SensorUnit::Rpm)
            .unwrap();
        assert_eq!((row.status, row.value), (status, value));
        assert_eq!(row.observed_unix_ms.is_some(), value.is_some());
    }
    f.write("hwmon/hwmon0/fan1_input", &"1".repeat(65537));
    assert_eq!(
        provider
            .collect()
            .into_iter()
            .find(|r| r.unit == SensorUnit::Rpm)
            .unwrap()
            .status,
        SensorStatus::Invalid
    );
}

#[test]
fn hwmon_fan_binding_survives_renumber_and_revokes_missing_or_stale_readings() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "nct6775");
    f.write("hwmon/hwmon0/fan1_input", "1100");
    let mut provider = f.provider();
    let row = provider
        .collect()
        .into_iter()
        .find(|r| r.unit == SensorUnit::Rpm)
        .unwrap();
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon8"),
    )
    .unwrap();
    let renamed = provider
        .collect()
        .into_iter()
        .find(|r| r.unit == SensorUnit::Rpm)
        .unwrap();
    assert_eq!(row.id, renamed.id);
    let cache = SensorCache::default();
    let start = Instant::now();
    cache.publish(start - Duration::from_secs(4), vec![renamed]);
    let stale = cache.readings();
    let stale = stale.iter().find(|r| r.id == row.id).unwrap();
    assert_eq!((stale.status, stale.value), (SensorStatus::Stale, None));
    fs::remove_file(f.dir.path().join("hwmon/hwmon8/fan1_input")).unwrap();
    assert!(!provider.collect().iter().any(|r| r.id == row.id));
    f.write("hwmon/hwmon8/fan1_input", "1400");
    let recovered = provider
        .collect()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert_eq!(recovered.value, Some(1400.0));
}

#[test]
fn hwmon_fan_catalog_rejects_ambiguous_identity_and_overflow() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "nct6775");
    f.write("hwmon/hwmon0/fan1_input", "1200");
    let duplicate = f.dir.path().join("hwmon/hwmon1");
    fs::create_dir(&duplicate).unwrap();
    symlink(f.dir.path().join("devices/cpu"), duplicate.join("device")).unwrap();
    fs::write(duplicate.join("name"), "nct6775").unwrap();
    fs::write(duplicate.join("fan1_input"), "1300").unwrap();
    let mut provider = f.provider();
    let row = provider
        .collect()
        .into_iter()
        .find(|r| r.unit == SensorUnit::Rpm)
        .unwrap();
    assert_eq!((row.status, row.value), (SensorStatus::Invalid, None));
    fs::remove_dir_all(duplicate).unwrap();
    for index in 2..=257 {
        f.write(&format!("hwmon/hwmon0/fan{index}_input"), "1200");
    }
    let rows = provider.collect();
    assert!(!rows.iter().any(|r| r.unit == SensorUnit::Rpm));
    assert!(rows
        .iter()
        .any(|r| r.id == "hwmon:overflow" && r.status == SensorStatus::Invalid));
}

#[test]
fn gpu_voltage_uses_driver_specific_rail_units_and_gpu_membership() {
    use lconnect_daemon::sensor_selection::{candidates, recommended, validate_selection};
    use lconnect_ipc::{DefaultSensorRole, SensorDefaults};
    for (driver, channel, other, label) in [
        ("i915", "in0", "in1", "GPU GT voltage"),
        ("xe", "in1", "in0", "GPU package voltage"),
        ("nouveau", "in0", "in1", "GPU core voltage"),
    ] {
        let f = Fixture::new();
        f.write("hwmon/hwmon0/name", driver);
        f.write(&format!("hwmon/hwmon0/{channel}_input"), "975");
        f.write(&format!("hwmon/hwmon0/{other}_input"), "1200");
        let mut provider = f.provider();
        let rows = provider.collect();
        let voltage: Vec<_> = rows
            .iter()
            .filter(|r| r.unit == SensorUnit::Volts)
            .collect();
        assert_eq!(voltage.len(), 1);
        assert_eq!(voltage[0].value, Some(0.975));
        assert!(voltage[0].label.ends_with(label));
        let choices = candidates(&rows);
        let choice = choices
            .iter()
            .find(|c| c.role == DefaultSensorRole::GpuVoltage)
            .unwrap();
        assert_eq!(choice.reading.id, voltage[0].id);
        assert!(choice.gpu_id.as_ref().unwrap().ends_with(driver));
        let defaults = recommended(&rows, 0);
        assert_eq!(
            defaults.gpu.as_ref().unwrap().voltage.as_ref(),
            Some(&voltage[0].id)
        );
        validate_selection(&defaults, &SensorDefaults::default(), &rows).unwrap();
        fs::rename(
            f.dir.path().join("hwmon/hwmon0"),
            f.dir.path().join("hwmon/hwmon9"),
        )
        .unwrap();
        assert!(provider
            .collect()
            .iter()
            .any(|r| r.id == voltage[0].id && r.value == Some(0.975)));
        fs::remove_file(f.dir.path().join(format!("hwmon/hwmon9/{channel}_input"))).unwrap();
        let missing = provider.collect();
        assert!(!missing.iter().any(|r| r.id == voltage[0].id));
        validate_selection(&defaults, &defaults, &missing).unwrap();
        assert!(validate_selection(&defaults, &SensorDefaults::default(), &missing).is_err());
        f.write(&format!("hwmon/hwmon9/{channel}_input"), "825");
        let recovered = provider.collect();
        assert!(recovered
            .iter()
            .any(|r| r.id == voltage[0].id && r.value == Some(0.825)));
        assert_eq!(recommended(&recovered, 0).gpu, defaults.gpu);
    }
}

#[test]
fn driver_voltage_zero_disabled_malformed_and_stale_do_not_alias_rails() {
    for (driver, channel) in [
        ("i915", "in0"),
        ("xe", "in1"),
        ("asusec", "in0"),
        ("nouveau", "in0"),
    ] {
        let f = Fixture::new();
        f.write("hwmon/hwmon0/name", driver);
        let mut provider = f.provider();
        for (input, enabled, status, value) in [
            ("0", "1", SensorStatus::Valid, Some(0.0)),
            ("975", "0", SensorStatus::Unavailable, None),
            ("-1", "1", SensorStatus::Invalid, None),
            ("bad", "1", SensorStatus::Invalid, None),
            ("975", "bad", SensorStatus::Invalid, None),
            ("975", "1", SensorStatus::Valid, Some(0.975)),
        ] {
            f.write(&format!("hwmon/hwmon0/{channel}_input"), input);
            f.write(&format!("hwmon/hwmon0/{channel}_enable"), enabled);
            let row = provider
                .collect()
                .into_iter()
                .find(|r| r.unit == SensorUnit::Volts)
                .unwrap();
            assert_eq!((row.status, row.value), (status, value));
        }
        let cache = SensorCache::default();
        cache.publish(Instant::now() - Duration::from_secs(4), provider.collect());
        let rows = cache.readings();
        let voltage = rows.iter().find(|r| r.unit == SensorUnit::Volts).unwrap();
        assert_eq!((voltage.status, voltage.value), (SensorStatus::Stale, None));
        f.write("hwmon/hwmon0/name", "nct6775");
        assert!(!provider
            .collect()
            .iter()
            .any(|r| r.unit == SensorUnit::Volts));
    }
}

#[test]
fn asus_ec_cpu_voltage_has_verified_role_and_stable_identity() {
    use lconnect_daemon::sensor_selection::{candidates, recommended, validate_selection};
    use lconnect_ipc::{DefaultSensorRole, SensorDefaults};
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "asusec");
    f.write("hwmon/hwmon0/in0_input", "1234");
    f.write("hwmon/hwmon0/in1_input", "3300");
    f.write("hwmon/hwmon0/cpu0_vid", "1400");
    let mut provider = f.provider();
    let rows = provider.collect();
    let volts: Vec<_> = rows
        .iter()
        .filter(|r| r.unit == SensorUnit::Volts)
        .collect();
    assert_eq!(
        volts.len(),
        1,
        "neither VID nor another rail is CPU core voltage"
    );
    let voltage = volts[0];
    assert_eq!(voltage.value, Some(1.234));
    assert!(voltage.label.ends_with("CPU core voltage"));
    let choices = candidates(&rows);
    let candidate = choices
        .iter()
        .find(|c| c.role == DefaultSensorRole::CpuVoltage)
        .unwrap();
    assert_eq!(candidate.reading.id, voltage.id);
    assert_eq!(candidate.gpu_id, None);
    let defaults = recommended(&rows, 0);
    assert_eq!(defaults.cpu.voltage.as_ref(), Some(&voltage.id));
    validate_selection(&defaults, &SensorDefaults::default(), &rows).unwrap();
    // Labels never determine CPU identity or recommendations.
    f.write("hwmon/hwmon0/in0_label", "Unexpected firmware label");
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon9"),
    )
    .unwrap();
    let renamed = provider.collect();
    assert!(renamed
        .iter()
        .any(|r| r.id == voltage.id && r.value == Some(1.234)));
    assert_eq!(recommended(&renamed, 0).cpu.voltage, defaults.cpu.voltage);
    fs::remove_file(f.dir.path().join("hwmon/hwmon9/in0_input")).unwrap();
    let missing = provider.collect();
    assert!(!missing.iter().any(|r| r.id == voltage.id));
    validate_selection(&defaults, &defaults, &missing).unwrap();
    assert!(validate_selection(&defaults, &SensorDefaults::default(), &missing).is_err());
    f.write("hwmon/hwmon9/in0_input", "1100");
    assert!(provider
        .collect()
        .iter()
        .any(|r| r.id == voltage.id && r.value == Some(1.1)));
}

#[test]
fn asus_wmi_voltage_is_opt_in_exact_rail_and_survives_channel_renumber() {
    use lconnect_daemon::sensor_selection::{candidates, recommended, validate_selection};
    use lconnect_ipc::{DefaultSensorRole, SensorDefaults};
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "asus_wmi_sensors");
    f.write("hwmon/hwmon0/in4_input", "1234");
    f.write("hwmon/hwmon0/in4_label", "CPU Core Voltage\n");
    f.write("hwmon/hwmon0/in0_input", "987");
    f.write("hwmon/hwmon0/in0_label", "CPU SOC Voltage");
    f.write("hwmon/hwmon0/in1_input", "3300");
    f.write("hwmon/hwmon0/in1_label", "DRAM Voltage");
    f.write("hwmon/hwmon0/cpu0_vid", "1400");
    assert!(!f
        .provider()
        .collect()
        .iter()
        .any(|r| r.unit == SensorUnit::Volts));
    let mut provider = f.provider().with_asus_wmi_voltage();
    let rows = provider.collect();
    let volts: Vec<_> = rows
        .iter()
        .filter(|r| r.unit == SensorUnit::Volts)
        .collect();
    assert_eq!(volts.len(), 1);
    let id = volts[0].id.clone();
    assert!(id.ends_with(":asus_wmi_sensors:cpu_core_voltage"));
    assert_eq!(volts[0].value, Some(1.234));
    assert!(candidates(&rows)
        .iter()
        .any(|c| c.reading.id == id && c.role == DefaultSensorRole::CpuVoltage));
    let defaults = recommended(&rows, 0);
    assert_eq!(defaults.cpu.voltage.as_ref(), Some(&id));
    validate_selection(&defaults, &SensorDefaults::default(), &rows).unwrap();
    // Channel number and hwmon enumeration may change; firmware rail identity does not.
    for suffix in ["input", "label"] {
        fs::rename(
            f.dir.path().join(format!("hwmon/hwmon0/in4_{suffix}")),
            f.dir.path().join(format!("hwmon/hwmon0/in7_{suffix}")),
        )
        .unwrap();
    }
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon9"),
    )
    .unwrap();
    assert!(provider
        .collect()
        .iter()
        .any(|r| r.id == id && r.value == Some(1.234)));
    f.write("hwmon/hwmon9/in7_label", "CPU VID");
    let missing = provider.collect();
    assert!(!missing.iter().any(|r| r.id == id));
    validate_selection(&defaults, &defaults, &missing).unwrap();
    assert!(validate_selection(&defaults, &SensorDefaults::default(), &missing).is_err());
    f.write("hwmon/hwmon9/in7_label", "CPU Core Voltage");
    f.write("hwmon/hwmon9/in7_input", "1100");
    assert!(provider
        .collect()
        .iter()
        .any(|r| r.id == id && r.value == Some(1.1)));
    // Two indistinguishable SIO/EC rails must not become an arbitrary first match.
    f.write("hwmon/hwmon9/in0_label", "CPU Core Voltage");
    let duplicate = provider.collect().into_iter().find(|r| r.id == id).unwrap();
    assert_eq!(
        (duplicate.status, duplicate.value),
        (SensorStatus::Invalid, None)
    );
    f.write("hwmon/hwmon9/in0_label", "CPU SOC Voltage");
    for (value, status, expected) in [
        ("0", SensorStatus::Valid, Some(0.0)),
        ("bad", SensorStatus::Invalid, None),
        ("-1", SensorStatus::Invalid, None),
    ] {
        f.write("hwmon/hwmon9/in7_input", value);
        let row = provider.collect().into_iter().find(|r| r.id == id).unwrap();
        assert_eq!((row.status, row.value), (status, expected));
    }
    f.write("hwmon/hwmon9/in7_input", "1200");
    let cache = SensorCache::default();
    cache.publish(Instant::now() - Duration::from_secs(4), provider.collect());
    let stale = cache.readings().into_iter().find(|r| r.id == id).unwrap();
    assert_eq!((stale.status, stale.value), (SensorStatus::Stale, None));
}

#[test]
fn nct_cpuvc_voltage_identity_selection_and_recovery() {
    use lconnect_daemon::sensor_selection::{candidates, recommended, validate_selection};
    use lconnect_ipc::{DefaultSensorRole, SensorDefaults};
    for driver in [
        "nct6779", "nct6791", "nct6792", "nct6793", "nct6795", "nct6796", "nct6797", "nct6798",
        "nct6799",
    ] {
        let f = Fixture::new();
        f.write("hwmon/hwmon0/name", driver);
        f.write("hwmon/hwmon0/in0_input", "1232");
        f.write("hwmon/hwmon0/in1_input", "3300");
        f.write("hwmon/hwmon0/in1_label", "CPU Core Voltage");
        f.write("hwmon/hwmon0/cpu0_vid", "1400");
        let mut provider = f.provider();
        let rows = provider.collect();
        let voltage: Vec<_> = rows
            .iter()
            .filter(|r| r.unit == SensorUnit::Volts)
            .collect();
        assert_eq!(
            voltage.len(),
            1,
            "Only CPUVC, never another pin or VID: {driver}"
        );
        let id = voltage[0].id.clone();
        assert_eq!(voltage[0].value, Some(1.232));
        assert!(voltage[0].label.ends_with("CPU core voltage (CPUVC)"));
        assert!(candidates(&rows)
            .iter()
            .any(|c| c.role == DefaultSensorRole::CpuVoltage
                && c.reading.id == id
                && c.gpu_id.is_none()));
        let defaults = recommended(&rows, 0);
        assert_eq!(defaults.cpu.voltage.as_ref(), Some(&id));
        validate_selection(&defaults, &SensorDefaults::default(), &rows).unwrap();
        fs::rename(
            f.dir.path().join("hwmon/hwmon0"),
            f.dir.path().join("hwmon/hwmon9"),
        )
        .unwrap();
        for (input, expected_status, expected_value) in [
            ("bad", SensorStatus::Invalid, None),
            ("-1", SensorStatus::Invalid, None),
            ("0", SensorStatus::Valid, Some(0.0)),
            ("984", SensorStatus::Valid, Some(0.984)),
        ] {
            f.write("hwmon/hwmon9/in0_input", input);
            let row = provider.collect().into_iter().find(|r| r.id == id).unwrap();
            assert_eq!((row.status, row.value), (expected_status, expected_value));
        }
        f.write("hwmon/hwmon9/in0_enable", "0");
        let row = provider.collect().into_iter().find(|r| r.id == id).unwrap();
        assert_eq!((row.status, row.value), (SensorStatus::Unavailable, None));
        fs::remove_file(f.dir.path().join("hwmon/hwmon9/in0_enable")).unwrap();
        let cache = SensorCache::default();
        cache.publish(Instant::now() - Duration::from_secs(4), provider.collect());
        let stale = cache.readings().into_iter().find(|r| r.id == id).unwrap();
        assert_eq!((stale.status, stale.value), (SensorStatus::Stale, None));
        fs::remove_file(f.dir.path().join("hwmon/hwmon9/in0_input")).unwrap();
        let missing = provider.collect();
        assert!(!missing.iter().any(|r| r.id == id));
        validate_selection(&defaults, &defaults, &missing).unwrap();
        assert!(validate_selection(&defaults, &SensorDefaults::default(), &missing).is_err());
        f.write("hwmon/hwmon9/in0_input", "1040");
        assert!(provider
            .collect()
            .iter()
            .any(|r| r.id == id && r.value == Some(1.04)));
    }
    for driver in ["nct6775", "nct6106", "nct6794", "nct9999", "it87"] {
        let f = Fixture::new();
        f.write("hwmon/hwmon0/name", driver);
        f.write("hwmon/hwmon0/in0_input", "1200");
        f.write("hwmon/hwmon0/in0_label", "CPU Core Voltage");
        assert!(
            !f.provider()
                .collect()
                .iter()
                .any(|r| r.unit == SensorUnit::Volts),
            "Unverified driver {driver} must not become CPU voltage from a label"
        );
    }
}

#[test]
fn zenpower_core_voltage_and_temperatures_have_verified_roles_without_soc_or_vid() {
    use lconnect_daemon::sensor_selection::{candidates, recommended, validate_selection};
    use lconnect_ipc::{DefaultSensorRole, SensorDefaults};
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "zenpower");
    f.write("hwmon/hwmon0/in0_input", "999");
    f.write("hwmon/hwmon0/in0_label", "SVI2_Core");
    f.write("hwmon/hwmon0/in1_input", "1234");
    f.write("hwmon/hwmon0/in1_label", "SVI2_Core");
    f.write("hwmon/hwmon0/in2_input", "1100");
    f.write("hwmon/hwmon0/in2_label", "SVI2_SoC");
    f.write("hwmon/hwmon0/cpu0_vid", "1550");
    f.write("hwmon/hwmon0/temp1_label", "Tdie");
    let mut provider = f.provider();
    let rows = provider.collect();
    let voltage: Vec<_> = rows
        .iter()
        .filter(|r| r.unit == SensorUnit::Volts)
        .collect();
    assert_eq!(voltage.len(), 1);
    let id = voltage[0].id.clone();
    assert_eq!(voltage[0].value, Some(1.234));
    let choices = candidates(&rows);
    assert!(choices
        .iter()
        .any(|c| c.role == DefaultSensorRole::CpuVoltage && c.reading.id == id));
    assert!(choices
        .iter()
        .any(|c| c.role == DefaultSensorRole::CpuTemperature));
    let defaults = recommended(&rows, 0);
    assert_eq!(defaults.cpu.voltage.as_ref(), Some(&id));
    validate_selection(&defaults, &SensorDefaults::default(), &rows).unwrap();
    for label in ["SVI2_Core", "cpu0 SVI2_Core", "cpu1 SVI2_Core"] {
        f.write("hwmon/hwmon0/in1_label", label);
        assert!(provider
            .collect()
            .iter()
            .any(|r| r.id == id && r.label.ends_with(label)));
    }
    fs::rename(
        f.dir.path().join("hwmon/hwmon0"),
        f.dir.path().join("hwmon/hwmon41"),
    )
    .unwrap();
    assert!(provider
        .collect()
        .iter()
        .any(|r| r.id == id && r.value == Some(1.234)));
    fs::remove_file(f.dir.path().join("hwmon/hwmon41/in1_input")).unwrap();
    let missing = provider.collect();
    assert!(!missing.iter().any(|r| r.id == id));
    validate_selection(&defaults, &defaults, &missing).unwrap();
    assert!(validate_selection(&defaults, &SensorDefaults::default(), &missing).is_err());
    f.write("hwmon/hwmon41/in1_input", "900");
    assert!(provider
        .collect()
        .iter()
        .any(|r| r.id == id && r.value == Some(0.9)));
}
#[test]
fn zenpower_requires_exact_driver_core_channel_label_and_fresh_valid_reading() {
    let f = Fixture::new();
    f.write("hwmon/hwmon0/name", "zenpower");
    f.write("hwmon/hwmon0/in1_input", "900");
    f.write("hwmon/hwmon0/in2_input", "1000");
    f.write("hwmon/hwmon0/in2_label", "SVI2_Core");
    let mut provider = f.provider();
    for label in [
        "SVI2_SoC",
        "Core VID",
        "cpu2 SVI2_Core",
        "unverified",
        "",
        "SVI3_Core",
    ] {
        f.write("hwmon/hwmon0/in1_label", label);
        assert!(!provider
            .collect()
            .iter()
            .any(|r| r.unit == SensorUnit::Volts));
    }
    fs::remove_file(f.dir.path().join("hwmon/hwmon0/in1_label")).unwrap();
    assert!(!provider
        .collect()
        .iter()
        .any(|r| r.unit == SensorUnit::Volts));
    f.write("hwmon/hwmon0/in1_label", "SVI2_Core");
    for (input, enable, status, value) in [
        ("0", "1", SensorStatus::Valid, Some(0.0)),
        ("900", "0", SensorStatus::Unavailable, None),
        ("bad", "1", SensorStatus::Invalid, None),
        ("-1", "1", SensorStatus::Invalid, None),
        ("900", "bad", SensorStatus::Invalid, None),
        ("900", "1", SensorStatus::Valid, Some(0.9)),
    ] {
        f.write("hwmon/hwmon0/in1_input", input);
        f.write("hwmon/hwmon0/in1_enable", enable);
        let row = provider
            .collect()
            .into_iter()
            .find(|r| r.unit == SensorUnit::Volts)
            .unwrap();
        assert_eq!((row.status, row.value), (status, value));
    }
    let cache = SensorCache::default();
    cache.publish(Instant::now() - Duration::from_secs(4), provider.collect());
    let stale = cache
        .readings()
        .into_iter()
        .find(|r| r.unit == SensorUnit::Volts)
        .unwrap();
    assert_eq!((stale.status, stale.value), (SensorStatus::Stale, None));
    for driver in ["k10temp", "zenpower3", "unknown"] {
        f.write("hwmon/hwmon0/name", driver);
        assert!(!provider
            .collect()
            .iter()
            .any(|r| r.unit == SensorUnit::Volts));
    }
}
