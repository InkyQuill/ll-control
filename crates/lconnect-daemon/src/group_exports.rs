//! File-backed full-group backups. No device writes; packing owns its permit
//! after caller cancellation. Metadata locks never span archive packing.
use crate::{
    profile_archive::ProfileArchives,
    profile_group::{GroupProfile, GroupSettings},
};
use lconnect_ipc::{
    DeviceId, FullProfileGroup, GroupArchivePhase as Phase, GroupArchiveReply as Reply,
    GroupArchiveSnapshot as Snapshot, GroupArchiveState as State, GroupArchiveSummary as Summary,
    IpcError, Result, MAX_PROFILE_ARCHIVE_BYTES as MAX, PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;
const CAPACITY: usize = 4;
const IDLE: Duration = Duration::from_secs(300);
struct Entry {
    state: Mutex<State>,
    file: Mutex<Option<File>>,
    cancel: AtomicBool,
    active: AtomicUsize,
    reserved: AtomicU64,
    produced: AtomicU64,
    touched: Mutex<Instant>,
    cleanup_owner: Option<crate::admission::Lease>,
}
impl Drop for Entry {
    fn drop(&mut self) {
        let file = self
            .file
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(owner) = self.cleanup_owner.take() {
            owner.cleanup(move || {
                drop(file);
                Ok(())
            });
        }
    }
}

struct Active(Arc<Entry>);
impl Active {
    fn new(entry: &Arc<Entry>) -> Self {
        entry.active.fetch_add(1, Ordering::AcqRel);
        Self(entry.clone())
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}
#[derive(Default)]
struct Registry {
    entries: BTreeMap<String, Arc<Entry>>,
}
impl Drop for Registry {
    fn drop(&mut self) {
        for entry in self.entries.values() {
            entry.cancel.store(true, Ordering::Release);
        }
    }
}
#[derive(Clone)]
pub struct GroupExports {
    archives: ProfileArchives,
    registry: Arc<AsyncMutex<Registry>>,
    packers: crate::admission::WorkSlots,
    readers: crate::admission::WorkSlots,
    reaper: Arc<AtomicBool>,
}
// These locks protect transient bookkeeping, never durable settings. Recover
// them for cleanup if a worker panicked; partial archives are never published.
fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn invalid(error: impl std::fmt::Display) -> IpcError {
    IpcError::InvalidArgument(error.to_string())
}
fn busy(error: impl std::fmt::Display) -> IpcError {
    IpcError::DeviceBusy(error.to_string())
}
fn id_valid(id: &str) -> Result<()> {
    if id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(invalid("group archive id must be32 lowercase hex digits"))
    }
}
fn snapshot(entry: &Entry) -> State {
    let mut state = locked(&entry.state).clone();
    state.produced = entry.produced.load(Ordering::Acquire);
    state
}
fn reply(entry: &Entry) -> Reply {
    Reply::Transfer {
        state: snapshot(entry),
        offset: None,
        bytes: vec![],
        eof: false,
    }
}
fn close(entry: &Entry, touch: bool) {
    drop(locked(&entry.file).take());
    entry.reserved.store(0, Ordering::Release);
    locked(&entry.state).phase = Phase::Cancelled;
    if touch {
        *locked(&entry.touched) = Instant::now();
    }
}
impl GroupExports {
    pub(crate) async fn close_staging(
        &self,
        maintenance: &crate::admission::Maintenance,
    ) -> anyhow::Result<()> {
        let lease = maintenance.child(&self.packers.admission())?;
        let mut registry = self.registry.lock().await;
        let entries = std::mem::take(&mut registry.entries);
        for entry in entries.values() {
            entry.cancel.store(true, Ordering::Release);
        }
        drop(registry);
        lease.cleanup(move || {
            drop(entries);
            Ok(())
        });
        Ok(())
    }
    pub(crate) fn with_admission(mut self, admission: crate::admission::Admission) -> Self {
        self.packers = self.packers.with_admission(admission.clone());
        self.readers = self.readers.with_admission(admission.clone());
        self
    }
    pub fn new(archives: ProfileArchives) -> Self {
        Self {
            archives,
            registry: Arc::default(),
            packers: crate::admission::WorkSlots::new(2, Default::default()),
            readers: crate::admission::WorkSlots::new(2, Default::default()),
            reaper: Arc::default(),
        }
    }
    async fn get(&self, dev: &DeviceId, group: FullProfileGroup, id: &str) -> Result<Active> {
        id_valid(id)?;
        let registry = self.registry.lock().await;
        let entry = registry
            .entries
            .get(id)
            .ok_or_else(|| invalid("group archive session unavailable or expired"))?;
        let state = locked(&entry.state);
        if &state.snapshot.dev != dev || state.snapshot.group != group {
            return Err(invalid("group archive target differs from session"));
        }
        drop(state);
        *locked(&entry.touched) = Instant::now();
        Ok(Active::new(entry))
    }
    pub async fn existing(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
        expected: &str,
    ) -> Result<Option<Reply>> {
        id_valid(id)?;
        let registry = self.registry.lock().await;
        let Some(entry) = registry.entries.get(id) else {
            return Ok(None);
        };
        let state = locked(&entry.state);
        if &state.snapshot.dev != dev
            || state.snapshot.group != group
            || state.snapshot.expected != expected
        {
            return Err(invalid("group export id already has different parameters"));
        }
        drop(state);
        *locked(&entry.touched) = Instant::now();
        Ok(Some(reply(entry)))
    }
    pub async fn begin(
        &self,
        id: String,
        source: Snapshot,
        profile: GroupProfile,
    ) -> Result<Reply> {
        id_valid(&id)?;
        self.start_reaper();
        let mut registry = self.registry.lock().await;
        if let Some(entry) = registry.entries.get(&id) {
            if locked(&entry.state).snapshot != source {
                return Err(invalid("group export id already has different parameters"));
            }
            return Ok(reply(entry));
        }
        if registry.entries.len() >= CAPACITY {
            let obsolete = registry
                .entries
                .iter()
                .find(|(_, e)| {
                    e.active.load(Ordering::Acquire) == 0 && e.reserved.load(Ordering::Acquire) == 0
                })
                .map(|(id, _)| id.clone());
            if let Some(id) = obsolete {
                registry.entries.remove(&id);
            }
        }
        if registry.entries.len() >= CAPACITY
            || registry
                .entries
                .values()
                .map(|e| e.reserved.load(Ordering::Acquire))
                .sum::<u64>()
                > MAX
        {
            return Err(busy(
                "group archive capacity exhausted; release another export",
            ));
        }
        let permit = self
            .packers
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group archive packers busy"))?;
        let entry = Arc::new(Entry {
            cleanup_owner: Some(self.packers.admission().enter().map_err(busy)?),
            state: Mutex::new(State {
                id: id.clone(),
                snapshot: source,
                phase: Phase::Preparing,
                size: None,
                produced: 0,
                last_error: None,
            }),
            file: Mutex::new(None),
            cancel: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            reserved: AtomicU64::new(MAX),
            produced: AtomicU64::new(0),
            touched: Mutex::new(Instant::now()),
        });
        let active = Active::new(&entry);
        registry.entries.insert(id, entry.clone());
        drop(registry);
        let archives = self.archives.clone();
        let monitor_lease = permit.lease();
        let observer = entry.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _active = active;
            let result = (|| -> anyhow::Result<File> {
                let mut file = tempfile::tempfile()?;
                archives.export_group(
                    &profile,
                    &mut GuardedWriter {
                        file: &mut file,
                        entry: &_active.0,
                    },
                )?;
                file.sync_all()?;
                file.seek(SeekFrom::Start(0))?;
                Ok(file)
            })();
            match result {
                Ok(file) => {
                    let mut state = locked(&_active.0.state);
                    if _active.0.cancel.load(Ordering::Acquire) {
                        drop(state);
                        drop(file);
                        close(&_active.0, true);
                    } else {
                        let size = _active.0.produced.load(Ordering::Acquire);
                        *locked(&_active.0.file) = Some(file);
                        _active.0.reserved.store(size, Ordering::Release);
                        state.size = Some(size);
                        state.phase = Phase::Ready;
                    }
                }
                Err(error) => {
                    let mut state = locked(&_active.0.state);
                    state.phase = if _active.0.cancel.load(Ordering::Acquire) {
                        Phase::Cancelled
                    } else {
                        Phase::Failed
                    };
                    state.last_error = if state.phase == Phase::Failed {
                        Some(error.to_string())
                    } else {
                        None
                    };
                    _active.0.reserved.store(0, Ordering::Release);
                }
            }
            *locked(&_active.0.touched) = Instant::now();
        });
        tokio::spawn(async move {
            let _lease = monitor_lease;
            if let Err(error) = worker.await {
                let cleanup_lease = _lease.child();
                tokio::task::spawn_blocking(move || {
                    let _lease = cleanup_lease;
                    drop(locked(&observer.file).take());
                    observer.reserved.store(0, Ordering::Release);
                    let mut state = locked(&observer.state);
                    state.phase = Phase::Failed;
                    state.last_error = Some(format!("group archive worker failed: {error}"));
                })
                .await
                .ok();
            }
        });
        Ok(reply(&entry))
    }
    pub async fn status(&self, dev: &DeviceId, group: FullProfileGroup, id: &str) -> Result<Reply> {
        let active = self.get(dev, group, id).await?;
        Ok(reply(&active.0))
    }
    pub async fn read(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
        offset: u64,
        length: usize,
    ) -> Result<Reply> {
        if length == 0 || length > CHUNK {
            return Err(invalid("group archive read must be1..=64KiB"));
        }
        let active = self.get(dev, group, id).await?;
        let permit = self
            .readers
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group archive readers busy"))?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let entry = &active.0;
            let state = snapshot(entry);
            if state.phase != Phase::Ready || entry.cancel.load(Ordering::Acquire) {
                return Err(busy("group archive is not ready"));
            }
            let size = state.size.ok_or_else(|| busy("missing archive size"))?;
            if offset > size {
                return Err(invalid("group archive offset outside file"));
            }
            let mut file = locked(&entry.file);
            let file = file
                .as_mut()
                .ok_or_else(|| busy("group archive file released"))?;
            file.seek(SeekFrom::Start(offset)).map_err(busy)?;
            let mut bytes = vec![0; u64::min(length as u64, size - offset) as usize];
            file.read_exact(&mut bytes).map_err(busy)?;
            if entry.cancel.load(Ordering::Acquire) {
                return Err(busy("group archive cancelled"));
            }
            Ok(Reply::Transfer {
                eof: offset + bytes.len() as u64 == size,
                state,
                offset: Some(offset),
                bytes,
            })
        })
        .await
        .map_err(busy)?
    }
    pub async fn cancel(&self, dev: &DeviceId, group: FullProfileGroup, id: &str) -> Result<Reply> {
        let active = self.get(dev, group, id).await?;
        let mut state = locked(&active.0.state);
        active.0.cancel.store(true, Ordering::Release);
        match state.phase {
            Phase::Preparing => state.phase = Phase::Cancelling,
            Phase::Ready => {
                state.phase = Phase::Cancelling;
                drop(state);
                let entry = active.0.clone();
                entry.active.fetch_add(1, Ordering::AcqRel);
                tokio::task::spawn_blocking(move || {
                    let active = Active(entry);
                    close(&active.0, true);
                });
                return Ok(reply(&active.0));
            }
            _ => {}
        }
        drop(state);
        Ok(reply(&active.0))
    }
    pub async fn release(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
    ) -> Result<Reply> {
        let active = self.get(dev, group, id).await?;
        let state = snapshot(&active.0);
        if matches!(state.phase, Phase::Preparing | Phase::Cancelling) {
            drop(active);
            return self.cancel(dev, group, id).await;
        }
        let registry = self.registry.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut registry = registry.blocking_lock();
            if active.0.active.load(Ordering::Acquire) > 1
                && !matches!(snapshot(&active.0).phase, Phase::Cancelled | Phase::Failed)
            {
                return Err(busy("group archive still in use"));
            }
            active.0.cancel.store(true, Ordering::Release);
            close(&active.0, true);
            let result = reply(&active.0);
            if registry
                .entries
                .get(&id)
                .is_some_and(|e| Arc::ptr_eq(e, &active.0))
            {
                registry.entries.remove(&id);
            }
            Ok(result)
        })
        .await
        .map_err(busy)?
    }
    fn start_reaper(&self) {
        if self.reaper.swap(true, Ordering::AcqRel) {
            return;
        }
        let weak = Arc::downgrade(&self.registry);
        let admission = self.packers.admission();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(15)).await;
                let Some(registry) = weak.upgrade() else {
                    break;
                };
                let Ok(lease) = admission.enter() else {
                    break;
                };
                tokio::task::spawn_blocking(move || {
                    let _lease = lease;
                    let mut registry = registry.blocking_lock();
                    registry.entries.retain(|_, e| {
                        if e.active.load(Ordering::Acquire) > 0
                            || locked(&e.touched).elapsed() < IDLE
                        {
                            return true;
                        }
                        e.cancel.store(true, Ordering::Release);
                        close(e, false);
                        false
                    });
                })
                .await
                .ok();
            }
        });
    }
}
struct GuardedWriter<'a> {
    file: &'a mut File,
    entry: &'a Entry,
}
impl Write for GuardedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.entry.cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::other("group export cancelled"));
        }
        if self
            .entry
            .produced
            .load(Ordering::Acquire)
            .saturating_add(bytes.len() as u64)
            > MAX
        {
            return Err(std::io::Error::other("group export exceeds budget"));
        }
        let written = self.file.write(bytes)?;
        self.entry
            .produced
            .fetch_add(written as u64, Ordering::AcqRel);
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
pub fn summary(profile: &GroupProfile) -> Result<Summary> {
    let mut info = Summary {
        version: profile.version,
        slots: profile
            .profiles
            .iter()
            .map(|p| lconnect_ipc::GroupArchiveSlot {
                group: p.content.group(),
                slot: p.slot,
                name: p.name.clone(),
            })
            .collect(),
        lcd: None,
        theme_name: None,
        media_key: None,
        fan_manual_percent: None,
        pump_manual: None,
        cooling: None,
        fan_led_mode: None,
        pump_led_mode: None,
        dynamic_rgb: None,
    };
    match &profile.content {
        GroupSettings::Screen(group) => {
            let settings = crate::persistence::DeviceSettings {
                lcd: group.lcd.clone(),
                ..Default::default()
            };
            info.lcd = Some(crate::actor::screen_group::screen_config(&settings)?);
            info.theme_name = group.theme.as_ref().map(|t| t.name.clone());
            info.media_key = group.media_clip.clone();
        }
        GroupSettings::FanAndLighting(group) => {
            info.fan_manual_percent = group
                .fan
                .has_manual_setting()
                .then_some(group.fan.pwm_percent);
            info.pump_manual =
                group
                    .pump
                    .has_manual_setting()
                    .then_some(lconnect_ipc::GroupManualPump {
                        percent: group.pump.pwm_percent,
                        source: if matches!(
                            group.pump.source.as_str(),
                            "application" | "Application"
                        ) {
                            lconnect_ipc::PwmSource::Application
                        } else {
                            lconnect_ipc::PwmSource::Mcu
                        },
                    });
            info.cooling = Some(group.cooling.clone());
            info.fan_led_mode = group.lighting.fan.as_ref().map(|s| s.mode.clone());
            info.pump_led_mode = group.lighting.pump.as_ref().map(|s| s.mode.clone());
            info.dynamic_rgb = group.dynamic_rgb.clone();
        }
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        persistence::{DeviceSettings, PersistenceStore},
        profile_group::{GroupKind, GroupProfile},
    };
    fn source() -> (
        tempfile::TempDir,
        PersistenceStore,
        GroupExports,
        Snapshot,
        GroupProfile,
    ) {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        let data = GroupProfile::capture(
            &DeviceSettings::default(),
            &Default::default(),
            GroupKind::Screen,
        )
        .unwrap();
        let snap = Snapshot {
            dev: DeviceId::new("mock:source"),
            group: FullProfileGroup::Screen,
            expected: serde_json::to_string(&data.revision).unwrap(),
            summary: summary(&data.profile).unwrap(),
        };
        let exports = GroupExports::new(ProfileArchives::new(
            store.theme_assets(),
            store.theme_fonts(),
            store.media_assets(),
        ));
        (root, store, exports, snap, data.profile)
    }
    #[tokio::test]
    async fn maintenance_waits_for_cancelled_export_packer_and_final_entry_cleanup() {
        let (_root, store, exports, snap, profile) = source();
        let gate = store.admission();
        let exports = exports.with_admission(gate.clone());
        let assets = store.theme_assets();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (finish, blocked) = std::sync::mpsc::channel();
        let lock = tokio::task::spawn_blocking(move || {
            assets.with_catalog(move |_| {
                entered.send(()).unwrap();
                blocked.recv().unwrap();
                Ok(())
            })
        });
        ready.await.unwrap();
        exports
            .begin("11111111111111111111111111111111".into(), snap, profile)
            .await
            .unwrap();
        let maintenance = gate.begin_maintenance().unwrap();
        exports.close_staging(&maintenance).await.unwrap();
        assert!(exports.registry.lock().await.entries.is_empty());
        let drain = tokio::spawn(maintenance.wait_checked());
        tokio::task::yield_now().await;
        assert!(!drain.is_finished());
        finish.send(()).unwrap();
        lock.await.unwrap().unwrap();
        tokio::time::timeout(Duration::from_secs(3), drain)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(gate.active(), 0);
    }
    #[tokio::test]
    async fn cancelled_packer_retains_admission_until_catalog_read_exits_and_other_sessions_cannot_bypass_limits(
    ) {
        let (_root, store, exports, snap, profile) = source();
        let assets = store.theme_assets();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (release, hold) = std::sync::mpsc::channel();
        let locked = tokio::task::spawn_blocking(move || {
            assets.with_catalog(move |_| {
                entered.send(()).unwrap();
                hold.recv().unwrap();
                Ok(())
            })
        });
        ready.await.unwrap();
        let first = "11111111111111111111111111111111";
        let second = "22222222222222222222222222222222";
        exports
            .begin(first.into(), snap.clone(), profile.clone())
            .await
            .unwrap();
        exports
            .begin(second.into(), snap.clone(), profile.clone())
            .await
            .unwrap();
        let result = exports.cancel(&snap.dev, snap.group, first).await.unwrap();
        let Reply::Transfer { state, .. } = result else {
            panic!()
        };
        assert_eq!(state.phase, Phase::Cancelling);
        assert!(
            exports
                .begin(
                    "33333333333333333333333333333333".into(),
                    snap.clone(),
                    profile
                )
                .await
                .is_err(),
            "cancellation cannot release live packer's quota/permit"
        );
        exports.status(&snap.dev, snap.group, second).await.unwrap();
        release.send(()).unwrap();
        locked.await.unwrap().unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let Reply::Transfer { state, .. } =
                    exports.status(&snap.dev, snap.group, first).await.unwrap()
                else {
                    panic!()
                };
                if state.phase == Phase::Cancelled {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        exports.release(&snap.dev, snap.group, first).await.unwrap();
        assert!(exports.status(&snap.dev, snap.group, first).await.is_err());
        exports.cancel(&snap.dev, snap.group, second).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let Reply::Transfer { state, .. } =
                    exports.status(&snap.dev, snap.group, second).await.unwrap()
                else {
                    panic!()
                };
                if state.phase == Phase::Cancelled {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        exports
            .release(&snap.dev, snap.group, second)
            .await
            .unwrap();
    }
}
