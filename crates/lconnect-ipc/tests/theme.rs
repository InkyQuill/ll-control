use lconnect_ipc::*;

fn reading() -> SensorReading {
    SensorReading {
        id: "cpu".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        value: Some(42.5),
        status: SensorStatus::Valid,
        age_ms: Some(1),
        observed_unix_ms: Some(1),
    }
}
fn show(r: SensorReading) -> String {
    theme_sensor_text(
        &[r],
        "cpu",
        SensorUnit::Celsius,
        TemperatureUnit::Fahrenheit,
        1,
    )
}
#[test]
fn sensor_value_conversion_and_failure_never_show_fake_zero() {
    assert_eq!(show(reading()), "108.5°F");
    for status in [
        SensorStatus::Stale,
        SensorStatus::Invalid,
        SensorStatus::Unavailable,
    ] {
        assert_eq!(
            show(SensorReading {
                status,
                ..reading()
            }),
            "—"
        );
    }
    for value in [None, Some(f64::NAN), Some(f64::INFINITY)] {
        assert_eq!(show(SensorReading { value, ..reading() }), "—");
    }
    for age_ms in [None, Some(3001)] {
        assert_eq!(
            show(SensorReading {
                age_ms,
                ..reading()
            }),
            "—"
        );
    }
    assert_eq!(
        show(SensorReading {
            id: "other-cpu".into(),
            ..reading()
        }),
        "—"
    );
    assert_eq!(
        show(SensorReading {
            unit: SensorUnit::Percent,
            ..reading()
        }),
        "—"
    );
}
#[test]
fn image_theme_roundtrip_rejects_traversal_and_oversized_rectangles() {
    let mut t = LcdTheme {
        page_cache: Vec::new(),
        basic_editor: None,
        version: 1,
        name: "Моя тема".into(),
        background: [1, 2, 3],
        background_image: None,
        background_video: None,
        background_fill: None,
        layers: vec![ThemeLayer::Image {
            watermark_position: None,
            shadow: None,
            asset: "image-1".into(),
            x: 0,
            y: 0,
            width: 480,
            height: 480,
            fit: ImageFit::Crop,
            opacity: 255,
        }],
    };
    t.validate().unwrap();
    let json = serde_json::to_string(&t).unwrap();
    assert_eq!(serde_json::from_str::<LcdTheme>(&json).unwrap(), t);
    if let ThemeLayer::Image { asset, .. } = &mut t.layers[0] {
        *asset = "../secret".into();
    }
    assert!(t.validate().is_err());
    if let ThemeLayer::Image {
        asset, x, width, ..
    } = &mut t.layers[0]
    {
        *asset = "ok".into();
        *x = u16::MAX;
        *width = u16::MAX;
    }
    assert!(t.validate().is_err());
}
#[test]
fn future_versions_and_unknown_fields_cannot_silently_change_saved_theme() {
    let mut t = LcdTheme {
        page_cache: Vec::new(),
        basic_editor: None,
        version: 2,
        name: "Future".into(),
        background: [0; 3],
        background_image: None,
        background_video: None,
        background_fill: None,
        layers: vec![],
    };
    assert!(t.validate().is_err());
    t.version = 1;
    let mut value = serde_json::to_value(t).unwrap();
    value["unknown"] = true.into();
    assert!(serde_json::from_value::<LcdTheme>(value).is_err());
}

#[test]
fn power_reading_formats_watts_and_never_temperature_converts_or_reuses_stale_value() {
    let mut rows = vec![lconnect_ipc::SensorReading {
        id: "powercap:rapl:package-0:power".into(),
        label: "CPU power".into(),
        unit: lconnect_ipc::SensorUnit::Watts,
        value: Some(52.25),
        status: lconnect_ipc::SensorStatus::Valid,
        age_ms: Some(0),
        observed_unix_ms: Some(1),
    }];
    let format = |rows: &[lconnect_ipc::SensorReading]| {
        lconnect_ipc::theme_sensor_text(
            rows,
            "powercap:rapl:package-0:power",
            lconnect_ipc::SensorUnit::Watts,
            lconnect_ipc::TemperatureUnit::Fahrenheit,
            1,
        )
    };
    assert_eq!(format(&rows), "52.2 W");
    rows[0].age_ms = Some(3001);
    assert_eq!(format(&rows), "—");
    let encoded = serde_json::to_value(&rows[0]).unwrap();
    assert_eq!(encoded["unit"], "watts");
    assert_eq!(
        serde_json::from_value::<lconnect_ipc::SensorReading>(encoded).unwrap(),
        rows[0]
    );
}

#[test]
fn legacy_text_style_keeps_default_appearance_and_serialized_shape() {
    let original =
        serde_json::json!({"x":240,"y":100,"size":40,"color":[255,255,255],"align":"center"});
    let style: lconnect_ipc::TextStyle = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(style.font, lconnect_ipc::ThemeFont::Sans);
    assert_eq!(style.opacity, 100);
    assert!(style.shadow.is_none());
    assert_eq!(serde_json::to_value(style).unwrap(), original);
}

#[test]
fn image_shadow_is_optional_bounded_and_round_trips() {
    let original = serde_json::json!({"version":1,"name":"watermark","background":[0,0,0],"layers":[
        {"kind":"image","asset":"logo","x":0,"y":0,"width":480,"height":480,"fit":"contain","opacity":255}
    ]});
    let old: LcdTheme = serde_json::from_value(original.clone()).unwrap();
    old.validate().unwrap();
    assert_eq!(serde_json::to_value(&old).unwrap(), original);
    let mut new = original;
    new["layers"][0]["shadow"] =
        serde_json::json!({"x":-32,"y":32,"blur":16,"color":[20,30,40],"opacity":100});
    let theme: LcdTheme = serde_json::from_value(new.clone()).unwrap();
    theme.validate().unwrap();
    assert_eq!(serde_json::to_value(theme).unwrap(), new);
    for (key, value) in [("x", -33), ("y", 33), ("blur", 17), ("opacity", 101)] {
        let mut invalid = new.clone();
        invalid["layers"][0]["shadow"][key] = value.into();
        assert!(serde_json::from_value::<LcdTheme>(invalid)
            .unwrap()
            .validate()
            .is_err());
    }
}

#[test]
fn clock_parts_preserve_legacy_json_and_reject_unknown_format() {
    let value = serde_json::json!({"version":1,"name":"Clock","background":[0,0,0],"layers":[
        {"kind":"clock","twenty_four_hour":true,"show_seconds":false,"style":{"x":240,"y":200,"size":32,"color":[255,255,255],"align":"center"}}
    ]});
    let old: LcdTheme = serde_json::from_value(value.clone()).unwrap();
    old.validate().unwrap();
    assert_eq!(serde_json::to_value(old).unwrap(), value);
    for format in ["date", "period", "time_without_period"] {
        let mut new = value.clone();
        new["layers"][0]["format"] = format.into();
        let parsed: LcdTheme = serde_json::from_value(new.clone()).unwrap();
        parsed.validate().unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), new);
    }
    let mut invalid = value;
    invalid["layers"][0]["format"] = "arbitrary-format".into();
    assert!(serde_json::from_value::<LcdTheme>(invalid).is_err());
}

#[test]
fn watermark_position_is_bounded_and_does_not_expand_render_allocations() {
    let mut value = serde_json::json!({"version":1,"name":"Watermark","background":[0,0,0],"layers":[
        {"kind":"image","asset":"logo","x":0,"y":0,"width":480,"height":480,"fit":"contain","opacity":255,"watermark_position":{"horizontal":100,"vertical":0}}
    ]});
    let valid: LcdTheme = serde_json::from_value(value.clone()).unwrap();
    valid.validate().unwrap();
    assert_eq!(serde_json::to_value(valid).unwrap(), value);
    value["layers"][0]["width"] = 481.into();
    assert!(serde_json::from_value::<LcdTheme>(value.clone())
        .unwrap()
        .validate()
        .is_err());
    value["layers"][0]["width"] = 480.into();
    value["layers"][0]["watermark_position"]["horizontal"] = 101.into();
    assert!(serde_json::from_value::<LcdTheme>(value)
        .unwrap()
        .validate()
        .is_err());
}

#[test]
fn sensor_parts_validate_nested_styles_and_keep_legacy_shape() {
    let style =
        serde_json::json!({"x":240,"y":200,"size":32,"color":[255,255,255],"align":"center"});
    let old = serde_json::json!({"version":1,"name":"sensor","background":[0,0,0],"layers":[{"kind":"sensor","sensor_id":"cpu","label":"CPU","unit":"celsius","temperature_unit":"celsius","decimals":1,"style":style.clone()}]});
    let parsed: LcdTheme = serde_json::from_value(old.clone()).unwrap();
    parsed.validate().unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), old);
    let mut value = old;
    value["layers"][0]["parts"] = serde_json::json!({"label":style.clone(),"unit":style});
    serde_json::from_value::<LcdTheme>(value.clone())
        .unwrap()
        .validate()
        .unwrap();
    for part in ["label", "unit"] {
        let mut bad = value.clone();
        bad["layers"][0]["parts"][part]["size"] = 1000.into();
        assert!(serde_json::from_value::<LcdTheme>(bad)
            .unwrap()
            .validate()
            .is_err());
    }
}

#[test]
fn text_layout_bounds_and_round_trip() {
    let mut value = serde_json::json!({"version":1,"name":"wrap","background":[0,0,0],"layers":[{"kind":"text","text":"long text","style":{"x":240,"y":240,"size":43,"align":"center","vertical_align":"center","max_width":358,"line_height":58,"color":[255,255,255]}}]});
    let theme: LcdTheme = serde_json::from_value(value.clone()).unwrap();
    theme.validate().unwrap();
    assert_eq!(serde_json::to_value(theme).unwrap(), value);
    for (field, n) in [
        ("max_width", 0),
        ("max_width", 481),
        ("line_height", 7),
        ("line_height", 321),
    ] {
        let mut invalid = value.clone();
        invalid["layers"][0]["style"][field] = n.into();
        assert!(serde_json::from_value::<LcdTheme>(invalid)
            .unwrap()
            .validate()
            .is_err());
    }
    value["layers"][0]["style"]["vertical_align"] = "unknown".into();
    assert!(serde_json::from_value::<LcdTheme>(value).is_err());
}

#[test]
fn voltage_and_frequency_units_roundtrip_without_temperature_conversion() {
    use lconnect_ipc::{SensorReading, SensorStatus};
    for (unit, value, expected, wire) in [
        (SensorUnit::Volts, 0.975, "0.97 V", "volts"),
        (
            SensorUnit::BytesPerSecond,
            1000.0,
            "1000.00 B/s",
            "bytes_per_second",
        ),
        (SensorUnit::Hertz, 1500000000.0, "1500.00 MHz", "hertz"),
    ] {
        let mut row = SensorReading {
            id: wire.into(),
            label: wire.into(),
            unit,
            value: Some(value),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(1),
            age_ms: Some(0),
        };
        let format = |r: &SensorReading| {
            lconnect_ipc::theme_sensor_text(
                std::slice::from_ref(r),
                wire,
                unit,
                TemperatureUnit::Fahrenheit,
                2,
            )
        };
        assert_eq!(format(&row), expected);
        let encoded = serde_json::to_value(&row).unwrap();
        assert_eq!(encoded["unit"], wire);
        assert_eq!(
            serde_json::from_value::<SensorReading>(encoded).unwrap(),
            row
        );
        row.age_ms = Some(3001);
        assert_eq!(format(&row), "—");
        row.age_ms = Some(0);
        row.unit = SensorUnit::Celsius;
        assert_eq!(format(&row), "—", "changed unit must not rebind silently");
    }
}

#[test]
fn gradient_contract_preserves_legacy_and_rejects_malformed_settings() {
    let legacy = serde_json::json!({"version":1,"name":"Legacy","background":[1,2,3],"layers":[]});
    let decoded: LcdTheme = serde_json::from_value(legacy.clone()).unwrap();
    assert!(decoded.background_fill.is_none());
    assert_eq!(serde_json::to_value(decoded).unwrap(), legacy);
    let colors = serde_json::json!({"colors":[[0,0,0],[255,255,255]],"flip":true});
    let mut valid = legacy.clone();
    valid["background_fill"] =
        serde_json::json!({"mode":"radial","horizontal":colors,"vertical":colors,"radial":colors});
    let theme: LcdTheme = serde_json::from_value(valid.clone()).unwrap();
    theme.validate().unwrap();
    assert_eq!(serde_json::to_value(theme).unwrap(), valid);
    for (pointer, value) in [
        ("/background_fill/mode", serde_json::json!("unknown")),
        (
            "/background_fill/radial/colors",
            serde_json::json!([[1, 2, 3]]),
        ),
        ("/background_fill/radial/colors/0/0", serde_json::json!(256)),
        ("/background_fill/radial/flip", serde_json::json!(1)),
        ("/background_fill/vertical", serde_json::json!(null)),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            serde_json::from_value::<LcdTheme>(bad).is_err(),
            "{pointer}"
        );
    }
    let mut bad = valid;
    bad["background_fill"]["radial"]["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<LcdTheme>(bad).is_err());
}

#[test]
fn line_box_is_optional_and_strictly_boolean() {
    let legacy = serde_json::json!({"x":20,"y":30,"size":40,"color":[1,2,3],"align":"left"});
    let style: TextStyle = serde_json::from_value(legacy.clone()).unwrap();
    assert!(!style.line_box);
    let serialized = serde_json::to_value(style).unwrap();
    assert!(serialized.get("line_box").is_none());
    let mut enabled = legacy;
    enabled["line_box"] = serde_json::json!(true);
    assert!(
        serde_json::from_value::<TextStyle>(enabled.clone())
            .unwrap()
            .line_box
    );
    enabled["line_box"] = serde_json::json!("true");
    assert!(serde_json::from_value::<TextStyle>(enabled).is_err());
}

#[test]
fn managed_font_keys_are_strict_including_sensor_parts_and_roundtrip() {
    let mut value = serde_json::json!({"version":1,"name":"Fonts","background":[0,0,0],"layers":[{
        "kind":"sensor","sensor_id":"cpu","label":"CPU","unit":"celsius","temperature_unit":"celsius","decimals":0,
        "style":{"x":1,"y":1,"size":20,"color":[255,255,255],"align":"left"}
    }]});
    let style = value["layers"][0]["style"].clone();
    value["layers"][0]["parts"] = serde_json::json!({"label":style,"unit":style});
    for key in [
        "",
        "../secret",
        "A123456789abcdef0123456789abcdef",
        "0123456789abcdef0123456789abcde",
        "0123456789abcdef0123456789abcdefg",
    ] {
        value["layers"][0]["parts"]["unit"]["font_asset"] = key.into();
        assert!(serde_json::from_value::<LcdTheme>(value.clone())
            .unwrap()
            .validate()
            .is_err());
    }
    let key = "0123456789abcdef0123456789abcdef";
    value["layers"][0]["parts"]["unit"]["font_asset"] = key.into();
    let theme: LcdTheme = serde_json::from_value(value).unwrap();
    theme.validate().unwrap();
    assert_eq!(
        theme.font_assets().into_iter().collect::<Vec<_>>(),
        vec![key]
    );
    assert_eq!(
        serde_json::from_slice::<LcdTheme>(&serde_json::to_vec(&theme).unwrap()).unwrap(),
        theme
    );
}

#[test]
fn video_theme_version_keeps_media_and_layers_in_one_identity() {
    let legacy = serde_json::json!({"version":1,"name":"Legacy","background":[1,2,3],"layers":[]});
    let old: LcdTheme = serde_json::from_value(legacy.clone()).unwrap();
    old.validate().unwrap();
    assert_eq!(serde_json::to_value(&old).unwrap(), legacy);
    let mut video = old.clone();
    video.version = 2;
    assert!(video.validate().is_err());
    video.background_video = Some("a".repeat(32));
    video.validate().unwrap();
    assert_eq!(
        serde_json::from_slice::<LcdTheme>(&serde_json::to_vec(&video).unwrap()).unwrap(),
        video
    );
    video.version = 1;
    assert!(video.validate().is_err());
    video.version = 2;
    for key in ["../escape", "", "ABCDEF0123456789ABCDEF0123456789AB"] {
        video.background_video = Some(key.into());
        assert!(video.validate().is_err());
    }
}

#[test]
fn static_background_schema_tracks_references_and_rejects_ambiguous_or_unsafe_sources() {
    let mut theme: lconnect_ipc::LcdTheme = serde_json::from_value(serde_json::json!({
        "version":1,"name":"Background","background":[0,0,0],"layers":[],
        "background_image":{"asset":"builtin-midnight-v1","fit":"crop"}
    }))
    .unwrap();
    theme.validate().unwrap();
    assert!(theme.image_assets().contains("builtin-midnight-v1"));
    let saved = serde_json::to_vec(&theme).unwrap();
    assert_eq!(
        serde_json::from_slice::<lconnect_ipc::LcdTheme>(&saved).unwrap(),
        theme
    );
    theme.version = 2;
    theme.background_video = Some("a".repeat(32));
    assert!(theme.validate().is_err());
    theme.version = 1;
    theme.background_video = None;
    theme.background_image.as_mut().unwrap().asset = "../outside".into();
    assert!(theme.validate().is_err());
}

#[test]
fn divider_fractional_geometry_roundtrips_and_rejects_outside_canvas_or_bad_shadow() {
    let mut theme = serde_json::json!({"version":1,"name":"Editable divider","background":[0,0,0],"layers":[{"kind":"divider","axis":"vertical","x":240.5,"y":264.0,"length":166.0,"thickness":2.0,"color":[71,201,137],"opacity":100,"shadow":{"x":0,"y":0,"blur":5,"color":[0,0,0],"opacity":100}}]});
    let parsed: lconnect_ipc::LcdTheme = serde_json::from_value(theme.clone()).unwrap();
    parsed.validate().unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap()["layers"],
        theme["layers"]
    );
    assert!(parsed.font_assets().is_empty() && parsed.image_assets().is_empty());
    for (key, value) in [
        ("x", serde_json::json!(-0.5)),
        ("length", serde_json::json!(0)),
        ("thickness", serde_json::json!(17)),
        ("opacity", serde_json::json!(101)),
        ("y", serde_json::json!(400)),
    ] {
        let mut bad = theme.clone();
        bad["layers"][0][key] = value;
        assert!(
            serde_json::from_value::<lconnect_ipc::LcdTheme>(bad)
                .unwrap()
                .validate()
                .is_err(),
            "{key}"
        );
    }
    theme["layers"][0]["shadow"]["blur"] = 17.into();
    assert!(serde_json::from_value::<lconnect_ipc::LcdTheme>(theme)
        .unwrap()
        .validate()
        .is_err());
}
