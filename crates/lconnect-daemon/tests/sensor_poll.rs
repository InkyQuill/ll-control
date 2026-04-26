//! Integration test for the sensor_poll actor against the in-process
//! `MockBackend`. Verifies that after one or more poll ticks, the
//! `watch::Receiver` carries a `SensorSnapshot` decoded from a canned
//! GetHandshakeInfo response.

use lconnect_daemon::actor::{hid_io, sensor_poll};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

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

/// Build a synthetic A-class GetHandshakeInfo response matching the
/// fixture in `crates/protocol/tests/galahad2_lcdv2_handshake.rs`:
///   fan_rpm = 1234, pump_rpm = 2980, temp_valid = 1,
///   temp_integer = 42, temp_fraction = 7  → 42.7 °C → 427 tenths.
fn canned_handshake_response() -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0] = 0x01; // ReportID
    buf[1] = 129; // command echo (0x81)
    buf[5] = 7; // length of payload
    buf[6] = 0x04;
    buf[7] = 0xD2; // fan_rpm = 1234
    buf[8] = 0x0B;
    buf[9] = 0xA4; // pump_rpm = 2980
    buf[10] = 1; // temp_valid
    buf[11] = 42; // temp_integer
    buf[12] = 7; // temp_fraction
    buf
}

#[tokio::test]
async fn sensor_poll_publishes_decoded_snapshot() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    // Queue several canned responses so a few ticks have something to
    // read even if scheduling jitter lets multiple polls fire.
    for _ in 0..4 {
        mock.queue_response(&lcdv2_info(), canned_handshake_response());
    }

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);

    let hid_tx = hid_io::spawn(backend, lcdv2_info(), sd_rx.clone()).expect("spawn hid_io");
    let mut snap_rx = sensor_poll::spawn(hid_tx, 50, sd_rx);

    // Wait for at least one tick to land. `changed()` resolves on the
    // first non-initial value; bound it with a timeout so a hung test
    // can't wedge CI forever.
    tokio::time::timeout(Duration::from_millis(750), snap_rx.changed())
        .await
        .expect("sensor_poll did not publish within 750 ms")
        .expect("sensor_poll watch closed");

    let snap = *snap_rx.borrow();
    assert_eq!(snap.fan_rpm, 1234, "fan_rpm decoded");
    assert_eq!(snap.pump_rpm, 2980, "pump_rpm decoded");
    assert!(snap.temp_valid, "temp_valid decoded");
    assert_eq!(
        snap.coolant_c_x10, 427,
        "tenths-of-C: 42 * 10 + (7 % 10) = 427"
    );
    assert!(
        snap.last_seen_unix_ms > 0,
        "last_seen_unix_ms must be set on the first successful tick"
    );
}
