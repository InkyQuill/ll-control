use llcontrol_daemon::config::Config;

#[test]
fn defaults_match_spec() {
    let c = Config::default();
    assert_eq!(c.daemon.log_level, "info");
    assert!(!c.daemon.allow_raw_send);
    assert_eq!(c.poll.hotplug_ms, 500);
    assert_eq!(c.poll.sensor_hz, 1);
    assert_eq!(c.lcd.refresh_ms, 15_000);
    assert_eq!(c.lighting.reissue_ms, 1_500);
}

#[test]
fn parses_partial_toml() {
    let toml_text = r#"
[daemon]
log_level = "debug"

[lcd]
refresh_ms = 12000
"#;
    let c: Config = toml::from_str(toml_text).unwrap();
    assert_eq!(c.daemon.log_level, "debug");
    assert_eq!(c.lcd.refresh_ms, 12000);
    // Other fields fall back to default
    assert_eq!(c.poll.sensor_hz, 1);
}
