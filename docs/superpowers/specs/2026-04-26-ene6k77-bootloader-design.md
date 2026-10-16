# ENE 6K77xx Bootloader — Design

- **Status:** approved (brainstorming complete) — ready for implementation plan
- **Date:** 2026-04-26
- **Author:** brainstorming session with user
- **Scope:** reverse-engineer Lian-Li's `FwUpdate.exe` (the Windows-only
  firmware updater for the ENE 6K77 fan-controller family) into a Linux
  protocol spec, a pure-codec Rust crate, and an experimental CLI harness.
- **Hardware status:** none. Spec and codec are static-analysis-only;
  the harness will be untested until someone with a target device runs it.
- **Related:** complements `docs/protocol/cross-device-survey/per-family/ene6k77fan.md`,
  which covers the same chip family's HID *application* protocol and
  flagged the bootloader half as "presumably out-of-tree" — this design
  fills that gap.

---

## 1. Context

The L-Connect 3 Windows install ships a tiny standalone CLI updater,
`FwUpdate.exe` (~10 KB, x86_64 PE, VS2022, links `libusb-1.0.dll`),
that flashes firmware to the ENE 6K77xx-class fan controllers
(SLFan / ALFan / SLInfinity / SLV2 / ALV2 / SLV2A / SLRedragon —
VID `0x0CF2`, PIDs `0xA100..0xA106`). Unlike the rest of the L-Connect
codebase, which talks to these devices via HID feature/output reports
(see `cross-device-survey/per-family/ene6k77fan.md`), the updater drops
the device into a separate USB personality (PID `0x7758`) and drives
the flash via raw libusb vendor control transfers.

The user has BinaryNinja decompiler outputs for this binary
(`fw-update.txt` for pseudo-C, `fw-update-hlil.txt` for HLIL, both at
the parent directory's repo root) plus the binary itself at
`winver/L-Connect 3/FwUpdate.exe`. There is no PDB; the
`D:\Work\fw-update\x64\Release\FwUpdate.pdb` string embedded in the PE
is debug metadata that confirms identity but is not a file we hold.

Linux users currently have no path to update firmware on these devices.
Even with this design landed, they still won't have a *validated* path —
but they will have:

1. A complete protocol reference document for whoever picks the work up later.
2. A no-I/O codec crate that locks down the wire format in tested Rust.
3. A working harness binary that anyone with hardware can take a chance on.

The harness is explicitly experimental. The chip family is "fan
controller", individual SKUs are inexpensive, and recovery from a
botched flash plausibly requires programming hardware (SWD/ICSP/JTAG).
Posting an unvalidated flasher carries real bricking risk; the design
treats that risk as central, not peripheral.

## 2. Scope

### In scope

- **`docs/protocol/ene6k77-bootloader.md`** — protocol reference doc,
  same shape and citation discipline as `galahad2-lcdv2.md`. Covers
  both USB personalities, the bank-A/bank-B alternation, the two
  register namespaces, the 9-byte command-header layout, and the
  full enter-bootloader → write → verify-checksum sequence.
- **`crates/ene6k77-bootloader/`** — new workspace member. Pure codec,
  `#![forbid(unsafe_code)]`, no `rusb`/`hidapi`/async. Mirrors the
  `crates/protocol/` conventions: `build_*` functions for requests,
  `<Type>::from_response` for responses, `crate::Result<T>` errors.
- **`tools/fw-update-ene6k77/`** — experimental binary. Wires the
  codec to `rusb`. Implements the full app-mode → bootloader → flash
  → verify pipeline. Heavily gated by acknowledgement flags and
  warning banners. Functional mirror of `FwUpdate.exe`, written
  cleanly in Rust from the protocol spec rather than transliterated
  from decompiler output.
- Commit `fw-update.txt` and `fw-update-hlil.txt` to the repo at a
  documented path so the spec doc's `:line-range` citations resolve
  for future readers.

### Out of scope

- Live verification against any device.
- Sourcing or distributing firmware `.bin` files.
- Daemon integration. The bootloader codec is independent of any
  Phase 1 daemon work; the harness binary is standalone.
- The HID application-mode protocol for this family — already covered
  in `cross-device-survey/per-family/ene6k77fan.md`; cross-link only.
- A udev rule for the bootloader PID. Mentioned in the harness's
  README as a follow-on but not bundled in this work.
- Other Lian-Li firmware-update paths. The shipped firmware files
  fall into two distinct families:
  - **Older / fan-controller class** (ENE 6K77xx) — flashed by
    `FwUpdate.exe` (this spec) or its GUI sibling
    `UsbISPDialog_v.1.0.1.18.exe` in the same toolkit. Filenames
    in the `LianLi_FwUpdate-tools` zip all start with
    `6K77xx_LLUF_*` / `6K77XX_LLUF-*`; sizes 41-78 KB.
  - **Newer / LCD-class** (Galahad II, HydroShift, etc.) — flashed
    in-app through L-Connect 3 itself, almost certainly via the
    same `BCommandPacket` multi-packet HID write path the LCDV2
    JPEG/H.264 commands use. Filenames sit alongside the zip, not
    inside it; sizes ~600 KB. `pid7395` (Galahad II Vision) and
    `pid739A` (HydroShift) are the observed examples.

  This spec covers only the first family. The second family is a
  separate decompile/spec/codec exercise (target enum location:
  `LConnectCore/Products/Galahad2Vision/...` for the request
  byte; TLFan already has `SendFirmware`/`ActiveFirmware` enum
  entries that probably hint at the shape).

### Future direction (informational, not in scope here)

The eventual goal is a single Linux flasher tool that handles both
families behind one CLI — pick a target device, pick a `.bin`, the
tool routes to either the bootloader-mode vendor-control path (this
codec) or the in-app HID multi-packet path (future codec). The
codec crate produced here will become one of two siblings under a
shared `lconnect-firmware` umbrella; the harness binary produced
here is a stepping stone, not the final UX. Designing now in a way
that doesn't paint that future into a corner: the codec stays
chip-named (`ene6k77-bootloader`, not `lconnect-firmware-ene`) so a
future Galahad codec can sit next to it as a peer rather than
needing to invade its namespace.

### Explicitly *not* providing

- An "enter bootloader without flashing" diagnostic subcommand.
  Attractive nuisance; leaves devices stuck.
- Automatic retry on mid-flash failure. Increases the chance of
  making a half-flashed device worse.
- A `--dry-run` default. The user explicitly asked for a usable tool
  with warnings, not one walled behind dry-run.

## 3. The protocol — summary for this doc

Full citations live in `docs/protocol/ene6k77-bootloader.md`. Summary:

**Two USB personalities** on VID `0x0CF2`:

| Mode | PID | Transport | Used for |
|---|---|---|---|
| Application | `0xA100..0xA106` (per SKU) | HID feature/output reports | Normal operation, see `ene6k77fan.md` |
| Bootloader | `0x7758` | libusb vendor control transfers | Firmware update only |

**Wire-level conventions**:

- `bmRequestType = 0x40` (host→device) or `0xC0` (device→host); the
  `0xf040`/`0xf080` values BinaryNinja shows are register-reuse
  artefacts of the Win64 calling convention — only the low byte
  reaches the wire.
- `bRequest = 0x80` (write) or `0x81` (read).
- `wValue = 0` always.
- `wIndex` is the operation selector (see below).
- Multi-byte fields in payloads are big-endian.
- Standard timeout is 1000 ms (decimal).

**Two register namespaces**, both selected via `wIndex`:

1. **Control registers** (`0x600..0x606`) — single-byte read/write.
   `0x600` is mode/control, `0x601..0x604` participate in a 5-step
   WDT manipulation, `0x606` is a status/mask register read with a
   separate "clear status by writing back" path.

2. **Flash protocol** (`0xF000..0xF1FF`) — paired bank-A/bank-B
   addresses for command, data, and status:

   | Operation | Bank A wIndex | Bank B wIndex |
   |---|---|---|
   | Status poll (1 byte read) | `0xF0F9` | `0xF1F9` |
   | 9-byte command header | `0xF0F1` | `0xF1F1` |
   | Bulk data (≤ 128 B for write, 16 KB for confirm-read) | `0xF000` | `0xF100` |

   The host alternates between banks on every operation, presumably
   so the chip can be processing one slot while the host is filling
   the other. Initial bank is set by the open path (cold-open at
   PID `0x7758` starts at bank B; transition from app mode starts
   at bank A) and toggled by every status-poll call.

**The 9-byte command header** (sent at wIndex `0xF0F1` / `0xF1F1`):

```
offset  size  field
------  ----  -----
  0      1   address byte 2 (high)
  1      1   address byte 1 (mid)
  2      1   address byte 0 (low)
  3      1   reserved (always 0)
  4      1   reserved (always 0)
  5      1   length byte 1 (high)
  6      1   length byte 0 (low)
  7      1   cmd byte
  8      1   reserved (always 0)
```

`cmd byte` observed values:

| Value | Inferred meaning | Where emitted |
|---|---|---|
| `0x01` | Dummy / probe (no payload) | Enter-bootloader handshake |
| `0x30` | Read flash (followed by data on bulk wIndex) | Read-back / verify pass |
| `0x40` | Write flash (followed by data on bulk wIndex) | Main flash loop |
| `0x70` | Open programming session | After bootloader confirm |
| `0x80` | Verify checksum / finalise | End of flash, twice (mid + close) |

**The `0x1800` skip region**. Both the write loop and the read-back
loop in the original tool gate every flash operation with
`if (offset >= 0x1800)`. The first 6 KB of any `.bin` is therefore
never written to or compared against. This is a hard invariant — the
region almost certainly is the on-chip bootloader's own code, and
overwriting it would brick the device. The spec documents this
clearly; the codec exposes a helper but does not silently apply the
skip; the harness applies the skip in its main loop.

## 4. Spec doc — `docs/protocol/ene6k77-bootloader.md`

Structured to match `galahad2-lcdv2.md`. Frontmatter establishes
device family, USB IDs, source-doc map, status legend, and one
prominent line stating that **every entry is `inferred`** —
static-analysis only, no live capture, no hardware. That single
declaration replaces per-section status flags.

Sections in order:

1. **Source materials** — points at `winver/L-Connect 3/FwUpdate.exe`
   and the two BinaryNinja outputs at parent-repo root, plus a brief
   note on the embedded PDB string for binary-identity confirmation.
   Establishes the citation convention (`fw-update.txt:NNNN-NNNN`
   for pseudo-C, `fw-update-hlil.txt:NNNN-NNNN` for HLIL — drawing
   from whichever output makes the point clearer per section).

2. **Two USB personalities** — table of app-mode PIDs per SKU
   (cross-referenced to `KnownDevices.cs` line ranges from
   `ene6k77fan.md`), bootloader PID `0x0CF2:0x7758`, the iManufacturer/
   iProduct ASCII descriptor substring filter (`"LianLi"` or
   `"6K7750"`), and a dismissal of the Windows-specific
   `MI_00 DeviceInterfaceGUIDs` error path as a Linux non-issue.

3. **Wire-level conventions** — bmRequestType / bRequest / wValue /
   wIndex / endianness / timeout. Calls out the `0xf040`/`0xf080`
   HLIL artefact and explains why the wire byte is `0x40`/`0x80`.

4. **Bank-A / bank-B alternation** — the table of `wIndex` pairs,
   where the initial bank comes from, where it toggles, why it
   exists (likely a double-buffered command queue on the chip).

5. **Control register namespace** (`0x600..0x606`) — table of
   registers used and their semantics as observed: known values,
   read/write patterns, the `0x606` read-modify-write mask
   convention. Each row cites the line range that authorises it.

6. **9-byte command header layout** — the table from §3 above,
   expanded with citations to each call site for each `cmd byte`.

7. **Status register format** — bit 1 = ready, bits `0xF0` = error
   mask, error-clear procedure (write `0x02` back). Cited to the
   wait-ready function in the decomp.

8. **Enter-bootloader sequence** — step-by-step prose with the
   exact register writes in order, the dummy command, the 3-second
   polling loop, the device re-enumeration as PID `0x7758`.

9. **Flash write sequence** — the open → chunked write → wait-ready
   → re-read → memcmp → checksum → close pipeline. Calls out the
   `0x1800` skip region as a hard invariant. Notes the 128-byte
   write chunk size and the 16 KB bootloader-confirmation read.

10. **Open questions / unverified** — explicit list of guesses that
    only hardware can settle: cmd-byte semantics, WDT sequence
    behaviour, bit-mask value meanings, whether `0x1800` is exactly
    the bootloader-region size, whether bank alternation is required
    or just incidental, descriptor-string coverage across SKUs.

11. **Cross-references** — back to `ene6k77fan.md`, forward to the
    codec crate and the harness binary.

**Discipline**: no verbatim block of more than ~5 contiguous lines
from either decompiler output. Citations and behavioural prose in
the doc author's words. Short snippets where they anchor a single
specific claim (e.g., a single-line bit pattern) are fine.

## 5. Codec crate — `crates/ene6k77-bootloader/`

### Cargo manifest

- `name = "ene6k77-bootloader"`
- `edition = "2021"`, `rust-version = "1.82"` (workspace MSRV)
- `description` includes the "untested codec" caveat
- One dep: `thiserror`
- `[lints]` matches the existing protocol crate (clippy denials etc.)

### Module layout

```
src/
  lib.rs              — Result<T>, Error enum, re-exports, top-level
                         doc-comment establishing static-analysis-only
                         provenance and listing the unverified claims.
  bank.rs             — Bank enum (A/B), w_index lookup tables per
                         (operation, bank) pair, toggle helper.
  control.rs          — bmRequestType / bRequest constants, timeout.
                         The ControlTransfer struct lives here.
  register.rs         — Register address constants (0x600..0x606),
                         build_register_read / build_register_write,
                         StatusReg decoder (bit 1 = ready, mask 0xF0
                         = error), bit-mask read-modify-write helper.
  flash.rs            — 9-byte command header builder, Cmd enum
                         (Probe = 0x01, Read = 0x30, Write = 0x40,
                         Open = 0x70, VerifyChecksum = 0x80),
                         CHUNK_SIZE = 0x80, SKIP_REGION_END = 0x1800,
                         is_writable_address(addr) helper.
  enter_bootloader.rs — build_sequence() returning a
                         Vec<ControlTransfer<'static>> for the
                         disable_wdt → bit_mask → wdt_reset →
                         dummy-cmd → status-poll handshake.
  identify.rs         — SkuName enum (Sl, Al, Si, S2, A2, St),
                         pid_for_sku, pid_for_sku_str, BOOTLOADER_PID,
                         DESCRIPTOR_NEEDLES (the substring filter).
```

### Public API surface

Three patterns side by side:

1. **Pure builders** for one-shot transfers — return either a
   `ControlTransfer<'a>` (for full transfer descriptors) or `[u8; N]`
   (for the 9-byte header that gets passed as the data field of a
   transfer). Every builder is a pure function, no allocation
   beyond the return value.

2. **Decoders** for one-shot reads — `<Type>::from_response(buf: &[u8])
   -> Result<Self>`. Validate length, return typed structs.

3. **Sequence emitters** for fixed-shape multi-step operations
   (currently just the enter-bootloader handshake) — return
   `Vec<ControlTransfer<'static>>`. The harness iterates and
   dispatches; the codec never touches I/O.

### `ControlTransfer` type

Plain struct, no `rusb` dependency:

```rust
pub struct ControlTransfer<'a> {
    pub bm_request_type: u8,    // 0x40 or 0xC0
    pub b_request: u8,          // 0x80 or 0x81
    pub w_value: u16,           // always 0 in this protocol
    pub w_index: u16,           // bank-selected via Bank
    pub data: ControlData<'a>,  // In(&'a mut [u8]) | Out(Cow<'a, [u8]>)
    pub timeout_ms: u32,
}

pub enum ControlData<'a> {
    In(&'a mut [u8]),       // device-to-host: caller-provided buffer
    Out(Cow<'a, [u8]>),     // host-to-device: borrowed slice or owned
}
```

`Cow<'a, [u8]>` on the out side lets sequence emitters return owned
9-byte buffers without forcing the caller to keep them alive
separately, while one-shot builders that take a caller-owned slice
(like the bulk-data write) can borrow.

### Error enum

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("response too short: expected {expected}, got {got}")]
    ResponseTooShort { expected: usize, got: usize },
    #[error("invalid status byte: 0x{0:02x}")]
    InvalidStatusByte(u8),
    #[error("address 0x{addr:06x} is in the bootloader skip region (< 0x1800)")]
    AddressInSkipRegion { addr: u32 },
    #[error("length {len} overflows chunk size {max}")]
    LengthOverflowsChunk { len: u16, max: u16 },
    #[error("unknown command byte: 0x{0:02x}")]
    UnknownCmd(u8),
}

pub type Result<T> = std::result::Result<T, Error>;
```

No I/O variants — those belong to the harness's `anyhow` chain.

### Doc-comment discipline

Every public item carries a `## Source` rustdoc block citing the
line range of `fw-update.txt` and/or `fw-update-hlil.txt` that
authorises it. Mirrors how the existing `crates/protocol/` cites
the C# source files. `cargo doc --workspace --no-deps` is part of
the CI gate so broken citations or rustdoc warnings fail the build.

`lib.rs` opens with a top-level doc-comment that:

- States the codec is derived from static analysis of `FwUpdate.exe`
- Lists the unverified claims (cmd-byte interpretations, WDT
  semantics, skip-region size, bank-alternation requirement)
- Points at the spec doc and the harness binary
- Carries the same untested/may-brick warning text as the harness

### What the crate does *not* contain

- No `flash_image()` or other end-to-end orchestrator function.
  Whoever writes the harness assembles the steps, so they have to
  read the spec and own the bricking risk explicitly.
- No bank-alternation state machine. `Bank` is an input parameter;
  toggling lives with the caller.
- No `0x1800` skip enforcement on writes — only an
  `is_writable_address` predicate the caller chooses to apply (or
  not). The harness applies it; tests confirm it returns the right
  values.
- No I/O, no `rusb`, no async, no logging.

## 6. Harness binary — `tools/fw-update-ene6k77/`

### Cargo manifest

- `name = "fw-update-ene6k77"`
- Same edition / MSRV as the rest of the workspace
- `description` is the runtime warning text (becomes part of `--help`)
- Deps: `ene6k77-bootloader` (workspace path), `rusb`, `clap` with
  `derive`, `anyhow`

### CLI shape (clap derive)

```
fw-update-ene6k77 <subcommand> [--verbose]

  list                   Enumerate ENE 6K77 devices on the bus.
                         Read-only. No risk-acceptance flag needed.

  inspect --vid 0xNNNN --pid 0xNNNN
                         Read status registers of a device. Read-only,
                         no enter-bootloader sequence triggered.
                         No risk-acceptance flag needed.

  flash --i-accept-untested-firmware-risk
        --vid 0xNNNN
        --pid 0xNNNN
        --bin path/to/firmware.bin
        [--yes]          Skip the interactive prompt (the risk flag
                         is still mandatory).
```

The risk-acceptance flag is the literal long phrase
`--i-accept-untested-firmware-risk` — no short alias, no env-var
equivalent, no config-file fallback. The user types it on the
command line every invocation.

### `flash` subcommand data flow

1. Parse args. Missing risk flag → clap-level error with the warning
   banner re-printed.
2. Validate the file exists, is readable, and is at least `0x1800`
   bytes long (smaller can't be a valid image — the first 6 KB is
   the bootloader region).
3. Print runtime banner: untested, may permanently brick, no
   recovery without programming hardware.
4. Unless `--yes` was given: prompt `Type "yes" to proceed: `. Only
   the literal lowercase word `yes` advances; anything else aborts.
5. `rusb::open_device_with_vid_pid` → `claim_interface(0)`.
6. If PID is in the app-mode range
   (`identify::is_app_mode_pid(pid)`): iterate the
   `Vec<ControlTransfer>` from
   `enter_bootloader::build_sequence()`, dispatching each via
   `rusb::DeviceHandle::write_control` /
   `read_control`. Then `sleep(200ms)`, re-enumerate at PID
   `0x7758`, re-claim.
7. Confirm bootloader: 1-byte read at wIndex `0x4000`. Failure →
   abort, device left in bootloader.
8. flash_open: send 9-byte cmd-`0x70` header, then wait_ready (poll
   status reg with bank alternation).
9. Read the `.bin` into `Vec<u8>`.
10. Write loop: for `addr in (0x1800..bin.len()).step_by(0x80)`,
    call wait_ready (which alternates bank), send 9-byte cmd-`0x40`
    header at the resulting bank, send the up-to-128-byte data chunk
    at the same bank's bulk-data wIndex. Progress to stderr every
    chunk. (The original tool iterates from `0x0` and gates this
    block with `if (addr >= 0x1800)`; starting at `0x1800` directly
    is the same operation with the no-op iterations elided.)
11. Read-back loop into a `Vec<u8>` of equal size to `bin`,
    populated only at addresses ≥ `0x1800`.
12. `bin[0x1800..] == verify[0x1800..]` check; mismatch →
    `Error::Mismatch { offset, expected, got }`.
13. flash_verify_checksum: send 9-byte cmd-`0x80` header, wait_ready.
14. Close sequence: bit_mask register `0x606` with mask `0xF0`,
    value `0xF0`, wait_ready, send 9-byte cmd-`0x80` header, sleep
    200 ms, dummy-cmd header, release / exit.
15. Print "flash completed; please power-cycle the device".

### Error handling at the harness layer

- All errors use `anyhow::Result<()>` with `.context("step name")`.
- Failure between step 8 and step 14 prints a loud, multi-line
  message to stderr stating the device is in an indeterminate state,
  not to power-cycle it, and that retrying the same `.bin` is the
  least-bad recovery; if that fails, the device may need
  programming-hardware recovery. Exits non-zero.
- **No automatic retry.** Making a half-flashed device worse is the
  failure mode to avoid.

### Logging

- Default: progress to stderr — chunk index out of total, percent.
- `--verbose`: every `ControlTransfer` printed via a
  `Display` impl on the codec's struct
  (`bmRT/bReq/wIdx/data-len/direction`). Useful for postmortem if
  someone does brick a device.

### Safety wall — warning placements

1. `Cargo.toml` description (visible in `cargo metadata` and on any
   future crates.io / lib.rs page).
2. Top of `main.rs` doc-comment.
3. Top of `README.md` — fenced warning block, plus the SKU→PID
   table for reference and the recovery caveat.
4. Clap `about` string — abbreviated form before usage in `--help`.
5. Runtime banner before step 4 of the `flash` flow.
6. Any non-`list`/`inspect` subcommand without the risk flag exits
   with the full warning re-printed.

## 7. Testing strategy

### Discipline

Every "expected" byte sequence in the codec tests is derived from
the decompiled binary, not observed on a live device. These are
**lock-the-layout** tests, not **prove-correctness** tests. They
catch refactors that change the wire shape; they cannot catch an
upstream misreading of the decomp. A banner in the test directory's
top-level doc-comment states this clearly.

### Test layout

`crates/ene6k77-bootloader/tests/` mirrors the existing
`crates/protocol/tests/` convention — one file per logical
operation:

```
ene6k77_bank_alternation.rs           — Bank::toggle, w_index tables
ene6k77_register_read_write.rs        — bmRT/bReq/wIdx for 0x600..0x606
ene6k77_status_decode.rs              — StatusReg parsing
ene6k77_flash_command_header.rs       — 9-byte layout per cmd byte
ene6k77_flash_address_invariants.rs   — 0x1800 skip-region helper
ene6k77_enter_bootloader_sequence.rs  — ordered ControlTransfer Vec
ene6k77_identify.rs                   — SKU strings, descriptor needles
```

### Per-test discipline

- `build_request_emits_expected_layout` — assert exact bytes at
  known offsets, length, header values.
- `parse_known_good_decodes_fields` — synthetic buffer assembled
  from constants.
- `parse_too_short_returns_response_too_short` — pass `&[]` and
  shorter-than-expected slices.
- `reject_wrong_command_echo` (where applicable).

### Breaking the circularity

Each test file includes one **byte-fingerprint test** that
hard-codes the expected output as a literal `[u8; N]` array, with a
comment citing the exact line of `fw-update.txt` or
`fw-update-hlil.txt` it was transcribed from. Transcribing from the
decomp output, not from the codec's own constants, breaks the
circularity: a typo in the codec's constants fails the fingerprint
test even if the rest of the test file is internally consistent.

### Doc-tests

Every public builder carries a `///` example showing the call and
asserting one or two output bytes. Doubles as usage docs in
`cargo doc`.

### Not doing

- No property testing (`proptest`/`quickcheck`). The codec is
  fixed-size byte builders; property tests would add mass without
  confidence.
- No mock-USB integration testing of the harness binary. Mocking
  the libusb layer would test our mock more than the code. The
  harness is small enough that manual verification is appropriate.

### CI gates

```
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

After implementation lands: dispatch `rust-practices-verifier` on
the codec crate per ambient project policy.
`hid-safety-reviewer` doesn't strictly apply (different transport,
different chip family) but running it on the harness's I/O paths
isn't a bad idea.

### What testing explicitly cannot validate

Listed in `lib.rs`'s top-level doc-comment, the spec doc's "Open
questions" section, and the harness's runtime banner:

- Whether the `0x40`/`0x30` cmd-byte interpretation as "write/read"
  is correct.
- Whether the WDT 5-step sequence puts the device in bootloader vs
  hangs it.
- Whether `0x1800` is exactly the bootloader-region size or
  coincidence.
- Whether bank alternation is required by the firmware or just
  incidental.
- Whether the `0xE1`/`0xF3` bit-mask values mean what we think they
  mean.
- Whether the descriptor-string substring filter (`"LianLi"` /
  `"6K7750"`) covers all SKU variants.

## 8. File / commit layout

New files:

```
docs/protocol/ene6k77-bootloader.md                            (spec)
docs/superpowers/specs/2026-04-26-ene6k77-bootloader-design.md (this doc)
crates/ene6k77-bootloader/Cargo.toml
crates/ene6k77-bootloader/src/lib.rs
crates/ene6k77-bootloader/src/bank.rs
crates/ene6k77-bootloader/src/control.rs
crates/ene6k77-bootloader/src/register.rs
crates/ene6k77-bootloader/src/flash.rs
crates/ene6k77-bootloader/src/enter_bootloader.rs
crates/ene6k77-bootloader/src/identify.rs
crates/ene6k77-bootloader/tests/ene6k77_bank_alternation.rs
crates/ene6k77-bootloader/tests/ene6k77_register_read_write.rs
crates/ene6k77-bootloader/tests/ene6k77_status_decode.rs
crates/ene6k77-bootloader/tests/ene6k77_flash_command_header.rs
crates/ene6k77-bootloader/tests/ene6k77_flash_address_invariants.rs
crates/ene6k77-bootloader/tests/ene6k77_enter_bootloader_sequence.rs
crates/ene6k77-bootloader/tests/ene6k77_identify.rs
tools/fw-update-ene6k77/Cargo.toml
tools/fw-update-ene6k77/README.md
tools/fw-update-ene6k77/src/main.rs
tools/fw-update-ene6k77/src/cli.rs
tools/fw-update-ene6k77/src/usb.rs
tools/fw-update-ene6k77/src/flash_flow.rs
tools/fw-update-ene6k77/src/banner.rs
docs/decomp/fw-update/README.md             (provenance + how to regenerate)
docs/decomp/fw-update/fw-update.txt         (moved from parent root)
docs/decomp/fw-update/fw-update-hlil.txt    (moved from parent root)
```

Modified files:

```
Cargo.toml                                  (add the two new members)
docs/decompiled-source-map.md               (link to new spec)
docs/protocol/cross-device-survey/per-family/ene6k77fan.md
                                            (cross-link to bootloader spec)
README.md                                   (add tool to the list)
```

The decomp outputs move from the parent-repo root into the
`lconnect-linux` repo at `docs/decomp/fw-update/` so the spec doc's
citations resolve from inside the repo. A short `README.md` next to
them documents what the binary is, where it came from, and how to
regenerate the decomp if BinaryNinja's output changes between
versions.

Suggested commit ordering for the implementation plan to lay out:

1. Move decomp files into `docs/decomp/fw-update/` + add the README.
2. Write the protocol spec doc.
3. Add the codec crate (skeleton, lib.rs only).
4. Module-by-module: `bank`, `control`, `register`, `flash`,
   `enter_bootloader`, `identify`, each with its tests, each
   committed separately.
5. Update `decompiled-source-map.md` and `ene6k77fan.md` cross-links.
6. Add the harness binary skeleton (clap, no flash flow yet).
7. Wire in `list` and `inspect` subcommands (read-only).
8. Wire in the `flash` flow incrementally:
   enter-bootloader → flash_open → write loop → verify-read →
   memcmp → checksum → close. Each step a separate commit.
9. Final pass: README, banner text review, run
   `rust-practices-verifier`.

## 9. Risks and how the design responds

| Risk | Response |
|---|---|
| Someone runs the harness on real hardware and bricks their device | Five-layer warning wall (`Cargo.toml`, `main.rs`, `README.md`, clap `about`, runtime banner); literal-phrase risk-acceptance flag with no shortcut; interactive y/N requiring full word `yes`; "Open questions / unverified" section enumerated in three places |
| The codec hides protocol invariants the original tool got right | Codec is intentionally thin — no orchestrator function, no bank-state machine, no skip enforcement. The harness applies invariants; failure to apply them shows up as bugs in the harness, not as silent codec correctness drift |
| Misreading of the decomp leaks into both codec and tests | Byte-fingerprint tests transcribe expected output from the decomp directly, not from the codec's constants — typos in constants fail those tests even if the rest is internally consistent |
| `winver/L-Connect 3/FwUpdate.exe` ships a different binary in a future L-Connect 3 release than what we analysed | `docs/decomp/fw-update/README.md` documents the embedded PDB-string fingerprint and instructs how to regenerate the decomp; spec citations are to line numbers in the committed decomp files, not the binary, so they remain stable even if Lian-Li ships a new updater |
| The codec drifts out of sync with the spec doc as either changes | Both live in `lconnect-linux`; rustdoc cites the same spec-doc paragraphs as the spec cites decomp lines; CI fails on stale rustdoc links via `cargo doc --no-deps` |
| Harness becomes a partial daemon over time (effect creep) | "What the harness explicitly does NOT do" list in §6, plus the codec being I/O-free, structurally prevents reuse from any daemon. A future Phase-1 daemon that needs firmware-update functionality should consume the codec directly, not depend on the harness binary |
