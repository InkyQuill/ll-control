use verify_galahad::probe::{parse_hex_payload, Framing, ProbeArgs};

#[test]
fn parse_hex_payload_accepts_lower_case_no_prefix() {
    assert_eq!(parse_hex_payload("aabb").unwrap(), vec![0xaa, 0xbb]);
}

#[test]
fn parse_hex_payload_accepts_upper_case_with_spaces() {
    assert_eq!(parse_hex_payload("AA BB").unwrap(), vec![0xaa, 0xbb]);
}

#[test]
fn parse_hex_payload_accepts_0x_prefix() {
    assert_eq!(parse_hex_payload("0xaa 0xbb").unwrap(), vec![0xaa, 0xbb]);
}

#[test]
fn parse_hex_payload_accepts_empty_string() {
    assert_eq!(parse_hex_payload("").unwrap(), Vec::<u8>::new());
}

#[test]
fn parse_hex_payload_rejects_odd_length() {
    assert!(parse_hex_payload("aabbc").is_err());
}

#[test]
fn parse_hex_payload_rejects_non_hex() {
    assert!(parse_hex_payload("aazz").is_err());
}

#[test]
fn framing_a_buffer_size_is_64() {
    assert_eq!(Framing::A.output_len(), 64);
    assert_eq!(Framing::A.input_len(), 64);
}

#[test]
fn framing_b_buffer_size_is_1024_out_512_in() {
    assert_eq!(Framing::B.output_len(), 1024);
    assert_eq!(Framing::B.input_len(), 512);
}

#[test]
fn framing_c_buffer_size_is_512_out_512_in() {
    assert_eq!(Framing::C.output_len(), 512);
    assert_eq!(Framing::C.input_len(), 512);
}

#[test]
fn framing_a_report_id_is_1() {
    assert_eq!(Framing::A.report_id(), 0x01);
}

#[test]
fn framing_b_report_id_is_2() {
    assert_eq!(Framing::B.report_id(), 0x02);
}

#[test]
fn framing_c_report_id_is_3() {
    assert_eq!(Framing::C.report_id(), 0x03);
}

#[test]
fn probe_args_holds_cmd_framing_payload() {
    let args = ProbeArgs {
        cmd: 0x86,
        framing: Framing::A,
        payload: vec![0x01, 0x02],
    };
    assert_eq!(args.cmd, 0x86);
    assert_eq!(args.framing.output_len(), 64);
    assert_eq!(args.payload, vec![0x01, 0x02]);
}
