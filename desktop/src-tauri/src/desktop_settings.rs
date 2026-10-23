//! Desktop preferences and an explicitly requested XDG autostart registration.
//! Desired settings are durable before integration, so failed file operations
//! remain visible/retryable. No daemon, session bus, tray or process spawning here.
use anyhow::{ensure, Context, Result};
use rustix::fs::{flock, FlockOperation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAX_BYTES: u64 = 64 * 1024;
const ENTRY: &str = "net.inkyquill.lconnect.desktop";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub close_to_tray: bool,
    pub start_minimized: bool,
    pub autostart: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            start_minimized: false,
            autostart: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    revision: u64,
    preferences: Preferences,
    /// Current observed owned entry and next intended entry. Persist these before
    /// rename/unlink, so retries after process death can recognize either state.
    owned_autostart_hashes: Vec<String>,
}
impl Default for Record {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            preferences: Preferences::default(),
            owned_autostart_hashes: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub present: bool,
    pub enabled: Option<bool>,
    pub managed: bool,
    pub problem: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Readback {
    pub revision: u64,
    pub preferences: Preferences,
    pub registration: Registration,
    pub integration_error: Option<String>,
}
#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    autostart: PathBuf,
    executable: PathBuf,
    config_dirs: Vec<PathBuf>,
    gate: Arc<Mutex<()>>,
}
impl Store {
    pub fn new(root: PathBuf, config_home: PathBuf, executable: PathBuf) -> Result<Self> {
        ensure!(
            root.is_absolute() && config_home.is_absolute() && executable.is_absolute(),
            "Desktop paths must be absolute"
        );
        Ok(Self {
            root,
            autostart: config_home.join("autostart").join(ENTRY),
            executable,
            config_dirs: vec![],
            gate: Arc::new(Mutex::new(())),
        })
    }
    /// Supply XDG_CONFIG_DIRS explicitly. Tests need no host configuration.
    pub fn with_config_dirs(mut self, dirs: Vec<PathBuf>) -> Result<Self> {
        ensure!(
            dirs.len() <= 32 && dirs.iter().all(|p| p.is_absolute()),
            "Invalid XDG config directories"
        );
        self.config_dirs = dirs;
        Ok(self)
    }
    fn system_entry_present(&self) -> Result<bool> {
        for dir in &self.config_dirs {
            match fs::symlink_metadata(dir.join("autostart").join(ENTRY)) {
                Ok(_) => return Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(false)
    }
    fn process_lock(&self) -> Result<File> {
        fs::create_dir_all(&self.root)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.root.join("desktop-settings.lock"))?;
        ensure!(
            file.metadata()?.is_file(),
            "Desktop settings lock must be a regular file"
        );
        flock(&file, FlockOperation::LockExclusive)?;
        Ok(file)
    }
    fn record(&self) -> Result<Record> {
        let Some(bytes) = read_regular(&self.root.join("desktop.json"))? else {
            return Ok(Record::default());
        };
        let record: Record = serde_json::from_slice(&bytes)
            .context("Invalid desktop preferences; existing file was preserved")?;
        ensure!(
            record.version == 1
                && record.owned_autostart_hashes.len() <= 2
                && record.owned_autostart_hashes.iter().all(|s| s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
            "Unsupported desktop settings version or ownership record"
        );
        Ok(record)
    }
    pub fn read(&self) -> Result<Readback> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Desktop settings unavailable"))?;
        let _file = self.process_lock()?;
        self.readback(self.record()?, None)
    }
    fn registration(&self, record: &Record) -> Registration {
        match read_regular(&self.autostart) {
            Ok(None) => {
                let system = self.system_entry_present();
                Registration {
                    present: !matches!(system, Ok(false)),
                    enabled: matches!(system, Ok(false)).then_some(false),
                    managed: false,
                    problem: match system {
                        Ok(false) => None,
                        Ok(true) => Some("A system autostart entry exists; its session policy is unknown. Saving disabled creates a user Hidden override.".into()),
                        Err(e) => Some(format!("Cannot inspect system autostart policy: {e}")),
                    },
                }
            }
            Ok(Some(bytes)) => {
                let managed = record.owned_autostart_hashes.contains(&digest(&bytes));
                Registration {
                    present: true,
                    enabled: managed.then(|| !bytes.split(|b| *b == b'\n').any(|line| line == b"Hidden=true")),
                    managed,
                    problem: (!managed).then(|| format!("Autostart file was edited or is not owned by this application; preserved: {}", self.autostart.display())),
                }
            }
            Err(e) => Registration {
                present: true,
                enabled: None,
                managed: false,
                problem: Some(format!("Cannot inspect autostart registration: {e}")),
            },
        }
    }
    fn readback(&self, record: Record, integration_error: Option<String>) -> Result<Readback> {
        Ok(Readback {
            revision: record.revision,
            preferences: record.preferences.clone(),
            registration: self.registration(&record),
            integration_error,
        })
    }
    pub fn apply(&self, expected_revision: u64, preferences: Preferences) -> Result<Readback> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Desktop settings unavailable"))?;
        let _file = self.process_lock()?;
        let mut record = self.record()?;
        ensure!(
            record.revision == expected_revision,
            "Desktop settings changed; refresh before saving"
        );
        let desired = if preferences.autostart {
            Some(entry(&self.executable)?)
        } else if self.system_entry_present().unwrap_or(true) {
            Some(b"# Managed by LL-Control desktop v1\n[Desktop Entry]\nType=Application\nName=LL-Control\nHidden=true\nExec=/usr/bin/true\nTerminal=false\n".to_vec())
        } else {
            None
        };
        let observed = read_regular(&self.autostart);
        let unrecognized = match &observed {
            Ok(None) => false,
            Ok(Some(bytes)) => !record.owned_autostart_hashes.contains(&digest(bytes)),
            Err(_) => true,
        };
        let mut allowed = observed
            .as_ref()
            .ok()
            .and_then(|v| v.as_ref())
            .map(|b| digest(b))
            .filter(|hash| record.owned_autostart_hashes.contains(hash))
            .into_iter()
            .collect::<Vec<_>>();
        // While disabling retain the previous allowlist across an unlink/sync
        // failure; never authorize an unrecognized file merely by observing it.
        if desired.is_none() || unrecognized {
            allowed = record.owned_autostart_hashes.clone();
        }
        if let Some(bytes) = desired.as_ref().filter(|_| !unrecognized) {
            let hash = digest(bytes);
            if !allowed.contains(&hash) {
                allowed.push(hash);
            }
        }
        if record.preferences != preferences || record.owned_autostart_hashes != allowed {
            record.revision = record
                .revision
                .checked_add(1)
                .context("Desktop revision exhausted")?;
            record.preferences = preferences;
            record.owned_autostart_hashes = allowed;
            atomic_write(
                &self.root.join("desktop.json"),
                &serde_json::to_vec_pretty(&record)?,
            )?;
        }
        let integrated = (|| -> Result<()> {
            let existing = read_regular(&self.autostart)?;
            if let Some(bytes) = existing.as_ref() {
                ensure!(
                    record.owned_autostart_hashes.contains(&digest(bytes)),
                    "Autostart file is not owned or was edited; it was preserved: {}",
                    self.autostart.display()
                );
            }
            match desired {
                Some(bytes) => {
                    // persist_noclobber for initial registration. Existing entry
                    // replacement is allowed only for hashes retained in intent.
                    if existing.is_some() {
                        atomic_write(&self.autostart, &bytes)?;
                    } else {
                        fs::create_dir_all(self.autostart.parent().unwrap())?;
                        let mut temporary =
                            tempfile::NamedTempFile::new_in(self.autostart.parent().unwrap())?;
                        temporary.write_all(&bytes)?;
                        temporary.as_file().sync_all()?;
                        temporary
                            .persist_noclobber(&self.autostart)
                            .map_err(|e| e.error)?;
                        sync_parent(&self.autostart)?;
                    }
                }
                None => {
                    if existing.is_some() {
                        fs::remove_file(&self.autostart)?;
                        sync_parent(&self.autostart)?;
                    }
                }
            }
            Ok(())
        })();
        self.readback(record, integrated.err().map(|e| e.to_string()))
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read_regular(path: &Path) -> Result<Option<Vec<u8>>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() <= MAX_BYTES,
        "Not a bounded regular settings file: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "Settings file exceeds budget"
    );
    Ok(Some(bytes))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("Missing settings directory")?)?;
    // Reject redirected/special final files before atomic replacement.
    read_regular(path)?;
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)?;
    sync_parent(path)
}
fn sync_parent(path: &Path) -> Result<()> {
    File::open(path.parent().context("Missing parent")?)?.sync_all()?;
    Ok(())
}
fn entry(executable: &Path) -> Result<Vec<u8>> {
    let path = executable
        .to_str()
        .context("Autostart executable path must be UTF-8")?;
    ensure!(
        executable.is_absolute() && !path.contains('=') && !path.chars().any(char::is_control),
        "Executable path cannot be represented by XDG Exec"
    );
    let metadata = fs::metadata(executable).context("Autostart executable is unavailable")?;
    ensure!(
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        "Autostart target must be an executable regular file"
    );
    ensure!(
        fs::metadata("/usr/bin/env")?.is_file(),
        "XDG launch helper is unavailable"
    );
    let mut quoted = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    let encoded = quoted.replace('\\', "\\\\");
    Ok(format!("# Managed by LL-Control desktop v1\n[Desktop Entry]\nType=Application\nName=LL-Control\nExec=/usr/bin/env -- {encoded} --autostart\nTerminal=false\nX-GNOME-Autostart-enabled=true\n").into_bytes())
}

#[cfg(test)]
mod tests;
