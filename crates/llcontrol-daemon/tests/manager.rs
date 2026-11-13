//! Integration tests for the device manager (P1D-10).
//!
//! Wires a `MockBackend` into a `DeviceManager`, then drives the
//! 50 ms hot-plug poll by mutating the mock device list and waiting
//! for the next reconciliation.

use llcontrol_daemon::config::Config;
use llcontrol_daemon::manager::DeviceManager;
use llcontrol_daemon::persistence::PersistenceStore;
use llcontrol_daemon::stats::Stats;
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use llcontrol_ipc::DeviceId;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

/// Build a `Config` with a tight 50 ms hot-plug poll so reconciliation
/// happens fast enough for tests to complete in <1 s.
fn fast_poll_config() -> Config {
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 50;
    cfg
}

fn lcdv2_info(serial: &str, path: &str) -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some(serial.to_string()),
        PathBuf::from(path),
    )
}

#[tokio::test]
async fn attach_spawns_actor_tree() {
    let mock = MockBackend::new();
    let backend: Arc<dyn HidBackend> = Arc::new(mock.clone());

    let tmp = TempDir::new().expect("tempdir");
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));

    let manager = DeviceManager::new(backend, store, fast_poll_config(), Arc::new(Stats::new()));
    let handle = manager.spawn().await;

    // Initially: no devices.
    assert!(handle.list().await.is_empty(), "manager starts empty");

    // Attach one device. The poll loop is on a 50 ms tick and
    // `replay_persisted_settings` adds ~50 ms of settle (no PWM/LCD
    // replay because the on-disk settings are at default). 400 ms of
    // slack covers tokio scheduling jitter on loaded CI runners.
    mock.add_device(lcdv2_info("SN-A", "/mock/hidraw0"));
    tokio::time::sleep(Duration::from_millis(400)).await;

    let devs = handle.list().await;
    assert_eq!(devs.len(), 1, "exactly one device after attach");
    assert_eq!(devs[0].id, DeviceId::new("galahad2-lcdv2:0001"));
    assert_eq!(devs[0].family, "galahad2-lcdv2");
    assert_eq!(devs[0].vid, 0x0416);
    assert_eq!(devs[0].pid, 0x7395);
    assert_eq!(devs[0].serial.as_deref(), Some("SN-A"));
}

#[tokio::test]
async fn detach_drops_actor_tree() {
    let mock = MockBackend::new();
    let backend: Arc<dyn HidBackend> = Arc::new(mock.clone());

    let tmp = TempDir::new().expect("tempdir");
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));

    let info = lcdv2_info("SN-A", "/mock/hidraw0");
    mock.add_device(info.clone());

    let manager = DeviceManager::new(backend, store, fast_poll_config(), Arc::new(Stats::new()));
    let handle = manager.spawn().await;

    // Wait for the attach to be observed (poll tick + replay settle).
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(handle.list().await.len(), 1, "one device after attach");

    // Capture a clone of the hid_io_tx so we can prove the channel
    // closes after the detach reconciles.
    let id = DeviceId::new("galahad2-lcdv2:0001");
    let hid_tx = handle
        .with_device(&id, |d| d.handles.hid_io_tx.clone())
        .await
        .expect("device present before detach");

    // Detach: remove from the mock's enumeration list. The next poll
    // tick (within ~50 ms) drops the ManagedDevice, which drops its
    // shutdown::Sender, which flips the watch and drives every actor
    // out of its select! loop.
    mock.remove_device(&info);
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        handle.list().await.is_empty(),
        "device map empty after detach"
    );

    // Best-effort liveness check: once the hid_io actor's select!
    // observes the shutdown signal, it drops its receiver and the
    // matching Sender starts reporting closed. We give it a brief
    // grace period to converge, since the actor may still be draining
    // an in-flight request at the moment we drop the watch.
    let mut closed = false;
    for _ in 0..20 {
        if hid_tx.is_closed() {
            closed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        closed,
        "hid_io_tx still open after detach: actor tree did not shut down"
    );
}

#[tokio::test]
async fn device_id_allocation_per_family() {
    let mock = MockBackend::new();
    let backend: Arc<dyn HidBackend> = Arc::new(mock.clone());

    let tmp = TempDir::new().expect("tempdir");
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));

    // Pre-populate the mock with two devices of the same family but
    // distinct (vid, pid, serial, path) tuples.
    mock.add_device(lcdv2_info("SN-A", "/mock/hidraw0"));
    mock.add_device(lcdv2_info("SN-B", "/mock/hidraw1"));

    let manager = DeviceManager::new(backend, store, fast_poll_config(), Arc::new(Stats::new()));
    let handle = manager.spawn().await;

    // Wait for both attachments to reconcile (poll tick + replay
    // settle for each device, attached sequentially in poll_once).
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut devs = handle.list().await;
    devs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    assert_eq!(devs.len(), 2, "expected two devices");
    assert_eq!(
        devs[0].id,
        DeviceId::new("galahad2-lcdv2:0001"),
        "first attach => :0001"
    );
    assert_eq!(
        devs[1].id,
        DeviceId::new("galahad2-lcdv2:0002"),
        "second attach => :0002"
    );

    // Distinct serials must round-trip through the descriptor.
    let serials: Vec<Option<String>> = devs.iter().map(|d| d.serial.clone()).collect();
    assert!(
        serials.contains(&Some("SN-A".to_string())) && serials.contains(&Some("SN-B".to_string())),
        "both serials present, got {:?}",
        serials
    );
}

/// P1D-15: replay-on-spawn re-issues the persisted fan PWM as the
/// first HID write after the actor tree comes up.
///
/// We pre-seed the stable VID/PID/serial settings key for the device.
/// On attach, `replay_persisted_settings` should issue exactly one
/// `SetFanPWM` (cmd 139) write through the hid_io actor; the
/// MockBackend records it in `write_history`.
#[tokio::test]
async fn replay_writes_fan_pwm_on_attach() {
    use llcontrol_protocol::galahad2_lcdv2::commands::Command;

    let mock = MockBackend::new();
    let backend: Arc<dyn HidBackend> = Arc::new(mock.clone());

    let tmp = TempDir::new().expect("tempdir");

    // SN-A UTF-8 bytes are hex encoded in the stable settings filename.
    // Hand-written TOML avoids depending on PersistenceStore::write.
    let devices_dir = tmp.path().join("devices");
    std::fs::create_dir_all(&devices_dir).expect("create devices dir");
    std::fs::write(
        devices_dir.join("0416-7395-serial-534e2d41.toml"),
        "schema_version = 1\n[fan]\npwm_percent = 75\n",
    )
    .expect("seed persistence file");

    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));

    let info = lcdv2_info("SN-A", "/mock/hidraw0");
    mock.add_device(info.clone());

    let manager = DeviceManager::new(backend, store, fast_poll_config(), Arc::new(Stats::new()));
    let _handle = manager.spawn().await;

    // 50 ms hot-plug poll + 50 ms replay settle + 50 ms post-fan-PWM
    // gap. 400 ms gives ample slack for tokio scheduling.
    tokio::time::sleep(Duration::from_millis(400)).await;

    let history = mock.write_history(&info);
    assert!(
        !history.is_empty(),
        "expected at least one write to the device after replay"
    );

    let cmd_byte = Command::SetFanPWM as u8;
    let fan_pwm_writes: Vec<&Vec<u8>> = history
        .iter()
        .filter(|w| w.len() >= 2 && w[1] == cmd_byte)
        .collect();
    assert!(
        !fan_pwm_writes.is_empty(),
        "expected a SetFanPWM (cmd {}) write in replay, got history: {:?}",
        cmd_byte,
        history
            .iter()
            .map(|w| w.get(1).copied())
            .collect::<Vec<_>>()
    );

    // The fan PWM payload format is `[group=0, percent]` starting at
    // offset 6 (after the 6-byte A-class header). Verify the percent
    // round-trips so we can be sure we replayed THIS device's TOML
    // and not some unrelated default value.
    let pkt = fan_pwm_writes[0];
    assert!(pkt.len() >= 8, "A-class packet must be >= 8 bytes");
    assert_eq!(pkt[6], 0, "fan group byte at offset 6 should be 0");
    assert_eq!(
        pkt[7], 75,
        "fan PWM percent byte at offset 7 should match persisted 75"
    );
}
