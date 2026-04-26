use lconnect_protocol::galahad2_lcdv2::cpacket::{
    CCommandPacket, CPacketChunks, HEADER_LEN, INPUT_LEN, OUTPUT_LEN, PAYLOAD_MAX, REPORT_ID,
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
    let chunks: Vec<_> = CPacketChunks::new(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 3);
}

#[test]
fn chunks_iterator_handles_empty_payload() {
    let chunks: Vec<_> = CPacketChunks::new(0x0D, &[]).collect();
    assert_eq!(
        chunks.len(),
        1,
        "empty payload still emits one zero-length chunk"
    );
}

#[test]
fn chunks_iterator_handles_exact_multiples_of_payload_max() {
    let payload = vec![0u8; PAYLOAD_MAX];
    let chunks: Vec<_> = CPacketChunks::new(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 1);

    let payload = vec![0u8; PAYLOAD_MAX * 2];
    let chunks: Vec<_> = CPacketChunks::new(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 2);
}
