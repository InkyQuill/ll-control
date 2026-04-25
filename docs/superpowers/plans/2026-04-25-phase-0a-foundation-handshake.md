# Phase 0a — Foundation + Handshake Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the Cargo workspace, the `protocol` crate skeleton, the `verify-galahad` test harness, and prove the architecture end-to-end by reading live sensor data from a real Galahad II LCDV2 device through fully-tested protocol code.

**Architecture:** Multi-crate Rust workspace. The `protocol` crate is pure (no I/O, no async, no hidapi); it owns byte layouts and types. The `verify-galahad` binary is a temporary Phase-0 test harness that opens the device via `hidapi-rs` and exercises commands using the `protocol` crate. Everything is TDD — each command's encode and decode is locked behind golden-byte tests before any live device interaction.

**Tech Stack:** Rust 1.92 / Edition 2021. Crates: `bytes` 1, `thiserror` 1 (protocol crate); `hidapi` 2.5, `anyhow` 1, `clap` 4 (verify-galahad). No tokio, no GUI — those come in Phase 1+.

**Source-of-truth references throughout this plan (decompiled C#):**
- `L-Connect.Core/LConnectCore/Products/Galahad2Vision/ACommandPacket.cs` — packet framing
- `L-Connect.Core/LConnectCore/Products/Galahad2Vision/Commands.cs` — command codes
- `L-Connect.Core/LConnectCore/Products/Galahad2Vision/HandshakeInfo.cs` — response parsing
- `L-Connect.Core/LConnectCore/Products/Galahad2Vision/Galahad2VisionDevice.cs` — send/receive flow
- `L-Connect.Core/LConnectCore/Devices/KnownDevices.cs` — VID/PID table

All paths in this plan are relative to `/home/inky/Development/lian-li/lconnect-linux/` unless otherwise stated.

---

## Part A — Repo cleanup and workspace foundation

### Task 1: Archive the buggy existing scaffold

The existing top-level `Cargo.toml`, `src/`, root-level `*.rs` test binaries, `demo.sh`, and the legacy markdown files do not fit the new workspace layout and contain the bugs documented in the design spec §3. Move them all into `docs/protocol/captures/legacy-scaffold/` for reference (Phase 0 may want to read them when comparing observed behaviour to past attempts). They are *kept*, not deleted — the design spec promises they are referenced from the protocol doc.

**Files:**
- Create: `docs/protocol/captures/legacy-scaffold/` (directory)
- Move (preserving content):
  - `Cargo.toml` → `docs/protocol/captures/legacy-scaffold/Cargo.toml.txt`
  - `Cargo.lock` → `docs/protocol/captures/legacy-scaffold/Cargo.lock.txt`
  - `src/` → `docs/protocol/captures/legacy-scaffold/src/`
  - `test_hid.rs`, `test_hid_advanced.rs`, `parse_test.rs`, `final_test.rs` → `docs/protocol/captures/legacy-scaffold/`
  - `demo.sh` → `docs/protocol/captures/legacy-scaffold/demo.sh`
  - `RUST_RECOMMENDATION.md`, `SETUP_PERMISSIONS.md`, `TEST_RESULTS.md` → `docs/protocol/captures/legacy-scaffold/`
  - `99-lian-li-galahad2.rules` → `docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules`
- Keep in place: `assets/`, `README.md`, `.gitignore`, `.git/`, `docs/superpowers/`

The `.txt` extension on `Cargo.toml`/`Cargo.lock` is deliberate — `cargo` would otherwise see them as part of a sub-package and complain.

- [ ] **Step 1: Create archive directory**

```bash
mkdir -p docs/protocol/captures/legacy-scaffold
```

- [ ] **Step 2: Move files (using `git mv` so history follows where applicable, but most are untracked so plain `mv` is fine)**

```bash
mv Cargo.toml docs/protocol/captures/legacy-scaffold/Cargo.toml.txt
mv Cargo.lock docs/protocol/captures/legacy-scaffold/Cargo.lock.txt
mv src docs/protocol/captures/legacy-scaffold/src
mv test_hid.rs test_hid_advanced.rs parse_test.rs final_test.rs \
   docs/protocol/captures/legacy-scaffold/
mv demo.sh docs/protocol/captures/legacy-scaffold/
mv RUST_RECOMMENDATION.md SETUP_PERMISSIONS.md TEST_RESULTS.md \
   docs/protocol/captures/legacy-scaffold/
mv 99-lian-li-galahad2.rules docs/protocol/captures/legacy-scaffold/
```

- [ ] **Step 3: Add a README inside the archive explaining what's there and why**

Create `docs/protocol/captures/legacy-scaffold/README.md`:

```markdown
# Legacy scaffold (archived 2026-04-25)

Pre-rewrite Rust scaffold for L-Connect Linux. Kept as a reference for Phase 0
protocol verification — specifically because the handshake byte offsets in
`src/device/galahad.rs` are correct (despite the broken architecture around
them) and the `99-lian-li-galahad2.rules` file documents the device path the
author tested against.

**Do not import from this directory.** The new code lives in
`crates/` and `tools/`. This is reference-only.

## Known issues with the archived code

1. `src/app.rs` runs blocking HID I/O on the egui paint thread. See design
   spec §3.
2. `src/device/galahad.rs` `set_fan_pwm` / `set_pump_pwm` are wrong: they
   place the PWM byte at packet[2] (the reserved byte). The decompiled C#
   places `[group, pwm]` as data bytes (packet[6..7]) with `Length=2` at
   packet[5]. See `Galahad2VisionDevice.cs` line 280 / 321.
3. `set_blocking_mode(false)` followed by `read_timeout(...)` — undefined
   behaviour in hidapi.
```

- [ ] **Step 4: Verify the move**

Run:

```bash
ls -la
```

Expected: top-level lists `assets/`, `README.md`, `docs/`, `.git/`, `.gitignore`. **No** `Cargo.toml`, `Cargo.lock`, `src/`, root-level `.rs` files, `demo.sh`, etc.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: archive pre-rewrite scaffold to docs/protocol/captures/

The pre-rewrite Rust scaffold had architectural bugs (blocking HID
I/O on the egui paint thread; wrong byte layout for SetFanPWM/
SetPumpPWM). It is kept for reference under docs/protocol/captures/
legacy-scaffold/ but not imported."
```

---

### Task 2: Create the workspace `Cargo.toml`

The workspace owns common dependency versions and the member list. Member crates inherit `edition`, `version`, etc. via `package.workspace = true`.

**Files:**
- Create: `Cargo.toml` (workspace root)

- [ ] **Step 1: Write the workspace manifest**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/protocol",
    "tools/verify-galahad",
]

[workspace.package]
version      = "0.1.0"
edition      = "2021"
rust-version = "1.82"
license      = "GPL-3.0-or-later"
repository   = "https://github.com/inky/lconnect-linux"
authors      = ["L-Connect Linux contributors"]

[workspace.dependencies]
# Used by `protocol`
bytes      = "1"
thiserror  = "1"

# Used by `verify-galahad`
hidapi     = "2.5"
anyhow     = "1"
clap       = { version = "4", features = ["derive"] }

[profile.release]
opt-level     = 3
lto           = true
strip         = true
codegen-units = 1
```

- [ ] **Step 2: Verify the manifest parses (no members exist yet, so `cargo metadata` fails — that's fine, we only check that the workspace TOML itself is syntactically valid)**

Run:

```bash
cargo metadata --no-deps --format-version 1 2>&1 | head -3
```

Expected: an error mentioning the missing `crates/protocol/Cargo.toml` (or `tools/verify-galahad/Cargo.toml`). It must **not** be a TOML parse error. Any error containing the substring `expected an array` or `invalid type` means the manifest is malformed — fix it.

- [ ] **Step 3: Update `.gitignore`**

Replace `.gitignore` with:

```gitignore
# Rust
/target
**/*.rs.bk
Cargo.lock

# IDE
.vscode/
.idea/

# OS
.DS_Store

# Local capture artefacts (real device serials, etc.)
/docs/protocol/captures/private/
```

Note: `Cargo.lock` is gitignored at the workspace root because this project produces both a library (`protocol`) and binaries (`verify-galahad`), and committing `Cargo.lock` forces every consumer onto the same dep versions. We can revisit before the v1 release.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml .gitignore
git commit -m "chore: add workspace Cargo.toml and updated .gitignore"
```

---

### Task 3: Create the `protocol` crate skeleton

A library crate with empty modules matching the spec layout. No types yet — those come task by task.

**Files:**
- Create: `crates/protocol/Cargo.toml`
- Create: `crates/protocol/src/lib.rs`
- Create: `crates/protocol/src/common/mod.rs`
- Create: `crates/protocol/src/galahad2_lcdv2/mod.rs`

- [ ] **Step 1: Create the crate manifest**

Create `crates/protocol/Cargo.toml`:

```toml
[package]
name         = "lconnect-protocol"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
description = "Pure protocol types and codecs for Lian Li L-Connect devices."

[lib]
name = "lconnect_protocol"

[dependencies]
bytes     = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 2: Create `lib.rs`**

Create `crates/protocol/src/lib.rs`:

```rust
//! Pure protocol types and codecs for Lian Li L-Connect devices.
//!
//! No I/O, no async runtime, no hidapi. Bytes in, bytes out. Hardware
//! interaction lives in higher-level crates (`hid`, `daemon`, the
//! Phase-0 `verify-galahad` harness).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod common;
pub mod galahad2_lcdv2;

/// Errors produced by the protocol crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A response or input buffer was too short for the operation.
    #[error("buffer too short: need {need} bytes, got {got}")]
    BufferTooShort {
        /// Required byte count.
        need: usize,
        /// Actual byte count.
        got: usize,
    },

    /// The HID Report ID byte was not the expected value.
    #[error("unexpected report id: expected {expected:#04x}, got {got:#04x}")]
    UnexpectedReportId {
        /// Expected report id.
        expected: u8,
        /// Actual report id.
        got: u8,
    },

    /// The command echo byte did not match the request command.
    #[error("unexpected command echo: expected {expected:#04x}, got {got:#04x}")]
    UnexpectedCommand {
        /// Expected command byte.
        expected: u8,
        /// Actual command byte.
        got: u8,
    },

    /// A response payload's `Length` field claimed more bytes than fit.
    #[error("invalid length field: {length} (max {max})")]
    InvalidLength {
        /// The bogus length value.
        length: usize,
        /// The maximum allowed.
        max: usize,
    },

    /// A response indicated its data was not valid (device-side flag).
    #[error("device reported response data invalid")]
    DataNotValid,
}

/// Convenience alias for `Result<T, protocol::Error>`.
pub type Result<T> = core::result::Result<T, Error>;
```

- [ ] **Step 3: Create `common/mod.rs` (placeholder; types added in Task 5)**

Create `crates/protocol/src/common/mod.rs`:

```rust
//! Cross-SKU common types.
//!
//! Filled in as multi-SKU work begins. For Phase 0a we do not need
//! anything here yet, but the module exists so future tasks can add
//! files without churning `lib.rs`.
```

- [ ] **Step 4: Create `galahad2_lcdv2/mod.rs` (placeholder; submodules added in Tasks 4–8)**

Create `crates/protocol/src/galahad2_lcdv2/mod.rs`:

```rust
//! Galahad II LCDV2 (`0x0416:0x7395`) protocol.
//!
//! Decompiled-source references are in
//! `docs/protocol/decompiled-source-map.md`.
//!
//! USB / HID identity:
//! - Vendor ID:  `0x0416`  (Winbond / Lian Li)
//! - Product ID: `0x7395`  (Galahad II LCDV2)
//! - Firmware:   `1.6` (only verified revision in Phase 0a)
//!
//! Submodules are added incrementally as commands are verified.
```

- [ ] **Step 5: Verify the crate builds**

Run:

```bash
cargo build -p lconnect-protocol
```

Expected: success, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/protocol/
git commit -m "feat(protocol): scaffold lconnect-protocol crate

Empty modules for common/ and galahad2_lcdv2/, plus the Error enum
covering the failure modes the codecs will return. No types or codecs
yet — those land per-command, TDD'd."
```

---

### Task 4: Create the `verify-galahad` harness skeleton

A tiny binary that compiles, parses CLI args, and prints "hello". Real subcommands land later.

**Files:**
- Create: `tools/verify-galahad/Cargo.toml`
- Create: `tools/verify-galahad/src/main.rs`

- [ ] **Step 1: Create the crate manifest**

Create `tools/verify-galahad/Cargo.toml`:

```toml
[package]
name         = "verify-galahad"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
description = "Phase-0 test harness for Galahad II LCDV2 protocol verification. Not a product binary."
publish     = false

[[bin]]
name = "verify-galahad"
path = "src/main.rs"

[dependencies]
lconnect-protocol = { path = "../../crates/protocol" }
hidapi            = { workspace = true }
anyhow            = { workspace = true }
clap              = { workspace = true }
```

- [ ] **Step 2: Create `main.rs`**

Create `tools/verify-galahad/src/main.rs`:

```rust
//! Phase-0 test harness for Galahad II LCDV2.
//!
//! Not a product binary. This exists only to exercise commands defined
//! in `lconnect-protocol` against a live device and print the results
//! into `docs/protocol/galahad2-lcdv2.md`'s "Verification" sections.
//!
//! Once Phase 1 stands up the real daemon, this tool is retired.

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Phase-0 test harness for Galahad II LCDV2.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List attached Lian Li devices.
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::List => {
            println!("verify-galahad: no commands implemented yet");
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Verify the binary builds**

Run:

```bash
cargo build -p verify-galahad
```

Expected: success. The first build will compile `hidapi` and its C deps — that's slow on a cold cache (~30s) but only happens once.

- [ ] **Step 4: Run it once to confirm CLI parsing works**

Run:

```bash
cargo run -q -p verify-galahad -- list
```

Expected output:

```
verify-galahad: no commands implemented yet
```

- [ ] **Step 5: Commit**

```bash
git add tools/verify-galahad/
git commit -m "feat(verify-galahad): scaffold Phase-0 test harness binary

Empty subcommand list (only \`list\` exists; it does nothing yet).
Compiles, links hidapi, parses CLI args. Real command-verification
subcommands are added per command in subsequent tasks."
```

---

## Part B — Decompiled-source map

### Task 5: Write the decompiled-source map document

A single document that says, for each subsystem we'll touch, exactly which decompiled C# files implement it. Future tasks reference this rather than re-discovering it.

**Files:**
- Create: `docs/protocol/decompiled-source-map.md`

- [ ] **Step 1: Write the source map**

Create `docs/protocol/decompiled-source-map.md`:

```markdown
# Decompiled source map — Galahad II LCDV2

Maps subsystems we implement in `crates/protocol/galahad2_lcdv2/` to the
decompiled C# files at
`/home/inky/Development/lian-li/L-Connect.Core/LConnectCore/`.

All paths below are relative to that root.

## Device identity

| Topic | File | Notes |
|---|---|---|
| VID/PID table | `Devices/KnownDevices.cs` | `Galahad2LCDV2 = (1046, 29589)` = `0x0416:0x7395`. Lines 36–47. |
| Controller type enum | `Devices/ControllerTypeID.cs` | Maps SKU names to enum. |
| HID device interface | `Devices/IHidDevice.cs`, `Devices/HidApiHidDevice.cs` | Used at the I/O layer; not needed in `protocol` crate. |

## Galahad II Vision (folder: `Products/Galahad2Vision/`)

This folder implements multiple SKUs (Trinity Performance / Trinity Regular
/ LCD / LCDV2). The wire protocol within the folder is shared; SKU
divergence is by VID/PID at discovery time. **For Phase 0a we treat all of
`Galahad2Vision/` as describing LCDV2's protocol.** If Phase 0b finds a
SKU-specific branch, we annotate it then.

### Packet framing

| Packet type | File | Use |
|---|---|---|
| `ACommandPacket` (64 B) | `Products/Galahad2Vision/ACommandPacket.cs` | Standard request/response. **Used by all Phase 0a commands.** |
| `BCommandPacket` (512 B) | `Products/Galahad2Vision/BCommandPacket.cs` | Bulk transfers (firmware upload, JPEG/H264 frames). Phase 0b. |
| `CCommandPacket` | `Products/Galahad2Vision/CCommandPacket.cs` | Yet to characterise. Phase 0b. |
| Base `Packet` | `Devices/Packet.cs` | Holds the underlying `byte[]`. |

### Commands

| Topic | File | Notes |
|---|---|---|
| Command code enum | `Products/Galahad2Vision/Commands.cs` | 24 commands; lines 5–30. |
| Request types (high-level) | `Products/Galahad2Vision/RequestType.cs` | 14 entries. Lines 5–21. Note: `RequestType` is a higher-level abstraction; it maps to one or more `Commands` codes inside the device class. Phase 0a only needs `Commands`. |

### Per-command source

| Command (code) | Read in | Send in | Response parser |
|---|---|---|---|
| `GetHandshakeInfo` (129) | `Galahad2VisionDevice.cs:107–119` (sync), `:204–217` (event) | `Galahad2VisionDevice.cs:250–253` (`SendGetHandshakeInfo`) | `HandshakeInfo.cs:58–76` (`TryParse`) |
| `GetFirmwareVersion` (134) | `Galahad2VisionDevice.cs` (around `FirmwareVersion`) | `Galahad2VisionDevice.cs:writeACommand(134)` | `FirmwareVersion.cs` |
| `SetFanPWM` (139) | n/a (write only) | `Galahad2VisionDevice.cs:280` (`writeACommand(139, new byte[]{0, pwm})`) | n/a |
| `SetPumpPWM` (138) | n/a | `Galahad2VisionDevice.cs:321` (`writeACommand(138, new byte[]{source, pwm})`) | n/a |
| `GetPumpLightingInfo` (130) | TBD Phase 0b | `Galahad2VisionDevice.cs:` `writeACommand(130)` | `PumpLightingInfo.cs` |
| `SetPumpLighting` (131) | n/a | `Galahad2VisionDevice.cs:312` (`writeACommand(131, setting.ToBytes())`) | n/a |
| `GetFanLightingInfo` (132) | TBD Phase 0b | similar | `FanLightingInfo.cs` |
| `SetFanLighting` (133) | n/a | `Galahad2VisionDevice.cs:271` | `FanLightingSetting.cs` `ToBytes` |
| `LCDControl` (12) | (B-packet) | `Galahad2VisionDevice.cs:LCDControl(...)` | `LCDControlSetting.cs` |
| `IsLCDAvailable` (23) | TBD Phase 0b | TBD | TBD |
| `SendJpeg` (14) | n/a | TBD Phase 0b | n/a |
| `UploadJpeg` (15) | n/a | TBD Phase 0b | n/a |
| `SendH264Frame` (13) | n/a | `Galahad2VisionDevice.cs:SendH264Frame` | n/a |
| `ResetDevice` (142) | n/a | `Galahad2VisionDevice.cs:233–246` | response Length=1, data[0]=0x01 = success |

### Crypto

| Topic | File | Status |
|---|---|---|
| Crypto helper | `Crypto/CryptoHelper.cs` | Needs Phase 0b investigation: is it called per-packet (transport encrypted) or only on theme/profile bundles? Search for `CryptoHelper.` callers. |

### Sensor / telemetry types

| Type | File |
|---|---|
| `HandshakeInfo` (live sensor reading) | `Products/Galahad2Vision/HandshakeInfo.cs` |
| `FirmwareVersion` | `Products/Galahad2Vision/FirmwareVersion.cs` |
| `SensorType` enum | `Products/Galahad2Vision/SensorType.cs` |

### Lighting types

| Type | File |
|---|---|
| `FanLightingMode` | `Products/Galahad2Vision/FanLightingMode.cs` |
| `PumpLightingMode` | `Products/Galahad2Vision/PumpLightingMode.cs` |
| `FanLightingSetting` | `Products/Galahad2Vision/FanLightingSetting.cs` |
| `PumpLightingSetting` | `Products/Galahad2Vision/PumpLightingSetting.cs` |
| `FanLightingInfo` | `Products/Galahad2Vision/FanLightingInfo.cs` |
| `PumpLightingInfo` | `Products/Galahad2Vision/PumpLightingInfo.cs` |
| `LightingDirection` | `Products/Galahad2Vision/LightingDirection.cs` |
| `ARGBSignalSource` | `Products/Galahad2Vision/ARGBSignalSource.cs` |
| `ScreenLEDLightingSetting` | `Products/Galahad2Vision/ScreenLEDLightingSetting.cs` |
| `ScreenLEDLightingSubSetting` | `Products/Galahad2Vision/ScreenLEDLightingSubSetting.cs` |

### LCD / screen types

| Type | File |
|---|---|
| `LCDControlMode` | `Products/Galahad2Vision/LCDControlMode.cs` |
| `LCDControlSetting` | `Products/Galahad2Vision/LCDControlSetting.cs` |
| `ScreenRotation` | `Products/Galahad2Vision/ScreenRotation.cs` |
| `SyncScreenInfo` | `Products/Galahad2Vision/SyncScreenInfo.cs` |
```

- [ ] **Step 2: Verify the doc renders**

Open it (manual): `xdg-open docs/protocol/decompiled-source-map.md` or any markdown viewer. Confirm tables render. Skim the file paths for typos.

- [ ] **Step 3: Commit**

```bash
git add docs/protocol/decompiled-source-map.md
git commit -m "docs(protocol): add decompiled-source map

Maps every subsystem we'll implement in crates/protocol/ to the
exact decompiled C# files. Future tasks reference this rather than
re-discovering paths."
```

---

## Part C — `ACommandPacket` (the base 64-byte packet)

`ACommandPacket` is the wire format for every Phase 0a command. We TDD it before any handshake-specific code so all later tests reuse it.

**Verified layout (`ACommandPacket.cs:12–18, 20–80`):**

```
byte[ 0]    ReportID = 0x01 (constant)
byte[ 1]    Command  (e.g., 0x81 = 129 = GetHandshakeInfo)
byte[ 2]    0x00 (set in constructor; unused)
byte[ 3]    PacketNumber, big-endian high byte
byte[ 4]    PacketNumber, big-endian low byte
byte[ 5]    Length: number of valid data bytes in this packet (0..=58)
byte[ 6..63] Data (up to 58 bytes; bytes past Length are unspecified)
```

Constants:
- `PACKET_LEN = 64`
- `HEADER_LEN = 6`
- `MAX_PAYLOAD = 58`

### Task 6: TDD `ACommandPacket::new_request` (build a request packet from cmd + data)

**Files:**
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs` (add `pub mod packets;`)
- Create: `crates/protocol/src/galahad2_lcdv2/packets.rs`
- Create: `crates/protocol/tests/galahad2_lcdv2_packets.rs`

- [ ] **Step 1: Add the module declaration**

Edit `crates/protocol/src/galahad2_lcdv2/mod.rs`, append:

```rust

pub mod packets;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/protocol/tests/galahad2_lcdv2_packets.rs`:

```rust
//! Golden-byte tests for ACommandPacket.

use lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket;

#[test]
fn empty_request_is_64_bytes_of_header_only() {
    let packet = ACommandPacket::new_request(0x81, &[]);
    let bytes = packet.as_bytes();

    assert_eq!(bytes.len(), 64);
    assert_eq!(bytes[0], 0x01, "ReportID");
    assert_eq!(bytes[1], 0x81, "Command echo (129 = GetHandshakeInfo)");
    assert_eq!(bytes[2], 0x00, "reserved");
    assert_eq!(bytes[3], 0x00, "PacketNumber high");
    assert_eq!(bytes[4], 0x00, "PacketNumber low");
    assert_eq!(bytes[5], 0x00, "Length = 0 for empty request");
    assert!(bytes[6..].iter().all(|&b| b == 0), "payload zero-filled");
}

#[test]
fn request_with_two_data_bytes_sets_length_and_payload() {
    // Mirrors `writeACommand(139, new byte[]{0, pwm})` from
    // Galahad2VisionDevice.cs:280 (SetFanPWM).
    let packet = ACommandPacket::new_request(0x8B /* 139 */, &[0x00, 0x42]);
    let bytes = packet.as_bytes();

    assert_eq!(bytes.len(), 64);
    assert_eq!(bytes[1], 0x8B);
    assert_eq!(bytes[5], 0x02, "Length = 2");
    assert_eq!(bytes[6], 0x00, "data[0] = group");
    assert_eq!(bytes[7], 0x42, "data[1] = pwm = 0x42");
    assert!(bytes[8..].iter().all(|&b| b == 0));
}

#[test]
fn request_clamps_data_to_58_bytes() {
    let huge = [0xAAu8; 100];
    let packet = ACommandPacket::new_request(0x10, &huge);
    let bytes = packet.as_bytes();

    assert_eq!(bytes[5], 58, "Length saturates at MAX_PAYLOAD");
    assert!(bytes[6..64].iter().all(|&b| b == 0xAA));
}
```

- [ ] **Step 3: Run the tests to confirm they fail**

Run:

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_packets 2>&1 | tail -10
```

Expected: compile error mentioning `ACommandPacket`. (Or, depending on cargo version, a "could not find module `packets`" error.)

- [ ] **Step 4: Implement `ACommandPacket` (just enough for these tests)**

Create `crates/protocol/src/galahad2_lcdv2/packets.rs`:

```rust
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
```

- [ ] **Step 5: Run the tests to confirm they pass**

Run:

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_packets 2>&1 | tail -10
```

Expected: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/src/galahad2_lcdv2/packets.rs \
        crates/protocol/tests/galahad2_lcdv2_packets.rs
git commit -m "feat(protocol): add ACommandPacket request builder

Verified against ACommandPacket.cs:12-80 with three golden-byte
tests covering empty payload, two-byte payload (mirrors SetFanPWM),
and >58-byte truncation."
```

---

### Task 7: TDD `ACommandPacket::parse_response` (decode an inbound 64-byte packet)

**Files:**
- Modify: `crates/protocol/tests/galahad2_lcdv2_packets.rs` (append)
- Modify: `crates/protocol/src/galahad2_lcdv2/packets.rs` (append)

- [ ] **Step 1: Append failing tests**

Edit `crates/protocol/tests/galahad2_lcdv2_packets.rs`, append:

```rust

#[test]
fn parse_response_extracts_header_and_data_slice() {
    // Synthetic GetHandshakeInfo response.
    // Header: ReportID=1, Cmd=129, reserved=0, PacketNumber=0, Length=7.
    // Data:   FanRpm=1234 (BE 0x04 0xD2), PumpRpm=2980 (BE 0x0B 0xA4),
    //         TempValid=1, TempInteger=42, TempFraction=7.
    let mut buf = [0u8; 64];
    buf[0] = 0x01; // ReportID
    buf[1] = 0x81; // Command echo (129)
    buf[5] = 0x07; // Length
    buf[6] = 0x04;
    buf[7] = 0xD2;
    buf[8] = 0x0B;
    buf[9] = 0xA4;
    buf[10] = 0x01;
    buf[11] = 0x2A;
    buf[12] = 0x07;

    let parsed = ACommandPacket::parse_response(&buf, 0x81)
        .expect("valid response should parse");
    assert_eq!(parsed.command(), 0x81);
    assert_eq!(parsed.length(), 7);
    assert_eq!(parsed.data(), &[0x04, 0xD2, 0x0B, 0xA4, 0x01, 0x2A, 0x07]);
}

#[test]
fn parse_response_rejects_wrong_report_id() {
    let mut buf = [0u8; 64];
    buf[0] = 0xFF;   // wrong
    buf[1] = 0x81;
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedReportId { expected: 0x01, got: 0xFF }
    ));
}

#[test]
fn parse_response_rejects_wrong_command_echo() {
    let mut buf = [0u8; 64];
    buf[0] = 0x01;
    buf[1] = 0x86; // 134, not what we asked for
    buf[5] = 0;
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedCommand { expected: 0x81, got: 0x86 }
    ));
}

#[test]
fn parse_response_rejects_short_buffer() {
    let buf = [0u8; 5];
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(err, lconnect_protocol::Error::BufferTooShort { .. }));
}

#[test]
fn parse_response_rejects_oversize_length_field() {
    let mut buf = [0u8; 64];
    buf[0] = 0x01;
    buf[1] = 0x81;
    buf[5] = 0xFF; // claims 255 bytes; max is 58
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::InvalidLength { length: 0xFF, max: 58 }
    ));
}
```

- [ ] **Step 2: Confirm the new tests fail to compile**

Run:

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_packets 2>&1 | tail -15
```

Expected: errors mentioning `parse_response`, `command`, `length`, `data` not found.

- [ ] **Step 3: Implement `parse_response` and accessors**

Edit `crates/protocol/src/galahad2_lcdv2/packets.rs`, append:

```rust

impl ACommandPacket {
    /// Parse and validate an inbound 64-byte HID buffer as an
    /// `ACommandPacket` reply to a request that used `expected_command`.
    ///
    /// Validates: minimum length, ReportID, command echo, and that the
    /// `Length` field fits in `MAX_PAYLOAD`.
    pub fn parse_response(
        buf: &[u8],
        expected_command: u8,
    ) -> crate::Result<ParsedResponse<'_>> {
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
```

- [ ] **Step 4: Run all tests**

Run:

```bash
cargo test -p lconnect-protocol 2>&1 | tail -10
```

Expected: `test result: ok. 8 passed; 0 failed` (3 existing + 5 new).

- [ ] **Step 5: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/packets.rs \
        crates/protocol/tests/galahad2_lcdv2_packets.rs
git commit -m "feat(protocol): add ACommandPacket::parse_response

Validates ReportID (0x01), command echo, and that the Length byte
(packet[5]) fits in MAX_PAYLOAD. Returns a borrowed ParsedResponse
view onto the data slice. Five new golden-byte tests cover the
happy path and four reject paths."
```

---

## Part D — `Commands` enum

### Task 8: Add the full Commands enum

The full enum is 24 lines and we need at least `GetHandshakeInfo` for Task 9. Adding the complete enum now keeps the doc-source aligned.

**Files:**
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs`
- Create: `crates/protocol/src/galahad2_lcdv2/commands.rs`

- [ ] **Step 1: Add module declaration**

Edit `crates/protocol/src/galahad2_lcdv2/mod.rs`, append:

```rust

pub mod commands;
```

- [ ] **Step 2: Write the failing test**

Append to `crates/protocol/tests/galahad2_lcdv2_packets.rs`:

```rust

#[test]
fn command_codes_match_decompiled_enum() {
    // From Products/Galahad2Vision/Commands.cs lines 7–29.
    use lconnect_protocol::galahad2_lcdv2::commands::Command;
    assert_eq!(Command::GetHandshakeInfo as u8, 129);
    assert_eq!(Command::GetFirmwareVersion as u8, 134);
    assert_eq!(Command::SetPumpPWM as u8, 138);
    assert_eq!(Command::SetFanPWM as u8, 139);
    assert_eq!(Command::ResetDevice as u8, 142);
    assert_eq!(Command::LCDControl as u8, 12);
    assert_eq!(Command::SendH264Frame as u8, 13);
    assert_eq!(Command::SendJpeg as u8, 14);
    assert_eq!(Command::IsLCDAvailable as u8, 23);
}
```

- [ ] **Step 3: Confirm it fails**

Run:

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_packets command_codes 2>&1 | tail -5
```

Expected: a "could not find `commands`" or "module `commands` not found" error.

- [ ] **Step 4: Create the commands module**

Create `crates/protocol/src/galahad2_lcdv2/commands.rs`:

```rust
//! Command codes for Galahad II LCDV2.
//!
//! Mirrors `Products/Galahad2Vision/Commands.cs`. The numeric values are
//! the **wire format** — never reorder.

/// Command byte sent in `ACommandPacket.bytes[1]`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Command {
    /// Activate already-uploaded firmware. (Code 10.)
    ActiveFirmware = 10,
    /// Begin a firmware upload. (Code 11.) `BCommandPacket`.
    UploadFirmware = 11,
    /// Set LCD control mode (manual / auto / etc.). (Code 12.)
    LCDControl = 12,
    /// Send one H.264 frame. (Code 13.) `BCommandPacket`.
    SendH264Frame = 13,
    /// Send one JPEG frame. (Code 14.) `BCommandPacket`.
    SendJpeg = 14,
    /// Upload a JPEG file. (Code 15.)
    UploadJpeg = 15,
    /// Upload an AVI file. (Code 18.)
    UploadAvi = 18,
    /// Upload an H.264 file. (Code 19.)
    UploadH264 = 19,
    /// Set per-LED screen colours. (Code 20.)
    SetLEDColors = 20,
    /// Send H.264 stream metadata. (Code 21.)
    UploadH264Info = 21,
    /// Set the boot animation. (Code 22.)
    UploadStartupAviMovie = 22,
    /// Query whether an LCD is attached. (Code 23.)
    IsLCDAvailable = 23,
    /// Sensors + temp + RPMs. (Code 129.)
    GetHandshakeInfo = 129,
    /// Read pump-lighting state. (Code 130.)
    GetPumpLightingInfo = 130,
    /// Write pump-lighting state. (Code 131.)
    SetPumpLighting = 131,
    /// Read fan-lighting state. (Code 132.)
    GetFanLightingInfo = 132,
    /// Write fan-lighting state. (Code 133.)
    SetFanLighting = 133,
    /// Read firmware version string. (Code 134.)
    GetFirmwareVersion = 134,
    /// Read serial number. (Code 136.)
    GetSerialNumber = 136,
    /// Write serial number. (Code 137.)
    SetSerialNumber = 137,
    /// Set pump PWM (data: `[source, percent]`). (Code 138.)
    SetPumpPWM = 138,
    /// Set fan PWM (data: `[group, percent]`). (Code 139.)
    SetFanPWM = 139,
    /// Soft-reset the device. (Code 142.)
    ResetDevice = 142,
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p lconnect-protocol 2>&1 | tail -5
```

Expected: `test result: ok. 9 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/src/galahad2_lcdv2/commands.rs \
        crates/protocol/tests/galahad2_lcdv2_packets.rs
git commit -m "feat(protocol): add Galahad II LCDV2 Command enum

Full enum mirroring Commands.cs:7-29. Documented per-variant.
non_exhaustive so adding new commands is non-breaking for downstream."
```

---

## Part E — Handshake (sensors)

### Task 9: TDD the `HandshakeInfo` decoder

**Files:**
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs`
- Create: `crates/protocol/src/galahad2_lcdv2/handshake.rs`
- Create: `crates/protocol/tests/galahad2_lcdv2_handshake.rs`

- [ ] **Step 1: Module wiring**

Edit `crates/protocol/src/galahad2_lcdv2/mod.rs`, append:

```rust

pub mod handshake;
```

- [ ] **Step 2: Write failing tests**

Create `crates/protocol/tests/galahad2_lcdv2_handshake.rs`:

```rust
//! Tests for GetHandshakeInfo (cmd 129).
//!
//! Source: Products/Galahad2Vision/HandshakeInfo.cs:58-76.

use lconnect_protocol::galahad2_lcdv2::handshake::{HandshakeInfo, build_request};
use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

#[test]
fn build_request_emits_a_64_byte_command_129_packet() {
    let bytes = build_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[0], 0x01, "ReportID");
    assert_eq!(bytes[1], 129, "Command");
    assert_eq!(bytes[5], 0, "Length = 0");
    assert!(bytes[6..].iter().all(|&b| b == 0), "no payload");
}

#[test]
fn parse_known_response_yields_expected_values() {
    // Synthetic well-formed response: fan=1234, pump=2980,
    // temp_valid=1, temp_int=42, temp_frac=7.  Expect 42.7°C.
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01; // ReportID
    buf[1] = 129;  // command echo
    buf[5] = 7;    // length
    buf[6] = 0x04;
    buf[7] = 0xD2; // 1234
    buf[8] = 0x0B;
    buf[9] = 0xA4; // 2980
    buf[10] = 1;   // temp valid
    buf[11] = 42;
    buf[12] = 7;

    let info = HandshakeInfo::from_response(&buf).expect("should parse");
    assert_eq!(info.fan_rpm, 1234);
    assert_eq!(info.pump_rpm, 2980);
    assert!(info.temp_valid);
    assert_eq!(info.temp_integer, 42);
    assert_eq!(info.temp_fraction, 7);
    let c = info.temperature_c();
    assert!((c - 42.7).abs() < 1e-6, "got {}", c);
}

#[test]
fn parse_response_with_invalid_temp_flag_still_decodes_rpms() {
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 129;
    buf[5] = 7;
    buf[6] = 0x00;
    buf[7] = 0x32; // fan = 50
    buf[8] = 0x00;
    buf[9] = 0xC8; // pump = 200
    buf[10] = 0;   // temp NOT valid
    buf[11] = 99;
    buf[12] = 99;

    let info = HandshakeInfo::from_response(&buf).expect("still parses");
    assert_eq!(info.fan_rpm, 50);
    assert_eq!(info.pump_rpm, 200);
    assert!(!info.temp_valid);
    // We still expose the raw bytes; callers can ignore them.
    assert_eq!(info.temp_integer, 99);
}

#[test]
fn parse_response_rejects_too_short_data() {
    // Length field claims < 7 bytes — handshake needs 7.
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 129;
    buf[5] = 6; // one byte short
    let err = HandshakeInfo::from_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { need: 7, got: 6 }
    ));
}

#[test]
fn temp_fraction_modulo_10() {
    // C# does `TemperatureFraction % 10` to compute the displayed °C.
    // We mirror that in `temperature_c()`.
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 129;
    buf[5] = 7;
    buf[10] = 1;
    buf[11] = 30;
    buf[12] = 17; // 17 % 10 = 7

    let info = HandshakeInfo::from_response(&buf).unwrap();
    assert_eq!(info.temp_fraction, 17, "raw byte preserved");
    assert!(
        (info.temperature_c() - 30.7).abs() < 1e-6,
        "modulo applied in display"
    );
}
```

- [ ] **Step 3: Confirm tests fail to compile**

Run:

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_handshake 2>&1 | tail -15
```

Expected: errors about missing `handshake` module, `HandshakeInfo`, `build_request`.

- [ ] **Step 4: Implement the handshake module**

Create `crates/protocol/src/galahad2_lcdv2/handshake.rs`:

```rust
//! GetHandshakeInfo (Command 129) — live sensor read.
//!
//! Sources:
//! - Request: `Galahad2VisionDevice.cs:250-253` (`SendGetHandshakeInfo`).
//! - Response framing: `ACommandPacket.cs`.
//! - Response data layout: `HandshakeInfo.cs:58-76`.

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID packet to request a handshake from the device.
///
/// Returns the raw bytes ready for `hid_device.write(&buf)`.
pub fn build_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetHandshakeInfo as u8, &[])
        .as_bytes()
}

/// Decoded GetHandshakeInfo response.
///
/// Mirrors `LConnectCore.Products.Galahad2Vision.HandshakeInfo` field-for-
/// field, with the small simplification that we drop the C# `IsValid`
/// boolean — failure is reported via `Result` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandshakeInfo {
    /// Fan tach reading in RPM.
    pub fan_rpm: u16,
    /// Pump tach reading in RPM.
    pub pump_rpm: u16,
    /// Whether the device considers the temperature reading reliable
    /// (e.g., probe attached).
    pub temp_valid: bool,
    /// Coolant temperature, integer part (°C).
    pub temp_integer: u8,
    /// Coolant temperature, fractional part — *raw* byte. The C# code
    /// applies `% 10` before display; see [`Self::temperature_c`].
    pub temp_fraction: u8,
}

impl HandshakeInfo {
    /// Parse a 64-byte HID response buffer.
    ///
    /// Validates the `ACommandPacket` header (ReportID, command echo,
    /// Length) before decoding. Returns
    /// [`crate::Error::BufferTooShort`] if `Length < 7`.
    pub fn from_response(buf: &[u8]) -> crate::Result<Self> {
        let parsed = ACommandPacket::parse_response(
            buf,
            Command::GetHandshakeInfo as u8,
        )?;
        let data = parsed.data();
        if data.len() < 7 {
            return Err(crate::Error::BufferTooShort {
                need: 7,
                got: data.len(),
            });
        }
        Ok(HandshakeInfo {
            fan_rpm:       u16::from_be_bytes([data[0], data[1]]),
            pump_rpm:      u16::from_be_bytes([data[2], data[3]]),
            temp_valid:    data[4] != 0,
            temp_integer:  data[5],
            temp_fraction: data[6],
        })
    }

    /// Convert to a single floating-point °C value (only meaningful when
    /// `temp_valid == true`). Mirrors the C# `Temperature` getter:
    /// `TemperatureInteger + (TemperatureFraction % 10) / 10`.
    pub fn temperature_c(&self) -> f32 {
        self.temp_integer as f32 + ((self.temp_fraction % 10) as f32) / 10.0
    }
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p lconnect-protocol 2>&1 | tail -10
```

Expected: `test result: ok. 14 passed; 0 failed` (9 from earlier + 5 new).

- [ ] **Step 6: Run clippy with strict warnings to catch design slips**

Run:

```bash
cargo clippy -p lconnect-protocol --all-targets -- -D warnings
```

Expected: `Finished` with no warnings. If clippy nags about a `must_use` or naming choice, fix it before committing.

- [ ] **Step 7: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/src/galahad2_lcdv2/handshake.rs \
        crates/protocol/tests/galahad2_lcdv2_handshake.rs
git commit -m "feat(protocol): implement GetHandshakeInfo (cmd 129)

build_request() emits a 64-byte command-129 packet; HandshakeInfo
::from_response decodes the 7-byte payload (fan_rpm BE, pump_rpm BE,
temp_valid, temp_int, temp_frac).  temperature_c() applies the
%10 trick on the fraction byte that the C# code does in its
Temperature getter.  5 golden-byte tests; clippy clean."
```

---

## Part F — `verify-galahad`: device discovery + live handshake

### Task 10: Implement device discovery (`verify-galahad list`)

**Files:**
- Modify: `tools/verify-galahad/src/main.rs`

- [ ] **Step 1: Replace `main.rs` with a working `list` subcommand**

Replace the contents of `tools/verify-galahad/src/main.rs`:

```rust
//! Phase-0 test harness for Galahad II LCDV2.
//!
//! Not a product binary. This exercises commands defined in
//! `lconnect-protocol` against a live device. Once Phase 1 stands up
//! the daemon, this tool is retired.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use hidapi::HidApi;

/// VID/PID pair for Galahad II LCDV2.
const GALAHAD2_LCDV2_VID: u16 = 0x0416;
const GALAHAD2_LCDV2_PID: u16 = 0x7395;

/// Phase-0 test harness for Galahad II LCDV2.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List Lian Li devices visible to hidapi (matches Galahad II LCDV2 VID/PID).
    List,
    /// Send GetHandshakeInfo (cmd 129) and decode the response.
    Handshake,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::List => list_devices(),
        Cmd::Handshake => handshake(),
    }
}

fn list_devices() -> Result<()> {
    let api = HidApi::new().context("HidApi::new failed")?;
    let mut found = 0usize;
    for info in api.device_list() {
        if info.vendor_id() == GALAHAD2_LCDV2_VID
            && info.product_id() == GALAHAD2_LCDV2_PID
        {
            found += 1;
            println!(
                "vid=0x{:04x} pid=0x{:04x}  path={:?}  manufacturer={:?}  product={:?}  serial={:?}  interface={}",
                info.vendor_id(),
                info.product_id(),
                info.path(),
                info.manufacturer_string(),
                info.product_string(),
                info.serial_number(),
                info.interface_number(),
            );
        }
    }
    if found == 0 {
        return Err(anyhow!(
            "no Galahad II LCDV2 (0x{:04x}:0x{:04x}) found by hidapi. \
             Check `lsusb`, plug the device in, and re-run.",
            GALAHAD2_LCDV2_VID,
            GALAHAD2_LCDV2_PID,
        ));
    }
    Ok(())
}

fn handshake() -> Result<()> {
    // Implemented in Task 11. For now, return an explanatory error so the
    // build still succeeds.
    Err(anyhow!("handshake subcommand: implemented in next task"))
}
```

- [ ] **Step 2: Build and check**

Run:

```bash
cargo build -p verify-galahad
```

Expected: success.

- [ ] **Step 3: Run `list` (this requires the device to be plugged in and the udev rule from the legacy scaffold to be present, OR running as root once)**

Run:

```bash
cargo run -q -p verify-galahad -- list
```

Expected on success (your real output will vary):

```
vid=0x0416 pid=0x7395  path="/dev/hidraw3"  manufacturer=Some("Winbond Electronics Corp.")  product=Some("LianLi-GA_II-LCD_v1.6")  serial=Some("...")  interface=0
```

If you get a permission error like `Permission denied (os error 13)` or
"hidapi error: ...":

```bash
sudo install -m 0644 \
  docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules \
  /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Then unplug and replug the device, and retry. (A proper, multi-device
udev rule file lands in Phase 1; we reuse the legacy single-device rule
for Phase 0.)

If `list` reports "no … found" but `lsusb` shows the device, something is
masking hidraw — investigate before continuing. Likely culprits: a kernel
module taking exclusive access (`hid-generic` should claim it; check with
`udevadm info /dev/hidrawN | grep -i driver`).

- [ ] **Step 4: Commit**

```bash
git add tools/verify-galahad/src/main.rs
git commit -m "feat(verify-galahad): implement device list subcommand

Enumerates hidapi devices matching Galahad II LCDV2 VID/PID and prints
hidraw path, manufacturer, product, serial, and interface number for
each match. Returns non-zero if none are found."
```

---

### Task 11: Implement `verify-galahad handshake` (the live test)

**Files:**
- Modify: `tools/verify-galahad/src/main.rs`

- [ ] **Step 1: Replace the stub `handshake` function**

Edit `tools/verify-galahad/src/main.rs` and replace the body of `fn handshake()` with:

```rust
fn handshake() -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::handshake::{
        build_request, HandshakeInfo,
    };
    use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    // Send the 64-byte request.
    let req = build_request();
    let written = device
        .write(&req)
        .context("HID write failed")?;
    println!("wrote {} bytes (expect 65 incl. report id, or 64 depending on backend)", written);

    // Read up to 64 bytes of response with a 1 s timeout.
    let mut buf = [0u8; PACKET_LEN];
    let read = device
        .read_timeout(&mut buf, 1000)
        .context("HID read_timeout failed")?;
    println!("read {} bytes", read);

    // Print the raw bytes for the protocol doc.
    print!("raw response:");
    for (i, b) in buf.iter().enumerate().take(read.max(13)) {
        if i % 16 == 0 {
            print!("\n  {:02}: ", i);
        }
        print!("{:02x} ", b);
    }
    println!();

    // Decode using the protocol crate.
    match HandshakeInfo::from_response(&buf) {
        Ok(info) => {
            println!();
            println!("decoded:");
            println!("  fan_rpm       = {}", info.fan_rpm);
            println!("  pump_rpm      = {}", info.pump_rpm);
            println!("  temp_valid    = {}", info.temp_valid);
            println!("  temp_integer  = {}", info.temp_integer);
            println!("  temp_fraction = {}", info.temp_fraction);
            println!("  temperature_c = {:.1}", info.temperature_c());
            Ok(())
        }
        Err(e) => Err(anyhow!("decode failed: {e}\nraw bytes printed above")),
    }
}
```

- [ ] **Step 2: Build**

Run:

```bash
cargo build -p verify-galahad
```

Expected: success.

- [ ] **Step 3: Run against the live device**

Make sure the device is plugged in. Run:

```bash
cargo run -q -p verify-galahad -- handshake
```

Expected output (numbers will differ; what matters is plausibility):

```
wrote 64 bytes (expect 65 incl. report id, or 64 depending on backend)
read 64 bytes
raw response:
  00: 01 81 00 00 00 07 04 d2 0b a4 01 22 03 00 00 00
  16: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
  ...

decoded:
  fan_rpm       = 1234
  pump_rpm      = 2980
  temp_valid    = true
  temp_integer  = 34
  temp_fraction = 3
  temperature_c = 34.3
```

Sanity checks (do these in your head):

- `fan_rpm` should be in the 600–3000 range under normal load. If it's 0,
  fans aren't connected / the AIO isn't getting power.
- `pump_rpm` should be in the 1500–3500 range when running. 0 ⇒ pump off.
- `temperature_c` should be the coolant temp — normally 25–55 °C. >70 ⇒
  something is wrong (or the sensor is reading garbage; check
  `temp_valid`).

If decode fails: copy the raw bytes from the output and add a comment to
the protocol doc — *that* is the artefact Phase 0 is meant to produce.

- [ ] **Step 4: Save the live capture**

Once handshake succeeds, paste the output (raw bytes + decoded values)
into a new file:

Create `docs/protocol/captures/2026-04-25-handshake-live.txt` with the
output you got, plus a header line like:

```
# Galahad II LCDV2 (0x0416:0x7395) firmware 1.6
# Captured: 2026-04-25 by verify-galahad handshake
# CachyOS, kernel <kernel version from `uname -r`>, hidapi 2.5
```

This file is committed.

- [ ] **Step 5: Commit code + capture**

```bash
git add tools/verify-galahad/src/main.rs \
        docs/protocol/captures/2026-04-25-handshake-live.txt
git commit -m "feat(verify-galahad): implement live handshake test

Sends GetHandshakeInfo (cmd 129) via the protocol crate's
build_request() and decodes the response with HandshakeInfo
::from_response. Prints both raw bytes and decoded fields.

Includes the first live capture against the user's actual device
under docs/protocol/captures/, which is what Phase 0 is designed
to produce."
```

---

## Part G — Wrap-up: the protocol doc + README

### Task 12: Write the initial `galahad2-lcdv2.md` protocol doc

The first authoritative English protocol doc. It supersedes
`PROTOCOL_DOCUMENTATION_RU.md` for the handshake command and grows in
Phase 0b.

**Files:**
- Create: `docs/protocol/galahad2-lcdv2.md`

- [ ] **Step 1: Write the doc**

Create `docs/protocol/galahad2-lcdv2.md` with this exact content:

```markdown
# Galahad II LCDV2 — Protocol

**Device:** Lian Li Galahad II LCDV2 ("Vision")
**USB ID:** `0x0416:0x7395` (Winbond / Lian Li, GA_II-LCD)
**Verified firmware:** `1.6`
**Decompiled-source map:** [decompiled-source-map.md](./decompiled-source-map.md)
**Status legend:** `verified` (live capture exists), `inferred` (only static
analysis), `unknown` (TBD).

This document supersedes `PROTOCOL_DOCUMENTATION_RU.md` for any command
listed below. The Russian doc remains as a legacy reference for commands
not yet covered here.

---

## Packet framing — `ACommandPacket` (64 bytes)

**Source:** `LConnectCore/Products/Galahad2Vision/ACommandPacket.cs:12-80`.
**Status:** `verified` for handshake (responses arrive with this layout).

```
offset  size  field         description
------  ----  -----         ---------------------------------------------
  0      1   ReportID       always 0x01
  1      1   Command        e.g. 0x81 = 129 = GetHandshakeInfo
  2      1   reserved       always 0x00 in current firmware
  3      2   PacketNumber   big-endian u16; 0 unless multi-packet upload
  5      1   Length         valid data bytes (0..=58)
  6     58   Data           command-specific payload, zero-padded
```

Constants:

- `PACKET_LEN = 64`
- `HEADER_LEN = 6`
- `MAX_PAYLOAD = 58`

---

## `GetHandshakeInfo` — Command 129 (`0x81`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-handshake-live.txt](./captures/2026-04-25-handshake-live.txt)).

**Sources:**
- Send: `Galahad2VisionDevice.cs:250-253` (`SendGetHandshakeInfo`)
- Receive (sync): `Galahad2VisionDevice.cs:107-119`
- Decode: `HandshakeInfo.cs:58-76`

### Request (host → device)

A bare `ACommandPacket` with `Command = 129` and no data:

```
01 81 00 00 00 00  00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00  00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00  00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00  00 00 00 00 00 00 00 00 00 00
```

### Response (device → host)

`ACommandPacket` with `Command = 129` echo and `Length = 7`. Data bytes:

| offset (data) | offset (raw) | size | field            | encoding              |
|---------------|--------------|------|------------------|-----------------------|
| 0             | 6            | 2    | `FanRpm`         | big-endian u16, RPM   |
| 2             | 8            | 2    | `PumpRpm`        | big-endian u16, RPM   |
| 4             | 10           | 1    | `TemperatureIsValid` | `0` or `1`        |
| 5             | 11           | 1    | `TemperatureInteger` | u8, °C             |
| 6             | 12           | 1    | `TemperatureFraction`| u8, **% 10 = tenths °C** |

Display temperature: `TemperatureInteger + (TemperatureFraction % 10) / 10`.

The C# `IsValid` field is set true unconditionally inside `TryParse` whenever
the buffer is ≥ 7 bytes; we model failure with `Result` instead.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/handshake.rs`
- `crates/protocol/tests/galahad2_lcdv2_handshake.rs`

---

## Commands not yet documented

The full enum lives in
[`commands.rs`](../../crates/protocol/src/galahad2_lcdv2/commands.rs).
Coverage in subsequent phases:

- **Phase 0b:** `GetFirmwareVersion` (134), `SetFanPWM` (139),
  `SetPumpPWM` (138), fan curve, lighting (130-133), `LCDControl` (12),
  `IsLCDAvailable` (23), `ResetDevice` (142), `SendJpeg` (14),
  `UploadJpeg` (15), `SendH264Frame` (13).
- **Phase 0b:** crypto investigation (whether `CryptoHelper.cs` wraps
  packets or only theme bundles).
```

- [ ] **Step 2: Verify the live-capture path actually exists**

Run:

```bash
ls docs/protocol/captures/2026-04-25-handshake-live.txt
```

Expected: the file (created in Task 11). If missing, the link in the doc
is broken — go fix that first.

- [ ] **Step 3: Commit**

```bash
git add docs/protocol/galahad2-lcdv2.md
git commit -m "docs(protocol): write initial galahad2-lcdv2.md

Covers ACommandPacket framing and GetHandshakeInfo (cmd 129),
both verified by the Phase-0a live capture. Other commands listed
as TBD with phase assignments. Supersedes PROTOCOL_DOCUMENTATION_RU.md
for these specific topics; the Russian doc remains the legacy
reference for everything else."
```

---

### Task 13: Update the top-level README

The README currently advertises a buggy single-binary GUI. Replace with
something that reflects the real Phase 0a state.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Read the current README to see what's there**

Run:

```bash
cat README.md
```

(For reference; the content was already inspected during planning.)

- [ ] **Step 2: Replace the README**

Overwrite `README.md` with:

```markdown
# L-Connect Linux

Linux-native client for Lian Li L-Connect 3 hardware. Currently in
**Phase 0a** — protocol verification only. No daemon, no GUI yet.

## Status

- ✅ Cargo workspace set up
- ✅ `lconnect-protocol` crate, with verified `ACommandPacket` framing
- ✅ Galahad II LCDV2 `GetHandshakeInfo` (cmd 129) verified on real hardware
- 🚧 All other commands — Phase 0b
- 🚧 Daemon, GUI, theme engine — Phases 1–6

See [`docs/superpowers/specs/2026-04-25-lconnect-linux-design.md`](docs/superpowers/specs/2026-04-25-lconnect-linux-design.md)
for the full design and [`docs/superpowers/plans/`](docs/superpowers/plans/)
for phase-by-phase implementation plans.

## Hardware coverage (current)

| Device | VID:PID | Status |
|---|---|---|
| Galahad II LCDV2 ("Vision") | `0416:7395` | Phase 0a in progress |
| Everything else from L-Connect 3 | various | future phases |

## Building (developers only — there's nothing to use yet)

```bash
# Prerequisites
sudo pacman -S rust pkg-config systemd  # CachyOS / Arch
# or:  apt install build-essential libudev-dev pkg-config

# Workspace build
cargo build --workspace

# Run protocol unit tests
cargo test -p lconnect-protocol

# Phase-0 verification harness against a live device
cargo run -p verify-galahad -- list
cargo run -p verify-galahad -- handshake
```

## udev permissions

For non-root access to the Galahad II LCDV2 hidraw node, install the rule
from `docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules`
(a proper multi-device rule lands in Phase 1):

```bash
sudo install -m 0644 \
  docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules \
  /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
# unplug and replug the device
```

## License

GPL-3.0-or-later.
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README for Phase 0a state

Removes claims about features that don't exist in the new layout
(GUI, fan curve editor, JPEG upload). Documents what is actually
buildable and runnable in Phase 0a: workspace, protocol crate,
handshake, and the verify-galahad harness."
```

---

## Phase 0a complete — exit criteria

- [ ] `cargo build --workspace` succeeds.
- [ ] `cargo test -p lconnect-protocol` reports `14 passed; 0 failed`.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `verify-galahad list` finds the device.
- [ ] `verify-galahad handshake` prints plausible RPMs and a temperature
      that matches what the Windows app reported, OR (if you didn't
      check Windows) is in a sane range.
- [ ] `docs/protocol/galahad2-lcdv2.md` exists with the handshake section
      filled in and links to a real capture file under
      `docs/protocol/captures/`.
- [ ] `git log --oneline` shows ≥10 commits, each with a `<type>:` prefix
      (`docs:`, `feat:`, `chore:`).

When all of these are checked, run:

```bash
cargo test --workspace 2>&1 | tail -5
cargo build --workspace --release 2>&1 | tail -5
```

…to confirm the release build also succeeds, then move on to writing the
**Phase 0b** plan, which covers all remaining commands plus the crypto
investigation.

---

## Post-execution notes

- **Final test count: 15** (plan baseline was 14). Commit `fd20ab4`
  (`test(protocol): cover ACommandPacket::parse_response truncated-data
  branch`) added one defensive test for the post-length-field bounds-check
  branch caught by the Task 7 code-quality review. The branch was correct
  but unexercised under the original suite.
- **Three doc artefacts shipped instead of two**: the planned
  `decompiled-source-map.md` and `galahad2-lcdv2.md`, plus
  `captures/2026-04-25-handshake-live.txt` (the live handshake byte-by-
  byte cross-check captured against the user's actual hardware on
  CachyOS 7.0.1, hidapi 2.5).
- **Workspace `bytes` dependency was dropped** post-completion as unused
  (commit landing alongside this note). Re-introduce when B-packet or
  zero-copy frame buffers need it in Phase 0b.
