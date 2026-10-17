//! i915 system-wide engine busy counters. No process fdinfo aggregation.
//! PMU ABI/provenance and remaining xe/memory work: docs/development/parity/sensor-provider-inventory.md.
use super::{reading, text, STALE_AFTER};
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    time::Instant,
};

const MAX_ENTRIES: usize = 256;
const MAX_DEVICES: usize = 32;
const MAX_ENGINES: usize = 32;

#[derive(Clone, Copy)]
struct Sample {
    busy: u64,
    enabled: u64,
    running: u64,
}
trait Counter: Send {
    fn sample(&mut self) -> io::Result<Sample>;
}
impl Counter for perf_event::Counter {
    fn sample(&mut self) -> io::Result<Sample> {
        let value = self.read_count_and_time()?;
        Ok(Sample {
            busy: value.count,
            enabled: value.time_enabled,
            running: value.time_running,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Event {
    pmu_type: u32,
    config: u64,
    cpu: usize,
}
fn open(event: &Event) -> io::Result<Box<dyn Counter>> {
    let mut builder = perf_event::Builder::new().any_pid().one_cpu(event.cpu);
    builder.include_kernel().include_hv();
    builder.attrs_mut().type_ = event.pmu_type;
    builder.attrs_mut().config = event.config;
    let mut counter = builder.build()?;
    counter.enable()?;
    Ok(Box::new(counter))
}
struct Active {
    event: Event,
    counter: Box<dyn Counter>,
    previous: Option<(Sample, Instant)>,
}

pub(super) struct IntelSensors {
    drm: PathBuf,
    pmus: PathBuf,
    online: PathBuf,
    active: BTreeMap<String, Active>,
}
impl IntelSensors {
    pub(super) fn reset_baselines(&mut self) {
        self.active.clear();
    }

    pub(super) fn new(drm: PathBuf, pmus: PathBuf, online: PathBuf) -> Self {
        Self {
            drm,
            pmus,
            online,
            active: BTreeMap::new(),
        }
    }
    pub(super) fn collect(&mut self, now: u64, at: Instant) -> Vec<SensorReading> {
        self.collect_with(now, at, open)
    }
    fn collect_with(
        &mut self,
        now: u64,
        at: Instant,
        mut open: impl FnMut(&Event) -> io::Result<Box<dyn Counter>>,
    ) -> Vec<SensorReading> {
        let devices = match self.devices() {
            Ok(devices) => devices,
            Err(status) => {
                self.active.clear();
                return vec![row(
                    "intel:discovery",
                    "Intel GPU discovery",
                    Err(status),
                    now,
                )];
            }
        };
        // Only one device without a PCI-qualified PMU may use the legacy i915 PMU.
        let legacy_count = devices
            .iter()
            .filter(|(_, pci)| {
                !self
                    .pmus
                    .join(format!("i915_{}", pci.replace(':', "_")))
                    .exists()
            })
            .count();
        let mut rows = Vec::new();
        let mut next = BTreeMap::new();
        for (device, pci) in devices {
            use std::os::unix::ffi::OsStrExt;
            let identity: String = device
                .as_os_str()
                .as_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            let prefix = format!("intel:{identity}:i915");
            let qualified = self.pmus.join(format!("i915_{}", pci.replace(':', "_")));
            let pmu = if qualified.exists() {
                qualified
            } else if legacy_count == 1 {
                self.pmus.join("i915")
            } else {
                rows.push(row(
                    &format!("{prefix}:discovery"),
                    &format!("i915 {pci}: ambiguous PMU"),
                    Err(SensorStatus::Unavailable),
                    now,
                ));
                continue;
            };
            let events = match self.events(&pmu) {
                Ok(events) => events,
                Err(status) => {
                    rows.push(row(
                        &format!("{prefix}:discovery"),
                        &format!("i915 {pci}: PMU unavailable"),
                        Err(status),
                        now,
                    ));
                    continue;
                }
            };
            for (name, event) in events {
                // Config encodes engine class/instance; names are labels, not identities.
                let id = format!("{prefix}:engine:{:x}:utilization", event.config);
                let label = format!("i915 {pci}: {name} engine utilization");
                let old = self.active.remove(&id).filter(|a| a.event == event);
                let mut active = match old {
                    Some(active) => active,
                    None => match open(&event) {
                        Ok(counter) => Active {
                            event,
                            counter,
                            previous: None,
                        },
                        Err(_) => {
                            rows.push(row(&id, &label, Err(SensorStatus::Unavailable), now));
                            continue;
                        }
                    },
                };
                match active.counter.sample() {
                    Ok(sample) => {
                        let value = active.previous.map_or(
                            Err(SensorStatus::Unavailable),
                            |(old, previous_at)| {
                                utilization(old, sample, at.saturating_duration_since(previous_at))
                            },
                        );
                        active.previous = Some((sample, at));
                        rows.push(row(&id, &label, value, now));
                        next.insert(id, active);
                    }
                    Err(_) => rows.push(row(&id, &label, Err(SensorStatus::Unavailable), now)),
                }
            }
        }
        // Dropping missing/failed handles releases PMU ownership; reconnect warms up anew.
        self.active = next;
        rows
    }
    fn devices(&self) -> Result<Vec<(PathBuf, String)>, SensorStatus> {
        let mut devices = BTreeMap::new();
        for entry in entries(&self.drm)? {
            let name = entry.file_name().unwrap_or_default().to_string_lossy();
            if !name
                .strip_prefix("card")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            {
                continue;
            }
            let device =
                fs::canonicalize(entry.join("device")).map_err(|_| SensorStatus::Unavailable)?;
            let Ok(driver) = fs::canonicalize(device.join("driver")) else {
                continue;
            };
            if driver.file_name().is_none_or(|n| n != "i915") {
                continue;
            }
            let pci = device
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|s| pci_address(s))
                .ok_or(SensorStatus::Invalid)?
                .to_owned();
            if devices.insert(device, pci).is_some() || devices.len() > MAX_DEVICES {
                return Err(SensorStatus::Invalid);
            }
        }
        Ok(devices.into_iter().collect())
    }
    fn events(&self, pmu: &Path) -> Result<Vec<(String, Event)>, SensorStatus> {
        let pmu_type = text(&pmu.join("type"))?
            .trim()
            .parse::<u32>()
            .map_err(|_| SensorStatus::Invalid)?;
        let online = cpu_list(&text(&self.online)?)?;
        let mask = pmu.join("cpumask");
        let allowed = match fs::metadata(&mask) {
            Ok(_) => cpu_list(&text(&mask)?)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => online.clone(),
            Err(_) => return Err(SensorStatus::Unavailable),
        };
        let cpu = online
            .intersection(&allowed)
            .next()
            .copied()
            .ok_or(SensorStatus::Unavailable)?;
        let mut configs = BTreeSet::new();
        let mut events = Vec::new();
        for path in entries(&pmu.join("events"))? {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let Some(engine) = name.strip_suffix("-busy") else {
                continue;
            };
            if engine.is_empty()
                || engine.len() > 32
                || !engine.bytes().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(SensorStatus::Invalid);
            }
            if text(&path.with_file_name(format!("{name}.unit")))?.trim() != "ns" {
                return Err(SensorStatus::Invalid);
            }
            let config = text(&path)?;
            let config = config
                .trim()
                .strip_prefix("config=0x")
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .filter(|v| *v < 0x10_0000 && v & 0xf == 0)
                .ok_or(SensorStatus::Invalid)?;
            if !configs.insert(config) || events.len() == MAX_ENGINES {
                return Err(SensorStatus::Invalid);
            }
            events.push((
                engine.to_owned(),
                Event {
                    pmu_type,
                    config,
                    cpu,
                },
            ));
        }
        if events.is_empty() {
            return Err(SensorStatus::Unavailable);
        }
        Ok(events)
    }
}
fn entries(path: &Path) -> Result<Vec<PathBuf>, SensorStatus> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(|_| SensorStatus::Unavailable)? {
        if entries.len() == MAX_ENTRIES {
            return Err(SensorStatus::Invalid);
        }
        entries.push(entry.map_err(|_| SensorStatus::Unavailable)?.path());
    }
    Ok(entries)
}
pub(super) fn cpu_list(value: &str) -> Result<BTreeSet<usize>, SensorStatus> {
    let mut cpus = BTreeSet::new();
    for part in value.trim().split(',') {
        let (first, last) = part.split_once('-').unwrap_or((part, part));
        let parse = |s: &str| {
            s.parse::<usize>()
                .ok()
                .filter(|n| *n < 1_048_576)
                .ok_or(SensorStatus::Invalid)
        };
        let first = parse(first)?;
        let last = parse(last)?;
        if last < first || last - first >= 4096 {
            return Err(SensorStatus::Invalid);
        }
        cpus.extend(first..=last);
        if cpus.len() > 4096 {
            return Err(SensorStatus::Invalid);
        }
    }
    Ok(cpus)
}
pub(super) fn pci_address(s: &str) -> bool {
    s.len() == 12
        && s.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b':',
            10 => b == b'.',
            _ => b.is_ascii_hexdigit(),
        })
}
fn row(id: &str, label: &str, value: Result<f64, SensorStatus>, now: u64) -> SensorReading {
    reading(id, label, SensorUnit::Percent, value, now)
}
fn utilization(
    old: Sample,
    new: Sample,
    elapsed: std::time::Duration,
) -> Result<f64, SensorStatus> {
    if elapsed.is_zero() || elapsed > STALE_AFTER {
        return Err(SensorStatus::Unavailable);
    }
    let busy = new
        .busy
        .checked_sub(old.busy)
        .ok_or(SensorStatus::Invalid)?;
    let enabled = new
        .enabled
        .checked_sub(old.enabled)
        .ok_or(SensorStatus::Invalid)?;
    let running = new
        .running
        .checked_sub(old.running)
        .ok_or(SensorStatus::Invalid)?;
    // Reject multiplexed/incomplete intervals instead of guessing an extrapolated load.
    if running == 0 || enabled != running {
        return Err(SensorStatus::Unavailable);
    }
    let value = 100.0 * busy as f64 / running as f64;
    // busy and perf timestamps are sampled separately; tolerate <=1% edge skew.
    if value > 101.0 {
        return Err(SensorStatus::Invalid);
    }
    Ok(value.min(100.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    fn sample(busy: u64, time: u64) -> Sample {
        Sample {
            busy,
            enabled: time,
            running: time,
        }
    }
    struct Fake {
        sample: Arc<Mutex<Option<Sample>>>,
        drops: Arc<AtomicUsize>,
    }
    impl Counter for Fake {
        fn sample(&mut self) -> io::Result<Sample> {
            self.sample
                .lock()
                .unwrap()
                .ok_or_else(|| io::Error::other("removed"))
        }
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    struct Fixture {
        root: tempfile::TempDir,
        samples: Arc<Mutex<Option<Sample>>>,
        drops: Arc<AtomicUsize>,
    }
    impl Fixture {
        fn new() -> Self {
            let fixture = Self {
                root: tempfile::tempdir().unwrap(),
                samples: Arc::new(Mutex::new(Some(sample(0, 0)))),
                drops: Arc::new(AtomicUsize::new(0)),
            };
            fixture.write("online", "0-3\n");
            fs::create_dir_all(fixture.path("drivers/i915")).unwrap();
            fixture.device("card0", "0000:00:02.0");
            fixture.pmu("i915");
            fixture
        }
        fn path(&self, name: &str) -> PathBuf {
            self.root.path().join(name)
        }
        fn write(&self, name: &str, value: &str) {
            let path = self.path(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }
        fn device(&self, card: &str, pci: &str) {
            fs::create_dir_all(self.path(&format!("drm/{card}"))).unwrap();
            fs::create_dir_all(self.path(&format!("devices/{pci}"))).unwrap();
            symlink(
                self.path("drivers/i915"),
                self.path(&format!("devices/{pci}/driver")),
            )
            .unwrap();
            symlink(
                self.path(&format!("devices/{pci}")),
                self.path(&format!("drm/{card}/device")),
            )
            .unwrap();
        }
        fn pmu(&self, name: &str) {
            self.write(&format!("pmu/{name}/type"), "19\n");
            self.write(&format!("pmu/{name}/events/rcs0-busy"), "config=0x0\n");
            self.write(&format!("pmu/{name}/events/rcs0-busy.unit"), "ns\n");
        }
        fn provider(&self) -> IntelSensors {
            IntelSensors::new(self.path("drm"), self.path("pmu"), self.path("online"))
        }
        fn poll(&self, provider: &mut IntelSensors, at: Instant) -> Vec<SensorReading> {
            provider.collect_with(123, at, |event| {
                assert_eq!(event.cpu, 0);
                Ok(Box::new(Fake {
                    sample: self.samples.clone(),
                    drops: self.drops.clone(),
                }))
            })
        }
        fn set(&self, busy: u64, time: u64) {
            *self.samples.lock().unwrap() = Some(sample(busy, time));
        }
    }
    #[test]
    fn engine_load_survives_card_renumbering_and_reconnect_warms_up() {
        let f = Fixture::new();
        let mut p = f.provider();
        let start = Instant::now();
        let first = f.poll(&mut p, start);
        assert_eq!(first[0].status, SensorStatus::Unavailable);
        f.set(250_000_000, 1_000_000_000);
        fs::rename(f.path("drm/card0"), f.path("drm/card7")).unwrap();
        let next = f.poll(&mut p, start + Duration::from_secs(1));
        assert_eq!(next[0].id, first[0].id);
        assert_eq!(next[0].value, Some(25.0));
        assert_eq!(next[0].unit, SensorUnit::Percent);
        assert_eq!(next[0].observed_unix_ms, Some(123));
        fs::remove_dir_all(f.path("drm/card7")).unwrap();
        assert!(f.poll(&mut p, start + Duration::from_secs(2)).is_empty());
        assert_eq!(f.drops.load(Ordering::SeqCst), 1);
        fs::create_dir_all(f.path("drm/card1")).unwrap();
        symlink(f.path("devices/0000:00:02.0"), f.path("drm/card1/device")).unwrap();
        assert_eq!(
            f.poll(&mut p, start + Duration::from_secs(3))[0].status,
            SensorStatus::Unavailable
        );
        drop(p);
        assert_eq!(f.drops.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn permission_failure_and_failed_read_recover_without_fake_zero() {
        let f = Fixture::new();
        let mut p = f.provider();
        let start = Instant::now();
        let denied = p.collect_with(123, start, |_| {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        });
        assert_eq!(denied[0].value, None);
        assert_eq!(denied[0].status, SensorStatus::Unavailable);
        assert_eq!(
            f.poll(&mut p, start + Duration::from_secs(1))[0].value,
            None
        );
        *f.samples.lock().unwrap() = None;
        assert_eq!(
            f.poll(&mut p, start + Duration::from_secs(2))[0].value,
            None
        );
        assert_eq!(f.drops.load(Ordering::SeqCst), 1);
        f.set(20, 100);
        assert_eq!(
            f.poll(&mut p, start + Duration::from_secs(3))[0].value,
            None
        );
        f.set(20, 200);
        assert_eq!(
            f.poll(&mut p, start + Duration::from_secs(4))[0].value,
            Some(0.0)
        );
    }
    #[test]
    fn multi_gpu_requires_unambiguous_pmu_and_keeps_distinct_ids() {
        let f = Fixture::new();
        f.device("card1", "0000:03:00.0");
        let mut p = f.provider();
        let start = Instant::now();
        let ambiguous = p.collect_with(123, start, |_| panic!("must not open ambiguous PMU"));
        assert_eq!(ambiguous.len(), 2);
        assert!(ambiguous
            .iter()
            .all(|r| r.status == SensorStatus::Unavailable));
        f.pmu("i915_0000_03_00.0");
        let rows = f.poll(&mut p, start);
        assert_eq!(rows.len(), 2);
        assert_ne!(rows[0].id, rows[1].id);
        f.set(30, 100);
        assert!(f
            .poll(&mut p, start + Duration::from_secs(1))
            .iter()
            .all(|r| r.value == Some(30.0)));
    }
    #[test]
    fn malformed_catalog_is_rejected_and_existing_handles_are_released() {
        let f = Fixture::new();
        let mut p = f.provider();
        let start = Instant::now();
        f.poll(&mut p, start);
        f.write("pmu/i915/events/vcs0-busy", "config=0x0");
        f.write("pmu/i915/events/vcs0-busy.unit", "ns");
        let rows = f.poll(&mut p, start + Duration::from_secs(1));
        assert_eq!(rows[0].status, SensorStatus::Invalid); // duplicate encoded engine
        assert_eq!(f.drops.load(Ordering::SeqCst), 1);
        fs::remove_file(f.path("pmu/i915/events/vcs0-busy")).unwrap();
        for malformed in [
            "config=0x200000",
            "config=0x1",
            "config=0x100000",
            "event=0x0",
            "config=0x0,foo=1",
        ] {
            f.write("pmu/i915/events/rcs0-busy", malformed);
            assert_eq!(f.poll(&mut p, start)[0].status, SensorStatus::Invalid);
        }
        f.write("pmu/i915/events/rcs0-busy", "config=0x0");
        f.write("pmu/i915/events/rcs0-busy.unit", "ticks");
        assert_eq!(f.poll(&mut p, start)[0].status, SensorStatus::Invalid);
    }
    #[test]
    fn changed_pmu_type_and_oversized_catalog_cannot_reuse_old_samples() {
        let f = Fixture::new();
        let mut p = f.provider();
        let at = Instant::now();
        f.poll(&mut p, at);
        f.set(20, 100);
        f.write("pmu/i915/type", "20");
        assert_eq!(f.poll(&mut p, at + Duration::from_secs(1))[0].value, None);
        assert_eq!(f.drops.load(Ordering::SeqCst), 1);
        for i in 0..MAX_ENTRIES {
            f.write(&format!("pmu/i915/events/extra{i}"), "0");
        }
        assert_eq!(
            f.poll(&mut p, at + Duration::from_secs(2))[0].status,
            SensorStatus::Invalid
        );
        assert_eq!(f.drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn pmu_cpu_mask_overrides_first_online_cpu_and_migration_reopens() {
        let f = Fixture::new();
        f.write("online", "0-7");
        f.write("pmu/i915/cpumask", "4");
        let mut p = f.provider();
        let at = Instant::now();
        let mut poll = |cpu, time| {
            p.collect_with(123, time, |event| {
                assert_eq!(event.cpu, cpu);
                Ok(Box::new(Fake {
                    sample: f.samples.clone(),
                    drops: f.drops.clone(),
                }))
            })
        };
        assert_eq!(poll(4, at)[0].status, SensorStatus::Unavailable);
        f.write("pmu/i915/cpumask", "6");
        assert_eq!(
            poll(6, at + Duration::from_secs(1))[0].status,
            SensorStatus::Unavailable
        );
        assert_eq!(f.drops.load(Ordering::SeqCst), 1);
        f.write("pmu/i915/cpumask", "9");
        assert_eq!(
            poll(9, at + Duration::from_secs(2))[0].status,
            SensorStatus::Unavailable
        );
        assert_eq!(f.drops.load(Ordering::SeqCst), 2);
        for malformed in ["", "0-x", "1-0", "0-1000000", "0,1bad", "-1"] {
            assert!(cpu_list(malformed).is_err());
        }
    }

    #[test]
    fn counter_deltas_reject_reset_stale_multiplexing_and_impossible_values() {
        let t = Duration::from_secs(1);
        assert_eq!(utilization(sample(0, 0), sample(75, 100), t), Ok(75.0));
        assert_eq!(utilization(sample(0, 0), sample(0, 100), t), Ok(0.0));
        assert_eq!(utilization(sample(0, 0), sample(101, 100), t), Ok(100.0));
        assert_eq!(
            utilization(sample(10, 100), sample(9, 200), t),
            Err(SensorStatus::Invalid)
        );
        assert_eq!(
            utilization(sample(0, 0), sample(102, 100), t),
            Err(SensorStatus::Invalid)
        );
        assert_eq!(
            utilization(sample(0, 0), sample(0, 0), t),
            Err(SensorStatus::Unavailable)
        );
        assert_eq!(
            utilization(sample(0, 0), sample(5, 100), Duration::from_secs(4)),
            Err(SensorStatus::Unavailable)
        );
        assert_eq!(
            utilization(
                sample(0, 0),
                Sample {
                    busy: 5,
                    enabled: 100,
                    running: 50
                },
                t
            ),
            Err(SensorStatus::Unavailable)
        );
    }
    #[test]
    fn sleep_reset_releases_counter_and_requires_new_engine_pair() {
        let f = Fixture::new();
        let mut p = f.provider();
        let at = Instant::now();
        let initial = f.poll(&mut p, at);
        f.set(250_000_000, 1_000_000_000);
        assert_eq!(
            f.poll(&mut p, at + Duration::from_secs(1))[0].value,
            Some(25.0)
        );
        p.reset_baselines();
        assert_eq!(f.drops.load(Ordering::SeqCst), 1);
        f.set(2_000_000_000, 3_000_000_000);
        let warm = f.poll(&mut p, at + Duration::from_secs(2));
        assert_eq!(warm[0].id, initial[0].id);
        assert_eq!(warm[0].status, SensorStatus::Unavailable);
        f.set(2_100_000_000, 4_000_000_000);
        assert_eq!(
            f.poll(&mut p, at + Duration::from_secs(3))[0].value,
            Some(10.0)
        );
    }
}
