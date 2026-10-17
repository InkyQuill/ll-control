#[path = "support/screen_runtime.rs"]
mod fixture;
use lconnect_ipc::{BackgroundMode, ClockFormat, ImageFit, TemperatureUnit, TextAlign, ThemeLayer};
use lconnect_profile::linux_screen::{parse_bindings, prepare_runtime, PreparedRuntime};
use serde_json::{json, Value};
fn prepare(source: &Value) -> Result<PreparedRuntime, lconnect_profile::Error> {
    prepare_runtime(
        &source.to_string(),
        &parse_bindings(&fixture::bindings().to_string().into_bytes())?,
    )
}
#[test]
fn custom_text_uses_explicit_fonts_colors_wrapping_alignment_and_windows_blank_line_semantics() {
    let mut layout = fixture::layout();
    layout["FontColors"] = json!([[17, 33, 99]]);
    layout["HorizontalAlignment"] = json!("Right");
    layout["TextAlignment"] = json!("Left");
    let source =
        fixture::source(json!({"Type":"Custom","Text":"first\n\nsecond","Layout":[layout]}));
    let result = prepare(&source).unwrap();
    result.theme.validate().unwrap();
    let ThemeLayer::Text { text, style } = &result.theme.layers[0] else {
        panic!("text")
    };
    assert_eq!(text, "first\nsecond");
    assert_eq!((style.x, style.y, style.size), (144, 240, 43));
    assert_eq!(style.align, TextAlign::Left);
    assert_eq!(style.color, [17, 33, 99]);
    assert_eq!(style.max_width, Some(314));
    assert_eq!(style.line_height, Some(58));
    assert_eq!(style.opacity, 80);
    assert_eq!(style.shadow.as_ref().unwrap().opacity, 35);
    assert!(result.layout_review_required);
    assert!(
        !result.pixel_parity_verified
            && !result.resource_payloads_verified
            && !result.sensor_binding_verified
            && !result.editor_banks_converted
            && !result.apply_supported
    );
    assert_eq!(result.used_fonts, vec!["HyperspaceRace_BoldItalic"]);
}
#[test]
fn all_basic_overlay_types_preserve_roles_order_styles_colors_and_clock_semantics() {
    for (kind, roles, count) in [
        (
            "SliderSensor",
            vec!["GPULoad", "CPUTemperature", "PumpRPM"],
            1,
        ),
        ("DuoSensor", vec!["PumpRPM", "CPULoad"], 2),
        ("ThreeSensor", vec!["CoolantTemp", "GPULoad", "CPULoad"], 3),
        (
            "FourSensor",
            vec!["GPULoad", "CPUTemperature", "CPULoad", "PumpRPM"],
            4,
        ),
    ] {
        let mut text = json!({"Type":kind,"Sensors":roles,"Layout":if kind=="DuoSensor" {json!([fixture::layout(),fixture::layout()])}else{json!([fixture::layout()])}});
        if matches!(kind, "ThreeSensor" | "FourSensor") {
            text["Layout"][0]["FontColors"] = json!([[210, 20, 30], [20, 210, 30]]);
            text["Layout"][0]["Opacity"] = json!(100);
        }
        if kind == "DuoSensor" {
            text["Layout"][0]["HorizontalAlignment"] = json!("Left");
            text["Layout"][1]["HorizontalAlignment"] = json!("Right");
        }
        if kind == "SliderSensor" {
            text["LoopSecond"] = json!(7);
        }
        let result = prepare(&fixture::source(text)).unwrap();
        let divider_count = match kind {
            "ThreeSensor" => 2,
            "FourSensor" => 3,
            _ => 0,
        };
        assert_eq!(result.theme.layers.len(), count + divider_count);
        let dividers: Vec<_> = result
            .theme
            .layers
            .iter()
            .filter(|layer| matches!(layer, ThemeLayer::Divider { .. }))
            .collect();
        assert_eq!(dividers.len(), divider_count);
        for layer in dividers {
            let ThemeLayer::Divider {
                color,
                thickness,
                opacity,
                shadow,
                ..
            } = layer
            else {
                unreachable!()
            };
            assert_eq!(*color, [20, 210, 30]);
            assert_eq!(*thickness, 2.0);
            assert_eq!(*opacity, 100);
            assert_eq!(shadow.as_ref().unwrap().blur, 5);
        }
        if kind == "SliderSensor" {
            let ThemeLayer::SensorCycle {
                sensors,
                interval_seconds,
                parts,
                ..
            } = &result.theme.layers[0]
            else {
                panic!("cycle")
            };
            assert_eq!(*interval_seconds, 7);
            if let ThemeLayer::SensorCycle {
                style,
                parts: Some(parts),
                ..
            } = &result.theme.layers[0]
            {
                assert_eq!(style.size, 135);
                assert_eq!(parts.label.size, 32);
            }
            assert_eq!(
                sensors
                    .iter()
                    .map(|s| s.sensor_id.clone())
                    .collect::<Vec<_>>(),
                roles
                    .iter()
                    .map(|r| format!("explicit:{r}"))
                    .collect::<Vec<_>>()
            );
            assert_eq!(sensors[1].temperature_unit, TemperatureUnit::Fahrenheit);
            assert!(parts.is_some());
        } else {
            for (layer, role) in result.theme.layers.iter().zip(roles) {
                let ThemeLayer::Sensor {
                    sensor_id,
                    label,
                    style,
                    parts: Some(parts),
                    ..
                } = layer
                else {
                    panic!("sensor")
                };
                assert_eq!(sensor_id, &format!("explicit:{role}"));
                assert_eq!(label, &format!("Owned {role}"));
                if matches!(kind, "ThreeSensor" | "FourSensor") {
                    assert_eq!(style.color, [210, 20, 30]);
                    assert_eq!(parts.label.color, [20, 210, 30]);
                    assert_eq!(parts.unit.color, [20, 210, 30]);
                    assert_eq!(parts.label.y, parts.unit.y);
                    assert_eq!(
                        (
                            style.shadow.as_ref().unwrap().x,
                            style.shadow.as_ref().unwrap().blur
                        ),
                        (0, 2)
                    );
                } else {
                    assert_eq!(style.color, [20, 210, 30]);
                    assert_eq!(parts.label.color, [210, 20, 30]);
                    assert_eq!(parts.unit.color, [20, 30, 210]);
                }
            }
        }
    }
    let result = prepare(&fixture::source(
        json!({"Type":"Time","Layout":[fixture::layout()]}),
    ))
    .unwrap();
    for (layer, format) in result.theme.layers.iter().zip([
        ClockFormat::Date,
        ClockFormat::TimeWithoutPeriod,
        ClockFormat::Period,
    ]) {
        let ThemeLayer::Clock {
            format: actual,
            twenty_four_hour,
            show_seconds,
            ..
        } = layer
        else {
            panic!("clock")
        };
        assert_eq!(*actual, format);
        if let ThemeLayer::Clock { style, .. } = layer {
            assert_eq!(
                style.size,
                if format == ClockFormat::TimeWithoutPeriod {
                    100
                } else {
                    32
                }
            );
        }
        assert!(!twenty_four_hour && !show_seconds);
    }
    assert!(prepare(&fixture::source(json!({"Type":"None"})))
        .unwrap()
        .theme
        .layers
        .is_empty());
}
#[test]
fn image_video_gradients_and_watermark_use_exact_explicit_resource_keys() {
    let mut source = fixture::source(json!({"Type":"None"}));
    source["Screens"][0]["Theme"]["Path"] = json!("file:///C:/owned/art.png");
    let result = prepare(&source).unwrap();
    let image = result.theme.background_image.unwrap();
    assert_eq!(image.asset, "11111111111111111111111111111111");
    assert_eq!(image.fit, ImageFit::Stretch);
    source["Screens"][0]["Theme"]["Path"] = json!("file:///C:/owned/clip.mp4");
    let result = prepare(&source).unwrap();
    assert_eq!(result.theme.version, 2);
    assert_eq!(
        result.theme.background_video.as_deref(),
        Some("22222222222222222222222222222222")
    );
    for (kind, mode) in [
        ("SolidColor", BackgroundMode::Solid),
        ("HorizontalGradient", BackgroundMode::Horizontal),
        ("VerticalGradient", BackgroundMode::Vertical),
        ("RadialGradient", BackgroundMode::Radial),
    ] {
        source["Screens"][0]["Theme"] = json!({"Type":"Color","ColorType":kind,"Colors":if kind=="SolidColor" {json!([[4,5,6]])}else{json!([[4,5,6],[70,80,90]])},"Flip":true});
        let result = prepare(&source).unwrap();
        let fill = result.theme.background_fill.unwrap();
        assert_eq!(fill.mode, mode);
        assert!(fill.horizontal.flip && fill.vertical.flip && fill.radial.flip);
        assert_eq!(result.theme.background, [4, 5, 6]);
    }
    source["Screens"][0]["Watermark"] = json!({"HasImage":true,"Image":"C:\\owned\\logo.png","Horizontal":0,"Vertical":100,"Shadow":62,"Opacity":50});
    let result = prepare(&source).unwrap();
    let ThemeLayer::Image {
        asset,
        opacity,
        shadow,
        watermark_position: Some(pos),
        ..
    } = &result.theme.layers[0]
    else {
        panic!("watermark")
    };
    assert_eq!(asset, "33333333333333333333333333333333");
    assert_eq!(*opacity, 128);
    assert_eq!(shadow.as_ref().unwrap().opacity, 62);
    assert_eq!((pos.horizontal, pos.vertical), (0, 100));
    source["Screens"][0]["Watermark"]["Image"] = json!("file:///C:/owned/clip.mp4");
    assert!(prepare(&source).is_err());
}
#[test]
fn no_implicit_bindings_fallbacks_unknown_fields_or_out_of_bounds_values() {
    let source = fixture::source(
        json!({"Type":"SliderSensor","Sensors":["CPULoad"],"LoopSecond":7,"Layout":[fixture::layout()]}),
    );
    for section in ["fonts", "sensors"] {
        let mut bindings = fixture::bindings();
        bindings[section] = json!({});
        assert!(prepare_runtime(
            &source.to_string(),
            &parse_bindings(bindings.to_string().as_bytes()).unwrap()
        )
        .is_err());
    }
    let mut bindings = fixture::bindings();
    bindings["sensors"]["CPULoad"]["unit"] = json!("celsius");
    assert!(parse_bindings(bindings.to_string().as_bytes()).is_err());
    let mut bindings = fixture::bindings();
    bindings["sensors"]["GPULoad"]["sensor_id"] =
        bindings["sensors"]["CPULoad"]["sensor_id"].clone();
    assert!(parse_bindings(bindings.to_string().as_bytes()).is_err());
    let mut bindings = fixture::bindings();
    bindings["sensors"]["CPULoad"]["typo"] = json!(true);
    assert!(parse_bindings(bindings.to_string().as_bytes()).is_err());
    for (field, value) in [
        ("Style", json!("Vertical")),
        ("Screens", json!([])),
        ("Unknown", json!(1)),
    ] {
        let mut source = source.clone();
        source[field] = value;
        assert!(prepare(&source).is_err());
    }
    for (field, value) in [
        ("FontFamily", json!("Unknown Font")),
        ("FontColors", json!([[1, 2, 3]])),
        ("Opacity", json!(101)),
        ("Shadow", json!(-1)),
        ("VerticalAlignment", json!("Stretch")),
        ("FontSize", json!(200)),
        ("Typo", json!(true)),
    ] {
        let mut source = source.clone();
        source["Screens"][0]["Text"]["Layout"][0][field] = value;
        assert!(prepare(&source).is_err(), "{field}");
    }
    let mut duo = fixture::source(
        json!({"Type":"DuoSensor","Sensors":["CPULoad","GPULoad"],"Layout":[fixture::layout(),fixture::layout()]}),
    );
    assert!(prepare(&duo).is_err());
    duo["Screens"][0]["Text"]["Layout"][0]["HorizontalAlignment"] = json!("Left");
    duo["Screens"][0]["Text"]["Layout"][1]["HorizontalAlignment"] = json!("Right");
    assert!(prepare(&duo).is_ok());
    duo["Screens"][0]["Text"]["Layout"][1]["Opacity"] = json!(42);
    assert!(prepare(&duo).is_err());
    let mut image = fixture::source(json!({"Type":"None"}));
    image["Screens"][0]["Theme"]["Path"] = json!("C:/other/art.png");
    assert!(prepare(&image).is_err());
    assert!(parse_bindings(br#"{"name":"one","name":"two"}"#).is_err());
    assert!(parse_bindings(&vec![b' '; 65537]).is_err());
    assert!(prepare_runtime(
        "",
        &parse_bindings(fixture::bindings().to_string().as_bytes()).unwrap()
    )
    .is_err());
}

#[test]
fn source_family_css_sizes_are_preserved_independently_of_the_chosen_linux_font() {
    for (family, size) in [
        ("HyperspaceRace_BoldItalic", 135),
        ("LTCFournierLeJeune_Regular", 152),
        ("ManoloMono_Regular", 154),
        ("Caprasimo_Regular", 112),
    ] {
        let mut layout = fixture::layout();
        layout["FontFamily"] = json!(family);
        let source = fixture::source(
            json!({"Type":"SliderSensor","Sensors":["CPULoad"],"LoopSecond":10,"Layout":[layout]}),
        );
        let mut mapping = fixture::bindings();
        mapping["fonts"][family] = json!({"font":"sans","font_asset":null});
        let result = prepare_runtime(
            &source.to_string(),
            &parse_bindings(mapping.to_string().as_bytes()).unwrap(),
        )
        .unwrap();
        let ThemeLayer::SensorCycle { style, .. } = &result.theme.layers[0] else {
            panic!("cycle")
        };
        assert_eq!(style.size, size);
    }
}
