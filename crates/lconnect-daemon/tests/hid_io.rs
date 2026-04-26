//! Integration test for the hid_io actor against the in-process
//! `MockBackend`. Verifies that a `WriteRead64` command queued on the
//! actor's mpsc returns the canned A-class response from the mock.

use lconnect_daemon::actor::{hid_io, HidCommand};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{oneshot, watch};

fn lcdv2_info() -> DeviceInfo {
    // `DeviceInfo` is `#[non_exhaustive]`; use the public constructor.
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        PathBuf::from("/mock/hidraw0"),
    )
}

#[tokio::test]
async fn write_read_64_returns_canned_response() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    // Queue an A-class echo response: [report_id=0x01, cmd_echo=0x86, ...].
    let mut canned = vec![0u8; 64];
    canned[0] = 0x01;
    canned[1] = 0x86;
    mock.queue_response(&lcdv2_info(), canned);

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let tx = hid_io::spawn(backend, lcdv2_info(), sd_rx).expect("spawn hid_io");

    let (reply_tx, reply_rx) = oneshot::channel();
    let mut bytes = vec![0u8; 64];
    bytes[0] = 0x01;
    bytes[1] = 0x86;
    tx.send(HidCommand::WriteRead64 {
        bytes,
        expected_cmd: 0x86,
        reply: reply_tx,
    })
    .await
    .expect("send hid command");

    let resp = reply_rx
        .await
        .expect("hid_io dropped reply channel")
        .expect("hid_io returned an error");
    assert_eq!(resp[0], 0x01);
    assert_eq!(resp[1], 0x86);
}
