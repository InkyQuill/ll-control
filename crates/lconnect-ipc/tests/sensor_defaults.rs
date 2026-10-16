use lconnect_ipc::*;
fn reading(id: &str, unit: SensorUnit) -> SensorReading {
    SensorReading {
        id: id.into(),
        label: "identical display label".into(),
        unit,
        value: Some(42.0),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
fn config() -> SensorDefaults {
    SensorDefaults {
        cpu: CpuSensorDefaults {
            temperature: Some("cpu:physical-a:temp".into()),
            ..Default::default()
        },
        gpu: Some(GpuSensorDefaults {
            device_id: "gpu:physical-b".into(),
            temperature: None,
            clock: None,
            load: Some("gpu:b:load".into()),
            power: None,
            fan: None,
            voltage: None,
            memory_used: None,
        }),
        ..Default::default()
    }
}
#[test]
fn saved_choices_roundtrip_without_hardware_and_resolve_only_exact_identity() {
    let original = config();
    let saved = serde_json::to_vec(&original).unwrap();
    let restored: SensorDefaults = serde_json::from_slice(&saved).unwrap();
    restored.validate().unwrap();
    assert_eq!(original, restored);
    assert!(restored
        .resolve(DefaultSensorRole::CpuTemperature, &[])
        .is_none());
    let mut catalog = vec![
        reading("gpu:a:load", SensorUnit::Percent),
        reading("gpu:b:load", SensorUnit::Percent),
    ];
    assert_eq!(
        restored
            .resolve(DefaultSensorRole::GpuLoad, &catalog)
            .unwrap()
            .id,
        "gpu:b:load"
    );
    catalog.pop();
    assert!(restored
        .resolve(DefaultSensorRole::GpuLoad, &catalog)
        .is_none());
    catalog.push(reading("gpu:b:load", SensorUnit::Percent));
    assert!(restored
        .resolve(DefaultSensorRole::GpuLoad, &catalog)
        .is_some());
    // Explicit callers keep their own bindings; resolution never rewrites catalog.
    assert_eq!(catalog[0].id, "gpu:a:load");
}
#[test]
fn stale_invalid_mismatched_and_ambiguous_samples_never_become_defaults() {
    let cfg = config();
    let good = reading("cpu:physical-a:temp", SensorUnit::Celsius);
    for bad in [
        SensorReading {
            status: SensorStatus::Stale,
            ..good.clone()
        },
        SensorReading {
            age_ms: Some(3001),
            ..good.clone()
        },
        SensorReading {
            age_ms: None,
            ..good.clone()
        },
        SensorReading {
            value: None,
            ..good.clone()
        },
        SensorReading {
            value: Some(f64::NAN),
            ..good.clone()
        },
        SensorReading {
            unit: SensorUnit::Percent,
            ..good.clone()
        },
    ] {
        assert!(cfg
            .resolve(DefaultSensorRole::CpuTemperature, &[bad])
            .is_none());
    }
    assert!(cfg
        .resolve(
            DefaultSensorRole::CpuTemperature,
            &[good.clone(), good.clone()]
        )
        .is_none());
    assert!(cfg
        .resolve(DefaultSensorRole::CpuTemperature, &[good])
        .is_some());
}
#[test]
fn schema_rejects_unknown_fields_future_versions_and_invalid_identities() {
    let value = serde_json::to_value(config()).unwrap();
    for key in ["", "cpu", "gpu"] {
        let mut mutated = value.clone();
        let target = if key.is_empty() {
            &mut mutated
        } else {
            &mut mutated[key]
        };
        target["future"] = true.into();
        assert!(serde_json::from_value::<SensorDefaults>(mutated).is_err());
    }
    let mut cfg = config();
    cfg.version = 2;
    assert!(cfg.validate().is_err());
    cfg.version = 1;
    for id in [String::new(), "a\nb".into(), "x".repeat(1025)] {
        cfg.cpu.temperature = Some(id);
        assert!(cfg.validate().is_err());
    }
    let cfg = SensorDefaults::default();
    cfg.validate().unwrap();
    assert!(DefaultSensorRole::ALL
        .iter()
        .all(|role| cfg.binding(*role).is_none()));
}
