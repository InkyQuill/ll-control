use llcontrol_hid::{DeviceInfo, MockBackend};

// The real transport drains stale reads before a command. Queue the firmware
// reply only when that command is written, exactly as a device would answer.
pub struct FirmwareMock(pub MockBackend);
struct FirmwareHandle {
    inner: Box<dyn llcontrol_hid::OpenHandle>,
    mock: MockBackend,
    info: DeviceInfo,
}
impl llcontrol_hid::HidBackend for FirmwareMock {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        self.0.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn llcontrol_hid::OpenHandle>> {
        Ok(Box::new(FirmwareHandle {
            inner: self.0.open(info)?,
            mock: self.0.clone(),
            info: info.clone(),
        }))
    }
}
impl llcontrol_hid::OpenHandle for FirmwareHandle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        self.inner.write(bytes)?;
        if bytes.starts_with(&[1, 134]) {
            let packet = llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
                134,
                b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
            );
            self.mock
                .queue_response(&self.info, packet.as_bytes().to_vec());
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], timeout_ms: i32) -> llcontrol_hid::Result<usize> {
        self.inner.read_timeout(bytes, timeout_ms)
    }
}
