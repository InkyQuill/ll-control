//! Integration test for the hid_io actor against the in-process
//! `MockBackend`. Verifies that a `WriteRead64` command queued on the
//! actor's mpsc returns the canned A-class response from the mock.

use lconnect_daemon::actor::{hid_io, HidCommand};
use lconnect_daemon::stats::Stats;
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
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
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");

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
    assert_eq!(
        stats.hid_write_count.load(Ordering::Relaxed),
        1,
        "hid_io must bump hid_write_count once on a successful WriteRead64"
    );
}

#[tokio::test]
async fn write_read_512_c_class_returns_canned_response() {
    // C-class framing uses report_id 0x03 (e.g., SendH264Frame512 cmd 0x0d).
    // Pre-fix, drain_for_echo_512 hardcoded report_id 0x02 and would
    // discard this response as stale, exhausting the retry budget and
    // poisoning the device's USB read buffer for the next call.
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let mut canned = vec![0u8; 512];
    canned[0] = 0x03;
    canned[1] = 0x0d;
    mock.queue_response(&lcdv2_info(), canned);

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");

    let (reply_tx, reply_rx) = oneshot::channel();
    let mut bytes = vec![0u8; 512];
    bytes[0] = 0x03;
    bytes[1] = 0x0d;
    tx.send(HidCommand::WriteRead512 {
        bytes,
        expected_report_id: 0x03,
        expected_cmd: 0x0d,
        reply: reply_tx,
    })
    .await
    .expect("send hid command");

    let resp = reply_rx
        .await
        .expect("hid_io dropped reply channel")
        .expect("hid_io returned an error");
    assert_eq!(resp[0], 0x03);
    assert_eq!(resp[1], 0x0d);
}

/// Pinning the regression that motivated moving the counter into hid_io:
/// a plain `HidCommand::Write` (the variant that keeper-driven writes
/// use) must bump `Stats::hid_write_count`. Pre-fix, only the IPC
/// helpers counted, so keeper-driven writes (lighting_keeper at 1.5s,
/// lcd_keeper at 15s) silently undercounted `daemon stats hid_write_count`.
#[tokio::test]
async fn write_command_increments_hid_write_count() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let stats = Arc::new(Stats::new());
    let tx = hid_io::spawn(backend, lcdv2_info(), Arc::clone(&stats), sd_rx).expect("spawn hid_io");

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(HidCommand::Write {
        bytes: vec![0x01, 0x86, 0x00, 0x00, 0x00, 0x00],
        reply: reply_tx,
    })
    .await
    .expect("send hid command");

    reply_rx
        .await
        .expect("hid_io dropped reply channel")
        .expect("hid_io returned an error");

    assert_eq!(
        stats.hid_write_count.load(Ordering::Relaxed),
        1,
        "hid_io must bump hid_write_count on a successful Write"
    );
}
