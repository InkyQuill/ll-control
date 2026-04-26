use lconnect_hid::mock_backend::MockBackend;
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend};
use std::path::PathBuf;

fn lcdv2_info() -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        PathBuf::from("/mock/hidraw0"),
    )
}

#[test]
fn enumerate_returns_registered_devices() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let list = mock.enumerate().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].family.as_str(), "galahad2-lcdv2");
}

#[test]
fn enumerate_empty_when_no_devices() {
    let mock = MockBackend::new();
    let list = mock.enumerate().unwrap();
    assert!(list.is_empty());
}

#[test]
fn open_returns_handle_for_registered_device() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    assert!(mock.open(&lcdv2_info()).is_ok());
}

#[test]
fn open_unregistered_returns_device_not_found() {
    use lconnect_hid::Error;
    let mock = MockBackend::new();
    match mock.open(&lcdv2_info()) {
        Err(Error::DeviceNotFound(_)) => {}
        Err(other) => panic!("expected DeviceNotFound, got {other:?}"),
        Ok(_) => panic!("expected DeviceNotFound, got Ok(handle)"),
    }
}

#[test]
fn handle_records_writes() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let handle = mock.open(&lcdv2_info()).unwrap();
    handle.write(&[0x01, 0x86]).unwrap();
    handle.write(&[0x01, 0x87, 0xAA]).unwrap();
    let history = mock.write_history(&lcdv2_info());
    assert_eq!(history.len(), 2);
    assert_eq!(history[0], vec![0x01, 0x86]);
    assert_eq!(history[1], vec![0x01, 0x87, 0xAA]);
}

#[test]
fn handle_replays_canned_responses_in_order() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    mock.queue_response(&lcdv2_info(), vec![0x01, 0x86, 0xAA]);
    mock.queue_response(&lcdv2_info(), vec![0x01, 0x87, 0xBB]);

    let handle = mock.open(&lcdv2_info()).unwrap();
    let mut buf = [0u8; 64];
    let n = handle.read_timeout(&mut buf, 100).unwrap();
    assert_eq!(&buf[..n], &[0x01, 0x86, 0xAA]);

    let n = handle.read_timeout(&mut buf, 100).unwrap();
    assert_eq!(&buf[..n], &[0x01, 0x87, 0xBB]);
}

#[test]
fn read_timeout_returns_zero_when_no_responses() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let handle = mock.open(&lcdv2_info()).unwrap();
    let mut buf = [0u8; 64];
    let n = handle.read_timeout(&mut buf, 100).unwrap();
    assert_eq!(n, 0, "expected timeout signal (Ok(0))");
}

#[test]
fn write_history_for_unregistered_returns_empty() {
    let mock = MockBackend::new();
    let history = mock.write_history(&lcdv2_info());
    assert!(history.is_empty());
}
