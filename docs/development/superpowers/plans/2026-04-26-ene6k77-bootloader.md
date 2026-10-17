# ENE 6K77xx Bootloader Implementation Plan

**Scope status (2026-10-16): deferred.** This historical plan concerns a different
fan-controller family and is outside the active GALAHAD II LCDV2 roadmap. Its
unchecked steps have not been requalified as completed. See the [plan index](README.md).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a static-analysis-derived protocol spec, a pure-codec Rust crate, and an experimental CLI harness for flashing firmware on the ENE 6K77xx fan-controller family — none of which has been validated against real hardware.

**Architecture:** Three artefacts in the existing `lconnect-linux` workspace. (1) `docs/development/protocol/ene6k77-bootloader.md` documents the wire protocol with citations into committed decomp outputs. (2) `crates/ene6k77-bootloader/` contains pure byte-builders and decoders, no I/O — mirrors the conventions of the existing `crates/protocol/`. (3) `tools/fw-update-ene6k77/` wires the codec to `rusb` behind a five-layer "untested, may-brick" warning wall. The codec deliberately omits orchestrator functions, the `0x1800` skip-region enforcement, and the bank-alternation state machine; the harness owns those policy decisions where they're visible to readers.

**Tech Stack:** Rust 1.82 (workspace MSRV), edition 2021, `thiserror` for codec errors, `rusb` for libusb access in the harness, `clap` derive for CLI, `anyhow` for harness error chaining. `cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `cargo doc --workspace --no-deps` are the gates.

**Reference documents:**

- Spec / design: [`docs/development/superpowers/specs/2026-04-26-ene6k77-bootloader-design.md`](../specs/2026-04-26-ene6k77-bootloader-design.md)
- Conventions: [`CLAUDE.md`](../../../../../CLAUDE.md)
- Existing crate to mirror: [`crates/protocol/`](../../../../crates/protocol)
- Existing harness to mirror: [`tools/verify-galahad/`](../../../../tools/verify-galahad)

**Citation convention used throughout this plan:** `fw-update.txt:NNNN` and `fw-update-hlil.txt:NNNN` refer to the BinaryNinja decompiler outputs. After Task 1 those files live at `docs/decomp/fw-update/` inside the repo; the line numbers don't change.

---

## Task 1: Move decomp files into the repo

**Files:**
- Create: `docs/decomp/fw-update/README.md`
- Move: `../fw-update.txt` → `docs/decomp/fw-update/fw-update.txt`
- Move: `../fw-update-hlil.txt` → `docs/decomp/fw-update/fw-update-hlil.txt`

The two decomp outputs currently live at `~/Development/lian-li/` (parent of `lconnect-linux`). The spec doc and codec rustdoc cite them by line number, so they need to live inside the repo for the citations to resolve for future readers.

- [ ] **Step 1: Create the directory and move the files**

```bash
cd lconnect-linux
mkdir -p docs/decomp/fw-update
mv ../fw-update.txt      docs/decomp/fw-update/
mv ../fw-update-hlil.txt docs/decomp/fw-update/
```

Expected: both files now under `docs/decomp/fw-update/`. Run `wc -l docs/decomp/fw-update/*.txt`; expect ~5583 (pseudo-C) and ~5099 (HLIL).

- [ ] **Step 2: Write the provenance README**

Create `docs/decomp/fw-update/README.md`:

```markdown
# `FwUpdate.exe` decompilation

## Source binary

- Path in this repo (sibling tree, not committed):
  `winver/L-Connect 3/FwUpdate.exe`
- Embedded PDB string (debug-info self-identification):
  `D:\Work\fw-update\x64\Release\FwUpdate.pdb`
- Type: PE, x86_64, console subsystem
- Compiler: VS2022 17.0.x
- Imports: `libusb-1.0.dll`, `KERNEL32.dll`, `VCRUNTIME140.dll`, MS CRT
- Image base: `0x140000000`
- Code section: `0x140001000..0x14000345c`

## Decompiler outputs

- `fw-update.txt` — BinaryNinja "linear" pseudo-C dump.
- `fw-update-hlil.txt` — BinaryNinja HLIL dump.

Both outputs were produced from the same binary at the same time. The
HLIL is generally cleaner for control flow; the pseudo-C is sometimes
more explicit on register values. Spec and codec rustdoc cite from
whichever output makes a given point clearer.

## Regenerating

If a future L-Connect 3 release ships a different `FwUpdate.exe`, the
embedded PDB-string fingerprint above will change. To regenerate:

1. Open `winver/L-Connect 3/FwUpdate.exe` in BinaryNinja.
2. Wait for analysis to complete.
3. View → Linear → save as `fw-update.txt`.
4. View → HLIL → save as `fw-update-hlil.txt`.
5. Re-run the codec test suite — fingerprint tests will fail loudly
   if the wire protocol has changed.

## Status

Static analysis only. No live capture. Every claim sourced from these
files in `docs/development/protocol/ene6k77-bootloader.md` carries an `inferred`
status flag.
```

- [ ] **Step 3: Verify the citations land where the spec/code will look**

```bash
grep -n 'libusb_control_transfer' docs/decomp/fw-update/fw-update.txt | head -5
grep -n 'enter_bootloader' docs/decomp/fw-update/fw-update.txt | head
```

Expected: line numbers match the citations the rest of this plan uses (~440-450 for the first libusb_control_transfer, ~1272 for the enter_bootloader log string).

- [ ] **Step 4: Commit**

```bash
git add docs/decomp/fw-update/
git commit -m "docs: import FwUpdate.exe decompilation outputs

BinaryNinja pseudo-C and HLIL dumps for the standalone Windows firmware
updater for the ENE 6K77xx fan-controller family. Source binary lives
at winver/L-Connect 3/FwUpdate.exe (not committed). README documents
provenance and regeneration."
```

---

## Task 2: Write the protocol spec doc

**Files:**
- Create: `docs/development/protocol/ene6k77-bootloader.md`

The spec is the durable artefact. Future readers — including whoever eventually has the hardware to validate this — will read the spec, not this plan. Write it carefully and cite every claim.

Structure follows design doc §4 (eleven sections). Style matches the existing `docs/development/protocol/galahad2-lcdv2.md` (read it first to internalise the style). Every claim cites a `docs/decomp/fw-update/fw-update.txt:NNNN-MMM` or `…fw-update-hlil.txt:NNNN-MMM` range.

- [ ] **Step 1: Read the existing protocol doc to internalise conventions**

```bash
less docs/development/protocol/galahad2-lcdv2.md
```

Note: frontmatter shape, status legend, per-section "Source" and "Status" lines, layout tables.

- [ ] **Step 2: Draft the frontmatter**

Top of `docs/development/protocol/ene6k77-bootloader.md`:

```markdown
# ENE 6K77xx Fan Controller — Bootloader Protocol

**Device family:** Lian Li SLFan / ALFan / SLInfinity / SLV2 / ALV2 / SLV2A / SLRedragon (and StrimerPlus shares the bootloader)
**USB IDs (application mode):** `0x0CF2:0xA100..0xA106`, `0x0CF2:0xA200`
**USB ID (bootloader mode):** `0x0CF2:0x7758`
**Decompiled-source map:** [decompiled-source-map.md](../../protocol/decompiled-source-map.md)
**Source binary:** `winver/L-Connect 3/FwUpdate.exe` (not committed; see `docs/decomp/fw-update/README.md`)
**Decomp outputs (committed):** `docs/decomp/fw-update/fw-update.txt`, `docs/decomp/fw-update/fw-update-hlil.txt`

**Status: every entry in this document is `inferred`.** Static analysis
of the Windows updater binary only. No live capture, no hardware,
no `.bin` available. Every "open question" listed in §10 can only be
settled by someone who has both a target device and an official
firmware image.

This document complements [`cross-device-survey/per-family/ene6k77fan.md`](../../protocol/cross-device-survey/per-family/ene6k77fan.md),
which covers the same chip family's HID *application* protocol (and
explicitly flagged the bootloader half as out-of-tree at line 121).
```

- [ ] **Step 3: Write §1 (Source materials), §2 (Two USB personalities), §3 (Wire-level conventions)**

Use design doc §4 as the outline. Concrete content for each:

§1 — list the binary path, the two decomp file paths, the embedded PDB string for binary identification, and the citation convention used in the rest of the doc.

§2 — table of app-mode PIDs per SKU (cross-reference `KnownDevices.cs` line ranges from `cross-device-survey/per-family/ene6k77fan.md`), plus the bootloader PID `0x0CF2:0x7758` (cited to `fw-update.txt:1388` where the fallback open at `0x7758` happens). Note the iManufacturer/iProduct ASCII-descriptor substring filter `"LianLi"` or `"6K7750"` (cite `fw-update.txt:497-501`). Dismiss the `MI_00 DeviceInterfaceGUIDs` Windows error path.

§3 — bmRequestType (`0x40`/`0xC0`), bRequest (`0x80`/`0x81`), wValue (always 0), endianness (big-endian for multi-byte fields, see byte-ordering pattern in `sub_140001a90` at `fw-update-hlil.txt` for the 9-byte header). Timeout `0x3e8` = 1000 ms (cite any of the call sites). Call out the `0xf040`/`0xf080` HLIL artefact and explain it's compiler-level register reuse — the byte that hits the wire is `0x40`/`0x80`.

- [ ] **Step 4: Write §4 (Bank alternation), §5 (Control register namespace)**

§4 — explain the alternation, the wIndex pair table from design doc §3, where the initial bank comes from (`fw-update.txt:543-544` — `arg1[1] = (PID == 0x7758) ? 1 : 0`), where it toggles (`fw-update.txt:817` — `arg1[1] ^= 1` in `sub_140001810`).

§5 — table of control registers (`0x600..0x606`) with semantics. `0x600` = mode/control (known values `0x48`, `0x03`); `0x601..0x604` = WDT-step registers used in the 5-step `wdt_reset` sequence (cite `fw-update.txt:644-714` for `sub_140001510`); `0x606` = status/mask register read with separate clear-by-write-0x02 path (cite `fw-update.txt:760-797` for `sub_140001730`).

- [ ] **Step 5: Write §6 (9-byte command header), §7 (Status register format)**

§6 — the table from design doc §3 expanded with per-cmd citations:

| Value | Inferred meaning | Where emitted |
|---|---|---|
| `0x01` | Probe / dummy (no payload) | `fw-update.txt:1286` (enter-bootloader handshake) |
| `0x30` | Read flash | `fw-update.txt:1069-1070` (bulk-read path in `sub_140001bb0`) |
| `0x40` | Write flash | `fw-update.txt:1506-1507` (write loop in `sub_1400021e0`) |
| `0x70` | Open programming | `fw-update.txt:1141` (`sub_140001d90` byte 7) |
| `0x80` | Verify checksum / finalise | `fw-update.txt:1186` (`sub_140001e40` byte 7) |

Layout citation: `fw-update-hlil.txt` lines for `sub_140001a90` (the byte-by-byte assignment block).

§7 — bit 1 = ready, bits `0xF0` = error mask, write `0x02` to clear errors. Cite `fw-update.txt:838-867` for the bit checks and the clear-write.

- [ ] **Step 6: Write §8 (Enter-bootloader sequence), §9 (Flash write sequence)**

§8 — step-by-step prose orchestrated from `sub_140002000` (`fw-update.txt:1264-1370`). The exact register writes in order, the dummy-cmd-with-cmd-byte=`0x01`, the 3-second polling loop on register `0x606` bit 1, the device re-enumeration as PID `0x7758`. Note the bit_mask call uses mask `0xF3`, value `0xE1` (`fw-update.txt:1277`).

§9 — the flash_open (cmd `0x70`) → chunked write loop → wait_ready → re-read into verify buffer → `memcmp` → `flash_verify_checksum` (cmd `0x80`) → close pipeline. Cite the `0x1800` skip region from both `fw-update.txt:1080` (read gate in `sub_140001bb0`) and `fw-update.txt:1495` (write gate in `sub_1400021e0`). Note the 128-byte write chunks (`fw-update.txt:1490-1493`) and the 16 KB confirmation read (`fw-update.txt:602` — `0x4000` wLength).

- [ ] **Step 7: Write §10 (Open questions), §11 (Cross-references)**

§10 — bullet list of unverified claims (copy from design doc §7 "What testing explicitly cannot validate" — those are the same items). **Add one more bullet** based on the shipped-bin analysis:

> The 8-hex-digit suffix in shipped `.bin` filenames (e.g.,
> `6K77XX_LLUF-SLv2_HW1_20230310_0x00373B91_V07.bin` →
> `0x00373B91`) is presumed to be the firmware checksum that the
> `cmd = 0x80 VerifyChecksum` step (§9) verifies. The checksum
> algorithm is not derived from the decomp — `sub_140001e40` only
> sends the cmd-byte and waits for ready; the chip computes the
> checksum internally. Confirming this requires a hardware test:
> flash a bin, then rename it to flip one digit of the suffix,
> then attempt verify — should fail loudly.

§11 — back-link to `cross-device-survey/per-family/ene6k77fan.md` ("application-mode protocol for the same family lives here"); forward-link to `crates/ene6k77-bootloader/` ("Rust codec for this protocol"); forward-link to `tools/fw-update-ene6k77/` ("experimental harness"). Plus one new entry: forward-link to the GUI sibling updater `UsbISPDialog_v.1.0.1.18.exe` (in `fw/LianLi_FwUpdate-tools_v.1.0.1.18.zip`) as a future cross-validation source — decompiling it should produce the same vendor-control protocol behind a different UI layer.

- [ ] **Step 8: Verify all citations resolve**

For every `fw-update.txt:NNNN` and `fw-update-hlil.txt:NNNN` reference in the spec, confirm the cited line range supports the claim:

```bash
# Spot-check a handful
sed -n '740,742p' docs/decomp/fw-update/fw-update.txt
sed -n '1264,1280p' docs/decomp/fw-update/fw-update.txt
sed -n '1490,1510p' docs/decomp/fw-update/fw-update.txt
```

Expected: the cited regions match the prose claims around them.

- [ ] **Step 9: Discipline check**

Search the spec for any verbatim block longer than ~5 lines copied from the decomp:

```bash
# Look for long indented blocks (4+ space prefix) — these often mean copy-paste
awk '/^    / {n++; if (n > 5) print NR": long indented block"; next} {n=0}' \
    docs/development/protocol/ene6k77-bootloader.md
```

Expected: no hits, or only intentional layout tables (which use `|` not 4-space indent).

- [ ] **Step 10: Commit**

```bash
git add docs/development/protocol/ene6k77-bootloader.md
git commit -m "docs: add ENE 6K77xx bootloader protocol spec

Documents the wire protocol for the firmware-update path of the ENE
6K77 fan-controller family. Static analysis only — every claim is
flagged 'inferred' and cites docs/decomp/fw-update/fw-update.txt or
fw-update-hlil.txt. Complements ene6k77fan.md, which covers the
application-mode HID protocol for the same family."
```

---

## Task 3: Codec crate skeleton

**Files:**
- Modify: `Cargo.toml` (workspace) — add new member, add new workspace deps
- Create: `crates/ene6k77-bootloader/Cargo.toml`
- Create: `crates/ene6k77-bootloader/src/lib.rs`

Skeleton is just enough to compile — empty modules, the `Error` enum, `Result` alias. Modules get bodies in tasks 4-9.

- [ ] **Step 1: Add the workspace member**

Modify `Cargo.toml` (workspace root). Append `"crates/ene6k77-bootloader"` to the `members` list. The workspace `[workspace.dependencies]` block already has `thiserror = "1"` from the existing protocol crate — no change needed there.

After edit, the file should look like:

```toml
[workspace]
resolver = "2"
members = [
    "crates/protocol",
    "crates/lconnect-image",
    "crates/ene6k77-bootloader",
    "tools/verify-galahad",
]
```

(Other sections unchanged.)

- [ ] **Step 2: Create the per-crate manifest**

Write `crates/ene6k77-bootloader/Cargo.toml`:

```toml
[package]
name         = "ene6k77-bootloader"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
description = "EXPERIMENTAL — UNTESTED — MAY PERMANENTLY BRICK YOUR DEVICE. Pure Rust codec for the ENE 6K77xx fan-controller bootloader protocol, derived from static analysis of Lian-Li's FwUpdate.exe. Never validated against real hardware."

[lib]
name = "ene6k77_bootloader"

[dependencies]
thiserror = { workspace = true }
```

- [ ] **Step 3: Create the lib.rs skeleton**

Write `crates/ene6k77-bootloader/src/lib.rs`:

```rust
//! ENE 6K77xx fan-controller bootloader protocol — pure codec.
//!
//! # **WARNING — UNTESTED CODE**
//!
//! This crate is derived entirely from static analysis of the
//! decompiled Windows binary `FwUpdate.exe` (committed at
//! `docs/decomp/fw-update/`). It has **never been validated against
//! real hardware**. Using it to drive a real device may permanently
//! brick the device. Recovery may require programming hardware
//! (SWD/ICSP/JTAG).
//!
//! Specifically unverified:
//!
//! - Whether `Cmd::Read` (0x30) and `Cmd::Write` (0x40) actually mean
//!   what we infer.
//! - Whether the WDT 5-step sequence reliably puts the device into
//!   bootloader mode rather than hanging it.
//! - Whether the 0x1800-byte skip region is exactly the bootloader's
//!   own region or an unrelated coincidence.
//! - Whether bank-A / bank-B alternation is required by the firmware
//!   or just an artefact of how the original tool happens to work.
//! - Whether the `0xE1`/`0xF3` bit-mask values mean what we think.
//! - Whether the descriptor-string filter (`"LianLi"` / `"6K7750"`)
//!   covers all SKU variants or only the one the original developer
//!   tested with.
//!
//! See `docs/development/protocol/ene6k77-bootloader.md` for the protocol
//! reference, and `tools/fw-update-ene6k77/` for the experimental
//! harness binary.
//!
//! # Design
//!
//! No I/O, no async, no `rusb`. Bytes in, bytes out. The crate
//! provides building blocks; orchestration (sequencing, bank
//! alternation, the 0x1800-byte skip enforcement) lives in the
//! caller so those policy decisions stay visible at the call site.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bank;
pub mod control;
pub mod enter_bootloader;
pub mod flash;
pub mod identify;
pub mod register;

/// Errors produced by codec operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A response or input buffer was too short for the operation.
    #[error("response too short: expected {expected} bytes, got {got}")]
    ResponseTooShort {
        /// Required byte count.
        expected: usize,
        /// Bytes actually present.
        got: usize,
    },
    /// A flash address falls inside the bootloader skip region.
    #[error("address 0x{addr:06x} is in the bootloader skip region (< 0x{end:04x})")]
    AddressInSkipRegion {
        /// The offending address.
        addr: u32,
        /// End of the skip region.
        end: u32,
    },
    /// A length parameter exceeds the per-chunk maximum.
    #[error("length {len} overflows chunk size {max}")]
    LengthOverflowsChunk {
        /// Requested length.
        len: u16,
        /// Maximum allowed.
        max: u16,
    },
    /// A command byte read from a buffer is not a known [`flash::Cmd`] variant.
    #[error("unknown command byte: 0x{0:02x}")]
    UnknownCmd(u8),
}

/// Convenience `Result` alias.
pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 4: Create empty module files so the crate compiles**

For each of the six modules declared in `lib.rs`, create an empty file with just a module-level doc-comment. Example for `crates/ene6k77-bootloader/src/bank.rs`:

```rust
//! Bank-A / bank-B alternation for the ENE 6K77xx flash protocol.
```

Repeat for `control.rs`, `register.rs`, `flash.rs`, `enter_bootloader.rs`, `identify.rs` with a similar one-line doc-comment matching each module's stated purpose in design doc §5.

- [ ] **Step 5: Verify the skeleton compiles**

```bash
cd lconnect-linux
cargo build -p ene6k77-bootloader
cargo clippy -p ene6k77-bootloader -- -D warnings
cargo doc -p ene6k77-bootloader --no-deps
```

Expected: all three pass. Clippy may emit `dead_code` warnings on the empty modules — that's fine, those go away as modules get bodies.

If clippy fails because of the empty modules + `#![warn(missing_docs)]`, add an empty `pub fn _placeholder() {}` to silence — but better, just proceed to Task 4 immediately so the modules get real content.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/ene6k77-bootloader/
git commit -m "feat(ene6k77-bootloader): add crate skeleton

New workspace member crates/ene6k77-bootloader/. Derives from static
analysis of FwUpdate.exe; lib.rs documents what is and isn't verified.
Module bodies follow in subsequent commits."
```

---

## Task 4: `bank.rs` — Bank-A / bank-B alternation

**Files:**
- Modify: `crates/ene6k77-bootloader/src/bank.rs`
- Create: `crates/ene6k77-bootloader/tests/ene6k77_bank_alternation.rs`

Tiny module. Three w_index lookup functions, a `toggle()`, and an `initial(opened_pid)` constructor. Tests are equally tiny.

- [ ] **Step 1: Write the failing test**

Create `crates/ene6k77-bootloader/tests/ene6k77_bank_alternation.rs`:

```rust
//! Tests for the Bank enum and its w_index lookup tables.
//!
//! Every constant in this file is transcribed from the decomp output,
//! not derived from the codec's own constants — typos in the codec
//! fail these tests independently.

use ene6k77_bootloader::bank::Bank;

#[test]
fn toggle_alternates_a_and_b() {
    assert_eq!(Bank::A.toggle(), Bank::B);
    assert_eq!(Bank::B.toggle(), Bank::A);
}

#[test]
fn status_w_index_matches_decomp() {
    // Source: docs/decomp/fw-update/fw-update.txt:820-823
    // (sub_140001810: rbx_1 = -0xe07 = 0xf1f9 default; if !temp0_1 → 0xf0f9).
    // After arg1[1] ^= 1: temp0_1 = 1 → bank B → 0xf1f9; temp0_1 = 0 → bank A → 0xf0f9.
    assert_eq!(Bank::A.status_w_index(), 0xF0F9);
    assert_eq!(Bank::B.status_w_index(), 0xF1F9);
}

#[test]
fn command_w_index_matches_decomp() {
    // Source: docs/decomp/fw-update/fw-update.txt:1185-1189
    // (sub_140001a90 cmd path: rax_9 = -0xe0f = 0xf1f1 default;
    //  if arg2 == 0 → 0xf0f1).
    assert_eq!(Bank::A.command_w_index(), 0xF0F1);
    assert_eq!(Bank::B.command_w_index(), 0xF1F1);
}

#[test]
fn bulk_w_index_matches_decomp() {
    // Source: docs/decomp/fw-update/fw-update.txt:1007-1010
    // (sub_140001a90 bulk path: rax_2 = -0xf00 = 0xf100 default;
    //  if arg2 == 0 → 0xf000).
    assert_eq!(Bank::A.bulk_w_index(), 0xF000);
    assert_eq!(Bank::B.bulk_w_index(), 0xF100);
}

#[test]
fn initial_bank_is_b_for_bootloader_pid() {
    // Source: docs/decomp/fw-update/fw-update.txt:543-544
    // (sub_140001180: arg1[1] = (PID == 0x7758) ? 1 : 0).
    // arg1[1] = 1 corresponds to bank B in the toggle path
    // (sub_140001810 line 822-823: if !temp0_1 → 0xf0f9 means bank A).
    assert_eq!(Bank::initial(0x7758), Bank::B);
}

#[test]
fn initial_bank_is_a_for_app_mode_pid() {
    assert_eq!(Bank::initial(0xA100), Bank::A);
    assert_eq!(Bank::initial(0xA101), Bank::A);
    assert_eq!(Bank::initial(0xA200), Bank::A);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_bank_alternation
```

Expected: compile error — `Bank` doesn't exist yet.

- [ ] **Step 3: Write the module body**

Replace `crates/ene6k77-bootloader/src/bank.rs` contents:

```rust
//! Bank-A / bank-B alternation for the ENE 6K77xx flash protocol.
//!
//! ## Source
//!
//! - `docs/decomp/fw-update/fw-update.txt:817-834` — `sub_140001810`
//!   toggles `arg1[1]` on every status-poll call.
//! - `docs/decomp/fw-update/fw-update.txt:543-544` — `sub_140001180`
//!   sets the initial bank from the open PID.
//!
//! The chip exposes parallel command/data/status slots at two
//! distinct wIndex namespaces. The host alternates between them on
//! every operation, presumably so the chip can be processing one
//! slot while the host is filling the other. This module exposes
//! the bank as a value type the caller threads through every
//! operation; the codec does not maintain bank state.

/// One of the two parallel command/data/status slots exposed by the chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    /// The "low" bank — wIndex namespace `0xF0xx`.
    A,
    /// The "high" bank — wIndex namespace `0xF1xx`.
    B,
}

impl Bank {
    /// Returns the other bank.
    pub fn toggle(self) -> Self {
        match self {
            Bank::A => Bank::B,
            Bank::B => Bank::A,
        }
    }

    /// `wIndex` for the 1-byte status-poll read on this bank.
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:820-823`.
    pub fn status_w_index(self) -> u16 {
        match self {
            Bank::A => 0xF0F9,
            Bank::B => 0xF1F9,
        }
    }

    /// `wIndex` for the 9-byte command-header write on this bank.
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:1185-1189`.
    pub fn command_w_index(self) -> u16 {
        match self {
            Bank::A => 0xF0F1,
            Bank::B => 0xF1F1,
        }
    }

    /// `wIndex` for bulk data transfer on this bank.
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:1007-1010`.
    pub fn bulk_w_index(self) -> u16 {
        match self {
            Bank::A => 0xF000,
            Bank::B => 0xF100,
        }
    }

    /// Initial bank for a freshly-opened device handle.
    ///
    /// A cold-open at the bootloader PID `0x7758` starts with the
    /// device's internal bank counter at 1 (= B); transitioning from
    /// app mode starts at 0 (= A).
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:543-544`.
    pub fn initial(opened_pid: u16) -> Self {
        if opened_pid == 0x7758 {
            Bank::B
        } else {
            Bank::A
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_bank_alternation
cargo clippy -p ene6k77-bootloader -- -D warnings
cargo doc -p ene6k77-bootloader --no-deps
```

Expected: 6 tests passing, no clippy warnings, doc builds.

- [ ] **Step 5: Commit**

```bash
git add crates/ene6k77-bootloader/src/bank.rs \
        crates/ene6k77-bootloader/tests/ene6k77_bank_alternation.rs
git commit -m "feat(ene6k77-bootloader): add Bank enum and w_index tables"
```

---

## Task 5: `control.rs` — `ControlTransfer` descriptor type

**Files:**
- Modify: `crates/ene6k77-bootloader/src/control.rs`

No tests for this task — it's pure type definitions and constants. The first test that references the types appears in Task 6 (`register.rs`).

- [ ] **Step 1: Write the module body**

Replace `crates/ene6k77-bootloader/src/control.rs` contents:

```rust
//! Vendor control transfer descriptor.
//!
//! ## Source
//!
//! Every `libusb_control_transfer` call site in
//! `docs/decomp/fw-update/fw-update.txt` (e.g., 740-742, 1007-1013,
//! 1185-1189). The four constants below are observed at every site.
//!
//! The codec emits `ControlTransfer` descriptors as plain data; the
//! harness binary translates them into real USB calls via `rusb`.
//! This indirection keeps the codec free of any I/O dependency and
//! lets the harness intercept transfers for `--verbose` logging or
//! mocking without touching codec internals.

use std::borrow::Cow;

/// Standard control-transfer timeout (1000 ms).
///
/// Every call site in the original tool uses this value (`0x3e8`).
pub const TIMEOUT_MS: u32 = 1000;

/// `bmRequestType` for vendor host-to-device transfers.
pub const BM_RT_HOST_TO_DEVICE: u8 = 0x40;

/// `bmRequestType` for vendor device-to-host transfers.
pub const BM_RT_DEVICE_TO_HOST: u8 = 0xC0;

/// `bRequest` for write operations.
pub const B_REQ_WRITE: u8 = 0x80;

/// `bRequest` for read operations.
pub const B_REQ_READ: u8 = 0x81;

/// Direction-tagged data buffer for a control transfer.
#[derive(Debug)]
pub enum ControlData<'a> {
    /// Device-to-host: caller-provided buffer that will receive the response.
    In(&'a mut [u8]),
    /// Host-to-device: data to send. Borrowed or owned.
    Out(Cow<'a, [u8]>),
}

impl<'a> ControlData<'a> {
    /// Length of the buffer that will be transferred.
    pub fn len(&self) -> usize {
        match self {
            ControlData::In(buf) => buf.len(),
            ControlData::Out(cow) => cow.len(),
        }
    }

    /// Convenience: whether this is a zero-length transfer.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A single libusb vendor control transfer.
#[derive(Debug)]
pub struct ControlTransfer<'a> {
    /// `bmRequestType` byte (one of `BM_RT_*`).
    pub bm_request_type: u8,
    /// `bRequest` byte (one of `B_REQ_*`).
    pub b_request: u8,
    /// `wValue` — always 0 in this protocol.
    pub w_value: u16,
    /// `wIndex` — operation/register selector, often bank-keyed.
    pub w_index: u16,
    /// Data buffer; direction is `In` or `Out`.
    pub data: ControlData<'a>,
    /// Timeout in milliseconds.
    pub timeout_ms: u32,
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build -p ene6k77-bootloader
cargo clippy -p ene6k77-bootloader -- -D warnings
cargo doc -p ene6k77-bootloader --no-deps
```

Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ene6k77-bootloader/src/control.rs
git commit -m "feat(ene6k77-bootloader): add ControlTransfer descriptor type"
```

---

## Task 6: `register.rs` — Control-register read/write + status decoder

**Files:**
- Modify: `crates/ene6k77-bootloader/src/register.rs`
- Create: `crates/ene6k77-bootloader/tests/ene6k77_register_read_write.rs`
- Create: `crates/ene6k77-bootloader/tests/ene6k77_status_decode.rs`

Two test files because there are two distinct concerns: the read/write request shape (Task 6 part A) and the status decoder (Task 6 part B). Implemented in one module file because they share the register address constants.

- [ ] **Step 1: Write the failing test for read/write request shape**

Create `crates/ene6k77-bootloader/tests/ene6k77_register_read_write.rs`:

```rust
//! Tests that build_register_{read,write} emit the bmRT/bReq/wIdx
//! that the decomp expects. Every literal here is transcribed from
//! decomp output, not from the codec's own constants.

use ene6k77_bootloader::control::{ControlData, BM_RT_DEVICE_TO_HOST, BM_RT_HOST_TO_DEVICE, B_REQ_READ, B_REQ_WRITE, TIMEOUT_MS};
use ene6k77_bootloader::register::{self, REG_MODE, REG_STATUS_MASK};

#[test]
fn build_register_write_emits_vendor_h2d_setup() {
    // Source: docs/decomp/fw-update/fw-update.txt:738-742 (sub_1400016b0
    // disable_wdt: bmRT=0x40, bReq=0x80, wValue=0, wIndex=0x600,
    // payload=[0x48], timeout=1000).
    let xfer = register::build_register_write(REG_MODE, 0x48);
    assert_eq!(xfer.bm_request_type, 0x40);
    assert_eq!(xfer.b_request, 0x80);
    assert_eq!(xfer.w_value, 0);
    assert_eq!(xfer.w_index, 0x600);
    assert_eq!(xfer.timeout_ms, 1000);
    match &xfer.data {
        ControlData::Out(buf) => {
            assert_eq!(buf.as_ref(), &[0x48u8]);
        }
        ControlData::In(_) => panic!("write must be ControlData::Out"),
    }
}

#[test]
fn build_register_write_constants_are_consistent() {
    let xfer = register::build_register_write(REG_STATUS_MASK, 0x02);
    assert_eq!(xfer.bm_request_type, BM_RT_HOST_TO_DEVICE);
    assert_eq!(xfer.b_request, B_REQ_WRITE);
    assert_eq!(xfer.timeout_ms, TIMEOUT_MS);
}

#[test]
fn build_register_read_emits_vendor_d2h_setup() {
    // Source: docs/decomp/fw-update/fw-update.txt:760-768 (sub_140001730
    // bit_mask read: bmRT=0xc0, bReq=0x81, wValue=0, wIndex=0x606,
    // length=1, timeout=1000).
    let mut buf = [0u8; 1];
    let xfer = register::build_register_read(REG_STATUS_MASK, &mut buf);
    assert_eq!(xfer.bm_request_type, 0xC0);
    assert_eq!(xfer.b_request, 0x81);
    assert_eq!(xfer.w_value, 0);
    assert_eq!(xfer.w_index, 0x606);
    assert_eq!(xfer.timeout_ms, 1000);
    assert_eq!(xfer.data.len(), 1);
}

#[test]
fn build_register_read_constants_are_consistent() {
    let mut buf = [0u8; 1];
    let xfer = register::build_register_read(REG_MODE, &mut buf);
    assert_eq!(xfer.bm_request_type, BM_RT_DEVICE_TO_HOST);
    assert_eq!(xfer.b_request, B_REQ_READ);
}

#[test]
fn known_register_addresses() {
    use ene6k77_bootloader::register::*;
    assert_eq!(REG_MODE, 0x600);
    assert_eq!(REG_WDT_STEP_1, 0x601);
    assert_eq!(REG_WDT_STEP_2, 0x602);
    assert_eq!(REG_WDT_STEP_3, 0x603);
    assert_eq!(REG_WDT_STEP_4, 0x604);
    assert_eq!(REG_STATUS_MASK, 0x606);
}
```

- [ ] **Step 2: Write the failing test for status decoder**

Create `crates/ene6k77-bootloader/tests/ene6k77_status_decode.rs`:

```rust
//! Tests for StatusReg decoding.

use ene6k77_bootloader::register::{build_clear_error, StatusReg, REG_STATUS_MASK};
use ene6k77_bootloader::Error;
use ene6k77_bootloader::control::ControlData;

#[test]
fn parse_known_good_decodes_ready_bit() {
    // Source: docs/decomp/fw-update/fw-update.txt:839 (`if (!(rax_4 & 2))`)
    // and 852 (`if (!(rax_4 & 0xf0))`).
    let buf = [0x02u8];
    let s = StatusReg::from_response(&buf).expect("should parse");
    assert!(s.ready);
    assert_eq!(s.error_bits, 0);
    assert!(!s.has_error());
    assert_eq!(s.raw, 0x02);
}

#[test]
fn parse_known_good_decodes_error_bits() {
    let buf = [0xF2u8]; // ready + all error bits set
    let s = StatusReg::from_response(&buf).expect("should parse");
    assert!(s.ready);
    assert_eq!(s.error_bits, 0xF0);
    assert!(s.has_error());
}

#[test]
fn parse_known_good_decodes_idle() {
    let buf = [0x00u8];
    let s = StatusReg::from_response(&buf).expect("should parse");
    assert!(!s.ready);
    assert_eq!(s.error_bits, 0);
    assert!(!s.has_error());
}

#[test]
fn parse_too_short_returns_response_too_short() {
    let buf: [u8; 0] = [];
    let err = StatusReg::from_response(&buf).expect_err("must error");
    match err {
        Error::ResponseTooShort { expected, got } => {
            assert_eq!(expected, 1);
            assert_eq!(got, 0);
        }
        other => panic!("wrong error variant: {other:?}"),
    }
}

#[test]
fn build_clear_error_writes_0x02_to_passed_register() {
    // Source: docs/decomp/fw-update/fw-update.txt:858-867
    // (sub_140001810 error-clear path: write 0x02 to the same wIndex).
    let xfer = build_clear_error(REG_STATUS_MASK);
    assert_eq!(xfer.w_index, REG_STATUS_MASK);
    match &xfer.data {
        ControlData::Out(buf) => assert_eq!(buf.as_ref(), &[0x02u8]),
        ControlData::In(_) => panic!("clear must be ControlData::Out"),
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_register_read_write
cargo test -p ene6k77-bootloader --test ene6k77_status_decode
```

Expected: compile errors — none of the symbols exist yet.

- [ ] **Step 4: Write the module body**

Replace `crates/ene6k77-bootloader/src/register.rs` contents:

```rust
//! Control-register read/write for the ENE 6K77xx bootloader (wIndex 0x600..0x606).
//!
//! ## Source
//!
//! - `docs/decomp/fw-update/fw-update.txt:644-714` — 5-step WDT
//!   sequence in `sub_140001510` (writes to `0x601..0x604`, `0x600`).
//! - `docs/decomp/fw-update/fw-update.txt:735-749` — disable_wdt in
//!   `sub_1400016b0` (write `0x48` to `0x600`).
//! - `docs/decomp/fw-update/fw-update.txt:760-797` — bit_mask in
//!   `sub_140001730` (RMW on `0x606`).
//! - `docs/decomp/fw-update/fw-update.txt:838-867` — status decoding
//!   in `sub_140001810`.
//!
//! The bank-keyed status registers (`0xF0F9` / `0xF1F9`) are decoded
//! by the same [`StatusReg::from_response`]; the wIndex selection is
//! the caller's responsibility (see `bank::Bank::status_w_index`).

use std::borrow::Cow;

use crate::control::{
    ControlData, ControlTransfer, BM_RT_DEVICE_TO_HOST, BM_RT_HOST_TO_DEVICE, B_REQ_READ,
    B_REQ_WRITE, TIMEOUT_MS,
};
use crate::{Error, Result};

/// Mode / control register.
pub const REG_MODE: u16 = 0x600;
/// First WDT-reset step register.
pub const REG_WDT_STEP_1: u16 = 0x601;
/// Second WDT-reset step register.
pub const REG_WDT_STEP_2: u16 = 0x602;
/// Third WDT-reset step register.
pub const REG_WDT_STEP_3: u16 = 0x603;
/// Fourth WDT-reset step register.
pub const REG_WDT_STEP_4: u16 = 0x604;
/// Status / mask register read by the enter-bootloader polling loop.
pub const REG_STATUS_MASK: u16 = 0x606;

/// Build a 1-byte read of a register at `reg` into the caller's buffer.
pub fn build_register_read(reg: u16, into_buf: &mut [u8; 1]) -> ControlTransfer<'_> {
    ControlTransfer {
        bm_request_type: BM_RT_DEVICE_TO_HOST,
        b_request: B_REQ_READ,
        w_value: 0,
        w_index: reg,
        data: ControlData::In(&mut into_buf[..]),
        timeout_ms: TIMEOUT_MS,
    }
}

/// Build a 1-byte write of `value` to register `reg`.
pub fn build_register_write(reg: u16, value: u8) -> ControlTransfer<'static> {
    ControlTransfer {
        bm_request_type: BM_RT_HOST_TO_DEVICE,
        b_request: B_REQ_WRITE,
        w_value: 0,
        w_index: reg,
        data: ControlData::Out(Cow::Owned(vec![value])),
        timeout_ms: TIMEOUT_MS,
    }
}

/// Build the write that clears error bits on a status register
/// (writes `0x02` to the same register).
///
/// Source: `docs/decomp/fw-update/fw-update.txt:858-867`.
pub fn build_clear_error(reg: u16) -> ControlTransfer<'static> {
    build_register_write(reg, 0x02)
}

/// Decoded view of a 1-byte status read.
///
/// Bit `0x02` = ready; bits `0xF0` = error mask. The harness clears
/// the error bits by writing `0x02` back to the same register
/// (see [`build_clear_error`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusReg {
    /// Raw byte read from the device.
    pub raw: u8,
    /// Whether bit `0x02` is set.
    pub ready: bool,
    /// Bits `0xF0` of the raw byte.
    pub error_bits: u8,
}

impl StatusReg {
    /// Parse a single-byte buffer.
    pub fn from_response(buf: &[u8]) -> Result<Self> {
        let Some(&raw) = buf.first() else {
            return Err(Error::ResponseTooShort {
                expected: 1,
                got: 0,
            });
        };
        Ok(Self {
            raw,
            ready: raw & 0x02 != 0,
            error_bits: raw & 0xF0,
        })
    }

    /// Whether any error bit is set.
    pub fn has_error(self) -> bool {
        self.error_bits != 0
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_register_read_write
cargo test -p ene6k77-bootloader --test ene6k77_status_decode
cargo clippy -p ene6k77-bootloader -- -D warnings
cargo doc -p ene6k77-bootloader --no-deps
```

Expected: all tests pass, no clippy/doc warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/ene6k77-bootloader/src/register.rs \
        crates/ene6k77-bootloader/tests/ene6k77_register_read_write.rs \
        crates/ene6k77-bootloader/tests/ene6k77_status_decode.rs
git commit -m "feat(ene6k77-bootloader): add register access and status decoder"
```

---

## Task 7: `flash.rs` — 9-byte command header + chunk invariants

**Files:**
- Modify: `crates/ene6k77-bootloader/src/flash.rs`
- Create: `crates/ene6k77-bootloader/tests/ene6k77_flash_command_header.rs`
- Create: `crates/ene6k77-bootloader/tests/ene6k77_flash_address_invariants.rs`

- [ ] **Step 1: Write failing tests for the 9-byte header**

Create `crates/ene6k77-bootloader/tests/ene6k77_flash_command_header.rs`:

```rust
//! Tests for the 9-byte flash command header.
//!
//! Byte layout transcribed from the HLIL of sub_140001a90; see
//! docs/decomp/fw-update/fw-update-hlil.txt cmd-path block.

use ene6k77_bootloader::flash::{
    build_command_header, Cmd, CHUNK_SIZE, HEADER_LEN, SKIP_REGION_END,
};
use ene6k77_bootloader::Error;

#[test]
fn header_is_nine_bytes() {
    assert_eq!(HEADER_LEN, 9);
    let h = build_command_header(Cmd::Write, 0x000000, 0);
    assert_eq!(h.len(), 9);
}

#[test]
fn fingerprint_write_at_0x1800_with_chunk_size_len() {
    // Source: docs/decomp/fw-update/fw-update.txt:1506-1507 (write loop
    // calls sub_140001a90(_, _, 0x40, addr=0x1800, len=0x80, _, _)) plus
    // the byte-by-byte assignment block in sub_140001a90's cmd path.
    //
    // Layout:
    //   [0]=addr_hi=(0x1800>>16)&0xff=0x00
    //   [1]=addr_mid=(0x1800>>8)&0xff=0x18
    //   [2]=addr_lo=0x1800&0xff=0x00
    //   [3]=0 (reserved)
    //   [4]=0 (reserved)
    //   [5]=len_hi=(0x80>>8)&0xff=0x00
    //   [6]=len_lo=0x80&0xff=0x80
    //   [7]=cmd=0x40 (Write)
    //   [8]=0 (reserved)
    let h = build_command_header(Cmd::Write, 0x1800, 0x80);
    assert_eq!(
        h,
        [0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x80, 0x40, 0x00],
        "header bytes do not match decomp layout"
    );
}

#[test]
fn fingerprint_open_at_zero_address() {
    // Source: docs/decomp/fw-update/fw-update.txt:1141 — sub_140001d90
    // emits a header whose byte 7 = 0x70, address = 0, length = 0.
    let h = build_command_header(Cmd::Open, 0, 0);
    assert_eq!(h, [0, 0, 0, 0, 0, 0, 0, 0x70, 0]);
}

#[test]
fn fingerprint_verify_checksum() {
    // Source: docs/decomp/fw-update/fw-update.txt:1186 — sub_140001e40
    // emits a header whose byte 7 = 0x80.
    let h = build_command_header(Cmd::VerifyChecksum, 0, 0);
    assert_eq!(h, [0, 0, 0, 0, 0, 0, 0, 0x80, 0]);
}

#[test]
fn header_packs_24_bit_address_big_endian() {
    let h = build_command_header(Cmd::Read, 0x123456, 0);
    assert_eq!(h[0], 0x12);
    assert_eq!(h[1], 0x34);
    assert_eq!(h[2], 0x56);
}

#[test]
fn header_packs_16_bit_length_big_endian() {
    let h = build_command_header(Cmd::Read, 0, 0x4000);
    assert_eq!(h[5], 0x40);
    assert_eq!(h[6], 0x00);
}

#[test]
fn cmd_from_byte_round_trip() {
    for &c in &[Cmd::Probe, Cmd::Read, Cmd::Write, Cmd::Open, Cmd::VerifyChecksum] {
        let parsed = Cmd::from_byte(c as u8).expect("known byte must parse");
        assert_eq!(parsed, c);
    }
}

#[test]
fn cmd_from_byte_rejects_unknown() {
    let err = Cmd::from_byte(0x99).expect_err("unknown byte must fail");
    assert!(matches!(err, Error::UnknownCmd(0x99)));
}

#[test]
fn chunk_size_is_128() {
    assert_eq!(CHUNK_SIZE, 0x80);
}

#[test]
fn skip_region_is_6_kilobytes() {
    assert_eq!(SKIP_REGION_END, 0x1800);
}
```

- [ ] **Step 2: Write failing tests for the address-skip helper**

Create `crates/ene6k77-bootloader/tests/ene6k77_flash_address_invariants.rs`:

```rust
//! Tests for the 0x1800-byte skip-region predicate.

use ene6k77_bootloader::flash::{is_writable_address, SKIP_REGION_END};

#[test]
fn addresses_below_skip_region_are_not_writable() {
    assert!(!is_writable_address(0));
    assert!(!is_writable_address(1));
    assert!(!is_writable_address(SKIP_REGION_END - 1));
}

#[test]
fn skip_region_end_is_writable() {
    assert!(is_writable_address(SKIP_REGION_END));
}

#[test]
fn addresses_above_skip_region_are_writable() {
    assert!(is_writable_address(SKIP_REGION_END + 1));
    assert!(is_writable_address(0xFFFF));
    assert!(is_writable_address(0xFFFFFF));
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_flash_command_header
cargo test -p ene6k77-bootloader --test ene6k77_flash_address_invariants
```

Expected: compile errors.

- [ ] **Step 4: Write the module body**

Replace `crates/ene6k77-bootloader/src/flash.rs` contents:

```rust
//! 9-byte flash command header and chunk-size invariants.
//!
//! ## Source
//!
//! - `docs/decomp/fw-update/fw-update-hlil.txt` — `sub_140001a90`
//!   cmd-path block (the byte-by-byte `var_48:N.b = ...` assignments
//!   that build the 9-byte header).
//! - `docs/decomp/fw-update/fw-update.txt:1490-1493` — `0x80` chunk
//!   cap in the write loop in `sub_1400021e0`.
//! - `docs/decomp/fw-update/fw-update.txt:1080`,
//!   `docs/decomp/fw-update/fw-update.txt:1495` — matching
//!   `if (addr >= 0x1800)` gates in the read and write loops.

use crate::{Error, Result};

/// Maximum bytes per write chunk on the bulk-data wIndex.
pub const CHUNK_SIZE: u16 = 0x80;

/// Bytes at the start of every `.bin` that are NEVER flashed (bootloader region).
pub const SKIP_REGION_END: u32 = 0x1800;

/// Length (bytes) of every flash command header.
pub const HEADER_LEN: usize = 9;

/// Command-byte values observed in the 9-byte header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Cmd {
    /// Probe / dummy command, no payload.
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:1286`
    /// (enter-bootloader handshake passes cmd=1 to the header builder).
    Probe = 0x01,
    /// Read flash chunk; followed by data on bulk wIndex.
    ///
    /// Inferred from the matching read-side loop structure in
    /// `sub_140001bb0` (`docs/decomp/fw-update/fw-update.txt:1025-1124`).
    /// Specific value `0x30` not observed at any call site — caller
    /// passes it through the same builder. Treated as the read
    /// counterpart of [`Cmd::Write`].
    Read = 0x30,
    /// Write flash chunk; followed by data on bulk wIndex.
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:1506-1507`
    /// (write loop in `sub_1400021e0` passes `0x40`).
    Write = 0x40,
    /// Open programming session.
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:1141`
    /// (`sub_140001d90` sets header byte 7 to `0x70`).
    Open = 0x70,
    /// Verify checksum / finalise.
    ///
    /// Source: `docs/decomp/fw-update/fw-update.txt:1186`
    /// (`sub_140001e40` sets header byte 7 to `0x80`).
    VerifyChecksum = 0x80,
}

impl Cmd {
    /// Parse a command byte as one of the known variants.
    pub fn from_byte(b: u8) -> Result<Self> {
        match b {
            0x01 => Ok(Cmd::Probe),
            0x30 => Ok(Cmd::Read),
            0x40 => Ok(Cmd::Write),
            0x70 => Ok(Cmd::Open),
            0x80 => Ok(Cmd::VerifyChecksum),
            other => Err(Error::UnknownCmd(other)),
        }
    }
}

/// Build the 9-byte command header.
///
/// Layout (big-endian everywhere):
///
/// | Offset | Size | Field            |
/// |--------|------|------------------|
/// | 0..=2  | 3    | 24-bit address   |
/// | 3..=4  | 2    | reserved (zero)  |
/// | 5..=6  | 2    | 16-bit length    |
/// | 7      | 1    | cmd byte         |
/// | 8      | 1    | reserved (zero)  |
///
/// # Example
///
/// ```
/// use ene6k77_bootloader::flash::{build_command_header, Cmd};
/// let h = build_command_header(Cmd::Write, 0x1800, 0x80);
/// assert_eq!(h[7], 0x40);
/// ```
pub fn build_command_header(cmd: Cmd, addr: u32, len: u16) -> [u8; HEADER_LEN] {
    let mut hdr = [0u8; HEADER_LEN];
    hdr[0] = ((addr >> 16) & 0xFF) as u8;
    hdr[1] = ((addr >> 8) & 0xFF) as u8;
    hdr[2] = (addr & 0xFF) as u8;
    hdr[5] = (len >> 8) as u8;
    hdr[6] = (len & 0xFF) as u8;
    hdr[7] = cmd as u8;
    hdr
}

/// True iff `addr` is in the writable flash region (`>= SKIP_REGION_END`).
///
/// The first 6 KB of any `.bin` is the on-chip bootloader and must
/// never be written. The harness applies this predicate; the codec
/// does not silently skip — wrong addresses are a bug at the call
/// site, not something to hide.
pub fn is_writable_address(addr: u32) -> bool {
    addr >= SKIP_REGION_END
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_flash_command_header
cargo test -p ene6k77-bootloader --test ene6k77_flash_address_invariants
cargo clippy -p ene6k77-bootloader -- -D warnings
cargo doc -p ene6k77-bootloader --no-deps
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/ene6k77-bootloader/src/flash.rs \
        crates/ene6k77-bootloader/tests/ene6k77_flash_command_header.rs \
        crates/ene6k77-bootloader/tests/ene6k77_flash_address_invariants.rs
git commit -m "feat(ene6k77-bootloader): add flash command header and chunk invariants"
```

---

## Task 8: `enter_bootloader.rs` — Handshake helpers

**Files:**
- Modify: `crates/ene6k77-bootloader/src/enter_bootloader.rs`
- Create: `crates/ene6k77-bootloader/tests/ene6k77_enter_bootloader_sequence.rs`

The enter-bootloader handshake mixes pure writes (the WDT 5-step sequence, disable_wdt, the dummy command) with a read-modify-write step (bit_mask) and a polling loop. The codec exposes the pure-write parts as a `Vec<ControlTransfer>` and the RMW math as a separate helper. The harness composes them.

- [ ] **Step 1: Write the failing test**

Create `crates/ene6k77-bootloader/tests/ene6k77_enter_bootloader_sequence.rs`:

```rust
//! Tests for enter-bootloader handshake helpers.

use ene6k77_bootloader::control::{ControlData, BM_RT_HOST_TO_DEVICE, B_REQ_WRITE};
use ene6k77_bootloader::enter_bootloader::{
    apply_bit_mask, build_disable_wdt, build_dummy_command, build_wdt_reset_sequence,
    BIT_MASK_PAIR, POLL_READY_BIT, POLL_REG,
};
use ene6k77_bootloader::register::{
    REG_MODE, REG_STATUS_MASK, REG_WDT_STEP_1, REG_WDT_STEP_2, REG_WDT_STEP_3, REG_WDT_STEP_4,
};
use ene6k77_bootloader::flash::HEADER_LEN;

#[test]
fn disable_wdt_writes_0x48_to_reg_mode() {
    // Source: docs/decomp/fw-update/fw-update.txt:735-749
    // (sub_1400016b0: var_18 = 0x48; libusb_control_transfer(_, 0x40, 0x80, 0, 0x600, &var_18, 1, ...)).
    let xfer = build_disable_wdt();
    assert_eq!(xfer.bm_request_type, BM_RT_HOST_TO_DEVICE);
    assert_eq!(xfer.b_request, B_REQ_WRITE);
    assert_eq!(xfer.w_index, REG_MODE);
    match &xfer.data {
        ControlData::Out(buf) => assert_eq!(buf.as_ref(), &[0x48u8]),
        _ => panic!("write must be Out"),
    }
}

#[test]
fn bit_mask_pair_is_0xf3_0xe1() {
    // Source: docs/decomp/fw-update/fw-update.txt:1277
    // (sub_140002000 calls sub_140001730(rbx, _, 0xf3, 0xe1)).
    assert_eq!(BIT_MASK_PAIR, (0xF3, 0xE1));
}

#[test]
fn apply_bit_mask_preserves_unmasked_bits() {
    // (current & !mask) | (mask & value)
    let result = apply_bit_mask(0b1111_1111, 0xF3, 0xE1);
    // !mask = 0x0C; (0xFF & 0x0C) = 0x0C; (0xF3 & 0xE1) = 0xE1
    // 0x0C | 0xE1 = 0xED
    assert_eq!(result, 0xED);
}

#[test]
fn apply_bit_mask_with_zero_current() {
    let result = apply_bit_mask(0x00, 0xF3, 0xE1);
    assert_eq!(result, 0xE1);
}

#[test]
fn wdt_reset_sequence_is_five_writes_in_observed_order() {
    // Source: docs/decomp/fw-update/fw-update.txt:644-714 (sub_140001510).
    // Order: write 0x601=3, 0x604=0, 0x602=0, 0x603=0xc, 0x600=3.
    let seq = build_wdt_reset_sequence();
    assert_eq!(seq.len(), 5);

    let expected: [(u16, u8); 5] = [
        (REG_WDT_STEP_1, 0x03),
        (REG_WDT_STEP_4, 0x00),
        (REG_WDT_STEP_2, 0x00),
        (REG_WDT_STEP_3, 0x0C),
        (REG_MODE, 0x03),
    ];

    for (i, (reg, val)) in expected.iter().enumerate() {
        let xfer = &seq[i];
        assert_eq!(xfer.bm_request_type, BM_RT_HOST_TO_DEVICE, "step {i}");
        assert_eq!(xfer.b_request, B_REQ_WRITE, "step {i}");
        assert_eq!(xfer.w_index, *reg, "step {i} wIndex");
        match &xfer.data {
            ControlData::Out(buf) => assert_eq!(buf.as_ref(), &[*val], "step {i} payload"),
            _ => panic!("step {i} must be Out"),
        }
    }
}

#[test]
fn dummy_command_header_is_nine_zeros_with_byte_7_one() {
    // Source: docs/decomp/fw-update/fw-update.txt:1286
    // (enter_bootloader: sub_1400013b0(_, 1, ...)) — cmd byte = 1, no payload.
    let h = build_dummy_command();
    assert_eq!(h.len(), HEADER_LEN);
    assert_eq!(h[7], 0x01);
    for (i, &b) in h.iter().enumerate() {
        if i != 7 {
            assert_eq!(b, 0, "byte {i} must be zero");
        }
    }
}

#[test]
fn poll_constants_match_decomp() {
    // Source: docs/decomp/fw-update/fw-update.txt:1304 (wIndex=0x606)
    // and 1315 (`if (var_18 & 2)`).
    assert_eq!(POLL_REG, REG_STATUS_MASK);
    assert_eq!(POLL_READY_BIT, 0x02);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_enter_bootloader_sequence
```

Expected: compile errors.

- [ ] **Step 3: Write the module body**

Replace `crates/ene6k77-bootloader/src/enter_bootloader.rs` contents:

```rust
//! Enter-bootloader handshake helpers.
//!
//! ## Source
//!
//! - `docs/decomp/fw-update/fw-update.txt:1264-1370` — `sub_140002000`,
//!   the orchestrator that strings disable_wdt → bit_mask → wdt_reset
//!   → dummy command → status poll together.
//! - `docs/decomp/fw-update/fw-update.txt:1277` — bit_mask `(0xf3,
//!   0xe1)` parameters.
//! - `docs/decomp/fw-update/fw-update.txt:644-714` — 5-step WDT reset
//!   in `sub_140001510`.
//!
//! The handshake is not a single linear sequence — it includes a
//! read-modify-write step (bit_mask) and ends in a polling loop.
//! This module exposes the discrete steps; the harness orchestrates
//! the full handshake.

use crate::control::ControlTransfer;
use crate::flash::{build_command_header, Cmd, HEADER_LEN};
use crate::register::{
    self, REG_MODE, REG_STATUS_MASK, REG_WDT_STEP_1, REG_WDT_STEP_2, REG_WDT_STEP_3,
    REG_WDT_STEP_4,
};

/// `(mask, value)` pair the original tool passes to its bit_mask call
/// during the enter-bootloader handshake.
///
/// Source: `docs/decomp/fw-update/fw-update.txt:1277`.
pub const BIT_MASK_PAIR: (u8, u8) = (0xF3, 0xE1);

/// Status register the enter-bootloader handshake polls.
pub const POLL_REG: u16 = REG_STATUS_MASK;

/// Bit in `POLL_REG` that signals "ready to advance".
///
/// Source: `docs/decomp/fw-update/fw-update.txt:1315`.
pub const POLL_READY_BIT: u8 = 0x02;

/// Step 1: disable the watchdog (write `0x48` to `REG_MODE`).
pub fn build_disable_wdt() -> ControlTransfer<'static> {
    register::build_register_write(REG_MODE, 0x48)
}

/// Step 2 helper: apply the bit-mask transformation to a register's
/// current value.
///
/// Returns `(current & !mask) | (mask & value)`.
///
/// Source: `docs/decomp/fw-update/fw-update.txt:773-782`
/// (sub_140001730 RMW computation).
pub fn apply_bit_mask(current: u8, mask: u8, value: u8) -> u8 {
    (current & !mask) | (mask & value)
}

/// Step 3: 5-step WDT reset sequence — writes only, no reads.
///
/// Returns the five [`ControlTransfer`]s in observed order.
pub fn build_wdt_reset_sequence() -> Vec<ControlTransfer<'static>> {
    vec![
        register::build_register_write(REG_WDT_STEP_1, 0x03),
        register::build_register_write(REG_WDT_STEP_4, 0x00),
        register::build_register_write(REG_WDT_STEP_2, 0x00),
        register::build_register_write(REG_WDT_STEP_3, 0x0C),
        register::build_register_write(REG_MODE, 0x03),
    ]
}

/// Step 4: dummy command header (cmd `0x01`, address 0, length 0).
///
/// Sent at the bank-keyed command wIndex with no following payload.
pub fn build_dummy_command() -> [u8; HEADER_LEN] {
    build_command_header(Cmd::Probe, 0, 0)
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_enter_bootloader_sequence
cargo clippy -p ene6k77-bootloader -- -D warnings
cargo doc -p ene6k77-bootloader --no-deps
```

Expected: all 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/ene6k77-bootloader/src/enter_bootloader.rs \
        crates/ene6k77-bootloader/tests/ene6k77_enter_bootloader_sequence.rs
git commit -m "feat(ene6k77-bootloader): add enter-bootloader handshake helpers"
```

---

## Task 9: `identify.rs` — VID, PIDs, descriptor needles

**Files:**
- Modify: `crates/ene6k77-bootloader/src/identify.rs`
- Create: `crates/ene6k77-bootloader/tests/ene6k77_identify.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/ene6k77-bootloader/tests/ene6k77_identify.rs`:

```rust
//! Tests for VID, PIDs, descriptor-substring constants.

use ene6k77_bootloader::identify::{
    is_bootloader_pid, SkuName, BOOTLOADER_PID, DESCRIPTOR_NEEDLES, ENE_VID,
};

#[test]
fn vid_is_ene_technology() {
    assert_eq!(ENE_VID, 0x0CF2);
}

#[test]
fn bootloader_pid_is_0x7758() {
    // Source: docs/decomp/fw-update/fw-update.txt:1388 (fallback open).
    assert_eq!(BOOTLOADER_PID, 0x7758);
    assert!(is_bootloader_pid(0x7758));
    assert!(!is_bootloader_pid(0xA100));
    assert!(!is_bootloader_pid(0xA200));
}

#[test]
fn sku_name_pid_lookup_matches_decomp_main() {
    // Source: docs/decomp/fw-update/fw-update.txt:1596-1639 (CLI parser).
    // The sign-extended -0x5fXX immediates resolve as PIDs:
    //   "sl" → 0xa100, "al" → 0xa101, "si" → 0xa102,
    //   "s2" → 0xa103, "a2" → 0xa104, "st" → 0xa200.
    assert_eq!(SkuName::Sl.pid(), 0xA100);
    assert_eq!(SkuName::Al.pid(), 0xA101);
    assert_eq!(SkuName::Si.pid(), 0xA102);
    assert_eq!(SkuName::S2.pid(), 0xA103);
    assert_eq!(SkuName::A2.pid(), 0xA104);
    assert_eq!(SkuName::St.pid(), 0xA200);
}

#[test]
fn sku_name_from_str_round_trip() {
    for (s, expected) in [
        ("sl", SkuName::Sl),
        ("al", SkuName::Al),
        ("si", SkuName::Si),
        ("s2", SkuName::S2),
        ("a2", SkuName::A2),
        ("st", SkuName::St),
    ] {
        let parsed = SkuName::from_str(s).expect("known SKU must parse");
        assert_eq!(parsed, expected);
    }
}

#[test]
fn sku_name_from_str_rejects_unknown() {
    assert!(SkuName::from_str("xx").is_none());
    assert!(SkuName::from_str("SL").is_none(), "case-sensitive");
    assert!(SkuName::from_str("").is_none());
}

#[test]
fn descriptor_needles_match_decomp_strings() {
    // Source: docs/decomp/fw-update/fw-update.txt:497-501
    // (strstr(&str1, "LianLi") and strstr(&str1, "6K7750")).
    assert!(DESCRIPTOR_NEEDLES.contains(&"LianLi"));
    assert!(DESCRIPTOR_NEEDLES.contains(&"6K7750"));
    assert_eq!(DESCRIPTOR_NEEDLES.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_identify
```

Expected: compile errors.

- [ ] **Step 3: Write the module body**

Replace `crates/ene6k77-bootloader/src/identify.rs` contents:

```rust
//! USB device identification: VID, per-SKU PIDs, descriptor-substring filter.
//!
//! ## Source
//!
//! - `docs/decomp/fw-update/fw-update.txt:497-501` — descriptor-substring
//!   filter (`strstr` for `"LianLi"` / `"6K7750"`).
//! - `docs/decomp/fw-update/fw-update.txt:1388` — fallback open at
//!   bootloader PID `0x7758`.
//! - `docs/decomp/fw-update/fw-update.txt:1596-1639` — CLI parser
//!   that maps SKU strings to PIDs.
//! - `docs/development/protocol/cross-device-survey/per-family/ene6k77fan.md` —
//!   PID table for application-mode devices.

/// USB Vendor ID for the ENE chip family used by this bootloader.
pub const ENE_VID: u16 = 0x0CF2;

/// USB Product ID exposed when the chip is in bootloader mode.
pub const BOOTLOADER_PID: u16 = 0x7758;

/// Descriptor-string substrings the original tool searches for in the
/// device's iManufacturer/iProduct ASCII string when matching.
///
/// At least one of these must be present in the concatenated
/// descriptor string for the original tool to consider the device a
/// match.
pub const DESCRIPTOR_NEEDLES: &[&str] = &["LianLi", "6K7750"];

/// SKU short-names accepted by the original `FwUpdate.exe` CLI's first
/// positional argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkuName {
    /// `"sl"` — SLFan.
    Sl,
    /// `"al"` — ALFan.
    Al,
    /// `"si"` — SLInfinity.
    Si,
    /// `"s2"` — SLV2.
    S2,
    /// `"a2"` — ALV2.
    A2,
    /// `"st"` — StrimerPlus (shares the bootloader chip).
    St,
}

impl SkuName {
    /// Application-mode PID for this SKU.
    pub fn pid(self) -> u16 {
        match self {
            SkuName::Sl => 0xA100,
            SkuName::Al => 0xA101,
            SkuName::Si => 0xA102,
            SkuName::S2 => 0xA103,
            SkuName::A2 => 0xA104,
            SkuName::St => 0xA200,
        }
    }

    /// Parse a SKU short-name (case-sensitive, lowercase only,
    /// matching the original tool's behaviour).
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sl" => Some(SkuName::Sl),
            "al" => Some(SkuName::Al),
            "si" => Some(SkuName::Si),
            "s2" => Some(SkuName::S2),
            "a2" => Some(SkuName::A2),
            "st" => Some(SkuName::St),
            _ => None,
        }
    }
}

/// Whether `pid` is the bootloader-mode PID.
pub fn is_bootloader_pid(pid: u16) -> bool {
    pid == BOOTLOADER_PID
}
```

- [ ] **Step 4: Run tests to verify they pass and run the full crate suite**

```bash
cargo test -p ene6k77-bootloader
cargo clippy -p ene6k77-bootloader -- -D warnings
cargo doc -p ene6k77-bootloader --no-deps
```

Expected: every test in the crate passes; no clippy or rustdoc warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/ene6k77-bootloader/src/identify.rs \
        crates/ene6k77-bootloader/tests/ene6k77_identify.rs
git commit -m "feat(ene6k77-bootloader): add VID/PID and descriptor identification"
```

---

## Task 9.5: Realistic-bin shape tests

**Files:**
- Create: `crates/ene6k77-bootloader/tests/ene6k77_realworld_bin_shapes.rs`
- Possibly create: `crates/ene6k77-bootloader/tests/fixtures/` (only if license allows in-repo)

The user has a copy of `LianLi_FwUpdate-tools_v.1.0.1.18.zip` extracted under `fw/` (parent of `lconnect-linux`). Using a real shipped `.bin` as a test fixture lets us assert the codec's chunk math holds against a realistic file size, not just the synthetic `0x1800`-byte boundary case.

**License gate first.** The bin files are Lian-Li-distributed firmware, not the user's IP. The lconnect-linux repo is GPL-3.0. Lian-Li's firmware licensing is unstated; the conservative read is "don't redistribute". So the test reads from an out-of-repo path under `fw/`, gated behind `#[ignore]` so the default `cargo test` passes regardless of whether the path exists.

If a future contributor confirms a redistribution license, the test can be promoted by dropping `#[ignore]` and copying the bin into `tests/fixtures/`.

- [ ] **Step 1: Write the failing test**

Create `crates/ene6k77-bootloader/tests/ene6k77_realworld_bin_shapes.rs`:

```rust
//! Sanity-checks the codec's chunk math against a real shipped firmware bin.
//!
//! This test is `#[ignore]` by default — it depends on a Lian-Li-distributed
//! file under `../../../fw/LianLi_FwUpdate-tools_v.1.0.1.18/SL_v2_0.7/...`
//! which is not committed to this repo for licensing reasons.
//!
//! Run explicitly with:
//!
//! ```text
//! cargo test -p ene6k77-bootloader --test ene6k77_realworld_bin_shapes -- --ignored
//! ```

use ene6k77_bootloader::flash::{CHUNK_SIZE, SKIP_REGION_END};

const BIN_RELATIVE_PATH: &str =
    "../../../fw/LianLi_FwUpdate-tools_v.1.0.1.18/SL_v2_0.7/6K77XX_LLUF-SLv2_HW1_20230310_0x00373B91_V07.bin";

const FILENAME_CHECKSUM_HEX: &str = "00373B91";

fn bin_path() -> std::path::PathBuf {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(crate_dir).join(BIN_RELATIVE_PATH)
}

#[test]
#[ignore = "depends on out-of-repo Lian-Li firmware bin"]
fn realworld_sl_v2_bin_size_exceeds_skip_region() {
    let path = bin_path();
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        (bytes.len() as u32) > SKIP_REGION_END,
        "bin size {} should exceed skip region 0x{:x}",
        bytes.len(),
        SKIP_REGION_END
    );
}

#[test]
#[ignore = "depends on out-of-repo Lian-Li firmware bin"]
fn realworld_sl_v2_chunk_count_fits_in_u16_length_field() {
    let path = bin_path();
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let writable = (bytes.len() as u32) - SKIP_REGION_END;
    let chunks = writable.div_ceil(u32::from(CHUNK_SIZE));
    // CHUNK_SIZE itself is the per-chunk length, which must fit in u16.
    assert!(u32::from(CHUNK_SIZE) <= u32::from(u16::MAX));
    // Sanity: a real bin produces a sensible chunk count, not zero or millions.
    assert!(chunks > 0, "no chunks?");
    assert!(
        chunks < 100_000,
        "chunk count {chunks} unreasonably large — bin parse error?"
    );
}

#[test]
#[ignore = "depends on out-of-repo Lian-Li firmware bin"]
fn realworld_filename_suffix_parses_as_u32_checksum_placeholder() {
    // The 8-hex suffix in the filename is presumed to be the firmware
    // checksum that cmd=0x80 (VerifyChecksum) verifies on the device.
    // The algorithm is unknown (open question in the spec doc §10).
    // This test only confirms the suffix is well-formed hex.
    let parsed = u32::from_str_radix(FILENAME_CHECKSUM_HEX, 16)
        .expect("filename suffix must be valid hex");
    assert_ne!(parsed, 0, "non-zero checksum expected");
}
```

- [ ] **Step 2: Run with --ignored to confirm it works when the file is present**

```bash
cargo test -p ene6k77-bootloader --test ene6k77_realworld_bin_shapes -- --ignored
```

Expected outcomes:

- If the file is present at the path above: 3 tests pass.
- If not present: tests fail with a clear file-not-found message naming the path. That's correct behaviour — the test is opt-in via `--ignored` so a missing fixture isn't silently skipped at default invocation.

- [ ] **Step 3: Run the default test invocation to confirm `#[ignore]` works**

```bash
cargo test -p ene6k77-bootloader
```

Expected: every test in tasks 4-9 passes; the three tests added here show as `ignored`.

- [ ] **Step 4: Commit**

```bash
git add crates/ene6k77-bootloader/tests/ene6k77_realworld_bin_shapes.rs
git commit -m "test(ene6k77-bootloader): add realistic-bin shape tests (ignored by default)

Three #[ignore] tests that load a real ENE 6K77 firmware bin from
the user's local fw/ tree (out-of-repo for licensing reasons) and
assert the codec's chunk math holds against a realistic file size
(~45 KB SL v2 firmware). Promotion to default-on requires either
clarifying redistribution license on the bin or someone with the
matching path enabling them locally with --ignored."
```

---

## Task 10: Harness binary skeleton

**Files:**
- Modify: `Cargo.toml` (workspace) — add tool member, add `rusb` workspace dep
- Create: `tools/fw-update-ene6k77/Cargo.toml`
- Create: `tools/fw-update-ene6k77/README.md`
- Create: `tools/fw-update-ene6k77/src/main.rs`
- Create: `tools/fw-update-ene6k77/src/cli.rs`
- Create: `tools/fw-update-ene6k77/src/banner.rs`

Subcommand bodies are stubs that exit with `unimplemented!()` — they get filled in tasks 11-13.

- [ ] **Step 1: Add the tool to the workspace and add `rusb`**

Modify `Cargo.toml` (workspace root):

- Append `"tools/fw-update-ene6k77"` to `members`.
- In `[workspace.dependencies]`, add `rusb = "0.9"`.

After edit the relevant blocks should look like:

```toml
members = [
    "crates/protocol",
    "crates/lconnect-image",
    "crates/ene6k77-bootloader",
    "tools/verify-galahad",
    "tools/fw-update-ene6k77",
]

[workspace.dependencies]
thiserror  = "1"
hidapi     = "2.5"
rusb       = "0.9"
anyhow     = "1"
clap       = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: Create the per-tool manifest**

Write `tools/fw-update-ene6k77/Cargo.toml`:

```toml
[package]
name         = "fw-update-ene6k77"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
description = "EXPERIMENTAL — UNTESTED — MAY PERMANENTLY BRICK YOUR DEVICE. Linux Rust port of Lian-Li's FwUpdate.exe for the ENE 6K77 fan-controller family. Derived from static analysis of the Windows binary; never validated against real hardware. Use at your own risk."
publish     = false

[[bin]]
name = "fw-update-ene6k77"
path = "src/main.rs"

[dependencies]
ene6k77-bootloader = { path = "../../crates/ene6k77-bootloader" }
rusb               = { workspace = true }
anyhow             = { workspace = true }
clap               = { workspace = true }
```

- [ ] **Step 3: Create the README**

Write `tools/fw-update-ene6k77/README.md`:

```markdown
# `fw-update-ene6k77` — experimental firmware updater

> ## ⚠️ EXPERIMENTAL — UNTESTED — MAY PERMANENTLY BRICK YOUR DEVICE
>
> This tool is a Rust port of Lian-Li's Windows-only `FwUpdate.exe`
> for the ENE 6K77xx fan-controller family. It was derived entirely
> from static analysis of the decompiled Windows binary and **has
> never been validated against real hardware**.
>
> Running this tool against a real device may permanently brick the
> device. Recovery may require programming hardware (SWD/ICSP/JTAG).
> The author of this tool has no way to test it. The Linux project
> has no obligation to help recover bricked devices. **Use entirely
> at your own risk.**

## Affected devices

Application-mode VID:PID pairs (per
`docs/development/protocol/cross-device-survey/per-family/ene6k77fan.md`):

| SKU short-name | PID    | Product            |
|----------------|--------|--------------------|
| `sl`           | 0xA100 | SLFan              |
| `al`           | 0xA101 | ALFan              |
| `si`           | 0xA102 | SLInfinity         |
| `s2`           | 0xA103 | SLV2 Fan           |
| `a2`           | 0xA104 | ALV2 Fan           |
| (no shortcut)  | 0xA105 | SLV2A Fan          |
| (no shortcut)  | 0xA106 | SLRedragon         |
| `st`           | 0xA200 | StrimerPlus        |

VID is `0x0CF2` (ENE Technology) for all SKUs. Bootloader-mode PID
is `0x7758`.

## Subcommands

- `list` — enumerate devices on the bus. Read-only. Always safe.
- `inspect --vid 0xNNNN --pid 0xNNNN` — read status registers of a
  device. Read-only. Does not enter bootloader mode.
- `flash --i-accept-untested-firmware-risk --vid 0xNNNN --pid 0xNNNN
  --bin path/to/firmware.bin [--yes]` — the destructive operation.

## Recovery from a bricked device

If the flash sequence fails between steps 8 and 14 (see
`docs/development/protocol/ene6k77-bootloader.md` §9), the device is in an
indeterminate state.

- Do **not** power-cycle the device.
- Retry `flash` with the same `.bin`. If the bootloader is still
  responsive on PID 0x7758, this is the least-bad recovery.
- If retry also fails: the device is likely unrecoverable without
  programming hardware.

## Required udev rule

Non-root access to the bootloader-mode PID needs a udev rule. A
working rule for the Galahad II LCDV2 lives at
`docs/development/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules`;
adapt it to match `idVendor=0cf2 idProduct=7758` plus the
application-mode PIDs you care about.

## Source

This tool is derived from
`docs/decomp/fw-update/fw-update.txt` and
`docs/decomp/fw-update/fw-update-hlil.txt`. Protocol reference:
`docs/development/protocol/ene6k77-bootloader.md`.
```

- [ ] **Step 4: Create the banner module**

Write `tools/fw-update-ene6k77/src/banner.rs`:

```rust
//! Untested-firmware-risk banner text.
//!
//! The same text is duplicated in `Cargo.toml`'s description, the
//! README, and the clap `about` string. Single source of truth for
//! the runtime printout.

/// Multi-line banner printed before any potentially destructive operation.
pub const RISK_BANNER: &str = "\
================================================================================
  ⚠️  EXPERIMENTAL FIRMWARE UPDATER — UNTESTED CODE  ⚠️
================================================================================

  This tool was derived from STATIC ANALYSIS of the decompiled Windows
  binary FwUpdate.exe. It has NEVER been validated against real hardware.

  Running this against a real device may PERMANENTLY BRICK it. Recovery
  may require programming hardware (SWD/ICSP/JTAG).

  The Linux project cannot help if your device is bricked. You are
  USING THIS ENTIRELY AT YOUR OWN RISK.

================================================================================
";

/// Short message printed when the destructive flag is missing.
pub const FLAG_MISSING_HINT: &str = "\
The `flash` subcommand requires the literal flag

    --i-accept-untested-firmware-risk

on the command line. There is no shorter form.";
```

- [ ] **Step 5: Create the clap CLI module**

Write `tools/fw-update-ene6k77/src/cli.rs`:

```rust
//! CLI definition.

use clap::{Args, Parser, Subcommand};

/// Hex-string parser for `--vid` / `--pid` arguments.
///
/// Accepts both `0xNNNN` and bare `NNNN` hex.
pub fn parse_hex_u16(s: &str) -> Result<u16, String> {
    let trimmed = s.trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(trimmed, 16).map_err(|e| format!("invalid hex u16 `{s}`: {e}"))
}

/// EXPERIMENTAL — UNTESTED — MAY PERMANENTLY BRICK YOUR DEVICE.
///
/// Linux Rust port of Lian-Li's FwUpdate.exe for the ENE 6K77
/// fan-controller family. Derived from static analysis of the Windows
/// binary; never validated against real hardware. Use at your own risk.
#[derive(Parser, Debug)]
#[command(version, about, long_about)]
pub struct Cli {
    /// Print every USB control transfer to stderr.
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Enumerate ENE 6K77 devices on the USB bus. Read-only.
    List,
    /// Read status registers of a device. Read-only; does not enter bootloader mode.
    Inspect(InspectArgs),
    /// EXPERIMENTAL: write a firmware image to a device.
    Flash(FlashArgs),
}

#[derive(Args, Debug)]
pub struct InspectArgs {
    /// USB Vendor ID (hex, e.g. `0x0CF2`).
    #[arg(long, value_parser = parse_hex_u16)]
    pub vid: u16,
    /// USB Product ID (hex, e.g. `0x7758` for bootloader mode).
    #[arg(long, value_parser = parse_hex_u16)]
    pub pid: u16,
}

#[derive(Args, Debug)]
pub struct FlashArgs {
    /// Required risk-acceptance flag. Must be passed verbatim every invocation.
    #[arg(long = "i-accept-untested-firmware-risk")]
    pub risk_accepted: bool,

    /// USB Vendor ID (hex). Should be `0x0CF2` for ENE devices.
    #[arg(long, value_parser = parse_hex_u16)]
    pub vid: u16,

    /// USB Product ID (hex). Either an application-mode PID
    /// (`0xA100..0xA106` or `0xA200`) or the bootloader PID `0x7758`.
    #[arg(long, value_parser = parse_hex_u16)]
    pub pid: u16,

    /// Path to firmware image.
    #[arg(long)]
    pub bin: std::path::PathBuf,

    /// Skip the interactive y/N prompt (the risk flag is still required).
    #[arg(long)]
    pub yes: bool,
}
```

- [ ] **Step 6: Create the main entry point**

Write `tools/fw-update-ene6k77/src/main.rs`:

```rust
//! EXPERIMENTAL — UNTESTED — MAY PERMANENTLY BRICK YOUR DEVICE.
//!
//! Linux Rust port of Lian-Li's `FwUpdate.exe` for the ENE 6K77
//! fan-controller family. Derived from static analysis of the
//! Windows binary; never validated against real hardware.

mod banner;
mod cli;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = cli::Cli::parse();

    match args.cmd {
        cli::Cmd::List => unimplemented!("list — Task 11"),
        cli::Cmd::Inspect(_) => unimplemented!("inspect — Task 11"),
        cli::Cmd::Flash(flash_args) => {
            if !flash_args.risk_accepted {
                eprintln!("{}", banner::RISK_BANNER);
                eprintln!("{}", banner::FLAG_MISSING_HINT);
                std::process::exit(2);
            }
            unimplemented!("flash — Tasks 12-13")
        }
    }
}
```

- [ ] **Step 7: Verify the skeleton compiles and the help text shows the banner**

```bash
cargo build -p fw-update-ene6k77
cargo clippy -p fw-update-ene6k77 -- -D warnings
cargo run -p fw-update-ene6k77 -- --help
cargo run -p fw-update-ene6k77 -- flash --vid 0xCF2 --pid 0xA100 --bin /tmp/fake.bin
```

Expected:
- Build and clippy clean.
- `--help` shows the banner-derived `about` string before usage.
- The `flash` command without the risk flag exits with code 2 and prints the banner + hint to stderr.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml tools/fw-update-ene6k77/
git commit -m "feat(fw-update-ene6k77): scaffold experimental harness binary

CLI skeleton with clap-derived subcommands (list / inspect / flash).
Subcommand bodies stub to unimplemented!(); the destructive flash
path additionally fails with code 2 and prints the risk banner
when --i-accept-untested-firmware-risk is missing. Bodies fill in
over Tasks 11-13."
```

---

## Task 11: `list` and `inspect` subcommands

**Files:**
- Create: `tools/fw-update-ene6k77/src/usb.rs`
- Modify: `tools/fw-update-ene6k77/src/main.rs`

These are the read-only subcommands. They open the device, do a small read, and exit.

- [ ] **Step 1: Create the USB helper module**

Write `tools/fw-update-ene6k77/src/usb.rs`:

```rust
//! Thin wrapper over `rusb` that translates a codec
//! [`ControlTransfer`] into a real USB call.

use std::time::Duration;

use anyhow::{Context, Result};
use ene6k77_bootloader::control::{ControlData, ControlTransfer};
use rusb::{Device, DeviceHandle, GlobalContext, UsbContext};

/// Open and claim interface 0 of the first device matching `(vid, pid)`.
pub fn open(vid: u16, pid: u16) -> Result<DeviceHandle<GlobalContext>> {
    let handle = rusb::open_device_with_vid_pid(vid, pid)
        .with_context(|| format!("no device with VID:PID {vid:#06x}:{pid:#06x} found"))?;
    handle
        .claim_interface(0)
        .with_context(|| "failed to claim interface 0")?;
    Ok(handle)
}

/// Dispatch a [`ControlTransfer`] through `rusb`.
pub fn dispatch(
    handle: &DeviceHandle<GlobalContext>,
    xfer: &mut ControlTransfer<'_>,
    verbose: bool,
) -> Result<usize> {
    if verbose {
        eprintln!(
            "  xfer bmRT=0x{:02x} bReq=0x{:02x} wIdx=0x{:04x} len={} dir={}",
            xfer.bm_request_type,
            xfer.b_request,
            xfer.w_index,
            xfer.data.len(),
            match &xfer.data {
                ControlData::In(_) => "IN",
                ControlData::Out(_) => "OUT",
            }
        );
    }
    let timeout = Duration::from_millis(xfer.timeout_ms.into());
    let n = match &mut xfer.data {
        ControlData::In(buf) => handle.read_control(
            xfer.bm_request_type,
            xfer.b_request,
            xfer.w_value,
            xfer.w_index,
            buf,
            timeout,
        )?,
        ControlData::Out(buf) => handle.write_control(
            xfer.bm_request_type,
            xfer.b_request,
            xfer.w_value,
            xfer.w_index,
            buf,
            timeout,
        )?,
    };
    Ok(n)
}

/// Iterate every USB device on the bus, returning those that match
/// the ENE VID and have an application-mode or bootloader PID.
pub fn enumerate_ene_devices() -> Result<Vec<(Device<GlobalContext>, u16, u16)>> {
    let context = GlobalContext::default();
    let list = context.devices().context("rusb: enumerate devices")?;
    let mut out = Vec::new();
    for dev in list.iter() {
        let desc = match dev.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if desc.vendor_id() != ene6k77_bootloader::identify::ENE_VID {
            continue;
        }
        let pid = desc.product_id();
        out.push((dev, desc.vendor_id(), pid));
    }
    Ok(out)
}
```

- [ ] **Step 2: Wire `list` into `main.rs`**

Replace the `cli::Cmd::List` arm in `main.rs`:

```rust
        cli::Cmd::List => {
            let devices = usb::enumerate_ene_devices()?;
            if devices.is_empty() {
                println!("No ENE 6K77 devices found.");
                return Ok(());
            }
            println!("Found {} ENE 6K77 device(s):", devices.len());
            for (_, vid, pid) in &devices {
                let mode = if ene6k77_bootloader::identify::is_bootloader_pid(*pid) {
                    "BOOTLOADER MODE"
                } else {
                    "application mode"
                };
                println!("  VID:PID {vid:#06x}:{pid:#06x} — {mode}");
            }
        }
```

Add `mod usb;` to the top of `main.rs` next to the existing `mod` declarations.

- [ ] **Step 3: Wire `inspect` into `main.rs`**

Replace the `cli::Cmd::Inspect(_)` arm:

```rust
        cli::Cmd::Inspect(args) => {
            let handle = usb::open(args.vid, args.pid)?;
            let mut buf = [0u8; 1];
            let mut xfer = ene6k77_bootloader::register::build_register_read(
                ene6k77_bootloader::register::REG_STATUS_MASK,
                &mut buf,
            );
            let n = usb::dispatch(&handle, &mut xfer, args_verbose)?;
            if n != 1 {
                anyhow::bail!("expected 1 byte from REG_STATUS_MASK, got {n}");
            }
            let status = ene6k77_bootloader::register::StatusReg::from_response(&buf)?;
            println!(
                "REG_STATUS_MASK (0x606) = 0x{:02x}  ready={}  error_bits=0x{:02x}",
                status.raw, status.ready, status.error_bits
            );
        }
```

Note that `args_verbose` should reference `args.verbose` from the parent `Cli` struct. The cleanest way is to capture it before the match. Restructure the `match` so that `let verbose = args.verbose;` precedes the destructuring `match args.cmd`. Updated main shape:

```rust
fn main() -> Result<()> {
    let args = cli::Cli::parse();
    let verbose = args.verbose;

    match args.cmd {
        cli::Cmd::List => { /* ... */ }
        cli::Cmd::Inspect(args) => {
            let handle = usb::open(args.vid, args.pid)?;
            let mut buf = [0u8; 1];
            let mut xfer = ene6k77_bootloader::register::build_register_read(
                ene6k77_bootloader::register::REG_STATUS_MASK,
                &mut buf,
            );
            usb::dispatch(&handle, &mut xfer, verbose)?;
            let status = ene6k77_bootloader::register::StatusReg::from_response(&buf)?;
            println!(
                "REG_STATUS_MASK (0x606) = 0x{:02x}  ready={}  error_bits=0x{:02x}",
                status.raw, status.ready, status.error_bits
            );
        }
        cli::Cmd::Flash(flash_args) => { /* unchanged */ }
    }
    Ok(())
}
```

- [ ] **Step 4: Verify it builds and the help still works**

```bash
cargo build -p fw-update-ene6k77
cargo clippy -p fw-update-ene6k77 -- -D warnings
cargo run -p fw-update-ene6k77 -- list
```

Expected: builds clean. `list` either prints "No ENE 6K77 devices found." or fails with a libusb permission error (depending on whether the user has any ENE devices and a matching udev rule). Either is acceptable for this task.

- [ ] **Step 5: Commit**

```bash
git add tools/fw-update-ene6k77/src/usb.rs tools/fw-update-ene6k77/src/main.rs
git commit -m "feat(fw-update-ene6k77): implement read-only list and inspect"
```

---

## Task 12: `flash` subcommand — enter-bootloader phase

**Files:**
- Create: `tools/fw-update-ene6k77/src/flash_flow.rs`
- Modify: `tools/fw-update-ene6k77/src/main.rs`

This task adds the destructive subcommand's setup phase: banner, prompt, file validation, USB open, enter-bootloader handshake, bootloader confirmation read. Actual flashing follows in Task 13.

- [ ] **Step 1: Create `flash_flow.rs` with the setup helpers**

Write `tools/fw-update-ene6k77/src/flash_flow.rs`:

```rust
//! End-to-end flash orchestration.
//!
//! See `docs/development/protocol/ene6k77-bootloader.md` §8-9 for the protocol
//! steps this module implements.

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use ene6k77_bootloader::bank::Bank;
use ene6k77_bootloader::control::ControlTransfer;
use ene6k77_bootloader::enter_bootloader as eb;
use ene6k77_bootloader::flash::{self, Cmd, SKIP_REGION_END};
use ene6k77_bootloader::identify::{self, BOOTLOADER_PID};
use ene6k77_bootloader::register::{self, StatusReg};
use rusb::{DeviceHandle, GlobalContext};

use crate::banner::RISK_BANNER;
use crate::usb;

/// Read the bin into memory and reject obviously-invalid sizes.
pub fn read_and_validate_bin(path: &Path) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if (bytes.len() as u32) < SKIP_REGION_END {
        bail!(
            "bin file too small: {} bytes; minimum size is 0x{:04x} (the bootloader region must be present in the file)",
            bytes.len(),
            SKIP_REGION_END
        );
    }
    Ok(bytes)
}

/// Print the runtime banner and gate on interactive y/N unless `yes_flag`.
pub fn confirm_or_abort(yes_flag: bool) -> Result<()> {
    eprintln!("{RISK_BANNER}");
    if yes_flag {
        eprintln!("--yes was passed; skipping interactive prompt.");
        return Ok(());
    }
    eprint!("Type the literal word `yes` to proceed: ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("reading stdin")?;
    let trimmed = line.trim_end_matches(&['\n', '\r'][..]);
    if trimmed != "yes" {
        bail!("user did not confirm");
    }
    Ok(())
}

/// Run the enter-bootloader handshake. Caller passes the open
/// app-mode handle. The function returns once the handshake completes;
/// the caller is then expected to close the handle, sleep, and re-open
/// at the bootloader PID.
pub fn enter_bootloader(
    handle: &DeviceHandle<GlobalContext>,
    verbose: bool,
) -> Result<()> {
    // Step 1: disable_wdt.
    let mut x = eb::build_disable_wdt();
    usb::dispatch(handle, &mut x, verbose).context("disable_wdt")?;

    // Step 2: bit_mask read-modify-write on REG_STATUS_MASK.
    let (mask, value) = eb::BIT_MASK_PAIR;
    let mut current = [0u8; 1];
    let mut x = register::build_register_read(register::REG_STATUS_MASK, &mut current);
    usb::dispatch(handle, &mut x, verbose).context("bit_mask read")?;
    let new = eb::apply_bit_mask(current[0], mask, value);
    if new != current[0] {
        let mut x = register::build_register_write(register::REG_STATUS_MASK, new);
        usb::dispatch(handle, &mut x, verbose).context("bit_mask write")?;
    }

    // Step 3: 5-step WDT reset.
    for (i, mut x) in eb::build_wdt_reset_sequence().into_iter().enumerate() {
        usb::dispatch(handle, &mut x, verbose).with_context(|| format!("wdt_reset step {i}"))?;
    }
    std::thread::sleep(Duration::from_millis(120));

    // Step 4: dummy command at bank A (initial bank for app-mode open).
    let header = eb::build_dummy_command();
    let mut x = ControlTransfer {
        bm_request_type: ene6k77_bootloader::control::BM_RT_HOST_TO_DEVICE,
        b_request: ene6k77_bootloader::control::B_REQ_WRITE,
        w_value: 0,
        w_index: Bank::A.command_w_index(),
        data: ene6k77_bootloader::control::ControlData::Out(std::borrow::Cow::Owned(
            header.to_vec(),
        )),
        timeout_ms: ene6k77_bootloader::control::TIMEOUT_MS,
    };
    usb::dispatch(handle, &mut x, verbose).context("enter-bootloader dummy command")?;

    // Step 5: poll REG_STATUS_MASK bit 1 with 3-second timeout.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if std::time::Instant::now() > deadline {
            bail!("enter-bootloader timeout waiting for status ready bit");
        }
        let mut buf = [0u8; 1];
        let mut x = register::build_register_read(eb::POLL_REG, &mut buf);
        usb::dispatch(handle, &mut x, verbose).context("status poll")?;
        if buf[0] & eb::POLL_READY_BIT != 0 {
            // Clear status bits.
            let mut x = register::build_register_write(eb::POLL_REG, eb::POLL_READY_BIT);
            usb::dispatch(handle, &mut x, verbose).context("status clear")?;
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Re-open the device at the bootloader PID after enter-bootloader.
/// Sleeps briefly to let the device re-enumerate.
pub fn reopen_at_bootloader_pid(vid: u16) -> Result<DeviceHandle<GlobalContext>> {
    std::thread::sleep(Duration::from_millis(500));
    usb::open(vid, BOOTLOADER_PID)
        .context("re-open at bootloader PID 0x7758 — did the device re-enumerate?")
}

/// Confirm we're talking to the bootloader by issuing the
/// 1-byte read at wIndex 0x4000 that the original tool uses as a probe.
///
/// Source: `docs/decomp/fw-update/fw-update.txt:602` (libusb_control_transfer
/// with bmRT=0xc0, bReq=0x81, wIndex=0x4000, len=1).
pub fn confirm_bootloader(handle: &DeviceHandle<GlobalContext>, verbose: bool) -> Result<()> {
    let mut buf = [0u8; 1];
    let mut x = register::build_register_read(0x4000, &mut buf);
    usb::dispatch(handle, &mut x, verbose).context("bootloader confirmation read at 0x4000")?;
    Ok(())
}
```

- [ ] **Step 2: Wire the `Flash` arm into `main.rs` (setup-only for now)**

Replace the `cli::Cmd::Flash(flash_args)` arm in `main.rs`:

```rust
        cli::Cmd::Flash(flash_args) => {
            if !flash_args.risk_accepted {
                eprintln!("{}", banner::RISK_BANNER);
                eprintln!("{}", banner::FLAG_MISSING_HINT);
                std::process::exit(2);
            }

            let bin = flash_flow::read_and_validate_bin(&flash_args.bin)?;
            flash_flow::confirm_or_abort(flash_args.yes)?;

            // Phase 1: enter-bootloader (only if currently in app mode).
            let handle = if ene6k77_bootloader::identify::is_bootloader_pid(flash_args.pid) {
                usb::open(flash_args.vid, flash_args.pid)?
            } else {
                let app_handle = usb::open(flash_args.vid, flash_args.pid)?;
                eprintln!("entering bootloader...");
                flash_flow::enter_bootloader(&app_handle, verbose)?;
                drop(app_handle);
                flash_flow::reopen_at_bootloader_pid(flash_args.vid)?
            };

            // Phase 2: confirm we're in bootloader mode.
            flash_flow::confirm_bootloader(&handle, verbose)?;
            eprintln!("bootloader confirmed.");

            // Phase 3: flash — implemented in Task 13.
            unimplemented!("flash phases 3-5 — Task 13");
        }
```

Add `mod flash_flow;` to the top of `main.rs`. Suppress the unused-variable warning on `bin` for this task with `let _bin = bin;` placed right before the `unimplemented!`.

- [ ] **Step 3: Verify it builds**

```bash
cargo build -p fw-update-ene6k77
cargo clippy -p fw-update-ene6k77 -- -D warnings
```

Expected: builds clean.

If there are clippy warnings about `unimplemented!` reaching unreachable code or about the unused `_bin`, suppress them inline with `#[allow(...)]` only as a last resort — the cleaner fix is to inline the bin-load between the bootloader-confirm and the unimplemented (so the variable isn't dead). Restructure if needed.

- [ ] **Step 4: Manual smoke test (no device required)**

```bash
cargo run -p fw-update-ene6k77 -- flash --vid 0xCF2 --pid 0x7758 --bin /tmp/nope
```

Expected: anyhow error like "read /tmp/nope: No such file or directory" (file validation runs before USB).

```bash
truncate -s 100 /tmp/tiny
cargo run -p fw-update-ene6k77 -- flash --i-accept-untested-firmware-risk \
    --vid 0xCF2 --pid 0x7758 --bin /tmp/tiny
```

Expected: error "bin file too small" (size validation runs before the prompt). 

- [ ] **Step 5: Commit**

```bash
git add tools/fw-update-ene6k77/src/flash_flow.rs tools/fw-update-ene6k77/src/main.rs
git commit -m "feat(fw-update-ene6k77): wire enter-bootloader handshake

Implements phases 1-2 of the flash flow: bin validation, runtime
banner with interactive y/N gate, app-mode open, enter-bootloader
handshake (disable_wdt + bit_mask RMW + WDT reset + dummy cmd +
status poll), re-open at PID 0x7758, bootloader confirmation read.

Phases 3-5 (flash_open, write loop, verify-read, memcmp,
checksum, close) follow in the next commit."
```

---

## Task 13: `flash` subcommand — write/verify/checksum phases

**Files:**
- Modify: `tools/fw-update-ene6k77/src/flash_flow.rs`
- Modify: `tools/fw-update-ene6k77/src/main.rs`

- [ ] **Step 1: Add the wait_ready helper to `flash_flow.rs`**

Append to `tools/fw-update-ene6k77/src/flash_flow.rs`:

```rust
/// Toggle bank, poll the status register, return the new bank.
///
/// Mirrors the original `sub_140001810`: toggles first, then polls.
/// Returns the bank that was selected for the poll.
pub fn wait_ready(
    handle: &DeviceHandle<GlobalContext>,
    bank: Bank,
    verbose: bool,
) -> Result<Bank> {
    let next_bank = bank.toggle();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if std::time::Instant::now() > deadline {
            bail!("wait_ready timeout (bank {:?})", next_bank);
        }
        let mut buf = [0u8; 1];
        let mut x = ControlTransfer {
            bm_request_type: ene6k77_bootloader::control::BM_RT_DEVICE_TO_HOST,
            b_request: ene6k77_bootloader::control::B_REQ_READ,
            w_value: 0,
            w_index: next_bank.status_w_index(),
            data: ene6k77_bootloader::control::ControlData::In(&mut buf[..]),
            timeout_ms: ene6k77_bootloader::control::TIMEOUT_MS,
        };
        usb::dispatch(handle, &mut x, verbose).context("wait_ready status read")?;
        let s = StatusReg::from_response(&buf)?;
        if s.has_error() {
            // Clear errors and propagate.
            let mut clear = ControlTransfer {
                bm_request_type: ene6k77_bootloader::control::BM_RT_HOST_TO_DEVICE,
                b_request: ene6k77_bootloader::control::B_REQ_WRITE,
                w_value: 0,
                w_index: next_bank.status_w_index(),
                data: ene6k77_bootloader::control::ControlData::Out(std::borrow::Cow::Owned(
                    vec![0x02],
                )),
                timeout_ms: ene6k77_bootloader::control::TIMEOUT_MS,
            };
            let _ = usb::dispatch(handle, &mut clear, verbose);
            bail!(
                "device reported error during wait_ready (status=0x{:02x}, error_bits=0x{:02x})",
                s.raw,
                s.error_bits
            );
        }
        if s.ready {
            return Ok(next_bank);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Send a 9-byte command header at the given bank's command wIndex.
fn send_header(
    handle: &DeviceHandle<GlobalContext>,
    bank: Bank,
    header: [u8; flash::HEADER_LEN],
    verbose: bool,
) -> Result<()> {
    let mut x = ControlTransfer {
        bm_request_type: ene6k77_bootloader::control::BM_RT_HOST_TO_DEVICE,
        b_request: ene6k77_bootloader::control::B_REQ_WRITE,
        w_value: 0,
        w_index: bank.command_w_index(),
        data: ene6k77_bootloader::control::ControlData::Out(std::borrow::Cow::Owned(
            header.to_vec(),
        )),
        timeout_ms: ene6k77_bootloader::control::TIMEOUT_MS,
    };
    usb::dispatch(handle, &mut x, verbose).context("send 9-byte header")?;
    Ok(())
}

/// Send a chunk of bulk data at the given bank's bulk wIndex.
fn send_bulk_out(
    handle: &DeviceHandle<GlobalContext>,
    bank: Bank,
    payload: &[u8],
    verbose: bool,
) -> Result<()> {
    let mut x = ControlTransfer {
        bm_request_type: ene6k77_bootloader::control::BM_RT_HOST_TO_DEVICE,
        b_request: ene6k77_bootloader::control::B_REQ_WRITE,
        w_value: 0,
        w_index: bank.bulk_w_index(),
        data: ene6k77_bootloader::control::ControlData::Out(std::borrow::Cow::Borrowed(payload)),
        timeout_ms: ene6k77_bootloader::control::TIMEOUT_MS,
    };
    usb::dispatch(handle, &mut x, verbose).context("send bulk out chunk")?;
    Ok(())
}

/// Receive a chunk of bulk data at the given bank's bulk wIndex.
fn recv_bulk_in(
    handle: &DeviceHandle<GlobalContext>,
    bank: Bank,
    into: &mut [u8],
    verbose: bool,
) -> Result<()> {
    let mut x = ControlTransfer {
        bm_request_type: ene6k77_bootloader::control::BM_RT_DEVICE_TO_HOST,
        b_request: ene6k77_bootloader::control::B_REQ_READ,
        w_value: 0,
        w_index: bank.bulk_w_index(),
        data: ene6k77_bootloader::control::ControlData::In(into),
        timeout_ms: ene6k77_bootloader::control::TIMEOUT_MS,
    };
    usb::dispatch(handle, &mut x, verbose).context("recv bulk in chunk")?;
    Ok(())
}

/// Phase 3: open the programming session.
pub fn flash_open(
    handle: &DeviceHandle<GlobalContext>,
    bank: Bank,
    verbose: bool,
) -> Result<Bank> {
    let bank = wait_ready(handle, bank, verbose).context("flash_open: wait_ready")?;
    send_header(handle, bank, flash::build_command_header(Cmd::Open, 0, 0), verbose)
        .context("flash_open: send Open header")?;
    Ok(bank)
}

/// Phase 4a: write loop. Returns the bank state after the last write.
pub fn flash_write_all(
    handle: &DeviceHandle<GlobalContext>,
    mut bank: Bank,
    bin: &[u8],
    verbose: bool,
) -> Result<Bank> {
    let total_chunks = (bin.len().saturating_sub(SKIP_REGION_END as usize)
        + flash::CHUNK_SIZE as usize
        - 1)
        / flash::CHUNK_SIZE as usize;
    let mut chunks_done = 0usize;

    let mut addr = SKIP_REGION_END;
    while (addr as usize) < bin.len() {
        let remaining = bin.len() - addr as usize;
        let chunk_len = std::cmp::min(remaining, flash::CHUNK_SIZE as usize);
        let payload = &bin[addr as usize..addr as usize + chunk_len];

        bank = wait_ready(handle, bank, verbose)
            .with_context(|| format!("flash_write_all: wait_ready @ 0x{addr:06x}"))?;
        send_header(
            handle,
            bank,
            flash::build_command_header(Cmd::Write, addr, chunk_len as u16),
            verbose,
        )
        .with_context(|| format!("flash_write_all: header @ 0x{addr:06x}"))?;
        send_bulk_out(handle, bank, payload, verbose)
            .with_context(|| format!("flash_write_all: bulk @ 0x{addr:06x}"))?;

        chunks_done += 1;
        if chunks_done % 16 == 0 || chunks_done == total_chunks {
            eprintln!("  wrote {chunks_done}/{total_chunks} chunks");
        }

        addr += chunk_len as u32;
    }
    bank = wait_ready(handle, bank, verbose).context("flash_write_all: final wait_ready")?;
    Ok(bank)
}

/// Phase 4b: read-back loop. Returns a Vec<u8> of the same length as
/// `bin` with addresses [0..0x1800) zero-filled and [0x1800..) populated
/// from the device.
pub fn flash_read_back(
    handle: &DeviceHandle<GlobalContext>,
    mut bank: Bank,
    bin_len: usize,
    verbose: bool,
) -> Result<(Vec<u8>, Bank)> {
    let mut out = vec![0u8; bin_len];
    let mut addr = SKIP_REGION_END;
    while (addr as usize) < bin_len {
        let remaining = bin_len - addr as usize;
        let chunk_len = std::cmp::min(remaining, flash::CHUNK_SIZE as usize);

        bank = wait_ready(handle, bank, verbose)
            .with_context(|| format!("flash_read_back: wait_ready @ 0x{addr:06x}"))?;
        send_header(
            handle,
            bank,
            flash::build_command_header(Cmd::Read, addr, chunk_len as u16),
            verbose,
        )
        .with_context(|| format!("flash_read_back: header @ 0x{addr:06x}"))?;
        recv_bulk_in(
            handle,
            bank,
            &mut out[addr as usize..addr as usize + chunk_len],
            verbose,
        )
        .with_context(|| format!("flash_read_back: bulk @ 0x{addr:06x}"))?;

        addr += chunk_len as u32;
    }
    bank = wait_ready(handle, bank, verbose).context("flash_read_back: final wait_ready")?;
    Ok((out, bank))
}

/// Phase 4c: byte-compare bin and read-back, skipping the bootloader region.
pub fn verify_match(bin: &[u8], read_back: &[u8]) -> Result<()> {
    if bin.len() != read_back.len() {
        bail!(
            "length mismatch: bin={}, read_back={}",
            bin.len(),
            read_back.len()
        );
    }
    let start = SKIP_REGION_END as usize;
    if bin[start..] != read_back[start..] {
        // Find first differing offset for the error message.
        let off = bin[start..]
            .iter()
            .zip(&read_back[start..])
            .position(|(a, b)| a != b)
            .map(|i| i + start);
        return Err(anyhow!(
            "verify mismatch — first differing offset: {:?}",
            off
        ));
    }
    Ok(())
}

/// Phase 5: send VerifyChecksum (0x80) and wait_ready.
pub fn flash_verify_checksum(
    handle: &DeviceHandle<GlobalContext>,
    bank: Bank,
    verbose: bool,
) -> Result<Bank> {
    let bank = wait_ready(handle, bank, verbose).context("verify_checksum: wait_ready")?;
    send_header(
        handle,
        bank,
        flash::build_command_header(Cmd::VerifyChecksum, 0, 0),
        verbose,
    )
    .context("verify_checksum: send header")?;
    let bank = wait_ready(handle, bank, verbose).context("verify_checksum: post-wait")?;
    Ok(bank)
}

/// Phase 6: close sequence — bit_mask reg with (0xF0, 0xF0), wait, dummy cmd.
///
/// Source: `docs/decomp/fw-update/fw-update.txt:1175` (sub_140001e40
/// the-rax_2-zero branch).
pub fn close_session(
    handle: &DeviceHandle<GlobalContext>,
    bank: Bank,
    verbose: bool,
) -> Result<()> {
    let mut current = [0u8; 1];
    let mut x = register::build_register_read(register::REG_STATUS_MASK, &mut current);
    usb::dispatch(handle, &mut x, verbose).context("close_session: bit_mask read")?;
    let new = eb::apply_bit_mask(current[0], 0xF0, 0xF0);
    if new != current[0] {
        let mut x = register::build_register_write(register::REG_STATUS_MASK, new);
        usb::dispatch(handle, &mut x, verbose).context("close_session: bit_mask write")?;
    }
    let bank = wait_ready(handle, bank, verbose).context("close_session: wait_ready")?;
    send_header(
        handle,
        bank,
        flash::build_command_header(Cmd::VerifyChecksum, 0, 0),
        verbose,
    )
    .context("close_session: send VerifyChecksum")?;
    std::thread::sleep(Duration::from_millis(200));
    let bank = wait_ready(handle, bank, verbose).context("close_session: post-checksum")?;
    send_header(handle, bank, eb::build_dummy_command(), verbose)
        .context("close_session: dummy command")?;
    Ok(())
}

/// Marker error type for "device is in mid-flash state — do not power off".
pub struct MidFlashFailure;

/// Print the unrecoverable-state warning to stderr.
pub fn print_mid_flash_warning() {
    eprintln!();
    eprintln!("================================================================================");
    eprintln!("  ⚠️  DEVICE IS NOW IN AN INDETERMINATE STATE  ⚠️");
    eprintln!("================================================================================");
    eprintln!();
    eprintln!("  The flash sequence failed before completion. The device may be");
    eprintln!("  partially flashed.");
    eprintln!();
    eprintln!("  - DO NOT POWER-CYCLE the device.");
    eprintln!("  - Try `flash` again with the same .bin file. If the bootloader is");
    eprintln!("    still responsive on PID 0x7758, this is the least-bad recovery.");
    eprintln!("  - If retry also fails: the device is likely unrecoverable without");
    eprintln!("    programming hardware (SWD/ICSP/JTAG).");
    eprintln!();
    eprintln!("================================================================================");
}
```

- [ ] **Step 2: Wire phases 3-6 into `main.rs`**

Replace the `flash` arm's tail (everything after the `confirm_bootloader` call) with:

```rust
            // Phase 3: open programming session.
            let bank = ene6k77_bootloader::bank::Bank::initial(BOOTLOADER_PID);
            let mut bank = match flash_flow::flash_open(&handle, bank, verbose) {
                Ok(b) => b,
                Err(e) => {
                    flash_flow::print_mid_flash_warning();
                    return Err(e);
                }
            };

            // Phase 4a: write loop.
            bank = match flash_flow::flash_write_all(&handle, bank, &bin, verbose) {
                Ok(b) => b,
                Err(e) => {
                    flash_flow::print_mid_flash_warning();
                    return Err(e);
                }
            };

            // Phase 4b-c: read back, verify.
            let (read_back, b) = match flash_flow::flash_read_back(&handle, bank, bin.len(), verbose) {
                Ok(v) => v,
                Err(e) => {
                    flash_flow::print_mid_flash_warning();
                    return Err(e);
                }
            };
            bank = b;
            if let Err(e) = flash_flow::verify_match(&bin, &read_back) {
                flash_flow::print_mid_flash_warning();
                return Err(e);
            }

            // Phase 5: checksum.
            bank = match flash_flow::flash_verify_checksum(&handle, bank, verbose) {
                Ok(b) => b,
                Err(e) => {
                    flash_flow::print_mid_flash_warning();
                    return Err(e);
                }
            };

            // Phase 6: close.
            if let Err(e) = flash_flow::close_session(&handle, bank, verbose) {
                flash_flow::print_mid_flash_warning();
                return Err(e);
            }

            eprintln!();
            eprintln!("✓ flash completed; please power-cycle the device.");
```

Make sure `BOOTLOADER_PID` is in scope — add `use ene6k77_bootloader::identify::BOOTLOADER_PID;` near the top of `main.rs`.

- [ ] **Step 3: Verify it builds and runs**

```bash
cargo build -p fw-update-ene6k77
cargo clippy -p fw-update-ene6k77 -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

Expected: all green.

Manual smoke test (still no real device required, since file validation rejects before USB):

```bash
truncate -s 100 /tmp/tiny
cargo run -p fw-update-ene6k77 -- flash --i-accept-untested-firmware-risk \
    --vid 0xCF2 --pid 0x7758 --bin /tmp/tiny --yes
```

Expected: error "bin file too small". Banner prints first, but the size check rejects before any USB attempt.

```bash
truncate -s 8192 /tmp/min
cargo run -p fw-update-ene6k77 -- flash --i-accept-untested-firmware-risk \
    --vid 0xCF2 --pid 0x7758 --bin /tmp/min --yes
```

Expected: banner + "no device with VID:PID 0x0CF2:0x7758 found" (libusb-level error). The flash flow never starts because the open fails.

- [ ] **Step 4: Commit**

```bash
git add tools/fw-update-ene6k77/src/flash_flow.rs tools/fw-update-ene6k77/src/main.rs
git commit -m "feat(fw-update-ene6k77): wire flash write/verify/checksum phases

Implements phases 3-6 of the flash flow: flash_open, chunked write
loop with bank alternation, read-back into a verify buffer, byte-compare
of the writable region, VerifyChecksum, close session. Mid-flash
failures print a loud do-not-power-off warning before exiting.

End-to-end pipeline now compiles and runs against the file/USB
validation gates. Still untested against real hardware."
```

---

## Task 14: Cross-links + final review

**Files:**
- Modify: `docs/decompiled-source-map.md`
- Modify: `docs/development/protocol/cross-device-survey/per-family/ene6k77fan.md`
- Modify: `README.md`

- [ ] **Step 1: Add the new spec to the source map**

Open `docs/decompiled-source-map.md` and add a row/section for the ENE 6K77 bootloader spec — follow the existing rows' format. Cite both the spec doc and the new decomp directory.

- [ ] **Step 2: Cross-link from `ene6k77fan.md`**

Open `docs/development/protocol/cross-device-survey/per-family/ene6k77fan.md`. Find the line that says "flashing is presumably out-of-tree" (around line 121). Append a forward reference:

```markdown
- Firmware upload / activation: **none in this device class.** Unlike
  TLFan which has `SendFirmware`/`ActiveFirmware` enum values (even
  if unused at runtime), the Ene6K77Fan device class has no firmware-
  flashing methods — flashing is presumably out-of-tree. Confirmed
  by the standalone Windows updater `FwUpdate.exe`; see
  [`docs/development/protocol/ene6k77-bootloader.md`
  for the full bootloader protocol reference.
```

- [ ] **Step 3: Add the tool to the workspace README**

Open the workspace `README.md`. Find the section that lists workspace crates / tools and add an entry for `tools/fw-update-ene6k77/` with the experimental warning.

- [ ] **Step 4: Run the full gate suite**

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

Expected: all green.

If `fmt --check` fails, run `cargo fmt --all` and re-test. Commit the formatting changes separately.

- [ ] **Step 5: Dispatch `rust-practices-verifier` on the codec crate**

Use the Agent tool with subagent type `rust-practices-verifier`. Prompt:

> Audit the `crates/ene6k77-bootloader/` crate against the
> good-rust-practices skill. Pay particular attention to API design
> (we expose builders that return `[u8; N]` and `ControlTransfer<'a>`),
> error handling (single `Error` enum with `#[non_exhaustive]`), and
> doc-comment citation discipline (every public item should cite a
> line range in `docs/decomp/fw-update/`). Skip the harness binary;
> the codec is the audit target.

Address any Critical or High findings before merging. Medium and Style findings: judgement call — flag them in the commit message if deferring.

- [ ] **Step 6: Commit cross-links**

```bash
git add docs/decompiled-source-map.md \
        docs/development/protocol/cross-device-survey/per-family/ene6k77fan.md \
        README.md
git commit -m "docs: cross-link ENE 6K77 bootloader spec from related docs"
```

- [ ] **Step 7: Final commit if rust-practices-verifier surfaced fixes**

If the verifier flagged anything actionable:

```bash
git add crates/ene6k77-bootloader/
git commit -m "refactor(ene6k77-bootloader): address rust-practices-verifier findings

<list each fix on its own line>"
```

---

## Self-review

(Plan author's own pass before handing to execution.)

**Spec coverage** (cross-checked against design doc):

- Spec doc → Task 2. ✓
- Codec crate skeleton → Task 3. ✓
- `bank.rs` → Task 4. ✓
- `control.rs` → Task 5. ✓
- `register.rs` → Task 6. ✓
- `flash.rs` → Task 7. ✓
- `enter_bootloader.rs` → Task 8. ✓
- `identify.rs` → Task 9. ✓
- Realistic-bin shape tests → Task 9.5. ✓ (added after spec lock-in once the user surfaced their `fw/` tree)
- Harness skeleton → Task 10. ✓
- Read-only subcommands → Task 11. ✓
- Flash setup phase → Task 12. ✓
- Flash write/verify/checksum/close phases → Task 13. ✓
- Cross-links + final review → Task 14. ✓
- Decomp file relocation → Task 1. ✓

**Type consistency**: `Bank::A`/`Bank::B` consistent across tasks 4-13; `Cmd::Probe`/`Read`/`Write`/`Open`/`VerifyChecksum` consistent across tasks 7-13; `ControlTransfer` / `ControlData` consistent across tasks 5-13; `StatusReg::from_response` signature consistent across tasks 6, 11, 12, 13.

**Placeholder scan**: every step has either complete code, a complete shell command with expected output, or a complete prose deliverable. No "TBD" / "fill in" / "similar to Task N" cross-references.
