//! Source-derived FW1.6 update reply recognition, without device I/O.
//!
//! Exact researched image SHA-256 and function offsets are recorded in
//! `docs/development/parity/firmware-lcdv2.md`: dispatcher 0x3c418, upload 0x81a94,
//! activation 0x815d8. These are not hardware-qualified ACKs. In particular:
//! - A 512-byte upload reply echoes only the first 501 payload bytes, even
//!   when the echoed header declares a 1013-byte chunk. Do not use the generic
//!   B-response payload parser or pad a short transport read to 512 bytes.
//! - Echoes have no transaction nonce. An identical delayed reply can match.
//! - Activation requests reset; it does not prove reboot or the new version.
//! - Device rejection and malformed/unmatched replies must never authorize
//!   advancing an update. Retry/recovery policy belongs to the update owner.

use super::bpacket::{BCommandPacket, HEADER_LEN, INPUT_LEN, PAYLOAD_MAX, REPORT_ID};

const UPLOAD: u8 = 11;
const ACTIVATE: u8 = 10;
const DEVICE_ERROR: u8 = 2;

/// Failure to recognize a reply in the exact researched response envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The expectation was not a nonempty canonical cmd11 chunk.
    #[error("invalid firmware chunk expectation")]
    InvalidChunk,
    /// Actual transport extent differs from the researched 512-byte reply.
    #[error("firmware reply must contain exactly 512 bytes, got {0}")]
    ResponseExtent(usize),
    /// The reply does not match the request or researched activation response.
    #[error("unmatched firmware reply")]
    UnmatchedReply,
    /// A cmd2 reply lacks the high-bit error status from the researched branch.
    #[error("malformed firmware error reply: status {0:#04x}")]
    MalformedDeviceError(u8),
    /// Error status overwrites the high PN byte, preventing full correlation.
    #[error("firmware error reply cannot correlate packet number above 65535")]
    AmbiguousErrorPacketNumber,
    /// Matching error envelope; preserve unknown statuses as failures too.
    /// This is not a complete error-code enum or a retry recommendation.
    #[error("firmware device rejected request: status {0:#04x}")]
    DeviceRejected(u8),
}

/// A recognized chunk echo; not a claim of flash completion or persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkEcho {
    /// Echoed 24-bit chunk index.
    pub packet_number: u32,
    /// Canonical end offset of the echoed request in the source image.
    pub end_offset: u32,
    /// Whether that request ends at the image's declared byte count.
    pub is_last: bool,
}

/// Request prefix and metadata owned by the caller awaiting one chunk reply.
///
/// This does not validate the image, order requests, or authorize sending them.
/// Construct from the actual packet whose full output write succeeded.
#[derive(Debug, Clone)]
pub struct ChunkExpectation {
    prefix: [u8; INPUT_LEN],
    echo: ChunkEcho,
}

impl ChunkExpectation {
    /// Check canonical 1013-byte chunking (including a shorter final chunk).
    /// Empty, out-of-image, wrong-length and non-cmd11 requests are rejected.
    pub fn new(request: &BCommandPacket) -> Result<Self, DecodeError> {
        let bytes = request.as_bytes();
        let size = u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        let pn = u32::from_be_bytes([0, bytes[6], bytes[7], bytes[8]]);
        let length = u16::from_be_bytes([bytes[9], bytes[10]]);
        // Widen before multiplying: a hostile 24-bit PN must not wrap into
        // the accepted u32 image range, even in release builds.
        let offset = u64::from(pn) * 1013;
        if bytes[0] != REPORT_ID
            || bytes[1] != UPLOAD
            || size == 0
            || offset >= u64::from(size)
            || u64::from(length) != (u64::from(size) - offset).min(1013)
            || bytes[HEADER_LEN + usize::from(length)..]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(DecodeError::InvalidChunk);
        }
        let end = offset + u64::from(length);
        let end_offset = u32::try_from(end).map_err(|_| DecodeError::InvalidChunk)?;
        let mut prefix = [0; INPUT_LEN];
        prefix.copy_from_slice(&bytes[..INPUT_LEN]);
        Ok(Self {
            prefix,
            echo: ChunkEcho {
                packet_number: pn,
                end_offset,
                is_last: end_offset == size,
            },
        })
    }

    /// Recognize the exact echo/error envelope from an unpadded transport read.
    /// A previous PN, other size/payload prefix, or changed padding is rejected.
    /// No matching echo can authenticate the unechoed remainder of a chunk.
    pub fn decode(&self, response: &[u8]) -> Result<ChunkEcho, DecodeError> {
        match_reply(&self.prefix, response, self.echo.packet_number)?;
        Ok(self.echo)
    }
}

/// Recognition that the researched activation handler requested a reset.
/// Reconnect to the same target and re-read its version before claiming success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetRequested;

/// Decode the researched cmd10 payload01 activation response.
///
/// FW1.6 preserves Length=1 and sets *raw* bytes11 and12 to1. The latter lies
/// outside that declared payload: do not index `BResponse::data()[1]` as the
/// decompiled Windows Core did. Require the actual 512-byte input extent.
pub fn decode_activation(response: &[u8]) -> Result<ResetRequested, DecodeError> {
    let request = BCommandPacket::new_request(ACTIVATE, 0, &[1], 1);
    let mut prefix = [0; INPUT_LEN];
    prefix.copy_from_slice(&request.as_bytes()[..INPUT_LEN]);
    // Error paths copy the original input, not the successful extra flag.
    if response.get(1) == Some(&DEVICE_ERROR) {
        match_reply(&prefix, response, 0)?;
        return Err(DecodeError::UnmatchedReply);
    }
    prefix[12] = 1;
    match_reply(&prefix, response, 0)?;
    Ok(ResetRequested)
}

fn match_reply(expected: &[u8; INPUT_LEN], response: &[u8], pn: u32) -> Result<(), DecodeError> {
    if response.len() != INPUT_LEN {
        return Err(DecodeError::ResponseExtent(response.len()));
    }
    if response[1] == DEVICE_ERROR {
        if pn > u32::from(u16::MAX) {
            return Err(DecodeError::AmbiguousErrorPacketNumber);
        }
        // cmd2 and status at byte6 replace command and high PN byte. All
        // surviving header/data/padding must still match the pending request.
        if response
            .iter()
            .zip(expected)
            .enumerate()
            .any(|(i, (a, b))| i != 1 && i != 6 && a != b)
        {
            return Err(DecodeError::UnmatchedReply);
        }
        if response[6] & 0x80 == 0 {
            return Err(DecodeError::MalformedDeviceError(response[6]));
        }
        return Err(DecodeError::DeviceRejected(response[6]));
    }
    if response != expected {
        return Err(DecodeError::UnmatchedReply);
    }
    Ok(())
}

// Keep constants beside the observed framing rather than letting magic-number
// changes in the shared packet codec silently change the researched envelope.
const _: () = assert!(PAYLOAD_MAX == 1013 && INPUT_LEN == 512 && HEADER_LEN == 11);
