//! lcd_keeper -- holds the current JPEG frame and re-pushes it on a fixed
//! cadence to keep the LCD out of LocalUI mode.
//!
//! Cmd 14 (`SendJpeg`) is empirically transient: the firmware decodes and
//! displays the JPEG within ~50ms but stops showing it within roughly
//! 20s if the keeper does not refresh (see the rustdoc on
//! `lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request`).
//! For sustained static-image display, this actor re-pushes the stored
//! frame every `refresh_ms` (production: 15000) so the firmware never
//! sees a >20s gap.
//!
//! Internal state is `Option<bytes::Bytes>` -- the "current frame slot".
//! - `LcdFrameMsg::Set(b)` pushes immediately AND stores the frame.
//! - `LcdFrameMsg::Clear` drops the slot. The keeper itself does NOT
//!   issue any wire command on Clear -- it just stops refreshing. The
//!   IPC layer's `clear_lcd` is the path that actually clears the
//!   panel, by sending `LCDControl{mode=LocalUI}` (cmd 12) directly
//!   through the hid_io channel after instructing this keeper to stop.
//!   P1H-1 live smoke confirmed the firmware does NOT auto-revert
//!   when the keeper goes quiet -- the panel holds the last uploaded
//!   frame indefinitely until an explicit mode change arrives.
//! - On interval tick, if the slot is `Some`, the frame is re-pushed.
//!
//! Phase 4's theme engine plugs into the same `frame_rx` mpsc with
//! generated JPEGs at any cadence; the keeper continues to drive the
//! HID layer at the configured refresh interval.

use super::{HidCommand, LcdFrameMsg, Shutdown};
use bytes::Bytes;
use lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// Spawn the per-device lcd_keeper actor.
///
/// `refresh_ms` is the re-push cadence in milliseconds (production uses
/// 15000 -- comfortably below the firmware's ~20s LocalUI auto-revert).
///
/// The actor terminates when the shutdown signal flips or `frame_rx` is
/// closed.
pub fn spawn(
    hid_io_tx: mpsc::Sender<HidCommand>,
    refresh_ms: u64,
    mut frame_rx: mpsc::Receiver<LcdFrameMsg>,
    mut shutdown: Shutdown,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut current: Option<Bytes> = None;
        let mut interval = tokio::time::interval(Duration::from_millis(refresh_ms));
        // Skip the immediate first tick: a freshly-spawned keeper with an
        // empty slot has nothing to push, and a Set arriving moments later
        // will push immediately on its own.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    debug!("lcd_keeper shutting down");
                    break;
                }
                maybe_msg = frame_rx.recv() => match maybe_msg {
                    Some(LcdFrameMsg::Set(jpeg)) => {
                        push_frame(&hid_io_tx, &jpeg).await;
                        current = Some(jpeg);
                    }
                    Some(LcdFrameMsg::Clear) => {
                        // Drop the slot so we stop refreshing. The
                        // keeper does NOT issue any wire command here
                        // -- the IPC layer's clear_lcd sends
                        // LCDControl{mode=LocalUI} directly to revert
                        // the panel. (Pre-P1H-1 we believed the
                        // firmware self-reverted; live smoke proved
                        // otherwise. See module-level docs.)
                        current = None;
                    }
                    None => {
                        debug!("lcd_keeper frame_rx closed; exiting");
                        break;
                    }
                },
                _ = interval.tick() => {
                    if let Some(jpeg) = &current {
                        push_frame(&hid_io_tx, jpeg).await;
                    }
                }
            }
        }
    })
}

/// Build the cmd-14 packet stream for `jpeg` and write each packet via
/// `hid_io_tx`, awaiting the per-packet oneshot reply.
///
/// On any error (build failure, send-side mpsc closed, write error,
/// dropped reply), logs at `warn!` and returns. The next interval tick
/// will retry; partial-write recovery on the firmware side is undefined
/// (see the rustdoc on `build_send_jpeg_request`), so we deliberately do
/// not attempt mid-stream resync.
async fn push_frame(hid_io_tx: &mpsc::Sender<HidCommand>, jpeg: &[u8]) {
    let packets = match build_send_jpeg_request(jpeg) {
        Ok(p) => p,
        Err(e) => {
            warn!("lcd_keeper: build_send_jpeg_request failed: {}", e);
            return;
        }
    };
    for (idx, packet) in packets.iter().enumerate() {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(e) = hid_io_tx
            .send(HidCommand::Write {
                bytes: packet.to_vec(),
                reply: reply_tx,
            })
            .await
        {
            warn!("lcd_keeper: hid_io_tx send failed at packet {}: {}", idx, e);
            return;
        }
        match reply_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!("lcd_keeper: write returned error at packet {}: {}", idx, e);
                return;
            }
            Err(e) => {
                warn!("lcd_keeper: reply dropped at packet {}: {}", idx, e);
                return;
            }
        }
    }
}
