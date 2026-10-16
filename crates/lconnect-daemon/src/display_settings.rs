//! Durable global display units. All IO runs on bounded, caller-independent workers.
use anyhow::{ensure, Context};
use lconnect_ipc::{DisplaySettings, DisplayTemperatureUnit};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
#[derive(Clone)]
pub struct DisplaySettingsStore {
    root: PathBuf,
    gate: Arc<Mutex<()>>,
    unit: Arc<AtomicBool>,
    jobs: crate::admission::WorkSlots,
}
impl DisplaySettingsStore {
    pub(crate) fn with_admission(mut self, admission: crate::admission::Admission) -> Self {
        self.jobs = self.jobs.with_admission(admission.clone());
        self
    }
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            gate: Arc::default(),
            unit: Arc::default(),
            jobs: crate::admission::WorkSlots::new(2, Default::default()),
        }
    }
    pub fn live_unit(&self) -> Arc<AtomicBool> {
        self.unit.clone()
    }
    fn publish(&self, settings: &DisplaySettings) {
        self.unit.store(
            settings.temperature_unit == DisplayTemperatureUnit::Fahrenheit,
            Ordering::Release,
        );
    }
    fn read(&self) -> anyhow::Result<DisplaySettings> {
        let path = self.root.join("display-settings.json");
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DisplaySettings::default())
            }
            Err(e) => return Err(e.into()),
        };
        ensure!(
            file.metadata()?.is_file(),
            "display settings must be a regular file"
        );
        let mut bytes = vec![];
        file.take(4097).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 4096, "display settings exceed size limit");
        let settings: DisplaySettings = serde_json::from_slice(&bytes)?;
        settings.validate()?;
        Ok(settings)
    }
    fn run(&self, proposed: Option<DisplaySettings>) -> anyhow::Result<DisplaySettings> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("display settings lock poisoned"))?;
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "display root must be a real directory"
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let Some(mut next) = proposed else {
            let current = self.read()?;
            self.publish(&current);
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
            .open(self.root.join(".display-settings.lock"))?;
        ensure!(
            lock.metadata()?.is_file(),
            "display lock must be a regular file"
        );
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .context("display settings are being edited by another process")?;
        let current = self.read()?;
        ensure!(
            next.revision == current.revision,
            "display settings changed; reload before editing"
        );
        next.revision = next
            .revision
            .checked_add(1)
            .context("display revision exhausted")?;
        next.validate()?;
        let mut tmp = tempfile::NamedTempFile::new_in(&self.root)?;
        tmp.write_all(&serde_json::to_vec_pretty(&next)?)?;
        tmp.as_file().sync_all()?;
        tmp.persist(self.root.join("display-settings.json"))?;
        // Rename is visible even if the subsequent directory durability barrier fails.
        self.publish(&next);
        let absolute = std::path::absolute(&self.root)?;
        for parent in absolute.ancestors() {
            File::open(parent)?.sync_all()?;
        }
        Ok(next)
    }
    pub async fn get(&self) -> lconnect_ipc::Result<DisplaySettings> {
        self.work(None).await
    }
    pub async fn save(&self, settings: DisplaySettings) -> lconnect_ipc::Result<DisplaySettings> {
        settings.validate()?;
        self.work(Some(settings)).await
    }
    async fn work(
        &self,
        settings: Option<DisplaySettings>,
    ) -> lconnect_ipc::Result<DisplaySettings> {
        let permit = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|e| lconnect_ipc::IpcError::DeviceBusy(e.to_string()))?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            this.run(settings)
                .map_err(|e| lconnect_ipc::IpcError::InvalidArgument(e.to_string()))
        })
        .await
        .map_err(|e| lconnect_ipc::IpcError::DeviceBusy(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fresh_english_legacy_russian_and_explicit_choices_survive_save_and_restart() {
        use lconnect_ipc::InterfaceLanguage;
        let root = tempfile::tempdir().unwrap();
        let store = DisplaySettingsStore::new(root.path().to_path_buf());
        assert_eq!(
            store.get().await.unwrap().interface_language,
            InterfaceLanguage::English
        );
        let path = root.path().join("display-settings.json");
        let legacy = br#"{"version":1,"revision":8,"temperature_unit":"fahrenheit"}"#;
        fs::write(&path, legacy).unwrap();
        let current = store.get().await.unwrap();
        assert_eq!(current.interface_language, InterfaceLanguage::Russian);
        assert_eq!(current.revision, 8);
        assert_eq!(
            fs::read(&path).unwrap(),
            legacy,
            "Reading legacy preferences must not migrate or write them"
        );
        let saved = store.save(current).await.unwrap();
        assert_eq!(saved.revision, 9);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap()
                ["interface_language"],
            "ru-RU"
        );
        let fresh = DisplaySettingsStore::new(root.path().to_path_buf());
        assert_eq!(fresh.get().await.unwrap(), saved);
        let en = fresh
            .save(DisplaySettings {
                interface_language: InterfaceLanguage::English,
                ..saved
            })
            .await
            .unwrap();
        assert_eq!(en.temperature_unit, DisplayTemperatureUnit::Fahrenheit);
        assert_eq!(
            DisplaySettingsStore::new(root.path().to_path_buf())
                .get()
                .await
                .unwrap(),
            en
        );
    }
    #[tokio::test]
    async fn durable_cas_restart_corruption_and_edited_files_never_reset_live_units_or_controls() {
        let root = tempfile::tempdir().unwrap();
        let store = DisplaySettingsStore::new(root.path().to_path_buf());
        assert_eq!(store.get().await.unwrap(), DisplaySettings::default());
        let mut next = store.get().await.unwrap();
        next.temperature_unit = DisplayTemperatureUnit::Fahrenheit;
        let saved = store.save(next.clone()).await.unwrap();
        assert_eq!(saved.revision, 1);
        assert!(store.live_unit().load(Ordering::Acquire));
        assert!(store.save(next).await.is_err());
        let fresh = DisplaySettingsStore::new(root.path().to_path_buf());
        assert_eq!(fresh.get().await.unwrap(), saved);
        assert!(fresh.live_unit().load(Ordering::Acquire));
        let path = root.path().join("display-settings.json");
        let future = br#"{"version":2,"revision":1,"temperature_unit":"fahrenheit"}"#;
        fs::write(&path, future).unwrap();
        assert!(store.get().await.is_err());
        assert!(store.save(saved).await.is_err());
        assert!(store.live_unit().load(Ordering::Acquire));
        assert_eq!(fs::read(path).unwrap(), future);
    }
    #[tokio::test]
    async fn links_fifos_and_foreign_process_lock_are_visible_errors_without_overwriting() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("display-settings.json");
        let target = root.path().join("missing");
        symlink(&target, &path).unwrap();
        let store = DisplaySettingsStore::new(root.path().to_path_buf());
        assert!(store.get().await.is_err());
        assert!(store.save(DisplaySettings::default()).await.is_err());
        assert!(fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
        fs::remove_file(&path).unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &path,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        assert!(store.get().await.is_err());
        fs::remove_file(&path).unwrap();
        let saved = store.save(DisplaySettings::default()).await.unwrap();
        let lock = File::open(root.path().join(".display-settings.lock")).unwrap();
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive).unwrap();
        assert!(DisplaySettingsStore::new(root.path().to_path_buf())
            .save(saved.clone())
            .await
            .is_err());
        assert_eq!(store.get().await.unwrap(), saved);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_save_keeps_owned_admission_and_publishes_after_the_waiter_disappears() {
        let root = tempfile::tempdir().unwrap();
        let store = DisplaySettingsStore::new(root.path().to_path_buf());
        let gate = store.gate.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let blocker = std::thread::spawn(move || {
            let _guard = gate.lock().unwrap();
            ready_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        });
        ready_rx.await.unwrap();
        let copy = store.clone();
        let caller = tokio::spawn(async move {
            copy.save(DisplaySettings {
                temperature_unit: DisplayTemperatureUnit::Fahrenheit,
                ..Default::default()
            })
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while store.jobs.available_permits() != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        let copy = store.clone();
        let reader = tokio::spawn(async move { copy.get().await });
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while store.jobs.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(matches!(
            store.get().await,
            Err(lconnect_ipc::IpcError::DeviceBusy(_))
        ));
        release_tx.send(()).unwrap();
        blocker.join().unwrap();
        reader.await.unwrap().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while store.jobs.available_permits() != 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            store.get().await.unwrap().temperature_unit,
            DisplayTemperatureUnit::Fahrenheit
        );
        assert!(store.live_unit().load(Ordering::Acquire));
    }
}
