//! `verify-galahad probe` codec + I/O layer — read-only cross-device experimentation.
//!
//! Sends a single request to the device using one of the three known
//! framing classes (A/B/C) and dumps the response as hex. Used by
//! Phase 0e's targeted-probe set to confirm or refute hypotheses
//! sourced from sibling device families surveyed in
//! `docs/development/protocol/cross-device-survey.md`.
//!
//! Owns the framing-class types, hex-payload parser, and HID round-trip
//! helper [`run_probe`]. The CLI dispatch (clap argument struct, safety
//! gate, hex-dump output) lives in `main.rs`.
//!
//! # Caveats
//!
//! Each invocation should target a command byte that has been
//! reviewed as **read-only** by hid-safety-reviewer. The CLI does not
//! enforce this — the caller is responsible for choosing a safe cmd
//! byte. See `docs/development/protocol/captures/2026-04-26-cross-device-probes.txt`
//! for the curated probe set.
//!
//! # See also
//!
//! - `tools/verify-galahad/src/main.rs` — CLI dispatch
//! - `docs/development/protocol/cross-device-survey.md` section 6.4 — risky-area flagging
//! - `docs/development/superpowers/plans/2026-04-26-phase-0e-cross-device-survey.md` — Phase 0e plan

use anyhow::{anyhow, Context, Result};
use hidapi::HidDevice;

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
///   per `docs/development/protocol/galahad2-lcdv2.md` framing notes
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
#[must_use = "feed the parsed bytes into ProbeArgs::payload"]
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

/// Maximum stale responses to drain while waiting for the cmd echo.
///
/// Picked to mirror the existing A-class drain budget in
/// `read_until_command` (which is time-bounded rather than count-bounded,
/// but typically settles within 1-2 stale packets on FW 1.6). Four gives
/// plenty of headroom without letting a totally silent device hang the
/// caller indefinitely.
const PROBE_DRAIN_LIMIT: usize = 4;

/// Per-read timeout while waiting for the echo, in milliseconds.
///
/// Matches the 500 ms used by the cross-device-survey targeted probes;
/// long enough that a slow B-class JPEG response can settle, short
/// enough that `PROBE_DRAIN_LIMIT * READ_TIMEOUT_MS` (= 2 s worst case)
/// stays well under a human's "did the CLI hang?" patience threshold.
const READ_TIMEOUT_MS: i32 = 500;

/// Perform one probe round-trip.
///
/// Constructs the output buffer for the chosen framing class with the
/// command byte at offset 1 (offset 0 is the report ID per
/// [`Framing::report_id`]) and the payload starting at offset 2; the
/// rest is zero-filled. Writes once, then drains responses looking
/// for one whose command-echo byte matches `args.cmd` until either a
/// match arrives or the device produces `PROBE_DRAIN_LIMIT` stale
/// responses in a row.
///
/// # Echo convention
///
/// Both A-class ([`crate::probe::Framing::A`]) and B-class
/// ([`crate::probe::Framing::B`]) responses on Galahad II LCDV2 echo
/// the **plain** request command byte at response offset 1, with the
/// report ID at offset 0. This matches
/// `crates/protocol/src/galahad2_lcdv2/packets.rs::ACommandPacket::parse_response`
/// and `bpacket.rs::BCommandPacket::from_response`. C-class is
/// extrapolated from the same C# pattern; live verification is part of
/// the Phase 0e probe runs that consume this helper.
///
/// The response is recognized by `buf[0] == report_id && buf[1] == cmd`
/// — both bytes are required so a stray B/C-class unsolicited packet
/// whose byte-1 happens to match the probed cmd byte cannot be
/// misinterpreted as our response.
///
/// # Errors
///
/// Returns an error if:
/// - the payload doesn't fit in the framing class's output buffer
/// - the HID write fails
/// - the HID write is short
/// - no response with the expected echo arrives within
///   `PROBE_DRAIN_LIMIT` read attempts (each capped at
///   `READ_TIMEOUT_MS`).
///
/// # Caller responsibilities
///
/// Sending an arbitrary cmd byte mutates device state if the cmd byte
/// has any write semantics. The caller is responsible for choosing a
/// hid-safety-reviewer-cleared read-only cmd byte; `run_probe` does
/// not enforce this. The CLI dispatch in `main.rs` gates this with a
/// `--yes-read-only` flag.
///
/// # See also
///
/// - `tools/verify-galahad/src/main.rs::Cmd::Probe` — CLI dispatch
/// - `docs/development/protocol/captures/2026-04-26-cross-device-probes.txt` — curated probe set
pub fn run_probe(device: &HidDevice, args: &ProbeArgs) -> Result<Vec<u8>> {
    let out_len = args.framing.output_len();
    let in_len = args.framing.input_len();
    let report_id = args.framing.report_id();

    // Build output buffer: [report_id, cmd, payload..., 0..]
    // Payload room is `out_len - 2` (one byte each for report_id and cmd).
    let payload_room = out_len.saturating_sub(2);
    if args.payload.len() > payload_room {
        return Err(anyhow!(
            "payload ({} bytes) exceeds framing-{:?} room ({} bytes)",
            args.payload.len(),
            args.framing,
            payload_room
        ));
    }
    let mut out = vec![0u8; out_len];
    out[0] = report_id;
    out[1] = args.cmd;
    out[2..2 + args.payload.len()].copy_from_slice(&args.payload);

    let written = device
        .write(&out)
        .with_context(|| format!("HID write failed for cmd 0x{:02x}", args.cmd))?;
    if written != out_len {
        return Err(anyhow!(
            "short write: {written}/{out_len} bytes for cmd 0x{:02x}",
            args.cmd
        ));
    }

    // Drain stale responses until the echo for our cmd arrives. Both
    // A-class and B-class place the echo at response offset 1 (after
    // the report ID at offset 0); we require both bytes to match so a
    // stray B/C-class unsolicited packet whose byte-1 happens to
    // match the probed cmd byte cannot be misinterpreted as our
    // response. This mirrors the existing `read_until_command`
    // helper's convention in `main.rs`.
    let mut buf = vec![0u8; in_len];
    let mut stale_count: u32 = 0;
    let mut timeout_count: u32 = 0;
    for _attempt in 0..PROBE_DRAIN_LIMIT {
        let n = device
            .read_timeout(&mut buf, READ_TIMEOUT_MS)
            .context("HID read_timeout failed")?;
        if n == 0 {
            timeout_count += 1;
            eprintln!("probe: drain attempt timed out (no bytes)");
            continue;
        }
        if n > 1 && buf[0] == report_id && buf[1] == args.cmd {
            buf.truncate(n);
            return Ok(buf);
        }
        stale_count += 1;
        eprintln!(
            "probe: stale response (cmd byte 0x{:02x} != expected 0x{:02x}); draining",
            buf.get(1).copied().unwrap_or(0),
            args.cmd
        );
    }
    Err(anyhow!(
        "drained {stale_count} stale + {timeout_count} timeout reads without finding echo of cmd 0x{:02x} (report_id 0x{report_id:02x})",
        args.cmd
    ))
}
