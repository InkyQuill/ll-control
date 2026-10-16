//! xe per-engine utilization: active ticks / total ticks, not host nanoseconds.
//! DRM topology query is isolated; persistent perf counters run off the executor.
use super::{
    intel::{cpu_list, pci_address},
    query_process::QueryProcess,
    reading, text, STALE_AFTER,
};
use lconnect_intel_memory::{Engine, EngineDevice, QueryError, MAX_DEVICES};
use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
const LIMIT: usize = 131_072;

#[derive(Clone, Copy, Debug)]
struct Count {
    ticks: u64,
    enabled: u64,
    running: u64,
}
#[derive(Clone, Copy, Debug)]
struct Sample {
    active: Count,
    total: Count,
}
trait Counter: Send {
    fn sample(&mut self) -> io::Result<Sample>;
}
struct PerfPair {
    active: perf_event::Counter,
    total: perf_event::Counter,
}
impl Counter for PerfPair {
    fn sample(&mut self) -> io::Result<Sample> {
        let read = |counter: &mut perf_event::Counter| {
            counter.read_count_and_time().map(|c| Count {
                ticks: c.count,
                enabled: c.time_enabled,
                running: c.time_running,
            })
        };
        Ok(Sample {
            active: read(&mut self.active)?,
            total: read(&mut self.total)?,
        })
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Event {
    pmu_type: u32,
    active: u64,
    total: u64,
    cpu: usize,
}
fn open(event: &Event) -> io::Result<Box<dyn Counter>> {
    let build = |config| {
        let mut builder = perf_event::Builder::new().any_pid().one_cpu(event.cpu);
        builder.include_kernel().include_hv();
        builder.attrs_mut().type_ = event.pmu_type;
        builder.attrs_mut().config = config;
        builder.build()
    };
    let mut pair = PerfPair {
        active: build(event.active)?,
        total: build(event.total)?,
    };
    pair.active.enable()?;
    pair.total.enable()?;
    Ok(Box::new(pair))
}
struct Active {
    event: Event,
    counter: Box<dyn Counter>,
    previous: Option<(Sample, Instant)>,
}
struct State {
    pmus: PathBuf,
    online: PathBuf,
    active: BTreeMap<String, Active>,
    previous: Vec<SensorReading>,
}
pub(super) struct XeSensors {
    process: QueryProcess,
    state: Option<State>,
}
impl XeSensors {
    pub(super) fn reset_baselines(&mut self) {
        if let Some(state) = &mut self.state {
            state.active.clear();
            state.previous.clear();
        }
    }

    pub(super) fn new(executable: PathBuf, pmus: PathBuf, online: PathBuf) -> Self {
        Self {
            process: QueryProcess::new(executable, "--query-intel-engines", LIMIT),
            state: Some(State {
                pmus,
                online,
                active: BTreeMap::new(),
                previous: Vec::new(),
            }),
        }
    }
    pub(super) async fn collect(&mut self, now: u64) -> Vec<SensorReading> {
        self.collect_with(now, open).await
    }
    async fn collect_with(
        &mut self,
        now: u64,
        open: impl FnMut(&Event) -> io::Result<Box<dyn Counter>> + Send + 'static,
    ) -> Vec<SensorReading> {
        let catalog = self.process.query().await.and_then(|bytes| parse(&bytes));
        let Some(mut state) = self.state.take() else {
            return vec![row(
                "xe:discovery",
                "xe collector unavailable",
                Err(SensorStatus::Unavailable),
                now,
            )];
        };
        match tokio::task::spawn_blocking(move || {
            let rows = state.collect(catalog, now, Instant::now(), open);
            (state, rows)
        })
        .await
        {
            Ok((state, rows)) => {
                self.state = Some(state);
                rows
            }
            Err(_) => vec![row(
                "xe:discovery",
                "xe collector failed",
                Err(SensorStatus::Unavailable),
                now,
            )],
        }
    }
}
fn parse(bytes: &[u8]) -> Result<Vec<EngineDevice>, SensorStatus> {
    if bytes.len() > LIMIT {
        return Err(SensorStatus::Invalid);
    }
    let devices: Vec<EngineDevice> =
        serde_json::from_slice(bytes).map_err(|_| SensorStatus::Invalid)?;
    if devices.len() > MAX_DEVICES {
        return Err(SensorStatus::Invalid);
    }
    let mut seen = BTreeSet::new();
    let mut pci_seen = BTreeSet::new();
    for d in &devices {
        if !pci_address(&d.pci)
            || !d.physical.starts_with("2f")
            || d.physical.len() > 900
            || d.physical.len() % 2 != 0
            || !d
                .physical
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !d.physical.ends_with(&format!("2f{}", hex(&d.pci)))
            || !seen.insert(&d.physical)
            || !pci_seen.insert(&d.pci)
        {
            return Err(SensorStatus::Invalid);
        }
        if let Ok(engines) = &d.engines {
            let mut seen = BTreeSet::new();
            if engines.len() > 32
                || engines
                    .iter()
                    .any(|e| e.gt > 15 || e.class > 4 || e.instance > 255 || !seen.insert(*e))
            {
                return Err(SensorStatus::Invalid);
            }
        }
    }
    Ok(devices)
}
fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}
fn row(id: &str, label: &str, value: Result<f64, SensorStatus>, now: u64) -> SensorReading {
    reading(id, label, SensorUnit::Percent, value, now)
}
impl State {
    fn collect(
        &mut self,
        catalog: Result<Vec<EngineDevice>, SensorStatus>,
        now: u64,
        at: Instant,
        mut open: impl FnMut(&Event) -> io::Result<Box<dyn Counter>>,
    ) -> Vec<SensorReading> {
        let devices = match catalog {
            Ok(v) => v,
            Err(status) => {
                self.active.clear();
                let mut rows: Vec<_> = self
                    .previous
                    .iter()
                    .map(|r| row(&r.id, &r.label, Err(status), now))
                    .collect();
                rows.push(row("xe:discovery", "xe engine discovery", Err(status), now));
                return rows;
            }
        };
        let mut rows = Vec::new();
        let mut next = BTreeMap::new();
        for d in devices {
            let prefix = format!("intel:{}:xe", d.physical);
            let events = d
                .engines
                .map_err(|e| match e {
                    QueryError::Unavailable => SensorStatus::Unavailable,
                    QueryError::Invalid => SensorStatus::Invalid,
                })
                .and_then(|engines| {
                    events(
                        &self.pmus.join(format!("xe_{}", d.pci.replace(':', "_"))),
                        &self.online,
                        &engines,
                    )
                });
            let events = match events {
                Ok(v) => v,
                Err(status) => {
                    rows.push(row(
                        &format!("{prefix}:discovery"),
                        &format!("xe {} PMU unavailable", d.pci),
                        Err(status),
                        now,
                    ));
                    continue;
                }
            };
            for (engine, event) in events {
                let id = format!(
                    "{prefix}:gt{}:class{}:instance{}:utilization",
                    engine.gt, engine.class, engine.instance
                );
                let class = ["render", "copy", "video decode", "video enhance", "compute"]
                    [engine.class as usize];
                let label = format!(
                    "xe {} GT{} {class} {} engine utilization",
                    d.pci, engine.gt, engine.instance
                );
                let old = self.active.remove(&id).filter(|a| a.event == event);
                let mut active = match old {
                    Some(a) => a,
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
        self.active = next;
        self.previous = rows.clone();
        rows
    }
}
fn events(
    pmu: &Path,
    online: &Path,
    engines: &[Engine],
) -> Result<Vec<(Engine, Event)>, SensorStatus> {
    // Require the advertised layout we implement, not a guessed kernel version.
    for (name, format) in [
        ("gt", "config:60-63"),
        ("function", "config:44-59"),
        ("engine_class", "config:20-27"),
        ("engine_instance", "config:12-19"),
        ("event", "config:0-11"),
    ] {
        if text(&pmu.join("format").join(name))?.trim() != format {
            return Err(SensorStatus::Invalid);
        }
    }
    let event_id = |name: &str| -> Result<u64, SensorStatus> {
        text(&pmu.join("events").join(name))?
            .trim()
            .strip_prefix("event=0x")
            .and_then(|s| u64::from_str_radix(s, 16).ok())
            .filter(|n| *n < 4096)
            .ok_or(SensorStatus::Invalid)
    };
    let active = event_id("engine-active-ticks")?;
    let total = event_id("engine-total-ticks")?;
    if active == total {
        return Err(SensorStatus::Invalid);
    }
    let pmu_type = text(&pmu.join("type"))?
        .trim()
        .parse()
        .map_err(|_| SensorStatus::Invalid)?;
    let online = cpu_list(&text(online)?)?;
    let allowed = match std::fs::metadata(pmu.join("cpumask")) {
        Ok(_) => cpu_list(&text(&pmu.join("cpumask"))?)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => online.clone(),
        Err(_) => return Err(SensorStatus::Unavailable),
    };
    let cpu = online
        .intersection(&allowed)
        .next()
        .copied()
        .ok_or(SensorStatus::Unavailable)?;
    Ok(engines
        .iter()
        .map(|engine| {
            // Function=0 means the physical function; no unrequested VF aggregation.
            let selector = ((engine.gt as u64) << 60)
                | ((engine.class as u64) << 20)
                | ((engine.instance as u64) << 12);
            (
                *engine,
                Event {
                    pmu_type,
                    active: selector | active,
                    total: selector | total,
                    cpu,
                },
            )
        })
        .collect())
}
fn utilization(old: Sample, new: Sample, elapsed: Duration) -> Result<f64, SensorStatus> {
    if elapsed.is_zero() || elapsed > STALE_AFTER {
        return Err(SensorStatus::Unavailable);
    }
    let delta = |old: Count, new: Count| -> Result<(u64, u64), SensorStatus> {
        let ticks = new
            .ticks
            .checked_sub(old.ticks)
            .ok_or(SensorStatus::Invalid)?;
        let enabled = new
            .enabled
            .checked_sub(old.enabled)
            .ok_or(SensorStatus::Invalid)?;
        let running = new
            .running
            .checked_sub(old.running)
            .ok_or(SensorStatus::Invalid)?;
        if running == 0 || running != enabled {
            return Err(SensorStatus::Unavailable);
        }
        Ok((ticks, running))
    };
    let (active, a_time) = delta(old.active, new.active)?;
    let (total, t_time) = delta(old.total, new.total)?;
    if total == 0 {
        return Err(SensorStatus::Unavailable);
    }
    // Separate perf FDs have a small read/enable skew. Reject mismatched windows.
    if a_time.abs_diff(t_time) as f64 > (a_time.max(t_time) as f64) * 0.01 {
        return Err(SensorStatus::Unavailable);
    }
    let value = 100.0 * active as f64 / total as f64;
    if value > 101.0 {
        return Err(SensorStatus::Invalid);
    }
    Ok(value.min(100.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };
    fn sample(active: u64, total: u64, time: u64) -> Sample {
        Sample {
            active: Count {
                ticks: active,
                enabled: time,
                running: time,
            },
            total: Count {
                ticks: total,
                enabled: time,
                running: time,
            },
        }
    }
    struct Fake {
        value: Arc<Mutex<Option<Sample>>>,
        drops: Arc<AtomicUsize>,
    }
    impl Counter for Fake {
        fn sample(&mut self) -> io::Result<Sample> {
            self.value
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
        value: Arc<Mutex<Option<Sample>>>,
        drops: Arc<AtomicUsize>,
    }
    impl Fixture {
        fn new() -> Self {
            let f = Self {
                root: tempfile::tempdir().unwrap(),
                value: Arc::new(Mutex::new(Some(sample(0, 0, 0)))),
                drops: Arc::new(AtomicUsize::new(0)),
            };
            f.write("online", "0-3");
            f.pmu("0000:03:00.0");
            f
        }
        fn write(&self, path: &str, data: &str) {
            let path = self.root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, data).unwrap();
        }
        fn pmu(&self, pci: &str) {
            let root = format!("pmu/xe_{}", pci.replace(':', "_"));
            for (file, value) in [
                ("format/gt", "config:60-63"),
                ("format/function", "config:44-59"),
                ("format/engine_class", "config:20-27"),
                ("format/engine_instance", "config:12-19"),
                ("format/event", "config:0-11"),
                ("events/engine-active-ticks", "event=0x02"),
                ("events/engine-total-ticks", "event=0x03"),
                ("type", "20"),
                ("cpumask", "2"),
            ] {
                self.write(&format!("{root}/{file}"), value);
            }
        }
        fn state(&self) -> State {
            State {
                pmus: self.root.path().join("pmu"),
                online: self.root.path().join("online"),
                active: BTreeMap::new(),
                previous: Vec::new(),
            }
        }
        fn opener(&self) -> impl FnMut(&Event) -> io::Result<Box<dyn Counter>> + Send + 'static {
            let value = self.value.clone();
            let drops = self.drops.clone();
            move |event| {
                assert_eq!(event.cpu, 2);
                Ok(Box::new(Fake {
                    value: value.clone(),
                    drops: drops.clone(),
                }))
            }
        }
        fn catalog(&self) -> Vec<EngineDevice> {
            vec![EngineDevice {
                physical: hex("/sys/devices/0000:03:00.0"),
                pci: "0000:03:00.0".into(),
                engines: Ok(vec![
                    Engine {
                        gt: 0,
                        class: 0,
                        instance: 0,
                    },
                    Engine {
                        gt: 1,
                        class: 4,
                        instance: 2,
                    },
                ]),
            }]
        }
    }
    #[test]
    fn tick_ratio_zero_stale_reset_multiplex_and_skew() {
        let t = Duration::from_secs(1);
        let old = sample(0, 0, 0);
        assert_eq!(
            utilization(old, sample(250, 1000, 1_000_000_000), t),
            Ok(25.0)
        );
        assert_eq!(utilization(old, sample(0, 1000, 1_000_000_000), t), Ok(0.0));
        assert_eq!(
            utilization(old, sample(0, 0, 1_000_000_000), t),
            Err(SensorStatus::Unavailable)
        );
        assert_eq!(
            utilization(sample(10, 10, 10), sample(5, 20, 20), t),
            Err(SensorStatus::Invalid)
        );
        assert_eq!(
            utilization(old, sample(102, 100, 100), t),
            Err(SensorStatus::Invalid)
        );
        assert_eq!(
            utilization(old, sample(25, 100, 100), Duration::from_secs(4)),
            Err(SensorStatus::Unavailable)
        );
        let mut skew = sample(25, 100, 100);
        skew.active.enabled = 120;
        skew.active.running = 120;
        assert_eq!(utilization(old, skew, t), Err(SensorStatus::Unavailable));
        let mut multiplex = sample(25, 100, 100);
        multiplex.total.running = 50;
        assert_eq!(
            utilization(old, multiplex, t),
            Err(SensorStatus::Unavailable)
        );
    }
    #[test]
    fn engine_coordinates_formats_and_cpu_are_not_guessed() {
        let f = Fixture::new();
        let engines = f.catalog().remove(0).engines.unwrap();
        let pmu = f.root.path().join("pmu/xe_0000_03_00.0");
        let online = f.root.path().join("online");
        let configs = events(&pmu, &online, &engines).unwrap();
        assert_eq!(configs[0].1.active, 2);
        assert_eq!(configs[0].1.total, 3);
        assert_eq!(configs[1].1.active, (1 << 60) | (4 << 20) | (2 << 12) | 2);
        assert_eq!(configs[1].1.total, (1 << 60) | (4 << 20) | (2 << 12) | 3);
        assert_eq!(configs[1].1.cpu, 2);
        f.write("pmu/xe_0000_03_00.0/format/gt", "config:56-59");
        assert!(events(&pmu, &online, &engines).is_err());
        f.pmu("0000:03:00.0");
        f.write(
            "pmu/xe_0000_03_00.0/events/engine-total-ticks",
            "event=0x02",
        );
        assert!(events(&pmu, &online, &engines).is_err());
        f.pmu("0000:03:00.0");
        f.write("pmu/xe_0000_03_00.0/cpumask", "8");
        assert!(events(&pmu, &online, &engines).is_err());
    }
    #[test]
    fn independent_engines_warm_up_reopen_and_release_without_aggregate() {
        let f = Fixture::new();
        let mut state = f.state();
        let at = Instant::now();
        let first = state.collect(Ok(f.catalog()), 1, at, f.opener());
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|r| r.value.is_none()));
        *f.value.lock().unwrap() = Some(sample(250, 1000, 1_000_000_000));
        let next = state.collect(Ok(f.catalog()), 2, at + Duration::from_secs(1), f.opener());
        assert_eq!(next.len(), 2);
        assert!(next.iter().all(|r| r.value == Some(25.0)));
        assert_ne!(next[0].id, next[1].id);
        f.write("pmu/xe_0000_03_00.0/type", "21");
        assert!(state
            .collect(Ok(f.catalog()), 3, at + Duration::from_secs(2), f.opener())
            .iter()
            .all(|r| r.value.is_none()));
        assert_eq!(f.drops.load(Ordering::SeqCst), 2);
        let failed = state.collect(
            Err(SensorStatus::Unavailable),
            4,
            at + Duration::from_secs(3),
            f.opener(),
        );
        assert!(failed.iter().all(|r| r.value.is_none()));
        assert_eq!(f.drops.load(Ordering::SeqCst), 4);
        assert!(state
            .collect(Ok(f.catalog()), 5, at + Duration::from_secs(4), f.opener())
            .iter()
            .all(|r| r.value.is_none()));
        assert!(state
            .collect(Ok(Vec::new()), 6, at + Duration::from_secs(5), f.opener())
            .is_empty());
        assert_eq!(f.drops.load(Ordering::SeqCst), 6);
    }
    #[test]
    fn permission_read_failures_and_multiple_gpu_membership() {
        let f = Fixture::new();
        f.pmu("0000:04:00.0");
        let mut state = f.state();
        let at = Instant::now();
        let mut catalog = f.catalog();
        let mut other = f.catalog().remove(0);
        other.pci = "0000:04:00.0".into();
        other.physical = hex("/sys/devices/0000:04:00.0");
        catalog.push(other);
        let denied = state.collect(Ok(catalog.clone()), 1, at, |_| {
            Err(io::ErrorKind::PermissionDenied.into())
        });
        assert_eq!(denied.len(), 4);
        assert!(denied.iter().all(|r| r.value.is_none()));
        state.collect(
            Ok(catalog.clone()),
            2,
            at + Duration::from_secs(1),
            f.opener(),
        );
        *f.value.lock().unwrap() = None;
        assert!(state
            .collect(
                Ok(catalog.clone()),
                3,
                at + Duration::from_secs(2),
                f.opener()
            )
            .iter()
            .all(|r| r.value.is_none()));
        assert_eq!(f.drops.load(Ordering::SeqCst), 4);
        *f.value.lock().unwrap() = Some(sample(0, 1000, 1000));
        state.collect(
            Ok(catalog.clone()),
            4,
            at + Duration::from_secs(3),
            f.opener(),
        );
        *f.value.lock().unwrap() = Some(sample(0, 2000, 2000));
        let rows = state.collect(Ok(catalog), 5, at + Duration::from_secs(4), f.opener());
        assert!(rows.iter().all(|r| r.value == Some(0.0)));
        let candidates = crate::sensor_selection::candidates(&rows);
        assert_eq!(candidates.len(), 4);
        assert_ne!(candidates[0].gpu_id, candidates[2].gpu_id);
        let mut defaults = crate::sensor_selection::recommended(&rows, 0);
        assert!(defaults.gpu.as_ref().unwrap().load.is_none());
        defaults.gpu.as_mut().unwrap().load = Some(candidates[0].reading.id.clone());
        crate::sensor_selection::validate_selection(&defaults, &Default::default(), &rows).unwrap();
        defaults.gpu.as_mut().unwrap().load = Some(candidates[2].reading.id.clone());
        assert!(
            crate::sensor_selection::validate_selection(&defaults, &Default::default(), &rows)
                .is_err()
        );
    }
    #[test]
    fn topology_contract_rejects_misbound_duplicate_or_oversized_catalog() {
        let f = Fixture::new();
        let catalog = f.catalog();
        assert!(parse(&serde_json::to_vec(&catalog).unwrap()).is_ok());
        let mut wrong = catalog.clone();
        wrong[0].pci = "0000:04:00.0".into();
        assert!(parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
        let mut wrong = catalog.clone();
        wrong.extend(catalog);
        assert!(parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
        assert!(parse(&vec![b' '; LIMIT + 1]).is_err());
        let mut wrong = f.catalog();
        wrong[0].engines.as_mut().unwrap()[0].gt = 16;
        assert!(parse(&serde_json::to_vec(&wrong).unwrap()).is_err());
    }
    #[tokio::test]
    async fn helper_engine_query_to_persistent_counter_and_lcd_cache() {
        let f = Fixture::new();
        let path = f.root.path().join("helper");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\n[ \"$*\" = --query-intel-engines ] || exit 1\nprintf '%s' '{}'\n",
                serde_json::to_string(&f.catalog()).unwrap()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut provider = XeSensors::new(
            path.clone(),
            f.root.path().join("pmu"),
            f.root.path().join("online"),
        );
        assert!(provider
            .collect_with(1, f.opener())
            .await
            .iter()
            .all(|r| r.value.is_none()));
        *f.value.lock().unwrap() = Some(sample(400, 1000, 1_000_000_000));
        let rows = provider.collect_with(2, f.opener()).await;
        let cache = super::super::SensorCache::default();
        cache.publish(Instant::now(), rows.clone());
        assert_eq!(
            lconnect_ipc::theme_sensor_text(
                &cache.readings(),
                &rows[0].id,
                SensorUnit::Percent,
                lconnect_ipc::TemperatureUnit::Celsius,
                1
            ),
            "40.0%"
        );
        std::fs::remove_file(&path).unwrap();
        let failed = provider.collect_with(3, f.opener()).await;
        assert!(failed.iter().all(|r| r.value.is_none()));
        assert_eq!(f.drops.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn sleep_reset_releases_engine_pairs_and_discards_cached_rows() {
        let f = Fixture::new();
        let mut p = XeSensors {
            process: QueryProcess::new("/fixture/unused".into(), "--unused", LIMIT),
            state: Some(f.state()),
        };
        let at = Instant::now();
        let initial = p
            .state
            .as_mut()
            .unwrap()
            .collect(Ok(f.catalog()), 0, at, f.opener());
        *f.value.lock().unwrap() = Some(sample(250, 1000, 1_000_000_000));
        assert!(p
            .state
            .as_mut()
            .unwrap()
            .collect(Ok(f.catalog()), 1, at + Duration::from_secs(1), f.opener())
            .iter()
            .all(|r| r.value == Some(25.0)));
        p.reset_baselines();
        assert_eq!(f.drops.load(Ordering::SeqCst), 2);
        assert!(p.state.as_ref().unwrap().previous.is_empty());
        *f.value.lock().unwrap() = Some(sample(6000, 10000, 2_000_000_000));
        let warm = p.state.as_mut().unwrap().collect(
            Ok(f.catalog()),
            2,
            at + Duration::from_secs(2),
            f.opener(),
        );
        assert_eq!(warm[0].id, initial[0].id);
        assert!(warm.iter().all(|r| r.status == SensorStatus::Unavailable));
        *f.value.lock().unwrap() = Some(sample(6100, 11000, 3_000_000_000));
        assert!(p
            .state
            .as_mut()
            .unwrap()
            .collect(Ok(f.catalog()), 3, at + Duration::from_secs(3), f.opener())
            .iter()
            .all(|r| r.value == Some(10.0)));
    }
}
