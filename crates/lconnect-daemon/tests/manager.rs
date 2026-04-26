//! Integration tests for the device manager (P1D-10).
//!
//! Wires a `MockBackend` into a `DeviceManager`, then drives the
//! 50 ms hot-plug poll by mutating the mock device list and waiting
//! for the next reconciliation.

use lconnect_daemon::config::Config;
use lconnect_daemon::manager::DeviceManager;
use lconnect_daemon::persistence::PersistenceStore;
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use lconnect_ipc::DeviceId;
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

    let manager = DeviceManager::new(backend, store, fast_poll_config());
    let handle = manager.spawn().await;

    // Initially: no devices.
    assert!(handle.list().await.is_empty(), "manager starts empty");

    // Attach one device. The poll loop is on a 50 ms tick, so giving
    // ourselves 200 ms of slack covers tokio scheduling jitter on
    // loaded CI runners.
    mock.add_device(lcdv2_info("SN-A", "/mock/hidraw0"));
    tokio::time::sleep(Duration::from_millis(200)).await;

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

    let manager = DeviceManager::new(backend, store, fast_poll_config());
    let handle = manager.spawn().await;

    // Wait for the attach to be observed.
    tokio::time::sleep(Duration::from_millis(200)).await;
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

    let manager = DeviceManager::new(backend, store, fast_poll_config());
    let handle = manager.spawn().await;

    // Wait for both attachments to reconcile.
    tokio::time::sleep(Duration::from_millis(200)).await;

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
