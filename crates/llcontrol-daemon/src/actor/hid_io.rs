//! hid_io actor — owns the HID handle, serialises all writes/reads.
//!
//! Per-device single-consumer mpsc. Every HID I/O runs on
//! `tokio::task::spawn_blocking`; hidapi-rs is synchronous and we
//! must not block the runtime.
//!
//! The drain-for-echo helpers mirror verify-galahad's `run_probe`
//! convention identified in Phase 0e: responses begin with the
//! report-id byte at offset 0 and a *plain* command echo at offset 1
//! (no `| 0x80` mask). Up to four reads are tolerated before giving
//! up with `Error::EchoExhausted`. Sensor requests instead retain one pending
//! reply while complete frames run; only nonblocking idle reads or the
//! existing frame observation consume that reply.

use super::{FirmwareResponses, FrameOutcome, FrameReadPolicy, HidCommand, Shutdown};
use crate::stats::Stats;
use llcontrol_hid::{DeviceInfo, HidBackend, OpenHandle};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

const MAX_FRAME_OBSERVATION_TIMEOUT_MS: i32 = 1_000;
const FRAME_HEADER_LEN: usize = 11;

const SENSOR_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const SENSOR_READ_INTERVAL: Duration = Duration::from_millis(5);

struct PendingSensor {
    reply: oneshot::Sender<llcontrol_hid::Result<[u8; 64]>>,
    deadline: tokio::time::Instant,
}

fn complete_sensor(pending: &mut Option<PendingSensor>, result: llcontrol_hid::Result<[u8; 64]>) {
    if let Some(sensor) = pending.take() {
        let _ = sensor.reply.send(result);
    }
}

fn route_sensor(pending: &mut Option<PendingSensor>, bytes: &[u8]) {
    // Parse the actual read extent before padding. Firmware PN1 and B/C
    // observations are unrelated; a raw observation is never an H.264 ACK.
    if pending.is_none()
        || bytes.len() > 64
        || bytes.get(2..5) != Some(&[0, 0, 0])
        || llcontrol_protocol::galahad2_lcdv2::handshake::HandshakeInfo::from_response(bytes)
            .is_err()
    {
        return;
    }
    let mut response = [0; 64];
    response[..bytes.len()].copy_from_slice(bytes);
    complete_sensor(pending, Ok(response));
}

async fn read_pending_sensor(handle: Arc<dyn OpenHandle>, pending: &mut Option<PendingSensor>) {
    let result = tokio::task::spawn_blocking(move || {
        let mut buffer = [0; 512];
        let count = handle.read_timeout(&mut buffer, 0)?;
        buffer.get(..count).map(<[u8]>::to_vec).ok_or_else(|| {
            llcontrol_hid::Error::BlockingTaskFailed(format!(
                "sensor read returned {count} bytes into a 512-byte buffer"
            ))
        })
    })
    .await;
    match result {
        Ok(Ok(bytes)) => route_sensor(pending, &bytes),
        Ok(Err(error)) => complete_sensor(pending, Err(error)),
        Err(error) => complete_sensor(
            pending,
            Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                "sensor read: {error}"
            ))),
        ),
    }
}

fn invalid_frame_batch(reason: impl Into<String>) -> llcontrol_hid::Error {
    llcontrol_hid::Error::BlockingTaskFailed(format!("invalid frame batch: {}", reason.into()))
}

/// Validate a complete LCDV2 frame transaction before its first HID write.
///
/// B framing follows `BCommandPacket.cs:1-189`; C framing follows
/// `CCommandPacket.cs:108-176`. Both use an 11-byte header containing the
/// total payload size, 24-bit packet number, and per-report payload length.
fn validate_frame_batch(
    packets: &[Vec<u8>],
    read_policy: FrameReadPolicy,
) -> llcontrol_hid::Result<()> {
    if let FrameReadPolicy::ObserveOnce512 { timeout_ms } = read_policy {
        if !(0..=MAX_FRAME_OBSERVATION_TIMEOUT_MS).contains(&timeout_ms) {
            return Err(invalid_frame_batch(format!(
                "observation timeout {timeout_ms}ms is outside 0..={MAX_FRAME_OBSERVATION_TIMEOUT_MS}ms"
            )));
        }
    }

    let first = packets
        .first()
        .ok_or_else(|| invalid_frame_batch("packet list is empty"))?;
    let (report_id, command, packet_len, payload_max, max_total) = match first.get(..2) {
        Some([0x02, 13]) => (
            0x02,
            13,
            llcontrol_protocol::galahad2_lcdv2::bpacket::OUTPUT_LEN,
            llcontrol_protocol::galahad2_lcdv2::bpacket::PAYLOAD_MAX,
            llcontrol_protocol::galahad2_lcdv2::h264::MAX_H264_FRAME_BYTES,
        ),
        Some([0x02, 14]) => (
            0x02,
            14,
            llcontrol_protocol::galahad2_lcdv2::bpacket::OUTPUT_LEN,
            llcontrol_protocol::galahad2_lcdv2::bpacket::PAYLOAD_MAX,
            llcontrol_protocol::galahad2_lcdv2::jpeg::MAX_JPEG_BYTES,
        ),
        Some([0x03, 13]) => (
            0x03,
            13,
            llcontrol_protocol::galahad2_lcdv2::cpacket::OUTPUT_LEN,
            llcontrol_protocol::galahad2_lcdv2::cpacket::PAYLOAD_MAX,
            llcontrol_protocol::galahad2_lcdv2::h264::MAX_H264_FRAME_BYTES,
        ),
        _ => return Err(invalid_frame_batch("unsupported report ID or command")),
    };
    if first.len() != packet_len {
        return Err(invalid_frame_batch(format!(
            "packet 0 has length {}, expected {packet_len}",
            first.len()
        )));
    }

    let declared_total = u32::from_be_bytes([first[2], first[3], first[4], first[5]]) as usize;
    if declared_total == 0 || declared_total > max_total {
        return Err(invalid_frame_batch(format!(
            "declared payload size {declared_total} is outside 1..={max_total}"
        )));
    }
    if packets.len() != declared_total.div_ceil(payload_max) {
        return Err(invalid_frame_batch(
            "packet count does not match declared payload size",
        ));
    }

    let mut payload_sum = 0usize;
    for (index, packet) in packets.iter().enumerate() {
        if packet.len() != packet_len {
            return Err(invalid_frame_batch(format!(
                "packet {index} has length {}, expected {packet_len}",
                packet.len()
            )));
        }
        if packet[0] != report_id || packet[1] != command {
            return Err(invalid_frame_batch(format!(
                "packet {index} changes report ID or command"
            )));
        }
        let packet_total =
            u32::from_be_bytes([packet[2], packet[3], packet[4], packet[5]]) as usize;
        if packet_total != declared_total {
            return Err(invalid_frame_batch(format!(
                "packet {index} changes declared payload size"
            )));
        }
        let packet_number = u32::from_be_bytes([0, packet[6], packet[7], packet[8]]) as usize;
        if packet_number != index {
            return Err(invalid_frame_batch(format!(
                "packet number {packet_number} does not match index {index}"
            )));
        }
        let length = u16::from_be_bytes([packet[9], packet[10]]) as usize;
        let expected_length = (declared_total - payload_sum).min(payload_max);
        if length != expected_length {
            return Err(invalid_frame_batch(format!(
                "packet {index} payload length {length}, expected {expected_length}"
            )));
        }
        if packet[FRAME_HEADER_LEN + length..]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(invalid_frame_batch(format!(
                "packet {index} has non-zero bytes after its payload"
            )));
        }
        payload_sum += length;
    }
    if payload_sum != declared_total {
        return Err(invalid_frame_batch(
            "payload lengths do not sum to declared size",
        ));
    }
    Ok(())
}

/// Spawn the per-device hid_io actor.
///
/// Opens the HID handle synchronously (caller is on the manager task,
/// not a hot path). The returned `Sender` is the only way for sibling
/// actors to perform I/O against this device.
///
/// `stats` is the process-wide counter set; the actor bumps
/// `hid_write_count` once per successful HID write (across all
/// `HidCommand` variants). Centralising the increment here is the
/// single source of truth -- keepers and the IPC layer all funnel
/// through `process_one`, so no caller can bypass the count by sending
/// `HidCommand::Write` directly.
///
/// # Errors
///
/// Returns the underlying `llcontrol_hid::Error` (wrapped via `anyhow`)
/// if `backend.open()` fails — typically `DeviceNotFound` if the
/// hidraw path has gone away between enumeration and spawn.
pub fn spawn(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    stats: Arc<Stats>,
    shutdown: Shutdown,
) -> anyhow::Result<mpsc::Sender<HidCommand>> {
    let (tx, _task) = spawn_tracked(backend, info, stats, shutdown)?;
    Ok(tx)
}

/// Spawn with a completion handle used by the device teardown barrier.
///
/// # Errors
/// Returns the error from opening the underlying device.
pub fn spawn_tracked(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    stats: Arc<Stats>,
    shutdown: Shutdown,
) -> anyhow::Result<(mpsc::Sender<HidCommand>, tokio::task::JoinHandle<()>)> {
    spawn_inner(backend, info, stats, shutdown, None)
}
pub(super) fn spawn_tracked_with_journal(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    stats: Arc<Stats>,
    shutdown: Shutdown,
    journal: crate::firmware_operation::Journal,
) -> anyhow::Result<(mpsc::Sender<HidCommand>, tokio::task::JoinHandle<()>)> {
    spawn_inner(backend, info, stats, shutdown, Some(journal))
}
struct UpdateContext {
    info: DeviceInfo,
    journal: Option<crate::firmware_operation::Journal>,
}
fn spawn_inner(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    stats: Arc<Stats>,
    mut shutdown: Shutdown,
    journal: Option<crate::firmware_operation::Journal>,
) -> anyhow::Result<(mpsc::Sender<HidCommand>, tokio::task::JoinHandle<()>)> {
    let handle: Box<dyn OpenHandle> = backend.open(&info)?;
    // `impl<T: ?Sized> From<Box<T>> for Arc<T>` lets us share the handle
    // across the spawn_blocking calls below without reallocating.
    let handle: Arc<dyn OpenHandle> = handle.into();
    let update_info = UpdateContext { info, journal };
    let (tx, mut rx) = mpsc::channel::<HidCommand>(64);

    let task = tokio::spawn(async move {
        let mut pending: Option<PendingSensor> = None;
        let mut deferred = None;
        let mut update_fenced = false;
        let mut next_read = tokio::time::Instant::now();
        loop {
            // Manager teardown drops the last sender without changing false.
            // Check closure before dispatching a deferred FIFO command too.
            if *shutdown.borrow() || shutdown.has_changed().is_err() {
                break;
            }
            if pending
                .as_ref()
                .is_some_and(|sensor| tokio::time::Instant::now() >= sensor.deadline)
            {
                complete_sensor(
                    &mut pending,
                    Err(llcontrol_hid::Error::EchoExhausted {
                        stale: 0,
                        timeout: 1,
                        expected_cmd: 129,
                    }),
                );
            }
            if pending.is_none() {
                if let Some(cmd) = deferred.take() {
                    process_one(
                        handle.clone(),
                        &stats,
                        cmd,
                        &mut pending,
                        &shutdown,
                        &mut update_fenced,
                        &update_info,
                    )
                    .await;
                    continue;
                }
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => {
                    debug!("hid_io shutting down");
                    break;
                }
                maybe_cmd = rx.recv(), if deferred.is_none() => match maybe_cmd {
                    Some(cmd) => {
                        if pending.is_some() && matches!(cmd, HidCommand::FirmwareUpdate { .. } | HidCommand::Firmware { .. } | HidCommand::PollSensors { .. } | HidCommand::WriteRead64 { .. } | HidCommand::WriteRead512 { .. }) {
                            // Hold only the FIFO head. Later frames cannot overtake a control.
                            deferred = Some(cmd);
                        } else {
                            process_one(handle.clone(), &stats, cmd, &mut pending, &shutdown, &mut update_fenced, &update_info).await;
                        }
                    }
                    None => break,
                },
                _ = tokio::time::sleep_until(next_read), if pending.is_some() => {
                    read_pending_sensor(handle.clone(), &mut pending).await;
                    next_read = tokio::time::Instant::now() + SENSOR_READ_INTERVAL;
                }
            }
        }
        // Dropping pending and queued replies cancels consumers only after any
        // started blocking transaction has joined above.
    });

    Ok((tx, task))
}

async fn process_one(
    handle: Arc<dyn OpenHandle>,
    stats: &Stats,
    cmd: HidCommand,
    pending: &mut Option<PendingSensor>,
    shutdown: &Shutdown,
    update_fenced: &mut bool,
    update_info: &UpdateContext,
) {
    if *update_fenced {
        reject_after_update(cmd);
        return;
    }
    match cmd {
        HidCommand::FirmwareUpdate {
            operation,
            image,
            cooling_hold,
            cancel,
            progress,
            reply,
        } => {
            *update_fenced = true;
            let Some(journal) = update_info.journal.clone() else {
                let _ = reply.send(super::firmware_update::Outcome::stopped(
                    super::firmware_update::Progress::default(),
                    "HID actor has no firmware journal scope",
                ));
                return;
            };
            progress.send_replace(super::firmware_update::Progress::default());
            let observed = progress.clone();
            let shutdown = shutdown.clone();
            let update_info = update_info.info.clone();
            let joined = tokio::task::spawn_blocking(move || {
                super::firmware_update::run(
                    &*handle,
                    image,
                    super::firmware_update::TransferOptions {
                        target: update_info,
                        operation: *operation,
                        journal,
                        cooling_hold,
                    },
                    cancel,
                    shutdown,
                    progress,
                )
            })
            .await;
            let outcome = joined.unwrap_or_else(|error| {
                super::firmware_update::Outcome::stopped(
                    observed.borrow().clone(),
                    format!("update blocking task: {error}"),
                )
            });
            for _ in 0..outcome.writes() {
                stats.inc_hid_write();
            }
            observed.send_replace(outcome.progress.clone());
            let _ = reply.send(outcome);
        }
        HidCommand::Firmware { reply } => {
            let h = handle.clone();
            let joined = tokio::task::spawn_blocking(move || {
                if let Err(error) =
                    h.write(&llcontrol_protocol::galahad2_lcdv2::firmware::build_request())
                {
                    return (false, Err(error));
                }
                // The Windows display startup allows three seconds for device identity.
                // A late PN0 must not be mistaken for an unsupported firmware version.
                (true, collect_firmware_until(&*h, Duration::from_secs(3)))
            })
            .await;
            let (written, result) = joined.unwrap_or_else(|error| {
                (
                    false,
                    Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                        "firmware query: {error}"
                    ))),
                )
            });
            if written {
                stats.inc_hid_write();
            }
            let _ = reply.send(result);
        }
        HidCommand::PollSensors { reply } => {
            let result = tokio::task::spawn_blocking(move || {
                handle.write(&llcontrol_protocol::galahad2_lcdv2::handshake::build_request())
            })
            .await
            .unwrap_or_else(|error| {
                Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                    "sensor request: {error}"
                )))
            });
            match result {
                Ok(()) => {
                    stats.inc_hid_write();
                    *pending = Some(PendingSensor {
                        reply,
                        deadline: tokio::time::Instant::now() + SENSOR_RESPONSE_TIMEOUT,
                    });
                }
                Err(error) => {
                    let _ = reply.send(Err(error));
                }
            }
        }
        HidCommand::FrameBatch {
            packets,
            read_policy,
            reply,
        } => {
            if let Err(error) = validate_frame_batch(&packets, read_policy) {
                let _ = reply.send(Err(error));
                return;
            }
            let report_count = packets.len();
            let h = handle.clone();
            let joined = tokio::task::spawn_blocking(move || {
                let started = Instant::now();
                let mut written_reports = 0;
                for packet in &packets {
                    if let Err(error) = h.write(packet) {
                        return (written_reports, Err(error));
                    }
                    written_reports += 1;
                }

                let readback = match read_policy {
                    FrameReadPolicy::None => Vec::new(),
                    FrameReadPolicy::ObserveOnce512 { timeout_ms } => {
                        let mut buffer = [0u8; 512];
                        let count = match h.read_timeout(&mut buffer, timeout_ms) {
                            Ok(count) => count,
                            Err(error) => return (written_reports, Err(error)),
                        };
                        let Some(bytes) = buffer.get(..count) else {
                            return (
                                written_reports,
                                Err(invalid_frame_batch(format!(
                                    "read returned {count} bytes into a 512-byte buffer"
                                ))),
                            );
                        };
                        bytes.to_vec()
                    }
                };

                (
                    written_reports,
                    Ok(FrameOutcome {
                        written_reports,
                        readback,
                        elapsed: started.elapsed(),
                    }),
                )
            })
            .await;

            let (written_reports, result) = match joined {
                Ok(result) => result,
                Err(join_err) => (
                    0,
                    Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                        "hid frame batch: {join_err}"
                    ))),
                ),
            };
            for _ in 0..written_reports {
                stats.inc_hid_write();
            }
            match &result {
                Ok(outcome) => route_sensor(pending, &outcome.readback),
                Err(error) if written_reports == report_count => complete_sensor(
                    pending,
                    Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                        "frame observation during sensor request: {error}"
                    ))),
                ),
                Err(_) => {}
            }
            let _ = reply.send(
                result.map_err(|source| llcontrol_hid::Error::FrameBatchFailed {
                    written_reports,
                    source: Box::new(source),
                }),
            );
        }
        HidCommand::Write { bytes, reply } => {
            let h = handle.clone();
            let r = match tokio::task::spawn_blocking(move || h.write(&bytes)).await {
                Ok(inner) => inner,
                Err(join_err) => Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                    "hid write: {join_err}"
                ))),
            };
            // Only count successful writes; a failed write is a HID
            // I/O error, not a write that hit the device.
            if r.is_ok() {
                stats.inc_hid_write();
            }
            let _ = reply.send(r);
        }
        HidCommand::WriteRead64 {
            bytes,
            expected_cmd,
            reply,
        } => {
            let h = handle.clone();
            let r = match tokio::task::spawn_blocking(move || -> llcontrol_hid::Result<[u8; 64]> {
                h.write(&bytes)?;
                let mut buf = [0u8; 64];
                drain_for_echo_64(&*h, &mut buf, expected_cmd)?;
                Ok(buf)
            })
            .await
            {
                Ok(inner) => inner,
                Err(join_err) => Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                    "hid write_read_64: {join_err}"
                ))),
            };
            if r.is_ok() {
                stats.inc_hid_write();
            }
            let _ = reply.send(r);
        }
        HidCommand::WriteRead512 {
            bytes,
            expected_report_id,
            expected_cmd,
            reply,
        } => {
            let h = handle.clone();
            let r =
                match tokio::task::spawn_blocking(move || -> llcontrol_hid::Result<[u8; 512]> {
                    h.write(&bytes)?;
                    let mut buf = [0u8; 512];
                    drain_for_echo_512(&*h, &mut buf, expected_report_id, expected_cmd)?;
                    Ok(buf)
                })
                .await
                {
                    Ok(inner) => inner,
                    Err(join_err) => Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                        "hid write_read_512: {join_err}"
                    ))),
                };
            if r.is_ok() {
                stats.inc_hid_write();
            }
            let _ = reply.send(r);
        }
    }
}

fn reject_after_update(command: HidCommand) {
    let error = || {
        llcontrol_hid::Error::BlockingTaskFailed(
        "HID attachment fenced after firmware update admission; teardown/reconnect verification required".into())
    };
    match command {
        HidCommand::FirmwareUpdate { reply, .. } => {
            // A repeated caller may reuse the first progress sender. Do not
            // overwrite the admitted transaction's terminal Unknown/awaiting state.
            let outcome = super::firmware_update::Outcome::stopped(
                super::firmware_update::Progress::default(),
                "update already admitted for this attachment",
            );
            let _ = reply.send(outcome);
        }
        HidCommand::Firmware { reply } => {
            let _ = reply.send(Err(error()));
        }
        HidCommand::FrameBatch { reply, .. } => {
            let _ = reply.send(Err(error()));
        }
        HidCommand::Write { reply, .. } => {
            let _ = reply.send(Err(error()));
        }
        HidCommand::PollSensors { reply } | HidCommand::WriteRead64 { reply, .. } => {
            let _ = reply.send(Err(error()));
        }
        HidCommand::WriteRead512 { reply, .. } => {
            let _ = reply.send(Err(error()));
        }
    }
}

fn drain_for_echo_64(
    h: &dyn OpenHandle,
    buf: &mut [u8; 64],
    expected_cmd: u8,
) -> llcontrol_hid::Result<()> {
    // A-class report_id is invariant per spec.
    drain_for_echo(h, &mut buf[..], 0x01, expected_cmd)
}

fn drain_for_echo_512(
    h: &dyn OpenHandle,
    buf: &mut [u8; 512],
    expected_report_id: u8,
    expected_cmd: u8,
) -> llcontrol_hid::Result<()> {
    // 512-byte responses span B-class (report_id 0x02) and C-class
    // (report_id 0x03); the caller is responsible for telling us
    // which one to expect so the drain loop matches the real reply
    // instead of burning the retry budget and leaving it in the
    // device's USB buffer for the next unrelated read.
    drain_for_echo(h, &mut buf[..], expected_report_id, expected_cmd)
}

/// Drain up to four responses looking for `[report_id, expected_cmd, ...]`.
///
/// Mirrors verify-galahad's `run_probe` convention (Phase 0e): plain
/// command echo at offset 1, no `| 0x80` mask. Stale frames (wrong
/// report_id or wrong cmd echo) and timeouts (`Ok(0)`) are both
/// tolerated up to the cap; once exhausted we return
/// `Error::EchoExhausted` with the per-class counts so the caller
/// can log a useful diagnosis.
fn drain_for_echo(
    h: &dyn OpenHandle,
    buf: &mut [u8],
    expected_report_id: u8,
    expected_cmd: u8,
) -> llcontrol_hid::Result<()> {
    let mut stale = 0u32;
    let mut timeout = 0u32;
    for _ in 0..4 {
        // Never combine a short read with bytes left over from an earlier response.
        buf.fill(0);
        let n = h.read_timeout(buf, 500)?;
        if n > buf.len() {
            return Err(llcontrol_hid::Error::BlockingTaskFailed(format!(
                "HID read returned {n} bytes into a {}-byte buffer",
                buf.len()
            )));
        }
        if n == 0 {
            timeout += 1;
            continue;
        }
        let payload_end = match expected_report_id {
            0x01 if n >= 6 => Some(6 + usize::from(buf[5])),
            0x02 | 0x03 if n >= 11 => Some(11 + usize::from(u16::from_be_bytes([buf[9], buf[10]]))),
            _ => None,
        };
        if payload_end.is_some_and(|end| end <= n)
            && buf[0] == expected_report_id
            && buf[1] == expected_cmd
        {
            return Ok(());
        }
        stale += 1;
    }
    Err(llcontrol_hid::Error::EchoExhausted {
        stale,
        timeout,
        expected_cmd,
    })
}

/// FW1.6 may omit PN1. Bound explicit identity queries to eight reads/one second.
pub(super) fn collect_firmware(
    handle: &dyn OpenHandle,
) -> llcontrol_hid::Result<FirmwareResponses> {
    collect_firmware_until(handle, Duration::from_secs(1))
}

fn collect_firmware_until(
    handle: &dyn OpenHandle,
    budget: Duration,
) -> llcontrol_hid::Result<FirmwareResponses> {
    use llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket;
    let deadline = Instant::now() + budget;
    let mut version = None;
    let mut datetime = None;
    for _ in 0..8 {
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if remaining == 0 {
            break;
        }
        let timeout = remaining.min(if version.is_some() { 100 } else { 500 }) as i32;
        let mut buffer = [0u8; 512];
        let count = handle.read_timeout(&mut buffer, timeout)?;
        let bytes = buffer.get(..count).ok_or_else(|| {
            llcontrol_hid::Error::BlockingTaskFailed(format!(
                "firmware read returned {count} bytes into a 512-byte buffer"
            ))
        })?;
        if count == 0 {
            if version.is_some() {
                break;
            }
            continue;
        }
        if count > 64 || ACommandPacket::parse_response(bytes, 134).is_err() {
            continue;
        }
        let number = u16::from_be_bytes([bytes[3], bytes[4]]);
        let target = match number {
            0 => &mut version,
            1 => &mut datetime,
            _ => continue,
        };
        if target.is_none() {
            let mut packet = [0; 64];
            packet[..count].copy_from_slice(bytes);
            *target = Some(packet);
        }
        if version.is_some() && datetime.is_some() {
            break;
        }
    }
    let version = version.ok_or_else(|| {
        llcontrol_hid::Error::BlockingTaskFailed("firmware PN0 version response missing".into())
    })?;
    Ok(FirmwareResponses { version, datetime })
}
