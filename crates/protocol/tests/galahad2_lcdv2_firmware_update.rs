//! Synthetic source-derived envelopes; no vendor bytes or device access.
use llcontrol_protocol::galahad2_lcdv2::{
    bpacket::{BCommandPacket, INPUT_LEN, PAYLOAD_MAX},
    firmware_update::{decode_activation, ChunkExpectation, DecodeError, ResetRequested},
};

fn echo(packet: &BCommandPacket) -> [u8; INPUT_LEN] {
    packet.as_bytes()[..INPUT_LEN].try_into().unwrap()
}

fn rejection(packet: &BCommandPacket, status: u8) -> [u8; INPUT_LEN] {
    let mut response = echo(packet);
    response[1] = 2;
    response[6] = status;
    response
}

fn activation() -> [u8; INPUT_LEN] {
    let mut response = echo(&BCommandPacket::new_request(10, 0, &[1], 1));
    response[12] = 1;
    response
}

#[test]
fn all_619_chunks_of_researched_image_size_use_512_byte_echo_not_declared_payload_extent() {
    let image = vec![0x6d; 626_212];
    let packets: Vec<_> = BCommandPacket::build_packets_for_write(11, &image).collect();
    assert_eq!(packets.len(), 619);
    for (index, packet) in packets.iter().enumerate() {
        let response = echo(packet);
        let reply = ChunkExpectation::new(packet)
            .unwrap()
            .decode(&response)
            .unwrap();
        assert_eq!(reply.packet_number as usize, index);
        assert_eq!(
            reply.end_offset as usize,
            ((index + 1) * PAYLOAD_MAX).min(image.len())
        );
        assert_eq!(reply.is_last, index == packets.len() - 1);
        if index < packets.len() - 1 {
            assert!(BCommandPacket::from_response(&response, 11).is_err());
        }
    }
    assert_eq!(&packets[618].as_bytes()[9..11], &178_u16.to_be_bytes());
}

#[test]
fn malformed_chunk_expectations_cannot_be_used_to_admit_replies() {
    for packet in [
        BCommandPacket::new_request(12, 0, &[1], 1),
        BCommandPacket::new_request(11, 0, &[], 0),
        BCommandPacket::new_request(11, 1, &[1], 1),
        BCommandPacket::new_request(11, 0, &[1], 2),
        BCommandPacket::new_request(11, 0, &[1, 2], 1),
        BCommandPacket::new_request(11, 0xff_ffff, &[1], u32::MAX),
    ] {
        assert_eq!(
            ChunkExpectation::new(&packet).unwrap_err(),
            DecodeError::InvalidChunk
        );
    }
}

#[test]
fn actual_extent_is_required_even_when_header_and_declared_payload_fit() {
    let packet = BCommandPacket::new_request(11, 0, &[1], 1);
    let expected = ChunkExpectation::new(&packet).unwrap();
    let response = echo(&packet);
    for length in 0..INPUT_LEN {
        assert_eq!(
            expected.decode(&response[..length]),
            Err(DecodeError::ResponseExtent(length))
        );
        assert_eq!(
            decode_activation(&activation()[..length]),
            Err(DecodeError::ResponseExtent(length))
        );
    }
    let mut oversize = response.to_vec();
    oversize.push(0);
    assert_eq!(
        expected.decode(&oversize),
        Err(DecodeError::ResponseExtent(513))
    );
    assert_eq!(
        decode_activation(&oversize),
        Err(DecodeError::ResponseExtent(513))
    );
}

#[test]
fn every_corrupted_byte_in_chunk_echo_or_activation_reply_is_rejected() {
    let packet = BCommandPacket::new_request(11, 0, &[7; 17], 17);
    let expected = ChunkExpectation::new(&packet).unwrap();
    for index in 0..INPUT_LEN {
        let mut response = echo(&packet);
        response[index] ^= 1;
        assert!(expected.decode(&response).is_err(), "chunk byte {index}");
        let mut response = activation();
        response[index] ^= 1;
        assert!(
            decode_activation(&response).is_err(),
            "activation byte {index}"
        );
    }
}

#[test]
fn prior_packet_and_other_image_size_or_prefix_do_not_match_pending_chunk() {
    let bytes = vec![5; 2100];
    let packets: Vec<_> = BCommandPacket::build_packets_for_write(11, &bytes).collect();
    let pending = ChunkExpectation::new(&packets[1]).unwrap();
    assert_eq!(
        pending.decode(&echo(&packets[0])),
        Err(DecodeError::UnmatchedReply)
    );
    let other_size = BCommandPacket::new_request(11, 1, &[5; 1013], 2101);
    assert_eq!(
        pending.decode(&echo(&other_size)),
        Err(DecodeError::UnmatchedReply)
    );
    let other_prefix = BCommandPacket::new_request(11, 1, &[6; 1013], 2100);
    assert_eq!(
        pending.decode(&echo(&other_prefix)),
        Err(DecodeError::UnmatchedReply)
    );
}

#[test]
fn all_observed_and_unknown_high_bit_device_errors_remain_errors() {
    let packet = BCommandPacket::new_request(11, 1, &[5; 1013], 2100);
    let pending = ChunkExpectation::new(&packet).unwrap();
    let activate = BCommandPacket::new_request(10, 0, &[1], 1);
    for status in [
        0x80, 0x81, 0x82, 0x83, 0x84, 0x86, 0x87, 0x8a, 0x8b, 0xfe, 0xff,
    ] {
        assert_eq!(
            pending.decode(&rejection(&packet, status)),
            Err(DecodeError::DeviceRejected(status))
        );
        assert_eq!(
            decode_activation(&rejection(&activate, status)),
            Err(DecodeError::DeviceRejected(status))
        );
    }
    for status in [0, 1, 0x7f] {
        assert_eq!(
            pending.decode(&rejection(&packet, status)),
            Err(DecodeError::MalformedDeviceError(status))
        );
    }
}

#[test]
fn unrelated_errors_and_response_shape_changes_cannot_advance_a_transfer() {
    let packet = BCommandPacket::new_request(11, 1, &[5; 1013], 2100);
    let pending = ChunkExpectation::new(&packet).unwrap();
    for index in [0, 2, 3, 4, 5, 7, 8, 9, 10, 11, 511] {
        let mut response = rejection(&packet, 0x84);
        response[index] ^= 1;
        assert_eq!(
            pending.decode(&response),
            Err(DecodeError::UnmatchedReply),
            "byte {index}"
        );
    }
    let activate = BCommandPacket::new_request(10, 0, &[1], 1);
    assert!(pending.decode(&rejection(&activate, 0x84)).is_err());
    assert!(decode_activation(&rejection(&packet, 0x84)).is_err());
}

#[test]
fn overwritten_high_packet_number_is_never_presented_as_correlated_error() {
    let size = (65_536 + 1) * 1013;
    let packet = BCommandPacket::new_request(11, 65_536, &[5; 1013], size);
    let expected = ChunkExpectation::new(&packet).unwrap();
    assert_eq!(
        expected.decode(&rejection(&packet, 0x84)),
        Err(DecodeError::AmbiguousErrorPacketNumber)
    );
    assert_eq!(
        expected.decode(&echo(&packet)).unwrap().packet_number,
        65_536
    );
}

#[test]
fn activation_requires_raw_extra_flag_beyond_declared_single_byte_payload() {
    let reply = activation();
    let generic = BCommandPacket::from_response(&reply, 10).unwrap();
    assert_eq!(generic.length(), 1);
    assert_eq!(generic.data(), &[1]);
    assert_eq!(decode_activation(&reply), Ok(ResetRequested));
    let mut no_extra_flag = reply;
    no_extra_flag[12] = 0;
    assert_eq!(
        decode_activation(&no_extra_flag),
        Err(DecodeError::UnmatchedReply)
    );
    let mut declared_two = reply;
    declared_two[10] = 2;
    assert_eq!(
        decode_activation(&declared_two),
        Err(DecodeError::UnmatchedReply)
    );
}

#[test]
fn identical_late_echo_and_unechoed_chunk_tail_are_explicitly_not_authenticated() {
    let data = [7; 1013];
    let packet = BCommandPacket::new_request(11, 0, &data, 1013);
    let mut different_tail = data;
    different_tail[1000] = 8;
    let other = BCommandPacket::new_request(11, 0, &different_tail, 1013);
    let pending = ChunkExpectation::new(&other).unwrap();
    assert_ne!(packet.as_bytes(), other.as_bytes());
    // Wire evidence cannot distinguish these requests, nor a previous identical
    // transaction. The eventual owner must enforce attachment/epoch/one-flight
    // ownership and preserve ambiguity; this codec cannot invent a nonce/hash.
    assert_eq!(
        pending.decode(&echo(&packet)),
        pending.decode(&echo(&other))
    );
}
