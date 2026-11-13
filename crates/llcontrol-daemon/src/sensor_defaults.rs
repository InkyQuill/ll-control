//! Durable global sensor choices. Run filesystem methods on a blocking worker.
use anyhow::{ensure, Context};
use llcontrol_ipc::SensorDefaults;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const MAX_BYTES: u64 = 64 * 1024;

/// A daemon must share clones of one store; the lock survives RPC cancellation.
/// This layer checks schema/revision. The caller must validate provider membership
/// before saving newly chosen bindings; loading never requires connected hardware.
#[derive(Clone)]
pub struct SensorDefaultsStore {
    root: PathBuf,
    lock: Arc<Mutex<()>>,
}
impl SensorDefaultsStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: Arc::default(),
        }
    }

    fn read(&self) -> anyhow::Result<SensorDefaults> {
        let path = self.root.join("sensor-defaults.json");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SensorDefaults::default())
            }
            Err(e) => return Err(e.into()),
        };
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "sensor defaults must be a regular file"
        );
        ensure!(
            metadata.len() <= MAX_BYTES,
            "sensor defaults exceed size limit"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "sensor defaults exceed size limit"
        );
        let defaults: SensorDefaults = serde_json::from_slice(&bytes)?;
        defaults.validate()?;
        Ok(defaults)
    }

    pub fn load(&self) -> anyhow::Result<SensorDefaults> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("sensor defaults lock poisoned"))?;
        self.read()
    }

    /// Replace a previously read revision. A lost reply requires readback, not
    /// automatic replay: rename may have succeeded even if directory fsync failed.
    pub fn save(&self, mut proposed: SensorDefaults) -> anyhow::Result<SensorDefaults> {
        proposed.validate()?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("sensor defaults lock poisoned"))?;
        let current = self.read()?;
        ensure!(
            proposed.revision == current.revision,
            "sensor defaults changed; reload before editing"
        );
        proposed.revision = current
            .revision
            .checked_add(1)
            .context("sensor defaults revision exhausted")?;
        let bytes = serde_json::to_vec_pretty(&proposed)?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "sensor defaults exceed size limit"
        );
        create_durable_directory(&self.root)?;
        let mut file = tempfile::NamedTempFile::new_in(&self.root)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(self.root.join("sensor-defaults.json"))?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(proposed)
    }
}

// Also commit parent links on retries: a previous attempt may have created the
// directory chain but failed before syncing it. Existence does not prove durability.
fn create_durable_directory(root: &Path) -> anyhow::Result<()> {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()?.join(root)
    };
    fs::create_dir_all(&absolute)?;
    for directory in absolute.ancestors() {
        fs::File::open(directory)?.sync_all()?;
    }
    Ok(())
}
