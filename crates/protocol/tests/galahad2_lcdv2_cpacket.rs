use lconnect_protocol::galahad2_lcdv2::cpacket::{
    CCommandPacket, HEADER_LEN, INPUT_LEN, OUTPUT_LEN, PAYLOAD_MAX, REPORT_ID,
};

#[test]
fn cpacket_constants() {
    assert_eq!(OUTPUT_LEN, 512);
    assert_eq!(INPUT_LEN, 512);
    assert_eq!(HEADER_LEN, 11);
    assert_eq!(PAYLOAD_MAX, 501);
    assert_eq!(REPORT_ID, 0x03);
}

#[test]
fn new_request_writes_header_correctly() {
    let payload = [0xAA, 0xBB, 0xCC];
    let pkt = CCommandPacket::new_request(0x0D, 1, &payload, payload.len() as u32);
    let bytes = pkt.as_bytes();
    assert_eq!(bytes[0], REPORT_ID);
    assert_eq!(bytes[1], 0x0D);
    // BE u32 DataSize at offset 2
    assert_eq!(&bytes[2..6], &(payload.len() as u32).to_be_bytes());
    // BE u24 PacketNumber = 1, written into 3 bytes at offset 6
    assert_eq!(&bytes[6..9], &[0x00, 0x00, 0x01]);
    // BE u16 Length at offset 9
    assert_eq!(&bytes[9..11], &(payload.len() as u16).to_be_bytes());
    // payload at offset 11
    assert_eq!(&bytes[11..14], &payload);
    // rest is zero-filled
    assert!(bytes[14..].iter().all(|&b| b == 0));
}

#[test]
fn from_response_parses_512_bytes_with_correct_echo() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = 0x0D;
    let resp = CCommandPacket::from_response(&buf, 0x0D).unwrap();
    assert_eq!(resp.command(), 0x0D);
}

#[test]
fn from_response_rejects_short_buffer() {
    let buf = [0u8; 100];
    assert!(CCommandPacket::from_response(&buf, 0x0D).is_err());
}

#[test]
fn from_response_rejects_wrong_command_echo() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = 0x99;
    assert!(CCommandPacket::from_response(&buf, 0x0D).is_err());
}

#[test]
fn chunks_iterator_yields_packet_per_chunk() {
    let payload = vec![0u8; PAYLOAD_MAX * 2 + 1];
    let chunks: Vec<_> = CCommandPacket::build_packets_for_write(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 3);
}

#[test]
fn chunks_iterator_handles_empty_payload() {
    let chunks: Vec<_> = CCommandPacket::build_packets_for_write(0x0D, &[]).collect();
    assert_eq!(
        chunks.len(),
        1,
        "empty payload still emits one zero-length chunk"
    );
}

#[test]
fn chunks_iterator_handles_exact_multiples_of_payload_max() {
    let payload = vec![0u8; PAYLOAD_MAX];
    let chunks: Vec<_> = CCommandPacket::build_packets_for_write(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 1);

    let payload = vec![0u8; PAYLOAD_MAX * 2];
    let chunks: Vec<_> = CCommandPacket::build_packets_for_write(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 2);
}

#[test]
fn from_response_rejects_wrong_report_id() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = 0x99; // not REPORT_ID = 0x03
    buf[1] = 0x0D;
    let r = CCommandPacket::from_response(&buf, 0x0D);
    assert!(r.is_err(), "expected error, got: {r:?}");
}

#[test]
fn from_response_rejects_length_exceeding_payload_max() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = 0x0D;
    // Length field at offset 9..11 = PAYLOAD_MAX + 1
    let bad_len: u16 = (PAYLOAD_MAX as u16) + 1;
    buf[9..11].copy_from_slice(&bad_len.to_be_bytes());
    let r = CCommandPacket::from_response(&buf, 0x0D);
    assert!(r.is_err(), "expected InvalidLength error, got: {r:?}");
}

#[test]
fn from_response_decodes_all_fields() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = 0x0D;
    buf[2..6].copy_from_slice(&0x12345678u32.to_be_bytes()); // data_size
    buf[6..9].copy_from_slice(&[0x00, 0xAB, 0xCD]); // packet_number = 0xABCD (24-bit BE)
    buf[9..11].copy_from_slice(&3u16.to_be_bytes()); // length
    buf[11..14].copy_from_slice(&[0xDE, 0xAD, 0xBE]); // payload
    let resp = CCommandPacket::from_response(&buf, 0x0D).unwrap();
    assert_eq!(resp.command(), 0x0D);
    assert_eq!(resp.data_size(), 0x12345678);
    assert_eq!(resp.packet_number(), 0xABCD);
    assert_eq!(resp.length(), 3);
    assert_eq!(resp.data(), &[0xDE, 0xAD, 0xBE]);
}

#[test]
fn new_request_silently_truncates_payload_over_max() {
    // Mirrors C# Math.Min semantics. Caller responsibility is to chunk
    // via build_packets_for_write; new_request itself does not panic.
    let huge = vec![0xAB; PAYLOAD_MAX + 100];
    let pkt = CCommandPacket::new_request(0x0D, 1, &huge, huge.len() as u32);
    let bytes = pkt.as_bytes();
    // Length field caps at PAYLOAD_MAX
    let len = u16::from_be_bytes([bytes[9], bytes[10]]) as usize;
    assert_eq!(len, PAYLOAD_MAX);
    // Only PAYLOAD_MAX bytes copied into payload region
    assert_eq!(
        &bytes[HEADER_LEN..HEADER_LEN + PAYLOAD_MAX],
        &huge[..PAYLOAD_MAX]
    );
}

#[test]
fn new_request_payload_at_payload_max_fills_buffer() {
    let payload = vec![0xCD; PAYLOAD_MAX];
    let pkt = CCommandPacket::new_request(0x0D, 1, &payload, PAYLOAD_MAX as u32);
    let bytes = pkt.as_bytes();
    assert_eq!(
        u16::from_be_bytes([bytes[9], bytes[10]]) as usize,
        PAYLOAD_MAX
    );
    assert_eq!(&bytes[HEADER_LEN..], &payload[..]); // fills to end of OUTPUT_LEN
}

#[test]
fn chunks_replicate_data_size_and_increment_packet_number() {
    // Two-chunk payload: first chunk should have packet_number=0; second=1;
    // both should carry data_size = total payload length. (Matches the
    // 0-based numbering used by BPacketChunks.)
    let payload = vec![0u8; PAYLOAD_MAX + 50];
    let chunks: Vec<_> = CCommandPacket::build_packets_for_write(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 2);

    let total = payload.len() as u32;
    for (idx, pkt) in chunks.iter().enumerate() {
        let bytes = pkt.as_bytes();
        let data_size = u32::from_be_bytes(bytes[2..6].try_into().unwrap());
        let pn = u32::from_be_bytes([0x00, bytes[6], bytes[7], bytes[8]]);
        assert_eq!(data_size, total, "chunk {idx} data_size");
        assert_eq!(pn as usize, idx, "chunk {idx} packet_number (0-based)");
    }
}
