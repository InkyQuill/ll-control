//! Durable opt-in FPS source intent. All IO runs on bounded, caller-independent workers.
use anyhow::{ensure, Context};
use lconnect_ipc::FpsSourceSettings;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
pub struct FpsSourceStore {
    root: PathBuf,
    gate: Arc<Mutex<()>>,
    live: tokio::sync::watch::Sender<Option<FpsSourceSettings>>,
    fallback: FpsSourceSettings,
    jobs: crate::admission::WorkSlots,
}
impl FpsSourceStore {
    pub(crate) fn with_admission(mut self, admission: crate::admission::Admission) -> Self {
        self.jobs = self.jobs.with_admission(admission.clone());
        self
    }
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            gate: Arc::default(),
            live: tokio::sync::watch::channel(None).0,
            fallback: FpsSourceSettings::default(),
            jobs: crate::admission::WorkSlots::new(2, Default::default()),
        }
    }
    pub(crate) fn with_legacy_log(mut self, path: Option<PathBuf>) -> Self {
        if let Some(path) = path {
            let proposed = FpsSourceSettings {
                enabled: true,
                log_path: path.to_str().map(str::to_owned),
                ..Default::default()
            };
            if proposed.validate().is_ok() {
                self.fallback = proposed;
            } else {
                tracing::warn!("invalid legacy FPS log path; FPS remains disabled");
            }
        }
        self
    }
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<Option<FpsSourceSettings>> {
        self.live.subscribe()
    }
    fn publish(&self, settings: &FpsSourceSettings, cache: &crate::sensors::SensorCache) {
        let changed = self.live.borrow().as_ref() != Some(settings);
        if changed {
            self.live.send_replace(Some(settings.clone()));
            cache.invalidate_fps();
        }
    }
    fn read(&self) -> anyhow::Result<FpsSourceSettings> {
        let path = self.root.join("fps-source.json");
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(self.fallback.clone()),
            Err(e) => return Err(e.into()),
        };
        ensure!(
            file.metadata()?.is_file(),
            "FPS source settings must be a regular file"
        );
        let mut bytes = vec![];
        file.take(4097).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 4096, "FPS source settings exceed size limit");
        let settings: FpsSourceSettings = serde_json::from_slice(&bytes)?;
        settings.validate()?;
        Ok(settings)
    }
    fn run(
        &self,
        proposed: Option<FpsSourceSettings>,
        cache: &crate::sensors::SensorCache,
    ) -> anyhow::Result<FpsSourceSettings> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("FPS source settings lock poisoned"))?;
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "FPS settings root must be a real directory"
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let Some(mut next) = proposed else {
            let current = self.read()?;
            self.publish(&current, cache);
            return Ok(current);
        };
        next.validate()?;
        fs::create_dir_all(&self.root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(self.root.join(".fps-source.lock"))?;
        ensure!(
            lock.metadata()?.is_file(),
            "FPS settings lock must be a regular file"
        );
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .context("FPS source settings are being edited by another process")?;
        let current = self.read()?;
        ensure!(
            next.revision == current.revision,
            "FPS source settings changed; reload before editing"
        );
        next.revision = next
            .revision
            .checked_add(1)
            .context("FPS settings revision exhausted")?;
        next.validate()?;
        let mut tmp = tempfile::NamedTempFile::new_in(&self.root)?;
        tmp.write_all(&serde_json::to_vec_pretty(&next)?)?;
        tmp.as_file().sync_all()?;
        tmp.persist(self.root.join("fps-source.json"))?;
        // Rename is visible even if the subsequent directory durability barrier fails.
        self.publish(&next, cache);
        let absolute = std::path::absolute(&self.root)?;
        for parent in absolute.ancestors() {
            File::open(parent)?.sync_all()?;
        }
        Ok(next)
    }
    pub async fn get(
        &self,
        cache: crate::sensors::SensorCache,
    ) -> lconnect_ipc::Result<FpsSourceSettings> {
        self.work(None, cache).await
    }
    pub async fn save(
        &self,
        settings: FpsSourceSettings,
        cache: crate::sensors::SensorCache,
    ) -> lconnect_ipc::Result<FpsSourceSettings> {
        settings.validate()?;
        self.work(Some(settings), cache).await
    }
    async fn work(
        &self,
        settings: Option<FpsSourceSettings>,
        cache: crate::sensors::SensorCache,
    ) -> lconnect_ipc::Result<FpsSourceSettings> {
        let permit = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|e| lconnect_ipc::IpcError::DeviceBusy(e.to_string()))?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            this.run(settings, &cache)
                .map_err(|e| lconnect_ipc::IpcError::InvalidArgument(e.to_string()))
        })
        .await
        .map_err(|e| lconnect_ipc::IpcError::DeviceBusy(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensors::{LinuxSensors, SensorCache};
    use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
    use std::time::Instant;
    fn sample(id: &str, unit: SensorUnit) -> SensorReading {
        SensorReading {
            id: id.into(),
            label: id.into(),
            unit,
            value: Some(50.0),
            status: SensorStatus::Valid,
            age_ms: Some(0),
            observed_unix_ms: Some(1),
        }
    }
    fn proposed(revision: u64, path: &std::path::Path, enabled: bool) -> FpsSourceSettings {
        FpsSourceSettings {
            version: 1,
            revision,
            enabled,
            log_path: Some(path.to_str().unwrap().into()),
        }
    }
    fn log(path: &std::path::Path) {
        fs::write(path,"fps,frametime,cpu_load,cpu_power,gpu_load,cpu_temp,gpu_temp,gpu_core_clock,gpu_mem_clock,gpu_vram_used,gpu_power,ram_used,swap_used,process_rss,cpu_mhz,elapsed\n").unwrap();
    }
    fn append(path: &std::path::Path, ms: f64, elapsed: u64) {
        let mut f = OpenOptions::new().append(true).open(path).unwrap();
        writeln!(
            f,
            "{},{ms},0,0,0,0,0,0,0,0,0,0,0,0,0,{elapsed}",
            1000.0 / ms
        )
        .unwrap();
    }
    #[tokio::test]
    async fn default_no_creation_cas_restart_and_persisted_disable_override_legacy_environment() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state");
        let cache = SensorCache::default();
        let store = FpsSourceStore::new(path.clone());
        assert_eq!(
            store.get(cache.clone()).await.unwrap(),
            FpsSourceSettings::default()
        );
        assert!(!path.exists());
        let legacy = store.with_legacy_log(Some(root.path().join("legacy.csv")));
        let initial = legacy.get(cache.clone()).await.unwrap();
        assert!(initial.enabled);
        assert_eq!(initial.revision, 0);
        let saved = legacy
            .save(
                FpsSourceSettings {
                    enabled: false,
                    ..initial.clone()
                },
                cache.clone(),
            )
            .await
            .unwrap();
        assert_eq!(saved.revision, 1);
        assert!(legacy.save(initial, cache.clone()).await.is_err());
        let reopened =
            FpsSourceStore::new(path).with_legacy_log(Some(root.path().join("another.csv")));
        assert_eq!(reopened.get(cache).await.unwrap(), saved);
    }
    #[tokio::test]
    async fn runtime_disable_revokes_cache_and_late_samples_without_resetting_cpu_then_switches_source(
    ) {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a.csv");
        let b = root.path().join("b.csv");
        log(&a);
        log(&b);
        let store = FpsSourceStore::new(root.path().join("state"));
        let cache = SensorCache::default();
        store.get(cache.clone()).await.unwrap();
        let mut provider = LinuxSensors::new(root.path().join("hwmon"), root.path().join("proc"))
            .with_fps_settings(store.subscribe());
        assert!(!provider
            .collect()
            .iter()
            .any(|r| r.unit == SensorUnit::FramesPerSecond));
        let first = store
            .save(proposed(0, &a, true), cache.clone())
            .await
            .unwrap();
        provider.collect();
        append(&a, 20.0, 1);
        let rows = provider.collect();
        let id = rows
            .iter()
            .find(|r| r.unit == SensorUnit::FramesPerSecond)
            .unwrap()
            .id
            .clone();
        let late = Instant::now();
        cache.publish(
            late,
            vec![
                sample(&id, SensorUnit::FramesPerSecond),
                sample("cpu", SensorUnit::Percent),
            ],
        );
        let disabled = store
            .save(
                FpsSourceSettings {
                    enabled: false,
                    ..first
                },
                cache.clone(),
            )
            .await
            .unwrap();
        let values = cache.readings();
        assert_eq!(values.iter().find(|r| r.id == id).unwrap().value, None);
        assert_eq!(
            values.iter().find(|r| r.id == "cpu").unwrap().value,
            Some(50.0)
        );
        cache.publish(
            late,
            vec![
                sample(&id, SensorUnit::FramesPerSecond),
                sample("cpu", SensorUnit::Percent),
            ],
        );
        assert_eq!(
            cache.readings().iter().find(|r| r.id == id).unwrap().status,
            SensorStatus::Stale
        );
        assert!(!provider
            .collect()
            .iter()
            .any(|r| r.unit == SensorUnit::FramesPerSecond));
        store
            .save(proposed(disabled.revision, &b, true), cache.clone())
            .await
            .unwrap();
        append(&b, 10.0, 1);
        assert_eq!(
            provider
                .collect()
                .iter()
                .find(|r| r.unit == SensorUnit::FramesPerSecond)
                .unwrap()
                .value,
            None,
            "old benchmark skipped after switch"
        );
        append(&b, 40.0, 2);
        let next = provider.collect();
        let fps = next
            .iter()
            .find(|r| r.unit == SensorUnit::FramesPerSecond)
            .unwrap();
        assert_ne!(fps.id, id);
        assert_eq!(fps.value, Some(25.0));
    }
    #[tokio::test]
    async fn cancelled_save_waiter_retains_worker_admission_publish_and_cache_invalidation() {
        let root = tempfile::tempdir().unwrap();
        let gate = crate::admission::Admission::default();
        let store = FpsSourceStore::new(root.path().into()).with_admission(gate.clone());
        let cache = SensorCache::default();
        cache.publish(
            Instant::now(),
            vec![sample("old", SensorUnit::FramesPerSecond)],
        );
        let shared = store.gate.clone();
        let lock = shared.lock().unwrap();
        let mut pending = Box::pin(store.save(
            proposed(0, &root.path().join("game.csv"), true),
            cache.clone(),
        ));
        use std::future::Future;
        let mut context = std::task::Context::from_waker(futures::task::noop_waker_ref());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        drop(pending);
        assert!(gate.active() > 0);
        let drain = gate.close();
        drop(lock);
        tokio::time::timeout(std::time::Duration::from_secs(3), drain.wait_checked())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(store.live.borrow().as_ref().unwrap().revision, 1);
        assert_eq!(cache.readings()[0].value, None);
        let bytes = fs::read(root.path().join("fps-source.json")).unwrap();
        assert!(store
            .save(proposed(1, &root.path().join("other.csv"), true), cache)
            .await
            .is_err());
        assert_eq!(
            fs::read(root.path().join("fps-source.json")).unwrap(),
            bytes
        );
    }
    #[tokio::test]
    async fn invalid_paths_unsupported_corruption_and_symlink_preserve_original_intent() {
        let root = tempfile::tempdir().unwrap();
        let store = FpsSourceStore::new(root.path().into());
        let cache = SensorCache::default();
        for path in [
            "relative.csv",
            "/bad\npath",
            &format!("/{}", "a".repeat(385)),
        ] {
            let settings = FpsSourceSettings {
                version: 1,
                revision: 0,
                enabled: true,
                log_path: Some(path.into()),
            };
            assert!(store.save(settings, cache.clone()).await.is_err());
        }
        assert!(store
            .save(
                FpsSourceSettings {
                    enabled: true,
                    ..Default::default()
                },
                cache.clone()
            )
            .await
            .is_err());
        let record = root.path().join("fps-source.json");
        fs::write(&record, b"broken").unwrap();
        assert!(store.get(cache.clone()).await.is_err());
        assert_eq!(fs::read(&record).unwrap(), b"broken");
        assert!(store.live.borrow().is_none());
        fs::remove_file(&record).unwrap();
        std::os::unix::fs::symlink("missing", &record).unwrap();
        assert!(store.get(cache).await.is_err());
        assert_eq!(fs::read_link(record).unwrap(), PathBuf::from("missing"));
    }
}
