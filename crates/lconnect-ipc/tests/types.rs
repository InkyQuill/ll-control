use lconnect_ipc::*;

#[test]
fn device_id_roundtrip() {
    let id = DeviceId::new("galahad2-lcdv2:0001");
    let s = serde_json::to_string(&id).unwrap();
    let d: DeviceId = serde_json::from_str(&s).unwrap();
    assert_eq!(d, id);
}

#[test]
fn pwm_validates_range() {
    assert!(Pwm::new(0).is_ok());
    assert!(Pwm::new(100).is_ok());
    assert!(Pwm::new(101).is_err());
}

#[test]
fn pwm_source_roundtrip() {
    let s = serde_json::to_string(&PwmSource::Mcu).unwrap();
    assert_eq!(s, r#""mcu""#);
}

#[test]
fn rgb_roundtrip() {
    let c = Rgb {
        r: 255,
        g: 0,
        b: 64,
    };
    let s = serde_json::to_string(&c).unwrap();
    let d: Rgb = serde_json::from_str(&s).unwrap();
    assert_eq!(d, c);
}

#[test]
fn ipc_error_codes() {
    assert_eq!(IpcError::DeviceNotFound("x".into()).jsonrpc_code(), -32000);
    assert_eq!(IpcError::InvalidArgument("y".into()).jsonrpc_code(), -32002);
    assert_eq!(IpcError::ConfirmationRequired.jsonrpc_code(), -32004);
}
