//! Classify the daemon's own provider IDs for global defaults. Never infer roles
//! from mutable labels or units alone. Generic hwmon fans are eligible for an
//! explicit CPU assignment only; recommendations never infer that wiring.
//! Keep this registry in sync with providers.
use anyhow::ensure;
use lconnect_ipc::{
    DefaultSensorRole as Role, SensorDefaultCandidate, SensorDefaults, SensorReading,
};
use std::collections::BTreeMap;

pub fn candidates(readings: &[SensorReading]) -> Vec<SensorDefaultCandidate> {
    let mut counts = BTreeMap::new();
    for reading in readings {
        *counts.entry(&reading.id).or_insert(0usize) += 1;
    }
    readings
        .iter()
        .filter_map(|reading| {
            if counts[&reading.id] != 1 {
                return None;
            }
            let (role, gpu_id) = classify(&reading.id)?;
            (reading.unit == role.unit()).then(|| SensorDefaultCandidate {
                role,
                gpu_id,
                reading: reading.clone(),
            })
        })
        .collect()
}

/// Preserve unchanged unavailable bindings, but reject new unknown or cross-GPU
/// choices. Clearing an obsolete choice remains possible with hardware unplugged.
pub fn validate_selection(
    proposed: &SensorDefaults,
    current: &SensorDefaults,
    readings: &[SensorReading],
) -> anyhow::Result<()> {
    proposed.validate()?;
    let choices = candidates(readings);
    let gpu = proposed.gpu.as_ref().map(|g| g.device_id.as_str());
    let old_gpu = current.gpu.as_ref().map(|g| g.device_id.as_str());
    if gpu != old_gpu {
        if let Some(gpu) = gpu {
            ensure!(
                choices.iter().any(|c| c.gpu_id.as_deref() == Some(gpu)),
                "selected GPU is unavailable"
            );
        }
    }
    for role in Role::ALL {
        let Some(id) = proposed.binding(role) else {
            continue;
        };
        let expected_gpu = if is_gpu(role) { gpu } else { None };
        if current.binding(role) == Some(id)
            && (!is_gpu(role) || gpu == old_gpu)
            && !readings.iter().any(|reading| reading.id == id)
        {
            continue;
        }
        ensure!(
            choices.iter().any(|c| c.role == role
                && c.reading.id == id
                && c.gpu_id.as_deref() == expected_gpu),
            "sensor is not a candidate for the selected CPU/GPU role"
        );
    }
    Ok(())
}
fn is_gpu(role: Role) -> bool {
    matches!(
        role,
        Role::GpuTemperature
            | Role::GpuClock
            | Role::GpuLoad
            | Role::GpuPower
            | Role::GpuFan
            | Role::GpuVoltage
            | Role::GpuMemoryUsed
    )
}
fn numbered(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
}
fn bounded_index(value: &str, prefix: &str, max: u16) -> bool {
    value.strip_prefix(prefix).is_some_and(|s| {
        s.parse::<u16>()
            .is_ok_and(|n| n <= max && n.to_string() == s)
    })
}
fn physical(value: &str) -> bool {
    value.starts_with("2f")
        && value.len().is_multiple_of(2)
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn classify(id: &str) -> Option<(Role, Option<String>)> {
    let parts: Vec<_> = id.split(':').collect();
    let cpu = |role| Some((role, None));
    let gpu = |role, physical: &str, driver: &str| {
        Some((role, Some(format!("sysfs:{physical}:{driver}"))))
    };
    match parts.as_slice() {
        ["proc", "cpu", "utilization"] => cpu(Role::CpuLoad),
        ["powercap", "rapl", package, "power"] if numbered(package, "package-") => {
            cpu(Role::CpuPower)
        }
        ["cpufreq", cpus, metric]
            if cpus
                .strip_prefix("cpus-")
                .is_some_and(|v| !v.is_empty() && v.split('.').all(|n| numbered(n, "")))
                && matches!(
                    *metric,
                    "cpuinfo_cur_freq" | "cpuinfo_avg_freq" | "scaling_cur_freq"
                ) =>
        {
            cpu(Role::CpuClock)
        }
        ["hwmon", path, driver, channel] if physical(path) => match *driver {
            "k10temp" | "coretemp" | "zenpower" if numbered(channel, "temp") => {
                cpu(Role::CpuTemperature)
            }
            "asusec" if *channel == "in0" => cpu(Role::CpuVoltage),
            "zenpower" if *channel == "in1" => cpu(Role::CpuVoltage),
            driver if crate::sensors::is_nct_cpu_voltage_driver(driver) && *channel == "in0" => {
                cpu(Role::CpuVoltage)
            }
            "asus_wmi_sensors" if *channel == "cpu_core_voltage" => cpu(Role::CpuVoltage),
            "amdgpu" | "nouveau" | "i915" | "xe" => {
                let role = if numbered(channel, "temp") {
                    Role::GpuTemperature
                } else if channel
                    .strip_suffix("_input")
                    .or_else(|| channel.strip_suffix("_average"))
                    .is_some_and(|c| numbered(c, "power"))
                {
                    Role::GpuPower
                } else if matches!(
                    (*driver, *channel),
                    ("amdgpu" | "i915" | "nouveau", "in0") | ("xe", "in1")
                ) {
                    Role::GpuVoltage
                } else if *driver == "amdgpu" && *channel == "freq1" {
                    Role::GpuClock
                } else {
                    return None;
                };
                gpu(role, path, driver)
            }
            _ if !driver.is_empty()
                && driver.len() <= 64
                && driver
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                && channel.strip_prefix("fan").is_some_and(|n| {
                    !n.is_empty()
                        && n.bytes().all(|b| b.is_ascii_digit())
                        && n.parse::<u16>().is_ok_and(|n| n > 0)
                }) =>
            {
                cpu(Role::CpuFan)
            }
            _ => None,
        },
        ["amdgpu", path, metric] if physical(path) => gpu(
            match *metric {
                "utilization" => Role::GpuLoad,
                "vram_used" => Role::GpuMemoryUsed,
                _ => return None,
            },
            path,
            "amdgpu",
        ),
        ["nvidia", uuid, metric] if valid_uuid(uuid) => Some((
            match *metric {
                "temperature" => Role::GpuTemperature,
                "utilization" => Role::GpuLoad,
                "power_draw" => Role::GpuPower,
                "graphics_clock" => Role::GpuClock,
                "core_voltage" => Role::GpuVoltage,
                "vram_used" => Role::GpuMemoryUsed,
                _ => return None,
            },
            Some(format!("nvidia:{uuid}")),
        )),
        ["intel", path, "i915", "engine", config, "utilization"]
            if physical(path)
                && u64::from_str_radix(config, 16).is_ok_and(|n| {
                    n < 0x10_0000 && n & 0xf == 0 && format!("{n:x}") == *config
                }) =>
        {
            gpu(Role::GpuLoad, path, "i915")
        }
        ["intel", path, "xe", gt, class, instance, "utilization"]
            if physical(path)
                && bounded_index(gt, "gt", 15)
                && bounded_index(class, "class", 4)
                && bounded_index(instance, "instance", 255) =>
        {
            gpu(Role::GpuLoad, path, "xe")
        }
        ["intel-memory", path, driver, "vram_used"]
            if physical(path) && matches!(*driver, "i915" | "xe") =>
        {
            gpu(Role::GpuMemoryUsed, path, driver)
        }
        ["intel-clocks", path, "i915", domain, metric]
            if physical(path)
                && (*domain == "legacy" || numbered(domain, "gt"))
                && matches!(*metric, "actual_frequency" | "requested_frequency") =>
        {
            gpu(Role::GpuClock, path, "i915")
        }
        ["intel-clocks", path, "xe", tile, gt, metric]
            if physical(path)
                && numbered(tile, "tile")
                && numbered(gt, "gt")
                && matches!(*metric, "actual_frequency" | "requested_frequency") =>
        {
            gpu(Role::GpuClock, path, "xe")
        }
        _ => None,
    }
}
fn valid_uuid(id: &str) -> bool {
    let Some(uuid) = id.strip_prefix("GPU-") else {
        return false;
    };
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit() && !b.is_ascii_uppercase()
            }
        })
}

/// A fresh, deterministic proposal only. Never persist or remap active bindings
/// during polling. Multiple physical GPUs use stable ID order, not enumeration.
pub fn recommended(readings: &[SensorReading], revision: u64) -> SensorDefaults {
    use lconnect_ipc::{CpuSensorDefaults, GpuSensorDefaults, SensorStatus};
    let mut choices = candidates(readings);
    choices.retain(|c| {
        c.reading.status == SensorStatus::Valid
            && c.reading.age_ms.is_some_and(|age| age <= 3000)
            && c.reading.value.is_some_and(f64::is_finite)
    });
    choices.sort_by(|a, b| {
        priority(&a.reading.id)
            .cmp(&priority(&b.reading.id))
            .then_with(|| a.reading.id.cmp(&b.reading.id))
    });
    let gpu_id = choices
        .iter()
        .filter_map(|c| c.gpu_id.as_ref())
        .min()
        .cloned();
    let pick = |role: Role| {
        choices
            .iter()
            .find(|c| {
                c.role == role && (!is_gpu(role) || c.gpu_id == gpu_id)
                // A selected engine is meaningful, but not an automatic whole-GPU estimate.
                && !(role == Role::GpuLoad && c.reading.id.starts_with("intel:"))
            })
            .map(|c| c.reading.id.clone())
    };
    SensorDefaults {
        version: 1,
        revision,
        cpu: CpuSensorDefaults {
            temperature: pick(Role::CpuTemperature),
            clock: pick(Role::CpuClock),
            load: pick(Role::CpuLoad),
            power: pick(Role::CpuPower),
            // Motherboard headers require the user to identify physical wiring.
            fan: None,
            voltage: pick(Role::CpuVoltage),
        },
        gpu: gpu_id.clone().map(|device_id| GpuSensorDefaults {
            device_id,
            temperature: pick(Role::GpuTemperature),
            clock: pick(Role::GpuClock),
            load: pick(Role::GpuLoad),
            power: pick(Role::GpuPower),
            fan: pick(Role::GpuFan),
            voltage: pick(Role::GpuVoltage),
            memory_used: pick(Role::GpuMemoryUsed),
        }),
    }
}
fn priority(id: &str) -> u8 {
    // Prefer measured clocks to driver requests; expose the chosen ID for review.
    if id.ends_with(":scaling_cur_freq") || id.ends_with(":requested_frequency") {
        2
    } else if id.ends_with(":cpuinfo_avg_freq") {
        1
    } else {
        0
    }
}
