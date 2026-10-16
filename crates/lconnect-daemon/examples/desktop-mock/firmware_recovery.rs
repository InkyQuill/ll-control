//! Saved-file fixtures only. No transfer, query, activation or real HID backend.
use anyhow::{ensure, Result};
use lconnect_daemon::{
    actor::firmware_update::CoolingHold,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
};
use lconnect_hid::DeviceInfo;
use lconnect_ipc::{DisplaySettings, InterfaceLanguage};
use std::path::Path;

pub async fn seed(root: &Path, store: &PersistenceStore, info: &DeviceInfo) -> Result<()> {
    let Ok(mode) = std::env::var("LCONNECT_NATIVE_RECOVERY_STATE") else {
        return Ok(());
    };
    ensure!(
        matches!(
            mode.as_str(),
            "none" | "guarded" | "guard_only" | "unreadable"
        ),
        "unknown private recovery fixture"
    );
    let language = if std::env::var("LCONNECT_NATIVE_RECOVERY_RUSSIAN").as_deref() == Ok("1") {
        InterfaceLanguage::Russian
    } else {
        InterfaceLanguage::English
    };
    store
        .display_settings()
        .save(DisplaySettings {
            interface_language: language,
            ..Default::default()
        })
        .await?;
    if matches!(mode.as_str(), "guarded" | "guard_only") {
        let settings = DeviceSettings::default();
        store.write(&settings_key(info), &settings)?;
        let mut armed = store
            .firmware_operations()
            .arm(info, CoolingHold::PumpOnly, settings)?;
        armed.effects_possible()?;
        drop(armed);
    }
    let path = root.join("settings/firmware-operation.json");
    match mode.as_str() {
        "guard_only" => std::fs::remove_file(path)?,
        "unreadable" => std::fs::write(path, b"{invalid private fixture")?,
        _ => (),
    }
    if std::env::var("LCONNECT_NATIVE_RECOVERY_SEALED").as_deref() == Ok("1") {
        store.admission().close();
    }
    Ok(())
}
