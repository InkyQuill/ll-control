use image::{DynamicImage, Rgba, RgbaImage};
use lconnect_image::theme::*;
use lconnect_ipc::*;
fn clock() -> ClockTime {
    ClockTime {
        year: 2026,
        month: 10,
        day: 10,
        hour: 13,
        minute: 2,
        second: 3,
    }
}
fn base(layers: Vec<ThemeLayer>) -> LcdTheme {
    LcdTheme {
        page_cache: Vec::new(),
        basic_editor: None,
        version: 1,
        name: "Тест".into(),
        background: [0, 0, 100],
        background_image: None,
        background_video: None,
        background_fill: None,
        layers,
    }
}
fn style() -> TextStyle {
    TextStyle {
        font_asset: None,
        line_box: false,
        max_width: None,
        line_height: None,
        vertical_align: Default::default(),
        font: Default::default(),
        opacity: 100,
        shadow: None,
        x: 240,
        y: 100,
        size: 40,
        color: [255; 3],
        align: TextAlign::Center,
    }
}
#[test]
fn image_alpha_and_contain_preserve_underlying_background() {
    let assets = ThemeAssets::from([(
        "logo".into(),
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 1, Rgba([200, 0, 0, 128]))).into(),
    )]);
    let theme = base(vec![ThemeLayer::Image {
        watermark_position: None,
        shadow: None,
        asset: "logo".into(),
        x: 0,
        y: 0,
        width: 100,
        height: 100,
        fit: ImageFit::Contain,
        opacity: 255,
    }]);
    let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(frame.dimensions(), (480, 480));
    assert_eq!(frame.get_pixel(50, 10).0, [0, 0, 100]);
    assert_eq!(frame.get_pixel(50, 50).0, [100, 0, 50]);
    let mut missing = theme;
    if let ThemeLayer::Image { asset, .. } = &mut missing.layers[0] {
        *asset = "gone".into();
    }
    assert!(matches!(
        render_theme(&missing, &[], clock(), &assets),
        Err(RenderError::MissingAsset(_))
    ));
}
#[test]
fn sensor_refresh_and_unavailability_change_rendered_frame() {
    let theme = base(vec![ThemeLayer::Sensor {
        parts: None,
        sensor_id: "cpu".into(),
        label: "ЦП".into(),
        unit: SensorUnit::Celsius,
        temperature_unit: TemperatureUnit::Celsius,
        decimals: 1,
        style: style(),
    }]);
    let mut sensor = SensorReading {
        id: "cpu".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        value: Some(42.0),
        status: SensorStatus::Valid,
        age_ms: Some(1),
        observed_unix_ms: Some(1),
    };
    let assets = ThemeAssets::new();
    let a = render_theme(&theme, &[sensor.clone()], clock(), &assets).unwrap();
    sensor.value = Some(68.0);
    let b = render_theme(&theme, &[sensor.clone()], clock(), &assets).unwrap();
    assert_ne!(a, b);
    sensor.status = SensorStatus::Stale;
    let c = render_theme(&theme, &[sensor], clock(), &assets).unwrap();
    assert_ne!(b, c);
    let placeholder = base(vec![ThemeLayer::Text {
        text: "ЦП —".into(),
        style: style(),
    }]);
    assert_eq!(
        c,
        render_theme(&placeholder, &[], clock(), &assets).unwrap()
    );
    assert!(c.pixels().any(|p| p.0 != theme.background));
}
#[test]
fn clock_format_and_unicode_multiline_are_deterministic() {
    let theme = base(vec![ThemeLayer::Clock {
        format: Default::default(),
        twenty_four_hour: false,
        show_seconds: true,
        style: style(),
    }]);
    let expected = base(vec![ThemeLayer::Text {
        text: "01:02:03 PM".into(),
        style: style(),
    }]);
    let assets = ThemeAssets::new();
    assert_eq!(
        render_theme(&theme, &[], clock(), &assets).unwrap(),
        render_theme(&expected, &[], clock(), &assets).unwrap()
    );
    let text = base(vec![ThemeLayer::Text {
        text: "Помпа\nТемпература".into(),
        style: style(),
    }]);
    let frame = render_theme(&text, &[], clock(), &assets).unwrap();
    assert!(frame
        .enumerate_pixels()
        .any(|(_, y, p)| y > 145 && p.0 != text.background));
    assert!(matches!(
        render_theme(
            &theme,
            &[],
            ClockTime {
                hour: 24,
                ..clock()
            },
            &assets
        ),
        Err(RenderError::Clock)
    ));
}

#[test]
fn extreme_aspect_crop_is_centered_and_empty_assets_fail() {
    let mut pixels = RgbaImage::from_pixel(10000, 1, Rgba([255, 0, 0, 255]));
    pixels.put_pixel(4999, 0, Rgba([0, 255, 0, 255]));
    let mut assets = ThemeAssets::from([("wide".into(), DynamicImage::ImageRgba8(pixels).into())]);
    let theme = base(vec![ThemeLayer::Image {
        watermark_position: None,
        shadow: None,
        asset: "wide".into(),
        x: 0,
        y: 0,
        width: 480,
        height: 480,
        fit: ImageFit::Crop,
        opacity: 255,
    }]);
    let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert!(frame.pixels().all(|p| p.0 == [0, 255, 0]));
    assets.insert("wide".into(), DynamicImage::new_rgba8(0, 0).into());
    assert!(matches!(
        render_theme(&theme, &[], clock(), &assets),
        Err(RenderError::EmptyAsset(_))
    ));
}

#[test]
fn sensor_cycle_switches_on_boundaries_keeps_missing_slot_and_wraps() {
    let bindings = vec![
        ThemeSensor {
            sensor_id: "cpu".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            temperature_unit: TemperatureUnit::Celsius,
            decimals: 1,
        },
        ThemeSensor {
            sensor_id: "power".into(),
            label: "Power".into(),
            unit: SensorUnit::Watts,
            temperature_unit: TemperatureUnit::Celsius,
            decimals: 1,
        },
    ];
    let theme = base(vec![ThemeLayer::SensorCycle {
        parts: None,
        sensors: bindings.clone(),
        interval_seconds: 10,
        style: style(),
    }]);
    let rows = vec![SensorReading {
        id: "cpu".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        value: Some(42.0),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }];
    let assets = ThemeAssets::new();
    for (elapsed, index) in [(0, 0), (9, 0), (10, 1), (19, 1), (20, 0), (86400, 0)] {
        let s = &bindings[index];
        let single = base(vec![ThemeLayer::Sensor {
            parts: None,
            sensor_id: s.sensor_id.clone(),
            label: s.label.clone(),
            unit: s.unit,
            temperature_unit: s.temperature_unit,
            decimals: s.decimals,
            style: style(),
        }]);
        assert_eq!(
            render_theme_elapsed(&theme, &rows, clock(), &assets, elapsed).unwrap(),
            render_theme(&single, &rows, clock(), &assets).unwrap()
        );
    }
    assert_ne!(
        render_theme_elapsed(&theme, &rows, clock(), &assets, 0).unwrap(),
        render_theme_elapsed(&theme, &rows, clock(), &assets, 10).unwrap()
    );
}

#[test]
fn invalid_cycle_is_rejected_before_division_or_indexing() {
    for (count, interval) in [(0, 10), (1, 0), (17, 10), (1, 3601)] {
        let sensors = vec![
            ThemeSensor {
                sensor_id: "cpu".into(),
                label: "".into(),
                unit: SensorUnit::Celsius,
                temperature_unit: TemperatureUnit::Celsius,
                decimals: 1
            };
            count
        ];
        let theme = base(vec![ThemeLayer::SensorCycle {
            parts: None,
            sensors,
            interval_seconds: interval,
            style: style(),
        }]);
        assert!(render_theme_elapsed(&theme, &[], clock(), &ThemeAssets::new(), 0).is_err());
    }
}

#[test]
fn fonts_shadows_and_opacity_render_without_changing_legacy_defaults() {
    let assets = ThemeAssets::new();
    let render = |style: TextStyle| {
        render_theme(
            &base(vec![ThemeLayer::Text {
                text: "Помпа LCD".into(),
                style,
            }]),
            &[],
            clock(),
            &assets,
        )
        .unwrap()
    };
    let legacy = style();
    assert_eq!(legacy, style());
    let normal = render(legacy.clone());
    for font in [ThemeFont::Serif, ThemeFont::Mono] {
        let mut changed = legacy.clone();
        changed.font = font;
        assert_ne!(normal, render(changed));
    }
    let mut shadowed = legacy.clone();
    shadowed.shadow = Some(TextShadow {
        x: 10,
        y: 10,
        blur: 2,
        color: [255, 0, 0],
        opacity: 100,
    });
    let shadow_frame = render(shadowed.clone());
    assert_ne!(normal, shadow_frame);
    assert!(shadow_frame
        .pixels()
        .zip(normal.pixels())
        .any(|(a, b)| a[0] > b[0] && a[1] <= b[1]));
    shadowed.opacity = 0;
    assert!(render(shadowed).pixels().all(|p| p.0 == [0, 0, 100]));
    let mut transparent = legacy;
    transparent.opacity = 50;
    assert_ne!(normal, render(transparent));
}

#[test]
fn text_style_rejects_unbounded_shadow_and_opacity() {
    for (x, blur, opacity) in [(33, 0, 100), (0, 17, 100), (0, 0, 101)] {
        let mut bad = style();
        bad.shadow = Some(TextShadow {
            x,
            y: 0,
            blur,
            color: [0; 3],
            opacity,
        });
        assert!(base(vec![ThemeLayer::Text {
            text: "test".into(),
            style: bad
        }])
        .validate()
        .is_err());
    }
    let mut bad = style();
    bad.opacity = 101;
    assert!(base(vec![ThemeLayer::Text {
        text: "test".into(),
        style: bad
    }])
    .validate()
    .is_err());
}

#[test]
fn watermark_shadow_follows_alpha_and_layer_opacity() {
    let mut logo = RgbaImage::new(2, 1);
    logo.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    let assets = ThemeAssets::from([("logo".into(), DynamicImage::ImageRgba8(logo).into())]);
    let mut theme = base(vec![ThemeLayer::Image {
        watermark_position: None,
        asset: "logo".into(),
        x: 100,
        y: 100,
        width: 2,
        height: 1,
        fit: ImageFit::Stretch,
        opacity: 255,
        shadow: Some(TextShadow {
            x: 4,
            y: 3,
            blur: 0,
            color: [255; 3],
            opacity: 100,
        }),
    }]);
    let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(frame.get_pixel(100, 100).0, [255, 0, 0]);
    assert_eq!(frame.get_pixel(104, 103).0, [255; 3]);
    // Transparent half of the logo must not cast a rectangular shadow.
    assert_eq!(frame.get_pixel(105, 103).0, theme.background);
    if let ThemeLayer::Image { opacity, .. } = &mut theme.layers[0] {
        *opacity = 128;
    }
    let faded = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(faded.get_pixel(104, 103).0, [128, 128, 178]);
    if let ThemeLayer::Image {
        opacity, shadow, ..
    } = &mut theme.layers[0]
    {
        *opacity = 255;
        shadow.as_mut().unwrap().blur = 2;
    }
    let blurred = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert!(blurred.get_pixel(105, 103)[0] > 0);
    if let ThemeLayer::Image { opacity, .. } = &mut theme.layers[0] {
        *opacity = 0;
    }
    assert!(render_theme(&theme, &[], clock(), &assets)
        .unwrap()
        .pixels()
        .all(|p| p.0 == theme.background));
}

#[test]
fn clock_parts_render_date_midnight_noon_and_independent_styles() {
    for (format, instant, expected) in [
        (
            ClockFormat::Date,
            ClockTime {
                year: 2000,
                month: 2,
                day: 29,
                ..clock()
            },
            "2000/02/29",
        ),
        (
            ClockFormat::Date,
            ClockTime {
                year: 2027,
                month: 1,
                day: 1,
                ..clock()
            },
            "2027/01/01",
        ),
        (
            ClockFormat::TimeWithoutPeriod,
            ClockTime {
                hour: 0,
                minute: 0,
                ..clock()
            },
            "12:00",
        ),
        (
            ClockFormat::TimeWithoutPeriod,
            ClockTime {
                hour: 12,
                minute: 0,
                ..clock()
            },
            "12:00",
        ),
        (
            ClockFormat::Period,
            ClockTime { hour: 0, ..clock() },
            "A.M.",
        ),
        (
            ClockFormat::Period,
            ClockTime {
                hour: 12,
                ..clock()
            },
            "P.M.",
        ),
    ] {
        let theme = base(vec![ThemeLayer::Clock {
            format,
            twenty_four_hour: false,
            show_seconds: false,
            style: style(),
        }]);
        let text = base(vec![ThemeLayer::Text {
            text: expected.into(),
            style: style(),
        }]);
        let assets = ThemeAssets::new();
        assert_eq!(
            render_theme(&theme, &[], instant, &assets).unwrap(),
            render_theme(&text, &[], instant, &assets).unwrap()
        );
    }
    let date = base(vec![ThemeLayer::Clock {
        format: ClockFormat::Date,
        twenty_four_hour: false,
        show_seconds: false,
        style: style(),
    }]);
    for instant in [
        ClockTime {
            year: 1900,
            month: 2,
            day: 29,
            ..clock()
        },
        ClockTime {
            month: 13,
            ..clock()
        },
        ClockTime { day: 0, ..clock() },
    ] {
        assert!(matches!(
            render_theme(&date, &[], instant, &ThemeAssets::new()),
            Err(RenderError::Clock)
        ));
    }
}

#[test]
fn watermark_center_uses_fitted_size_and_clips_at_canvas_edges() {
    let assets = ThemeAssets::from([(
        "logo".into(),
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(40, 20, Rgba([255, 0, 0, 255]))).into(),
    )]);
    let mut theme = base(vec![ThemeLayer::Image {
        asset: "logo".into(),
        x: 0,
        y: 0,
        width: 40,
        height: 40,
        fit: ImageFit::Contain,
        opacity: 255,
        shadow: None,
        watermark_position: Some(WatermarkPosition {
            horizontal: 50,
            vertical: 50,
        }),
    }]);
    let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(frame.get_pixel(220, 230).0, [255, 0, 0]);
    assert_eq!(frame.get_pixel(240, 229).0, theme.background);
    assert_eq!(frame.get_pixel(259, 249).0, [255, 0, 0]);
    assert_eq!(frame.get_pixel(260, 240).0, theme.background);
    for (percent, left, top) in [(0, 2, 12), (100, 460, 470)] {
        if let ThemeLayer::Image {
            watermark_position, ..
        } = &mut theme.layers[0]
        {
            *watermark_position = Some(WatermarkPosition {
                horizontal: percent,
                vertical: percent,
            });
        }
        let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
        assert_eq!(frame.get_pixel(left, top).0, [255, 0, 0]);
        assert_eq!(frame.get_pixel(left - 1, top).0, theme.background);
    }
}

#[test]
fn off_canvas_watermark_alpha_can_cast_a_visible_shadow() {
    let mut source = RgbaImage::new(100, 1);
    source.put_pixel(0, 0, Rgba([0, 255, 0, 255]));
    let assets = ThemeAssets::from([("logo".into(), DynamicImage::ImageRgba8(source).into())]);
    let theme = base(vec![ThemeLayer::Image {
        asset: "logo".into(),
        x: 0,
        y: 0,
        width: 100,
        height: 1,
        fit: ImageFit::Stretch,
        opacity: 255,
        watermark_position: Some(WatermarkPosition {
            horizontal: 0,
            vertical: 50,
        }),
        shadow: Some(TextShadow {
            x: 32,
            y: 0,
            blur: 0,
            color: [255, 0, 0],
            opacity: 100,
        }),
    }]);
    let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
    // Source pixel is at -28; its shadow reaches x=4.
    assert_eq!(frame.get_pixel(4, 240).0, [255, 0, 0]);
    assert_eq!(frame.get_pixel(5, 240).0, theme.background);
}

#[test]
fn native_contain_does_not_enlarge_small_assets_but_downscales_large_ones() {
    for (source_width, expected_width) in [(20, 20), (200, 100)] {
        let assets = ThemeAssets::from([(
            "logo".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                source_width,
                source_width / 2,
                Rgba([255, 0, 0, 255]),
            ))
            .into(),
        )]);
        let theme = base(vec![ThemeLayer::Image {
            asset: "logo".into(),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            fit: ImageFit::ContainNative,
            opacity: 255,
            shadow: None,
            watermark_position: Some(WatermarkPosition {
                horizontal: 50,
                vertical: 50,
            }),
        }]);
        let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
        assert_eq!(
            frame.pixels().filter(|p| p.0 == [255, 0, 0]).count(),
            (expected_width * expected_width / 2) as usize
        );
        assert_eq!(
            frame.get_pixel(240 - expected_width / 2, 240).0,
            [255, 0, 0]
        );
        assert_eq!(
            frame.get_pixel(239 - expected_width / 2, 240).0,
            theme.background
        );
    }
}

#[test]
fn sensor_parts_render_independent_colors_and_preserve_missing_value() {
    let label_style = TextStyle {
        y: 50,
        color: [255, 0, 0],
        ..style()
    };
    let unit_style = TextStyle {
        y: 160,
        color: [0, 255, 0],
        ..style()
    };
    let parts = SensorParts {
        label: label_style.clone(),
        unit: unit_style.clone(),
    };
    let theme = base(vec![ThemeLayer::Sensor {
        parts: Some(parts.clone()),
        sensor_id: "cpu".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        temperature_unit: TemperatureUnit::Fahrenheit,
        decimals: 1,
        style: style(),
    }]);
    let reading = SensorReading {
        id: "cpu".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        value: Some(10.0),
        status: SensorStatus::Valid,
        age_ms: Some(1),
        observed_unix_ms: Some(1),
    };
    for (rows, value) in [(vec![reading], "50.0"), (vec![], "—")] {
        let expected = base(vec![
            ThemeLayer::Text {
                text: value.into(),
                style: style(),
            },
            ThemeLayer::Text {
                text: "CPU".into(),
                style: label_style.clone(),
            },
            ThemeLayer::Text {
                text: "°F".into(),
                style: unit_style.clone(),
            },
        ]);
        assert_eq!(
            render_theme(&theme, &rows, clock(), &ThemeAssets::new()).unwrap(),
            render_theme(&expected, &[], clock(), &ThemeAssets::new()).unwrap()
        );
    }
    let cycle = base(vec![ThemeLayer::SensorCycle {
        parts: Some(parts),
        sensors: vec![
            ThemeSensor {
                sensor_id: "missing".into(),
                label: "Pump".into(),
                unit: SensorUnit::Rpm,
                temperature_unit: TemperatureUnit::Celsius,
                decimals: 0,
            },
            ThemeSensor {
                sensor_id: "missing2".into(),
                label: "Power".into(),
                unit: SensorUnit::Watts,
                temperature_unit: TemperatureUnit::Celsius,
                decimals: 1,
            },
        ],
        interval_seconds: 10,
        style: style(),
    }]);
    for (elapsed, label, unit) in [(0, "Pump", "RPM"), (10, "Power", "W")] {
        let expected = base(vec![
            ThemeLayer::Text {
                text: "—".into(),
                style: style(),
            },
            ThemeLayer::Text {
                text: label.into(),
                style: label_style.clone(),
            },
            ThemeLayer::Text {
                text: unit.into(),
                style: unit_style.clone(),
            },
        ]);
        assert_eq!(
            render_theme_elapsed(&cycle, &[], clock(), &ThemeAssets::new(), elapsed).unwrap(),
            render_theme(&expected, &[], clock(), &ThemeAssets::new()).unwrap()
        );
    }
}

#[test]
fn wrapped_text_and_vertical_anchors_match_explicit_lines() {
    let wrapped = TextStyle {
        font: ThemeFont::Mono,
        max_width: Some(70),
        line_height: Some(50),
        vertical_align: VerticalAlign::Center,
        y: 240,
        ..style()
    };
    let theme = base(vec![ThemeLayer::Text {
        text: "AA BB".into(),
        style: wrapped.clone(),
    }]);
    // Mono 40px: AA+space fits 70px, AA+space+B does not. Two lines are 90px high.
    let expected = base(vec![ThemeLayer::Text {
        text: "AA \nBB".into(),
        style: TextStyle {
            font_asset: None,
            line_box: false,
            max_width: None,
            vertical_align: VerticalAlign::Top,
            y: 195,
            ..wrapped.clone()
        },
    }]);
    assert_eq!(
        render_theme(&theme, &[], clock(), &ThemeAssets::new()).unwrap(),
        render_theme(&expected, &[], clock(), &ThemeAssets::new()).unwrap()
    );
    let bottom = base(vec![ThemeLayer::Text {
        text: "AA BB".into(),
        style: TextStyle {
            vertical_align: VerticalAlign::Bottom,
            y: 285,
            ..wrapped
        },
    }]);
    assert_eq!(
        render_theme(&bottom, &[], clock(), &ThemeAssets::new()).unwrap(),
        render_theme(&expected, &[], clock(), &ThemeAssets::new()).unwrap()
    );
}

#[test]
fn gradient_backgrounds_preserve_solid_and_compose_below_layers() {
    let mut theme = base(vec![]);
    let colors = GradientColors {
        colors: [[0, 0, 0], [240, 120, 60]],
        flip: false,
    };
    theme.background_fill = Some(BackgroundFill {
        mode: BackgroundMode::Horizontal,
        horizontal: colors.clone(),
        vertical: colors.clone(),
        radial: colors.clone(),
    });
    let assets = ThemeAssets::new();
    let horizontal = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(horizontal.get_pixel(0, 123).0, [0, 0, 0]);
    assert_eq!(horizontal.get_pixel(240, 0).0, [120, 60, 30]);
    assert_eq!(horizontal.get_pixel(240, 479), horizontal.get_pixel(240, 0));
    theme.background_fill.as_mut().unwrap().mode = BackgroundMode::Vertical;
    let vertical = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(vertical.get_pixel(123, 0).0, [0, 0, 0]);
    assert_eq!(vertical.get_pixel(0, 240).0, [120, 60, 30]);
    assert_eq!(vertical.get_pixel(479, 240), vertical.get_pixel(0, 240));
    theme.background_fill.as_mut().unwrap().mode = BackgroundMode::Radial;
    let radial = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(radial.get_pixel(240, 240).0, [0, 0, 0]);
    assert_eq!(radial.get_pixel(0, 0).0, [240, 120, 60]);
    assert_eq!(radial.get_pixel(0, 240).0, [170, 85, 42]);
    theme.background_fill.as_mut().unwrap().radial.flip = true;
    let flipped = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(flipped.get_pixel(240, 240).0, [240, 120, 60]);
    assert_eq!(flipped.get_pixel(0, 0).0, [0, 0, 0]);
    theme.background_fill.as_mut().unwrap().mode = BackgroundMode::Solid;
    assert_eq!(
        render_theme(&theme, &[], clock(), &assets).unwrap(),
        render_theme(&base(vec![]), &[], clock(), &assets).unwrap()
    );
    theme.background_fill.as_mut().unwrap().mode = BackgroundMode::Horizontal;
    theme.background_fill.as_mut().unwrap().horizontal.colors = [[7, 8, 9]; 2];
    assert!(render_theme(&theme, &[], clock(), &assets)
        .unwrap()
        .pixels()
        .all(|p| p.0 == [7, 8, 9]));
    theme.layers.push(ThemeLayer::Text {
        text: "Overlay".into(),
        style: style(),
    });
    let overlaid = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert!(overlaid.pixels().any(|p| p.0 != [7, 8, 9]));
    assert_eq!(overlaid.get_pixel(0, 0).0, [7, 8, 9]);
}

#[test]
fn css_line_boxes_keep_top_center_bottom_anchors_and_allow_negative_leading() {
    let assets = ThemeAssets::new();
    let render = |line_box, anchor, y, height| {
        let mut text_style = style();
        text_style.line_box = line_box;
        text_style.line_height = Some(height);
        text_style.vertical_align = anchor;
        text_style.y = y;
        render_theme(
            &base(vec![ThemeLayer::Text {
                text: "One\n\nThree".into(),
                style: text_style,
            }]),
            &[],
            clock(),
            &assets,
        )
        .unwrap()
    };
    // The complete block is three line boxes, including the blank line.
    for height in [80, 20] {
        let total = 3 * height;
        assert_eq!(
            render(true, VerticalAlign::Top, 100, height),
            render(true, VerticalAlign::Center, 100 + total / 2, height)
        );
        assert_eq!(
            render(true, VerticalAlign::Top, 100, height),
            render(true, VerticalAlign::Bottom, 100 + total, height)
        );
    }
    // Legacy metric-height text keeps its original 40 + 2*80 block height.
    assert_eq!(
        render(false, VerticalAlign::Top, 100, 80),
        render(false, VerticalAlign::Center, 200, 80)
    );
}

#[test]
fn bundled_tc_fonts_have_real_glyphs_and_distinct_regular_bold_rendering() {
    use ab_glyph::{Font, FontRef};
    let text = "繁體中文 溫度 風扇 幫浦 液晶螢幕 42°C ≒ ◈ \u{30ede}";
    for bytes in [
        include_bytes!("../assets/NotoSansTC-Regular.otf").as_slice(),
        include_bytes!("../assets/NotoSansTC-Bold.otf").as_slice(),
    ] {
        let font = FontRef::try_from_slice(bytes).unwrap();
        for ch in text.chars().filter(|c| !c.is_whitespace()) {
            let id = font.glyph_id(ch);
            assert_ne!(id.0, 0, "missing {ch}");
            assert!(
                font.outline_glyph(id.with_scale(40.0)).is_some(),
                "no outline for {ch}"
            );
        }
    }
    let render = |font| {
        let mut s = style();
        s.font = font;
        s.max_width = Some(358);
        render_theme(
            &base(vec![ThemeLayer::Text {
                text: text.into(),
                style: s,
            }]),
            &[],
            clock(),
            &ThemeAssets::new(),
        )
        .unwrap()
    };
    let regular = render(ThemeFont::NotoSansTcRegular);
    let bold = render(ThemeFont::NotoSansTcBold);
    assert_ne!(regular, bold);
    assert!(regular.pixels().any(|p| p.0 != [0, 0, 100]));
    assert!(bold.pixels().any(|p| p.0 != [0, 0, 100]));
}

#[test]
fn managed_fonts_match_bundled_render_for_every_text_role_and_never_fallback() {
    let mut s = style();
    s.font = ThemeFont::Serif;
    let sensor = ThemeLayer::Sensor {
        sensor_id: "missing".into(),
        label: "Sensor".into(),
        unit: SensorUnit::Celsius,
        temperature_unit: TemperatureUnit::Celsius,
        decimals: 0,
        parts: Some(SensorParts {
            label: s.clone(),
            unit: s.clone(),
        }),
        style: s.clone(),
    };
    let cycle = ThemeLayer::SensorCycle {
        sensors: vec![ThemeSensor {
            sensor_id: "missing".into(),
            label: "Cycle".into(),
            unit: SensorUnit::Celsius,
            temperature_unit: TemperatureUnit::Celsius,
            decimals: 0,
        }],
        interval_seconds: 1,
        parts: Some(SensorParts {
            label: s.clone(),
            unit: s.clone(),
        }),
        style: s.clone(),
    };
    let layers = [
        ThemeLayer::Text {
            text: "Width Wi 123".into(),
            style: s.clone(),
        },
        ThemeLayer::Clock {
            format: ClockFormat::Time,
            twenty_four_hour: true,
            show_seconds: true,
            style: s,
        },
        sensor,
        cycle,
    ];
    let key = "0123456789abcdef0123456789abcdef";
    let fonts = ThemeFonts::from([(
        key.into(),
        lconnect_image::fonts::prepare_font(include_bytes!("../assets/DejaVuSerif.ttf").to_vec())
            .unwrap(),
    )]);
    for layer in layers {
        let original = base(vec![layer]);
        let expected = render_theme(&original, &[], clock(), &ThemeAssets::new()).unwrap();
        let mut managed = original.clone();
        let set = |s: &mut TextStyle| {
            s.font_asset = Some(key.into());
            s.font = ThemeFont::Sans;
        };
        match &mut managed.layers[0] {
            ThemeLayer::Text { style, .. } | ThemeLayer::Clock { style, .. } => set(style),
            ThemeLayer::Sensor { style, parts, .. }
            | ThemeLayer::SensorCycle { style, parts, .. } => {
                set(style);
                let parts = parts.as_mut().unwrap();
                set(&mut parts.label);
                set(&mut parts.unit);
            }
            _ => unreachable!(),
        }
        assert_eq!(managed.font_assets().len(), 1);
        assert!(matches!(
            render_theme(&managed, &[], clock(), &ThemeAssets::new()),
            Err(RenderError::MissingFont(_))
        ));
        assert_eq!(
            render_theme_with_fonts(&managed, &[], clock(), &ThemeAssets::new(), &fonts, 0)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn bundled_display_fonts_have_outlines_and_select_distinct_rendering() {
    use ab_glyph::{Font, FontRef};
    use lconnect_image::theme::{render_theme_with_fonts, ThemeFonts};
    let mut rendered = Vec::new();
    for (choice, bytes, text) in [
        (
            ThemeFont::AbrilFatface,
            include_bytes!("../assets/AbrilFatface-Regular.ttf").as_slice(),
            "LCD 42",
        ),
        (
            ThemeFont::Caprasimo,
            include_bytes!("../assets/Caprasimo-Regular.ttf").as_slice(),
            "LCD 42",
        ),
        (
            ThemeFont::Cubic11,
            include_bytes!("../assets/Cubic_11_1.013_R.ttf").as_slice(),
            "LCD 42 繁體中文",
        ),
    ] {
        let font = FontRef::try_from_slice(bytes).unwrap();
        for ch in text.chars().filter(|c| !c.is_whitespace()) {
            let id = font.glyph_id(ch);
            assert_ne!(id.0, 0, "missing {ch}");
            assert!(font.outline_glyph(id.with_scale(40.0)).is_some());
        }
        let mut text_style = style();
        text_style.font = choice;
        let theme = base(vec![ThemeLayer::Text {
            text: text.into(),
            style: text_style,
        }]);
        let image = render_theme(&theme, &[], clock(), &ThemeAssets::new()).unwrap();
        let key = "0123456789abcdef0123456789abcdef";
        let fonts = ThemeFonts::from([(
            key.into(),
            lconnect_image::fonts::prepare_font(bytes.to_vec()).unwrap(),
        )]);
        let mut managed = theme.clone();
        if let ThemeLayer::Text { style, .. } = &mut managed.layers[0] {
            style.font_asset = Some(key.into());
            style.font = ThemeFont::Sans;
        }
        assert_eq!(
            image,
            render_theme_with_fonts(&managed, &[], clock(), &ThemeAssets::new(), &fonts, 0)
                .unwrap()
        );
        assert!(image.pixels().any(|p| p.0 != [0, 0, 100]));
        assert!(rendered.iter().all(|previous| previous != &image));
        rendered.push(image);
    }
}

#[test]
fn image_fit_modes_keep_expected_portrait_and_landscape_bounds() {
    for (width, height, portrait) in [(4, 2, false), (2, 4, true)] {
        let assets = ThemeAssets::from([(
            "asset".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                width,
                height,
                Rgba([10, 220, 30, 255]),
            ))
            .into(),
        )]);
        for (fit, bounds) in [
            (ImageFit::Crop, (100, 100, 8, 8)),
            (ImageFit::Stretch, (100, 100, 8, 8)),
            (
                ImageFit::Contain,
                if portrait {
                    (102, 100, 4, 8)
                } else {
                    (100, 102, 8, 4)
                },
            ),
            (
                ImageFit::ContainNative,
                if portrait {
                    (103, 102, 2, 4)
                } else {
                    (102, 103, 4, 2)
                },
            ),
        ] {
            let theme = base(vec![ThemeLayer::Image {
                asset: "asset".into(),
                x: 100,
                y: 100,
                width: 8,
                height: 8,
                fit,
                opacity: 255,
                shadow: None,
                watermark_position: None,
            }]);
            let rendered = render_theme(&theme, &[], clock(), &assets).unwrap();
            let (left, top, w, h) = bounds;
            for (x, y, pixel) in rendered.enumerate_pixels() {
                let expected = if (left..left + w).contains(&x) && (top..top + h).contains(&y) {
                    [10, 220, 30]
                } else {
                    theme.background
                };
                assert_eq!(
                    pixel.0, expected,
                    "{fit:?}, portrait={portrait}, pixel=({x},{y})"
                );
            }
        }
    }
}

#[test]
fn overlapping_image_order_and_zero_opacity_survive_right_bottom_clipping() {
    let assets = ThemeAssets::from([
        (
            "red".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 255]))).into(),
        ),
        (
            "green".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([0, 255, 0, 255]))).into(),
        ),
    ]);
    let layer = |asset: &str, horizontal, vertical| ThemeLayer::Image {
        asset: asset.into(),
        x: 0,
        y: 0,
        width: 40,
        height: 40,
        fit: ImageFit::Stretch,
        opacity: 255,
        shadow: None,
        watermark_position: Some(WatermarkPosition {
            horizontal,
            vertical,
        }),
    };
    let mut theme = base(vec![layer("red", 100, 98), layer("green", 98, 100)]);
    let first = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(first.get_pixel(479, 479).0, [0, 255, 0]);
    assert_eq!(first.get_pixel(470, 450).0, [255, 0, 0]);
    assert_eq!(first.get_pixel(450, 470).0, [0, 255, 0]);
    assert_eq!(first.get_pixel(0, 0).0, theme.background);
    theme.layers.reverse();
    let reordered = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(reordered.get_pixel(479, 479).0, [255, 0, 0]);
    assert_eq!(reordered.get_pixel(450, 470).0, [0, 255, 0]);
    if let ThemeLayer::Image { opacity, .. } = &mut theme.layers[1] {
        *opacity = 0;
    }
    let hidden = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(hidden.get_pixel(479, 479).0, [0, 255, 0]);
    assert_eq!(hidden.get_pixel(470, 450).0, theme.background);
    assert_eq!(hidden.get_pixel(0, 0).0, theme.background);
}

#[test]
fn video_frame_preserves_pixels_and_uses_the_same_layer_semantics() {
    use image::{Rgb, RgbImage};
    let mut sensor_style = style();
    sensor_style.y = 220;
    let mut clock_style = style();
    clock_style.y = 320;
    let theme = base(vec![
        ThemeLayer::Image {
            watermark_position: None,
            shadow: None,
            asset: "logo".into(),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            fit: ImageFit::Stretch,
            opacity: 255,
        },
        ThemeLayer::Text {
            text: "LCD · Видео".into(),
            style: style(),
        },
        ThemeLayer::Sensor {
            parts: None,
            sensor_id: "cpu".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            temperature_unit: TemperatureUnit::Celsius,
            decimals: 1,
            style: sensor_style,
        },
        ThemeLayer::Clock {
            format: Default::default(),
            twenty_four_hour: true,
            show_seconds: true,
            style: clock_style,
        },
    ]);
    let assets = ThemeAssets::from([(
        "logo".into(),
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 0, 0, 128]))).into(),
    )]);
    let readings = [SensorReading {
        id: "cpu".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        value: Some(42.0),
        status: SensorStatus::Valid,
        observed_unix_ms: None,
        age_ms: Some(0),
    }];
    for background in [[0, 0, 100], [10, 120, 30]] {
        let frame = RgbImage::from_pixel(480, 480, Rgb(background));
        let mut time = clock();
        for second in [0, 1] {
            time.second = second;
            let output = render_theme_over_frame(
                &theme,
                &readings,
                time,
                &assets,
                &ThemeFonts::new(),
                0,
                frame.clone(),
            )
            .unwrap();
            let mut expected = theme.clone();
            expected.background = background;
            assert_eq!(
                output,
                render_theme(&expected, &readings, time, &assets).unwrap()
            );
            assert_eq!(output.get_pixel(479, 479).0, background);
            assert_ne!(
                output.get_pixel(10, 10).0,
                background,
                "alpha watermark drawn over video"
            );
        }
    }
    assert!(matches!(
        render_theme_over_frame(
            &theme,
            &readings,
            clock(),
            &assets,
            &ThemeFonts::new(),
            0,
            RgbImage::new(479, 480)
        ),
        Err(RenderError::FrameSize)
    ));
}

#[test]
fn video_theme_requires_explicit_decoded_frame_without_solid_fallback() {
    let mut theme = base(vec![]);
    theme.version = 2;
    theme.background_video = Some("a".repeat(32));
    let assets = ThemeAssets::new();
    let fonts = ThemeFonts::new();
    assert!(matches!(
        render_theme_with_fonts(&theme, &[], clock(), &assets, &fonts, 0),
        Err(RenderError::MissingVideoFrame)
    ));
    let frame = RgbImage::from_pixel(480, 480, image::Rgb([17, 29, 53]));
    let rendered =
        render_theme_over_frame(&theme, &[], clock(), &assets, &fonts, 0, frame.clone()).unwrap();
    assert!(rendered == frame, "video canvas changed without layers");
}

#[test]
fn image_background_sits_under_overlays_and_missing_background_fails() {
    let mut theme = base(vec![ThemeLayer::Image {
        asset: "overlay".into(),
        x: 0,
        y: 0,
        width: 100,
        height: 100,
        fit: ImageFit::Stretch,
        opacity: 255,
        shadow: None,
        watermark_position: None,
    }]);
    theme.background_image = Some(BackgroundImage {
        asset: "back".into(),
        fit: ImageFit::Contain,
    });
    let assets = ThemeAssets::from([
        (
            "back".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 1, Rgba([200, 0, 0, 128]))).into(),
        ),
        (
            "overlay".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([0, 255, 0, 255]))).into(),
        ),
    ]);
    let frame = render_theme(&theme, &[], clock(), &assets).unwrap();
    assert_eq!(
        frame.get_pixel(10, 10).0,
        [0, 255, 0],
        "overlay stays on top"
    );
    assert_eq!(
        frame.get_pixel(240, 10).0,
        [0, 0, 100],
        "contain exposes saved color"
    );
    assert_eq!(
        frame.get_pixel(240, 240).0,
        [100, 0, 50],
        "image alpha blends over color"
    );
    let mut missing = assets;
    missing.remove("back");
    assert!(
        matches!(render_theme(&theme, &[], clock(), &missing), Err(RenderError::MissingAsset(key)) if key == "back")
    );
}

#[test]
fn weather_renders_above_background_image_and_below_ordered_overlays() {
    let weather = lconnect_ipc::WeatherEffect {
        retained_sensors: Vec::new(),
        presets: Vec::new(),
        kind: lconnect_ipc::WeatherKind::Snow,
        intensity: 3,
        sensor: None,
        dynamic: false,
        seed: 42,
    };
    let mut theme = base(vec![
        ThemeLayer::Weather {
            effect: weather.clone(),
        },
        ThemeLayer::Image {
            asset: "overlay".into(),
            x: 200,
            y: 200,
            width: 80,
            height: 80,
            fit: ImageFit::Stretch,
            opacity: 255,
            shadow: None,
            watermark_position: None,
        },
    ]);
    theme.version = 3;
    theme.background_image = Some(BackgroundImage {
        asset: "background".into(),
        fit: ImageFit::Stretch,
    });
    let assets = ThemeAssets::from([
        (
            "background".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(480, 480, Rgba([0, 0, 100, 255])))
                .into(),
        ),
        (
            "overlay".into(),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(80, 80, Rgba([255, 0, 0, 255]))).into(),
        ),
    ]);
    let mut scene = lconnect_image::effects::WeatherScene::new(weather).unwrap();
    let first = render_theme_weather_frame(
        &theme,
        &[],
        clock(),
        &assets,
        &ThemeFonts::new(),
        WeatherFrame {
            elapsed_millis: 0,
            scene: &mut scene,
        },
        RgbImage::new(480, 480),
    )
    .unwrap();
    assert_eq!(first.get_pixel(240, 240).0, [255, 0, 0]);
    assert!(
        first
            .pixels()
            .any(|p| p[0] > 100 && p[1] > 100 && p[2] > 100),
        "Background image must not hide snow"
    );
    let later = render_theme_weather_frame(
        &theme,
        &[],
        clock(),
        &assets,
        &ThemeFonts::new(),
        WeatherFrame {
            elapsed_millis: 42,
            scene: &mut scene,
        },
        RgbImage::new(480, 480),
    )
    .unwrap();
    assert_ne!(first, later);
    assert_eq!(later.get_pixel(240, 240).0, [255, 0, 0]);
}

#[test]
fn dividers_preserve_fractional_coverage_opacity_layer_order_and_shadow() {
    let vertical = ThemeLayer::Divider {
        axis: DividerAxis::Vertical,
        x: 1.5,
        y: 10.0,
        length: 8.0,
        thickness: 2.0,
        color: [200, 0, 0],
        opacity: 100,
        shadow: None,
    };
    let horizontal = ThemeLayer::Divider {
        axis: DividerAxis::Horizontal,
        x: 0.0,
        y: 14.0,
        length: 8.0,
        thickness: 1.0,
        color: [0, 0, 200],
        opacity: 50,
        shadow: None,
    };
    let mut theme = base(vec![vertical, horizontal]);
    theme.background = [0; 3];
    let frame = render_theme(&theme, &[], clock(), &ThemeAssets::new()).unwrap();
    assert_eq!(frame.get_pixel(0, 11).0, [0, 0, 0]);
    assert_eq!(frame.get_pixel(1, 11).0, [100, 0, 0]);
    assert_eq!(frame.get_pixel(2, 11).0, [200, 0, 0]);
    assert_eq!(frame.get_pixel(3, 11).0, [100, 0, 0]);
    assert_eq!(frame.get_pixel(2, 14).0, [100, 0, 100]);
    assert_eq!(frame.get_pixel(2, 18).0, [0, 0, 0]);
    theme.background = [255; 3];
    theme.layers = vec![ThemeLayer::Divider {
        axis: DividerAxis::Vertical,
        x: 10.0,
        y: 10.0,
        length: 8.0,
        thickness: 2.0,
        color: [255; 3],
        opacity: 100,
        shadow: Some(TextShadow {
            x: 0,
            y: 0,
            blur: 1,
            color: [0; 3],
            opacity: 100,
        }),
    }];
    let frame = render_theme(&theme, &[], clock(), &ThemeAssets::new()).unwrap();
    assert!(
        frame.get_pixel(9, 14)[0] < 255,
        "shadow must be visible outside the line"
    );
    assert_eq!(frame.get_pixel(10, 14).0, [255; 3]);
    assert_eq!(frame.get_pixel(0, 0).0, [255; 3]);
}

#[test]
fn measured_image_bounds_follow_fit_watermark_and_background_layer_index() {
    let assets = ThemeAssets::from([(
        "logo".into(),
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 50, Rgba([255; 4]))).into(),
    )]);
    let mut theme = base(vec![ThemeLayer::Image {
        watermark_position: Some(WatermarkPosition {
            horizontal: 0,
            vertical: 50,
        }),
        shadow: None,
        asset: "logo".into(),
        x: 300,
        y: 300,
        width: 80,
        height: 80,
        fit: ImageFit::Contain,
        opacity: 255,
    }]);
    theme.background_image = Some(BackgroundImage {
        asset: "logo".into(),
        fit: ImageFit::Stretch,
    });
    let frame = render_theme_measured(
        &theme,
        &[],
        clock(),
        &assets,
        &ThemeFonts::new(),
        0,
        render_background(&theme),
    )
    .unwrap();
    // The background has no selection entry. A zero watermark coordinate is the
    // Windows 22px center, while 50 percent is 240px. Contain fits 80x40.
    assert_eq!(
        frame.layers,
        vec![Some(LayerBounds {
            x: -18.0,
            y: 220.0,
            width: 80.0,
            height: 40.0
        })]
    );
    assert_eq!(
        frame.image,
        render_theme(&theme, &[], clock(), &assets).unwrap()
    );
}

#[test]
fn measured_text_and_sensor_parts_enclose_the_actual_foreground() {
    let mut value_style = style();
    value_style.x = 470;
    value_style.align = TextAlign::Right;
    value_style.max_width = Some(130);
    let mut label_style = style();
    label_style.x = 10;
    label_style.y = 20;
    label_style.align = TextAlign::Left;
    let mut unit_style = style();
    unit_style.y = 400;
    let theme = base(vec![ThemeLayer::Sensor {
        parts: Some(SensorParts {
            label: label_style,
            unit: unit_style,
        }),
        sensor_id: "missing".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        temperature_unit: TemperatureUnit::Celsius,
        decimals: 1,
        style: value_style,
    }]);
    let frame = render_theme_measured(
        &theme,
        &[],
        clock(),
        &ThemeAssets::new(),
        &ThemeFonts::new(),
        0,
        render_background(&theme),
    )
    .unwrap();
    let bounds = frame.layers[0].unwrap();
    assert!(bounds.x < 20.0 && bounds.y < 60.0);
    assert!(bounds.x + bounds.width > 430.0 && bounds.y + bounds.height > 400.0);
    for (x, y, pixel) in frame.image.enumerate_pixels() {
        if pixel.0 != theme.background {
            assert!((x as f32) >= bounds.x && (x as f32) < bounds.x + bounds.width);
            assert!((y as f32) >= bounds.y && (y as f32) < bounds.y + bounds.height);
        }
    }
    assert_eq!(
        frame.image,
        render_theme(&theme, &[], clock(), &ThemeAssets::new()).unwrap()
    );
}
