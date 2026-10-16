//! Read-only Linux host providers. Collection never uses the HID queue.
//!
//! ABI: <https://docs.kernel.org/hwmon/sysfs-interface.html> and
//! <https://docs.kernel.org/filesystems/proc.html>. Only known temperature
//! drivers are accepted; generic hwmon temperature files can report millivolts.
//! Fan inputs use the common RPM ABI; channel labels do not prove CPU ownership.
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

mod cpufreq;
mod intel;
mod intel_clocks;
mod intel_memory;
mod mangohud;
mod network;
mod nvidia;
mod nvidia_voltage;
mod power;
mod query_process;
mod storage;
mod xe;

const MAX_READINGS: usize = 256;
const MAX_HWMON_FILES: usize = 4096;
const STALE_AFTER: Duration = Duration::from_secs(3);

/// Injectable filesystem roots allow provider tests without touching hardware.
pub struct LinuxSensors {
    mangohud: Option<mangohud::MangoHud>,
    fps_settings: Option<watch::Receiver<Option<lconnect_ipc::FpsSourceSettings>>>,
    fps_applied: Option<lconnect_ipc::FpsSourceSettings>,
    cpufreq: Option<cpufreq::CpuFrequency>,
    intel_clocks: Option<intel_clocks::IntelClocks>,
    network: Option<network::Network>,
    storage: Option<storage::Storage>,
    intel_memory: Option<intel_memory::IntelMemory>,
    xe: Option<xe::XeSensors>,
    asus_wmi_voltage: bool,
    hwmon: PathBuf,
    proc: PathBuf,
    cpu_previous: Option<(u64, u64)>,
    nvidia: Option<nvidia::NvidiaSensors>,
    power: Option<power::PackagePower>,
    intel: Option<intel::IntelSensors>,
}

impl Default for LinuxSensors {
    fn default() -> Self {
        Self {
            asus_wmi_voltage: std::env::var("LCONNECT_ASUS_WMI_VOLTAGE").as_deref() == Ok("1"),
            storage: Some(storage::Storage::new("/usr/bin/findmnt".into())),
            network: Some(network::Network::new(
                "/sys/class/net".into(),
                "/proc/sys/kernel/random/boot_id".into(),
            )),
            intel_clocks: Some(intel_clocks::IntelClocks::new("/sys/class/drm".into())),
            mangohud: std::env::var_os("LCONNECT_MANGOHUD_PER_FRAME_LOG")
                .map(|p| mangohud::MangoHud::new(p.into())),
            cpufreq: Some(cpufreq::CpuFrequency::new(
                "/sys/devices/system/cpu/cpufreq".into(),
            )),
            intel: Some(intel::IntelSensors::new(
                "/sys/class/drm".into(),
                "/sys/bus/event_source/devices".into(),
                "/sys/devices/system/cpu/online".into(),
            )),
            power: Some(power::PackagePower::new("/sys/class/powercap".into())),
            nvidia: Some(
                nvidia::NvidiaSensors::new("/usr/bin/nvidia-smi".into()).with_voltage_from_env(),
            ),
            ..Self::new("/sys/class/hwmon", "/proc")
        }
    }
}

fn reading(
    id: impl Into<String>,
    label: impl Into<String>,
    unit: SensorUnit,
    value: Result<f64, SensorStatus>,
    now: u64,
) -> SensorReading {
    let (value, status) = match value {
        Ok(v) if v.is_finite() => (Some(v), SensorStatus::Valid),
        Ok(_) => (None, SensorStatus::Invalid),
        Err(e) => (None, e),
    };
    SensorReading {
        id: id.into(),
        label: label.into(),
        unit,
        value,
        status,
        observed_unix_ms: value.map(|_| now),
        age_ms: value.map(|_| 0),
    }
}

fn text(path: &Path) -> Result<String, SensorStatus> {
    // All inputs are kernel-generated scalar/proc files. Cap reads as well as
    // catalog size so a malformed test root cannot allocate without a bound.
    use std::io::Read;
    let file = fs::File::open(path).map_err(|_| SensorStatus::Unavailable)?;
    let mut result = String::new();
    file.take(65537)
        .read_to_string(&mut result)
        .map_err(|_| SensorStatus::Unavailable)?;
    if result.len() > 65536 {
        return Err(SensorStatus::Invalid);
    }
    Ok(result)
}

fn first_line(path: &Path) -> Result<String, SensorStatus> {
    use std::io::{BufRead, BufReader, Read};
    let file = fs::File::open(path).map_err(|_| SensorStatus::Unavailable)?;
    let mut line = String::new();
    BufReader::new(file.take(65537))
        .read_line(&mut line)
        .map_err(|_| SensorStatus::Unavailable)?;
    if line.len() > 65536 {
        return Err(SensorStatus::Invalid);
    }
    Ok(line)
}

fn number(path: &Path) -> Result<i64, SensorStatus> {
    text(path)?
        .trim()
        .parse()
        .map_err(|_| SensorStatus::Invalid)
}

// Linux v6.12 nct6775-core.c: these variants share NCT6779_REG_IN;
// channel 0 is CPUVC. Do not extend this to arbitrary Super-I/O inputs.
pub(crate) fn is_nct_cpu_voltage_driver(driver: &str) -> bool {
    matches!(
        driver,
        "nct6779"
            | "nct6791"
            | "nct6792"
            | "nct6793"
            | "nct6795"
            | "nct6796"
            | "nct6797"
            | "nct6798"
            | "nct6799"
    )
}

impl LinuxSensors {
    fn reset_baselines(&mut self) {
        if let Some(source) = &mut self.mangohud {
            source.reset();
        }
        self.cpu_previous = None;
        if let Some(provider) = &mut self.power {
            provider.reset_baselines();
        }
        if let Some(provider) = &mut self.network {
            provider.reset_baselines();
        }
        if let Some(provider) = &mut self.intel {
            provider.reset_baselines();
        }
        if let Some(provider) = &mut self.xe {
            provider.reset_baselines();
        }
    }

    pub fn new(hwmon: impl Into<PathBuf>, proc: impl Into<PathBuf>) -> Self {
        Self {
            mangohud: None,
            fps_settings: None,
            fps_applied: None,
            storage: None,
            intel_memory: None,
            xe: None,
            network: None,
            intel_clocks: None,
            cpufreq: None,
            asus_wmi_voltage: false,
            hwmon: hwmon.into(),
            proc: proc.into(),
            cpu_previous: None,
            nvidia: None,
            power: None,
            intel: None,
        }
    }

    /// Bind persisted live intent. Unreadable/uninitialized settings disable FPS;
    /// the other host providers continue collecting normally.
    pub fn with_fps_settings(
        mut self,
        settings: watch::Receiver<Option<lconnect_ipc::FpsSourceSettings>>,
    ) -> Self {
        self.fps_settings = Some(settings);
        self.mangohud = None;
        self
    }
    fn sync_fps_settings(&mut self) {
        if let Some(settings) = &self.fps_settings {
            let next = settings.borrow().clone();
            if next != self.fps_applied {
                self.mangohud = next
                    .as_ref()
                    .filter(|s| s.enabled)
                    .and_then(|s| s.log_path.as_ref())
                    .map(|p| mangohud::MangoHud::new(p.into()));
                self.fps_applied = next;
            }
        }
    }
    /// Opt in to a single already-running MangoHud log configured with
    /// log_interval=0. Does not enable logging or launch/inject another program.
    pub fn with_mangohud_per_frame_log(mut self, path: impl Into<PathBuf>) -> Self {
        self.mangohud = Some(mangohud::MangoHud::new(path.into()));
        self
    }

    /// Enable isolated Intel memory/topology queries and xe PMU counters through the selected daemon
    /// executable. Kept out of Default so test/example binaries never recurse.
    pub fn with_intel_query_helper(mut self, executable: impl Into<PathBuf>) -> Self {
        let executable = executable.into();
        self.xe = Some(xe::XeSensors::new(
            executable.clone(),
            "/sys/bus/event_source/devices".into(),
            "/sys/devices/system/cpu/online".into(),
        ));
        self.intel_memory = Some(intel_memory::IntelMemory::new(executable));
        self
    }

    /// Opt in to ASUS WMI CPU core voltage. Some BIOS versions have polling defects;
    /// never load the kernel driver or enable these extra reads implicitly.
    pub fn with_asus_wmi_voltage(mut self) -> Self {
        self.asus_wmi_voltage = true;
        self
    }

    /// Enable CPUFreq using an explicit sysfs root (also usable by fixtures).
    pub fn with_cpufreq_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.cpufreq = Some(cpufreq::CpuFrequency::new(root.into()));
        self
    }

    /// Enable Intel frequency telemetry using an explicit DRM sysfs root.
    pub fn with_intel_frequency_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.intel_clocks = Some(intel_clocks::IntelClocks::new(root.into()));
        self
    }

    /// One sequential sysfs/procfs pass; NVIDIA is appended asynchronously by spawn.
    /// Missing channels disappear from the catalog;
    /// consumers must treat a missing bound ID as unavailable, never remap it.
    pub fn collect(&mut self) -> Vec<SensorReading> {
        self.sync_fps_settings();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut result = BTreeMap::new();
        let mut overflow = false;
        match fs::read_dir(&self.hwmon) {
            Ok(entries) => {
                for (position, entry) in entries.flatten().take(MAX_READINGS + 1).enumerate() {
                    if position == MAX_READINGS {
                        overflow = true;
                        break;
                    }
                    let chip = entry.path();
                    let Ok(name) = text(&chip.join("name")) else {
                        continue;
                    };
                    let name = name.trim();
                    let known_temperature_driver = matches!(
                        name,
                        "k10temp"
                            | "coretemp"
                            | "zenpower"
                            | "amdgpu"
                            | "nouveau"
                            | "i915"
                            | "xe"
                            | "spd5118"
                    );
                    // Driver names enter IDs. Unknown drivers may expose standard
                    // fan RPM, but must not inject separators or unbounded names.
                    if name.is_empty()
                        || name.len() > 64
                        || !name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    {
                        continue;
                    }
                    // Reject missing physical identity instead of binding to hwmonN.
                    let Ok(device) = fs::canonicalize(chip.join("device")) else {
                        continue;
                    };
                    use std::os::unix::ffi::OsStrExt;
                    let identity: String = device
                        .as_os_str()
                        .as_bytes()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect();
                    if name == "amdgpu" {
                        let rows = amdgpu_readings(&device, &identity, now);
                        if result.len() + rows.len() > MAX_READINGS - 3 {
                            overflow = true;
                            break;
                        }
                        for row in rows {
                            if let Some(previous) = result.get_mut(&row.id) {
                                *previous = reading(
                                    row.id,
                                    row.label,
                                    row.unit,
                                    Err(SensorStatus::Invalid),
                                    now,
                                );
                            } else {
                                result.insert(row.id.clone(), row);
                            }
                        }
                    }
                    let Ok(channels) = fs::read_dir(&chip) else {
                        continue;
                    };
                    for (position, channel) in
                        channels.flatten().take(MAX_HWMON_FILES + 1).enumerate()
                    {
                        if position == MAX_HWMON_FILES {
                            overflow = true;
                            break;
                        }
                        let filename = channel.file_name();
                        let Some(filename) = filename.to_str() else {
                            continue;
                        };
                        // AMDGPU documents these rails and clocks directly; arbitrary
                        // motherboard inN channels may need board-specific scaling.
                        let amdgpu_channel = if name == "amdgpu" {
                            match filename {
                                "in0_input" => Some(("0", "in", SensorUnit::Volts, "input")),
                                "in1_input" => Some(("1", "in", SensorUnit::Volts, "input")),
                                "freq1_input" => Some(("1", "freq", SensorUnit::Hertz, "input")),
                                "freq2_input" => Some(("2", "freq", SensorUnit::Hertz, "input")),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        // i915 exposes GT voltage at in0; xe's package channel is
                        // in1 (in0 is card). Only these source-confirmed rails.
                        // WMI channels are enumerated by firmware. Only the documented exact
                        // CPU rail label qualifies; CPU SOC/VID and unlabelled pins do not.
                        let wmi_voltage = if self.asus_wmi_voltage && name == "asus_wmi_sensors" {
                            filename
                                .strip_prefix("in")
                                .and_then(|rest| rest.strip_suffix("_input"))
                                .filter(|index| {
                                    !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit())
                                })
                                .filter(|index| {
                                    text(&chip.join(format!("in{index}_label")))
                                        .is_ok_and(|label| label.trim() == "CPU Core Voltage")
                                })
                                .map(|index| (index, "in", SensorUnit::Volts, "input"))
                        } else {
                            None
                        };
                        // Pinned zenpower/zenpower3 ABI: in1 is SVI2 Core, in2
                        // is SoC; in0 is deliberately hidden. Require the exact
                        // source labels as well as driver/channel binding.
                        let zenpower_voltage = if name == "zenpower"
                            && filename == "in1_input"
                            && text(&chip.join("in1_label")).is_ok_and(|s| {
                                matches!(
                                    s.trim(),
                                    "SVI2_Core" | "cpu0 SVI2_Core" | "cpu1 SVI2_Core"
                                )
                            }) {
                            Some(("1", "in", SensorUnit::Volts, "input"))
                        } else {
                            None
                        };
                        let driver_voltage = match (name, filename) {
                            // asus-ec-sensors has one voltage channel: CPU Core, already mV.
                            ("asusec", "in0_input") => {
                                Some(("0", "in", SensorUnit::Volts, "input"))
                            }
                            (driver, "in0_input") if is_nct_cpu_voltage_driver(driver) => {
                                Some(("0", "in", SensorUnit::Volts, "input"))
                            }
                            // Linux nouveau_hwmon.c: one GPU core input, nvkm_volt_get uV /1000.
                            ("i915" | "nouveau", "in0_input") => {
                                Some(("0", "in", SensorUnit::Volts, "input"))
                            }
                            ("xe", "in1_input") => Some(("1", "in", SensorUnit::Volts, "input")),
                            _ => None,
                        };
                        let channel = filename
                            .strip_prefix("temp")
                            .and_then(|s| s.strip_suffix("_input"))
                            .filter(|_| known_temperature_driver)
                            .map(|index| (index, "temp", SensorUnit::Celsius, "input"))
                            .or_else(|| {
                                filename
                                    .strip_prefix("fan")?
                                    .strip_suffix("_input")
                                    .map(|index| (index, "fan", SensorUnit::Rpm, "input"))
                            })
                            .or_else(|| {
                                if !matches!(name, "amdgpu" | "nouveau" | "i915" | "xe") {
                                    return None;
                                }
                                let rest = filename.strip_prefix("power")?;
                                for suffix in ["input", "average"] {
                                    if let Some(index) = rest.strip_suffix(&format!("_{suffix}")) {
                                        return Some((index, "power", SensorUnit::Watts, suffix));
                                    }
                                }
                                None
                            })
                            .or(amdgpu_channel)
                            .or(driver_voltage)
                            .or(wmi_voltage)
                            .or(zenpower_voltage);
                        let Some((index, kind, unit, measure)) = channel else {
                            continue;
                        };
                        if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
                            continue;
                        }
                        if index
                            .parse::<u16>()
                            .ok()
                            .filter(|n| *n > 0 || kind == "in")
                            .is_none()
                        {
                            continue;
                        }
                        if result.len() >= MAX_READINGS - 3 {
                            overflow = true;
                            break;
                        }
                        let prefix = format!("{kind}{index}");
                        // Input and average are separate bindings, never substituted.
                        let identity_channel =
                            if name == "asus_wmi_sensors" && unit == SensorUnit::Volts {
                                // Duplicate firmware labels collide deliberately and become invalid.
                                "cpu_core_voltage".to_owned()
                            } else if unit == SensorUnit::Watts {
                                filename.to_owned()
                            } else {
                                prefix.clone()
                            };
                        let id = format!("hwmon:{identity}:{name}:{identity_channel}");
                        let label = text(&chip.join(format!("{prefix}_label")))
                            .ok()
                            .map(|s| s.trim().to_owned())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| match (name, prefix.as_str()) {
                                ("asusec", "in0") => "CPU core voltage".into(),
                                (driver, "in0") if is_nct_cpu_voltage_driver(driver) => {
                                    "CPU core voltage (CPUVC)".into()
                                }
                                ("amdgpu", "in0") => "GPU voltage".into(),
                                ("i915", "in0") => "GPU GT voltage".into(),
                                ("nouveau", "in0") => "GPU core voltage".into(),
                                ("xe", "in1") => "GPU package voltage".into(),
                                ("amdgpu", "in1") => "Northbridge voltage".into(),
                                ("amdgpu", "freq1") => "GFX clock".into(),
                                ("amdgpu", "freq2") => "Memory clock".into(),
                                _ => prefix.clone(),
                            });
                        let value = match unit {
                            SensorUnit::Watts => {
                                hwmon_scalar(&chip, &prefix, filename, 1_000_000.0)
                            }
                            SensorUnit::Volts => hwmon_scalar(&chip, &prefix, filename, 1000.0),
                            SensorUnit::Hertz => hwmon_scalar(&chip, &prefix, filename, 1.0),
                            SensorUnit::Rpm => fan_rpm(&chip, &prefix, filename),
                            _ => temperature(&chip, &prefix),
                        };
                        let label = if unit == SensorUnit::Watts {
                            format!("{label} ({measure})")
                        } else {
                            label
                        };
                        let row = reading(id.clone(), format!("{name}: {label}"), unit, value, now);
                        // Multiple channels with the same identity are ambiguous.
                        if let Some(previous) = result.get_mut(&id) {
                            *previous =
                                reading(id, row.label, row.unit, Err(SensorStatus::Invalid), now);
                        } else {
                            result.insert(id, row);
                        }
                    }
                }
            }
            Err(_) => {
                result.insert(
                    "hwmon:discovery".into(),
                    reading(
                        "hwmon:discovery",
                        "hwmon discovery unavailable",
                        SensorUnit::Celsius,
                        Err(SensorStatus::Unavailable),
                        now,
                    ),
                );
            }
        }
        if overflow {
            // Fail the whole hwmon catalog rather than publishing an arbitrary
            // directory-order-dependent subset that could silently change IDs.
            result.clear();
            result.insert(
                "hwmon:overflow".into(),
                reading(
                    "hwmon:overflow",
                    "hwmon catalog exceeds capacity",
                    SensorUnit::Celsius,
                    Err(SensorStatus::Invalid),
                    now,
                ),
            );
        }
        let cpu = first_line(&self.proc.join("stat")).and_then(|s| cpu_counters(&s));
        let load = match cpu {
            Ok(current) => {
                let previous = self.cpu_previous.replace(current);
                previous
                    .and_then(|p| {
                        let total = current.0.checked_sub(p.0)?;
                        let idle = current.1.checked_sub(p.1)?;
                        (total > 0 && idle <= total)
                            .then(|| 100.0 * (total - idle) as f64 / total as f64)
                    })
                    .ok_or(SensorStatus::Unavailable)
            }
            Err(e) => {
                self.cpu_previous = None;
                Err(e)
            }
        };
        result.insert(
            "proc:cpu:utilization".into(),
            reading(
                "proc:cpu:utilization",
                "CPU utilization",
                SensorUnit::Percent,
                load,
                now,
            ),
        );
        let memory = text(&self.proc.join("meminfo")).and_then(|s| memory_usage(&s));
        result.insert(
            "proc:memory:used".into(),
            reading(
                "proc:memory:used",
                "Memory used (total - available)",
                SensorUnit::Bytes,
                memory.map(|(used, _)| used as f64),
                now,
            ),
        );
        result.insert(
            "proc:memory:utilization".into(),
            reading(
                "proc:memory:utilization",
                "Memory utilization",
                SensorUnit::Percent,
                memory.map(|(used, total)| 100.0 * used as f64 / total as f64),
                now,
            ),
        );
        let mut rows: Vec<_> = result.into_values().collect();
        if let Some(source) = &mut self.mangohud {
            rows.push(source.collect(now));
        }
        if let Some(network) = &mut self.network {
            rows.extend(network.collect(now, Instant::now()));
        }
        if let Some(clocks) = &self.intel_clocks {
            rows.extend(clocks.collect(now));
        }
        if let Some(cpufreq) = &self.cpufreq {
            rows.extend(cpufreq.collect(now));
        }
        if let Some(power) = &mut self.power {
            rows.extend(power.collect(now, Instant::now()));
        }
        if let Some(intel) = &mut self.intel {
            rows.extend(intel.collect(now, Instant::now()));
        }
        rows
    }
}

/// AMDGPU's device sysfs ABI reports busy percent and VRAM bytes.
/// Missing attributes remain explicit unavailable readings for the discovered GPU.
/// https://docs.kernel.org/gpu/amdgpu/driver-misc.html
/// https://docs.kernel.org/gpu/amdgpu/thermal.html
fn amdgpu_readings(device: &Path, identity: &str, now: u64) -> Vec<SensorReading> {
    let load = number(&device.join("gpu_busy_percent")).and_then(|v| {
        if (0..=100).contains(&v) {
            Ok(v as f64)
        } else {
            Err(SensorStatus::Invalid)
        }
    });
    let unsigned = |name: &str| -> Result<u64, SensorStatus> {
        text(&device.join(name))?
            .trim()
            .parse()
            .map_err(|_| SensorStatus::Invalid)
    };
    let total = unsigned("mem_info_vram_total").and_then(|v| {
        if v > 0 {
            Ok(v)
        } else {
            Err(SensorStatus::Unavailable)
        }
    });
    let used = unsigned("mem_info_vram_used").and_then(|used| match total {
        Ok(total) if used > total => Err(SensorStatus::Invalid),
        _ => Ok(used),
    });
    let percent = used.and_then(|used| total.map(|total| 100.0 * used as f64 / total as f64));
    let label = device.file_name().unwrap_or_default().to_string_lossy();
    [
        ("utilization", "GPU utilization", SensorUnit::Percent, load),
        (
            "vram_used",
            "VRAM used",
            SensorUnit::Bytes,
            used.map(|v| v as f64),
        ),
        (
            "vram_total",
            "VRAM total",
            SensorUnit::Bytes,
            total.map(|v| v as f64),
        ),
        (
            "vram_utilization",
            "VRAM utilization",
            SensorUnit::Percent,
            percent,
        ),
    ]
    .into_iter()
    .map(|(suffix, metric, unit, value)| {
        reading(
            format!("amdgpu:{identity}:{suffix}"),
            format!("amdgpu {label}: {metric}"),
            unit,
            value,
            now,
        )
    })
    .collect()
}

/// Nonnegative hwmon scalar with optional enable flag and an explicit ABI scale.
fn hwmon_scalar(
    chip: &Path,
    prefix: &str,
    filename: &str,
    scale: f64,
) -> Result<f64, SensorStatus> {
    let enabled = chip.join(format!("{prefix}_enable"));
    match fs::symlink_metadata(&enabled) {
        Ok(_) => match number(&enabled)? {
            1 => {}
            0 => return Err(SensorStatus::Unavailable),
            _ => return Err(SensorStatus::Invalid),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(SensorStatus::Unavailable),
    }
    let value = number(&chip.join(filename))?;
    if value < 0 {
        return Err(SensorStatus::Invalid);
    }
    Ok(value as f64 / scale)
}

/// A stopped fan is valid zero; fault/disabled/malformed readings are not.
fn fan_rpm(chip: &Path, prefix: &str, filename: &str) -> Result<f64, SensorStatus> {
    let fault = chip.join(format!("{prefix}_fault"));
    match fs::symlink_metadata(&fault) {
        Ok(_) => match number(&fault)? {
            0 => {}
            1 => return Err(SensorStatus::Unavailable),
            _ => return Err(SensorStatus::Invalid),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(SensorStatus::Unavailable),
    }
    // Kernel drivers already account for fan_div/pulses. Do not scale again.
    hwmon_scalar(chip, prefix, filename, 1.0)
}

fn temperature(chip: &Path, prefix: &str) -> Result<f64, SensorStatus> {
    for (suffix, expected) in [("fault", 0), ("enable", 1)] {
        let path = chip.join(format!("{prefix}_{suffix}"));
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                if number(&path)? != expected {
                    return Err(SensorStatus::Unavailable);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(SensorStatus::Unavailable),
        }
    }
    let value = number(&chip.join(format!("{prefix}_input")))?;
    // Reject impossible values rather than silently clamp them for cooling.
    if !(-273_150..=300_000).contains(&value) {
        return Err(SensorStatus::Invalid);
    }
    Ok(value as f64 / 1000.0)
}

fn cpu_counters(input: &str) -> Result<(u64, u64), SensorStatus> {
    let mut fields = input
        .lines()
        .find(|s| s.starts_with("cpu "))
        .ok_or(SensorStatus::Invalid)?
        .split_whitespace();
    fields.next();
    let values: Vec<u64> = fields
        .take(8)
        .map(|s| s.parse().map_err(|_| SensorStatus::Invalid))
        .collect::<Result<_, _>>()?;
    if values.len() != 8 {
        return Err(SensorStatus::Invalid);
    }
    // guest/guest_nice are already included in user/nice; don't count twice.
    let total = values
        .iter()
        .try_fold(0u64, |a, b| a.checked_add(*b))
        .ok_or(SensorStatus::Invalid)?;
    let idle = values[3]
        .checked_add(values[4])
        .ok_or(SensorStatus::Invalid)?;
    Ok((total, idle))
}

fn memory_usage(input: &str) -> Result<(u64, u64), SensorStatus> {
    let field = |key: &str| -> Result<u64, SensorStatus> {
        let line = input
            .lines()
            .find_map(|s| s.strip_prefix(key))
            .ok_or(SensorStatus::Invalid)?;
        let mut parts = line.split_whitespace();
        let number: u64 = parts
            .next()
            .ok_or(SensorStatus::Invalid)?
            .parse()
            .map_err(|_| SensorStatus::Invalid)?;
        if parts.next() != Some("kB") || parts.next().is_some() {
            return Err(SensorStatus::Invalid);
        }
        number.checked_mul(1024).ok_or(SensorStatus::Invalid)
    };
    let total = field("MemTotal:")?;
    let available = field("MemAvailable:")?;
    if total == 0 {
        return Err(SensorStatus::Invalid);
    }
    Ok((
        total.checked_sub(available).ok_or(SensorStatus::Invalid)?,
        total,
    ))
}

struct Sample {
    started: Instant,
    rows: Vec<SensorReading>,
}

#[derive(Clone, Default)]
pub struct SensorCache(Arc<RwLock<CacheState>>);

#[derive(Default)]
struct CacheState {
    sample: Option<Sample>,
    invalidated_at: Option<Instant>,
    fps_invalidated_at: Option<Instant>,
}

impl SensorCache {
    fn invalidation_boundary(&self) -> Option<Instant> {
        self.0.read().ok().and_then(|state| state.invalidated_at)
    }

    pub fn publish(&self, started: Instant, rows: Vec<SensorReading>) {
        if let Ok(mut guard) = self.0.write() {
            if guard
                .invalidated_at
                .is_some_and(|boundary| started < boundary)
            {
                return;
            }
            guard.sample = Some(Sample { started, rows });
        }
    }
    pub fn invalidate_fps(&self) {
        if let Ok(mut state) = self.0.write() {
            state.fps_invalidated_at = Some(Instant::now());
        }
    }
    /// Invalidate values across sleep/wake, where monotonic time may not advance.
    /// A collector started before this boundary cannot republish an old sample.
    pub fn invalidate(&self) {
        if let Ok(mut guard) = self.0.write() {
            guard.invalidated_at = Some(Instant::now());
            if let Some(sample) = &mut guard.sample {
                for row in &mut sample.rows {
                    if row.status == SensorStatus::Valid {
                        row.status = SensorStatus::Stale;
                        row.value = None;
                    }
                }
            }
        }
    }

    /// Freshness uses monotonic time; wall-clock changes cannot revive samples.
    pub fn readings(&self) -> Vec<SensorReading> {
        let Ok(guard) = self.0.read() else {
            return Vec::new();
        };
        let Some(Sample { started, rows }) = guard.sample.as_ref() else {
            return Vec::new();
        };
        let age = started.elapsed();
        rows.iter()
            .cloned()
            .map(|mut row| {
                if row.unit == SensorUnit::FramesPerSecond
                    && guard.fps_invalidated_at.is_some_and(|b| *started <= b)
                {
                    row.status = SensorStatus::Stale;
                    row.value = None;
                    row.age_ms = None;
                    return row;
                }

                if row.status == SensorStatus::Valid {
                    row.age_ms = Some(age.as_millis().min(u128::from(u64::MAX)) as u64);
                    if age > STALE_AFTER {
                        row.status = SensorStatus::Stale;
                        row.value = None;
                    }
                }
                row
            })
            .collect()
    }
}

/// One collector at a time; join the current blocking read before stopping.
pub fn spawn(
    mut provider: LinuxSensors,
    cache: SensorCache,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_boundary = None;
        loop {
            if *shutdown.borrow() || shutdown.has_changed().is_err() {
                break;
            }
            let collection_cache = cache.clone();
            let result = tokio::task::spawn_blocking(move || {
                // Timestamp first: if invalidation races the boundary read or
                // collection, publish rejects this entire sample. The next pass
                // resets the counters, including asynchronous xe samples.
                let started = Instant::now();
                let boundary = collection_cache.invalidation_boundary();
                if boundary != last_boundary {
                    provider.reset_baselines();
                }
                let rows = provider.collect();
                (provider, started, rows, boundary)
            })
            .await;
            let Ok((returned, started, mut rows, boundary)) = result else {
                break;
            };
            provider = returned;
            last_boundary = boundary;
            if let Some(nvidia) = &mut provider.nvidia {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                rows.extend(nvidia.collect(now).await);
            }
            if let Some(storage) = &mut provider.storage {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                rows.extend(storage.collect(now).await);
            }
            if let Some(intel_memory) = &mut provider.intel_memory {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                rows.extend(intel_memory.collect(now).await);
            }
            if let Some(xe) = &mut provider.xe {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                rows.extend(xe.collect(now).await);
            }
            cache.publish(started, rows);
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {},
            }
        }
    })
}

/// Stable bindings for the target LCDV2, populated only on a real poll response.
pub fn device_readings(key: &str, snapshot: &lconnect_ipc::SensorSnapshot) -> Vec<SensorReading> {
    [
        (
            "coolant",
            SensorUnit::Celsius,
            if snapshot.temp_valid {
                Ok(f64::from(snapshot.coolant_c_x10) / 10.0)
            } else {
                Err(SensorStatus::Invalid)
            },
        ),
        ("fan_rpm", SensorUnit::Rpm, Ok(f64::from(snapshot.fan_rpm))),
        (
            "pump_rpm",
            SensorUnit::Rpm,
            Ok(f64::from(snapshot.pump_rpm)),
        ),
    ]
    .into_iter()
    .map(|(suffix, unit, value)| {
        reading(
            format!("lcdv2:{key}:{suffix}"),
            format!("LCDV2 {suffix}"),
            unit,
            value,
            snapshot.last_seen_unix_ms,
        )
    })
    .collect()
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    #[tokio::test]
    async fn sleep_during_async_collection_discards_result_and_rewarms_cpu_baseline() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let proc = root.path().join("proc");
        fs::create_dir(&proc).unwrap();
        let write_cpu = |text: &str| {
            fs::write(proc.join("stat.new"), text).unwrap();
            fs::rename(proc.join("stat.new"), proc.join("stat")).unwrap();
        };
        let cpu = |rows: Vec<SensorReading>| {
            rows.into_iter()
                .find(|r| r.id == "proc:cpu:utilization")
                .unwrap()
        };
        write_cpu("cpu 10 0 0 90 0 0 0 0\n");
        let mut provider = LinuxSensors::new(root.path().join("hwmon"), &proc);
        provider.collect();
        write_cpu("cpu 20 0 0 180 0 0 0 0\n");
        let cache = SensorCache::default();
        let before = Instant::now();
        let rows = provider.collect();
        assert_eq!(cpu(rows.clone()).value, Some(10.0));
        cache.publish(before, rows);
        // The fake query only gates the async half, after CPU collection. It
        // never invokes NVIDIA tools or reads host hardware.
        let entered = root.path().join("entered");
        let release = root.path().join("release");
        let executable = root.path().join("query");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\n: > '{}'\nwhile [ ! -f '{}' ]; do sleep 0.01; done\n",
                entered.display(),
                release.display(),
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        provider.nvidia = Some(nvidia::NvidiaSensors::new(executable));
        let (stop, shutdown) = watch::channel(false);
        let task = spawn(provider, cache.clone(), shutdown);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !entered.exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        cache.invalidate();
        assert_eq!(cpu(cache.readings()).status, SensorStatus::Stale);
        write_cpu("cpu 90000 0 0 10000 0 0 0 0\n");
        fs::write(release, "continue").unwrap();
        // Releasing the old query must not overwrite Stale with its CPU result.
        // The next full collection must reset even though the prior async pass
        // has already mutated its provider's baseline.
        let warm = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let row = cpu(cache.readings());
                if row.status != SensorStatus::Stale {
                    break row;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(warm.status, SensorStatus::Unavailable);
        assert_eq!(warm.value, None);
        write_cpu("cpu 90010 0 0 10090 0 0 0 0\n");
        let valid = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let row = cpu(cache.readings());
                if row.status == SensorStatus::Valid {
                    break row;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(valid.value, Some(10.0));
        stop.send(true).unwrap();
        task.await.unwrap();
    }

    #[test]
    fn sleep_boundary_invalidates_values_and_rejects_inflight_collection() {
        let cache = SensorCache::default();
        let started = Instant::now();
        let row = reading("fixture:cpu", "CPU", SensorUnit::Celsius, Ok(48.0), 123);
        cache.publish(started, vec![row.clone()]);
        assert_eq!(cache.readings()[0].value, Some(48.0));
        cache.invalidate();
        let stale = cache.readings();
        assert_eq!(stale[0].id, row.id);
        assert_eq!(stale[0].status, SensorStatus::Stale);
        assert_eq!(stale[0].value, None);
        cache.publish(started, vec![row.clone()]);
        assert_eq!(cache.readings()[0].status, SensorStatus::Stale);
        cache.publish(Instant::now(), vec![row]);
        assert_eq!(cache.readings()[0].status, SensorStatus::Valid);
        assert_eq!(cache.readings()[0].value, Some(48.0));
    }

    #[tokio::test]
    async fn nvidia_fixture_flows_through_collector_cache_and_shutdown() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("query");
        fs::write(
            &exe,
            "#!/bin/sh\nprintf '%s\n' 'GPU-12345678-1234-1234-1234-123456789abc,42,37,2048,8192,125.5,1800,7000'\n",
        )
        .unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        let mut provider = LinuxSensors::new(dir.path().join("hwmon"), dir.path().join("proc"));
        provider.nvidia = Some(nvidia::NvidiaSensors::new(exe));
        let cache = SensorCache::default();
        let (tx, rx) = watch::channel(false);
        let task = spawn(provider, cache.clone(), rx);
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(row) = cache
                    .readings()
                    .into_iter()
                    .find(|r| r.id.starts_with("nvidia:") && r.unit == SensorUnit::Celsius)
                {
                    assert_eq!(row.value, Some(42.0));
                    assert!(row.age_ms.unwrap() < 3000);
                    let power = cache
                        .readings()
                        .into_iter()
                        .find(|r| {
                            r.id == "nvidia:GPU-12345678-1234-1234-1234-123456789abc:power_draw"
                        })
                        .expect("power reaches the shared collector cache");
                    assert_eq!(power.unit, SensorUnit::Watts);
                    assert_eq!(power.value, Some(125.5));
                    assert!(power.age_ms.unwrap() < 3000);
                    let readings = cache.readings();
                    for (metric, expected) in [
                        ("graphics_clock", "1800.0 MHz"),
                        ("memory_clock", "7000.0 MHz"),
                    ] {
                        let id =
                            format!("nvidia:GPU-12345678-1234-1234-1234-123456789abc:{metric}");
                        assert_eq!(
                            lconnect_ipc::theme_sensor_text(
                                &readings,
                                &id,
                                SensorUnit::Hertz,
                                lconnect_ipc::TemperatureUnit::Celsius,
                                1
                            ),
                            expected
                        );
                    }
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
    }
}
