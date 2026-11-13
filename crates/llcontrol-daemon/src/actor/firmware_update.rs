//! Actor-owned update transport primitive, not a user-callable updater.
//!
//! The HID actor fences the attachment permanently on admission. A future job
//! coordinator must quiesce/join keepers and own reconnect/version verification
//! before exposing this through IPC. No retry, automatic reapply or success claim
//! follows an activation reply. Only the pinned researched image is accepted.
use super::Shutdown;
use llcontrol_hid::OpenHandle;
use llcontrol_protocol::galahad2_lcdv2::{
    bpacket::BCommandPacket,
    fan_pwm, firmware,
    firmware_update::{decode_activation, ChunkExpectation},
    packets::ACommandPacket,
    pump_pwm::{self, PumpPwmSource},
    pwm::Pwm,
};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tokio::sync::watch;

const IMAGE_SIZE: usize = 626_212;
pub(crate) const IMAGE_HASH: &str =
    "68e2d11c409e75b1e25bf9c2e84b891bcdc18ac735bafa67a0ee25a218b2d9d3";
pub(super) const IDENTITY: &str = "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6";

/// Explicit update hold selection. No fan ownership is inferred from zero RPM.
/// Both choices request pump MCU/100%; optional controller fan duty is100%.
/// These write-only requests do not establish physical duty/RPM or boot behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CoolingHold {
    PumpOnly,
    PumpAndControllerFan,
}
pub(crate) struct TransferOptions {
    pub target: llcontrol_hid::DeviceInfo,
    pub operation: crate::firmware_operation::ArmedOperation,
    pub journal: crate::firmware_operation::Journal,
    pub cooling_hold: CoolingHold,
}

/// Exact researched candidate bytes, owned until the blocking transaction joins.
pub struct ValidatedImage(Vec<u8>);
impl std::fmt::Debug for ValidatedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidatedImage")
            .field("size", &self.0.len())
            .finish()
    }
}
impl ValidatedImage {
    /// Recognize pinned bytes, not arbitrary formats or hardware compatibility.
    /// Future IPC file staging must separately bound its read before calling.
    pub fn recognize(bytes: Vec<u8>) -> Result<Self, &'static str> {
        if bytes.len() != IMAGE_SIZE {
            return Err("unknown firmware image size");
        }
        if format!("{:x}", Sha256::digest(&bytes)) != IMAGE_HASH {
            return Err("unknown firmware image SHA-256");
        }
        Ok(Self(bytes))
    }
}

/// Typed progress; descriptive state, never an instruction to retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Phase {
    Preparing,
    HoldingCooling,
    Uploading,
    Activating,
    NotStarted,
    Unknown,
    AwaitingReconnect,
}
/// Last *attempted* side-effecting report, including potentially partial writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Step {
    PumpHold,
    FanHold,
    Chunk(u32),
    Activate,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub phase: Phase,
    pub total_chunks: u32,
    pub cooling_hold: Option<CoolingHold>,
    pub pump_hold_written: bool,
    pub fan_hold_written: bool,
    pub query_written: bool,
    pub chunk_writes: u32,
    pub chunk_echoes: u32,
    pub activation_written: bool,
    pub attempted: Option<Step>,
}
impl Default for Progress {
    fn default() -> Self {
        Self {
            phase: Phase::Preparing,
            total_chunks: 0,
            cooling_hold: None,
            pump_hold_written: false,
            fan_hold_written: false,
            query_written: false,
            chunk_writes: 0,
            chunk_echoes: 0,
            activation_written: false,
            attempted: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub progress: Progress,
    /// Raw transport/codec diagnostic; future UI must use typed phase labels.
    pub diagnostic: Option<String>,
}
impl Outcome {
    pub(crate) fn stopped(mut progress: Progress, reason: impl ToString) -> Self {
        progress.phase = if progress.attempted.is_some() {
            Phase::Unknown
        } else {
            Phase::NotStarted
        };
        Self {
            progress,
            diagnostic: Some(reason.to_string()),
        }
    }
    pub(crate) fn writes(&self) -> u32 {
        u32::from(self.progress.query_written)
            + u32::from(self.progress.pump_hold_written)
            + u32::from(self.progress.fan_hold_written)
            + self.progress.chunk_writes
            + u32::from(self.progress.activation_written)
    }
}

#[derive(Clone, Copy)]
struct Timing {
    settle: Duration,
    inter_chunk: Duration,
    before_activate: Duration,
    reply: Duration,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            settle: Duration::from_secs(3),
            inter_chunk: Duration::from_millis(10),
            before_activate: Duration::from_secs(2),
            reply: Duration::from_secs(10),
        }
    }
}
fn stopping(cancel: &Shutdown, shutdown: &Shutdown) -> bool {
    *cancel.borrow()
        || cancel.has_changed().is_err()
        || *shutdown.borrow()
        || shutdown.has_changed().is_err()
}
fn wait(duration: Duration, cancel: &Shutdown, shutdown: &Shutdown) -> Result<(), String> {
    let end = Instant::now() + duration;
    loop {
        if stopping(cancel, shutdown) {
            return Err("update cancelled or shutting down".into());
        }
        let remaining = end.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}
fn drain(handle: &dyn OpenHandle) -> Result<(), String> {
    for _ in 0..10 {
        let mut bytes = [0; 512];
        match handle
            .read_timeout(&mut bytes, 0)
            .map_err(|e| e.to_string())?
        {
            0 => return Ok(()),
            1..=512 => (),
            _ => return Err("invalid input extent during preflight".into()),
        }
    }
    Err("input queue did not become quiet before update".into())
}
fn reply(
    handle: &dyn OpenHandle,
    timeout: Duration,
    cancel: &Shutdown,
    shutdown: &Shutdown,
) -> Result<Vec<u8>, String> {
    let end = Instant::now() + timeout;
    loop {
        wait(Duration::ZERO, cancel, shutdown)?;
        let remaining = end.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("update reply deadline elapsed".into());
        }
        let ms = i32::try_from(remaining.as_millis().clamp(1, 100)).unwrap_or(100);
        let mut bytes = [0; 512];
        let count = handle
            .read_timeout(&mut bytes, ms)
            .map_err(|e| e.to_string())?;
        let actual = bytes.get(..count).ok_or("invalid update reply extent")?;
        if count == 0 {
            wait(Duration::from_millis(1), cancel, shutdown)?;
            continue;
        }
        // A late PN1 from the one preflight query is not an upload ACK. No
        // new query/write is issued; it shares the original bounded deadline.
        if actual.len() <= 64
            && actual.get(3..5) == Some(&[0, 1])
            && ACommandPacket::parse_response(actual, 134).is_ok()
        {
            continue;
        }
        return Ok(actual.to_vec());
    }
}

pub(crate) fn run(
    handle: &dyn OpenHandle,
    image: ValidatedImage,
    options: TransferOptions,
    cancel: Shutdown,
    shutdown: Shutdown,
    progress: watch::Sender<Progress>,
) -> Outcome {
    run_timed(
        handle,
        image,
        options,
        cancel,
        shutdown,
        progress,
        Timing::default(),
    )
}
fn run_timed(
    handle: &dyn OpenHandle,
    image: ValidatedImage,
    mut options: TransferOptions,
    cancel: Shutdown,
    shutdown: Shutdown,
    progress: watch::Sender<Progress>,
    timing: Timing,
) -> Outcome {
    let mut state = Progress {
        total_chunks: u32::try_from(image.0.len().div_ceil(1013))
            .expect("recognized image is bounded to 626212 bytes"),
        cooling_hold: Some(options.cooling_hold),
        ..Progress::default()
    };
    progress.send_replace(state.clone());
    let operation = (|| -> Result<(), String> {
        if options.target.vid != 0x0416 || options.target.pid != 0x7395 {
            return Err("update target is not 0416:7395".into());
        }
        options
            .operation
            .check_scope(&options.journal)
            .map_err(|e| e.to_string())?;
        options
            .operation
            .check_binding(&options.target, options.cooling_hold)
            .map_err(|e| e.to_string())?;
        wait(Duration::ZERO, &cancel, &shutdown)?;
        drain(handle)?;
        handle
            .write(&firmware::build_request())
            .map_err(|e| e.to_string())?;
        state.query_written = true;
        progress.send_replace(state.clone());
        let version = super::hid_io::collect_firmware(handle).map_err(|e| e.to_string())?;
        let packet =
            ACommandPacket::parse_response(&version.version, 134).map_err(|e| e.to_string())?;
        let token = std::str::from_utf8(packet.data()).map_err(|e| e.to_string())?;
        if token.trim_matches('\0').trim() != IDENTITY {
            return Err("current firmware is not the researched LCDV2 1.6 identity".into());
        }
        wait(Duration::ZERO, &cancel, &shutdown)?;
        // Covers every later side effect even if no progress/outcome is saved.
        options
            .operation
            .effects_possible()
            .map_err(|e| e.to_string())?;
        state.phase = Phase::HoldingCooling;
        state.attempted = Some(Step::PumpHold);
        progress.send_replace(state.clone());
        let full = Pwm::max();
        handle
            .write(&pump_pwm::build_request(PumpPwmSource::Mcu, full))
            .map_err(|e| e.to_string())?;
        state.pump_hold_written = true;
        progress.send_replace(state.clone());
        if options.cooling_hold == CoolingHold::PumpAndControllerFan {
            wait(Duration::ZERO, &cancel, &shutdown)?;
            state.attempted = Some(Step::FanHold);
            progress.send_replace(state.clone());
            handle
                .write(&fan_pwm::build_request(full))
                .map_err(|e| e.to_string())?;
            state.fan_hold_written = true;
            progress.send_replace(state.clone());
        }
        wait(timing.settle, &cancel, &shutdown)?;
        drain(handle)?;
        for (index, packet) in BCommandPacket::build_packets_for_write(11, &image.0).enumerate() {
            let expected = ChunkExpectation::new(&packet).map_err(|e| e.to_string())?;
            wait(Duration::ZERO, &cancel, &shutdown)?;
            state.phase = Phase::Uploading;
            state.attempted = Some(Step::Chunk(
                u32::try_from(index).map_err(|e| e.to_string())?,
            ));
            progress.send_replace(state.clone());
            handle.write(packet.as_bytes()).map_err(|e| e.to_string())?;
            state.chunk_writes += 1;
            progress.send_replace(state.clone());
            let bytes = reply(handle, timing.reply, &cancel, &shutdown)?;
            expected.decode(&bytes).map_err(|e| e.to_string())?;
            state.chunk_echoes += 1;
            progress.send_replace(state.clone());
            wait(timing.inter_chunk, &cancel, &shutdown)?;
        }
        wait(timing.before_activate, &cancel, &shutdown)?;
        state.phase = Phase::Activating;
        state.attempted = Some(Step::Activate);
        progress.send_replace(state.clone());
        handle
            .write(BCommandPacket::new_request(10, 0, &[1], 1).as_bytes())
            .map_err(|e| e.to_string())?;
        state.activation_written = true;
        progress.send_replace(state.clone());
        let bytes = reply(handle, timing.reply, &cancel, &shutdown)?;
        decode_activation(&bytes).map_err(|e| e.to_string())?;
        state.phase = Phase::AwaitingReconnect;
        Ok(())
    })();
    let mut result = match operation {
        Ok(()) => Outcome {
            progress: state,
            diagnostic: None,
        },
        Err(error) => Outcome::stopped(state, error),
    };
    if let Err(error) = options.operation.finish(&result) {
        let before = result.diagnostic.take().unwrap_or_default();
        result.diagnostic = Some(format!("{before}; firmware outcome not durable: {error}"));
        result.progress.phase = Phase::Unknown;
    }
    progress.send_replace(result.progress.clone());
    result
}

#[cfg(test)]
mod tests;
