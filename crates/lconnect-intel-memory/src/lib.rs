//! Read-only Intel DRM memory queries for the isolated daemon helper.
//!
//! No HID, configuration, privilege escalation or GPU allocation. Call only in
//! a process with a supervising timeout: a DRM ioctl may block in the kernel.
//! ABI and limitations: docs/development/parity/intel-memory-query.md.
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsafe_code)]

mod engines;
#[allow(unsafe_code)]
mod ioctl;
mod regions;

use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io::Read, path::Path};

/// Maximum devices in one helper response.
pub const MAX_DEVICES: usize = 32;
/// Only these kernel drivers have a supported query layout.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Driver {
    /// i915 MEMORY_REGIONS query.
    I915,
    /// xe MEM_REGIONS query.
    Xe,
}
impl Driver {
    /// Kernel driver name, also part of the persistent sensor identity.
    pub fn name(self) -> &'static str {
        match self {
            Self::I915 => "i915",
            Self::Xe => "xe",
        }
    }
}
/// Query failure without substituting another GPU's reading.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryError {
    /// Missing device, permissions, unsupported ABI or no dedicated memory.
    Unavailable,
    /// Invalid, ambiguous or oversized discovery/query response.
    Invalid,
}
/// Sum of dedicated regions only; SYSMEM is never included.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    /// Dedicated capacity in bytes.
    pub total: u64,
    /// Estimated usage; None when zero cannot be distinguished from redaction.
    pub used: Option<u64>,
}
/// A device's result, including failed queries for independently found devices.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    /// Hex encoded canonical sysfs device path, shared with clock/hwmon IDs.
    pub physical: String,
    /// Bound kernel driver.
    pub driver: Driver,
    /// Dedicated memory or explicit failure.
    pub memory: Result<Memory, QueryError>,
}
/// Physical xe engine coordinates required by the PMU config.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// GT ID within this PCI device.
    pub gt: u16,
    /// Render/copy/video decode/video enhance/compute class, 0 through 4.
    pub class: u16,
    /// Engine instance within the class and GT.
    pub instance: u16,
}
/// Query result used by the persistent xe PMU collector.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EngineDevice {
    /// Hex encoded canonical physical device path.
    pub physical: String,
    /// PCI address used to locate the qualified xe PMU.
    pub pci: String,
    /// Engines or explicit query failure.
    pub engines: Result<Vec<Engine>, QueryError>,
}
/// Discover xe engines in a timeout-supervised helper process only.
pub fn collect_engines() -> Result<Vec<EngineDevice>, QueryError> {
    query_devices(
        Path::new("/sys/class/drm"),
        Path::new("/dev/dri"),
        |path, dev, driver| {
            if driver == Driver::Xe {
                ioctl::query_engines(path, dev)
            } else {
                Ok(Vec::new())
            }
        },
    )
    .map(|devices| {
        devices
            .into_iter()
            .filter(|d| d.driver == Driver::Xe)
            .map(|d| EngineDevice {
                physical: d.physical,
                pci: d.pci,
                engines: d.result,
            })
            .collect()
    })
}
struct DeviceQuery<T> {
    physical: String,
    pci: String,
    driver: Driver,
    result: Result<T, QueryError>,
}
/// Discover and query Intel render nodes. Must run in the isolated helper only.
pub fn collect() -> Result<Vec<Device>, QueryError> {
    collect_at(
        Path::new("/sys/class/drm"),
        Path::new("/dev/dri"),
        ioctl::query,
    )
}

fn bounded_text(path: &Path) -> Result<String, QueryError> {
    let mut text = String::new();
    fs::File::open(path)
        .map_err(|_| QueryError::Unavailable)?
        .take(4097)
        .read_to_string(&mut text)
        .map_err(|_| QueryError::Unavailable)?;
    if text.len() > 4096 {
        return Err(QueryError::Invalid);
    }
    Ok(text)
}
fn binding(node: &Path) -> Result<(std::path::PathBuf, Option<Driver>), QueryError> {
    let device = fs::canonicalize(node.join("device")).map_err(|_| QueryError::Unavailable)?;
    let driver = fs::canonicalize(device.join("driver")).map_err(|_| QueryError::Unavailable)?;
    let driver = match driver.file_name().and_then(|s| s.to_str()) {
        Some("i915") => Some(Driver::I915),
        Some("xe") => Some(Driver::Xe),
        _ => None,
    };
    Ok((device, driver))
}
fn collect_at(
    root: &Path,
    nodes: &Path,
    query: impl FnMut(&Path, u64, Driver) -> Result<Memory, QueryError>,
) -> Result<Vec<Device>, QueryError> {
    query_devices(root, nodes, query).map(|devices| {
        devices
            .into_iter()
            .map(|d| Device {
                physical: d.physical,
                driver: d.driver,
                memory: d.result,
            })
            .collect()
    })
}
fn query_devices<T>(
    root: &Path,
    nodes: &Path,
    mut query: impl FnMut(&Path, u64, Driver) -> Result<T, QueryError>,
) -> Result<Vec<DeviceQuery<T>>, QueryError> {
    use std::os::unix::ffi::OsStrExt;
    let mut entries = Vec::new();
    for (index, entry) in fs::read_dir(root)
        .map_err(|_| QueryError::Unavailable)?
        .enumerate()
    {
        if index >= 256 {
            return Err(QueryError::Invalid);
        }
        let entry = entry.map_err(|_| QueryError::Unavailable)?;
        let name = entry.file_name();
        if name
            .to_str()
            .and_then(|s| s.strip_prefix("renderD"))
            .is_some_and(|s| {
                s.parse::<u32>()
                    .is_ok_and(|n| n >= 128 && n.to_string() == s)
            })
        {
            entries.push(entry.path());
        }
    }
    entries.sort();
    let mut seen = BTreeSet::new();
    let mut devices = Vec::new();
    for node in entries {
        let (device, Some(driver)) = binding(&node)? else {
            continue;
        };
        if !seen.insert(device.clone()) || seen.len() > MAX_DEVICES {
            return Err(QueryError::Invalid);
        }
        let physical: String = device
            .as_os_str()
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if physical.len() > 900 {
            return Err(QueryError::Invalid);
        }
        let memory = (|| {
            let dev = bounded_text(&node.join("dev"))?;
            let (major, minor) = dev.trim().split_once(':').ok_or(QueryError::Invalid)?;
            let major: u32 = major.parse().map_err(|_| QueryError::Invalid)?;
            let minor: u32 = minor.parse().map_err(|_| QueryError::Invalid)?;
            // Render devices are DRM major 226, with render minors >= 128.
            if major != 226 || minor < 128 {
                return Err(QueryError::Invalid);
            }
            let name = node.file_name().ok_or(QueryError::Invalid)?;
            let result = query(&nodes.join(name), libc::makedev(major, minor), driver);
            // Hotplug/rebind while querying must not attach a new node to old ID.
            if binding(&node)? != (device.clone(), Some(driver)) {
                return Err(QueryError::Unavailable);
            }
            result
        })();
        let pci = device
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(QueryError::Invalid)?
            .to_owned();
        devices.push(DeviceQuery {
            physical,
            pci,
            driver,
            result: memory,
        });
    }
    devices.sort_by(|a, b| a.physical.cmp(&b.physical));
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    fn node(root: &Path, name: &str, pci: &str, driver: &str) {
        let device = root.join("devices").join(pci);
        fs::create_dir_all(&device).unwrap();
        fs::create_dir_all(root.join("drivers").join(driver)).unwrap();
        if !device.join("driver").exists() {
            symlink(root.join("drivers").join(driver), device.join("driver")).unwrap();
        }
        let node = root.join("drm").join(name);
        fs::create_dir_all(&node).unwrap();
        symlink(device, node.join("device")).unwrap();
        fs::write(
            node.join("dev"),
            format!("226:{}", name.strip_prefix("renderD").unwrap()),
        )
        .unwrap();
    }
    #[test]
    fn physical_binding_survives_reorder_and_independent_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        node(root, "renderD128", "0000:03:00.0", "i915");
        node(root, "renderD129", "0000:04:00.0", "xe");
        let query = |_: &Path, _: u64, driver| {
            if driver == Driver::I915 {
                Ok(Memory {
                    total: 4096,
                    used: Some(1024),
                })
            } else {
                Err(QueryError::Unavailable)
            }
        };
        let before = collect_at(&root.join("drm"), root, query).unwrap();
        fs::rename(root.join("drm/renderD128"), root.join("drm/renderD130")).unwrap();
        fs::write(root.join("drm/renderD130/dev"), "226:130").unwrap();
        let after = collect_at(&root.join("drm"), root, query).unwrap();
        assert_eq!(
            before
                .iter()
                .map(|d| (&d.physical, &d.memory))
                .collect::<Vec<_>>(),
            after
                .iter()
                .map(|d| (&d.physical, &d.memory))
                .collect::<Vec<_>>()
        );
        node(root, "renderD131", "0000:03:00.0", "i915");
        assert!(matches!(
            collect_at(&root.join("drm"), root, query),
            Err(QueryError::Invalid)
        ));
    }
    #[test]
    fn node_rebinding_during_query_is_unavailable() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        node(root, "renderD128", "0000:03:00.0", "i915");
        let rows = collect_at(&root.join("drm"), root, |_, _, _| {
            fs::remove_file(root.join("drm/renderD128/device")).unwrap();
            Err(QueryError::Invalid)
        })
        .unwrap();
        assert_eq!(rows[0].memory, Err(QueryError::Unavailable));
    }
}
