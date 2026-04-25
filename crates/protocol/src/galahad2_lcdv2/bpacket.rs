//! `BCommandPacket` — 1024-byte HID output / 512-byte input framing for
//! cmds that exceed the 58-byte ACommandPacket payload (image uploads,
//! per-pixel LED arrays, firmware blobs).
//!
//! Source: Products/Galahad2Vision/BCommandPacket.cs:1-189.

/// HID Report ID byte for B-class output packets.
pub const REPORT_ID: u8 = 0x02;
/// Output packet length on the wire (host → device).
pub const OUTPUT_LEN: usize = 1024;
/// Input packet length on the wire (device → host).
pub const INPUT_LEN: usize = 512;
/// Header bytes preceding the payload.
pub const HEADER_LEN: usize = 11;
/// Maximum payload bytes per packet.
pub const PAYLOAD_MAX: usize = OUTPUT_LEN - HEADER_LEN; // 1013

// Big-endian header field offsets (matches C# bit-shifts):
const OFF_DATA_SIZE_HI: usize = 2; // 4 bytes [2..6]
const OFF_PACKET_NUMBER: usize = 6; // 3 bytes [6..9]
const OFF_LENGTH: usize = 9; // 2 bytes [9..11]

/// One B-class HID output packet, 1024 bytes wide.
#[derive(Debug, Clone)]
pub struct BCommandPacket {
    bytes: [u8; OUTPUT_LEN],
}

impl BCommandPacket {
    /// Build a single B-packet for transmission.
    ///
    /// Caller is responsible for providing the correct `data_size` (total
    /// across all chunks) and `packet_number`; the multi-packet chunker
    /// (added in a follow-up commit) handles both for you.
    ///
    /// Silent truncation: if `payload.len() > PAYLOAD_MAX` (1013), the excess
    /// is dropped without warning. This mirrors C# `Math.Min` semantics in
    /// `BCommandPacket.cs:53,129,162`. For payloads larger than 1013 bytes,
    /// use the [multi-packet chunker][BCommandPacket::build_packets_for_write]
    /// (Phase 0d Task 2) — it splits and yields one packet per 1013-byte chunk.
    #[must_use = "returns the packet bytes; dropping without writing has no effect"]
    pub fn new_request(command: u8, packet_number: u32, payload: &[u8], data_size: u32) -> Self {
        let mut bytes = [0u8; OUTPUT_LEN];
        bytes[0] = REPORT_ID;
        bytes[1] = command;
        bytes[OFF_DATA_SIZE_HI..OFF_DATA_SIZE_HI + 4].copy_from_slice(&data_size.to_be_bytes());
        // 24-bit BE PacketNumber: take the low 3 bytes of the u32.
        let pn = packet_number.to_be_bytes();
        bytes[OFF_PACKET_NUMBER..OFF_PACKET_NUMBER + 3].copy_from_slice(&pn[1..4]);
        let n = payload.len().min(PAYLOAD_MAX);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "n is bounded to PAYLOAD_MAX (1013) ≤ u16::MAX"
        )]
        let length_be = (n as u16).to_be_bytes();
        bytes[OFF_LENGTH..OFF_LENGTH + 2].copy_from_slice(&length_be);
        bytes[HEADER_LEN..HEADER_LEN + n].copy_from_slice(&payload[..n]);
        Self { bytes }
    }

    /// Borrow the packet as the 1024-byte buffer ready for `hid_device.write(...)`.
    #[must_use = "returns the buffer for hid_device.write"]
    pub fn as_bytes(&self) -> &[u8; OUTPUT_LEN] {
        &self.bytes
    }

    /// Parse and validate an inbound B-class response (typically 512 bytes).
    pub fn from_response(buf: &[u8], expected_command: u8) -> crate::Result<BResponse<'_>> {
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
        let data_size = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]);
        let packet_number = u32::from_be_bytes([0, buf[6], buf[7], buf[8]]);
        let length = u16::from_be_bytes([buf[9], buf[10]]) as usize;
        if length > PAYLOAD_MAX {
            return Err(crate::Error::InvalidLength {
                length,
                max: PAYLOAD_MAX,
            });
        }
        if buf.len() < HEADER_LEN + length {
            return Err(crate::Error::BufferTooShort {
                need: HEADER_LEN + length,
                got: buf.len(),
            });
        }
        Ok(BResponse {
            command: buf[1],
            data_size,
            packet_number,
            length,
            data: &buf[HEADER_LEN..HEADER_LEN + length],
        })
    }
}

/// View into a parsed B-class response. Borrows from the input buffer.
#[derive(Debug, Clone, Copy)]
pub struct BResponse<'a> {
    command: u8,
    data_size: u32,
    packet_number: u32,
    length: usize,
    data: &'a [u8],
}

impl<'a> BResponse<'a> {
    /// Echoed command byte from the response.
    pub fn command(&self) -> u8 {
        self.command
    }
    /// Total payload size announced by the device (32-bit).
    pub fn data_size(&self) -> u32 {
        self.data_size
    }
    /// 0-based packet index in this transfer (24-bit).
    pub fn packet_number(&self) -> u32 {
        self.packet_number
    }
    /// Bytes-of-payload in this packet (16-bit).
    pub fn length(&self) -> usize {
        self.length
    }
    /// Payload slice, exactly `length` bytes.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

impl BCommandPacket {
    /// Iterator over B-packets to write to the device for a payload of
    /// arbitrary size. Empty payload → single packet with `DataSize=0`.
    /// Otherwise, payload is split into 1013-byte chunks; every packet
    /// replicates `command` and `data_size = payload.len()`, and
    /// `packet_number` increments from 0.
    ///
    /// Each yielded `BCommandPacket` owns its own 1024-byte buffer, so
    /// callers can safely `.collect()` chunks for batched submission
    /// without aliasing — unlike the C# original which reuses a single
    /// buffer (`BCommandPacket.cs:145-175`).
    #[must_use = "returns the chunk iterator; dropping without iterating sends no packets"]
    pub fn build_packets_for_write(command: u8, payload: &[u8]) -> BPacketChunks<'_> {
        BPacketChunks {
            command,
            payload,
            #[expect(
                clippy::cast_possible_truncation,
                reason = "payload sizes are KB-scale; the device can't store >4GiB"
            )]
            data_size: payload.len() as u32,
            cursor: 0,
            emitted_empty: false,
        }
    }
}

/// Iterator returned by [`BCommandPacket::build_packets_for_write`].
///
/// Yields one independently owned [`BCommandPacket`] per 1013-byte chunk
/// of the source payload (or a single zero-data packet when the payload
/// is empty).
#[derive(Debug)]
#[must_use = "iterator is lazy; dropping without consuming sends no packets"]
pub struct BPacketChunks<'a> {
    command: u8,
    payload: &'a [u8],
    data_size: u32,
    cursor: usize,
    emitted_empty: bool,
}

impl Iterator for BPacketChunks<'_> {
    type Item = BCommandPacket;

    fn next(&mut self) -> Option<Self::Item> {
        if self.payload.is_empty() {
            if self.emitted_empty {
                return None;
            }
            self.emitted_empty = true;
            return Some(BCommandPacket::new_request(self.command, 0, &[], 0));
        }
        if self.cursor >= self.payload.len() {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "packet index fits u32: payload ≤ 4 GiB and PAYLOAD_MAX = 1013, so max pn ≈ 4M << u32::MAX"
        )]
        let pn = (self.cursor / PAYLOAD_MAX) as u32;
        let end = (self.cursor + PAYLOAD_MAX).min(self.payload.len());
        let chunk = &self.payload[self.cursor..end];
        self.cursor = end;
        Some(BCommandPacket::new_request(
            self.command,
            pn,
            chunk,
            self.data_size,
        ))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.payload.is_empty() {
            let n = if self.emitted_empty { 0 } else { 1 };
            return (n, Some(n));
        }
        let total = self.payload.len().div_ceil(PAYLOAD_MAX);
        let yielded = self.cursor / PAYLOAD_MAX;
        let remaining = total - yielded;
        (remaining, Some(remaining))
    }
}
