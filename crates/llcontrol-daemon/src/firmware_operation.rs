//! Durable update intent and fail-closed ordinary replay quarantine.
//!
//! This is not a redo log: never repeat firmware I/O while recovering it.
//! All persisted stages remain quarantined; no verification/unlock API exists
//! until a coordinator can prove fresh target-bound reconnect and version.
use crate::{
    actor::firmware_update::{CoolingHold, Outcome, Phase, Progress},
    persistence::{settings_key, DeviceSettings},
    state_transaction::{atomic_write, check_directory, read_optional},
};
use anyhow::{ensure, Context};
use llcontrol_hid::DeviceInfo;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};
const FILE: &str = "firmware-operation.json";
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const IMAGE_SHA256: &str = crate::actor::firmware_update::IMAGE_HASH;

/// Persisted inside device TOML. Previous deny_unknown_fields readers reject it.
/// There is deliberately no clearing API before qualified reconnect resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsGuard {
    pub version: u32,
    pub operation_id: String,
    pub image_sha256: String,
}
impl SettingsGuard {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.version == 1
                && self.operation_id.len() == 32
                && self.operation_id.bytes().all(|b| b.is_ascii_hexdigit())
                && self.image_sha256 == IMAGE_SHA256,
            "invalid firmware settings guard"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub path: String,
    pub settings_key: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    Armed,
    EffectsPossible,
    NotStarted,
    Unknown,
    AwaitingReconnect,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub version: u32,
    pub operation_id: String,
    pub target: Target,
    pub image_sha256: String,
    pub cooling_hold: CoolingHold,
    pub restore_settings: DeviceSettings,
    pub stage: Stage,
    /// No progress after a crash is proof of zero writes. EffectsPossible may
    /// cover the entire transaction, including activation, without another save.
    pub progress: Option<Progress>,
    pub diagnostic: Option<String>,
}
impl Record {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.version == 1, "unsupported firmware journal version");
        ensure!(
            self.operation_id.len() == 32
                && self.operation_id.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid firmware operation id"
        );
        ensure!(
            self.target.vid == 0x0416 && self.target.pid == 0x7395,
            "unsupported firmware target"
        );
        ensure!(
            !self.target.path.is_empty()
                && self.target.path.len() <= 4096
                && !self.target.path.chars().any(char::is_control),
            "invalid firmware target path"
        );
        ensure!(
            self.target.serial.as_ref().is_none_or(|s| !s.is_empty()
                && s.len() <= 256
                && !s.chars().any(char::is_control)),
            "invalid firmware target serial"
        );
        crate::state_transaction::validate_key(&self.target.settings_key)?;
        let identity = DeviceInfo::new(
            llcontrol_hid::DeviceFamilyId::new("galahad2-lcdv2"),
            self.target.vid,
            self.target.pid,
            self.target.serial.clone(),
            self.target.path.clone().into(),
        );
        ensure!(
            settings_key(&identity) == self.target.settings_key,
            "firmware target/settings binding mismatch"
        );
        ensure!(
            self.image_sha256 == IMAGE_SHA256,
            "unresearched firmware image digest"
        );
        crate::persistence::validate_schema(&self.restore_settings)?;
        ensure!(
            self.diagnostic.as_ref().is_none_or(|s| s.len() <= 8192),
            "firmware diagnostic exceeds bound"
        );
        if let Some(p) = &self.progress {
            ensure!(
                p.total_chunks == 0 || p.total_chunks == 619,
                "invalid firmware chunk budget"
            );
            if p.total_chunks == 0 {
                ensure!(
                    self.stage == Stage::NotStarted && !p.query_written && p.attempted.is_none(),
                    "empty progress outside unadmitted operation"
                );
            } else {
                ensure!(
                    p.cooling_hold == Some(self.cooling_hold),
                    "missing hold plan in admitted progress"
                );
            }
            ensure!(
                p.chunk_echoes <= p.chunk_writes && p.chunk_writes <= 619,
                "invalid firmware progress counters"
            );
            ensure!(
                p.attempted.is_some()
                    || (!p.pump_hold_written
                        && !p.fan_hold_written
                        && p.chunk_writes == 0
                        && !p.activation_written),
                "effects without attempted step"
            );
            if let Some(step) = p.attempted {
                ensure!(p.query_written, "attempt without preflight query");
                match step {
                    crate::actor::firmware_update::Step::Chunk(pn) => {
                        ensure!(pn < 619, "invalid attempted chunk")
                    }
                    crate::actor::firmware_update::Step::FanHold => ensure!(
                        p.pump_hold_written
                            && self.cooling_hold == CoolingHold::PumpAndControllerFan,
                        "invalid attempted fan hold"
                    ),
                    crate::actor::firmware_update::Step::Activate => ensure!(
                        p.chunk_echoes == 619,
                        "activation attempt before chunk echoes"
                    ),
                    _ => (),
                }
            }
            ensure!(
                !p.pump_hold_written || p.query_written,
                "hold without preflight query"
            );
            ensure!(
                !p.fan_hold_written
                    || (p.pump_hold_written
                        && self.cooling_hold == CoolingHold::PumpAndControllerFan),
                "invalid fan hold progress"
            );
            ensure!(
                p.chunk_writes == 0 || p.pump_hold_written,
                "chunks without cooling hold"
            );
            ensure!(
                !p.activation_written || p.chunk_echoes == 619,
                "activation before all chunk echoes"
            );
            ensure!(
                p.cooling_hold.is_none_or(|hold| hold == self.cooling_hold),
                "firmware hold binding mismatch"
            );
            match self.stage {
                Stage::NotStarted => ensure!(
                    p.phase == Phase::NotStarted
                        && p.attempted.is_none()
                        && !p.pump_hold_written
                        && p.chunk_writes == 0
                        && !p.activation_written,
                    "invalid not-started result"
                ),
                Stage::Unknown => ensure!(p.phase == Phase::Unknown, "invalid unknown result"),
                Stage::AwaitingReconnect => ensure!(
                    p.phase == Phase::AwaitingReconnect && p.activation_written,
                    "invalid reconnect result"
                ),
                Stage::Armed | Stage::EffectsPossible => {
                    anyhow::bail!("unexpected terminal progress in nonterminal journal")
                }
            }
        } else {
            ensure!(
                matches!(self.stage, Stage::Armed | Stage::EffectsPossible),
                "terminal journal lacks progress"
            );
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Journal {
    root: PathBuf,
    store: crate::persistence::PersistenceStore,
}
impl std::fmt::Debug for Journal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Journal").field("root", &self.root).finish()
    }
}
impl Journal {
    pub fn new(root: PathBuf) -> Self {
        Self::for_store(
            root.clone(),
            crate::persistence::PersistenceStore::new_with_root(root),
        )
    }
    pub(crate) fn for_store(root: PathBuf, store: crate::persistence::PersistenceStore) -> Self {
        Self { root, store }
    }
    fn path(&self) -> PathBuf {
        self.root.join(FILE)
    }
    pub fn load(&self) -> anyhow::Result<Option<Record>> {
        Ok(self.read_snapshot()?.map(|(record, _)| record))
    }
    pub(crate) fn read_snapshot(&self) -> anyhow::Result<Option<(Record, String)>> {
        let Some(text) = read_optional(&self.path(), MAX_BYTES)? else {
            return Ok(None);
        };
        let record: Record =
            serde_json::from_str(&text).context("invalid firmware journal; original preserved")?;
        record.validate()?;
        Ok(Some((record, text)))
    }
    pub(crate) fn ensure_probe_guard(&self, record: &Record) -> anyhow::Result<()> {
        let guard = self
            .store
            .firmware_guard_readonly(&record.target.settings_key)?;
        ensure!(
            guard.operation_id == record.operation_id && guard.image_sha256 == record.image_sha256,
            "firmware settings guard binding mismatch"
        );
        Ok(())
    }
    /// Saved recovery state only: does not retry, resolve, probe, clear or replay.
    pub fn recovery_status(&self) -> llcontrol_ipc::FirmwareRecoveryStatus {
        use llcontrol_ipc::{
            FirmwareRecoveryProgress, FirmwareRecoveryState as State, FirmwareRecoveryStatus,
            FirmwareSavedOperation, FirmwareSavedStage,
        };
        let (record, guards) = match self.store.firmware_recovery_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return FirmwareRecoveryStatus {
                    state: State::Unreadable,
                    operation: None,
                    guards: Vec::new(),
                    diagnostic: Some(
                        format!(
                        "saved firmware inspection unavailable; original state preserved: {error}"
                    )
                        .chars()
                        .take(2048)
                        .collect(),
                    ),
                }
            }
        };
        let state = match (&record, guards.as_slice()) {
            (None, []) => State::NoSavedOperation,
            (None, _) => State::GuardOnly,
            (Some(record), []) if matches!(record.stage, Stage::Armed | Stage::NotStarted) => {
                State::JournalOnly
            }
            (Some(record), [guard])
                if guard.settings_key == record.target.settings_key
                    && guard.operation_id == record.operation_id
                    && guard.image_sha256 == record.image_sha256 =>
            {
                State::Guarded
            }
            _ => State::Inconsistent,
        };
        let diagnostic = match state {
            State::NoSavedOperation => None,
            State::JournalOnly => Some("Saved operation unresolved; absence of a saved effects marker is not proof of no writes."),
            State::Guarded => Some("Saved firmware operation quarantined; fresh target verification and qualified resolution required."),
            State::GuardOnly => Some("Settings guard exists without a firmware journal; outcome unknown. Preserve state for recovery."),
            State::Inconsistent => Some("Firmware journal/settings guards disagree; outcome unknown. Preserve state for recovery."),
            State::Unreadable => unreachable!(),
        }.map(str::to_owned);
        FirmwareRecoveryStatus {
            state,
            guards,
            diagnostic,
            operation: record.map(|r| FirmwareSavedOperation {
                operation_id: r.operation_id,
                image_sha256: r.image_sha256,
                stage: match r.stage {
                    Stage::Armed => FirmwareSavedStage::Armed,
                    Stage::EffectsPossible => FirmwareSavedStage::EffectsPossible,
                    Stage::NotStarted => FirmwareSavedStage::NotStarted,
                    Stage::Unknown => FirmwareSavedStage::Unknown,
                    Stage::AwaitingReconnect => FirmwareSavedStage::AwaitingReconnect,
                },
                vid: r.target.vid,
                pid: r.target.pid,
                serial: r.target.serial,
                previous_hid_path: r.target.path,
                settings_key: r.target.settings_key,
                diagnostic: r.diagnostic,
                progress: r.progress.map(|p| FirmwareRecoveryProgress {
                    total_chunks: p.total_chunks,
                    chunk_writes: p.chunk_writes,
                    chunk_echoes: p.chunk_echoes,
                    pump_hold_written: p.pump_hold_written,
                    fan_hold_written: p.fan_hold_written,
                    activation_written: p.activation_written,
                }),
            }),
        }
    }
    /// Ordinary manager attachment must never start keepers or replay while
    /// an operation for this SKU is unresolved. A different serial/path cannot
    /// count as successful reconnect. Probe-only binding is a later coordinator.
    pub fn ensure_replay_allowed(&self, info: &DeviceInfo) -> anyhow::Result<()> {
        if info.vid == 0x0416 && info.pid == 0x7395 {
            self.store.ensure_no_firmware_guard()?;
        }
        if let Some(record) = self.load()? {
            ensure!(
                info.vid != record.target.vid || info.pid != record.target.pid,
                "firmware operation {} unresolved; ordinary attachment/replay withheld",
                record.operation_id
            );
        }
        Ok(())
    }
    /// Atomically publish and fsync before the future coordinator admits any
    /// update side effect. Existing/corrupt operations are never overwritten.
    /// Publication does not itself stop a live actor or authorize transport.
    pub fn arm(
        &self,
        info: &DeviceInfo,
        hold: CoolingHold,
        restore_settings: DeviceSettings,
    ) -> anyhow::Result<ArmedOperation> {
        let record = Record {
            version: 1,
            operation_id: format!("{:032x}", rand::random::<u128>()),
            target: Target {
                vid: info.vid,
                pid: info.pid,
                serial: info.serial.clone().filter(|s| !s.is_empty()),
                path: info
                    .path
                    .to_str()
                    .context("non-UTF8 firmware target path")?
                    .into(),
                settings_key: settings_key(info),
            },
            image_sha256: IMAGE_SHA256.into(),
            cooling_hold: hold,
            restore_settings,
            stage: Stage::Armed,
            progress: None,
            diagnostic: None,
        };
        record.validate()?;
        let bytes = encode(&record)?;
        check_directory(&self.root)?;
        match fs::create_dir(&self.root) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        check_directory(&self.root)?;
        fs::File::open(
            self.root
                .parent()
                .context("missing firmware journal parent")?,
        )?
        .sync_all()?;
        let mut temp = tempfile::NamedTempFile::new_in(&self.root)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        let _published = temp
            .persist_noclobber(self.path())
            .context("firmware operation already exists or cannot be published")?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(ArmedOperation {
            journal: self.clone(),
            record,
            expected: bytes,
            require_fresh_snapshot: false,
        })
    }
}
fn encode(record: &Record) -> anyhow::Result<Vec<u8>> {
    record.validate()?;
    let data = serde_json::to_vec_pretty(record)?;
    ensure!(
        data.len() as u64 <= MAX_BYTES,
        "firmware journal exceeds size bound"
    );
    Ok(data)
}
/// Single owner, deliberately not Clone or recoverable as a writable lease.
/// Dropping it leaves the durable quarantine in place; recovery never retries.
pub struct ArmedOperation {
    journal: Journal,
    record: Record,
    expected: Vec<u8>,
    require_fresh_snapshot: bool,
}
impl std::fmt::Debug for ArmedOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArmedOperation")
            .field("operation_id", &self.record.operation_id)
            .field("stage", &self.record.stage)
            .finish()
    }
}
impl ArmedOperation {
    /// Coordinator-only opt-in: low-level protocol fixtures may supply a
    /// synthetic restore record; product execution must bind the real snapshot.
    pub(crate) fn require_fresh_snapshot(&mut self) -> anyhow::Result<()> {
        let current = self.journal.store.read(&self.record.target.settings_key)?;
        ensure!(
            serde_json::to_value(&current)? == serde_json::to_value(&self.record.restore_settings)?,
            "saved intent changed since firmware snapshot"
        );
        self.require_fresh_snapshot = true;
        Ok(())
    }
    pub fn record(&self) -> &Record {
        &self.record
    }
    pub(crate) fn check_binding(&self, info: &DeviceInfo, hold: CoolingHold) -> anyhow::Result<()> {
        ensure!(
            self.record.stage == Stage::Armed,
            "firmware operation already started"
        );
        let t = &self.record.target;
        ensure!(
            t.vid == info.vid
                && t.pid == info.pid
                && t.serial.as_deref() == info.serial.as_deref().filter(|s| !s.is_empty())
                && Some(t.path.as_str()) == info.path.to_str()
                && t.settings_key == settings_key(info)
                && self.record.cooling_hold == hold,
            "firmware operation binding mismatch"
        );
        self.check_current()
    }
    pub(crate) fn check_scope(&self, expected: &Journal) -> anyhow::Result<()> {
        ensure!(
            self.journal.root == expected.root,
            "firmware journal belongs to another store"
        );
        Ok(())
    }
    fn check_current(&self) -> anyhow::Result<()> {
        let current = read_optional(&self.journal.path(), MAX_BYTES)?
            .context("firmware journal disappeared")?;
        ensure!(
            current.as_bytes() == self.expected,
            "firmware journal changed externally; file preserved"
        );
        Ok(())
    }
    /// Persist uncertainty BEFORE calling PWM/flash. This marker covers any
    /// number of writes until finish; it is not an acknowledged-byte counter.
    pub fn effects_possible(&mut self) -> anyhow::Result<()> {
        ensure!(
            self.record.stage == Stage::Armed,
            "operation cannot be started twice"
        );
        self.check_current()?;
        self.journal.store.anchor_firmware(
            &self.record.target.settings_key,
            SettingsGuard {
                version: 1,
                operation_id: self.record.operation_id.clone(),
                image_sha256: self.record.image_sha256.clone(),
            },
            self.require_fresh_snapshot
                .then_some(&self.record.restore_settings),
        )?;
        let mut next = self.record.clone();
        next.stage = Stage::EffectsPossible;
        self.replace(next)
    }
    pub fn finish(mut self, outcome: &Outcome) -> anyhow::Result<()> {
        let mut next = self.record.clone();
        next.stage = match outcome.progress.phase {
            Phase::NotStarted => Stage::NotStarted,
            Phase::Unknown => Stage::Unknown,
            Phase::AwaitingReconnect => Stage::AwaitingReconnect,
            _ => anyhow::bail!("nonterminal firmware outcome"),
        };
        ensure!(
            self.record.stage == Stage::EffectsPossible || next.stage == Stage::NotStarted,
            "missing durable effects marker"
        );
        ensure!(
            next.stage != Stage::NotStarted || self.record.stage == Stage::Armed,
            "cannot downgrade possible effects to not-started"
        );
        next.progress = Some(outcome.progress.clone());
        next.diagnostic = outcome
            .diagnostic
            .as_ref()
            .map(|s| s.chars().take(2048).collect());
        self.replace(next)
    }
    fn replace(&mut self, next: Record) -> anyhow::Result<()> {
        let bytes = encode(&next)?;
        self.check_current()?;
        atomic_write(&self.journal.path(), &bytes)?;
        self.record = next;
        self.expected = bytes;
        Ok(())
    }
}
