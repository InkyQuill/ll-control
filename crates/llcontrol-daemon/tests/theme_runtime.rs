#[path = "support/firmware_fixture.rs"]
mod firmware_fixture;
use llcontrol_daemon::{
    actor::{lcd_keeper, FrameOutcome, HidCommand, LcdCommand},
    sensors::SensorCache,
    theme_runtime::ThemeRuntime,
};
use llcontrol_image::theme::ThemeAssets;
use llcontrol_ipc::*;
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};
fn theme() -> LcdTheme {
    LcdTheme {
        page_cache: Vec::new(),
        basic_editor: None,
        version: 1,
        name: "Live CPU".into(),
        background: [0; 3],
        background_image: None,
        background_video: None,
        background_fill: None,
        layers: vec![ThemeLayer::Sensor {
            editor: None,
            parts: None,
            sensor_id: "cpu".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            temperature_unit: TemperatureUnit::Celsius,
            decimals: 1,
            style: TextStyle {
                font_asset: None,
                line_box: false,
                max_width: None,
                line_height: None,
                vertical_align: Default::default(),
                font: Default::default(),
                opacity: 100,
                shadow: None,
                x: 240,
                y: 220,
                size: 48,
                color: [255; 3],
                align: TextAlign::Center,
            },
        }],
    }
}
fn publish(cache: &SensorCache, value: f64) {
    cache.publish(
        std::time::Instant::now(),
        vec![SensorReading {
            id: "cpu".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            value: Some(value),
            status: SensorStatus::Valid,
            observed_unix_ms: None,
            age_ms: Some(0),
        }],
    );
}
#[tokio::test]
async fn live_theme_updates_and_manual_image_replaces_its_producer() {
    let (hid, mut requests) = mpsc::channel(8);
    let (frames, mut observed) = mpsc::unbounded_channel();
    let fake_hid = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            match request {
                HidCommand::FrameBatch { packets, reply, .. } => {
                    let written_reports = packets.len();
                    frames.send(packets).unwrap();
                    let _ = reply.send(Ok(FrameOutcome {
                        written_reports,
                        readback: vec![],
                        elapsed: Duration::ZERO,
                    }));
                }
                HidCommand::Write { reply, .. } => {
                    let _ = reply.send(Ok(()));
                }
                HidCommand::Firmware { reply } => {
                    let _ = reply.send(Ok(firmware_fixture::reply()));
                }
                _ => panic!("unexpected HID command"),
            }
        }
    });
    let (commands, rx) = mpsc::channel(8);
    let (settings, _settings_rx) = mpsc::channel(8);
    let (shutdown, sd) = watch::channel(false);
    let actor = lcd_keeper::spawn(hid, 60000, Default::default(), settings, rx, sd);
    let cache = SensorCache::default();
    publish(&cache, 40.0);
    let runtime =
        Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![cache.clone()]).unwrap());
    let (reply, response) = oneshot::channel();
    commands
        .send(LcdCommand::SetTheme {
            prepared: runtime.prepare().await.unwrap(),
            preparation: None,
            reply,
        })
        .await
        .unwrap();
    assert_eq!(
        response.await.unwrap().unwrap().source,
        LcdSourceKind::Theme
    );
    let first = observed.recv().await.unwrap();
    publish(&cache, 75.0);
    let second = tokio::time::timeout(Duration::from_secs(5), observed.recv())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        first, second,
        "each theme tick must use the current sensor cache"
    );
    let (reply, response) = oneshot::channel();
    commands.send(LcdCommand::Theme { reply }).await.unwrap();
    assert_eq!(response.await.unwrap(), Some(theme()));
    let (reply, response) = oneshot::channel();
    commands
        .send(LcdCommand::SetImage {
            jpeg: llcontrol_image::prepare_jpeg_from_image(
                llcontrol_image::theme::RgbImage::from_pixel(480, 480, [220, 30, 30].into()).into(),
                llcontrol_image::FitMode::Stretch,
                llcontrol_image::Quality::default(),
            )
            .unwrap()
            .bytes
            .into(),
            reply,
        })
        .await
        .unwrap();
    assert_eq!(response.await.unwrap().unwrap().source, LcdSourceKind::Jpeg);
    publish(&cache, 99.0);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("manual.h264");
    let mut stream = Vec::new();
    // Capture past the next keyframe so queued live frames cannot mask a stale producer.
    for _ in 0..48 {
        let packets = tokio::time::timeout(Duration::from_secs(2), observed.recv())
            .await
            .unwrap()
            .unwrap();
        for packet in packets {
            assert!(packet.starts_with(&[3, 13]));
            let length = usize::from(u16::from_be_bytes([packet[9], packet[10]]));
            stream.extend_from_slice(&packet[11..11 + length]);
        }
    }
    std::fs::write(&path, stream).unwrap();
    let decoded = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "h264", "-i"])
        .arg(path)
        .args([
            "-vf",
            "scale=1:1",
            "-pix_fmt",
            "rgb24",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    assert!(decoded.stdout.len() >= 24 * 3);
    for pixel in decoded.stdout.rchunks_exact(3).take(24) {
        assert!(
            pixel[0] > 190 && pixel[1] < 60 && pixel[2] < 60,
            "{pixel:?}"
        );
    }
    let (reply, response) = oneshot::channel();
    commands.send(LcdCommand::Theme { reply }).await.unwrap();
    assert!(
        response.await.unwrap().is_none(),
        "manual JPEG must release the live theme"
    );

    shutdown.send(true).unwrap();
    actor.await.unwrap();
    drop(commands);
    fake_hid.await.unwrap();
}

#[tokio::test]
async fn preparation_fails_before_replacing_current_source() {
    let (hid, _requests) = mpsc::channel(8);
    let (commands, rx) = mpsc::channel(8);
    let (settings, _settings_rx) = mpsc::channel(8);
    let (shutdown, sd) = watch::channel(false);
    let actor = lcd_keeper::spawn(hid, 60000, Default::default(), settings, rx, sd);
    let mut bad = theme();
    bad.layers.push(ThemeLayer::Image {
        editor: None,
        watermark_position: None,
        shadow: None,
        asset: "missing".into(),
        x: 0,
        y: 0,
        width: 480,
        height: 480,
        fit: ImageFit::Crop,
        opacity: 255,
    });
    let runtime = Arc::new(ThemeRuntime::new(bad, ThemeAssets::new(), vec![]).unwrap());
    assert!(runtime.prepare().await.is_err());
    let (reply, response) = oneshot::channel();
    commands.send(LcdCommand::State { reply }).await.unwrap();
    let state = response.await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Idle);
    assert_eq!(state.generation, 0);
    shutdown.send(true).unwrap();
    actor.await.unwrap();
}

#[tokio::test]
async fn superseded_preparation_cannot_restore_a_cleared_or_newer_theme() {
    let (hid, mut requests) = mpsc::channel(8);
    let (frames, mut observed) = mpsc::unbounded_channel();
    let fake_hid = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            match request {
                HidCommand::FrameBatch { packets, reply, .. } => {
                    let written_reports = packets.len();
                    frames.send(packets).unwrap();
                    let _ = reply.send(Ok(FrameOutcome {
                        written_reports,
                        readback: vec![],
                        elapsed: Duration::ZERO,
                    }));
                }
                HidCommand::Write { reply, .. } => {
                    let _ = reply.send(Ok(()));
                }
                HidCommand::Firmware { reply } => {
                    let _ = reply.send(Ok(firmware_fixture::reply()));
                }
                _ => panic!("unexpected HID command"),
            }
        }
    });
    let (commands, rx) = mpsc::channel(8);
    let (settings, mut saved) = mpsc::channel(8);
    let (shutdown, sd) = watch::channel(false);
    let actor = lcd_keeper::spawn(hid, 60000, Default::default(), settings, rx, sd);
    let reserve = || async {
        let (reply, response) = oneshot::channel();
        commands
            .send(LcdCommand::BeginThemePreparation { reply })
            .await
            .unwrap();
        response.await.unwrap()
    };
    let submit = |ticket, name: &'static str| {
        let commands = commands.clone();
        async move {
            let mut next = theme();
            next.name = name.into();
            let runtime = Arc::new(ThemeRuntime::new(next, ThemeAssets::new(), vec![]).unwrap());
            let (reply, response) = oneshot::channel();
            commands
                .send(LcdCommand::SetTheme {
                    prepared: runtime.prepare().await.unwrap(),
                    preparation: Some(ticket),
                    reply,
                })
                .await
                .unwrap();
            response.await.unwrap()
        }
    };
    let old = reserve().await;
    let newer = reserve().await;
    // The newer intent must win even before its assets finish preparing.
    assert!(submit(old, "Finishes first but obsolete")
        .await
        .unwrap_err()
        .to_string()
        .contains("superseded"));
    assert!(observed.try_recv().is_err());
    assert!(saved.try_recv().is_err());
    submit(newer, "Newer").await.unwrap();
    observed.recv().await.unwrap();
    saved.recv().await.unwrap();
    assert!(submit(old, "Delayed old theme")
        .await
        .unwrap_err()
        .to_string()
        .contains("superseded"));
    let (reply, response) = oneshot::channel();
    commands.send(LcdCommand::Theme { reply }).await.unwrap();
    assert_eq!(response.await.unwrap().unwrap().name, "Newer");
    assert!(
        saved.try_recv().is_err(),
        "stale preparation must not persist intent"
    );

    let pending = reserve().await;
    let (reply, response) = oneshot::channel();
    commands.send(LcdCommand::Clear { reply }).await.unwrap();
    assert_eq!(response.await.unwrap().unwrap().source, LcdSourceKind::Idle);
    // Clear acknowledges after the old producer/HID transaction barrier.
    while observed.try_recv().is_ok() {}
    while saved.try_recv().is_ok() {}
    assert!(submit(pending, "Resurrection")
        .await
        .unwrap_err()
        .to_string()
        .contains("superseded"));
    let (reply, response) = oneshot::channel();
    commands.send(LcdCommand::Theme { reply }).await.unwrap();
    assert_eq!(response.await.unwrap(), None);
    assert!(
        observed.try_recv().is_err(),
        "no new frame after acknowledged clear"
    );
    assert!(
        saved.try_recv().is_err(),
        "clear must remain the durable intent"
    );

    let fresh = reserve().await;
    assert_eq!(
        submit(fresh, "Explicit recovery").await.unwrap().source,
        LcdSourceKind::Theme
    );
    observed.recv().await.unwrap();
    shutdown.send(true).unwrap();
    actor.await.unwrap();
    drop(commands);
    fake_hid.await.unwrap();
}

#[tokio::test]
async fn live_sensor_frames_revoke_missing_stale_invalid_and_wrong_binding_without_fake_zero() {
    let cache = SensorCache::default();
    let runtime =
        Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![cache.clone()]).unwrap());
    let unavailable = runtime.frame_at(0).await.unwrap();
    publish(&cache, 0.0);
    let zero = runtime.frame_at(0).await.unwrap();
    assert_ne!(
        zero, unavailable,
        "genuine zero and unavailable must render differently"
    );
    publish(&cache, 75.0);
    let valid = runtime.frame_at(0).await.unwrap();
    assert_ne!(valid, zero);
    for (status, unit, id, age) in [
        (SensorStatus::Valid, SensorUnit::Celsius, "cpu", 4),
        (SensorStatus::Invalid, SensorUnit::Celsius, "cpu", 0),
        (SensorStatus::Unavailable, SensorUnit::Celsius, "cpu", 0),
        (SensorStatus::Valid, SensorUnit::Percent, "cpu", 0),
        (SensorStatus::Valid, SensorUnit::Celsius, "other", 0),
    ] {
        cache.publish(
            std::time::Instant::now() - Duration::from_secs(age),
            vec![SensorReading {
                id: id.into(),
                label: "CPU".into(),
                unit,
                value: Some(0.0),
                status,
                observed_unix_ms: Some(1),
                age_ms: Some(0),
            }],
        );
        assert_eq!(
            runtime.frame_at(0).await.unwrap(),
            unavailable,
            "invalid data must render unavailable: {status:?}/{unit:?}/{id}/{age}"
        );
    }
    cache.publish(std::time::Instant::now(), vec![]);
    assert_eq!(runtime.frame_at(0).await.unwrap(), unavailable);
    publish(&cache, 75.0);
    assert_eq!(
        runtime.frame_at(0).await.unwrap(),
        valid,
        "same binding recovers without rebuilding or applying theme"
    );
    assert_eq!(runtime.theme(), &theme());
}

#[tokio::test]
async fn cycle_cache_staleness_and_reordered_bindings_keep_label_value_unit_together() {
    let cache = SensorCache::default();
    let mut cycle = theme();
    let ThemeLayer::Sensor { style, .. } = cycle.layers.remove(0) else {
        unreachable!()
    };
    let parts = SensorParts {
        label: TextStyle {
            y: 160,
            color: [255, 0, 0],
            ..style.clone()
        },
        unit: TextStyle {
            y: 280,
            color: [0, 255, 0],
            ..style.clone()
        },
    };
    let bindings = vec![
        ThemeSensor {
            sensor_id: "cpu".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            temperature_unit: TemperatureUnit::Fahrenheit,
            decimals: 1,
        },
        ThemeSensor {
            sensor_id: "power".into(),
            label: "Power".into(),
            unit: SensorUnit::Watts,
            temperature_unit: TemperatureUnit::Celsius,
            decimals: 0,
        },
    ];
    cycle.layers.push(ThemeLayer::SensorCycle {
        editor: None,
        sensors: bindings.clone(),
        interval_seconds: 2,
        parts: Some(parts.clone()),
        style: style.clone(),
    });
    let runtime = Arc::new(
        ThemeRuntime::new(cycle.clone(), ThemeAssets::new(), vec![cache.clone()]).unwrap(),
    );
    let rows = vec![
        SensorReading {
            id: "cpu".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            value: Some(40.0),
            status: SensorStatus::Valid,
            observed_unix_ms: None,
            age_ms: Some(0),
        },
        SensorReading {
            id: "power".into(),
            label: "Power".into(),
            unit: SensorUnit::Watts,
            value: Some(125.0),
            status: SensorStatus::Valid,
            observed_unix_ms: None,
            age_ms: Some(0),
        },
    ];
    // A single-sensor runtime is the oracle for all three independently styled parts.
    let mut singles = Vec::new();
    for binding in &bindings {
        let mut single = theme();
        single.layers = vec![ThemeLayer::Sensor {
            editor: None,
            sensor_id: binding.sensor_id.clone(),
            label: binding.label.clone(),
            unit: binding.unit,
            temperature_unit: binding.temperature_unit,
            decimals: binding.decimals,
            parts: Some(parts.clone()),
            style: style.clone(),
        }];
        singles.push(Arc::new(
            ThemeRuntime::new(single, ThemeAssets::new(), vec![cache.clone()]).unwrap(),
        ));
    }
    cache.publish(std::time::Instant::now(), rows.clone());
    let first = runtime.frame_at(0).await.unwrap();
    let second = runtime.frame_at(2).await.unwrap();
    assert_ne!(first, second);
    for (elapsed, index) in [(0, 0), (1, 0), (2, 1), (3, 1), (4, 0)] {
        assert_eq!(
            runtime.frame_at(elapsed).await.unwrap(),
            singles[index].frame_at(0).await.unwrap()
        );
    }
    cache.publish(
        std::time::Instant::now() - Duration::from_secs(4),
        rows.clone(),
    );
    assert_ne!(runtime.frame_at(0).await.unwrap(), first);
    assert_ne!(runtime.frame_at(2).await.unwrap(), second);
    for (elapsed, index) in [(0, 0), (2, 1)] {
        assert_eq!(
            runtime.frame_at(elapsed).await.unwrap(),
            singles[index].frame_at(0).await.unwrap()
        );
    }
    cache.publish(std::time::Instant::now(), vec![rows[0].clone()]);
    assert_eq!(runtime.frame_at(0).await.unwrap(), first);
    assert_eq!(
        runtime.frame_at(2).await.unwrap(),
        singles[1].frame_at(0).await.unwrap()
    );
    cache.publish(std::time::Instant::now(), rows);
    assert_eq!(runtime.frame_at(2).await.unwrap(), second);
    let ThemeLayer::SensorCycle { sensors, .. } = &mut cycle.layers[0] else {
        unreachable!()
    };
    sensors.swap(0, 1);
    let reordered = Arc::new(
        ThemeRuntime::new(cycle.clone(), ThemeAssets::new(), vec![cache.clone()]).unwrap(),
    );
    assert_eq!(reordered.frame_at(0).await.unwrap(), second);
    assert_eq!(reordered.frame_at(2).await.unwrap(), first);
    let ThemeLayer::SensorCycle { sensors, .. } = &mut cycle.layers[0] else {
        unreachable!()
    };
    sensors.pop();
    let only_power = Arc::new(ThemeRuntime::new(cycle, ThemeAssets::new(), vec![cache]).unwrap());
    assert_eq!(only_power.frame_at(100).await.unwrap(), second);
}

#[test]
fn first_frame_preparation_keeps_controls_available_and_clear_invalidates_apply() {
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    executor.block_on(async {
        let (hid, mut requests) = mpsc::channel(8);
        let io = tokio::spawn(async move {
            let Some(HidCommand::Write { reply, .. }) = requests.recv().await else {
                panic!("expected Clear before releasing the renderer");
            };
            reply.send(Ok(())).unwrap();
            assert!(
                requests.recv().await.is_none(),
                "obsolete first frame reached HID"
            );
        });
        let (commands, rx) = mpsc::channel(8);
        let (settings, _updates) = mpsc::channel(8);
        let (stop, shutdown) = watch::channel(false);
        let actor = lcd_keeper::spawn(hid, 60_000, Default::default(), settings, rx, shutdown);
        let (reply, response) = oneshot::channel();
        commands
            .send(LcdCommand::BeginThemePreparation { reply })
            .await
            .unwrap();
        let ticket = response.await.unwrap();

        // Occupy the sole blocking worker so the real raster/JPEG job cannot
        // complete. Control handling runs on Tokio, independently of this queue.
        let (entered, started) = oneshot::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            entered.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        started.await.unwrap();
        let runtime = Arc::new(ThemeRuntime::new(theme(), ThemeAssets::new(), vec![]).unwrap());
        let (preparing, waiting) = oneshot::channel();
        let preparation = tokio::spawn(async move {
            preparing.send(()).unwrap();
            runtime.prepare().await
        });
        waiting.await.unwrap();
        assert!(!preparation.is_finished());
        let (reply, response) = oneshot::channel();
        commands.send(LcdCommand::State { reply }).await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), response)
                .await
                .unwrap()
                .unwrap()
                .source,
            LcdSourceKind::Idle
        );
        let (reply, response) = oneshot::channel();
        commands.send(LcdCommand::Clear { reply }).await.unwrap();
        let cleared = tokio::time::timeout(Duration::from_secs(1), response)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(cleared.applied.unwrap().mode, "LocalUI");
        assert!(
            !preparation.is_finished(),
            "control replies must precede raster completion"
        );
        release.send(()).unwrap();
        blocker.await.unwrap();
        let prepared = preparation.await.unwrap().unwrap();
        let (reply, response) = oneshot::channel();
        commands
            .send(LcdCommand::SetTheme {
                prepared,
                preparation: Some(ticket),
                reply,
            })
            .await
            .unwrap();
        assert!(response
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("superseded"));
        let (reply, response) = oneshot::channel();
        commands.send(LcdCommand::State { reply }).await.unwrap();
        let state = response.await.unwrap();
        assert_eq!(state.source, LcdSourceKind::Idle);
        assert_eq!(state.generation, cleared.generation);
        stop.send(true).unwrap();
        actor.await.unwrap();
        io.await.unwrap();
    });
}

#[tokio::test]
async fn mixed_host_and_lcd_sensors_render_units_and_recover_independently() {
    let host = SensorCache::default();
    let device = SensorCache::default();
    let bindings = [
        ("cpu:temp", "CPU", SensorUnit::Celsius, 40.0, "CPU 104.0°F"),
        ("gpu:temp", "GPU", SensorUnit::Celsius, 50.0, "GPU 50.0°C"),
        (
            "cpu:load",
            "CPU load",
            SensorUnit::Percent,
            25.0,
            "CPU load 25.0%",
        ),
        (
            "gpu:load",
            "GPU load",
            SensorUnit::Percent,
            60.0,
            "GPU load 60.0%",
        ),
        ("ram:load", "RAM", SensorUnit::Percent, 75.0, "RAM 75.0%"),
        (
            "cpu:power",
            "Power",
            SensorUnit::Watts,
            65.0,
            "Power 65.0 W",
        ),
        (
            "lcd:coolant",
            "Coolant",
            SensorUnit::Celsius,
            34.0,
            "Coolant 34.0°C",
        ),
        (
            "lcd:pump",
            "Pump",
            SensorUnit::Rpm,
            2400.0,
            "Pump 2400.0 RPM",
        ),
        ("lcd:fan", "Fan", SensorUnit::Rpm, 0.0, "Fan 0.0 RPM"),
    ];
    let mut composition = theme();
    let ThemeLayer::Sensor { style, .. } = &composition.layers[0] else {
        unreachable!()
    };
    let style = style.clone();
    composition.layers = bindings
        .iter()
        .enumerate()
        .map(|(index, (id, label, unit, _, _))| {
            let mut style = style.clone();
            style.y = 20 + index as u16 * 48;
            style.size = 24;
            ThemeLayer::Sensor {
                editor: None,
                parts: None,
                sensor_id: (*id).into(),
                label: (*label).into(),
                unit: *unit,
                temperature_unit: if index == 0 {
                    TemperatureUnit::Fahrenheit
                } else {
                    TemperatureUnit::Celsius
                },
                decimals: 1,
                style,
            }
        })
        .collect();
    let rows: Vec<_> = bindings
        .iter()
        .map(|(id, label, unit, value, _)| SensorReading {
            id: (*id).into(),
            label: (*label).into(),
            unit: *unit,
            value: Some(*value),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(123),
            age_ms: Some(0),
        })
        .collect();
    let runtime = Arc::new(
        ThemeRuntime::new(
            composition.clone(),
            ThemeAssets::new(),
            vec![host.clone(), device.clone()],
        )
        .unwrap(),
    );
    // Literal expected text is independent of the production sensor formatter.
    async fn expected(composition: &LcdTheme, values: &[String]) -> bytes::Bytes {
        let mut text = composition.clone();
        text.layers = composition
            .layers
            .iter()
            .zip(values)
            .map(|(layer, value)| {
                let ThemeLayer::Sensor { style, .. } = layer else {
                    unreachable!()
                };
                ThemeLayer::Text {
                    editor: None,
                    text: value.clone(),
                    style: style.clone(),
                }
            })
            .collect();
        Arc::new(ThemeRuntime::new(text, ThemeAssets::new(), vec![]).unwrap())
            .frame_at(0)
            .await
            .unwrap()
    }
    let labels: Vec<String> = bindings.iter().map(|b| b.4.to_string()).collect();
    let fresh_frame = expected(&composition, &labels).await;
    host.publish(std::time::Instant::now(), rows[..6].to_vec());
    device.publish(std::time::Instant::now(), rows[6..].to_vec());
    assert_eq!(runtime.frame_at(0).await.unwrap(), fresh_frame);

    host.publish(
        std::time::Instant::now() - Duration::from_secs(4),
        rows[..6].to_vec(),
    );
    device.publish(std::time::Instant::now(), rows[6..].to_vec());
    let mut stale_host = labels.clone();
    for index in 0..6 {
        stale_host[index] = format!("{} —", bindings[index].1);
    }
    assert_eq!(
        runtime.frame_at(0).await.unwrap(),
        expected(&composition, &stale_host).await
    );

    host.publish(std::time::Instant::now(), rows[..6].to_vec());
    let mut damaged_device = rows[6..].to_vec();
    damaged_device[0].status = SensorStatus::Invalid;
    damaged_device[1].unit = SensorUnit::Percent;
    damaged_device[2].id = "lcd:other-fan".into();
    device.publish(std::time::Instant::now(), damaged_device);
    let mut missing_device = labels.clone();
    for index in 6..9 {
        missing_device[index] = format!("{} —", bindings[index].1);
    }
    assert_eq!(
        runtime.frame_at(0).await.unwrap(),
        expected(&composition, &missing_device).await
    );

    host.publish(std::time::Instant::now(), rows[..6].to_vec());
    device.publish(std::time::Instant::now(), rows[6..].to_vec());
    assert_eq!(runtime.frame_at(0).await.unwrap(), fresh_frame);
    assert_eq!(
        runtime.theme(),
        &composition,
        "recovery must not remap bindings or units"
    );
}

#[test]
fn decoded_video_composes_live_sensor_updates_and_missing_values_without_erasing_motion() {
    use llcontrol_image::theme::{render_theme_over_frame, ClockTime, RgbImage, ThemeFonts};
    use std::process::Command;
    let root = tempfile::tempdir().unwrap();
    let encoded = root.path().join("source.h264");
    let made = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=480x480:r=24",
            "-frames:v",
            "4",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-profile:v",
            "baseline",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
        ])
        .arg(&encoded)
        .output()
        .unwrap();
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let decoded = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(&encoded)
        .args(["-threads", "1", "-pix_fmt", "rgb24", "-f", "rawvideo", "-"])
        .output()
        .unwrap();
    assert!(decoded.status.success());
    const FRAME_BYTES: usize = 480 * 480 * 3;
    assert_eq!(decoded.stdout.len(), FRAME_BYTES * 4);
    assert_ne!(
        &decoded.stdout[..FRAME_BYTES],
        &decoded.stdout[FRAME_BYTES..2 * FRAME_BYTES]
    );
    let cache = SensorCache::default();
    let runtime = ThemeRuntime::new(theme(), ThemeAssets::new(), vec![cache.clone()]).unwrap();
    let raw = root.path().join("composed.rgb");
    let mut output = std::fs::File::create(&raw).unwrap();
    for (index, bytes) in decoded.stdout.chunks_exact(FRAME_BYTES).enumerate() {
        let text = match index {
            0 => {
                publish(&cache, 40.0);
                "CPU 40.0°C"
            }
            1 => {
                publish(&cache, 80.0);
                "CPU 80.0°C"
            }
            2 => {
                cache.publish(std::time::Instant::now(), vec![]);
                "CPU —"
            }
            _ => {
                publish(&cache, 50.0);
                "CPU 50.0°C"
            }
        };
        let frame = RgbImage::from_raw(480, 480, bytes.to_vec()).unwrap();
        let composed = runtime
            .compose_video_frame(frame.clone(), index as u64)
            .unwrap();
        let mut expected = theme();
        let ThemeLayer::Sensor { style, .. } = expected.layers.remove(0) else {
            unreachable!()
        };
        expected.layers.push(ThemeLayer::Text {
            editor: None,
            text: text.into(),
            style,
        });
        let explicit = render_theme_over_frame(
            &expected,
            &[],
            ClockTime {
                year: 2026,
                month: 10,
                day: 12,
                hour: 0,
                minute: 0,
                second: 0,
            },
            &ThemeAssets::new(),
            &ThemeFonts::new(),
            index as u64,
            frame.clone(),
        )
        .unwrap();
        assert!(
            composed == explicit,
            "sensor overlay mismatch on frame {index}"
        );
        assert!(composed != frame, "missing overlay on frame {index}");
        assert_eq!(composed.get_pixel(479, 479), frame.get_pixel(479, 479));
        use std::io::Write;
        output.write_all(composed.as_raw()).unwrap();
    }
    drop(output);
    let target = root.path().join("composed.h264");
    let encoded = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            "480x480",
            "-framerate",
            "24",
            "-i",
        ])
        .arg(&raw)
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-profile:v",
            "baseline",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
        ])
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        encoded.status.success(),
        "{}",
        String::from_utf8_lossy(&encoded.stderr)
    );
    let decoded = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(&target)
        .args(["-threads", "1", "-f", "framemd5", "-"])
        .output()
        .unwrap();
    assert!(decoded.status.success());
    let frames = String::from_utf8(decoded.stdout).unwrap();
    assert_eq!(
        frames
            .lines()
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
            .count(),
        4
    );
}

#[tokio::test]
async fn weather_runtime_uses_live_cache_and_preview_does_not_advance_stream_motion() {
    let cache = SensorCache::default();
    publish(&cache, 20.0);
    let theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Weather","background":[0,0,0],"layers":[{"kind":"weather","effect":{"kind":"snow","intensity":1,"dynamic":true,"seed":42,"sensor":{"sensor_id":"cpu","unit":"celsius","low":20,"high":80}}}]})).unwrap();
    let runtime =
        Arc::new(ThemeRuntime::new(theme, ThemeAssets::new(), vec![cache.clone()]).unwrap());
    let first = runtime.compose_frame_at_millis(None, 2500).unwrap();
    publish(&cache, 44.0);
    let changed = runtime.compose_frame_at_millis(None, 2500).unwrap();
    assert_eq!(first, changed, "Threshold must not teleport flakes");
    let preview = runtime.frame_at(99).await.unwrap();
    assert!(preview.starts_with(&[0xff, 0xd8]));
    assert_eq!(
        runtime.compose_frame_at_millis(None, 2500).unwrap(),
        changed,
        "Preview must not advance streaming timeline"
    );
    assert_ne!(
        runtime.compose_frame_at_millis(None, 3500).unwrap(),
        changed
    );
}

#[tokio::test]
async fn meteor_live_sources_change_palette_and_preview_keeps_stream_timeline() {
    let cache = SensorCache::default();
    publish(&cache, 20.0);
    let theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Meteor","background":[0,0,0],"layers":[{"kind":"meteor","effect":{"speed":2,"colors":[[0,255,0],[0,0,255]],"dynamic":true,"seed":42,"sensor":{"binding":{"sensor_id":"cpu","unit":"celsius","low":30,"high":60},"low_colors":[[255,0,0],[255,0,0]],"high_colors":[[0,0,255],[0,0,255]]}}}]})).unwrap();
    let runtime =
        Arc::new(ThemeRuntime::new(theme, ThemeAssets::new(), vec![cache.clone()]).unwrap());
    let low = runtime.compose_frame_at_millis(None, 2500).unwrap();
    publish(&cache, 100.0);
    let high = runtime.compose_frame_at_millis(None, 2500).unwrap();
    assert_ne!(low, high, "live palette must change");
    assert!(high.pixels().any(|p| p[2] > p[0]));
    let preview = runtime.frame_at(99).await.unwrap();
    assert!(preview.starts_with(&[0xff, 0xd8]));
    assert_eq!(
        runtime.compose_frame_at_millis(None, 2500).unwrap(),
        high,
        "preview advanced stream"
    );
    assert_ne!(runtime.compose_frame_at_millis(None, 3500).unwrap(), high);
}

#[tokio::test]
async fn snow3d_live_frames_keep_millisecond_motion_and_preview_is_independent() {
    let theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Snow3D","background":[12,18,30],"layers":[{"kind":"snow3d","effect":{"intensity":1,"seed":42}}]})).unwrap();
    let runtime = Arc::new(ThemeRuntime::new(theme, ThemeAssets::new(), vec![]).unwrap());
    let first = runtime.compose_frame_at_millis(None, 0).unwrap();
    let next = runtime.compose_frame_at_millis(None, 42).unwrap();
    assert_ne!(first, next, "motion must not be truncated to whole seconds");
    let preview = runtime.frame_at(99).await.unwrap();
    assert!(preview.starts_with(&[0xff, 0xd8]));
    assert_eq!(runtime.compose_frame_at_millis(None, 42).unwrap(), next);
}

#[tokio::test]
async fn fireworks_live_palette_changes_preserve_existing_launches_and_preview_timeline() {
    let cache = SensorCache::default();
    publish(&cache, 20.0);
    let theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Fireworks","background":[12,18,30],"layers":[{"kind":"fireworks","effect":{"intensity":1,"colors":FIREWORKS_COLORS,"dynamic":true,"seed":42,"sensor":{"binding":{"sensor_id":"cpu","unit":"celsius","low":30,"high":60},"low_colors":vec![[255,0,0];6],"high_colors":vec![[0,0,255];6]}}}]})).unwrap();
    let runtime =
        Arc::new(ThemeRuntime::new(theme, ThemeAssets::new(), vec![cache.clone()]).unwrap());
    let first = runtime.compose_frame_at_millis(None, 2500).unwrap();
    publish(&cache, 100.0);
    assert_eq!(
        runtime.compose_frame_at_millis(None, 2500).unwrap(),
        first,
        "existing particles recolored"
    );
    let preview = runtime.frame_at(99).await.unwrap();
    assert!(preview.starts_with(&[0xff, 0xd8]));
    assert_eq!(
        runtime.compose_frame_at_millis(None, 2500).unwrap(),
        first,
        "preview advanced simulation"
    );
    assert_ne!(runtime.compose_frame_at_millis(None, 3000).unwrap(), first);
}

#[tokio::test]
async fn global_temperature_changes_live_pixels_without_reapplying_or_rewriting_explicit_themes() {
    let root = tempfile::tempdir().unwrap();
    let store =
        llcontrol_daemon::display_settings::DisplaySettingsStore::new(root.path().to_path_buf());
    let cache = SensorCache::default();
    publish(&cache, 40.0);
    let mut system = theme();
    if let ThemeLayer::Sensor {
        temperature_unit, ..
    } = &mut system.layers[0]
    {
        *temperature_unit = TemperatureUnit::System;
    }
    let runtime = ThemeRuntime::new(system.clone(), ThemeAssets::new(), vec![cache.clone()])
        .unwrap()
        .with_display_unit(store.live_unit());
    let fixed = ThemeRuntime::new(theme(), ThemeAssets::new(), vec![cache.clone()])
        .unwrap()
        .with_display_unit(store.live_unit());
    let c = runtime.compose_frame_at_millis(None, 0).unwrap();
    assert_eq!(c, fixed.compose_frame_at_millis(None, 0).unwrap());
    let mut settings = store.get().await.unwrap();
    settings.temperature_unit = DisplayTemperatureUnit::Fahrenheit;
    store.save(settings).await.unwrap();
    let f = runtime.compose_frame_at_millis(None, 0).unwrap();
    assert_ne!(c, f);
    assert_eq!(runtime.theme(), &system);
    assert_eq!(fixed.compose_frame_at_millis(None, 0).unwrap(), c);
    let mut fahrenheit = theme();
    if let ThemeLayer::Sensor {
        temperature_unit, ..
    } = &mut fahrenheit.layers[0]
    {
        *temperature_unit = TemperatureUnit::Fahrenheit;
    }
    let reference = ThemeRuntime::new(fahrenheit, ThemeAssets::new(), vec![cache.clone()]).unwrap();
    assert_eq!(f, reference.compose_frame_at_millis(None, 0).unwrap());
    assert_eq!(cache.readings()[0].value, Some(40.0));
}
