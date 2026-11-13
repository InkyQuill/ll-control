use lconnect_hid::FamilyRegistry;

#[test]
fn empty_registry_finds_nothing() {
    let r = FamilyRegistry::new();
    assert!(r.lookup_vid_pid(0x0416, 0x7395).is_none());
}

#[test]
fn default_registry_has_lcdv2() {
    let r = FamilyRegistry::with_default_lcdv2();
    let f = r.lookup_vid_pid(0x0416, 0x7395).unwrap();
    assert_eq!(f.id().as_str(), "galahad2-lcdv2");
}

#[test]
fn default_registry_does_not_match_unknown_vid_pid() {
    let r = FamilyRegistry::with_default_lcdv2();
    assert!(r.lookup_vid_pid(0x1234, 0x5678).is_none());
}

#[test]
fn default_registry_lcdv2_display_name() {
    let r = FamilyRegistry::with_default_lcdv2();
    let f = r.lookup_vid_pid(0x0416, 0x7395).unwrap();
    assert_eq!(f.display_name(), "Galahad II LCDV2");
}

#[test]
fn default_registry_lcdv2_vid_pid_list() {
    let r = FamilyRegistry::with_default_lcdv2();
    let f = r.lookup_vid_pid(0x0416, 0x7395).unwrap();
    assert_eq!(f.vid_pids(), &[(0x0416, 0x7395)]);
}

#[test]
fn registry_is_default_lcdv2_via_default_trait() {
    // FamilyRegistry: Default returns the with_default_lcdv2() registry.
    let r = FamilyRegistry::default();
    assert!(r.lookup_vid_pid(0x0416, 0x7395).is_some());
}

#[test]
fn tl_fan_and_tl_lcd_are_not_misidentified_as_galahad_lcdv2() {
    // Windows 2.1.28 KnownDevices: TL hub and independent TL LCD endpoint.
    let registry = FamilyRegistry::default();
    for (vid, pid) in [(0x0416, 0x7372), (0x04fc, 0x7393), (0x0416, 0x7391)] {
        assert!(registry.lookup_vid_pid(vid, pid).is_none());
    }
}
