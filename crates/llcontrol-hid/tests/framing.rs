use llcontrol_hid::Framing;

#[test]
fn framing_a_constants() {
    assert_eq!(Framing::A.output_len(), 64);
    assert_eq!(Framing::A.input_len(), 64);
    assert_eq!(Framing::A.report_id(), 0x01);
}

#[test]
fn framing_b_constants() {
    assert_eq!(Framing::B.output_len(), 1024);
    assert_eq!(Framing::B.input_len(), 512);
    assert_eq!(Framing::B.report_id(), 0x02);
}

#[test]
fn framing_c_constants() {
    assert_eq!(Framing::C.output_len(), 512);
    assert_eq!(Framing::C.input_len(), 512);
    assert_eq!(Framing::C.report_id(), 0x03);
}

#[test]
fn framing_eq_and_hash() {
    use std::collections::HashMap;
    let mut m: HashMap<Framing, u8> = HashMap::new();
    m.insert(Framing::A, 1);
    m.insert(Framing::B, 2);
    m.insert(Framing::C, 3);
    assert_eq!(m[&Framing::B], 2);
    assert_eq!(Framing::A, Framing::A);
    assert_ne!(Framing::A, Framing::B);
}

#[test]
fn dyn_compat_assertions() {
    // Compile-time guarantees: HidBackend and OpenHandle must be
    // dyn-compatible so the daemon can hold them as trait objects.
    fn _assert_object_safe<T: ?Sized>() {}
    _assert_object_safe::<dyn llcontrol_hid::HidBackend>();
    _assert_object_safe::<dyn llcontrol_hid::OpenHandle>();
}

#[test]
fn matches_echo_rejects_short_buffer() {
    use llcontrol_hid::{matches_echo, Framing};
    assert!(!matches_echo(&[], Framing::A, 0x01));
    assert!(!matches_echo(&[0x01], Framing::A, 0x01)); // 1 byte: no cmd byte
}
