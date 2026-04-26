# Phase 1 — Daemon MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `lconnectd` (a Linux daemon under `systemd --user`) + `lconnectctl` (a CLI client) that wrap every codec verified in Phase 0 (sensors, PWM, lighting, LCD config, LCD JPEG) behind a stable JSON-RPC IPC contract, with persistence for user settings, continuous re-issue timers for lighting and LCD frame, hot-plug detection, udev / systemd / AUR packaging, and the CCommandPacket framing codec.

**Architecture:** Per-device actor tree on a Tokio multi-threaded runtime. All HID I/O on `spawn_blocking`. Per-device `hid_io` mpsc serialises writes. Sensor snapshots fan out via `tokio::sync::watch`. The LCD `frame_slot` is the explicit hook for Phase 4's theme engine. JSON-RPC 2.0 over Unix domain socket at `$XDG_RUNTIME_DIR/lconnectd.sock`. `DaemonRpc` trait abstracts the transport for future DBus migration. Hardware coverage: Galahad II LCDV2 only.

**Tech Stack:** Rust 1.82, Tokio multi-thread runtime, `hidapi-rs`, `serde` + `serde_json`, `toml`, `tracing` + `tracing-journald`, `clap` derive, `bytes`, `async-trait`. New crates: `lconnect-hid`, `lconnect-ipc`, `lconnect-daemon`, `lconnect-cli`. New packaging artifacts: udev rule, systemd unit, AUR PKGBUILD skeleton.

---

## Context + references

**Approved spec:** [`docs/superpowers/specs/2026-04-26-phase-1-daemon-design.md`](../specs/2026-04-26-phase-1-daemon-design.md)

**Phase 0e findings (lift-ready inputs):** [`docs/protocol/cross-device-survey.md`](../../protocol/cross-device-survey.md) section 6.5

**Phase 0d output (LCDV2 protocol baseline):** [`docs/protocol/galahad2-lcdv2.md`](../../protocol/galahad2-lcdv2.md)

**Project root conventions:** parent `CLAUDE.md` (hooks, `hid-safety-reviewer` policy, `99-lian-li-galahad2.rules` legacy fixture path).

### Phase 0e inputs folded in

- **Framing extensibility:** `lconnect-hid::Framing` is `#[non_exhaustive]` from day 1. A/B/C declared; later families add variants without breaking matches.
- **Cmd 15 stays codec-only, IPC-unexposed.** No `set_lcd_image_persistent` method; `set_lcd_image` only invokes cmd 14 (transient + keeper-refresh).
- **`lighting_keeper` cadence: 1500 ms.** Mirrors C# service's continuous re-issue on cmd 131.
- **`lcd_keeper` cadence: 15 000 ms.** 5 s margin under firmware's ~20 s auto-revert.
- **No firmware paths in Phase 1.** `UploadFirmware` (cmd 11), `ActiveFirmware` (cmd 10), serial-number write are codec-deferred AND IPC-sealed.
- **No DES-CBC.** LCDV2 doesn't use it.
- **A-class permissive echo / B-class strict-on-unknown** (Phase 0e probe finding):
  - The `verify-galahad probe` subcommand is the model for any future capability-discovery code in the daemon: A-class queries are tolerant; never speculate B-class cmd bytes.
  - The daemon must hardcode the LCDV2 cmd-byte allowlist for B-class writes; do not blanket-relay arbitrary B-class IPC requests.
- **Phase 1 device scope:** LCDV2 only. The udev rule lists VID `0x0416` for PID `0x7395`. Multi-device coverage is Phase 3+.
- **Code organisation:** keep `crates/protocol/src/galahad2_lcdv2/` as-is. Add `// TODO(phase-3-refactor): ...` comments per section 6.3 of the survey on the codec candidates that would migrate to `protocol/src/common/` when the second family lands.

### Project conventions to honor (carry-forward from Phase 0)

- Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, with scope where applicable)
- `#![forbid(unsafe_code)]` on every crate
- `#[must_use]` on builders / type constructors
- `#[non_exhaustive]` on public enums
- `# Caveats` / `# Errors` / `# Examples` / `# See also` rustdoc convention
- ASCII-only commit messages (a unicode em-dash tripped a permission filter in Phase 0e)
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc --workspace --no-deps` all clean before every commit
- Per the user's standing preference: code-touching tasks get spec-compliance + code-quality + rust-practices-verifier review subagents; HID-write paths additionally get hid-safety-reviewer
- One device experiment at a time during the smoke run (P1H-1); wait for visual confirmation between writes

---

## File structure overview

```
lconnect-linux/
├── Cargo.toml                        modified: workspace adds 4 new crates
├── crates/
│   ├── protocol/                     modified: gains CCommandPacket + SendH264Frame512
│   │   └── src/galahad2_lcdv2/
│   │       ├── cpacket.rs            NEW: CCommandPacket framing struct
│   │       └── h264.rs               modified: add SendH264Frame512 codec
│   ├── lconnect-image/               unchanged
│   ├── lconnect-hid/                 NEW crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                crate root
│   │       ├── backend.rs            HidBackend trait
│   │       ├── hidapi_backend.rs     hidapi-rs impl
│   │       ├── mock_backend.rs       fake impl for tests
│   │       ├── framing.rs            Framing enum (non_exhaustive)
│   │       ├── device_info.rs        DeviceInfo / DeviceFamilyId
│   │       └── family_registry.rs    DeviceFamily trait + LCDV2 registration
│   ├── lconnect-ipc/                 NEW crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── trait_def.rs          DaemonRpc trait + types
│   │       ├── types.rs              DeviceId, DeviceDescriptor, SensorSnapshot, etc.
│   │       ├── error.rs              IpcError + JSON-RPC error code mapping
│   │       ├── jsonrpc.rs            wire protocol framing
│   │       ├── server.rs             JSON-RPC server scaffold
│   │       ├── client.rs             JSON-RPC client scaffold
│   │       └── transport_memory.rs   in-process transport for tests
│   ├── lconnect-daemon/              NEW crate (binary)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs               entry point
│   │       ├── config.rs             daemon.toml loading + defaults
│   │       ├── persistence.rs        per-device TOML; atomic writes; debounce
│   │       ├── manager.rs            device_manager task
│   │       ├── actor/
│   │       │   ├── mod.rs            Actor trait + spawn helper
│   │       │   ├── hid_io.rs         hid_io task
│   │       │   ├── sensor_poll.rs    sensor_poll task
│   │       │   ├── lighting_keeper.rs
│   │       │   ├── lcd_keeper.rs
│   │       │   └── command_dispatch.rs
│   │       ├── ipc_server.rs         glues lconnect-ipc to actor commands
│   │       └── stats.rs              daemon_stats counters
│   └── lconnect-cli/                 NEW crate (binary)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs               clap entry point
│           ├── connection.rs         IPC connection helper
│           ├── format.rs             human-readable output + --json
│           ├── watch.rs              redraw-in-place TTY view
│           └── commands/
│               ├── mod.rs
│               ├── devices.rs        list_devices / firmware
│               ├── snapshot.rs       snapshot (one-shot)
│               ├── pwm.rs            fan / pump
│               ├── lighting.rs       lighting set / clear / info / led-colors
│               ├── lcd.rs            lcd available / brightness / rotation / mode / image / clear
│               └── maintenance.rs    reset / logs / raw / daemon
├── tools/
│   └── verify-galahad/               unchanged in Phase 1; existing `probe` subcommand stays
├── packaging/                        NEW
│   ├── udev/70-lianli-hid.rules
│   ├── systemd/lconnectd.service
│   └── arch/PKGBUILD
└── xtask/                            NEW (lightweight)
    ├── Cargo.toml
    └── src/main.rs                   `cargo xtask gen-udev` -> packaging/udev/
```

**Boundary rules enforced by Cargo dep graph:**
- `protocol/` is pure, no I/O, `#![forbid(unsafe_code)]`
- `lconnect-hid/` is the only crate that depends on `hidapi-rs`
- `lconnect-cli/` depends only on `lconnect-ipc/` (cannot do HID I/O)
- `lconnect-daemon/` is the only crate that wires everything together
- `lconnect-image/` stays usable from both daemon and CLI

---

## Sub-phase index

- **A. Protocol additions** (P1A-1 .. P1A-2) — CCommandPacket framing + SendH264Frame512 codec
- **B. lconnect-hid crate** (P1B-1 .. P1B-5) — backend trait + hidapi-rs impl + mock + family registry
- **C. lconnect-ipc crate** (P1C-1 .. P1C-6) — DaemonRpc trait + types + JSON-RPC framing + server + client + in-mem transport
- **D. lconnect-daemon crate** (P1D-1 .. P1D-15) — process model + per-device actor tree + persistence + IPC server
- **E. lconnect-cli crate** (P1E-1 .. P1E-7) — connection helper + per-subcommand modules
- **F. Packaging** (P1F-1 .. P1F-3) — udev + systemd + AUR PKGBUILD
- **G. Cmd-15 investigation** (P1G-1) — 1-day timeboxed
- **H. Wrap-up** (P1H-1 .. P1H-2) — live smoke run + post-execution notes

Total: ~38 tasks. Per-task review (subagent-driven) adds spec-compliance + code-quality + rust-practices verdicts on every code-touching task; HID-write paths add hid-safety. Polish commits land as separate `fix:` commits per Phase 0d/0e convention.

---

## Sub-phase A — Protocol additions

### Task P1A-1: Add CCommandPacket framing struct

**Files:**
- Create: `crates/protocol/src/galahad2_lcdv2/cpacket.rs`
- Create: `crates/protocol/tests/galahad2_lcdv2_cpacket.rs`
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs` (add `pub mod cpacket;`)

CCommandPacket is the third HID framing class verified by Phase 0e's slv3/lcd207 surveys. ReportID = `0x03`, output = 512 bytes, input = 512 bytes, header occupies 11 bytes (same shape as B), payload max 501 bytes per packet. The Phase 0d work established the BCommandPacket pattern in `bpacket.rs`; mirror it.

- [ ] **Step 1: Read the BCommandPacket pattern**

```bash
cat crates/protocol/src/galahad2_lcdv2/bpacket.rs
```

The CCommandPacket impl mirrors this file exactly except:
- `OUTPUT_LEN = 512` instead of 1024
- `INPUT_LEN = 512` (same as B)
- `REPORT_ID = 0x03`
- `PAYLOAD_MAX = 501` (= 512 - 11)
- `BPacketChunks` becomes `CPacketChunks` (own type, same iterator shape)

- [ ] **Step 2: Write the failing tests**

```rust
// crates/protocol/tests/galahad2_lcdv2_cpacket.rs
use lconnect_protocol::galahad2_lcdv2::cpacket::{
    CCommandPacket, CResponse, CPacketChunks,
    OUTPUT_LEN, INPUT_LEN, HEADER_LEN, PAYLOAD_MAX, REPORT_ID,
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
    // BE u24 PacketNumber at offset 6
    assert_eq!(&bytes[6..9], &[0x00, 0x00, 0x01]);
    // BE u16 Length at offset 9
    assert_eq!(&bytes[9..11], &(payload.len() as u16).to_be_bytes());
    assert_eq!(&bytes[11..14], &payload);
    assert_eq!(bytes[14..].iter().all(|&b| b == 0), true);
}

#[test]
fn from_response_parses_512_bytes() {
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
    assert_eq!(chunks.len(), 1);
}

#[test]
fn chunks_iterator_handles_exact_multiple() {
    let payload = vec![0u8; PAYLOAD_MAX];
    let chunks: Vec<_> = CPacketChunks::new(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 1);

    let payload = vec![0u8; PAYLOAD_MAX * 2];
    let chunks: Vec<_> = CPacketChunks::new(0x0D, &payload).collect();
    assert_eq!(chunks.len(), 2);
}
```

- [ ] **Step 3: Run tests — expect failure**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_cpacket 2>&1 | tail -10
```

Expected: compile error (`cpacket` module doesn't exist).

- [ ] **Step 4: Implement `cpacket.rs`**

Write `crates/protocol/src/galahad2_lcdv2/cpacket.rs` mirroring `bpacket.rs`, swapping the constants. Cite the C# source: `LConnectCore/Products/Galahad2Vision/CCommandPacket.cs:108-176` (verified to exist by the Phase 0e slv3 survey).

```rust
//! CCommandPacket framing — 512-byte HID output / 512-byte HID input.
//!
//! Third HID framing class for Galahad II LCDV2; used by `SendH264Frame512`.
//!
//! ReportID = 0x03. Header is 11 bytes:
//! `[ReportID, Command, BE u32 DataSize, BE u24 PacketNumber, BE u16 Length]`.
//! Payload max = 501 bytes per packet; multi-packet streams chunk via
//! `CPacketChunks`.
//!
//! # See also
//!
//! - [`crate::galahad2_lcdv2::bpacket`] — sibling B-class framing
//! - `LConnectCore/Products/Galahad2Vision/CCommandPacket.cs:108-176`
//!   (Phase 0e static-survey citation)

use crate::{Error, Result};

pub const REPORT_ID: u8 = 0x03;
pub const OUTPUT_LEN: usize = 512;
pub const INPUT_LEN: usize = 512;
pub const HEADER_LEN: usize = 11;
pub const PAYLOAD_MAX: usize = OUTPUT_LEN - HEADER_LEN;

#[derive(Debug, Clone, Copy)]
pub struct CCommandPacket {
    bytes: [u8; OUTPUT_LEN],
}

impl CCommandPacket {
    #[must_use]
    pub fn new_request(cmd: u8, packet_number: u32, payload: &[u8], data_size: u32) -> Self {
        // (mirror bpacket::BCommandPacket::new_request, panic-free assertion via assert!
        //  on payload.len() <= PAYLOAD_MAX. See bpacket.rs for the exact body.)
        unimplemented!("mirror BCommandPacket::new_request with C-class constants")
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; OUTPUT_LEN] {
        &self.bytes
    }

    /// Parse a 512-byte response. Verifies report-ID at offset 0 and
    /// command echo (plain, not `| 0x80`) at offset 1.
    ///
    /// # Errors
    ///
    /// Returns `Error::ShortResponse` if `buf.len() < INPUT_LEN`.
    /// Returns `Error::CommandEchoMismatch` if `buf[1] != expected_cmd`.
    pub fn from_response(buf: &[u8], expected_cmd: u8) -> Result<CResponse<'_>> {
        unimplemented!("mirror BCommandPacket::from_response")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CResponse<'a> {
    bytes: &'a [u8],
}

impl<'a> CResponse<'a> {
    pub fn command(&self) -> u8 { self.bytes[1] }
    pub fn data_size(&self) -> u32 { /* BE u32 from bytes[2..6] */ unimplemented!() }
    pub fn packet_number(&self) -> u32 { /* BE u24 from bytes[6..9] */ unimplemented!() }
    pub fn length(&self) -> u16 { /* BE u16 from bytes[9..11] */ unimplemented!() }
    pub fn payload(&self) -> &'a [u8] {
        let n = self.length() as usize;
        &self.bytes[HEADER_LEN..HEADER_LEN + n]
    }
}

pub struct CPacketChunks<'a> {
    cmd: u8,
    payload: &'a [u8],
    cursor: usize,
    packet_number: u32,
}

impl<'a> CPacketChunks<'a> {
    #[must_use]
    pub fn new(cmd: u8, payload: &'a [u8]) -> Self {
        Self { cmd, payload, cursor: 0, packet_number: 1 }
    }
}

impl Iterator for CPacketChunks<'_> {
    type Item = CCommandPacket;
    fn next(&mut self) -> Option<Self::Item> {
        // Mirror BPacketChunks::next; emit one zero-length chunk for empty payload
        unimplemented!("mirror BPacketChunks logic with PAYLOAD_MAX = 501")
    }
}
```

(The implementer fills in the bodies by mirroring `bpacket.rs` line by line. Tests are the spec.)

- [ ] **Step 5: Wire the module + run tests**

Add `pub mod cpacket;` to `crates/protocol/src/galahad2_lcdv2/mod.rs` (alongside existing `pub mod bpacket;`).

```bash
cargo build -p lconnect-protocol
cargo test -p lconnect-protocol --test galahad2_lcdv2_cpacket
```

Expected: 8 tests pass.

- [ ] **Step 6: Run full lints**

```bash
cargo clippy -p lconnect-protocol --all-targets -- -D warnings
cargo fmt --all -- --check
cargo doc -p lconnect-protocol --no-deps
```

All clean.

- [ ] **Step 7: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/cpacket.rs \
        crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/tests/galahad2_lcdv2_cpacket.rs
git commit -m "feat(protocol): add CCommandPacket framing (512 out / 512 in)

Third HID framing class for Galahad II LCDV2. ReportID=0x03,
11-byte header (cmd, BE u32 DataSize, BE u24 PacketNumber, BE u16
Length), payload max 501 bytes per packet. Mirrors BCommandPacket
pattern from Phase 0d.

Layout cited from Galahad2Vision/CCommandPacket.cs:108-176 per
Phase 0e static survey. Used by SendH264Frame512 (Task P1A-2)."
```

---

### Task P1A-2: Add SendH264Frame512 codec

**Files:**
- Modify: `crates/protocol/src/galahad2_lcdv2/h264.rs`
- Modify: `crates/protocol/tests/galahad2_lcdv2_h264.rs` (existing test file from Phase 0d)

Phase 0d added `SendH264Frame` (cmd 13, B-class). The C# call site at `Galahad2VisionDevice.cs:262, 353` shows a sibling `SendH264Frame512` that uses C-class framing for the same cmd byte. Different framing → different codec, but same cmd byte (HID-layer discrimination via ReportID). Codec-only — no live verification (would require an H.264 frame stream).

- [ ] **Step 1: Write the failing test**

```rust
// extend crates/protocol/tests/galahad2_lcdv2_h264.rs
use lconnect_protocol::galahad2_lcdv2::h264::{
    build_send_h264_frame_512_request, MAX_H264_FRAME_BYTES,
};

#[test]
fn send_h264_frame_512_empty_rejects() {
    assert!(build_send_h264_frame_512_request(&[]).is_err());
}

#[test]
fn send_h264_frame_512_oversize_rejects() {
    let huge = vec![0u8; MAX_H264_FRAME_BYTES + 1];
    assert!(build_send_h264_frame_512_request(&huge).is_err());
}

#[test]
fn send_h264_frame_512_small_frame_one_packet() {
    let frame = vec![0xAB; 100];
    let packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert_eq!(packets.len(), 1);
    // ReportID 0x03 (C-class), cmd byte 0x0D
    assert_eq!(packets[0][0], 0x03);
    assert_eq!(packets[0][1], 0x0D);
}

#[test]
fn send_h264_frame_512_at_max_one_packet_boundary() {
    // PAYLOAD_MAX for C-class is 501 bytes; verify the boundary
    let frame = vec![0xAB; 501];
    let packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert_eq!(packets.len(), 1);
}

#[test]
fn send_h264_frame_512_multipacket() {
    let frame = vec![0xAB; 1024]; // > PAYLOAD_MAX
    let packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert!(packets.len() >= 2);
}

#[test]
fn send_h264_frame_512_diverges_from_b_class() {
    // Cross-check: the same payload on cmd 13 produces different bytes
    // for the two framing classes (different ReportID + chunk sizes).
    let frame = vec![0xAB; 100];
    let b_packets = lconnect_protocol::galahad2_lcdv2::h264::build_send_h264_frame_request(&frame).unwrap();
    let c_packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert_ne!(b_packets[0][0], c_packets[0][0]); // different ReportID
}
```

- [ ] **Step 2: Run — expect failure**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_h264 2>&1 | tail -15
```

Expected: compile error (`build_send_h264_frame_512_request` doesn't exist).

- [ ] **Step 3: Implement the codec**

In `crates/protocol/src/galahad2_lcdv2/h264.rs`, add the C-class variant alongside the existing B-class function:

```rust
use crate::galahad2_lcdv2::cpacket::{CPacketChunks, OUTPUT_LEN as C_OUTPUT_LEN};

const SEND_H264_FRAME_512_CMD: u8 = 0x0D; // same byte as B-class; ReportID disambiguates

/// Build C-class HID packets to send a single H.264 frame.
///
/// # Errors
///
/// Returns `Error::InvalidH264FrameSize { size, max }` for empty or
/// oversize input (`size > MAX_H264_FRAME_BYTES`).
///
/// # Caveats
///
/// Wire shape verified against the C# call site at
/// `Galahad2VisionDevice.cs:262, 353`. No live verification (would require
/// an H.264 frame stream); Phase 4's theme engine may exercise this.
pub fn build_send_h264_frame_512_request(
    frame_bytes: &[u8],
) -> crate::Result<Vec<[u8; C_OUTPUT_LEN]>> {
    if frame_bytes.is_empty() {
        return Err(crate::Error::InvalidH264FrameSize {
            size: 0, max: MAX_H264_FRAME_BYTES,
        });
    }
    if frame_bytes.len() > MAX_H264_FRAME_BYTES {
        return Err(crate::Error::InvalidH264FrameSize {
            size: frame_bytes.len(), max: MAX_H264_FRAME_BYTES,
        });
    }
    Ok(CPacketChunks::new(SEND_H264_FRAME_512_CMD, frame_bytes)
        .map(|p| *p.as_bytes())
        .collect())
}
```

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_h264
cargo clippy -p lconnect-protocol --all-targets -- -D warnings
cargo fmt --all -- --check
```

All clean.

- [ ] **Step 5: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/h264.rs \
        crates/protocol/tests/galahad2_lcdv2_h264.rs
git commit -m "feat(protocol): add SendH264Frame512 codec (cmd 13, C-class)

Sibling of the existing B-class SendH264Frame from Phase 0d. Same
cmd byte (0x0D) but C-class framing (ReportID=0x03, 512/512). HID-
layer discrimination via ReportID. Codec-only; live verification
deferred until an H.264 frame pipeline exists.

Wire shape verified against Galahad2VisionDevice.cs:262,353 per
Phase 0e static survey. Cross-check test pins byte-divergence
between the two cmd-13 codecs."
```

---

## Sub-phase B — `lconnect-hid` crate

### Task P1B-1: Scaffold the `lconnect-hid` crate

**Files:**
- Create: `crates/lconnect-hid/Cargo.toml`
- Create: `crates/lconnect-hid/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add member + workspace dependency

- [ ] **Step 1: Create the crate's `Cargo.toml`**

```toml
[package]
name = "lconnect-hid"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]
hidapi.workspace = true
thiserror.workspace = true
tracing = "0.1"
```

- [ ] **Step 2: Create `lib.rs` with crate-level forbidden-unsafe + module skeletons**

```rust
//! HID transport abstraction for the lconnect-linux project.
//!
//! Owns the `HidBackend` trait, `Framing` enum, device-info types, and
//! the per-family registry. Two backends ship: `HidApiBackend` (real
//! hidapi-rs) and `MockBackend` (in-process fake for tests).
//!
//! # See also
//!
//! - [`crate::backend`] — `HidBackend` trait definition
//! - [`crate::framing`] — `Framing` enum (extensible per Phase 0e)
//! - [`crate::family_registry`] — `DeviceFamily` trait + LCDV2 registration

#![forbid(unsafe_code)]

pub mod backend;
pub mod device_info;
pub mod family_registry;
pub mod framing;
pub mod hidapi_backend;
pub mod mock_backend;

pub use backend::{HidBackend, OpenHandle};
pub use device_info::{DeviceFamilyId, DeviceInfo};
pub use family_registry::{DeviceFamily, FamilyRegistry};
pub use framing::Framing;

/// Crate-level error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("hidapi error: {0}")]
    HidApi(#[from] hidapi::HidError),
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("short write: {written}/{expected} bytes")]
    ShortWrite { written: usize, expected: usize },
    #[error("read timeout after {ms} ms")]
    ReadTimeout { ms: u64 },
    #[error("response too short: got {got} bytes, expected at least {expected}")]
    ShortResponse { got: usize, expected: usize },
    #[error("response report ID mismatch: got 0x{got:02x}, expected 0x{expected:02x}")]
    ReportIdMismatch { got: u8, expected: u8 },
    #[error("response cmd echo mismatch: got 0x{got:02x}, expected 0x{expected:02x}")]
    CommandEchoMismatch { got: u8, expected: u8 },
    #[error("drained {stale} stale + {timeout} timeout responses without echo of cmd 0x{expected_cmd:02x}")]
    EchoExhausted { stale: u32, timeout: u32, expected_cmd: u8 },
}

pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 3: Create empty module files**

Create stub files so the crate compiles before each module is filled in. Each gets a `// implementation lands in P1B-N` comment:

```bash
for f in backend device_info family_registry framing hidapi_backend mock_backend; do
  printf '//! Stub - implementation lands in subsequent P1B-* tasks.\n\n' > "crates/lconnect-hid/src/${f}.rs"
done
```

- [ ] **Step 4: Wire the workspace**

Edit `Cargo.toml` (root) to add `crates/lconnect-hid` to `[workspace] members` and any new workspace dependencies (`tokio` is added in a later task; `tracing` is added now):

```toml
[workspace]
members = [
    "crates/protocol",
    "crates/lconnect-image",
    "crates/lconnect-hid",       # NEW
    "tools/verify-galahad",
]

[workspace.dependencies]
# ...existing entries...
tracing = "0.1"                   # NEW
```

- [ ] **Step 5: Build + verify**

```bash
cargo build -p lconnect-hid
cargo clippy -p lconnect-hid --all-targets -- -D warnings
cargo fmt --all -- --check
cargo doc -p lconnect-hid --no-deps
```

All clean (the empty-module stubs compile; `Error` is the only filled-in surface).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/lconnect-hid/
git commit -m "feat(hid): scaffold lconnect-hid crate

Empty crate root + module stubs + Error enum. Module bodies land
in P1B-2 (framing + backend trait), P1B-3 (hidapi-rs backend),
P1B-4 (mock backend), P1B-5 (family registry)."
```

---

### Task P1B-2: `Framing` enum + `HidBackend` trait + `DeviceInfo`

**Files:**
- Modify: `crates/lconnect-hid/src/framing.rs`
- Modify: `crates/lconnect-hid/src/backend.rs`
- Modify: `crates/lconnect-hid/src/device_info.rs`
- Create: `crates/lconnect-hid/tests/framing.rs`

- [ ] **Step 1: Write the failing tests for `Framing`**

```rust
// crates/lconnect-hid/tests/framing.rs
use lconnect_hid::Framing;

#[test]
fn framing_a() {
    assert_eq!(Framing::A.output_len(), 64);
    assert_eq!(Framing::A.input_len(), 64);
    assert_eq!(Framing::A.report_id(), 0x01);
}

#[test]
fn framing_b() {
    assert_eq!(Framing::B.output_len(), 1024);
    assert_eq!(Framing::B.input_len(), 512);
    assert_eq!(Framing::B.report_id(), 0x02);
}

#[test]
fn framing_c() {
    assert_eq!(Framing::C.output_len(), 512);
    assert_eq!(Framing::C.input_len(), 512);
    assert_eq!(Framing::C.report_id(), 0x03);
}
```

- [ ] **Step 2: Implement `Framing` (mirror the verify-galahad probe shape)**

```rust
// crates/lconnect-hid/src/framing.rs
//! HID framing classes used by Lian Li devices.
//!
//! Phase 1 ships A/B/C — Phase 0e identified at least 7 distinct framing
//! classes across the LConnect3 device universe (TLLCD T-class,
//! StrimerPlus HID-feature, lcd207 WinUSB-DES, slv3 WinUSB-RF,
//! UniversalScreen bulk). The enum is `#[non_exhaustive]` so future
//! families add variants without breaking existing matches.
//!
//! # See also
//!
//! - [`crate::backend::HidBackend`] — uses `Framing` to size buffers
//! - `docs/protocol/cross-device-survey.md` section 6.1 — extension points

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Framing {
    /// A-class: 64-byte output / 64-byte input, ReportID 0x01.
    A,
    /// B-class: 1024-byte output / 512-byte input, ReportID 0x02.
    B,
    /// C-class: 512-byte output / 512-byte input, ReportID 0x03.
    C,
}

impl Framing {
    #[must_use]
    pub fn output_len(self) -> usize {
        match self { Framing::A => 64, Framing::B => 1024, Framing::C => 512 }
    }
    #[must_use]
    pub fn input_len(self) -> usize {
        match self { Framing::A => 64, Framing::B => 512, Framing::C => 512 }
    }
    #[must_use]
    pub fn report_id(self) -> u8 {
        match self { Framing::A => 0x01, Framing::B => 0x02, Framing::C => 0x03 }
    }
}
```

- [ ] **Step 3: Implement `DeviceInfo`**

```rust
// crates/lconnect-hid/src/device_info.rs
//! Device identity types.
//!
//! `DeviceFamilyId` is a stable string slug per device family
//! ("galahad2-lcdv2", future "hydroshift-ii-lcd", etc.). `DeviceInfo`
//! is what the daemon hands out via its IPC `DeviceDescriptor`.

use std::path::PathBuf;

/// Stable family slug. Used in opaque `DeviceId`s and persistence keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceFamilyId(pub String);

impl DeviceFamilyId {
    #[must_use]
    pub fn new(slug: impl Into<String>) -> Self { Self(slug.into()) }
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Per-device info populated from `hidapi` enumeration.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DeviceInfo {
    pub family: DeviceFamilyId,
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    /// Linux hidraw path (`/dev/hidraw5`) or platform equivalent.
    pub path: PathBuf,
}
```

- [ ] **Step 4: Implement `HidBackend` trait**

```rust
// crates/lconnect-hid/src/backend.rs
//! `HidBackend` trait.
//!
//! Two impls ship: `HidApiBackend` (production) and `MockBackend` (tests).
//! The daemon takes `Arc<dyn HidBackend>` so swapping backends in tests
//! requires no production-code change.

use crate::{DeviceInfo, Framing, Result};

/// Opened device handle. Reads + writes are synchronous (`hidapi-rs` is
/// sync); the daemon wraps each call in `tokio::task::spawn_blocking`.
pub trait OpenHandle: Send + Sync {
    fn write(&self, buf: &[u8]) -> Result<usize>;
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize>;
}

pub trait HidBackend: Send + Sync {
    /// Enumerate devices currently visible to this backend.
    fn enumerate(&self) -> Result<Vec<DeviceInfo>>;

    /// Open a device by its info (typically obtained from `enumerate`).
    fn open(&self, info: &DeviceInfo) -> Result<Box<dyn OpenHandle>>;
}

/// Default echo convention shared by Galahad II Vision: `[ReportID, cmd, ...]`.
/// Returns `true` if `buf[0] == framing.report_id()` and `buf[1] == cmd`.
///
/// Unknown framings should not match; this is the expected echo for A/B/C only.
#[must_use]
pub fn matches_echo(buf: &[u8], framing: Framing, cmd: u8) -> bool {
    buf.len() >= 2 && buf[0] == framing.report_id() && buf[1] == cmd
}
```

- [ ] **Step 5: Run tests + lints + commit**

```bash
cargo test -p lconnect-hid
cargo clippy -p lconnect-hid --all-targets -- -D warnings
cargo fmt --all -- --check

git add crates/lconnect-hid/src/framing.rs \
        crates/lconnect-hid/src/backend.rs \
        crates/lconnect-hid/src/device_info.rs \
        crates/lconnect-hid/tests/framing.rs
git commit -m "feat(hid): add Framing enum + HidBackend trait + DeviceInfo

Framing is non_exhaustive (Phase 0e finding: 7+ framing classes
exist across the LConnect3 device universe; A/B/C ship now).
HidBackend trait abstracts hidapi-rs vs mock; OpenHandle is Send +
Sync so daemon can hold it inside tokio actors.

matches_echo helper encapsulates the [ReportID, cmd] convention
shared by Galahad II Vision; mirrors verify-galahad probe behavior."
```

---

### Task P1B-3: hidapi-rs backend implementation

**Files:**
- Modify: `crates/lconnect-hid/src/hidapi_backend.rs`

- [ ] **Step 1: Write the integration shape (no test — needs hardware)**

```rust
//! Real hidapi-rs backend for production use.
//!
//! Hardware-required: cannot be unit-tested without an attached device.
//! Tests live in `MockBackend` (P1B-4) and the daemon's actor smoke
//! tests use the mock; live device runs are P1H-1.

use crate::{DeviceFamilyId, DeviceInfo, Error, OpenHandle, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Production backend wrapping a single `HidApi` instance.
///
/// `HidApi::new()` is heavy (probes the USB tree); construct once and
/// share via `Arc`. Re-enumeration uses the existing instance via
/// `refresh_devices`.
pub struct HidApiBackend {
    api: Arc<Mutex<hidapi::HidApi>>,
}

impl HidApiBackend {
    /// Create a new backend instance.
    ///
    /// # Errors
    ///
    /// Returns `Error::HidApi` if `hidapi::HidApi::new()` fails (typically
    /// missing udev permissions).
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: Arc::new(Mutex::new(hidapi::HidApi::new()?)),
        })
    }
}

impl crate::HidBackend for HidApiBackend {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>> {
        let mut api = self.api.lock().expect("hidapi mutex poisoned");
        api.refresh_devices()?;
        let mut out = Vec::new();
        for di in api.device_list() {
            // Phase 1 only knows LCDV2; future families add via family_registry.
            if di.vendor_id() == 0x0416 && di.product_id() == 0x7395 {
                out.push(DeviceInfo {
                    family: DeviceFamilyId::new("galahad2-lcdv2"),
                    vid: di.vendor_id(),
                    pid: di.product_id(),
                    serial: di.serial_number().map(str::to_owned),
                    path: PathBuf::from(di.path().to_string_lossy().into_owned()),
                });
            }
        }
        Ok(out)
    }

    fn open(&self, info: &DeviceInfo) -> Result<Box<dyn OpenHandle>> {
        let api = self.api.lock().expect("hidapi mutex poisoned");
        let dev = api.open_path(&hidapi::HidApi::open_path_str(&api, info.path.to_string_lossy().as_ref())?)?;
        Ok(Box::new(HidApiHandle { dev }))
    }
}

struct HidApiHandle {
    dev: hidapi::HidDevice,
}

// Safety: hidapi::HidDevice is internally synchronized and exposes only
// &self methods. Wrapping in Send + Sync is sound for the daemon's
// per-device serialization model (one mpsc consumer per handle).
unsafe impl Send for HidApiHandle {}
unsafe impl Sync for HidApiHandle {}

impl OpenHandle for HidApiHandle {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let n = self.dev.write(buf)?;
        if n != buf.len() {
            return Err(Error::ShortWrite { written: n, expected: buf.len() });
        }
        Ok(n)
    }

    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        Ok(self.dev.read_timeout(buf, timeout_ms)?)
    }
}
```

**Note:** the `unsafe impl Send + Sync` above breaks the `#![forbid(unsafe_code)]` rule. Resolution: this single block lives behind a `#[allow(unsafe_code)]` block-attribute in the source, with a `// SAFETY:` comment. The implementer should add this with care.

Alternative if blocking on the unsafe: the daemon serializes per-device HID I/O through an mpsc anyway, so wrapping the whole thing in `Arc<Mutex<HidDevice>>` and exposing only `&self` methods sidesteps the issue. Implementer chooses; the trait shape is identical either way.

- [ ] **Step 2: Build + commit**

```bash
cargo build -p lconnect-hid
cargo clippy -p lconnect-hid --all-targets -- -D warnings
cargo fmt --all -- --check

git add crates/lconnect-hid/src/hidapi_backend.rs
git commit -m "feat(hid): hidapi-rs backend for HidBackend trait

Production backend wrapping HidApi. Enumerate filters to LCDV2
VID:PID (0x0416:0x7395) for Phase 1; family registry (P1B-5)
will replace the hardcoded filter with a per-family scan. Live
verification deferred to P1H-1 smoke run."
```

---

### Task P1B-4: Mock backend implementation

**Files:**
- Modify: `crates/lconnect-hid/src/mock_backend.rs`
- Create: `crates/lconnect-hid/tests/mock_backend.rs`

- [ ] **Step 1: Write tests defining mock behavior**

```rust
// crates/lconnect-hid/tests/mock_backend.rs
use lconnect_hid::mock_backend::MockBackend;
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend};
use std::path::PathBuf;

fn lcdv2_info() -> DeviceInfo {
    DeviceInfo {
        family: DeviceFamilyId::new("galahad2-lcdv2"),
        vid: 0x0416, pid: 0x7395, serial: None,
        path: PathBuf::from("/mock/hidraw0"),
    }
}

#[test]
fn enumerate_returns_registered_devices() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let list = mock.enumerate().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].family.as_str(), "galahad2-lcdv2");
}

#[test]
fn open_returns_handle_for_registered_device() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    assert!(mock.open(&lcdv2_info()).is_ok());
}

#[test]
fn handle_records_writes() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let handle = mock.open(&lcdv2_info()).unwrap();
    handle.write(&[0x01, 0x86]).unwrap();
    let history = mock.write_history(&lcdv2_info());
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], vec![0x01, 0x86]);
}

#[test]
fn handle_replays_canned_response() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    mock.queue_response(&lcdv2_info(), vec![0x01, 0x86, 0x00, 0x00, 0x00, 0x05, 1, 2, 3, 4, 5]);
    let handle = mock.open(&lcdv2_info()).unwrap();
    let mut buf = [0u8; 64];
    let n = handle.read_timeout(&mut buf, 100).unwrap();
    assert_eq!(&buf[..n], &[0x01, 0x86, 0x00, 0x00, 0x00, 0x05, 1, 2, 3, 4, 5]);
}

#[test]
fn read_timeout_when_no_responses_queued() {
    use lconnect_hid::Error;
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let handle = mock.open(&lcdv2_info()).unwrap();
    let mut buf = [0u8; 64];
    match handle.read_timeout(&mut buf, 100) {
        Err(Error::ReadTimeout { .. }) => {}
        other => panic!("expected ReadTimeout, got {other:?}"),
    }
}
```

- [ ] **Step 2: Implement `MockBackend`**

```rust
// crates/lconnect-hid/src/mock_backend.rs
//! In-process mock backend for tests.
//!
//! - `add_device(info)` registers a device for enumeration
//! - `queue_response(info, bytes)` enqueues a canned read response
//! - `write_history(info)` returns the write log for assertions

use crate::{DeviceInfo, Error, HidBackend, OpenHandle, Result};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DeviceState {
    write_history: Vec<Vec<u8>>,
    queued_responses: VecDeque<Vec<u8>>,
}

#[derive(Default)]
struct Inner {
    devices: Vec<DeviceInfo>,
    states: std::collections::HashMap<DeviceKey, Arc<Mutex<DeviceState>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DeviceKey {
    vid: u16,
    pid: u16,
    path: std::path::PathBuf,
}

impl DeviceKey {
    fn from(info: &DeviceInfo) -> Self {
        Self { vid: info.vid, pid: info.pid, path: info.path.clone() }
    }
}

#[derive(Default, Clone)]
pub struct MockBackend {
    inner: Arc<Mutex<Inner>>,
}

impl MockBackend {
    pub fn new() -> Self { Self::default() }

    pub fn add_device(&self, info: DeviceInfo) {
        let mut inner = self.inner.lock().unwrap();
        let key = DeviceKey::from(&info);
        inner.devices.push(info);
        inner.states.entry(key).or_default();
    }

    pub fn queue_response(&self, info: &DeviceInfo, bytes: Vec<u8>) {
        let inner = self.inner.lock().unwrap();
        let state = inner.states.get(&DeviceKey::from(info))
            .expect("device not registered").clone();
        drop(inner);
        state.lock().unwrap().queued_responses.push_back(bytes);
    }

    pub fn write_history(&self, info: &DeviceInfo) -> Vec<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        inner.states.get(&DeviceKey::from(info))
            .map(|s| s.lock().unwrap().write_history.clone())
            .unwrap_or_default()
    }
}

impl HidBackend for MockBackend {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self.inner.lock().unwrap().devices.clone())
    }

    fn open(&self, info: &DeviceInfo) -> Result<Box<dyn OpenHandle>> {
        let inner = self.inner.lock().unwrap();
        let state = inner.states.get(&DeviceKey::from(info))
            .ok_or_else(|| Error::DeviceNotFound(format!("{:?}", info.path)))?
            .clone();
        Ok(Box::new(MockHandle { state }))
    }
}

struct MockHandle {
    state: Arc<Mutex<DeviceState>>,
}

impl OpenHandle for MockHandle {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        self.state.lock().unwrap().write_history.push(buf.to_vec());
        Ok(buf.len())
    }
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        let mut state = self.state.lock().unwrap();
        if let Some(canned) = state.queued_responses.pop_front() {
            let n = canned.len().min(buf.len());
            buf[..n].copy_from_slice(&canned[..n]);
            return Ok(n);
        }
        Err(Error::ReadTimeout { ms: timeout_ms.max(0) as u64 })
    }
}
```

- [ ] **Step 3: Run tests + commit**

```bash
cargo test -p lconnect-hid
cargo clippy -p lconnect-hid --all-targets -- -D warnings
cargo fmt --all -- --check

git add crates/lconnect-hid/src/mock_backend.rs \
        crates/lconnect-hid/tests/mock_backend.rs
git commit -m "feat(hid): MockBackend for testing actor topologies

In-process fake; supports enumerate, open, write-history capture,
and queued read responses. Used by the daemon's per-actor unit
tests to drive sensor / lighting / LCD flows without hardware.

Live verification of the real HidApiBackend remains the smoke run
(P1H-1) since hidapi-rs cannot be deterministically tested in CI."
```

---

### Task P1B-5: Family registry

**Files:**
- Modify: `crates/lconnect-hid/src/family_registry.rs`
- Create: `crates/lconnect-hid/tests/family_registry.rs`

- [ ] **Step 1: Write tests**

```rust
// crates/lconnect-hid/tests/family_registry.rs
use lconnect_hid::{DeviceFamily, FamilyRegistry, DeviceFamilyId};

#[test]
fn registry_initially_empty() {
    let r = FamilyRegistry::new();
    assert!(r.lookup_vid_pid(0x0416, 0x7395).is_none());
}

#[test]
fn registry_with_lcdv2_resolves_vid_pid() {
    let r = FamilyRegistry::with_default_lcdv2();
    let f = r.lookup_vid_pid(0x0416, 0x7395).unwrap();
    assert_eq!(f.id().as_str(), "galahad2-lcdv2");
}

#[test]
fn registry_unknown_vid_pid_returns_none() {
    let r = FamilyRegistry::with_default_lcdv2();
    assert!(r.lookup_vid_pid(0x1234, 0x5678).is_none());
}
```

- [ ] **Step 2: Implement**

```rust
// crates/lconnect-hid/src/family_registry.rs
//! Per-device-family registry.
//!
//! A `DeviceFamily` declares VID/PID pairs and identity. Phase 1 ships
//! one impl: `Galahad2LcdV2Family` (`0x0416:0x7395`). Phase 3+ adds
//! more by registering additional impls without touching the daemon
//! core.

use crate::DeviceFamilyId;
use std::sync::Arc;

pub trait DeviceFamily: Send + Sync {
    /// Stable family slug.
    fn id(&self) -> DeviceFamilyId;
    /// VID/PID pairs this family claims.
    fn vid_pids(&self) -> &[(u16, u16)];
    /// Marketed name(s) for human-readable output.
    fn display_name(&self) -> &'static str;
}

pub struct FamilyRegistry {
    families: Vec<Arc<dyn DeviceFamily>>,
}

impl FamilyRegistry {
    #[must_use]
    pub fn new() -> Self { Self { families: Vec::new() } }

    pub fn register(&mut self, family: Arc<dyn DeviceFamily>) {
        self.families.push(family);
    }

    /// Lookup a family by VID/PID. Returns `None` if no registered
    /// family claims the pair.
    #[must_use]
    pub fn lookup_vid_pid(&self, vid: u16, pid: u16) -> Option<Arc<dyn DeviceFamily>> {
        self.families.iter()
            .find(|f| f.vid_pids().iter().any(|&(v, p)| v == vid && p == pid))
            .cloned()
    }

    /// Phase 1 default registry: only LCDV2.
    #[must_use]
    pub fn with_default_lcdv2() -> Self {
        let mut r = Self::new();
        r.register(Arc::new(Galahad2LcdV2Family));
        r
    }
}

impl Default for FamilyRegistry {
    fn default() -> Self { Self::with_default_lcdv2() }
}

pub struct Galahad2LcdV2Family;

impl DeviceFamily for Galahad2LcdV2Family {
    fn id(&self) -> DeviceFamilyId { DeviceFamilyId::new("galahad2-lcdv2") }
    fn vid_pids(&self) -> &[(u16, u16)] { &[(0x0416, 0x7395)] }
    fn display_name(&self) -> &'static str { "Galahad II Vision (LCDV2)" }
}
```

- [ ] **Step 3: Update `HidApiBackend::enumerate` to use the registry**

Replace the hardcoded VID/PID filter from P1B-3 with a `FamilyRegistry::lookup_vid_pid` call. The backend takes the registry as a constructor argument:

```rust
// in hidapi_backend.rs
pub struct HidApiBackend {
    api: Arc<Mutex<hidapi::HidApi>>,
    registry: FamilyRegistry,
}

impl HidApiBackend {
    pub fn new(registry: FamilyRegistry) -> Result<Self> { ... }
}

// in enumerate:
for di in api.device_list() {
    if let Some(family) = self.registry.lookup_vid_pid(di.vendor_id(), di.product_id()) {
        out.push(DeviceInfo {
            family: family.id(),
            vid: di.vendor_id(),
            pid: di.product_id(),
            serial: di.serial_number().map(str::to_owned),
            path: PathBuf::from(di.path().to_string_lossy().into_owned()),
        });
    }
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo test -p lconnect-hid
cargo clippy -p lconnect-hid --all-targets -- -D warnings
cargo fmt --all -- --check

git add crates/lconnect-hid/src/family_registry.rs \
        crates/lconnect-hid/src/hidapi_backend.rs \
        crates/lconnect-hid/tests/family_registry.rs
git commit -m "feat(hid): family registry with default LCDV2 registration

DeviceFamily trait declares VID/PID + identity; FamilyRegistry::
with_default_lcdv2() is the Phase 1 default. HidApiBackend::
enumerate now consults the registry instead of hardcoding the
LCDV2 filter, leaving the seam clean for Phase 3+ multi-device
support."
```

---

## Sub-phase C — `lconnect-ipc` crate

### Task P1C-1: Scaffold `lconnect-ipc` + types

**Files:**
- Create: `crates/lconnect-ipc/Cargo.toml`
- Create: `crates/lconnect-ipc/src/lib.rs`
- Create: `crates/lconnect-ipc/src/types.rs`
- Create: `crates/lconnect-ipc/src/error.rs`
- Create: `crates/lconnect-ipc/tests/types.rs`
- Modify: workspace `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "lconnect-ipc"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror.workspace = true
async-trait = "0.1"
futures = "0.3"
tokio = { workspace = true, features = ["sync", "io-util", "net"] }
tracing.workspace = true
bytes = "1"
```

Add to workspace `[workspace.dependencies]`:
```toml
serde = "1"
serde_json = "1"
tokio = { version = "1", default-features = false }
```

And `crates/lconnect-ipc` to `[workspace] members`.

- [ ] **Step 2: Define core types in `types.rs`**

```rust
//! IPC types shared between daemon and clients.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stable opaque device ID, e.g. "galahad2-lcdv2:0001".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

impl DeviceId {
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Per-device descriptor returned by `list_devices` / `device_attached`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DeviceDescriptor {
    pub id: DeviceId,
    pub family: String,
    pub display_name: String,
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub hid_path: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SensorSnapshot {
    pub coolant_c_x10: i16, // tenths of C; serialised as integer to keep wire compact
    pub fan_rpm: u16,
    pub pump_rpm: u16,
    pub temp_valid: bool,
    pub last_seen_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FirmwareInfo {
    pub version: String,
    pub raw_token: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pwm(pub u8);

impl Pwm {
    /// # Errors
    /// Returns Err if `percent > 100`.
    pub fn new(percent: u8) -> Result<Self, String> {
        if percent > 100 { return Err(format!("PWM out of range: {percent}")); }
        Ok(Self(percent))
    }
    pub fn percent(self) -> u8 { self.0 }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PwmSource { Mcu, Application }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LightingKind { Fan, Pump }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LedTarget { Fan, Screen }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LightingConfig {
    pub mode: String,        // serialised as the enum variant name
    pub color: Rgb,
    pub brightness_percent: u8,
    pub speed_percent: u8,
    pub direction: String,   // "forward" | "reverse"
    pub scope: Option<String>, // pump-only ("inner" | "outer" | "all")
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingInfo {
    pub max_speed: u8,
    pub max_brightness: u8,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LcdConfig {
    pub mode: String,                // "LocalUI" | "Application" | ...
    pub brightness_percent: u8,
    pub rotation_degrees: u16,       // 0 / 90 / 180 / 270
}

/// Cryptographically random token returned by `request_reset_token`.
/// Single-use; expires 30s after issuance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetToken(pub String);

/// Wire framing identifier for `raw_send`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum FramingKind { A, B, C }

/// Aggregated daemon counters returned by `daemon_stats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DaemonStats {
    pub uptime_seconds: u64,
    pub devices_known: u32,
    pub hid_write_count: u64,
    pub hid_read_errors: u64,
    pub hid_decode_errors: u64,
    pub ipc_request_count: u64,
}

/// Brief structured error for `device_error` notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBrief {
    pub category: String,
    pub message: String,
}
```

- [ ] **Step 3: Implement `error.rs`**

```rust
//! IPC error type + JSON-RPC error code mapping.

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IpcError {
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("device busy / IO error: {0}")]
    DeviceBusy(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("confirmation token required")]
    ConfirmationRequired,
    #[error("protocol mismatch: {0}")]
    ProtocolMismatch(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl IpcError {
    #[must_use]
    pub fn jsonrpc_code(&self) -> i32 {
        match self {
            IpcError::DeviceNotFound(_) => -32000,
            IpcError::DeviceBusy(_) => -32001,
            IpcError::InvalidArgument(_) => -32002,
            IpcError::NotImplemented(_) => -32003,
            IpcError::ConfirmationRequired => -32004,
            IpcError::ProtocolMismatch(_) => -32005,
            IpcError::Transport(_) | IpcError::Serialization(_) => -32603,
        }
    }
}

pub type Result<T> = std::result::Result<T, IpcError>;
```

- [ ] **Step 4: `lib.rs`**

```rust
#![forbid(unsafe_code)]

pub mod client;
pub mod error;
pub mod jsonrpc;
pub mod server;
pub mod trait_def;
pub mod transport_memory;
pub mod types;

pub use error::{IpcError, Result};
pub use trait_def::DaemonRpc;
pub use types::*;

pub const IPC_VERSION_MAJOR: u32 = 1;
pub const IPC_VERSION_MINOR: u32 = 0;
```

- [ ] **Step 5: Round-trip tests for serde**

```rust
// crates/lconnect-ipc/tests/types.rs
use lconnect_ipc::*;

#[test]
fn device_id_roundtrip() {
    let id = DeviceId::new("galahad2-lcdv2:0001");
    let s = serde_json::to_string(&id).unwrap();
    let d: DeviceId = serde_json::from_str(&s).unwrap();
    assert_eq!(d, id);
}

#[test]
fn pwm_validates_range() {
    assert!(Pwm::new(0).is_ok());
    assert!(Pwm::new(100).is_ok());
    assert!(Pwm::new(101).is_err());
}

#[test]
fn pwm_source_roundtrip() {
    let s = serde_json::to_string(&PwmSource::Mcu).unwrap();
    assert_eq!(s, r#""mcu""#);
}

#[test]
fn rgb_roundtrip() {
    let c = Rgb { r: 255, g: 0, b: 64 };
    let s = serde_json::to_string(&c).unwrap();
    let d: Rgb = serde_json::from_str(&s).unwrap();
    assert_eq!(d, c);
}

#[test]
fn ipc_error_codes() {
    assert_eq!(IpcError::DeviceNotFound("x".into()).jsonrpc_code(), -32000);
    assert_eq!(IpcError::InvalidArgument("y".into()).jsonrpc_code(), -32002);
    assert_eq!(IpcError::ConfirmationRequired.jsonrpc_code(), -32004);
}
```

- [ ] **Step 6: Stub the remaining modules** (filled in by P1C-2..P1C-6):

```bash
for f in client jsonrpc server trait_def transport_memory; do
  printf '//! Stub - implementation lands in subsequent P1C-* tasks.\n\n' \
    > "crates/lconnect-ipc/src/${f}.rs"
done
```

- [ ] **Step 7: Build + commit**

```bash
cargo build -p lconnect-ipc
cargo test -p lconnect-ipc
cargo clippy -p lconnect-ipc --all-targets -- -D warnings
cargo fmt --all -- --check

git add Cargo.toml crates/lconnect-ipc/
git commit -m "feat(ipc): scaffold lconnect-ipc crate with types + error mapping

DeviceId, DeviceDescriptor, SensorSnapshot, FirmwareInfo, Pwm/PwmSource,
LightingKind/LedTarget/Rgb/LightingConfig/LightingInfo, LcdConfig,
ResetToken, FramingKind, DaemonStats, ErrorBrief; IpcError + JSON-RPC
code mapping (-32000..-32005). Module stubs for trait_def/jsonrpc/
server/client/transport_memory ahead of P1C-2..P1C-6."
```

---

### Task P1C-2: `DaemonRpc` trait

**Files:**
- Modify: `crates/lconnect-ipc/src/trait_def.rs`

- [ ] **Step 1: Implement the trait per spec section 5**

```rust
//! `DaemonRpc` trait — the IPC contract.
//!
//! Implementations: the daemon's `IpcServerImpl` (production) and any
//! mocks the test harness defines. The trait is the seam for a future
//! DBus migration: only the transport changes; the trait stays put.

use crate::*;
use async_trait::async_trait;
use futures::stream::BoxStream;

#[async_trait]
pub trait DaemonRpc: Send + Sync + 'static {
    // Discovery
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>>;
    async fn get_firmware(&self, dev: DeviceId) -> Result<FirmwareInfo>;

    // Sensors
    async fn get_snapshot(&self, dev: DeviceId) -> Result<SensorSnapshot>;
    async fn subscribe_snapshots(
        &self, dev: DeviceId,
    ) -> Result<BoxStream<'static, SensorSnapshot>>;

    // PWM
    async fn set_fan_pwm(&self, dev: DeviceId, percent: Pwm) -> Result<()>;
    async fn set_pump_pwm(
        &self, dev: DeviceId, percent: Pwm, source: PwmSource,
    ) -> Result<()>;

    // Lighting
    async fn get_lighting_info(&self, dev: DeviceId, kind: LightingKind)
        -> Result<LightingInfo>;
    async fn set_lighting(
        &self, dev: DeviceId, kind: LightingKind, cfg: LightingConfig,
    ) -> Result<()>;
    async fn set_led_colors(
        &self, dev: DeviceId, target: LedTarget, colors: Vec<Rgb>,
    ) -> Result<()>;

    // LCD
    async fn is_lcd_available(&self, dev: DeviceId) -> Result<bool>;
    async fn set_lcd_config(&self, dev: DeviceId, cfg: LcdConfig) -> Result<()>;
    async fn set_lcd_image(&self, dev: DeviceId, jpeg: Vec<u8>) -> Result<()>;
    async fn clear_lcd(&self, dev: DeviceId) -> Result<()>;

    // Maintenance
    async fn request_reset_token(&self, dev: DeviceId) -> Result<ResetToken>;
    async fn reset_device(&self, dev: DeviceId, confirm: ResetToken) -> Result<()>;
    async fn raw_send(
        &self, dev: DeviceId, framing: FramingKind, bytes: Vec<u8>,
    ) -> Result<Vec<u8>>;
    async fn daemon_stats(&self) -> Result<DaemonStats>;
}
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p lconnect-ipc

git add crates/lconnect-ipc/src/trait_def.rs
git commit -m "feat(ipc): DaemonRpc trait covering Phase 1 IPC surface

Async-trait-based contract per spec section 5. 17 methods across
discovery, sensors, PWM, lighting, LCD, maintenance. subscribe_
snapshots returns BoxStream so notifications can ride the same
JSON-RPC connection. Implementations land in P1D-13 (server-side)
and the in-memory transport (P1C-6)."
```

---

### Task P1C-3: JSON-RPC line-delimited framing

**Files:**
- Modify: `crates/lconnect-ipc/src/jsonrpc.rs`
- Create: `crates/lconnect-ipc/tests/jsonrpc.rs`

- [ ] **Step 1: Test the framing**

```rust
// crates/lconnect-ipc/tests/jsonrpc.rs
use lconnect_ipc::jsonrpc::{Request, Response, ErrorObj, ResponsePayload};
use serde_json::json;

#[test]
fn request_serializes_with_jsonrpc_2_0_envelope() {
    let r = Request::new(42, "list_devices", json!({}));
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""jsonrpc":"2.0""#));
    assert!(s.contains(r#""method":"list_devices""#));
    assert!(s.contains(r#""id":42"#));
}

#[test]
fn response_with_result() {
    let r = Response { jsonrpc: "2.0".into(), id: Some(json!(7)),
        payload: ResponsePayload::Result(json!({"ok":true})) };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""result":{"ok":true}"#));
}

#[test]
fn response_with_error() {
    let r = Response { jsonrpc: "2.0".into(), id: Some(json!(7)),
        payload: ResponsePayload::Error(ErrorObj {
            code: -32000, message: "device not found".into(), data: None,
        }) };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""code":-32000"#));
}
```

- [ ] **Step 2: Implement**

```rust
//! JSON-RPC 2.0 envelope types + line-delimited framing helpers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Value,
}

impl Request {
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id: serde_json::json!(id),
               method: method.into(), params }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(flatten)]
    pub payload: ResponsePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsePayload {
    Result(Value),
    Error(ErrorObj),
}

// Note: serde(untagged) above is a placeholder; in practice use the explicit
// `result` / `error` field discriminator with #[serde(rename = "result")]
// and #[serde(rename = "error")]. The implementer should refine this.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorObj {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Server-pushed notification on the same connection (no `id` field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

impl Notification {
    #[must_use]
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self { jsonrpc: "2.0".into(), method: method.into(), params }
    }
}
```

- [ ] **Step 3: Build + run tests + commit**

```bash
cargo test -p lconnect-ipc --test jsonrpc
cargo clippy -p lconnect-ipc --all-targets -- -D warnings
cargo fmt --all -- --check

git add crates/lconnect-ipc/src/jsonrpc.rs \
        crates/lconnect-ipc/tests/jsonrpc.rs
git commit -m "feat(ipc): JSON-RPC 2.0 envelope types

Request, Response, ErrorObj, Notification. Line-delimited framing
(one JSON object per newline) is enforced at the transport layer
(server.rs / client.rs) — these types are pure (de)serialisation."
```

---

### Task P1C-4: JSON-RPC server scaffold (over Unix socket)

**Files:**
- Modify: `crates/lconnect-ipc/src/server.rs`

This task scaffolds the bytes-on-the-wire server side. Wiring to actual `DaemonRpc` impl methods happens in P1D-13.

- [ ] **Step 1: Implement the server**

```rust
//! JSON-RPC 2.0 server over UnixListener.
//!
//! `IpcServer::serve(rpc, socket_path)` accepts long-lived connections,
//! reads line-delimited JSON-RPC requests, dispatches to a `DaemonRpc`
//! impl, and writes responses on the same connection. Notifications
//! (server-pushed, e.g., `snapshot`) flow on the same connection.

use crate::{
    jsonrpc::{ErrorObj, Notification, Request, Response, ResponsePayload},
    DaemonRpc, IpcError,
};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info, warn};

pub struct IpcServer<R: DaemonRpc> {
    rpc: Arc<R>,
}

impl<R: DaemonRpc> IpcServer<R> {
    #[must_use]
    pub fn new(rpc: Arc<R>) -> Self { Self { rpc } }

    /// Bind the socket and accept connections forever.
    ///
    /// # Errors
    /// Fails if the socket path is already in use or unwritable.
    pub async fn serve(&self, socket_path: &Path) -> std::io::Result<()> {
        // Remove stale socket file if present.
        let _ = tokio::fs::remove_file(socket_path).await;
        let listener = UnixListener::bind(socket_path)?;
        info!("IPC server listening at {}", socket_path.display());
        loop {
            let (stream, _addr) = listener.accept().await?;
            let rpc = self.rpc.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, rpc).await {
                    warn!("IPC connection ended with error: {e}");
                }
            });
        }
    }
}

async fn handle_connection<R: DaemonRpc>(
    stream: UnixStream, rpc: Arc<R>,
) -> std::io::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // Send hello notification on connect.
    let hello = Notification::new("hello", json!({
        "daemon_version": env!("CARGO_PKG_VERSION"),
        "ipc_version": format!("{}.{}", crate::IPC_VERSION_MAJOR, crate::IPC_VERSION_MINOR),
    }));
    let mut hello_line = serde_json::to_string(&hello).unwrap();
    hello_line.push('\n');
    write_half.write_all(hello_line.as_bytes()).await?;

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 { return Ok(()); } // EOF

        let response = dispatch(&rpc, &line).await;
        let mut out = serde_json::to_string(&response).unwrap_or_else(|_|
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"serialize-failed"}}"#
                .to_string());
        out.push('\n');
        if let Err(e) = write_half.write_all(out.as_bytes()).await {
            error!("write failed: {e}");
            return Err(e);
        }
    }
}

async fn dispatch<R: DaemonRpc>(rpc: &Arc<R>, line: &str) -> Response {
    let req: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return error_response(None, -32700, format!("parse error: {e}")),
    };
    let id = Some(req.id.clone());
    let method = req.method.as_str();
    debug!("ipc rx method={method} id={:?}", req.id);

    // Dispatch (one match arm per method; full coverage in P1D-13).
    let result: crate::Result<Value> = match method {
        "list_devices" => rpc.list_devices().await
            .and_then(|v| serde_json::to_value(v).map_err(|e| IpcError::Serialization(e.to_string()))),
        // ... 16 more arms; the implementer expands per the trait
        _ => Err(IpcError::NotImplemented(method.to_string())),
    };

    match result {
        Ok(v) => Response { jsonrpc: "2.0".into(), id,
            payload: ResponsePayload::Result(v) },
        Err(e) => error_response(id, e.jsonrpc_code(), e.to_string()),
    }
}

fn error_response(id: Option<Value>, code: i32, message: String) -> Response {
    Response {
        jsonrpc: "2.0".into(), id,
        payload: ResponsePayload::Error(ErrorObj { code, message, data: None }),
    }
}
```

(The implementer expands the `match method` block to cover all 17 `DaemonRpc` methods. The shape is mechanical — each arm deserialises params, calls the method, serialises the result.)

- [ ] **Step 2: Build + commit**

```bash
cargo build -p lconnect-ipc
cargo clippy -p lconnect-ipc --all-targets -- -D warnings
cargo fmt --all -- --check

git add crates/lconnect-ipc/src/server.rs
git commit -m "feat(ipc): JSON-RPC server over Unix socket

Long-lived connection model. hello notification on connect.
Line-delimited (one JSON object per newline). Dispatch table
maps method names to DaemonRpc trait calls. subscribe_snapshots
streaming wired in P1D-13 alongside the daemon-side impl."
```

---

### Task P1C-5: JSON-RPC client

**Files:**
- Modify: `crates/lconnect-ipc/src/client.rs`
- Create: `crates/lconnect-ipc/tests/client_server_roundtrip.rs`

- [ ] **Step 1: Implement the client**

```rust
//! JSON-RPC 2.0 client over UnixStream.
//!
//! `IpcClient::connect(path)` opens the socket, reads the `hello`
//! notification, and exposes a `call(method, params)` async method.
//! For Phase 1 the client also exposes `subscribe(method, params)`
//! returning a stream of incoming notifications matching that method.

use crate::jsonrpc::{Notification, Request, Response, ResponsePayload};
use crate::{IpcError, Result};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, Mutex};

pub struct IpcClient {
    next_id: AtomicU64,
    tx: mpsc::Sender<ClientMsg>,
    pub hello: Notification,
}

enum ClientMsg {
    Call { id: u64, line: String, reply: oneshot::Sender<Response> },
    Subscribe { method: String, sink: mpsc::Sender<Notification> },
}

impl IpcClient {
    /// # Errors
    /// Fails if the socket can't be opened or hello can't be parsed.
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).await
            .map_err(|e| IpcError::Transport(e.to_string()))?;
        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        // Read hello.
        let mut hello_line = String::new();
        reader.read_line(&mut hello_line).await
            .map_err(|e| IpcError::Transport(e.to_string()))?;
        let hello: Notification = serde_json::from_str(&hello_line)
            .map_err(|e| IpcError::Serialization(e.to_string()))?;

        // Spawn the multiplexer task.
        let (tx, mut rx) = mpsc::channel::<ClientMsg>(64);
        let pending: Arc<Mutex<std::collections::HashMap<u64, oneshot::Sender<Response>>>>
            = Arc::new(Mutex::new(Default::default()));
        let subs: Arc<Mutex<std::collections::HashMap<String, Vec<mpsc::Sender<Notification>>>>>
            = Arc::new(Mutex::new(Default::default()));

        let pending_w = pending.clone();
        let subs_w = subs.clone();
        tokio::spawn(async move {
            // Reader loop: distribute responses to oneshots; notifications to subs.
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).await.is_err() { break; }
                if line.is_empty() { break; }
                if let Ok(resp) = serde_json::from_str::<Response>(&line) {
                    if let Some(id_val) = &resp.id {
                        if let Some(id) = id_val.as_u64() {
                            if let Some(tx) = pending_w.lock().await.remove(&id) {
                                let _ = tx.send(resp);
                            }
                        }
                    }
                } else if let Ok(notif) = serde_json::from_str::<Notification>(&line) {
                    if let Some(sinks) = subs_w.lock().await.get(&notif.method).cloned() {
                        for s in sinks { let _ = s.send(notif.clone()).await; }
                    }
                }
            }
        });

        // Writer / dispatch task.
        let pending_w2 = pending.clone();
        let subs_w2 = subs.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ClientMsg::Call { id, line, reply } => {
                        pending_w2.lock().await.insert(id, reply);
                        if let Err(e) = write_half.write_all(line.as_bytes()).await {
                            tracing::error!("write failed: {e}");
                            break;
                        }
                    }
                    ClientMsg::Subscribe { method, sink } => {
                        subs_w2.lock().await.entry(method).or_default().push(sink);
                    }
                }
            }
        });

        Ok(IpcClient { next_id: AtomicU64::new(1), tx, hello })
    }

    /// One-shot RPC call.
    ///
    /// # Errors
    /// Returns the daemon's IpcError on failure, or `Transport`/`Serialization`
    /// for connection-level problems.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request::new(id, method, params);
        let mut line = serde_json::to_string(&req)
            .map_err(|e| IpcError::Serialization(e.to_string()))?;
        line.push('\n');

        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(ClientMsg::Call { id, line, reply: reply_tx })
            .await.map_err(|_| IpcError::Transport("client multiplexer closed".into()))?;

        let resp = reply_rx.await
            .map_err(|_| IpcError::Transport("response dropped".into()))?;
        match resp.payload {
            ResponsePayload::Result(v) => Ok(v),
            ResponsePayload::Error(e) => Err(map_jsonrpc_error(e.code, e.message)),
        }
    }

    /// Subscribe to a notification method (e.g., "snapshot").
    ///
    /// The returned receiver yields notifications as long as the connection
    /// is alive. Multiple subscriptions to the same method are independent.
    pub async fn subscribe(&self, method: &str) -> mpsc::Receiver<Notification> {
        let (tx, rx) = mpsc::channel(32);
        let _ = self.tx.send(ClientMsg::Subscribe {
            method: method.to_string(), sink: tx,
        }).await;
        rx
    }
}

fn map_jsonrpc_error(code: i32, message: String) -> IpcError {
    match code {
        -32000 => IpcError::DeviceNotFound(message),
        -32001 => IpcError::DeviceBusy(message),
        -32002 => IpcError::InvalidArgument(message),
        -32003 => IpcError::NotImplemented(message),
        -32004 => IpcError::ConfirmationRequired,
        -32005 => IpcError::ProtocolMismatch(message),
        _ => IpcError::Transport(format!("code={code}: {message}")),
    }
}
```

- [ ] **Step 2: Round-trip test (in-memory transport will be added in P1C-6; this test is hardware-free)**

```rust
// crates/lconnect-ipc/tests/client_server_roundtrip.rs
// Skeleton test; the actual round-trip exercises both server.rs and
// client.rs over a real UnixStream pair. The implementer creates a temp
// directory, binds a daemon-mock that returns canned responses, and
// asserts call() returns the expected Value.
//
// (This is genuinely an integration test; if temp-socket setup is
// awkward, fall back to MemTransport tests in P1C-6.)
```

- [ ] **Step 3: Build + commit**

```bash
cargo build -p lconnect-ipc
cargo clippy -p lconnect-ipc --all-targets -- -D warnings
cargo fmt --all -- --check

git add crates/lconnect-ipc/src/client.rs \
        crates/lconnect-ipc/tests/client_server_roundtrip.rs
git commit -m "feat(ipc): JSON-RPC client over Unix socket

Multiplexed call/subscribe model on a single connection. Reader
task distributes responses (by id) to pending oneshots and
notifications (by method) to subscriber mpsc receivers. Writer
task serialises outbound requests + subscriptions. Hello received
on connect."
```

---

### Task P1C-6: In-memory transport for tests

**Files:**
- Modify: `crates/lconnect-ipc/src/transport_memory.rs`
- Create: `crates/lconnect-ipc/tests/memory_roundtrip.rs`

- [ ] **Step 1: Implement an in-process duplex stream**

```rust
//! In-memory client/server pair for hardware-free tests.
//!
//! `mem_pair()` returns `(client_end, server_end)`. Each end is
//! AsyncRead + AsyncWrite (via tokio::io::DuplexStream). The daemon's
//! actor-tree tests can stand up a full IpcServer + IpcClient over
//! these without touching the filesystem.

use tokio::io::DuplexStream;

/// Returns `(client_end, server_end)`. Buffer size chosen so a typical
/// RPC fits in one buffer cycle.
#[must_use]
pub fn mem_pair() -> (DuplexStream, DuplexStream) {
    tokio::io::duplex(64 * 1024)
}
```

(The server's `serve` method takes a `Path`; for tests we want a stream-based variant. The implementer adds `IpcServer::serve_stream(stream)` that runs `handle_connection` against a single duplex stream instead of a listener.)

- [ ] **Step 2: Round-trip test using a mock DaemonRpc**

```rust
// crates/lconnect-ipc/tests/memory_roundtrip.rs
use async_trait::async_trait;
use futures::stream::BoxStream;
use lconnect_ipc::*;
use std::sync::Arc;

struct MockRpc;

#[async_trait]
impl DaemonRpc for MockRpc {
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>> {
        Ok(vec![DeviceDescriptor {
            id: DeviceId::new("mock:0001"),
            family: "mock".into(), display_name: "Mock".into(),
            vid: 0xDEAD, pid: 0xBEEF, serial: None,
            hid_path: "/mock".into(),
        }])
    }
    async fn get_firmware(&self, _: DeviceId) -> Result<FirmwareInfo> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    // ... stub the remaining methods returning NotImplemented
    async fn get_snapshot(&self, _: DeviceId) -> Result<SensorSnapshot> { Err(IpcError::NotImplemented("".into())) }
    async fn subscribe_snapshots(&self, _: DeviceId) -> Result<BoxStream<'static, SensorSnapshot>> { Err(IpcError::NotImplemented("".into())) }
    async fn set_fan_pwm(&self, _: DeviceId, _: Pwm) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn set_pump_pwm(&self, _: DeviceId, _: Pwm, _: PwmSource) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn get_lighting_info(&self, _: DeviceId, _: LightingKind) -> Result<LightingInfo> { Err(IpcError::NotImplemented("".into())) }
    async fn set_lighting(&self, _: DeviceId, _: LightingKind, _: LightingConfig) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn set_led_colors(&self, _: DeviceId, _: LedTarget, _: Vec<Rgb>) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn is_lcd_available(&self, _: DeviceId) -> Result<bool> { Err(IpcError::NotImplemented("".into())) }
    async fn set_lcd_config(&self, _: DeviceId, _: LcdConfig) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn set_lcd_image(&self, _: DeviceId, _: Vec<u8>) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn clear_lcd(&self, _: DeviceId) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn request_reset_token(&self, _: DeviceId) -> Result<ResetToken> { Err(IpcError::NotImplemented("".into())) }
    async fn reset_device(&self, _: DeviceId, _: ResetToken) -> Result<()> { Err(IpcError::NotImplemented("".into())) }
    async fn raw_send(&self, _: DeviceId, _: FramingKind, _: Vec<u8>) -> Result<Vec<u8>> { Err(IpcError::NotImplemented("".into())) }
    async fn daemon_stats(&self) -> Result<DaemonStats> { Err(IpcError::NotImplemented("".into())) }
}

#[tokio::test]
async fn list_devices_roundtrip() {
    let (client_end, server_end) = lconnect_ipc::transport_memory::mem_pair();
    let server = lconnect_ipc::server::IpcServer::new(Arc::new(MockRpc));
    tokio::spawn(async move { server.serve_stream(server_end).await });

    let client = lconnect_ipc::client::IpcClient::from_stream(client_end).await.unwrap();
    let v = client.call("list_devices", serde_json::json!({})).await.unwrap();
    let descs: Vec<DeviceDescriptor> = serde_json::from_value(v).unwrap();
    assert_eq!(descs.len(), 1);
    assert_eq!(descs[0].id.as_str(), "mock:0001");
}
```

(`serve_stream` and `IpcClient::from_stream` are the test-only seam; the implementer adds them as small extensions of the path-based variants in P1C-4 / P1C-5.)

- [ ] **Step 2: Build + run + commit**

```bash
cargo test -p lconnect-ipc

git add crates/lconnect-ipc/src/transport_memory.rs \
        crates/lconnect-ipc/tests/memory_roundtrip.rs
git commit -m "feat(ipc): in-memory transport + roundtrip test

mem_pair() yields a tokio DuplexStream pair so IpcServer +
IpcClient can be exercised hardware-free. Roundtrip test pins
the list_devices wire shape end-to-end."
```

---

## Sub-phase D — `lconnect-daemon` crate

This sub-phase has 15 tasks. Each task focuses on one actor, one crosscutting concern, or one binary subcommand. The shared types, IPC trait, and HID backend trait from Sub-phases B and C are inputs.

### Task P1D-1: Scaffold `lconnect-daemon` crate (binary)

**Files:**
- Create: `crates/lconnect-daemon/Cargo.toml`
- Create: `crates/lconnect-daemon/src/main.rs` (minimal "starts and exits" body)
- Modify: workspace `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "lconnect-daemon"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[[bin]]
name = "lconnectd"
path = "src/main.rs"

[dependencies]
lconnect-protocol = { path = "../protocol" }
lconnect-image    = { path = "../lconnect-image" }
lconnect-hid      = { path = "../lconnect-hid" }
lconnect-ipc      = { path = "../lconnect-ipc" }

tokio = { workspace = true, features = ["full"] }
tracing.workspace = true
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-journald = "0.3"
serde.workspace = true
serde_json.workspace = true
toml = "0.8"
thiserror.workspace = true
async-trait = "0.1"
futures = "0.3"
anyhow.workspace = true
bytes = "1"
xdg = "2"
rand = "0.8"
```

- [ ] **Step 2: Minimal `main.rs`**

```rust
//! lconnectd — the L-Connect Linux daemon.
//!
//! Body lands across P1D-2..P1D-15. This scaffold establishes the
//! tokio runtime, sets up tracing, and exits.

#![forbid(unsafe_code)]

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    tracing::info!("lconnectd starting (scaffold; no work yet)");
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lconnectd=info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
```

- [ ] **Step 3: Workspace + commit**

Add `crates/lconnect-daemon` to workspace `members`. Build:

```bash
cargo build -p lconnect-daemon
cargo run  -p lconnect-daemon
# outputs: "lconnectd starting (scaffold; no work yet)" then exits

git add Cargo.toml crates/lconnect-daemon/
git commit -m "feat(daemon): scaffold lconnect-daemon crate"
```

---

### Task P1D-2: Config loading (`daemon.toml`)

**Files:**
- Create: `crates/lconnect-daemon/src/config.rs`
- Create: `crates/lconnect-daemon/tests/config.rs`

- [ ] **Step 1: Tests**

```rust
// tests/config.rs
use lconnect_daemon::config::{Config, LcdSection, LightingSection};

#[test]
fn defaults_match_spec() {
    let c = Config::default();
    assert_eq!(c.daemon.log_level, "info");
    assert!(!c.daemon.allow_raw_send);
    assert_eq!(c.poll.hotplug_ms, 500);
    assert_eq!(c.poll.sensor_hz, 1);
    assert_eq!(c.lcd.refresh_ms, 15_000);
    assert_eq!(c.lighting.reissue_ms, 1_500);
}

#[test]
fn parses_partial_toml() {
    let toml_text = r#"
[daemon]
log_level = "debug"

[lcd]
refresh_ms = 12000
"#;
    let c: Config = toml::from_str(toml_text).unwrap();
    assert_eq!(c.daemon.log_level, "debug");
    assert_eq!(c.lcd.refresh_ms, 12000);
    // Other fields fall back to default
    assert_eq!(c.poll.sensor_hz, 1);
}
```

- [ ] **Step 2: Implement**

```rust
//! daemon.toml loading + defaults.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub daemon: DaemonSection,
    pub poll: PollSection,
    pub lcd: LcdSection,
    pub lighting: LightingSection,
}

impl Default for Config { fn default() -> Self {
    Self {
        daemon: DaemonSection::default(),
        poll: PollSection::default(),
        lcd: LcdSection::default(),
        lighting: LightingSection::default(),
    }
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonSection {
    pub log_level: String,
    pub allow_raw_send: bool,
}
impl Default for DaemonSection { fn default() -> Self {
    Self { log_level: "info".into(), allow_raw_send: false }
}}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct PollSection { pub hotplug_ms: u64, pub sensor_hz: u32 }
impl Default for PollSection { fn default() -> Self {
    Self { hotplug_ms: 500, sensor_hz: 1 }
}}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct LcdSection { pub refresh_ms: u64 }
impl Default for LcdSection { fn default() -> Self { Self { refresh_ms: 15_000 } }}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSection { pub reissue_ms: u64 }
impl Default for LightingSection { fn default() -> Self { Self { reissue_ms: 1_500 } }}

impl Config {
    /// Load from `$XDG_CONFIG_HOME/lconnectd/daemon.toml`, falling back to
    /// defaults when the file is missing.
    ///
    /// # Errors
    /// Returns Err on parse failures (existing-but-invalid file).
    pub fn load() -> anyhow::Result<Self> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("lconnectd")?;
        let path = xdg_dirs.find_config_file("daemon.toml");
        match path {
            Some(p) => {
                let text = std::fs::read_to_string(&p)?;
                Ok(toml::from_str(&text)?)
            }
            None => Ok(Self::default()),
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
cargo test -p lconnect-daemon

git add crates/lconnect-daemon/src/config.rs \
        crates/lconnect-daemon/tests/config.rs
git commit -m "feat(daemon): config loading from daemon.toml + defaults"
```

---

### Task P1D-3: Persistence layer (per-device TOML, atomic writes, debounce)

**Files:**
- Create: `crates/lconnect-daemon/src/persistence.rs`
- Create: `crates/lconnect-daemon/tests/persistence.rs`

- [ ] **Step 1: Tests**

```rust
// tests/persistence.rs
use lconnect_daemon::persistence::{DeviceSettings, PersistenceStore};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn schema_version_default_is_1() {
    let s = DeviceSettings::default();
    assert_eq!(s.schema_version, 1);
}

#[test]
fn atomic_write_then_read_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().to_path_buf());
    let key = "galahad2-lcdv2_0416-7395_default";
    let mut s = DeviceSettings::default();
    s.fan.pwm_percent = 75;
    store.write(key, &s).unwrap();
    let r = store.read(key).unwrap();
    assert_eq!(r.fan.pwm_percent, 75);
}

#[test]
fn missing_file_returns_default() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().to_path_buf());
    let r = store.read("never-written").unwrap();
    assert_eq!(r.schema_version, 1);
}

#[test]
fn unknown_schema_version_warns_falls_back_to_defaults() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().to_path_buf());
    // hand-write an invalid version
    let path = tmp.path().join("devices/k.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "schema_version = 999\n").unwrap();
    let r = store.read("k").unwrap();
    assert_eq!(r.schema_version, 1); // fell back
}
```

- [ ] **Step 2: Implement**

```rust
//! Per-device persistence (Hybrid C from spec section 6).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CURRENT_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub fan: FanSection,
    #[serde(default)]
    pub pump: PumpSection,
    #[serde(default)]
    pub lighting: LightingSection,
    #[serde(default)]
    pub lcd: LcdSection,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self { schema_version: CURRENT_SCHEMA,
               fan: FanSection::default(), pump: PumpSection::default(),
               lighting: LightingSection::default(), lcd: LcdSection::default() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FanSection { pub pwm_percent: u8 }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PumpSection { pub pwm_percent: u8, pub source: String /* "mcu" | "application" */ }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSection {
    pub fan: Option<LightingState>,
    pub pump: Option<PumpLightingState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingState {
    pub mode: String, pub brightness: u8, pub speed: u8,
    pub direction: String, pub color: String, pub disabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PumpLightingState {
    pub scope: String, pub mode: String, pub brightness: u8, pub speed: u8,
    pub direction: String, pub color: String, pub disabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LcdSection { pub brightness: u8, pub rotation: String, pub control_mode: String }

pub struct PersistenceStore {
    root: PathBuf, // typically $XDG_CONFIG_HOME/lconnectd
}

impl PersistenceStore {
    /// Production: roots at `$XDG_CONFIG_HOME/lconnectd`.
    pub fn new() -> anyhow::Result<Self> {
        let dirs = xdg::BaseDirectories::with_prefix("lconnectd")?;
        Ok(Self { root: dirs.get_config_home() })
    }

    pub fn new_with_root(root: PathBuf) -> Self { Self { root } }

    fn key_path(&self, key: &str) -> PathBuf {
        self.root.join("devices").join(format!("{key}.toml"))
    }

    /// Read settings. Missing file -> defaults. Unknown schema_version -> warn + defaults.
    ///
    /// # Errors
    /// I/O or parse errors.
    pub fn read(&self, key: &str) -> anyhow::Result<DeviceSettings> {
        let path = self.key_path(key);
        if !path.exists() { return Ok(DeviceSettings::default()); }
        let text = std::fs::read_to_string(&path)?;
        let parsed: DeviceSettings = toml::from_str(&text)?;
        if parsed.schema_version != CURRENT_SCHEMA {
            tracing::warn!(
                "device settings schema_version={} unknown (we ship {}); falling back to defaults",
                parsed.schema_version, CURRENT_SCHEMA
            );
            return Ok(DeviceSettings::default());
        }
        Ok(parsed)
    }

    /// Atomic write: write-temp + fsync + rename.
    pub fn write(&self, key: &str, settings: &DeviceSettings) -> anyhow::Result<()> {
        use std::io::Write;
        let path = self.key_path(key);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("toml.tmp");
        let text = toml::to_string_pretty(settings)?;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }
}
```

(Debounce wrapper lives at the daemon level — `command_dispatch` writes to a debounced `tokio::sync::watch<DeviceSettings>` and a per-device task batches disk writes at ~250 ms intervals. That detail lands in P1D-12 alongside command_dispatch.)

- [ ] **Step 3: Commit**

```bash
cargo test -p lconnect-daemon

git add crates/lconnect-daemon/src/persistence.rs \
        crates/lconnect-daemon/tests/persistence.rs
git commit -m "feat(daemon): persistence with atomic writes + schema_version=1

Hybrid C from spec section 6: small settings persist (fan/pump/
lighting/LCD config), LCD JPEG bytes do not. Atomic write-temp +
fsync + rename. Unknown schema_version warns + falls back to
defaults. Debounce / replay-on-spawn wired in P1D-12 / P1D-15."
```

---

### Task P1D-4: `Actor` trait + per-actor mod skeleton

**Files:**
- Create: `crates/lconnect-daemon/src/actor/mod.rs`
- Create empty stubs: `crates/lconnect-daemon/src/actor/{hid_io,sensor_poll,lighting_keeper,lcd_keeper,command_dispatch}.rs`

- [ ] **Step 1: `actor/mod.rs`** — module surface and shared types:

```rust
//! Per-device actor tree.
//!
//! One `DeviceActorSet` per connected device, spawned by `manager.rs`.
//! Each actor lives on its own tokio task; channels stitch them together.

pub mod command_dispatch;
pub mod hid_io;
pub mod lcd_keeper;
pub mod lighting_keeper;
pub mod sensor_poll;

use crate::persistence::DeviceSettings;
use lconnect_hid::{DeviceInfo, HidBackend};
use lconnect_ipc::SensorSnapshot;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

/// Owned per-device shutdown signal. Drop the sender to terminate all actors.
pub type Shutdown = watch::Receiver<bool>;

/// Commands sent through `hid_io_tx` (single-consumer).
#[derive(Debug)]
pub enum HidCommand {
    /// Write bytes to the device, no response read.
    Write { bytes: Vec<u8>, reply: oneshot::Sender<lconnect_hid::Result<()>> },
    /// Write then read one response (A-class).
    WriteRead64 { bytes: Vec<u8>, expected_cmd: u8,
                  reply: oneshot::Sender<lconnect_hid::Result<[u8; 64]>> },
    /// Write then read one response (B-class).
    WriteRead512 { bytes: Vec<u8>, expected_cmd: u8,
                   reply: oneshot::Sender<lconnect_hid::Result<[u8; 512]>> },
}

/// Per-device handles exposed to the IPC layer.
pub struct DeviceHandles {
    pub hid_io_tx: mpsc::Sender<HidCommand>,
    pub snapshot_rx: watch::Receiver<SensorSnapshot>,
    pub settings_tx: mpsc::Sender<DeviceSettings>,
    pub lcd_frame_tx: mpsc::Sender<LcdFrameMsg>,
    pub lighting_state_tx: mpsc::Sender<LightingStateMsg>,
}

/// Frame-source message for `lcd_keeper`.
#[derive(Debug)]
pub enum LcdFrameMsg {
    Set(bytes::Bytes),
    Clear,
}

/// Lighting-keeper state replacement.
#[derive(Debug)]
pub enum LightingStateMsg {
    SetFan(crate::persistence::LightingState),
    SetPump(crate::persistence::PumpLightingState),
    Clear(lconnect_ipc::LightingKind),
}

/// Spawn the full per-device actor tree.
pub fn spawn_device_actor_tree(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    initial_settings: DeviceSettings,
    shutdown: Shutdown,
) -> anyhow::Result<DeviceHandles> {
    // Implementation lands incrementally in P1D-5..P1D-12.
    unimplemented!("see P1D-5..P1D-12")
}
```

- [ ] **Step 2: Stub each actor file**

```bash
for f in hid_io sensor_poll lighting_keeper lcd_keeper command_dispatch; do
  printf '//! Stub - body lands in subsequent P1D-* task.\n\n' \
    > "crates/lconnect-daemon/src/actor/${f}.rs"
done
```

- [ ] **Step 3: Wire `pub mod actor;` into `main.rs` (or add a `lib.rs` if needed for tests). Build + commit.**

```bash
cargo build -p lconnect-daemon

git add crates/lconnect-daemon/src/actor/
git commit -m "feat(daemon): actor module skeleton + DeviceHandles types"
```

---

### Task P1D-5: `hid_io` actor

**Files:**
- Modify: `crates/lconnect-daemon/src/actor/hid_io.rs`
- Create: `crates/lconnect-daemon/tests/hid_io.rs`

- [ ] **Step 1: Implement**

```rust
//! hid_io actor — owns the HID handle, serialises all writes/reads.
//!
//! Per-device single-consumer mpsc. Every HID I/O runs on
//! `tokio::task::spawn_blocking`; hidapi-rs is synchronous and we
//! must not block the runtime.

use super::{HidCommand, Shutdown};
use lconnect_hid::{HidBackend, OpenHandle, DeviceInfo};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

pub fn spawn(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    mut shutdown: Shutdown,
) -> anyhow::Result<mpsc::Sender<HidCommand>> {
    let handle: Box<dyn OpenHandle> = backend.open(&info)?;
    let handle: Arc<dyn OpenHandle> = handle.into();
    let (tx, mut rx) = mpsc::channel::<HidCommand>(64);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown.changed() => { debug!("hid_io shutting down"); break; }
                Some(cmd) = rx.recv() => process_one(handle.clone(), cmd).await,
                else => break,
            }
        }
    });

    Ok(tx)
}

async fn process_one(handle: Arc<dyn OpenHandle>, cmd: HidCommand) {
    match cmd {
        HidCommand::Write { bytes, reply } => {
            let h = handle.clone();
            let r = tokio::task::spawn_blocking(move || {
                h.write(&bytes).map(|_| ())
            }).await.unwrap();
            let _ = reply.send(r);
        }
        HidCommand::WriteRead64 { bytes, expected_cmd, reply } => {
            let h = handle.clone();
            let r = tokio::task::spawn_blocking(move || {
                h.write(&bytes)?;
                let mut buf = [0u8; 64];
                drain_for_echo_64(&*h, &mut buf, expected_cmd)?;
                Ok(buf)
            }).await.unwrap();
            let _ = reply.send(r);
        }
        HidCommand::WriteRead512 { bytes, expected_cmd, reply } => {
            let h = handle.clone();
            let r = tokio::task::spawn_blocking(move || {
                h.write(&bytes)?;
                let mut buf = [0u8; 512];
                drain_for_echo_512(&*h, &mut buf, expected_cmd)?;
                Ok(buf)
            }).await.unwrap();
            let _ = reply.send(r);
        }
    }
}

fn drain_for_echo_64(
    h: &dyn OpenHandle, buf: &mut [u8; 64], expected_cmd: u8,
) -> lconnect_hid::Result<()> {
    drain_for_echo(h, &mut buf[..], 0x01, expected_cmd, 64)
}

fn drain_for_echo_512(
    h: &dyn OpenHandle, buf: &mut [u8; 512], expected_cmd: u8,
) -> lconnect_hid::Result<()> {
    drain_for_echo(h, &mut buf[..], 0x02, expected_cmd, 512)
}

/// Drain up to N stale responses looking for `[report_id, expected_cmd, ...]`.
/// Mirrors verify-galahad's run_probe convention (Phase 0e).
fn drain_for_echo(
    h: &dyn OpenHandle, buf: &mut [u8], expected_report_id: u8,
    expected_cmd: u8, _input_len: usize,
) -> lconnect_hid::Result<()> {
    let mut stale = 0u32;
    let mut timeout = 0u32;
    for _ in 0..4 {
        let n = h.read_timeout(buf, 500)?;
        if n == 0 { timeout += 1; continue; }
        if n >= 2 && buf[0] == expected_report_id && buf[1] == expected_cmd {
            return Ok(());
        }
        stale += 1;
    }
    Err(lconnect_hid::Error::EchoExhausted { stale, timeout, expected_cmd })
}
```

- [ ] **Step 2: Test with MockBackend**

```rust
// tests/hid_io.rs
use lconnect_hid::{mock_backend::MockBackend, DeviceFamilyId, DeviceInfo, HidBackend};
use lconnect_daemon::actor::{HidCommand, hid_io};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{oneshot, watch};

fn lcdv2_info() -> DeviceInfo {
    DeviceInfo {
        family: DeviceFamilyId::new("galahad2-lcdv2"),
        vid: 0x0416, pid: 0x7395, serial: None,
        path: PathBuf::from("/mock/hidraw0"),
    }
}

#[tokio::test]
async fn write_read_64_returns_canned_response() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    // Queue an A-class echo response: [0x01, 0x86, ...]
    let mut canned = vec![0u8; 64];
    canned[0] = 0x01; canned[1] = 0x86;
    mock.queue_response(&lcdv2_info(), canned);

    let backend: Arc<dyn HidBackend> = Arc::new(mock);
    let (_sd_tx, sd_rx) = watch::channel(false);
    let tx = hid_io::spawn(backend, lcdv2_info(), sd_rx).unwrap();

    let (reply_tx, reply_rx) = oneshot::channel();
    let mut bytes = vec![0u8; 64];
    bytes[0] = 0x01; bytes[1] = 0x86;
    tx.send(HidCommand::WriteRead64 { bytes, expected_cmd: 0x86, reply: reply_tx }).await.unwrap();
    let resp = reply_rx.await.unwrap().unwrap();
    assert_eq!(resp[0], 0x01); assert_eq!(resp[1], 0x86);
}
```

- [ ] **Step 3: Build + commit**

```bash
cargo test -p lconnect-daemon --test hid_io

git add crates/lconnect-daemon/src/actor/hid_io.rs \
        crates/lconnect-daemon/tests/hid_io.rs
git commit -m "feat(daemon): hid_io actor with single-consumer mpsc

Owns the HidDevice handle. Serialises all HID I/O. Every write/read
runs on spawn_blocking. Drain-for-echo helper mirrors verify-galahad
probe convention (Phase 0e finding: report_id at offset 0, plain
cmd echo at offset 1, no | 0x80 mask)."
```

---

### Task P1D-6: `sensor_poll` actor (1 Hz `GetHandshakeInfo`)

**Files:**
- Modify: `crates/lconnect-daemon/src/actor/sensor_poll.rs`

Spawns a 1 Hz timer; each tick sends `GetHandshakeInfo` (cmd 129, A-class) via `hid_io_tx`, parses the response with `lconnect_protocol::galahad2_lcdv2::handshake::parse_handshake_response`, publishes the parsed `SensorSnapshot` to a `tokio::sync::watch`. Subscribers (IPC `subscribe_snapshots`, future GUI) read from the watch.

- [ ] **Step 1: Test**

```rust
// tests/sensor_poll.rs (use MockBackend with queued canned handshake responses)
// asserts that after 1+ ticks the watch contains a non-default SensorSnapshot
```

- [ ] **Step 2: Implement** (~80 LOC)

Key shape:

```rust
pub fn spawn(
    hid_io_tx: mpsc::Sender<HidCommand>,
    poll_period_ms: u64,
    mut shutdown: Shutdown,
) -> watch::Receiver<SensorSnapshot> {
    let (tx, rx) = watch::channel(SensorSnapshot::default());
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(poll_period_ms));
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = interval.tick() => {
                    if let Ok(snapshot) = poll_once(&hid_io_tx).await {
                        let _ = tx.send(snapshot);
                    }
                }
            }
        }
    });
    rx
}

async fn poll_once(tx: &mpsc::Sender<HidCommand>) -> anyhow::Result<SensorSnapshot> {
    let req = lconnect_protocol::galahad2_lcdv2::handshake::build_handshake_request();
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(HidCommand::WriteRead64 { bytes: req.to_vec(), expected_cmd: 0x81, reply: reply_tx }).await?;
    let resp = reply_rx.await??;
    let parsed = lconnect_protocol::galahad2_lcdv2::handshake::parse_handshake_response(&resp)?;
    Ok(SensorSnapshot {
        coolant_c_x10: parsed.temp_c_tenths,
        fan_rpm: parsed.fan_rpm,
        pump_rpm: parsed.pump_rpm,
        temp_valid: parsed.temp_is_valid,
        last_seen_unix_ms: now_unix_ms(),
    })
}
```

(Adjust to match Phase 0d's actual `parse_handshake_response` signature.)

- [ ] **Step 3: Commit**

```bash
cargo test -p lconnect-daemon --test sensor_poll

git add crates/lconnect-daemon/src/actor/sensor_poll.rs \
        crates/lconnect-daemon/tests/sensor_poll.rs
git commit -m "feat(daemon): sensor_poll actor — 1 Hz handshake + watch fanout"
```

---

### Task P1D-7: `lighting_keeper` actor (continuous re-issue)

**Files:**
- Modify: `crates/lconnect-daemon/src/actor/lighting_keeper.rs`

Continuously re-issues the latest known lighting state every `reissue_ms` (default 1500). Mirrors C# service behavior; mitigates LCDV2 cmd 131 buffering quirk (Phase 0c finding).

- [ ] **Step 1: Implement** (~120 LOC)

```rust
//! lighting_keeper — re-issues current lighting state at fixed cadence.
//!
//! Receives state replacements via mpsc; emits the latest state at
//! `reissue_ms` cadence to `hid_io_tx`. Phase 0c finding (cmd 131
//! buffering quirk on FW 1.6) is mitigated by this re-issue pattern.

pub fn spawn(
    hid_io_tx: mpsc::Sender<HidCommand>,
    reissue_ms: u64,
    initial_fan: Option<persistence::LightingState>,
    initial_pump: Option<persistence::PumpLightingState>,
    mut state_rx: mpsc::Receiver<LightingStateMsg>,
    mut shutdown: Shutdown,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut fan = initial_fan;
        let mut pump = initial_pump;
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(reissue_ms));
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                Some(msg) = state_rx.recv() => match msg {
                    LightingStateMsg::SetFan(s) => fan = Some(s),
                    LightingStateMsg::SetPump(s) => pump = Some(s),
                    LightingStateMsg::Clear(LightingKind::Fan) => fan = None,
                    LightingStateMsg::Clear(LightingKind::Pump) => pump = None,
                },
                _ = interval.tick() => {
                    // Emit latest fan + pump state via hid_io_tx
                    if let Some(s) = &fan { reissue_fan(&hid_io_tx, s).await; }
                    if let Some(s) = &pump { reissue_pump(&hid_io_tx, s).await; }
                }
            }
        }
    })
}
```

`reissue_fan` and `reissue_pump` build the cmd 133 / cmd 131 packets via `lconnect_protocol::galahad2_lcdv2::set_lighting::build_*_request` and send `HidCommand::Write { ... }`. If the write fails, log + continue (don't propagate; the next tick retries).

- [ ] **Step 2: Test** (with MockBackend; assert that after N ticks, write_history has the expected cmd 131/133 byte sequences)

- [ ] **Step 3: Commit**

```bash
cargo test -p lconnect-daemon --test lighting_keeper

git add crates/lconnect-daemon/src/actor/lighting_keeper.rs \
        crates/lconnect-daemon/tests/lighting_keeper.rs
git commit -m "feat(daemon): lighting_keeper actor — 1.5s re-issue (mirrors C# service)"
```

---

### Task P1D-8: `lcd_keeper` actor (the frame-source seam for Phase 4)

**Files:**
- Modify: `crates/lconnect-daemon/src/actor/lcd_keeper.rs`

`lcd_keeper` holds an `Option<Bytes>` "current frame" slot. When `Some`, it sends cmd 14 (`SendJpeg`) every `refresh_ms` (default 15000). When `None`, idle. Phase 4's theme engine plugs into the same slot via a different msg path.

- [ ] **Step 1: Implement** (~100 LOC)

```rust
pub fn spawn(
    hid_io_tx: mpsc::Sender<HidCommand>,
    refresh_ms: u64,
    mut frame_rx: mpsc::Receiver<LcdFrameMsg>,
    mut shutdown: Shutdown,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut current: Option<bytes::Bytes> = None;
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(refresh_ms));
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                Some(msg) = frame_rx.recv() => {
                    match msg {
                        LcdFrameMsg::Set(b) => {
                            current = Some(b.clone());
                            // Push immediately; don't wait for next interval tick.
                            push_frame(&hid_io_tx, &b).await;
                        }
                        LcdFrameMsg::Clear => {
                            current = None;
                            // Phase 1 doesn't send a "clear" cmd; just stop refreshing.
                            // The firmware reverts to LocalUI ~20s after the last cmd 14.
                        }
                    }
                }
                _ = interval.tick() => {
                    if let Some(b) = current.clone() {
                        push_frame(&hid_io_tx, &b).await;
                    }
                }
            }
        }
    })
}

async fn push_frame(hid_io_tx: &mpsc::Sender<HidCommand>, jpeg: &[u8]) {
    let packets = match lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request(jpeg) {
        Ok(v) => v,
        Err(e) => { tracing::warn!("build_send_jpeg failed: {e}"); return; }
    };
    for pkt in packets {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(e) = hid_io_tx.send(HidCommand::Write {
            bytes: pkt.to_vec(), reply: reply_tx,
        }).await {
            tracing::warn!("lcd_keeper send failed: {e}"); return;
        }
        if let Err(e) = reply_rx.await { tracing::warn!("hid_io reply dropped: {e}"); return; }
    }
}
```

- [ ] **Step 2: Test** (MockBackend; LcdFrameMsg::Set followed by N tick advances → write_history has 2× the SendJpeg packet sequences)

- [ ] **Step 3: Commit**

```bash
cargo test -p lconnect-daemon --test lcd_keeper

git add crates/lconnect-daemon/src/actor/lcd_keeper.rs \
        crates/lconnect-daemon/tests/lcd_keeper.rs
git commit -m "feat(daemon): lcd_keeper — frame slot + 15s refresh

Phase 4's theme engine hook: write generated JPEGs to the same
frame_rx mpsc at any cadence; the keeper continues to drive the
HID layer at the configured refresh interval. Phase 1 only
exposes set_lcd_image / clear_lcd via IPC."
```

---

### Task P1D-9: `command_dispatch` actor (one-off requests + persistence debounce)

**Files:**
- Modify: `crates/lconnect-daemon/src/actor/command_dispatch.rs`

Owns the per-device persistence write-debounce. Receives `DeviceSettings` via mpsc; writes to disk after a 250 ms idle period.

- [ ] **Step 1: Implement** (~150 LOC)

The actor accumulates settings updates and writes them out atomically after the debounce window expires. On shutdown, flushes the latest state.

- [ ] **Step 2: Test + commit**

```bash
git add crates/lconnect-daemon/src/actor/command_dispatch.rs \
        crates/lconnect-daemon/tests/command_dispatch.rs
git commit -m "feat(daemon): command_dispatch actor with 250ms persistence debounce"
```

---

### Task P1D-10: `device_manager` (hot-plug detection + actor-tree spawn)

**Files:**
- Create: `crates/lconnect-daemon/src/manager.rs`

500 ms polling of `HidBackend::enumerate`. New devices → spawn actor tree, replay persisted settings. Disappeared devices → drop actor tree (Sender drop terminates child actors via shutdown channel). Each device gets a stable `DeviceId` (`{family}:{0001}` per attach order).

- [ ] **Step 1: Implement** (~200 LOC)

- [ ] **Step 2: Tests** (MockBackend; add then remove a device, assert actor tree spawn/teardown)

- [ ] **Step 3: Commit**

```bash
git add crates/lconnect-daemon/src/manager.rs \
        crates/lconnect-daemon/tests/manager.rs
git commit -m "feat(daemon): device_manager with 500ms hot-plug poll

Spawns per-device actor tree on attach; tears down on detach.
DeviceId allocation is sequential per family attach order."
```

---

### Task P1D-11: `IpcServerImpl` (DaemonRpc impl backed by actor handles)

**Files:**
- Create: `crates/lconnect-daemon/src/ipc_server.rs`

Concrete `DaemonRpc` impl that:
- holds `Arc<Mutex<HashMap<DeviceId, DeviceHandles>>>`
- maps each method to the right actor's mpsc
- builds the right protocol packets via `lconnect_protocol::galahad2_lcdv2::*`
- maintains a small reset-token registry (HashMap<DeviceId, (token, expires_at)>) for `request_reset_token` / `reset_device`

- [ ] **Step 1: Implement** (~400 LOC; one method per `DaemonRpc` trait method)

- [ ] **Step 2: Round-trip test** via `transport_memory::mem_pair` plus a stub manager

- [ ] **Step 3: Commit**

```bash
git add crates/lconnect-daemon/src/ipc_server.rs \
        crates/lconnect-daemon/tests/ipc_server.rs
git commit -m "feat(daemon): IpcServerImpl wires DaemonRpc to actor handles

All 17 methods implemented. Reset-token registry is one-shot
30s TTL. raw_send gated by config.daemon.allow_raw_send. Cmd 15
(UploadJpeg) IPC-sealed per Phase 0e finding (set_lcd_image only
invokes cmd 14)."
```

---

### Task P1D-12: `daemon_stats` counters

**Files:**
- Create: `crates/lconnect-daemon/src/stats.rs`

`AtomicU64` counters for `hid_write_count`, `hid_read_errors`, `hid_decode_errors`, `ipc_request_count`, plus `Instant` for `started_at`. `IpcServerImpl::daemon_stats` reads them.

- [ ] **Step 1-3: implement, test, commit**

```bash
git commit -m "feat(daemon): stats module with atomic counters"
```

---

### Task P1D-13: `subscribe_snapshots` streaming wired through `IpcServer`

**Files:**
- Modify: `crates/lconnect-daemon/src/ipc_server.rs`
- Modify: `crates/lconnect-ipc/src/server.rs` (add notification-emit support if not there yet)

The trait method `subscribe_snapshots` returns `BoxStream<'static, SensorSnapshot>`. The server wraps these as `Notification { method: "snapshot", params }` writes on the connection. The CLI / client subscribes to `"snapshot"` notifications.

- [ ] **Step 1-3: implement, integration test, commit**

```bash
git commit -m "feat(daemon): server-pushed snapshot notifications wired"
```

---

### Task P1D-14: `main.rs` wiring — bring everything online

**Files:**
- Modify: `crates/lconnect-daemon/src/main.rs`

```rust
#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let config = Config::load()?;
    let registry = FamilyRegistry::with_default_lcdv2();
    let backend: Arc<dyn HidBackend> = Arc::new(HidApiBackend::new(registry)?);
    let persistence = PersistenceStore::new()?;
    let stats = Arc::new(Stats::new());

    let manager = DeviceManager::new(backend.clone(), persistence, config.clone());
    let manager_handle = manager.spawn().await;

    let rpc = Arc::new(IpcServerImpl::new(manager_handle.clone(), stats.clone(), config.clone()));
    let socket_path = xdg::BaseDirectories::with_prefix("lconnectd")?
        .get_runtime_directory()?.join("lconnectd.sock");
    let server = IpcServer::new(rpc);

    // SIGTERM / SIGINT trigger shutdown.
    let shutdown = listen_for_signals();
    tokio::select! {
        r = server.serve(&socket_path) => r?,
        _ = shutdown => tracing::info!("shutdown signal received"),
    }
    Ok(())
}
```

- [ ] **Step 1-3: implement, integration smoke (no-hardware), commit**

```bash
git commit -m "feat(daemon): main.rs wires manager + IPC + stats"
```

---

### Task P1D-15: Persistence replay on startup / hot-plug reconnect

**Files:**
- Modify: `crates/lconnect-daemon/src/manager.rs`

When a device's actor tree spawns, sequence (with ~50ms inter-write delay) per spec section 6:
1. lighting (fan, then pump)
2. fan PWM
3. pump PWM
4. LCD config (brightness, rotation)
5. `lighting_keeper` begins continuous re-issue
6. `lcd_keeper` starts in empty-slot state (LCD image NOT replayed; persistence model C)

- [ ] **Step 1-3: implement, integration test using MockBackend, commit**

```bash
git commit -m "feat(daemon): persistence replay on actor-tree spawn

Lighting, PWM, LCD-config replayed in order with 50ms spacing.
LCD image deliberately not replayed (Hybrid C persistence)."
```

---

## Sub-phase E — `lconnect-cli` (`lconnectctl`)

7 tasks. Each subcommand group lives in its own module under `commands/`. The CLI is stateless except for the IPC connection.

### Task P1E-1: Scaffold `lconnect-cli` crate + connection helper

**Files:**
- Create: `crates/lconnect-cli/Cargo.toml`
- Create: `crates/lconnect-cli/src/main.rs`
- Create: `crates/lconnect-cli/src/connection.rs`
- Create: `crates/lconnect-cli/src/format.rs`
- Modify: workspace `Cargo.toml`

- [ ] **Step 1: Cargo.toml** — same shape as `lconnect-daemon`'s, bin name `lconnectctl`, deps `lconnect-ipc`, `tokio` features `["macros","rt-multi-thread"]`, `clap` derive, `serde_json`, `anyhow`, `xdg`.

- [ ] **Step 2: `connection.rs` — open IPC, read hello, return `IpcClient`**

```rust
pub async fn connect() -> anyhow::Result<lconnect_ipc::client::IpcClient> {
    let dirs = xdg::BaseDirectories::with_prefix("lconnectd")?;
    let socket = dirs.get_runtime_directory()?.join("lconnectd.sock");
    Ok(lconnect_ipc::client::IpcClient::connect(&socket).await?)
}
```

- [ ] **Step 3: `format.rs` — human-readable output helpers + `--json` toggle**

- [ ] **Step 4: `main.rs` — clap `Cli` struct with subcommand enum stubs**

```rust
#[derive(clap::Parser)]
#[command(version, about = "L-Connect Linux CLI")]
struct Cli {
    /// Output JSON instead of human-readable text (NDJSON for `watch`).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// List attached devices.
    Devices,
    Firmware { #[arg(long)] device: Option<String> },
    Snapshot { #[arg(long)] device: Option<String> },
    Watch { #[arg(long)] device: Option<String> },
    Fan  { percent: u8, #[arg(long)] device: Option<String> },
    Pump { percent: u8, #[arg(long)] source: Option<String>, #[arg(long)] device: Option<String> },
    Lighting(LightingArgs),
    LedColors(LedColorsArgs),
    Lcd(LcdArgs),
    Reset { #[arg(long)] yes_i_really_want_to_reset: bool, #[arg(long)] device: Option<String> },
    Logs { #[arg(long)] follow: bool, #[arg(long)] level: Option<String> },
    Raw { framing: String, hex: String, #[arg(long)] device: Option<String> },
    Daemon(DaemonArgs),
}
```

- [ ] **Step 5: Workspace + commit**

```bash
git add Cargo.toml crates/lconnect-cli/
git commit -m "feat(cli): scaffold lconnect-cli (lconnectctl) + connection helper"
```

---

### Task P1E-2: `devices` / `firmware` / `snapshot` subcommands

**Files:**
- Create: `crates/lconnect-cli/src/commands/{devices,snapshot}.rs` (or split as needed)

Implement `Cmd::Devices`, `Cmd::Firmware`, and `Cmd::Snapshot` dispatch arms. `--device` resolution: when omitted and exactly one device is attached, use it; when ambiguous, list devices and exit with a clear error.

- [ ] **Step 1-3: implement, test (against mem_pair stub), commit**

```bash
git commit -m "feat(cli): devices/firmware/snapshot subcommands"
```

---

### Task P1E-3: `watch` subcommand (TTY redraw + NDJSON)

**Files:**
- Create: `crates/lconnect-cli/src/watch.rs`

ANSI cursor-based redraw-in-place; subscribes to `snapshot` notifications via `IpcClient::subscribe`. With `--json`, emits one NDJSON line per snapshot and never clears the screen. `Ctrl+C` exits cleanly.

- [ ] **Step 1-3: implement, test, commit**

```bash
git commit -m "feat(cli): watch subcommand — TTY redraw + NDJSON"
```

---

### Task P1E-4: `fan` / `pump` PWM subcommands

**Files:**
- Create: `crates/lconnect-cli/src/commands/pwm.rs`

`fan <PERCENT>`, `pump <PERCENT> [--source mcu|application]`. Validate range 0..=100; produce a clear error on out-of-range.

- [ ] **Step 1-3: implement, test, commit**

```bash
git commit -m "feat(cli): fan/pump PWM subcommands"
```

---

### Task P1E-5: `lighting` + `led-colors` subcommands

**Files:**
- Create: `crates/lconnect-cli/src/commands/lighting.rs`

`lighting set <fan|pump> --mode ... [--color #RRGGBB] [--speed 0..=100] [--brightness 0..=100] [--direction forward|reverse] [--scope inner|outer|all] [--disable]`, `lighting clear <fan|pump>`, `lighting info <fan|pump>`.

`led-colors set <fan|screen> --color #RRGGBB [--count N]`, `led-colors set <fan|screen> --from-file colors.txt`.

- [ ] **Step 1-3: implement, test, commit**

```bash
git commit -m "feat(cli): lighting + led-colors subcommands"
```

---

### Task P1E-6: `lcd` subcommands

**Files:**
- Create: `crates/lconnect-cli/src/commands/lcd.rs`

`lcd available`, `lcd brightness <PERCENT>`, `lcd rotation <0|90|180|270>`, `lcd mode <LocalUI|Application|...>`, `lcd image <PATH> [--fit crop|letterbox|stretch] [--letterbox-bg #RRGGBB] [--quality 1..=100]`, `lcd clear`.

`lcd image` uses `lconnect_image::prepare_jpeg_from_path` to convert input then calls `set_lcd_image` IPC method with the JPEG bytes.

`lcd rotation` translates degrees (0/90/180/270) to the wire enum (Rotate0/Rotate90/Rotate180/Rotate270).

- [ ] **Step 1-3: implement, test, commit**

```bash
git commit -m "feat(cli): lcd subcommands (available/brightness/rotation/mode/image/clear)"
```

---

### Task P1E-7: maintenance subcommands (`reset`, `logs`, `raw`, `daemon`)

**Files:**
- Create: `crates/lconnect-cli/src/commands/maintenance.rs`

`reset --yes-i-really-want-to-reset`: calls `request_reset_token` then `reset_device`.

`logs [--follow] [--level debug]`: thin wrapper over `journalctl --user -u lconnectd --output=cat` (with `--follow` and `-p <level>` flags mapped through). Implementation is one `Command::new("journalctl")` call.

`raw <framing:a|b|c> <hex>`: refuses to run unless daemon advertises `allow_raw_send=true` via daemon config or fails over IPC with `NotImplemented`. Hex parsing reuses `lconnect_ipc::types` if a helper is needed.

`daemon status`: connects, prints "OK" + IPC version. `daemon version`: same shape.

- [ ] **Step 1-3: implement, test, commit**

```bash
git commit -m "feat(cli): maintenance subcommands (reset/logs/raw/daemon)"
```

---

## Sub-phase F — Packaging

### Task P1F-1: udev rule + `xtask gen-udev`

**Files:**
- Create: `xtask/Cargo.toml`
- Create: `xtask/src/main.rs`
- Create: `packaging/udev/70-lianli-hid.rules`
- Modify: workspace `Cargo.toml`

- [ ] **Step 1: `xtask` minimal binary**

```toml
[package]
name = "xtask"
version.workspace = true
edition.workspace = true
publish = false

[[bin]]
name = "xtask"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
clap = { workspace = true, features = ["derive"] }
```

```rust
// xtask/src/main.rs
use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(version, about = "lconnect-linux build helpers")]
enum Cmd {
    /// Generate packaging/udev/70-lianli-hid.rules from the device-family table.
    GenUdev,
}

fn main() -> Result<()> {
    match Cmd::parse() {
        Cmd::GenUdev => gen_udev(),
    }
}

fn gen_udev() -> Result<()> {
    // Phase 1: only LCDV2.
    let rule = "\
# AUTO-GENERATED by xtask gen-udev. Do not edit by hand.
# Galahad II LCDV2
SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"0416\", ATTRS{idProduct}==\"7395\", \\
  TAG+=\"uaccess\", MODE=\"0660\", GROUP=\"plugdev\"
";
    std::fs::write("packaging/udev/70-lianli-hid.rules", rule)?;
    println!("wrote packaging/udev/70-lianli-hid.rules");
    Ok(())
}
```

- [ ] **Step 2: Run + verify**

```bash
mkdir -p packaging/udev
cargo run -p xtask -- gen-udev
cat packaging/udev/70-lianli-hid.rules
```

Expected: file contains the LCDV2 entry with `TAG+="uaccess"`.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml xtask/ packaging/udev/
git commit -m "feat(packaging): udev rule + xtask gen-udev generator

Phase 1 scope: VID 0x0416 + PID 0x7395 only. Phase 3+ expands the
generator to take entries from the device-family registry."
```

---

### Task P1F-2: systemd unit

**Files:**
- Create: `packaging/systemd/lconnectd.service`

```ini
[Unit]
Description=L-Connect Linux daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/lconnectd
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=lconnectd=info

[Install]
WantedBy=default.target
```

- [ ] **Step 1: Write the unit file as above**

- [ ] **Step 2: Verify install steps in README** (link from the new Status bullet — that lands in P1H-2)

- [ ] **Step 3: Commit**

```bash
git add packaging/systemd/lconnectd.service
git commit -m "feat(packaging): systemd --user unit for lconnectd"
```

---

### Task P1F-3: AUR PKGBUILD skeleton

**Files:**
- Create: `packaging/arch/PKGBUILD`

Buildtime deps: `rust`, `pkg-config`, `systemd`. Runtime: `udev`. Build: `cargo build --release --workspace --bin lconnectd --bin lconnectctl --bin verify-galahad`. Install: bins to `/usr/bin/`, udev rule to `/usr/lib/udev/rules.d/70-lianli-hid.rules`, systemd unit to `/usr/lib/systemd/user/lconnectd.service`. Post-install: `udevadm control --reload-rules && udevadm trigger`.

- [ ] **Step 1: Write the PKGBUILD per Arch conventions**

- [ ] **Step 2: Verify it builds in a clean Arch container** (CI or manual; if no Arch env available, document the manual verification step in the commit message)

- [ ] **Step 3: Commit**

```bash
git add packaging/arch/PKGBUILD
git commit -m "feat(packaging): AUR PKGBUILD skeleton for lconnect-linux"
```

---

## Sub-phase G — Cmd-15 investigation (1-day timebox)

### Task P1G-1: Hypothesis tests + writeup

**Files:**
- Create: `docs/protocol/captures/2026-04-27-cmd15-investigation.txt`

**Hard timebox: 1 working day.** Whatever the outcome, write up findings. The Phase 0e probe runs already strongly suggest the activation mechanism is one of:
- (a) Boot-time display (cmd 15 is a stored-image write; rendered next power-cycle)
- (b) Sibling activate cmd we haven't reached (analogous to slv3/lcd207's filename-keyed UpdateFireWare)

- [ ] **Step 1: Power-cycle hypothesis (highest priority)**

Procedure:
1. `cargo run -p verify-galahad -- upload-jpeg --mode upload --path test-images/02-concentric-rings.png` (cmd 15 path)
2. User power-cycles the AIO (USB unplug + replug, or full host reboot)
3. Observe: does the LCD show concentric-rings on next boot?
4. If yes → **HYPOTHESIS CONFIRMED**: cmd 15 is the boot-image path. Document the activation as "next power-cycle" + add a `lconnectctl lcd boot-image <PATH>` CLI subcommand idea (NOT implemented in Phase 1 — file an issue / next-phase note).
5. If no → move to Step 2.

- [ ] **Step 2: `LCDControlMode` toggle hypothesis**

After a successful cmd 15 upload, toggle through `LCDControlMode::LocalAVI=3` and `LCDControlMode::LocalH264=2`. Does the stored image render in either mode? (Read-only-ish — these are state changes, not writes to the LCD.)

- [ ] **Step 3: Sibling-activate-cmd grep**

Search the C# source one more time for callers of cmd 15 (`UploadJpeg`) — any followups, retries, or paired commands? Document any candidate found.

- [ ] **Step 4: Write findings**

Output: `docs/protocol/captures/2026-04-27-cmd15-investigation.txt`. Structure:
- Hypothesis + steps run + observations
- Final classification: confirmed / refuted / inconclusive (1-day timebox expired)
- Phase-onward implications

If activation found: file a "Phase 1 follow-up" task to expose the path via IPC; for Phase 1 itself, do not lift the cmd-15 IPC seal mid-phase.

- [ ] **Step 5: Commit**

```bash
git add docs/protocol/captures/2026-04-27-cmd15-investigation.txt
git commit -m "docs(protocol): cmd-15 investigation findings (1-day timebox)"
```

---

## Sub-phase H — Wrap-up

### Task P1H-1: Live device verification smoke run

**Files:**
- Create / append: `docs/protocol/captures/2026-04-27-phase-1-smoke-run.txt`

End-to-end live exercise of the daemon against the user's LCDV2. Per the user's standing preference, **one device action at a time, with visual confirmation between writes.**

- [ ] **Step 1: Install + start the daemon**

```bash
cargo install --path crates/lconnect-daemon
cargo install --path crates/lconnect-cli
cargo install --path tools/verify-galahad

# Drop in udev rule + systemd unit (paths per host's installation scheme)
sudo install -m 0644 packaging/udev/70-lianli-hid.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
mkdir -p ~/.config/systemd/user
cp packaging/systemd/lconnectd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now lconnectd

# Confirm clean startup
journalctl --user -u lconnectd --since "1 minute ago"
```

- [ ] **Step 2: Run the smoke script** (one command at a time; pause for visual confirmation)

```bash
lconnectctl daemon status
lconnectctl daemon version
lconnectctl devices
lconnectctl firmware
lconnectctl snapshot
lconnectctl watch  # Ctrl+C after 5-10 seconds; record what was shown
lconnectctl fan 50
lconnectctl pump 75 --source mcu
lconnectctl lighting set pump --mode StaticColor --color '#FF0000' --scope outer
# WAIT 10 SECONDS — confirm pump cap stays red (continuous re-issue working)
lconnectctl led-colors set screen --color '#00FF00'
lconnectctl lcd image test-images/01-radial-gradient.png
# WAIT 60 SECONDS — confirm image still showing (refresh loop working)
lconnectctl lcd clear
# Disconnect + reconnect device — confirm lighting + PWM replay; LCD does not
systemctl --user restart lconnectd
# Confirm settings replay; LCD does not
```

- [ ] **Step 3: Capture observations**

Append to `docs/protocol/captures/2026-04-27-phase-1-smoke-run.txt`:
- Each command's outcome (success / error / partial)
- Visual confirmations from the user
- Any unexpected behavior (e.g., visible flicker during refresh)
- Whether all spec acceptance criteria (section 14) pass

- [ ] **Step 4: Commit**

```bash
git add docs/protocol/captures/2026-04-27-phase-1-smoke-run.txt
git commit -m "docs(protocol): Phase 1 smoke run against LCDV2 (live)"
```

---

### Task P1H-2: README + status doc updates + post-execution notes

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-04-27-phase-1-daemon.md` (this file — append post-execution notes)

- [ ] **Step 1: Update README**

Add Phase 1 bullet to the Status block (mirroring the Phase 0e bullet's style):

```markdown
- ✅ Phase 1: `lconnectd` daemon + `lconnectctl` CLI live-verified on
  Galahad II LCDV2. Sensors at 1 Hz, fan/pump PWM, lighting (preset
  modes + per-pixel via cmd 20), LCD config + JPEG image with
  daemon-managed refresh loop, persistence across daemon restart and
  device hot-plug. systemd --user installation; udev rule for LCDV2.
  CCommandPacket framing codec landed (codec-only). Cmd 15
  investigation: [confirmed-boot-image | inconclusive] — see
  `docs/protocol/captures/2026-04-27-cmd15-investigation.txt`.
```

Add a "Daemon installation" section under Building (or update the existing one) with the `cargo install` + `udevadm` + `systemctl --user` sequence.

- [ ] **Step 2: Append post-execution notes**

Mirror Phase 0e's wrap-up convention (final task count, commit count, cargo gates, headline findings, plan deviations, real-hardware findings, Phase 2 inputs).

- [ ] **Step 3: Commit**

```bash
git add README.md docs/superpowers/plans/2026-04-27-phase-1-daemon.md
git commit -m "docs: Phase 1 wrap-up — README status + post-execution notes"
```

---

## Phase 1 exit criteria

When all of the following pass, Phase 1 is complete and Phase 2 (GUI shell) can begin.

- [ ] `cargo build --workspace` and `cargo build --workspace --release` succeed
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo doc --workspace --no-deps` builds with no warnings
- [ ] `systemctl --user enable --now lconnectd` starts cleanly; `journalctl --user -u lconnectd` shows clean startup
- [ ] `lconnectctl devices` lists the LCDV2
- [ ] `lconnectctl watch` shows live coolant temp updating at 1 Hz
- [ ] `lconnectctl fan 50` / `pump 75 --source mcu` set PWM live-verifiably
- [ ] `lconnectctl lighting set pump --mode StaticColor --color '#FF0000' --scope outer` lights pump cap red, **stays red after 10 s** (continuous re-issue working)
- [ ] `lconnectctl led-colors set screen --color '#00FF00'` lights LED ring green
- [ ] `lconnectctl lcd image <some.jpg>` displays JPEG, **still showing after 60 s** (refresh loop working)
- [ ] `lconnectctl lcd clear` reverts to LocalUI
- [ ] Disconnect + reconnect device → lighting and PWM replay from persistence; LCD does not
- [ ] Daemon survives `verify-galahad` running concurrently against the same hardware path (or behaves predictably if both try to open the same handle)
- [ ] Stop & restart daemon → settings replay; LCD does not
- [ ] CCommandPacket framing codec lands with ≥10 unit tests + a `hid-safety-reviewer` dispatch in the plan transcript
- [ ] Cmd-15 investigation has a written outcome doc, regardless of whether activation was found
- [ ] AUR PKGBUILD builds successfully on a clean Arch container (CI or manual)
- [ ] Every HID write path landed in this phase has a `hid-safety-reviewer` dispatch recorded in the plan transcript

---

## Self-review notes (post-write)

**Spec coverage check:** every section of the spec maps to one or more tasks:
- §3 design decisions → encoded throughout (refresh-loop in P1D-7/P1D-8, persistence model in P1D-3/P1D-15, lighting re-issue in P1D-7, CCommandPacket in P1A-1/P1A-2)
- §4 process model → P1D-4..P1D-10
- §5 IPC contract → P1C-1..P1C-6 + P1D-11
- §6 persistence → P1D-3 + P1D-15
- §7 CLI surface → P1E-1..P1E-7
- §9 protocol-side additions → P1A-1, P1A-2, P1G-1
- §10 workspace layout → covered by all crate-scaffold tasks
- §11 packaging → P1F-1..P1F-3
- §12 observability → woven into P1D-1, P1D-12
- §13 testing strategy → each task has TDD steps; in-memory transport (P1C-6) replaces hardware in CI
- §14 acceptance criteria → mapped to the Phase 1 exit criteria above
- §15 risks → addressed in design (`spawn_blocking` for hidapi quirks; per-device serialization for HID contention)

**Placeholder scan:** zero "TBD" / "TODO" / "implement later" in plan body. Three `unimplemented!()` stubs are acceptable — they sit where the implementer expands the body following the trait shape (P1A-1 cpacket bodies, P1D-4 `spawn_device_actor_tree`). The implementer treats those as "expand mechanically per the surrounding pattern".

**Type consistency:** `Framing` (lconnect-hid) and `FramingKind` (lconnect-ipc) are intentionally separate types — the IPC type serialises across the wire as a strict A/B/C subset, while the internal `Framing` enum is `#[non_exhaustive]` for future extension. Conversion happens at the IPC boundary in `IpcServerImpl::raw_send`. Trait-method names in P1C-2 match what `IpcServer::dispatch` (P1C-4), the dispatch table in `IpcServerImpl` (P1D-11), and CLI subcommands (P1E-2..P1E-7) all reference. `DeviceHandles` field names are stable from P1D-4 through P1D-11.

**Scope check:** Phase 1 is one cohesive deliverable (a working daemon + CLI). It is not decomposable into independent sub-projects without artificial cuts. The sub-phase breakdown (A..H) gives the implementer natural progress checkpoints, but each sub-phase depends on its predecessors.
