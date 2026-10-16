use lconnect_ipc::*;
#[test]
fn installed_js_modes_and_canvas_scaled_population_match() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/snow3d-2.1.28-parameters.json")).unwrap();
    for row in rows {
        let config = Snow3dEffect {
            intensity: row["intensity"].as_u64().unwrap() as u8,
            seed: 42,
        };
        let p = config.parameters().unwrap();
        assert_eq!(
            u64::from(p.exported_speed),
            row["exported_speed"].as_u64().unwrap()
        );
        assert_eq!(u64::from(p.speed), row["speed"].as_u64().unwrap());
        assert_eq!(
            u64::from(p.particle_level),
            row["particle_level"].as_u64().unwrap()
        );
        assert_eq!(u64::from(p.particles), row["particles"].as_u64().unwrap());
    }
    for intensity in [0, 4, 255] {
        assert!(Snow3dEffect {
            intensity,
            seed: 42
        }
        .validate()
        .is_err());
    }
    assert!(
        serde_json::from_str::<Snow3dEffect>(r#"{"intensity":1,"seed":42,"dynamic":true}"#)
            .is_err()
    );
}
#[test]
fn snow3d_theme_and_cached_page_are_independent_and_only_active_scene_streams() {
    let mut theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Snow3D","background":[0,0,0],"layers":[{"kind":"snow3d","effect":{"intensity":1,"seed":42}}]})).unwrap();
    theme.validate().unwrap();
    assert!(theme.requires_streaming());
    assert_eq!(theme.page_kind(), LcdPageKind::Snow3d);
    let basic: LcdTheme = serde_json::from_value(
        serde_json::json!({"version":1,"name":"Basic","background":[0,0,0],"layers":[]}),
    )
    .unwrap();
    let mut root = basic;
    root.page_cache.push(SavedLcdPage {
        page: LcdPageKind::Snow3d,
        theme: Box::new(theme.clone()),
    });
    root.validate().unwrap();
    assert!(!root.requires_streaming());
    let profile = ProfileContent::Screen(root);
    assert_eq!(
        serde_json::from_slice::<ProfileContent>(&serde_json::to_vec(&profile).unwrap()).unwrap(),
        profile
    );
    theme.layers.push(ThemeLayer::Snow3d {
        effect: Snow3dEffect::default(),
    });
    assert!(theme.validate().is_err());
}
