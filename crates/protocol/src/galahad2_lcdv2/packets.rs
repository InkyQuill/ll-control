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

impl ACommandPacket {
    /// Parse and validate an inbound 64-byte HID buffer as an
    /// `ACommandPacket` reply to a request that used `expected_command`.
    ///
    /// Validates: minimum length, ReportID, command echo, and that the
    /// `Length` field fits in `MAX_PAYLOAD`.
    pub fn parse_response(buf: &[u8], expected_command: u8) -> crate::Result<ParsedResponse<'_>> {
        if buf.len() < HEADER_LEN {
            return Err(crate::Error::BufferTooShort {
                need: HEADER_LEN,
                got: buf.len(),
            });
        }
        if buf[0] != REPORT_ID {
            return Err(crate::Error::UnexpectedReportId {
                expected: REPORT_ID,
                got: buf[0],
            });
        }
        if buf[1] != expected_command {
            return Err(crate::Error::UnexpectedCommand {
                expected: expected_command,
                got: buf[1],
            });
        }
        let length = buf[5] as usize;
        if length > MAX_PAYLOAD {
            return Err(crate::Error::InvalidLength {
                length,
                max: MAX_PAYLOAD,
            });
        }
        // The buffer may be exactly 64 bytes (the typical hidraw read size)
        // or longer; we never look past `HEADER_LEN + length`.
        if buf.len() < HEADER_LEN + length {
            return Err(crate::Error::BufferTooShort {
                need: HEADER_LEN + length,
                got: buf.len(),
            });
        }

        Ok(ParsedResponse {
            command: buf[1],
            length,
            data: &buf[HEADER_LEN..HEADER_LEN + length],
        })
    }
}

/// A view into a successfully-parsed `ACommandPacket` response.
///
/// Returned by [`ACommandPacket::parse_response`]. Borrows from the input
/// buffer; lifetime is the buffer's.
#[derive(Debug, Clone, Copy)]
pub struct ParsedResponse<'a> {
    command: u8,
    length: usize,
    data: &'a [u8],
}

impl<'a> ParsedResponse<'a> {
    /// The command echo byte from the response.
    pub fn command(&self) -> u8 {
        self.command
    }
    /// The advertised data length.
    pub fn length(&self) -> usize {
        self.length
    }
    /// The data slice, exactly `length` bytes.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}
