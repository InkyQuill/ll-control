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

#[tokio::test]
async fn independent_deltas_preserve_siblings_and_shutdown_drains_queue() {
    use command_dispatch::SettingsUpdate;
    use lconnect_daemon::persistence::{FanSection, LcdSection, LightingState, PumpSection};
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let (tx, rx) = mpsc::channel(16);
    let (stop, shutdown) = watch::channel(false);
    let task = command_dispatch::spawn_updates(
        store.clone(),
        KEY.into(),
        DeviceSettings::default(),
        rx,
        shutdown,
    );
    tx.send(SettingsUpdate::Lcd(LcdSection {
        brightness: 73,
        rotation: "90".into(),
        control_mode: "LocalUI".into(),
    }))
    .await
    .unwrap();
    tx.send(SettingsUpdate::Fan(FanSection {
        pwm_percent: 40,
        ..Default::default()
    }))
    .await
    .unwrap();
    tx.send(SettingsUpdate::Pump(PumpSection {
        pwm_percent: 60,
        source: "mcu".into(),
    }))
    .await
    .unwrap();
    tx.send(SettingsUpdate::LightingFan(LightingState {
        mode: "static".into(),
        ..Default::default()
    }))
    .await
    .unwrap();
    stop.send(true).unwrap();
    task.await.unwrap();
    let settings = store.read(KEY).unwrap();
    assert_eq!(settings.lcd.brightness, 73);
    assert_eq!(settings.lcd.rotation, "90");
    assert_eq!(settings.fan.pwm_percent, 40);
    assert_eq!(settings.pump.pwm_percent, 60);
    assert_eq!(settings.lighting.fan.unwrap().mode, "static");
}

#[tokio::test]
async fn failed_debounced_write_retries_without_another_user_update() {
    use command_dispatch::SettingsUpdate;
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
    // A file at the directory path reliably fails even when tests run as root.
    std::fs::write(tmp.path().join("devices"), "blocked").unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, shutdown_rx) = watch::channel(false);
    let (handle, mut status) = command_dispatch::spawn_updates_observed(
        store.clone(),
        KEY.into(),
        DeviceSettings::default(),
        rx,
        shutdown_rx,
    );
    tx.send(SettingsUpdate::Replace(Box::new(settings_with_fan_pwm(67))))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            status.changed().await.unwrap();
            let state = status.borrow().clone();
            if state.last_error.is_some() {
                assert!(state.pending);
                assert!(!state.saved);
                break;
            }
        }
    })
    .await
    .unwrap();
    std::fs::remove_file(tmp.path().join("devices")).unwrap();
    // Rename makes the file readable before parent fsync/status publication.
    // Observe the durable acknowledgement, then verify the retried intent.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = status.borrow().clone();
            if !state.pending && state.saved && state.last_error.is_none() {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .expect("failed intent should durably retry without another update");
    assert_eq!(store.read(KEY).unwrap().fan.pwm_percent, 67);
    drop(tx);
    handle.await.unwrap();
}

#[tokio::test]
async fn shutdown_flushes_failed_pending_intent_and_keeps_latest_independent_delta() {
    use command_dispatch::SettingsUpdate;
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
    std::fs::write(tmp.path().join("devices"), "blocked").unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (shutdown, shutdown_rx) = watch::channel(false);
    let handle = command_dispatch::spawn_updates(
        store.clone(),
        KEY.into(),
        DeviceSettings::default(),
        rx,
        shutdown_rx,
    );
    tx.send(SettingsUpdate::Replace(Box::new(settings_with_fan_pwm(67))))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    tx.send(SettingsUpdate::Lcd(
        lconnect_daemon::persistence::LcdSection {
            brightness: 44,
            ..Default::default()
        },
    ))
    .await
    .unwrap();
    std::fs::remove_file(tmp.path().join("devices")).unwrap();
    shutdown.send(true).unwrap();
    handle.await.unwrap();
    let saved = store.read(KEY).unwrap();
    assert_eq!(saved.fan.pwm_percent, 67);
    assert_eq!(saved.lcd.brightness, 44);
}

#[tokio::test(start_paused = true)]
async fn media_pin_survives_queued_update_failed_write_retry_and_final_clear() {
    use command_dispatch::SettingsUpdate;
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let assets = store.media_assets();
    let clip = lconnect_ipc::H264Clip {
        fps: 24,
        units: vec![lconnect_ipc::H264AccessUnit {
            bytes: vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1],
            keyframe: true,
        }],
    };
    let saved = assets.import("Pending".into(), clip).unwrap();
    let (_, pin) = assets.acquire(&saved.key).unwrap();
    let (tx, rx) = mpsc::channel(8);
    tx.send(SettingsUpdate::MediaClip {
        key: saved.key.clone(),
        pin: Some(pin),
    })
    .await
    .unwrap();
    // Before the actor starts, the queue itself owns the file protection.
    assert!(assets
        .delete_unreferenced(&saved.key, || Ok(false))
        .is_err());
    std::fs::write(tmp.path().join("devices"), b"block persistence").unwrap();
    let (_stop, stop) = watch::channel(false);
    let (task, mut status) = command_dispatch::spawn_updates_observed(
        store.clone(),
        KEY.into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if status.borrow_and_update().last_error.is_some() {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(assets
        .delete_unreferenced(&saved.key, || Ok(false))
        .is_err());
    std::fs::remove_file(tmp.path().join("devices")).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if status.borrow_and_update().saved {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(
        store.read(KEY).unwrap().media_clip.as_ref(),
        Some(&saved.key)
    );
    tx.send(SettingsUpdate::Theme(None)).await.unwrap();
    status.changed().await.unwrap();
    assert!(status.borrow().pending);
    // Clear releases the in-memory pin, but the old on-disk selection still wins
    // until the clear itself is committed.
    assert!(assets
        .delete_unreferenced(&saved.key, || Ok(store
            .referenced_media_clips()?
            .contains(&saved.key)))
        .is_err());
    drop(tx);
    task.await.unwrap();
    assert!(store.read(KEY).unwrap().media_clip.is_none());
    assets
        .delete_unreferenced(&saved.key, || {
            Ok(store.referenced_media_clips()?.contains(&saved.key))
        })
        .unwrap();
}
