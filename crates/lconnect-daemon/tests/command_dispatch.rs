//! Integration tests for the `command_dispatch` actor (P1D-9).
//!
//! Verifies the 250 ms debounce, burst coalescing, and shutdown-flush
//! semantics. Each test uses a fresh `tempfile::TempDir` so disk state
//! is scoped.

use lconnect_daemon::actor::command_dispatch;
use lconnect_daemon::persistence::{DeviceSettings, PersistenceStore};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{mpsc, watch};

const KEY: &str = "galahad2-lcdv2:0001";

/// Build a [`DeviceSettings`] whose only distinguishing field is the
/// fan PWM percent -- handy for asserting that the on-disk value
/// matches a particular update in the burst.
fn settings_with_fan_pwm(pwm: u8) -> DeviceSettings {
    let mut s = DeviceSettings::default();
    s.fan.pwm_percent = pwm;
    s
}

#[tokio::test]
async fn single_update_writes_after_debounce() {
    let tmp = TempDir::new().expect("tempdir");
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));

    let (settings_tx, settings_rx) = mpsc::channel::<DeviceSettings>(8);
    let (_sd_tx, sd_rx) = watch::channel(false);

    let handle = command_dispatch::spawn(
        store.clone(),
        KEY.to_string(),
        DeviceSettings::default(),
        settings_rx,
        sd_rx,
    );

    settings_tx
        .send(settings_with_fan_pwm(42))
        .await
        .expect("send update");

    // Debounce is 250 ms; sleep a bit longer so the write is observable.
    tokio::time::sleep(Duration::from_millis(350)).await;

    let on_disk = store.read(KEY).expect("read after debounce");
    assert_eq!(
        on_disk.fan.pwm_percent, 42,
        "expected debounced write to land within 350 ms"
    );

    drop(settings_tx);
    handle.await.expect("actor join");
}

#[tokio::test]
async fn burst_coalesces_to_one_write() {
    let tmp = TempDir::new().expect("tempdir");
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));

    let (settings_tx, settings_rx) = mpsc::channel::<DeviceSettings>(32);
    let (_sd_tx, sd_rx) = watch::channel(false);

    let handle = command_dispatch::spawn(
        store.clone(),
        KEY.to_string(),
        DeviceSettings::default(),
        settings_rx,
        sd_rx,
    );

    // Send 5 updates back-to-back. Each `recv()` resets the debounce
    // sleep arm, so only the LAST value should ever hit disk.
    for pwm in [10u8, 20, 30, 40, 50] {
        settings_tx
            .send(settings_with_fan_pwm(pwm))
            .await
            .expect("send update");
    }

    // No file should exist yet -- the debounce hasn't elapsed and we
    // sent the burst much faster than 250 ms.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mid = store.read(KEY).expect("read mid-burst");
    assert_eq!(
        mid.fan.pwm_percent, 0,
        "no write should land before the debounce window expires"
    );

    // Wait out the debounce window.
    tokio::time::sleep(Duration::from_millis(350)).await;

    let on_disk = store.read(KEY).expect("read after debounce");
    assert_eq!(
        on_disk.fan.pwm_percent, 50,
        "expected only the last update in the burst to be persisted"
    );

    drop(settings_tx);
    handle.await.expect("actor join");
}

#[tokio::test]
async fn shutdown_flushes_pending_update() {
    let tmp = TempDir::new().expect("tempdir");
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));

    let (settings_tx, settings_rx) = mpsc::channel::<DeviceSettings>(8);
    let (sd_tx, sd_rx) = watch::channel(false);

    let handle = command_dispatch::spawn(
        store.clone(),
        KEY.to_string(),
        DeviceSettings::default(),
        settings_rx,
        sd_rx,
    );

    settings_tx
        .send(settings_with_fan_pwm(77))
        .await
        .expect("send update");

    // Yield once so the actor task observes the message before we flip
    // shutdown -- otherwise the watch::changed signal could win the
    // race and the dirty bit would never be set.
    tokio::task::yield_now().await;

    // Trip shutdown well before the 250 ms debounce window elapses;
    // the flush-on-shutdown path is what we are exercising.
    sd_tx.send(true).expect("send shutdown");

    handle.await.expect("actor join");

    let on_disk = store.read(KEY).expect("read after shutdown flush");
    assert_eq!(
        on_disk.fan.pwm_percent, 77,
        "expected pending update to be flushed on shutdown"
    );

    drop(settings_tx);
}
