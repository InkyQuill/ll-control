//! `ACommandPacket` — the standard 64-byte request/response packet.
//!
//! Source: `LConnectCore/Products/Galahad2Vision/ACommandPacket.cs`.

/// Total HID packet length on the wire.
pub const PACKET_LEN: usize = 64;
/// Number of header bytes preceding the data payload.
pub const HEADER_LEN: usize = 6;
/// Maximum data bytes per packet.
pub const MAX_PAYLOAD: usize = PACKET_LEN - HEADER_LEN;
/// HID Report ID byte.
pub const REPORT_ID: u8 = 0x01;

/// A standard 64-byte command packet (`ACommandPacket` in the C# source).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ACommandPacket {
    bytes: [u8; PACKET_LEN],
}

impl ACommandPacket {
    /// Build a request packet from a command byte and up to 58 data bytes.
    ///
    /// If `data` is longer than 58 bytes it is **truncated** silently (this
    /// matches the C# `Math.Min((int)value.Length, 58)` semantics in
    /// `ACommandPacket.cs:51`). Multi-packet uploads use a separate
    /// chunking helper (see `chunk_for_write`).
    pub fn new_request(command: u8, data: &[u8]) -> Self {
        let mut bytes = [0u8; PACKET_LEN];
        bytes[0] = REPORT_ID;
        bytes[1] = command;
        // bytes[2] = 0  — reserved, already zero
        // bytes[3..4] = PacketNumber 0 — already zero
        let n = data.len().min(MAX_PAYLOAD);
        bytes[5] = n as u8;
        bytes[HEADER_LEN..HEADER_LEN + n].copy_from_slice(&data[..n]);
        Self { bytes }
    }

    /// Borrow the packet as a 64-byte slice ready for `hid_device.write(...)`.
    pub fn as_bytes(&self) -> &[u8; PACKET_LEN] {
        &self.bytes
    }
}
