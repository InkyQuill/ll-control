use lconnect_ipc::{CoolingBankSource, CoolingModeBanks, CoolingModeProfile, CoolingPolicy};

fn banks() -> CoolingModeBanks {
    CoolingModeBanks {
        version: 1,
        fan: vec![CoolingModeProfile {
            use_default_name: false,
            id: "Quiet".into(),
            name: "Saved fan".into(),
            policy: CoolingPolicy::Fixed { percent: 50 },
        }],
        pump: vec![CoolingModeProfile {
            use_default_name: false,
            id: "FixRPM".into(),
            name: "Saved pump".into(),
            policy: CoolingPolicy::FixedRpm { rpm: 2200 },
        }],
        source: None,
    }
}

#[test]
fn inactive_policies_names_duplicates_and_future_versions_are_validated() {
    let valid = banks();
    valid.validate().unwrap();
    let mut bad = valid.clone();
    bad.fan.push(bad.fan[0].clone());
    assert!(bad.validate().is_err());
    let mut bad = valid.clone();
    bad.version = 2;
    assert!(bad.validate().is_err());
    let mut bad = valid.clone();
    bad.pump[0].policy = CoolingPolicy::FixedRpm { rpm: 799 };
    assert!(bad.validate().is_err());
    let mut bad = valid.clone();
    bad.fan[0].policy = CoolingPolicy::FixedRpm { rpm: 2200 };
    assert!(bad.validate().is_err());
    for (id, name) in [
        ("../Quiet".into(), "Saved".into()),
        ("Quiet".into(), "\n".into()),
        ("Quiet".into(), "x".repeat(129)),
    ] {
        let mut bad = valid.clone();
        bad.fan[0].id = id;
        bad.fan[0].name = name;
        assert!(bad.validate().is_err());
    }
    let mut bad = valid;
    bad.fan = (0..17)
        .map(|i| CoolingModeProfile {
            use_default_name: false,
            id: format!("Mode{i}"),
            name: "Saved".into(),
            policy: CoolingPolicy::Fixed { percent: 50 },
        })
        .collect();
    assert!(bad.validate().is_err());
}

#[test]
fn historical_metadata_is_bounded_object_json_and_future_formats_are_rejected() {
    let mut valid = banks();
    valid.source = Some(CoolingBankSource::Windows2128 {
        fan_json: r#"{"Name":null}"#.into(),
        pump_json: "{}".into(),
    });
    valid.validate().unwrap();
    for value in [
        "[]".into(),
        "broken".into(),
        format!("{{\"padding\":\"{}\"}}", "x".repeat(64 * 1024)),
    ] {
        let mut bad = valid.clone();
        bad.source = Some(CoolingBankSource::Windows2128 {
            fan_json: value,
            pump_json: "{}".into(),
        });
        assert!(bad.validate().is_err());
    }
    let mut json = serde_json::to_value(valid).unwrap();
    json["source"]["format"] = "future_format".into();
    assert!(serde_json::from_value::<CoolingModeBanks>(json).is_err());
}

#[test]
fn factory_modes_keep_reference_catalog_endpoints_fixed_rpm_and_explicit_default_names() {
    use lconnect_ipc::{CoolingTarget, FanPreset, PumpPreset};
    let modes = lconnect_ipc::lcdv2_default_cooling_modes("cpu:chosen").unwrap();
    assert_eq!(
        modes.fan.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        [
            "Quiet",
            "StandardSpeed",
            "HighSpeed",
            "FullSpeed",
            "UserDefine1",
            "UserDefine2",
            "UserDefine3",
            "UserDefine4"
        ]
    );
    assert_eq!(
        modes.pump.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["FixRPM", "PWMPerformance", "PWMQuiet"]
    );
    for (m, p) in modes.fan.iter().zip([
        FanPreset::Quiet,
        FanPreset::Standard,
        FanPreset::High,
        FanPreset::Full,
        FanPreset::Quiet,
        FanPreset::Quiet,
        FanPreset::Quiet,
        FanPreset::Quiet,
    ]) {
        assert_eq!(m.policy, p.policy("cpu:chosen".into(), 0.0, 100));
        assert!(m.use_default_name);
    }
    assert_eq!(modes.pump[0].policy, CoolingPolicy::FixedRpm { rpm: 2200 });
    assert_eq!(
        modes.pump[1].policy,
        PumpPreset::Performance.policy("cpu:chosen".into(), 0.0, 100)
    );
    assert_eq!(
        modes.pump[2].policy,
        PumpPreset::Quiet.policy("cpu:chosen".into(), 0.0, 100)
    );
    assert!(modes.source.is_none());
    for m in &modes.pump {
        assert!(m.use_default_name);
        assert_eq!(
            lconnect_ipc::lcdv2_default_mode_name(&m.id, CoolingTarget::Pump),
            Some(m.name.as_str())
        );
    }
    let mut named = modes.clone();
    named.pump[0].name = "My pump".into();
    assert!(named.validate().is_err());
    named.pump[0].use_default_name = false;
    named.validate().unwrap();
    assert!(
        serde_json::to_value(&named.pump[0])
            .unwrap()
            .get("use_default_name")
            .is_none(),
        "literal names keep the legacy portable shape"
    );
    for sensor in ["", " ", "cpu\0x", &"x".repeat(4097)] {
        assert!(lconnect_ipc::lcdv2_default_cooling_modes(sensor).is_err());
    }
}

#[test]
fn individual_factory_reset_requires_binding_only_for_curves_and_rejects_wrong_target() {
    use lconnect_ipc::{lcdv2_default_cooling_mode, CoolingTarget};
    let fixed = lcdv2_default_cooling_mode(CoolingTarget::Pump, "FixRPM", "").unwrap();
    assert_eq!(fixed.policy, CoolingPolicy::FixedRpm { rpm: 2200 });
    assert!(fixed.use_default_name);
    for (target, id) in [
        (CoolingTarget::Fan, "StandardSpeed"),
        (CoolingTarget::Pump, "PWMQuiet"),
    ] {
        assert!(lcdv2_default_cooling_mode(target, id, "").is_err());
        let expected = lconnect_ipc::lcdv2_default_cooling_modes("cpu:chosen").unwrap();
        let modes = if target == CoolingTarget::Fan {
            expected.fan
        } else {
            expected.pump
        };
        assert_eq!(
            lcdv2_default_cooling_mode(target, id, "cpu:chosen").unwrap(),
            modes.into_iter().find(|mode| mode.id == id).unwrap()
        );
    }
    for (target, id) in [
        (CoolingTarget::Fan, "FixRPM"),
        (CoolingTarget::Pump, "StandardSpeed"),
        (CoolingTarget::Fan, "unknown"),
    ] {
        assert!(lcdv2_default_cooling_mode(target, id, "cpu:chosen").is_err());
    }
}
