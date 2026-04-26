use lconnect_daemon::stats::Stats;

#[test]
fn counters_start_at_zero() {
    let s = Stats::new();
    let snap = s.snapshot(0);
    assert_eq!(snap.hid_write_count, 0);
    assert_eq!(snap.devices_known, 0);
    assert_eq!(snap.ipc_request_count, 0);
}

#[test]
fn increments_are_visible_in_snapshot() {
    let s = Stats::new();
    s.inc_hid_write();
    s.inc_hid_write();
    s.inc_ipc_request();
    let snap = s.snapshot(2);
    assert_eq!(snap.hid_write_count, 2);
    assert_eq!(snap.ipc_request_count, 1);
    assert_eq!(snap.devices_known, 2);
}

#[test]
fn uptime_increases() {
    let s = Stats::new();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let snap = s.snapshot(0);
    assert!(snap.uptime_seconds >= 1, "got {}", snap.uptime_seconds);
}
