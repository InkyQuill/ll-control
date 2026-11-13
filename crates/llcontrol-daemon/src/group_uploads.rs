//! Bounded file-backed preparation of full-group imports. All disk work owns
//! admission after RPC cancellation; metadata reads do not wait for inspection.
use crate::{profile_archive::ProfileArchives, profile_group::GroupKind};
use llcontrol_ipc::{
    DeviceId, FullProfileGroup, GroupArchiveSnapshot, GroupUploadPhase as Phase,
    GroupUploadState as State, IpcError, Result, MAX_PROFILE_ARCHIVE_BYTES as MAX,
    PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
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
use tokio::sync::{Mutex as AsyncMutex, Semaphore};
const CAPACITY: usize = 4;
const IDLE: Duration = Duration::from_secs(300);
#[derive(Default)]
struct Data {
    file: Option<File>,
    windows: Option<crate::profile_archive::StagedWindowsScreen>,
}
struct Entry {
    state: Mutex<State>,
    windows: Option<(
        llcontrol_ipc::WindowsScreenSource,
        crate::profile_group::GroupProfile,
    )>,
    data: Mutex<Data>,
    busy: Arc<Semaphore>,
    cancel: AtomicBool,
    active: AtomicUsize,
    reserved: AtomicU64,
    touched: Mutex<Instant>,
    cleanup_owner: Option<crate::admission::Lease>,
}
impl Drop for Entry {
    fn drop(&mut self) {
        let data = std::mem::take(self.data.get_mut().unwrap_or_else(|e| e.into_inner()));
        if let Some(owner) = self.cleanup_owner.take() {
            owner.cleanup(move || {
                drop(data);
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
        for e in self.entries.values() {
            e.cancel.store(true, Ordering::Release);
        }
    }
}
#[derive(Clone)]
pub struct GroupUploads {
    registry: Arc<AsyncMutex<Registry>>,
    jobs: crate::admission::WorkSlots,
    reaper: Arc<AtomicBool>,
    finish_jobs: crate::admission::WorkSlots,
}
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn invalid(e: impl std::fmt::Display) -> IpcError {
    IpcError::InvalidArgument(e.to_string())
}
fn busy(e: impl std::fmt::Display) -> IpcError {
    IpcError::DeviceBusy(e.to_string())
}
fn valid_id(id: &str) -> Result<()> {
    crate::profile_group::validate_operation_id(id).map_err(invalid)
}
fn state(e: &Entry) -> State {
    lock(&e.state).clone()
}
// Call on a blocking worker, while holding the entry operation permit.
fn clear(e: &Entry, phase: Phase, error: Option<String>) {
    drop(std::mem::take(&mut *lock(&e.data)));
    e.reserved.store(0, Ordering::Release);
    let mut state = lock(&e.state);
    state.phase = phase;
    state.last_error = error;
    *lock(&e.touched) = Instant::now();
}
impl Default for GroupUploads {
    fn default() -> Self {
        Self::new()
    }
}
/// Raw archive held by one admitted Finish task. Configuration mutations are
/// exclusively the responsibility of the selected device's actor transaction.
pub(crate) struct FinishSource {
    _admission: crate::admission::Lease,
    entry: Arc<Entry>,
}
impl FinishSource {
    pub fn descriptor(&self) -> State {
        state(&self.entry)
    }
    pub async fn prepare(
        self,
        archives: ProfileArchives,
        publication: Arc<crate::asset_publication::Publication>,
    ) -> Result<Arc<crate::profile_archive::PreparedGroup>> {
        tokio::task::spawn_blocking(move || {
            let _admission = self._admission;
            let current = state(&self.entry);
            let mut data = lock(&self.entry.data);
            if self.entry.windows.is_some() {
                let staged = data
                    .windows
                    .take()
                    .ok_or_else(|| invalid("missing private Windows Screen preparation"))?;
                if Some(staged.source_sha256()) != current.source_sha256.as_deref() {
                    return Err(invalid("private Windows Screen identity changed"));
                }
                return staged
                    .publish(&archives, Some(publication), &self.entry.cancel)
                    .map(Arc::new)
                    .map_err(invalid);
            }
            let file = data
                .file
                .as_mut()
                .ok_or_else(|| invalid("missing Finish archive"))?;
            file.seek(SeekFrom::Start(0)).map_err(invalid)?;
            let prepared = archives
                .prepare_group_scoped(file, Some(publication))
                .map_err(invalid)?;
            if Some(prepared.source_sha256()) != current.source_sha256.as_deref() {
                return Err(invalid("validated archive identity changed"));
            }
            let kind = match current.target.group {
                FullProfileGroup::Screen => GroupKind::Screen,
                FullProfileGroup::FanAndLighting => GroupKind::FanAndLighting,
            };
            if prepared.profile().kind() != kind {
                return Err(invalid("Finish archive group mismatch"));
            }
            Ok(Arc::new(prepared))
        })
        .await
        .map_err(busy)?
    }
}
impl GroupUploads {
    pub(crate) async fn close_staging(
        &self,
        maintenance: &crate::admission::Maintenance,
    ) -> anyhow::Result<()> {
        let lease = maintenance.child(&self.jobs.admission())?;
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
        self.jobs = self.jobs.with_admission(admission.clone());
        self.finish_jobs = self.finish_jobs.with_admission(admission.clone());
        self
    }
    pub fn new() -> Self {
        Self {
            registry: Arc::default(),
            jobs: crate::admission::WorkSlots::new(2, Default::default()),
            reaper: Arc::default(),
            finish_jobs: crate::admission::WorkSlots::new(2, Default::default()),
        }
    }
    async fn get(&self, dev: &DeviceId, group: FullProfileGroup, id: &str) -> Result<Active> {
        valid_id(id)?;
        let registry = self.registry.lock().await;
        let entry = registry
            .entries
            .get(id)
            .ok_or_else(|| invalid("group upload unavailable or expired"))?;
        let current = lock(&entry.state);
        if &current.target.dev != dev || current.target.group != group {
            return Err(invalid("group upload target mismatch"));
        }
        drop(current);
        *lock(&entry.touched) = Instant::now();
        Ok(Active::new(entry))
    }
    pub async fn existing(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
        size: u64,
        expected: &str,
    ) -> Result<Option<State>> {
        self.existing_source(dev, group, id, size, expected, None)
            .await
    }
    pub(crate) async fn existing_source(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
        size: u64,
        expected: &str,
        source: Option<&llcontrol_ipc::WindowsScreenSource>,
    ) -> Result<Option<State>> {
        valid_id(id)?;
        let registry = self.registry.lock().await;
        let Some(e) = registry.entries.get(id) else {
            return Ok(None);
        };
        let current = state(e);
        if &current.target.dev != dev
            || current.target.group != group
            || current.target.expected != expected
            || current.size != size
            || e.windows.as_ref().map(|(options, _)| options) != source
        {
            return Err(invalid("group upload id has different parameters"));
        }
        *lock(&e.touched) = Instant::now();
        Ok(Some(current))
    }
    pub async fn begin(
        &self,
        id: String,
        target: GroupArchiveSnapshot,
        size: u64,
    ) -> Result<State> {
        self.begin_source(id, target, size, None).await
    }
    pub(crate) async fn begin_source(
        &self,
        id: String,
        target: GroupArchiveSnapshot,
        size: u64,
        windows: Option<(
            llcontrol_ipc::WindowsScreenSource,
            crate::profile_group::GroupProfile,
        )>,
    ) -> Result<State> {
        if windows.as_ref().is_some_and(|(_, profile)| {
            target.group != FullProfileGroup::Screen || profile.kind() != GroupKind::Screen
        }) {
            return Err(invalid("Windows Screen source requires a Screen target"));
        }
        valid_id(&id)?;
        if !(44..=MAX).contains(&size) {
            return Err(invalid("group upload size outside archive limit"));
        }
        self.start_reaper();
        let mut registry = self.registry.lock().await;
        if let Some(e) = registry.entries.get(&id) {
            let current = state(e);
            if current.target != target
                || current.size != size
                || e.windows.as_ref().map(|(options, _)| options)
                    != windows.as_ref().map(|(options, _)| options)
            {
                return Err(invalid("group upload id has different parameters"));
            }
            return Ok(current);
        }
        if registry.entries.len() >= CAPACITY {
            let old = registry
                .entries
                .iter()
                .find(|(_, e)| {
                    e.active.load(Ordering::Acquire) == 0 && e.reserved.load(Ordering::Acquire) == 0
                })
                .map(|(id, _)| id.clone());
            if let Some(id) = old {
                registry.entries.remove(&id);
            }
        }
        let reserved: u64 = registry
            .entries
            .values()
            .map(|e| e.reserved.load(Ordering::Acquire))
            .sum();
        if registry.entries.len() >= CAPACITY || reserved.saturating_add(size) > 2 * MAX {
            return Err(busy("group upload capacity exhausted"));
        }
        let entry = Arc::new(Entry {
            cleanup_owner: Some(self.jobs.admission().enter().map_err(busy)?),
            windows,
            state: Mutex::new(State {
                id: id.clone(),
                target,
                size,
                received: 0,
                phase: Phase::Uploading,
                summary: None,
                source_sha256: None,
                last_error: None,
                committed: None,
            }),
            data: Mutex::default(),
            busy: Arc::new(Semaphore::new(1)),
            cancel: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            reserved: AtomicU64::new(size),
            touched: Mutex::new(Instant::now()),
        });
        // Reserve metadata first. The first chunk creates the file in its owned
        // blocking worker; Begin never opens files on the runtime thread.
        registry.entries.insert(id, entry.clone());
        Ok(state(&entry))
    }
    async fn work<F>(&self, active: Active, operation: F) -> Result<State>
    where
        F: FnOnce(&Entry) -> Result<()> + Send + 'static,
    {
        let permit = active
            .0
            .busy
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group upload operation busy"))?;
        let job = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group upload workers busy"))?;
        let monitor_lease = job.lease();
        let observer = active.0.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _job = job;
            if active.0.cancel.load(Ordering::Acquire) {
                clear(&active.0, Phase::Cancelled, None);
                return Ok(state(&active.0));
            }
            let result = operation(&active.0);
            if active.0.cancel.load(Ordering::Acquire) {
                clear(&active.0, Phase::Cancelled, None);
            } else if let Err(error) = &result {
                // A late Chunk rejected after inspection must not destroy an
                // already validated file. Errors during actual upload/inspect
                // are terminal; they may follow a partial disk write.
                if matches!(state(&active.0).phase, Phase::Uploading | Phase::Inspecting) {
                    clear(&active.0, Phase::Failed, Some(error.to_string()));
                }
            }
            *lock(&active.0.touched) = Instant::now();
            result.map(|()| state(&active.0))
        });
        // An owned monitor survives a dropped RPC future. In particular, a
        // worker panic must not leave an apparently Inspecting session forever.
        tokio::spawn(async move {
            let _lease = monitor_lease;
            match worker.await {
                Ok(result) => result,
                Err(error) => {
                    let message = format!("group upload worker failed: {error}");
                    let cleanup_lease = _lease.child();
                    tokio::task::spawn_blocking(move || {
                        let _lease = cleanup_lease;
                        clear(&observer, Phase::Failed, Some(message))
                    })
                    .await
                    .map_err(busy)?;
                    Err(busy(error))
                }
            }
        })
        .await
        .map_err(busy)?
    }
    pub async fn chunk(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
        offset: u64,
        bytes: Vec<u8>,
    ) -> Result<State> {
        if bytes.is_empty() || bytes.len() > CHUNK {
            return Err(invalid("group chunk must be1..=64KiB"));
        }
        let active = self.get(dev, group, id).await?;
        let current = state(&active.0);
        if current.phase != Phase::Uploading
            || offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > current.size)
            || offset > current.received
        {
            return Err(invalid("group chunk outside upload window"));
        }
        self.work(active, move |entry| {
            let current = state(entry);
            if current.phase != Phase::Uploading {
                return Err(busy("group upload is not writable"));
            }
            let mut data = lock(&entry.data);
            if data.file.is_none() {
                data.file = Some(tempfile::tempfile().map_err(invalid)?);
            }
            let file = data.file.as_mut().unwrap();
            if offset < current.received {
                if offset + bytes.len() as u64 > current.received {
                    return Err(invalid("partially overlapping group chunk"));
                }
                let mut previous = vec![0; bytes.len()];
                file.seek(SeekFrom::Start(offset)).map_err(invalid)?;
                file.read_exact(&mut previous).map_err(invalid)?;
                if previous != bytes {
                    return Err(invalid("repeated group chunk differs"));
                }
                return Ok(());
            }
            file.seek(SeekFrom::Start(offset)).map_err(invalid)?;
            file.write_all(&bytes).map_err(invalid)?;
            lock(&entry.state).received = offset + bytes.len() as u64;
            Ok(())
        })
        .await
    }
    pub async fn inspect(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
    ) -> Result<State> {
        self.inspect_source(dev, group, id, None).await
    }
    pub(crate) async fn inspect_source(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
        target_archives: Option<ProfileArchives>,
    ) -> Result<State> {
        let active = self.get(dev, group, id).await?;
        let current = state(&active.0);
        if matches!(current.phase, Phase::Inspecting | Phase::Prepared) {
            return Ok(current);
        }
        if current.phase != Phase::Uploading || current.received != current.size {
            return Err(invalid("group upload is incomplete"));
        }
        // Start a background operation, with bounded entry/global admission.
        // Inspect returns metadata immediately; Status remains available while
        // validation/decoding holds the data lock.
        let this = self.clone();
        let observer = active.0.clone();
        let (started, ready) = tokio::sync::oneshot::channel();
        let inspect_lease = self.jobs.admission().enter().map_err(busy)?;
        tokio::spawn(async move {
            let _lease = inspect_lease;
            let result = this
                .work(active, move |entry| {
                    lock(&entry.state).phase = Phase::Inspecting;
                    let _ = started.send(());
                    let mut data = lock(&entry.data);
                    let file = data
                        .file
                        .as_mut()
                        .ok_or_else(|| invalid("missing group archive"))?;
                    file.sync_all().map_err(invalid)?;
                    file.seek(SeekFrom::Start(0)).map_err(invalid)?;
                    if let Some((options, target)) = &entry.windows {
                        let archives = target_archives
                            .as_ref()
                            .ok_or_else(|| invalid("missing target resource stores"))?;
                        let bindings = llcontrol_profile::linux_screen::parse_bindings(
                            options.bindings_json.as_bytes(),
                        )
                        .map_err(invalid)?;
                        let resources = llcontrol_profile::screen_plan::parse_resource_map(
                            options.resources_json.as_bytes(),
                        )
                        .map_err(invalid)?;
                        let (source, plan) = llcontrol_profile::screen_plan::inspect_and_prepare(
                            file,
                            options.record,
                            &bindings,
                            &resources,
                            &entry.cancel,
                        )
                        .map_err(invalid)?;
                        let staged = archives
                            .stage_windows_screen(&source, plan, target, &entry.cancel)
                            .map_err(invalid)?;
                        let summary = crate::group_exports::summary(staged.profile())?;
                        let source_sha256 = staged.source_sha256().to_owned();
                        data.windows = Some(staged);
                        let mut current = lock(&entry.state);
                        if !entry.cancel.load(Ordering::Acquire) {
                            current.summary = Some(summary);
                            current.source_sha256 = Some(source_sha256);
                            current.phase = Phase::Prepared;
                        }
                        return Ok(());
                    }
                    // Validate in a private quarantine: Inspect must not add
                    // uploads to the target's legacy/shared gallery catalog.
                    let quarantine = tempfile::tempdir().map_err(invalid)?;
                    let store = crate::persistence::PersistenceStore::new_with_root(
                        quarantine.path().into(),
                    );
                    let archives = ProfileArchives::new(
                        store.theme_assets(),
                        store.theme_fonts(),
                        store.media_assets(),
                    );
                    let profile = archives.inspect_group(file).map_err(invalid)?;
                    let kind = match group {
                        FullProfileGroup::Screen => GroupKind::Screen,
                        FullProfileGroup::FanAndLighting => GroupKind::FanAndLighting,
                    };
                    if profile.kind() != kind {
                        return Err(invalid("archive contains a different group"));
                    }
                    if entry.cancel.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    file.seek(SeekFrom::Start(0)).map_err(invalid)?;
                    let prepared = archives.prepare_group(file).map_err(invalid)?;
                    let summary = crate::group_exports::summary(prepared.profile())?;
                    let source_sha256 = prepared.source_sha256().to_owned();
                    drop(prepared);
                    drop(archives);
                    drop(store);
                    quarantine.close().map_err(invalid)?;
                    let mut state = lock(&entry.state);
                    if !entry.cancel.load(Ordering::Acquire) {
                        state.summary = Some(summary);
                        state.source_sha256 = Some(source_sha256);
                        state.phase = Phase::Prepared;
                    }
                    Ok(())
                })
                .await;
            result
        });
        // If admission failed, the sender is dropped and callers can retry.
        ready
            .await
            .map_err(|_| busy("group inspection admission failed; retry"))?;
        Ok(state(&observer))
    }
    pub(crate) async fn finish<F, Fut>(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
        apply: F,
    ) -> Result<State>
    where
        F: FnOnce(FinishSource) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<llcontrol_ipc::FullGroupCommit>> + Send + 'static,
    {
        let active = self.get(dev, group, id).await?;
        let current = state(&active.0);
        if matches!(
            current.phase,
            Phase::Importing | Phase::Imported | Phase::RecoveryRequired
        ) {
            return Ok(current);
        }
        if current.phase != Phase::Prepared {
            return Err(invalid("inspect complete archive before Finish"));
        }
        let permit = active
            .0
            .busy
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group upload operation busy"))?;
        let job = self
            .finish_jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy("group Finish workers busy"))?;
        {
            let mut current = lock(&active.0.state);
            if current.phase != Phase::Prepared || active.0.cancel.load(Ordering::Acquire) {
                return Err(busy("Finish superseded by cancellation"));
            }
            current.phase = Phase::Importing;
        }
        let observer = active.0.clone();
        let result_entry = observer.clone();
        let monitor_lease = job.lease();
        let worker = tokio::spawn(async move {
            let _permit = permit;
            let _job = job;
            let result = apply(FinishSource {
                _admission: _job.lease(),
                entry: active.0.clone(),
            })
            .await;
            let cleanup_lease = _job.lease();
            // Closing staging is part of the owned operation, before terminal
            // metadata/reservation becomes reclaimable.
            tokio::task::spawn_blocking(move || {
                let _lease = cleanup_lease;
                let _active = active;
                match result {
                    Ok(commit) => {
                        drop(std::mem::take(&mut *lock(&_active.0.data)));
                        _active.0.reserved.store(0, Ordering::Release);
                        let mut current = lock(&_active.0.state);
                        current.committed = Some(commit);
                        current.last_error = None;
                        current.phase = Phase::Imported;
                        *lock(&_active.0.touched) = Instant::now();
                    }
                    Err(error) => {
                        let phase = if matches!(error, IpcError::InvalidArgument(_)) {
                            Phase::Failed
                        } else {
                            Phase::RecoveryRequired
                        };
                        clear(&_active.0, phase, Some(error.to_string()));
                    }
                }
            })
            .await
            .map_err(busy)
        });
        tokio::spawn(async move {
            let _lease = monitor_lease;
            let error = match worker.await {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error.to_string()),
                Err(error) => Some(error.to_string()),
            };
            if let Some(error) = error {
                let message = format!("Finish outcome requires readback: {error}");
                let cleanup_lease = _lease.child();
                tokio::task::spawn_blocking(move || {
                    let _lease = cleanup_lease;
                    clear(&observer, Phase::RecoveryRequired, Some(message))
                })
                .await
                .ok();
            }
        });
        Ok(state(&result_entry))
    }
    pub async fn status(&self, dev: &DeviceId, group: FullProfileGroup, id: &str) -> Result<State> {
        Ok(state(&self.get(dev, group, id).await?.0))
    }
    pub async fn cancel(&self, dev: &DeviceId, group: FullProfileGroup, id: &str) -> Result<State> {
        let active = self.get(dev, group, id).await?;
        let mut current = lock(&active.0.state);
        if matches!(
            current.phase,
            Phase::Importing | Phase::Imported | Phase::RecoveryRequired
        ) {
            return Ok(current.clone());
        }
        active.0.cancel.store(true, Ordering::Release);
        if matches!(
            current.phase,
            Phase::Cancelled | Phase::Failed | Phase::Cancelling
        ) {
            return Ok(current.clone());
        }
        current.phase = Phase::Cancelling;
        drop(current);
        let jobs = self.jobs.clone();
        let entry = active.0.clone();
        let cleanup = Active::new(&entry);
        let cleanup_lease = self.jobs.admission().enter().map_err(busy)?;
        tokio::spawn(async move {
            let _lease = cleanup_lease;
            let Ok(job) = jobs.acquire_owned().await else {
                return;
            };
            let Ok(permit) = entry.busy.clone().acquire_owned().await else {
                return;
            };
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let _job = job;
                clear(&cleanup.0, Phase::Cancelled, None);
            })
            .await
            .ok();
        });
        Ok(state(&active.0))
    }
    pub async fn release(
        &self,
        dev: &DeviceId,
        group: FullProfileGroup,
        id: &str,
    ) -> Result<State> {
        let active = self.get(dev, group, id).await?;
        let current = state(&active.0);
        if !matches!(
            current.phase,
            Phase::Cancelled | Phase::Failed | Phase::Imported | Phase::RecoveryRequired
        ) {
            drop(active);
            return self.cancel(dev, group, id).await;
        }
        let mut registry = self.registry.lock().await;
        // Terminal phase is published only after file/pins were dropped. A
        // late release must never remove a new session reusing the same id.
        if registry
            .entries
            .get(id)
            .is_some_and(|e| Arc::ptr_eq(e, &active.0))
        {
            registry.entries.remove(id);
        }
        Ok(current)
    }
    fn start_reaper(&self) {
        if self.reaper.swap(true, Ordering::AcqRel) {
            return;
        }
        let weak = Arc::downgrade(&self.registry);
        let admission = self.jobs.admission();
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
                        if e.active.load(Ordering::Acquire) > 0 || lock(&e.touched).elapsed() < IDLE
                        {
                            return true;
                        }
                        e.cancel.store(true, Ordering::Release);
                        clear(e, Phase::Cancelled, None);
                        false
                    });
                })
                .await
                .ok();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{persistence::DeviceSettings, profile_group::GroupProfile};
    #[tokio::test]
    async fn maintenance_drains_removed_idle_entries_and_cancelled_active_file_owner() {
        let profile = GroupProfile::capture(
            &DeviceSettings::default(),
            &Default::default(),
            GroupKind::Screen,
        )
        .unwrap();
        let target = GroupArchiveSnapshot {
            dev: DeviceId::new("maintenance-upload"),
            group: FullProfileGroup::Screen,
            expected: serde_json::to_string(&profile.revision).unwrap(),
            summary: crate::group_exports::summary(&profile.profile).unwrap(),
        };
        let gate = crate::admission::Admission::default();
        let uploads = GroupUploads::new().with_admission(gate.clone());
        let id = "11111111111111111111111111111111";
        uploads.begin(id.into(), target.clone(), 100).await.unwrap();
        let active = uploads.get(&target.dev, target.group, id).await.unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (finish, blocked) = std::sync::mpsc::channel();
        let copy = uploads.clone();
        let waiter = tokio::spawn(async move {
            copy.work(active, move |entry| {
                lock(&entry.data).file = Some(tempfile::tempfile().unwrap());
                started.send(()).unwrap();
                blocked.recv().unwrap();
                Ok(())
            })
            .await
        });
        ready.await.unwrap();
        waiter.abort();
        let _ = waiter.await;
        let maintenance = gate.begin_maintenance().unwrap();
        uploads.close_staging(&maintenance).await.unwrap();
        assert!(uploads.registry.lock().await.entries.is_empty());
        assert!(uploads
            .begin("22222222222222222222222222222222".into(), target, 100)
            .await
            .is_err());
        let drain = tokio::spawn(maintenance.wait_checked());
        tokio::task::yield_now().await;
        assert!(!drain.is_finished());
        finish.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), drain)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(gate.active(), 0);
    }
    #[tokio::test]
    async fn abandoned_upload_worker_and_cancel_retain_budget_until_real_work_exits() {
        let profile = GroupProfile::capture(
            &DeviceSettings::default(),
            &Default::default(),
            GroupKind::Screen,
        )
        .unwrap();
        let target = GroupArchiveSnapshot {
            dev: DeviceId::new("mock-upload"),
            group: FullProfileGroup::Screen,
            expected: serde_json::to_string(&profile.revision).unwrap(),
            summary: crate::group_exports::summary(&profile.profile).unwrap(),
        };
        let uploads = GroupUploads::new();
        let first = "11111111111111111111111111111111";
        uploads
            .begin(first.into(), target.clone(), MAX)
            .await
            .unwrap();
        uploads
            .begin(
                "22222222222222222222222222222222".into(),
                target.clone(),
                MAX,
            )
            .await
            .unwrap();
        let active = uploads.get(&target.dev, target.group, first).await.unwrap();
        let worker_uploads = uploads.clone();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (finish, hold) = std::sync::mpsc::channel();
        let waiter = tokio::spawn(async move {
            worker_uploads
                .work(active, move |_| {
                    entered.send(()).unwrap();
                    hold.recv().unwrap();
                    Ok(())
                })
                .await
        });
        ready.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert_eq!(
            uploads
                .cancel(&target.dev, target.group, first)
                .await
                .unwrap()
                .phase,
            Phase::Cancelling
        );
        assert!(uploads
            .begin(
                "33333333333333333333333333333333".into(),
                target.clone(),
                MAX
            )
            .await
            .is_err());
        tokio::time::timeout(
            Duration::from_millis(200),
            uploads.status(&target.dev, target.group, first),
        )
        .await
        .unwrap()
        .unwrap();
        finish.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if uploads
                    .status(&target.dev, target.group, first)
                    .await
                    .unwrap()
                    .phase
                    == Phase::Cancelled
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        uploads
            .begin(
                "33333333333333333333333333333333".into(),
                target.clone(),
                MAX,
            )
            .await
            .unwrap();
        uploads
            .release(&target.dev, target.group, first)
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod finish_tests {
    use super::*;
    #[tokio::test]
    async fn admitted_finish_survives_lost_reply_and_cannot_be_cancelled_or_hide_runtime_capacity()
    {
        let profile = crate::profile_group::GroupProfile::capture(
            &crate::persistence::DeviceSettings::default(),
            &Default::default(),
            GroupKind::Screen,
        )
        .unwrap();
        let target = GroupArchiveSnapshot {
            dev: DeviceId::new("mock-finish"),
            group: FullProfileGroup::Screen,
            expected: serde_json::to_string(&profile.revision).unwrap(),
            summary: crate::group_exports::summary(&profile.profile).unwrap(),
        };
        let uploads = GroupUploads::new();
        let ids = [
            "11111111111111111111111111111111",
            "22222222222222222222222222222222",
            "33333333333333333333333333333333",
        ];
        for id in ids {
            uploads.begin(id.into(), target.clone(), 44).await.unwrap();
            let active = uploads.get(&target.dev, target.group, id).await.unwrap();
            lock(&active.0.state).phase = Phase::Prepared;
        }
        let mut releases = Vec::new();
        for id in &ids[..2] {
            let (release, hold) = tokio::sync::oneshot::channel();
            releases.push(release);
            // Dropping this response models a lost Finish acknowledgement.
            uploads
                .finish(&target.dev, target.group, id, move |_| async move {
                    let _ = hold.await;
                    Err(busy("simulated unknown outcome"))
                })
                .await
                .unwrap();
        }
        assert_eq!(
            uploads
                .cancel(&target.dev, target.group, ids[0])
                .await
                .unwrap()
                .phase,
            Phase::Importing
        );
        assert_eq!(
            uploads
                .release(&target.dev, target.group, ids[0])
                .await
                .unwrap()
                .phase,
            Phase::Importing
        );
        assert!(uploads
            .finish(&target.dev, target.group, ids[2], |_| async {
                Err(invalid("not admitted"))
            })
            .await
            .is_err());
        assert_eq!(
            uploads
                .status(&target.dev, target.group, ids[2])
                .await
                .unwrap()
                .phase,
            Phase::Prepared
        );
        assert_eq!(
            uploads
                .finish(&target.dev, target.group, ids[0], |_| async {
                    panic!("repeat must not start a second job")
                })
                .await
                .unwrap()
                .phase,
            Phase::Importing
        );
        for release in releases {
            release.send(()).unwrap();
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if uploads
                    .status(&target.dev, target.group, ids[0])
                    .await
                    .unwrap()
                    .phase
                    == Phase::RecoveryRequired
                    && uploads
                        .status(&target.dev, target.group, ids[1])
                        .await
                        .unwrap()
                        .phase
                        == Phase::RecoveryRequired
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        uploads
            .finish(&target.dev, target.group, ids[2], |_| async {
                Err(invalid("known rejection"))
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if uploads
                    .status(&target.dev, target.group, ids[2])
                    .await
                    .unwrap()
                    .phase
                    == Phase::Failed
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
}

pub(crate) mod preview;
