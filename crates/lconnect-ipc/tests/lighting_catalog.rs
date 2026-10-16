use lconnect_ipc::{lighting_catalog, lighting_color_count, LightingKind};
use serde_json::Value;
#[test]
fn windows_2_1_28_controls_defaults_and_palettes_match_numeric_inventory() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/lighting-2.1.28-controls.json")).unwrap();
    for (kind, label) in [(LightingKind::Fan, "fan"), (LightingKind::Pump, "pump")] {
        let catalog = lighting_catalog(kind);
        let rows = fixture["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["kind"] == label)
            .collect::<Vec<_>>();
        assert_eq!(catalog.len(), rows.len());
        for (control, row) in catalog.iter().zip(rows) {
            assert_eq!(control.mode, row["mode"]);
            let colors = row["colors"].as_array().unwrap();
            assert_eq!(control.color_count, colors.len());
            assert_eq!(
                lighting_color_count(kind, &control.mode),
                Some(colors.len())
            );
            let cfg = &control.defaults;
            let actual = std::iter::once(&cfg.color)
                .chain(&cfg.additional_colors)
                .take(control.color_count)
                .map(|c| serde_json::json!([c.r, c.g, c.b]))
                .collect::<Vec<_>>();
            assert_eq!(actual, *colors, "{} {} colors", label, control.mode);
            assert_eq!(control.speed, !row["speed"].is_null());
            assert_eq!(cfg.speed_percent, row["speed"].as_u64().unwrap_or(0) as u8);
            assert!(control.brightness);
            assert_eq!(cfg.brightness_percent, row["brightness"]);
            assert_eq!(control.direction, row["direction"]);
            assert_eq!(cfg.direction, "forward");
            assert!(!cfg.disabled);
            assert_eq!(
                cfg.scope.as_deref(),
                if kind == LightingKind::Pump {
                    Some("inner")
                } else {
                    None
                }
            );
        }
    }
}

#[test]
fn native_fan_topology_boundaries_sku_mapping_and_retained_manual_count() {
    use lconnect_ipc::{FanLedSize, FanLightingTopology};
    for count in [8, 24, 50] {
        let mut topology = FanLightingTopology {
            manual_led_count: count,
            ..Default::default()
        };
        topology.validate().unwrap();
        assert_eq!(topology.effective_led_count(), count);
        topology.sync_to_pump = true;
        topology.size = FanLedSize::Mm280;
        assert_eq!(topology.effective_led_count(), 16);
        topology.size = FanLedSize::Mm360;
        assert_eq!(topology.effective_led_count(), 24);
        topology.sync_to_pump = false;
        assert_eq!(topology.effective_led_count(), count);
    }
    for count in [0, 7, 51, 255] {
        assert!(FanLightingTopology {
            manual_led_count: count,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
    assert!(serde_json::from_str::<FanLightingTopology>(
        r#"{"sync_to_pump":true,"size":"420","manual_led_count":24}"#
    )
    .is_err());
}
