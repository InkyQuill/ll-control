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
//! up with `Error::EchoExhausted`.

use super::{HidCommand, Shutdown};
use lconnect_hid::{DeviceInfo, HidBackend, OpenHandle};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::debug;

/// Spawn the per-device hid_io actor.
///
/// Opens the HID handle synchronously (caller is on the manager task,
/// not a hot path). The returned `Sender` is the only way for sibling
/// actors to perform I/O against this device.
///
/// # Errors
///
/// Returns the underlying `lconnect_hid::Error` (wrapped via `anyhow`)
/// if `backend.open()` fails — typically `DeviceNotFound` if the
/// hidraw path has gone away between enumeration and spawn.
pub fn spawn(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    mut shutdown: Shutdown,
) -> anyhow::Result<mpsc::Sender<HidCommand>> {
    let handle: Box<dyn OpenHandle> = backend.open(&info)?;
    // `impl<T: ?Sized> From<Box<T>> for Arc<T>` lets us share the handle
    // across the spawn_blocking calls below without reallocating.
    let handle: Arc<dyn OpenHandle> = handle.into();
    let (tx, mut rx) = mpsc::channel::<HidCommand>(64);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    debug!("hid_io shutting down");
                    break;
                }
                maybe_cmd = rx.recv() => match maybe_cmd {
                    Some(cmd) => process_one(handle.clone(), cmd).await,
                    None => break,
                },
            }
        }
    });

    Ok(tx)
}

async fn process_one(handle: Arc<dyn OpenHandle>, cmd: HidCommand) {
    match cmd {
        HidCommand::Write { bytes, reply } => {
            let h = handle.clone();
            let r = tokio::task::spawn_blocking(move || h.write(&bytes))
                .await
                .expect("hid write blocking task panicked");
            let _ = reply.send(r);
        }
        HidCommand::WriteRead64 {
            bytes,
            expected_cmd,
            reply,
        } => {
            let h = handle.clone();
            let r = tokio::task::spawn_blocking(move || -> lconnect_hid::Result<[u8; 64]> {
                h.write(&bytes)?;
                let mut buf = [0u8; 64];
                drain_for_echo_64(&*h, &mut buf, expected_cmd)?;
                Ok(buf)
            })
            .await
            .expect("hid write_read_64 blocking task panicked");
            let _ = reply.send(r);
        }
        HidCommand::WriteRead512 {
            bytes,
            expected_cmd,
            reply,
        } => {
            let h = handle.clone();
            let r = tokio::task::spawn_blocking(move || -> lconnect_hid::Result<[u8; 512]> {
                h.write(&bytes)?;
                let mut buf = [0u8; 512];
                drain_for_echo_512(&*h, &mut buf, expected_cmd)?;
                Ok(buf)
            })
            .await
            .expect("hid write_read_512 blocking task panicked");
            let _ = reply.send(r);
        }
    }
}

fn drain_for_echo_64(
    h: &dyn OpenHandle,
    buf: &mut [u8; 64],
    expected_cmd: u8,
) -> lconnect_hid::Result<()> {
    drain_for_echo(h, &mut buf[..], 0x01, expected_cmd)
}

fn drain_for_echo_512(
    h: &dyn OpenHandle,
    buf: &mut [u8; 512],
    expected_cmd: u8,
) -> lconnect_hid::Result<()> {
    drain_for_echo(h, &mut buf[..], 0x02, expected_cmd)
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
) -> lconnect_hid::Result<()> {
    let mut stale = 0u32;
    let mut timeout = 0u32;
    for _ in 0..4 {
        let n = h.read_timeout(buf, 500)?;
        if n == 0 {
            timeout += 1;
            continue;
        }
        if n >= 2 && buf[0] == expected_report_id && buf[1] == expected_cmd {
            return Ok(());
        }
        stale += 1;
    }
    Err(lconnect_hid::Error::EchoExhausted {
        stale,
        timeout,
        expected_cmd,
    })
}
