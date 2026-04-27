//! Integration test for the lcd_keeper actor against the in-process
//! `MockBackend`. Verifies that:
//!   1. `LcdFrameMsg::Set(jpeg)` pushes the cmd-14 (SendJpeg) packet
//!      stream immediately, and
//!   2. After one refresh interval, the stored frame is re-pushed.
//!
//! Cmd 14 is a B-class write: 1024-byte packets with REPORT_ID at
//! offset 0 and the cmd byte at offset 1.

use bytes::Bytes;
use lconnect_daemon::actor::{hid_io, lcd_keeper, LcdFrameMsg};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

const SEND_JPEG_CMD: u8 = 14;
const B_REPORT_ID: u8 = 0x02;
const B_PACKET_LEN: usize = 1024;

fn lcdv2_info() -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        PathBuf::from("/mock/hidraw0"),
    )
}

#[tokio::test]
async fn lcd_keeper_pushes_on_set_and_repushes_on_interval() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let mock_for_assert = mock.clone();

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);

    let hid_tx = hid_io::spawn(
        backend,
        lcdv2_info(),
        std::sync::Arc::new(lconnect_daemon::stats::Stats::new()),
        sd_rx.clone(),
    )
    .expect("spawn hid_io");

    let (frame_tx, frame_rx) = mpsc::channel::<LcdFrameMsg>(8);
    // 100ms refresh: at ~250ms we expect the immediate Set push plus at
    // least one interval re-push.
    let _handle = lcd_keeper::spawn(hid_tx, 100, frame_rx, sd_rx);

    // A short payload that fits in a single 1024-byte B-packet; the
    // codec accepts arbitrary bytes (no JPEG-structure validation), so
    // a synthetic SOI fragment is enough -- mirrors the protocol
    // crate's existing send_jpeg integration tests.
    let jpeg: Bytes = Bytes::from_static(&[0xFF, 0xD8, 0xFF, 0xE0]);
    frame_tx
        .send(LcdFrameMsg::Set(jpeg))
        .await
        .expect("frame_tx send");

    // Hold the sender open across the interval tick so frame_rx does
    // not close and terminate the keeper before the re-push fires.
    tokio::time::sleep(Duration::from_millis(250)).await;
    drop(frame_tx);

    let history = mock_for_assert.write_history(&lcdv2_info());

    let cmd14_writes: Vec<&Vec<u8>> = history
        .iter()
        .filter(|w| w.len() == B_PACKET_LEN && w[0] == B_REPORT_ID && w[1] == SEND_JPEG_CMD)
        .collect();

    assert!(
        cmd14_writes.len() >= 2,
        "expected >=2 cmd-14 packets (immediate Set push + >=1 interval re-push), \
         got {} (total writes: {})",
        cmd14_writes.len(),
        history.len(),
    );

    // Spot-check the payload was preserved at offset 11 (B-packet
    // header is 11 bytes: ReportID, Cmd, DataSize[4], PacketNumber[3],
    // Length[2]).
    let first = cmd14_writes[0];
    assert_eq!(
        &first[11..15],
        &[0xFF, 0xD8, 0xFF, 0xE0],
        "payload at offset 11"
    );
    assert_eq!(&first[2..6], &[0, 0, 0, 4], "DataSize = 4");
    assert_eq!(&first[9..11], &[0, 4], "Length = 4");
}
