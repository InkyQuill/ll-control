//! Opt-in owned mode-bank fixture, never used by the production daemon.
use lconnect_daemon::persistence::{settings_key, PersistenceStore};
use lconnect_hid::DeviceInfo;
use lconnect_ipc::{
    CoolingBankSource, CoolingModeBanks, CoolingModeProfile, CoolingPolicy, FanPreset, PumpPreset,
};
use std::path::Path;

pub fn seed(root: &Path, store: &PersistenceStore, info: &DeviceInfo) -> anyhow::Result<()> {
    if std::env::var("LCONNECT_NATIVE_COOLING_MODES").as_deref() != Ok("1") {
        return Ok(());
    }
    if std::env::var("LCONNECT_NATIVE_COOLING_DEFAULTS").as_deref() == Ok("1") {
        // Fresh-bank acceptance must use the production initializer in the GUI.
        // Never seed/recreate modes, including after restart.
        return Ok(());
    }
    let marker = root.join("cooling-modes-seeded");
    // Never recreate lost banks after a restart: the acceptance must detect loss.
    if marker.exists() {
        return Ok(());
    }
    let key = settings_key(info);
    let mut settings = store.read(&key)?;
    if settings.cooling_banks.is_none() {
        let mut fan = Vec::new();
        for (id, preset) in [
            ("Quiet", FanPreset::Quiet),
            ("StandardSpeed", FanPreset::Standard),
            ("HighSpeed", FanPreset::High),
            ("FullSpeed", FanPreset::Full),
        ] {
            fan.push(CoolingModeProfile {
                use_default_name: false,
                id: id.into(),
                name: format!("Native {id}"),
                policy: preset.policy("fixture:cpu".into(), 0.0, 100),
            });
        }
        for i in 1..=4 {
            fan.push(CoolingModeProfile {
                use_default_name: false,
                id: format!("UserDefine{i}"),
                name: format!("Native user {i}"),
                policy: FanPreset::Quiet.policy("fixture:cpu".into(), 0.0, 97),
            });
        }
        settings.cooling_banks = Some(CoolingModeBanks {
            version: 1,
            fan,
            pump: vec![
                CoolingModeProfile {
                    use_default_name: false,
                    id: "FixRPM".into(),
                    name: "Native fixed RPM".into(),
                    policy: CoolingPolicy::FixedRpm { rpm: 2300 },
                },
                CoolingModeProfile {
                    use_default_name: false,
                    id: "PWMQuiet".into(),
                    name: "Native Quiet".into(),
                    policy: PumpPreset::Quiet.policy("fixture:cpu".into(), 0.0, 97),
                },
                CoolingModeProfile {
                    use_default_name: false,
                    id: "PWMPerformance".into(),
                    name: "Native Performance".into(),
                    policy: PumpPreset::Performance.policy("fixture:cpu".into(), 0.0, 97),
                },
            ],
            source: Some(CoolingBankSource::Windows2128 {
                fan_json: r#"{"OwnedFixture":null}"#.into(),
                pump_json: "{}".into(),
            }),
        });
        store.write(&key, &settings)?;
    }
    std::fs::write(marker, b"Owned mock modes seeded once\n")?;
    Ok(())
}
