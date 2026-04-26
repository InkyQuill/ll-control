//! `verify-galahad probe` codec layer — read-only cross-device experimentation.
//!
//! Sends a single request to the device using one of the three known
//! framing classes (A/B/C) and dumps the response as hex. Used by
//! Phase 0e's targeted-probe set to confirm or refute hypotheses
//! sourced from sibling device families surveyed in
//! `docs/protocol/cross-device-survey.md`.
//!
//! This module owns the **codec layer only** — types, parsing, framing
//! sizes. The CLI dispatch + actual HID round-trip is in `main.rs`.
//!
//! # Caveats
//!
//! Each invocation should target a command byte that has been
//! reviewed as **read-only** by hid-safety-reviewer. The CLI does not
//! enforce this — the caller is responsible for choosing a safe cmd
//! byte. See `docs/protocol/captures/2026-04-26-cross-device-probes.txt`
//! for the curated probe set.
//!
//! # See also
//!
//! - `tools/verify-galahad/src/main.rs` — CLI dispatch
//! - `docs/protocol/cross-device-survey.md` section 6.4 — risky-area flagging
//! - `docs/superpowers/plans/2026-04-26-phase-0e-cross-device-survey.md` — Phase 0e plan

use anyhow::{anyhow, Context, Result};

/// HID framing class for a `probe` request.
///
/// Phase 0d's BCommandPacket framing is class B; Phase 0e adds class C
/// for the daemon's eventual `SendH264Frame512` codec. Other framings
/// (TLLCD-T, lcd207-WinUSB-EP01, slv3-RF, etc.) are documented in the
/// cross-device-survey but not yet selectable here — Phase 1+ extends
/// `Framing` if they become daemon-relevant.
///
/// # See also
///
/// The wire-level constants encoded here (output/input lengths, ReportID)
/// duplicate the protocol crate's authoritative definitions:
///
/// - `crates/protocol/src/galahad2_lcdv2/packets.rs` — A-class
///   `OUTPUT_LEN = 64`, `REPORT_ID = 0x01`
/// - `crates/protocol/src/galahad2_lcdv2/bpacket.rs` — B-class
///   `OUTPUT_LEN = 1024`, `INPUT_LEN = 512`, `REPORT_ID = 0x02`
/// - C-class (Phase 1+) — `OUTPUT_LEN = INPUT_LEN = 512`, `REPORT_ID = 0x03`,
///   per `docs/protocol/galahad2-lcdv2.md` framing notes
///
/// If those constants change, update `Framing` to match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[non_exhaustive]
pub enum Framing {
    /// `ACommandPacket` — 64-byte HID Output report, ReportID = `0x01`.
    A,
    /// `BCommandPacket` — 1024-byte output / 512-byte input, ReportID = `0x02`.
    B,
    /// `CCommandPacket` — 512-byte output / 512-byte input, ReportID = `0x03`.
    C,
}

impl Framing {
    /// Number of bytes to write per HID frame.
    #[must_use]
    pub fn output_len(self) -> usize {
        match self {
            Framing::A => 64,
            Framing::B => 1024,
            Framing::C => 512,
        }
    }

    /// Number of bytes to read per HID frame.
    #[must_use]
    pub fn input_len(self) -> usize {
        match self {
            Framing::A => 64,
            Framing::B => 512,
            Framing::C => 512,
        }
    }

    /// HID Report ID byte.
    #[must_use]
    pub fn report_id(self) -> u8 {
        match self {
            Framing::A => 0x01,
            Framing::B => 0x02,
            Framing::C => 0x03,
        }
    }
}

/// Arguments for a single probe round-trip.
#[derive(Debug, Clone)]
pub struct ProbeArgs {
    /// Command byte to send (e.g., `0x86` for `GetFirmwareVersion`).
    pub cmd: u8,
    /// HID framing class.
    pub framing: Framing,
    /// Hex payload to send after the cmd byte. Empty for bare requests.
    pub payload: Vec<u8>,
}

/// Parse a hex string into bytes.
///
/// The input is split on whitespace into tokens; each token may be
/// prefixed with `0x` or `0X` (stripped); each token must be an
/// even-length sequence of hex digits. Empty input yields an empty
/// `Vec`.
///
/// # Errors
///
/// Returns an error if any token has odd length after prefix stripping,
/// is empty after prefix stripping, or contains non-hex characters.
///
/// # Examples
///
/// ```
/// use verify_galahad::probe::parse_hex_payload;
/// assert_eq!(parse_hex_payload("aabb").unwrap(), vec![0xaa, 0xbb]);
/// assert_eq!(parse_hex_payload("0xAA 0xBB").unwrap(), vec![0xaa, 0xbb]);
/// assert_eq!(parse_hex_payload("").unwrap(), Vec::<u8>::new());
/// ```
#[must_use = "the parsed bytes are useless if discarded"]
pub fn parse_hex_payload(s: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for token in s.split_whitespace() {
        let stripped = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        if stripped.is_empty() {
            return Err(anyhow!("empty hex token in input: {s:?}"));
        }
        if stripped.len() % 2 != 0 {
            return Err(anyhow!(
                "hex token has odd length: token={token:?}, input={s:?}"
            ));
        }
        for i in (0..stripped.len()).step_by(2) {
            let byte = u8::from_str_radix(&stripped[i..i + 2], 16).with_context(|| {
                format!(
                    "non-hex digit in token {token:?} at byte offset {} of input {s:?}",
                    i / 2
                )
            })?;
            bytes.push(byte);
        }
    }
    Ok(bytes)
}
