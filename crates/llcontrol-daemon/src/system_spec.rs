//! Bounded read-only Linux inventory. Explicit roots support offline fixtures.
use llcontrol_ipc::{
    SpecField as Field, SpecGroup as Group, SpecItem as Item, SpecKind as Kind, SpecValue as Value,
    SystemSpec,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod binary;
#[derive(Clone)]
pub struct LinuxSpec {
    pub proc_root: PathBuf,
    pub sys_root: PathBuf,
    pub pci_ids: Vec<PathBuf>,
}
impl Default for LinuxSpec {
    fn default() -> Self {
        Self {
            proc_root: "/proc".into(),
            sys_root: "/sys".into(),
            pci_ids: vec![
                "/usr/share/hwdata/pci.ids".into(),
                "/usr/share/misc/pci.ids".into(),
            ],
        }
    }
}
fn read(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW).bits() as i32)
        .open(path)
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= limit).then_some(bytes)
}
fn clean(s: &str) -> Option<String> {
    let s = s.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    if s.is_empty()
        || s.len() > 2048
        || s.chars().any(char::is_control)
        || [
            "unknown",
            "not specified",
            "default string",
            "to be filled by o.e.m.",
            "none",
        ]
        .contains(&s.to_ascii_lowercase().as_str())
    {
        None
    } else {
        Some(s.into())
    }
}
fn text(path: &Path) -> Option<String> {
    clean(std::str::from_utf8(&read(path, 2048)?).ok()?)
}
fn number(path: &Path) -> Option<u64> {
    text(path)?.parse().ok()
}
fn dirs(path: &Path, max: usize) -> Option<Vec<PathBuf>> {
    let mut v = fs::read_dir(path)
        .ok()?
        .take(max + 1)
        .map(|e| e.ok().map(|e| e.path()))
        .collect::<Option<Vec<_>>>()?;
    if v.len() > max {
        return None;
    }
    v.sort();
    Some(v)
}
fn field(key: &str, value: Option<Value>, source: &str) -> Field {
    Field {
        key: key.into(),
        unavailable_reason: value
            .is_none()
            .then(|| "Not reported or not accessible from this Linux provider".into()),
        value,
        source: source.into(),
    }
}
fn txt(key: &str, value: Option<String>, source: &str) -> Field {
    field(key, value.map(Value::Text), source)
}
fn group(kind: Kind, items: Vec<Item>) -> Group {
    Group {
        kind,
        unavailable_reason: items
            .is_empty()
            .then(|| "No supported, accessible inventory records were found".into()),
        items,
    }
}
impl LinuxSpec {
    pub fn collect(&self) -> llcontrol_ipc::Result<SystemSpec> {
        let result = SystemSpec {
            version: 1,
            collected_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(u64::MAX as u128) as u64,
            groups: vec![
                self.cpu(),
                self.gpu(),
                self.memory(),
                self.monitors(),
                self.board(),
                self.storage(),
            ],
        };
        result.validate()?;
        Ok(result)
    }
    fn cpu(&self) -> Group {
        let Some(bytes) = read(&self.proc_root.join("cpuinfo"), 1024 * 1024) else {
            return group(Kind::Cpu, vec![]);
        };
        let Ok(input) = std::str::from_utf8(&bytes) else {
            return group(Kind::Cpu, vec![]);
        };
        let mut packages: BTreeMap<String, Vec<BTreeMap<&str, &str>>> = BTreeMap::new();
        for part in input.split("\n\n") {
            let row: BTreeMap<_, _> = part
                .lines()
                .filter_map(|line| line.split_once(':').map(|(k, v)| (k.trim(), v.trim())))
                .collect();
            if row
                .get("processor")
                .is_none_or(|v| v.parse::<u32>().is_err())
            {
                continue;
            }
            packages
                .entry(
                    row.get("physical id")
                        .and_then(|id| id.parse::<u32>().ok())
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "unassigned".into()),
                )
                .or_default()
                .push(row);
        }
        if packages.len() > 128 {
            return group(Kind::Cpu, vec![]);
        }
        group(
            Kind::Cpu,
            packages
                .into_iter()
                .map(|(id, rows)| {
                    let first = &rows[0];
                    let get = |key: &str| first.get(key).and_then(|s| clean(s));
                    let name = get("model name")
                        .or_else(|| get("Processor"))
                        .or_else(|| get("Hardware"));
                    let threads = rows
                        .iter()
                        .filter_map(|r| r.get("processor"))
                        .collect::<BTreeSet<_>>()
                        .len();
                    Item {
                        id: format!("cpu:{id}"),
                        title: name.clone().unwrap_or_else(|| format!("CPU {id}")),
                        fields: vec![
                            txt("model", name, "proc/cpuinfo"),
                            txt("vendor", get("vendor_id"), "proc/cpuinfo"),
                            txt("socket", (id != "unassigned").then_some(id), "proc/cpuinfo"),
                            field(
                                "logical_threads",
                                Some(Value::Count(threads as u64)),
                                "proc/cpuinfo",
                            ),
                            field(
                                "physical_cores",
                                get("cpu cores")
                                    .and_then(|s| s.parse::<u64>().ok())
                                    .filter(|v| *v > 0)
                                    .map(Value::Count),
                                "proc/cpuinfo",
                            ),
                        ],
                    }
                })
                .collect(),
        )
    }
    fn gpu(&self) -> Group {
        let entries = dirs(&self.sys_root.join("bus/pci/devices"), 4096).unwrap_or_default();
        let db = self
            .pci_ids
            .iter()
            .find_map(|p| read(p, 8 * 1024 * 1024))
            .and_then(|v| String::from_utf8(v).ok())
            .unwrap_or_default();
        let mut items = vec![];
        for p in entries {
            if !text(&p.join("class")).is_some_and(|s| s.starts_with("0x03")) {
                continue;
            }
            let vendor = text(&p.join("vendor"));
            let device = text(&p.join("device"));
            let model = vendor.as_ref().zip(device.as_ref()).and_then(|(v, d)| {
                pci_name(&db, v.trim_start_matches("0x"), d.trim_start_matches("0x"))
            });
            let id = p.file_name().unwrap().to_string_lossy();
            let driver = fs::read_link(p.join("driver"))
                .ok()
                .and_then(|p| p.file_name().and_then(|s| s.to_str()).and_then(clean));
            items.push(Item {
                id: format!("pci:{id}"),
                title: model.clone().unwrap_or_else(|| format!("PCI GPU {id}")),
                fields: vec![
                    txt("model", model, "pci.ids"),
                    txt("vendor_id", vendor, "sysfs/pci"),
                    txt("device_id", device, "sysfs/pci"),
                    txt("driver", driver, "sysfs/pci"),
                    txt("revision", text(&p.join("revision")), "sysfs/pci"),
                ],
            });
        }
        if items.len() > 128 {
            items.clear();
        }
        group(Kind::Gpu, items)
    }
    fn memory(&self) -> Group {
        let total = read(&self.proc_root.join("meminfo"), 64 * 1024)
            .and_then(|v| String::from_utf8(v).ok())
            .and_then(|s| {
                s.lines().find_map(|l| {
                    l.strip_prefix("MemTotal:").and_then(|v| {
                        let mut words = v.split_whitespace();
                        let n = words.next()?.parse::<u64>().ok()?;
                        (words.next() == Some("kB"))
                            .then(|| n.checked_mul(1024))
                            .flatten()
                    })
                })
            });
        let mut modules = vec![];
        let mut modules_overflow = false;
        for p in dirs(&self.sys_root.join("firmware/dmi/entries"), 2048).unwrap_or_default() {
            if p.file_name().unwrap().to_string_lossy().starts_with("17-") {
                if let Some(bytes) = read(&p.join("raw"), 64 * 1024) {
                    if let Some(item) =
                        binary::dimm(&bytes, &p.file_name().unwrap().to_string_lossy())
                    {
                        if modules.len() == 127 {
                            modules_overflow = true;
                            break;
                        }
                        modules.push(item);
                    }
                }
            }
        }
        let mut items = vec![Item {
            id: "memory:os".into(),
            title: "OS-visible memory".into(),
            fields: vec![
                field(
                    "os_visible_capacity",
                    total.filter(|v| *v > 0).map(Value::Bytes),
                    "proc/meminfo",
                ),
                field(
                    "reported_modules",
                    (!modules_overflow && !modules.is_empty())
                        .then_some(Value::Count(modules.len() as u64)),
                    "SMBIOS type17",
                ),
            ],
        }];
        if !modules_overflow {
            items.extend(modules);
        }
        group(Kind::Memory, items)
    }
    fn monitors(&self) -> Group {
        let mut items = vec![];
        for p in dirs(&self.sys_root.join("class/drm"), 512).unwrap_or_default() {
            if text(&p.join("status")).as_deref() != Some("connected") {
                continue;
            }
            let id = p.file_name().unwrap().to_string_lossy().into_owned();
            let decoded = read(&p.join("edid"), 32768).and_then(|b| binary::edid(&b));
            let (name, fields) =
                decoded.unwrap_or_else(|| (None, vec![txt("model", None, "DRM EDID")]));
            items.push(Item {
                id: format!("monitor:{id}"),
                title: name.unwrap_or_else(|| id.clone()),
                fields: std::iter::once(txt("connector", Some(id), "sysfs/drm"))
                    .chain(fields)
                    .collect(),
            });
        }
        if items.len() > 128 {
            items.clear();
        }
        group(Kind::Monitors, items)
    }
    fn board(&self) -> Group {
        let root = self.sys_root.join("class/dmi/id");
        let fields = [
            ("model", "board_name"),
            ("manufacturer", "board_vendor"),
            ("version", "board_version"),
            ("bios_vendor", "bios_vendor"),
            ("bios_version", "bios_version"),
            ("bios_date", "bios_date"),
        ]
        .into_iter()
        .map(|(key, file)| txt(key, text(&root.join(file)), "sysfs/dmi"))
        .collect::<Vec<_>>();
        if fields.iter().all(|f| f.value.is_none()) {
            return group(Kind::Motherboard, vec![]);
        }
        let title = text(&root.join("board_name")).unwrap_or_else(|| "Motherboard".into());
        group(
            Kind::Motherboard,
            vec![Item {
                id: "board:system".into(),
                title,
                fields,
            }],
        )
    }
    fn storage(&self) -> Group {
        let mut items = vec![];
        for p in dirs(&self.sys_root.join("block"), 512).unwrap_or_default() {
            let id = p.file_name().unwrap().to_string_lossy().into_owned();
            if id.starts_with("loop")
                || id.starts_with("ram")
                || id.starts_with("zram")
                || !p.join("device").exists()
            {
                continue;
            }
            let controller = id
                .strip_prefix("nvme")
                .and_then(|v| v.split_once('n').map(|(n, _)| format!("nvme{n}")))
                .filter(|s| s.len() > 4 && s[4..].bytes().all(|b| b.is_ascii_digit()));
            let model = text(&p.join("device/model")).or_else(|| {
                controller
                    .as_ref()
                    .and_then(|c| text(&self.sys_root.join("class/nvme").join(c).join("model")))
            });
            items.push(Item {
                id: format!("block:{id}"),
                title: model.clone().unwrap_or_else(|| id.clone()),
                fields: vec![
                    txt("device", Some(id), "sysfs/block"),
                    txt("model", model, "sysfs/block"),
                    txt("vendor", text(&p.join("device/vendor")), "sysfs/block"),
                    txt("firmware", text(&p.join("device/rev")), "sysfs/block"),
                    field(
                        "capacity",
                        number(&p.join("size"))
                            .filter(|v| *v > 0)
                            .and_then(|n| n.checked_mul(512))
                            .map(Value::Bytes),
                        "sysfs/block 512-byte sectors",
                    ),
                    txt(
                        "media",
                        number(&p.join("queue/rotational")).and_then(|v| match v {
                            0 => Some("Non-rotating".into()),
                            1 => Some("Rotating".into()),
                            _ => None,
                        }),
                        "sysfs/block",
                    ),
                ],
            });
        }
        if items.len() > 128 {
            items.clear();
        }
        group(Kind::Storage, items)
    }
}
fn pci_name(db: &str, vendor: &str, device: &str) -> Option<String> {
    let mut in_vendor = false;
    for line in db.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            in_vendor = line
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(vendor));
        } else if in_vendor && line.starts_with('\t') && !line.starts_with("\t\t") {
            let row = line.trim_start();
            if row.get(..4).is_some_and(|s| s.eq_ignore_ascii_case(device)) {
                return row.get(4..).and_then(clean);
            }
        }
    }
    None
}
#[derive(Clone)]
pub struct SpecService {
    provider: LinuxSpec,
    cache: Arc<Mutex<Option<(Instant, SystemSpec)>>>,
    jobs: crate::admission::WorkSlots,
}
impl SpecService {
    pub(crate) fn with_admission(mut self, admission: crate::admission::Admission) -> Self {
        self.jobs = self.jobs.with_admission(admission);
        self
    }

    pub fn new(provider: LinuxSpec) -> Self {
        Self {
            provider,
            cache: Arc::new(Mutex::new(None)),
            jobs: crate::admission::WorkSlots::new(1, Default::default()),
        }
    }
    pub async fn get(&self, refresh: bool) -> llcontrol_ipc::Result<SystemSpec> {
        if !refresh {
            if let Some((when, spec)) = self
                .cache
                .lock()
                .map_err(|e| llcontrol_ipc::IpcError::DeviceBusy(e.to_string()))?
                .as_ref()
            {
                if when.elapsed() < Duration::from_secs(30) {
                    return Ok(spec.clone());
                }
            }
        }
        let permit = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|e| llcontrol_ipc::IpcError::DeviceBusy(e.to_string()))?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let spec = this.provider.collect()?;
            *this
                .cache
                .lock()
                .map_err(|e| llcontrol_ipc::IpcError::DeviceBusy(e.to_string()))? =
                Some((Instant::now(), spec.clone()));
            Ok(spec)
        })
        .await
        .map_err(|e| llcontrol_ipc::IpcError::DeviceBusy(e.to_string()))?
    }
}
#[cfg(test)]
mod tests;
