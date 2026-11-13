pub fn reply() -> llcontrol_daemon::actor::FirmwareResponses {
    let packet = llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
        134,
        b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
    );
    llcontrol_daemon::actor::FirmwareResponses {
        version: *packet.as_bytes(),
        datetime: None,
    }
}
