//! Probe-only reconnect evidence. Never spawns actors, replays settings, retries
//! flash or clears quarantine. The coordinator must first close/join the old HID
//! owner and exclude all other writers. This helper alone cannot authorize Apply.
use crate::firmware_operation::{Journal, Record, Stage};
use anyhow::{ensure, Context};
use llcontrol_hid::{DeviceInfo, HidBackend};
use llcontrol_protocol::galahad2_lcdv2::{firmware, packets::ACommandPacket};

/// One observation of the actual backend enumeration, not a caller-supplied list.
#[derive(Debug)]
pub enum Observation {
    WaitingForDetach,
    WaitingForReconnect,
    Evidence(Box<ProbeEvidence>),
}
/// Read-only evidence for a future qualified resolution decision. A matching
/// version is not a hash/readback of the installed image or revision validation.
#[derive(Debug)]
pub struct ProbeEvidence {
    operation_id: String,
    target: DeviceInfo,
    identity: firmware::FirmwareVersion,
}
impl ProbeEvidence {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn target(&self) -> &DeviceInfo {
        &self.target
    }
    pub fn identity(&self) -> &firmware::FirmwareVersion {
        &self.identity
    }
}

/// Process-local, non-recoverable observation of absence followed by reappearance.
/// After daemon restart, seeing an attached device is not proof of fresh reset.
/// Serial-less targets cannot be automatically bound and remain quarantined.
#[derive(Debug)]
pub struct ReconnectProbe {
    journal: Journal,
    record: Record,
    snapshot: String,
    absent: bool,
    finished: bool,
}
impl ReconnectProbe {
    pub fn new(journal: Journal) -> anyhow::Result<Self> {
        let (record, snapshot) = journal
            .read_snapshot()?
            .context("firmware journal missing")?;
        ensure!(record.stage == Stage::AwaitingReconnect,
            "only acknowledged activation can produce reconnect evidence; uncertain outcome needs explicit recovery");
        ensure!(
            record.target.serial.is_some(),
            "serial-less firmware target cannot be automatically verified"
        );
        journal.ensure_probe_guard(&record)?;
        Ok(Self {
            journal,
            record,
            snapshot,
            absent: false,
            finished: false,
        })
    }
    pub(crate) fn bind_closed_target(
        &self,
        original: &DeviceInfo,
        operation_id: &str,
    ) -> anyhow::Result<()> {
        let target = &self.record.target;
        ensure!(
            self.record.operation_id == operation_id
                && target.vid == original.vid
                && target.pid == original.pid
                && target.serial == original.serial
                && target.path == original.path.to_string_lossy()
                && target.settings_key == crate::persistence::settings_key(original),
            "reconnect journal does not match joined original owner/operation"
        );
        Ok(())
    }
    fn check_current(&self) -> anyhow::Result<()> {
        let (_, current) = self
            .journal
            .read_snapshot()?
            .context("firmware journal disappeared")?;
        ensure!(
            current == self.snapshot,
            "firmware journal changed during reconnect probe"
        );
        self.journal.ensure_probe_guard(&self.record)
    }
    fn candidates(&self, backend: &dyn HidBackend) -> anyhow::Result<Vec<DeviceInfo>> {
        let target = &self.record.target;
        Ok(backend
            .enumerate()?
            .into_iter()
            .filter(|d| d.vid == target.vid && d.pid == target.pid && d.serial == target.serial)
            .collect())
    }
    /// Synchronous bounded protocol exchange: call from a joined blocking task.
    /// Backend open/write are synchronous and cannot be force-interrupted. Failed
    /// attempts do not clear markers or turn Unknown into success. No flash retry.
    pub fn observe(&mut self, backend: &dyn HidBackend) -> anyhow::Result<Observation> {
        ensure!(!self.finished, "reconnect evidence already delivered");
        self.check_current()?;
        let mut candidates = self.candidates(backend)?;
        ensure!(
            candidates.len() <= 1,
            "ambiguous duplicate firmware target serial"
        );
        let Some(target) = candidates.pop() else {
            self.absent = true;
            return Ok(Observation::WaitingForReconnect);
        };
        if !self.absent {
            return Ok(Observation::WaitingForDetach);
        }
        // No ordinary tree or replay is created, including on any failure path.
        let handle = backend.open(&target)?;
        let mut quiet = false;
        for _ in 0..10 {
            let mut bytes = [0; 512];
            let count = handle.read_timeout(&mut bytes, 0)?;
            ensure!(count <= bytes.len(), "invalid reconnect drain extent");
            if count == 0 {
                quiet = true;
                break;
            }
        }
        ensure!(quiet, "reconnect input never became quiet");
        self.check_current()?;
        handle.write(&firmware::build_request())?;
        let responses = super::hid_io::collect_firmware(&*handle)?;
        let packet = ACommandPacket::parse_response(&responses.version, 134)?;
        let raw = std::str::from_utf8(packet.data())?;
        ensure!(
            raw.trim_matches('\0').trim() == super::firmware_update::IDENTITY,
            "reconnected firmware differs from researched target identity"
        );
        let identity = firmware::FirmwareVersion::from_optional_responses(
            &responses.version,
            responses.datetime.as_ref().map(|b| b.as_slice()),
        )?;
        drop(handle);
        self.check_current()?;
        let after = self.candidates(backend)?;
        ensure!(
            after.len() == 1 && after[0] == target,
            "firmware target changed or disappeared during probe"
        );
        self.finished = true;
        Ok(Observation::Evidence(Box::new(ProbeEvidence {
            operation_id: self.record.operation_id.clone(),
            target,
            identity,
        })))
    }
}
