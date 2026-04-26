# Phase 0e — Cross-Device Command Survey Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce `docs/protocol/cross-device-survey.md` — a single document mapping every command, framing class, and payload shape that LConnect3 uses across all device families, cross-validated against OpenRGB, plus a `verify-galahad probe` subcommand for safe read-only cross-device experimentation on the LCDV2.

**Architecture:** Static analysis of decompiled C# under `/home/inky/Development/lian-li/L-Connect.Core/LConnectCore/Products/<family>/`, cross-checked against the OpenRGB Lian Li controller tree. Per-family survey writeups go to staging files (`docs/protocol/cross-device-survey/per-family/<family>.md`) so they can be authored in parallel by independent subagents without merge conflicts; a synthesis task assembles the final survey doc. The `verify-galahad probe` subcommand follows TDD (codec parse → CLI plumbing → live run), gated on a hid-safety-reviewer dispatch before any HID write or speculative read.

**Tech Stack:** Markdown for surveys, Rust + clap + hidapi for the probe CLI (extending existing `tools/verify-galahad/`).

---

## Reference: project conventions

This plan inherits the conventions documented in:

- [`docs/superpowers/specs/2026-04-26-phase-1-daemon-design.md`](../specs/2026-04-26-phase-1-daemon-design.md) — Phase 0e is Section 8 of this spec
- [`docs/superpowers/specs/2026-04-25-lconnect-linux-design.md`](../specs/2026-04-25-lconnect-linux-design.md) — overall design
- `CLAUDE.md` (parent dir) — repo conventions, hooks, hid-safety dispatch policy
- [`docs/protocol/galahad2-lcdv2.md`](../../protocol/galahad2-lcdv2.md) — Phase 0d output, the citation-style template every per-family writeup must mirror

**Citation convention (mandatory):** every protocol fact in the survey must carry a citation of the form `LConnectCore/Products/<family>/<file>.cs:<start>-<end>`. Example: `LConnectCore/Products/Galahad2Vision/RequestType.cs:8-30`. Citations let later readers verify our reading against the source they have on disk.

**IP boundary (mandatory):** the survey documents *protocol facts* (command bytes, payload offsets, framing). It does **not** copy C# or OpenRGB code into our tree. Cite by `file:line` and describe in our own words/tables.

**Standing review preference:** every code-touching task gets reviewed by spec-compliance + code-quality + rust-practices-verifier subagents (in parallel). Documentation-only tasks get a single "reviewer" subagent that re-reads the source and checks each fact against its citation. HID write paths get a hid-safety-reviewer dispatch in addition.

**Commit message convention:** conventional-commits prefixes. Survey writeups use `docs(protocol):`. Probe CLI work uses `feat(verify-galahad):` and `test(verify-galahad):`.

---

## Hardware target

Phase 0e involves **no HID writes** to the Galahad II LCDV2 except for the optional `probe` subcommand's read-only experiments, each of which is gated on an explicit hid-safety-reviewer dispatch. The static survey requires no hardware.

---

## Family list

Per the spec, twelve device families to survey. Folder layout from `LConnectCore/Products/`:

| Family | Source folder | Notes |
|---|---|---|
| TLFan | `TLFan/` | Fan controller, no LCD |
| Galahad2 Trinity | `Galahad2Trinity/` | LED-only pump, no LCD |
| Galahad2 Vision | `Galahad2Vision/` | LCDV2 + older LCD variants — already verified in Phase 0d, recap only |
| HydroShift LCD (I) | `HydroShiftLCD/` | Older LCD on pump |
| HydroShift II LCD | `HydroShiftIILCD/` | Closest LCDV2 analog (Circle/Square SKUs) |
| TLLCD | `TLLCD/` | Fan-mounted LCD |
| SLV3LCD | `SLV3LCD/` | Fan-mounted LCD |
| UniversalScreen 8.8" | `UniversalScreen8p8Inch/` | External LCD |
| StrimerPlus | `StrimerPlus/` | RGB cable extensions |
| Ene6K77Fan | `Ene6K77Fan/` | Fan controllers (AL/SL variants) |
| Lancool 207 Digital | `Lancool207Digital/` | Case-integrated controller |
| LWireless (hub) | `LWireless/` | Hub device — contains nested forwarders for HydroShiftIILCD, SLV3, SLV3LCD, StrimerPlus, TLV2LCD, Lancool217Inf. Document the hub itself; nested forwarders may share or override the underlying family's wire format. |

`TLV2LCD` and `Lancool217Inf` are not separate top-level family folders — they're inside `LWireless/`. Document them as "child families of LWireless" in that family's writeup.

---

## Task index

- **P0e-1** — Scaffold the survey doc + staging directory
- **P0e-2** through **P0e-12** — Per-family surveys, 11 tasks (parallelizable)
- **P0e-13** — Per-family review pass
- **P0e-14** — OpenRGB cross-validator pass
- **P0e-15** — Synthesize final `cross-device-survey.md`
- **P0e-16** — Findings & actionable outputs section
- **P0e-17** — TDD: `verify-galahad probe` subcommand — codec layer
- **P0e-18** — TDD: `verify-galahad probe` subcommand — CLI plumbing
- **P0e-19** — Define probe set + hid-safety-reviewer dispatch
- **P0e-20** — Run probes against LCDV2 (interactive, gated)
- **P0e-21** — Cross-link from existing docs + README
- **P0e-22** — Phase 0e wrap-up + post-execution notes

**Parallelism:** P0e-2 through P0e-12 (all per-family surveys) can be dispatched in parallel because each writes to its own staging file. P0e-14 (OpenRGB) can run in parallel with the per-family group. P0e-13 review depends on P0e-2..P0e-12 finishing. P0e-15 onward must run sequentially.

**Subagent dispatch note:** When dispatching a subagent on a per-family survey task (P0e-2..P0e-12), include the entire "Per-family survey: canonical procedure" section in the prompt — the subagent's context starts empty and the canonical procedure isn't repeated inside each family's task block.

---

## Task P0e-1: Scaffold the survey doc + staging directory

**Files:**
- Create: `docs/protocol/cross-device-survey.md`
- Create: `docs/protocol/cross-device-survey/per-family/.gitkeep`

- [ ] **Step 1:** Create the staging directory.

```bash
mkdir -p docs/protocol/cross-device-survey/per-family
touch docs/protocol/cross-device-survey/per-family/.gitkeep
```

- [ ] **Step 2:** Write the scaffold of `docs/protocol/cross-device-survey.md` with these top-level sections (content is the section headers + a short intro per section; the actual tables come from later tasks):

````markdown
# Lian Li L-Connect3 — Cross-Device Command Survey

- **Status:** in progress (Phase 0e)
- **Date:** 2026-04-26
- **Scope:** all device families managed by LConnect3, plus an OpenRGB
  cross-validator pass

## 1. Sources

1. **Primary:** decompiled `LConnectCore/Products/<family>/` under
   `/home/inky/Development/lian-li/L-Connect.Core/`
2. **Cross-validator:** [OpenRGB's `Controllers/LianLiController/`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers/LianLiController) (GPL-2.0+)
3. **Verified live:** `docs/protocol/captures/` (LCDV2 only as of Phase 0d)

OpenRGB is license-compatible for reading. The survey cites OpenRGB by
URL and file path; it does **not** copy OpenRGB code into our tree.
Protocol facts (command bytes, payload offsets, framing) are documented
in our own tables.

## 2. Conventions

- Every command entry carries a citation `LConnectCore/Products/<family>/<file>.cs:<start>-<end>`
- Status flags: ✅ verified (live or live-cited) | 🌗 inferred (static-only) | ❓ unclear (decompiled but partially obscured)
- Framing classes: **A** (64-byte ACommandPacket), **B** (1024 out / 512 in BCommandPacket), **C** (512/512 CCommandPacket — `Galahad2Vision/CCommandPacket.cs` confirms existence). Other framings noted as discovered.

## 3. Per-family writeups

(Assembled from `docs/protocol/cross-device-survey/per-family/*.md` by P0e-15.)

## 4. Cross-reference matrix

(P0e-15 fills these tables.)

### 4.1 Lighting commands across families
### 4.2 LCD-frame commands across families
### 4.3 Fan PWM commands across families
### 4.4 Sensor / handshake commands across families
### 4.5 Maintenance commands across families (reset, firmware, identity)

## 5. OpenRGB cross-validator findings

(P0e-14 fills.)

## 6. Findings & actionable outputs

(P0e-16 fills. Topics: framing-class extension points, cmd-15 hypotheses, reusable codec opportunities, risky-area flagging.)
````

- [ ] **Step 3:** Verify nothing else broke.

```bash
cargo check --workspace
```

Expected: clean (this is a docs-only change).

- [ ] **Step 4:** Commit.

```bash
git add docs/protocol/cross-device-survey.md \
        docs/protocol/cross-device-survey/per-family/.gitkeep
git commit -m "docs(protocol): scaffold cross-device-survey doc + staging dir

Phase 0e kickoff. Per-family writeups will land as staging files under
docs/protocol/cross-device-survey/per-family/<family>.md and be
synthesized into the top-level survey by P0e-15."
```

---

## Per-family survey: canonical procedure

Tasks P0e-2 through P0e-13 follow the same procedure with family-specific
overrides. Each task's writeup goes to its own staging file at
`docs/protocol/cross-device-survey/per-family/<family>.md`, so they can be
dispatched in parallel.

### Canonical staging-file template

Each `per-family/<family>.md` file follows this structure:

````markdown
## Family: <FamilyName>

**Source folder:** `LConnectCore/Products/<family>/`
**Marketed name(s):** <name 1>, <name 2>, …
**ControllerTypeID enum value(s):** <name>(s) cited from `LConnectCore/Devices/KnownDevices.cs:<line>`
**USB VID:PID:** <vid>:<pid> cited from `LConnectCore/Devices/USBDeviceID.cs:<line>` — one row per SKU under this family
**Framing classes used:** A | B | C | family-specific (cite the packet file(s))
**Subsystem fingerprint** (✅ has / ❌ absent):
- Sensors / handshake: <yes/no>
- Fan PWM: <yes/no>
- Pump PWM: <yes/no>
- Fan curve upload: <yes/no>
- Lighting (preset modes): <yes/no>
- Lighting (per-pixel LED): <yes/no>
- LCD on/off / brightness / rotation: <yes/no>
- LCD frame upload (JPEG / raw / H.264): <yes/no>
- Continuous re-issue pattern (e.g., 1-2s timer on lighting): <yes/no, cite if yes>
- Reset / soft-reset: <yes/no>
- Firmware upload / activation: <yes/no — note Phase 0d defers these>

### Commands

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| `0x86` | host→dev | `GetFirmwareVersion` | A | `RequestType.cs:14` | 🌗 | … |
| … | … | … | … | … | … | … |

### Subsystem patterns

(Free-form prose, 2-5 sentences each, citing `file:line`. Topics that apply: continuous re-issue timers, fan-curve serialization shape, lighting-buffering quirks, LCD frame chunking strategy.)

### Notable divergences from LCDV2

(Free-form, ≤300 words. Highlight commands or patterns that differ from what we already documented in `galahad2-lcdv2.md` — these are the most useful entries for the synthesis task.)

### Open questions / unclear from static analysis

(Free-form; flag any `🚧 unclear` cells with the reason. Synthesis task uses these to drive the targeted-probe set in P0e-19.)
````

### Canonical procedure steps

For each per-family task (P0e-2 through P0e-13), the step list is:

1. List the family's source folder; identify the device class file (typically `<Family>Device.cs` or `<Family>VisionDevice.cs`)
2. Open `KnownDevices.cs` and `USBDeviceID.cs`; record the `ControllerTypeID` entries and VID/PID values that map to this family. One family may map to multiple SKUs (e.g., HydroShiftIILCD → Circle + Square).
3. Open the family's `RequestType.cs` (or equivalent enum file); list every command byte
4. For each command, locate its handling in the device class file (the place the command byte gets written to a packet); identify which framing class is used
5. For each command, capture the payload shape: byte offsets, field names (mirror C# field names snake-cased), value ranges where evident from enum companions
6. Identify subsystem patterns: continuous re-issue timers (look for `Timer`, `DispatcherTimer`, or `Task.Delay` loops), fan curves (search `RPMConfig` references), LCD-frame chunking (search for `BCommandPacket`/`CCommandPacket` chunkers), lighting-buffering quirks (any retry/delay logic on lighting writes)
7. Note divergences from LCDV2 — anything Galahad II Vision doesn't have, or has in a different shape
8. Flag open questions with `🚧 unclear` and the reason
9. Write the staging file
10. Sanity check: every cited line range exists and the line range covers the claim being made (do `sed -n '<start>,<end>p' <file>.cs` if uncertain — but do not paste source into our doc)
11. Commit

The probe-set design in P0e-19 reads these staging files and turns the unclear entries into testable hypotheses.

---

## Task P0e-2: Survey TLFan

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/tlfan.md`

- [ ] **Step 1:** Apply the canonical procedure (above) to the family.
  - Source folder: `LConnectCore/Products/TLFan/`
  - Expected fingerprint: fan PWM ✅, lighting ✅, no LCD, no pump
  - Specific files of interest: `TLFanDevice.cs`, `RequestType.cs`, `FanLightingConfig.cs` (if present), `FanGroupRPMConfig.cs`

- [ ] **Step 2:** Validate citations.

```bash
# Sanity-check that every cited line range covers the claim. Spot-check
# 3 random citations from your file:
grep -n "" /home/inky/Development/lian-li/L-Connect.Core/LConnectCore/Products/TLFan/RequestType.cs | head -5
```

- [ ] **Step 3:** Commit.

```bash
git add docs/protocol/cross-device-survey/per-family/tlfan.md
git commit -m "docs(protocol): cross-device survey — TLFan family"
```

---

## Task P0e-3: Survey Galahad2 Trinity

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/galahad2-trinity.md`

- [ ] **Step 1:** Apply the canonical procedure to the family.
  - Source folder: `LConnectCore/Products/Galahad2Trinity/`
  - Expected fingerprint: pump PWM ✅, fan PWM ✅, lighting ✅ (LED-only, no LCD frame upload)
  - Specific interest: cmd 131 (`SetPumpLighting`) and cmd 133 (`SetFanLighting`) — does Trinity match LCDV2 byte-for-byte? If yes, that's strong evidence the LED MCU is shared firmware across the Galahad2 family.

- [ ] **Step 2:** Validate citations (spot-check three).

- [ ] **Step 3:** Commit.

```bash
git add docs/protocol/cross-device-survey/per-family/galahad2-trinity.md
git commit -m "docs(protocol): cross-device survey — Galahad2 Trinity family"
```

---

## Task P0e-4: Survey Galahad2 Vision (recap)

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/galahad2-vision.md`

This is a **recap** task — Phase 0d already produced [`docs/protocol/galahad2-lcdv2.md`](../../protocol/galahad2-lcdv2.md). The staging file here is a condensed table-only summary keyed for the cross-reference matrix; do **not** restate full byte tables.

- [ ] **Step 1:** Read `docs/protocol/galahad2-lcdv2.md` and extract the per-command rows into the canonical command table format. Each row's "Status" column should be ✅ since these were live-verified.

- [ ] **Step 2:** Confirm `Galahad2Vision/CCommandPacket.cs` exists and document the framing class entry; this is the C-class framing the Phase 1 plan will codec-implement.

- [ ] **Step 3:** Subsystem fingerprint mirrors Phase 0d findings: sensors ✅, fan PWM ✅, pump PWM ✅, lighting (preset + per-pixel) ✅, LCD on/off + brightness + rotation ✅, LCD frame upload (cmd 14 ✅; cmd 15 codec-only / activation unknown), continuous re-issue ✅ (lighting cmd 131, per Phase 0c capture).

- [ ] **Step 4:** "Notable divergences from LCDV2" section is N/A here — note "this IS the LCDV2; this entry is the synthesis baseline."

- [ ] **Step 5:** Commit.

```bash
git add docs/protocol/cross-device-survey/per-family/galahad2-vision.md
git commit -m "docs(protocol): cross-device survey — Galahad2 Vision (recap)"
```

---

## Task P0e-5: Survey Galahad2 LCD (older variant)

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/galahad2-lcd-older.md`

The older Galahad II LCD does not have a separate `Galahad2LCD/` folder — it's inferred to live within `Galahad2Vision/` as one of the SKUs ControllerTypeID enumerates. Phase 0a verified that `Galahad2Vision/` may implement multiple SKUs; this task confirms or refutes that.

- [ ] **Step 1:** In `KnownDevices.cs`, find every entry whose name contains `Galahad2` and is not `Galahad2LCDV2`, `Galahad2TrinityRegular`, or `Galahad2TrinityPerformance` (those are accounted for elsewhere). Likely candidates: `Galahad2LCD` at `0x7391`. Cite the line.

- [ ] **Step 2:** In `Galahad2VisionDevice.cs`, search for branches that key off the controller type — `if (this.ControllerType == ControllerTypeID.Galahad2LCD)` or similar. Document any branch where the older `Galahad2LCD` takes a different path than `Galahad2LCDV2`.

- [ ] **Step 3:** If the older variant is structurally identical (same commands, same framing), document this as "structurally identical to Galahad2 Vision" and the staging file is a one-paragraph entry pointing to galahad2-vision.md. If it diverges, write the divergences explicitly.

- [ ] **Step 4:** Commit.

```bash
git add docs/protocol/cross-device-survey/per-family/galahad2-lcd-older.md
git commit -m "docs(protocol): cross-device survey — Galahad2 LCD (older variant)"
```

---

## Task P0e-6: Survey HydroShift LCD (HS I, older)

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/hydroshift-lcd.md`

- [ ] **Step 1:** Apply canonical procedure.
  - Source folder: `LConnectCore/Products/HydroShiftLCD/`
  - Expected fingerprint: pump PWM ✅, lighting ✅, LCD on pump ✅
  - **Highest-priority comparison points for the survey:** how is the LCD frame uploaded? Same cmd 14/15 + BCommandPacket, or different? Does HS I have a separate "stored image" / "boot logo" command we don't have on LCDV2? This is the single most useful divergence to surface for the cmd-15 investigation.

- [ ] **Step 2:** Document any HS-I-specific command that touches stored-image flash, boot-display, or alternate-mode-image features. Cite each.

- [ ] **Step 3:** Validate citations + commit.

```bash
git add docs/protocol/cross-device-survey/per-family/hydroshift-lcd.md
git commit -m "docs(protocol): cross-device survey — HydroShift LCD (HS I)"
```

---

## Task P0e-7: Survey HydroShift II LCD

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/hydroshift-ii-lcd.md`

- [ ] **Step 1:** Apply canonical procedure. This family is **substantially larger** than other families (Phase 0e exploration found ~30+ files including `HydroShiftIILCDCustomTheme.cs`, `HydroShiftIILCDModular.cs`, `HydroShiftIILCDTemplate.cs`, font families, theme info, etc.). Allocate proportional time.
  - Source folder: `LConnectCore/Products/HydroShiftIILCD/` plus `LConnectCore/Products/HydroShiftIILCD/Requests/` (separate Requests subfolder)
  - SKUs: Circle and Square — document both VID/PIDs separately
  - Expected fingerprint: pump PWM ✅, lighting ✅, LCD on pump ✅, **theme files / templates / modular layouts ✅** (this is unique among the families and informs Phase 4's theme engine design)

- [ ] **Step 2:** Specifically inspect:
  - `HydroShiftIILCDConstants.cs` — LCD dimensions, frame rate, max payload sizes (compare against LCDV2's 480×480 @ 24fps)
  - `HydroShiftIILCDCustomThemeTypes.cs` and `HydroShiftIILCDModularTypes.cs` — theme model shapes (input for Phase 4's manifest design)
  - The `Requests/` subfolder — every per-command request class. List each with its cmd byte and framing.
  - Firmware-update commands (`HydroShiftIILCDFirmwareUpdateStatus.cs`) — note presence; do **not** document beyond "exists" since Phase 1 explicitly defers firmware operations

- [ ] **Step 3:** **For the cmd-15 hypothesis specifically:** does HS II have a separate "activate stored image" / "show boot logo" / "display from flash" command that LCDV2 might also accept but we haven't characterized? Document with explicit citation. This is gold for the investigation.

- [ ] **Step 4:** Validate citations + commit.

```bash
git add docs/protocol/cross-device-survey/per-family/hydroshift-ii-lcd.md
git commit -m "docs(protocol): cross-device survey — HydroShift II LCD"
```

---

## Task P0e-8: Survey TLLCD

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/tllcd.md`

- [ ] **Step 1:** Apply canonical procedure.
  - Source folder: `LConnectCore/Products/TLLCD/`
  - Expected fingerprint: fan PWM ✅, lighting ✅, fan-mounted LCD ✅
  - Note: also check whether `LWireless/TLV2LCD/` is a related/derived variant — surface the relationship in this writeup if so

- [ ] **Step 2:** Validate citations + commit.

```bash
git add docs/protocol/cross-device-survey/per-family/tllcd.md
git commit -m "docs(protocol): cross-device survey — TLLCD"
```

---

## Task P0e-9: Survey SLV3LCD

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/slv3lcd.md`

- [ ] **Step 1:** Apply canonical procedure.
  - Source folder: `LConnectCore/Products/SLV3LCD/`
  - Expected fingerprint: fan PWM ✅, lighting ✅, fan-mounted LCD ✅
  - Note: also check `LWireless/SLV3LCD/` and `LWireless/SLV3/`; the wireless hub may forward SLV3 commands. Document the relationship.

- [ ] **Step 2:** Validate citations + commit.

```bash
git add docs/protocol/cross-device-survey/per-family/slv3lcd.md
git commit -m "docs(protocol): cross-device survey — SLV3LCD"
```

---

## Task P0e-10: Survey UniversalScreen 8.8"

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/universalscreen.md`

- [ ] **Step 1:** Apply canonical procedure.
  - Source folder: `LConnectCore/Products/UniversalScreen8p8Inch/`
  - Expected fingerprint: large external LCD; likely uses different framing or chunking than LCDV2 due to higher resolution / data volume
  - Specific interest: framing class. The 8.8" screen has more pixels than the 480×480 LCDV2; if it uses CCommandPacket or a fourth framing class, document it. This directly informs whether `lconnect-protocol`'s `Framing` enum needs extension points.

- [ ] **Step 2:** Validate citations + commit.

```bash
git add docs/protocol/cross-device-survey/per-family/universalscreen.md
git commit -m "docs(protocol): cross-device survey — UniversalScreen 8.8\""
```

---

## Task P0e-11: Survey StrimerPlus

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/strimerplus.md`

- [ ] **Step 1:** Apply canonical procedure.
  - Source folder: `LConnectCore/Products/StrimerPlus/`
  - Expected fingerprint: lighting ✅ (RGB cable extensions are LED-only)
  - Cross-cutting interest: this family is most likely to overlap with OpenRGB, since OpenRGB targets RGB-only devices. Pre-flag for the OpenRGB cross-validator pass (P0e-14).
  - Note: also check `LWireless/StrimerPlus/`

- [ ] **Step 2:** Validate citations + commit.

```bash
git add docs/protocol/cross-device-survey/per-family/strimerplus.md
git commit -m "docs(protocol): cross-device survey — StrimerPlus"
```

---

## Task P0e-12: Survey Ene6K77Fan + LWireless + Lancool 207

**Files:**
- Create: `docs/protocol/cross-device-survey/per-family/ene6k77fan.md`
- Create: `docs/protocol/cross-device-survey/per-family/lwireless.md`
- Create: `docs/protocol/cross-device-survey/per-family/lancool207.md`

This task bundles three smaller-but-distinct families. They're grouped because each is expected to be small (a couple dozen commands each), and bundling avoids excessive task fanout for the same scope.

- [ ] **Step 1:** Survey **Ene6K77Fan**.
  - Source folder: `LConnectCore/Products/Ene6K77Fan/`
  - Expected fingerprint: fan PWM ✅, lighting ✅
  - SKUs: AL and SL variants — document both VID/PIDs

- [ ] **Step 2:** Survey **LWireless** (the hub itself).
  - Source folder: `LConnectCore/Products/LWireless/` (top level only — child families are separate folders inside but documented in their own staging files)
  - Expected fingerprint: hub device, named-pipe IPC for image transport (`PipeClientManager.cs`, `PipeImageSender.cs` — cite line ranges)
  - Document the named-pipe protocol shape: pipe name, message framing, payload structure
  - List every "child family" under `LWireless/` (HydroShiftIILCD, SLV3, SLV3LCD, StrimerPlus, TLV2LCD, Lancool217Inf) and note for each whether the hub's wire format is a passthrough or a transformation of the underlying family's wire format.
  - **TLV2LCD and Lancool 217 Digital are documented HERE**, since they don't have top-level family folders. Brief writeups (5-10 commands each, plus citation).

- [ ] **Step 3:** Survey **Lancool 207 Digital**.
  - Source folder: `LConnectCore/Products/Lancool207Digital/`
  - Expected fingerprint: case-integrated LCD/lighting/sensors; likely overlaps with LCDV2 lighting commands or has its own
  - Note: Lancool **217** Digital lives inside `LWireless/Lancool217Inf/` per step 2 and is NOT this task

- [ ] **Step 4:** Validate citations across all three files + commit.

```bash
git add docs/protocol/cross-device-survey/per-family/ene6k77fan.md \
        docs/protocol/cross-device-survey/per-family/lwireless.md \
        docs/protocol/cross-device-survey/per-family/lancool207.md
git commit -m "docs(protocol): cross-device survey — Ene6K77Fan, LWireless, Lancool207"
```

---

## Task P0e-13: Per-family review pass

**Files:** (no new; reviews all 11 staging files from P0e-2..P0e-12)

A reviewer subagent re-reads every staging file alongside the C# source it cites and checks: (1) every citation's line range covers the claim, (2) every command byte in the table matches `RequestType.cs`, (3) framing-class assignments match the packet construction code, (4) no C# code has been pasted in (only facts/tables in our own words).

- [ ] **Step 1:** Dispatch a reviewer subagent (general-purpose with the prompt below) to audit all 11 staging files in parallel.

Reviewer prompt (paste verbatim into the subagent):
````
Audit the per-family staging files at
docs/protocol/cross-device-survey/per-family/*.md against the source
files they cite under
/home/inky/Development/lian-li/L-Connect.Core/LConnectCore/Products/.

For each staging file, verify:
1. Every citation `LConnectCore/Products/<family>/<file>.cs:<start>-<end>`
   resolves to a real file and the cited line range covers the claim
   being made
2. Every command byte in the family's command table matches the value
   in `RequestType.cs` (or the equivalent enum file)
3. Framing-class assignments (A/B/C) match the actual packet
   construction in the family's device class
4. No C# source has been pasted into the markdown — facts only,
   formatted as our own tables/prose

Report findings in three buckets:
- BLOCKING: incorrect citations, wrong cmd bytes, wrong framing class,
  copied C# code
- ADVISORY: missing important commands the family handles, weak prose
  in subsystem fingerprint, citation line ranges that are too narrow
  (cite a 2-line range when the claim spans 20)
- PRAISE: anything done particularly well

Do NOT modify the staging files. Just report.
````

- [ ] **Step 2:** Address all BLOCKING findings; advisory findings are landed at the implementer's discretion.

- [ ] **Step 3:** Commit any fixes.

```bash
git add docs/protocol/cross-device-survey/per-family/
git commit -m "docs(protocol): apply per-family survey review fixes"
```

---

## Task P0e-14: OpenRGB cross-validator pass

**Files:**
- Modify: `docs/protocol/cross-device-survey.md` — fill in section 5 "OpenRGB cross-validator findings"

This task can be dispatched in parallel with P0e-2..P0e-13 (it doesn't depend on the per-family staging files; the cross-reference happens during synthesis in P0e-15). However, doing it after the per-family work means the per-family writeups can be cited inline.

- [ ] **Step 1:** Fetch the OpenRGB Lian Li controller tree.

```bash
mkdir -p /tmp/openrgb-survey
git clone --depth 1 https://gitlab.com/CalcProgrammer1/OpenRGB.git \
    /tmp/openrgb-survey/openrgb 2>&1 | tail -5
ls /tmp/openrgb-survey/openrgb/Controllers/LianLiController/
```

Expected: a list of subdirectories or `.cpp/.h` files for the various Lian Li product targets OpenRGB supports.

- [ ] **Step 2:** For each subcomponent under `Controllers/LianLiController/`, identify what device family it targets (look at file headers and any `vid_pid` arrays). Map each OpenRGB component to one of our survey families.

- [ ] **Step 3:** For each OpenRGB component → family mapping, extract the command bytes OpenRGB sends. Compare against our survey for that family. Populate the cross-validator finding for each command:
  - ✅ matches — OpenRGB sends the same byte for the same purpose
  - ⚠ diverges — OpenRGB sends a different byte or different payload shape
  - — not implemented — OpenRGB doesn't send this command at all

- [ ] **Step 4:** Write section 5 of `cross-device-survey.md` with:
  - Which families OpenRGB covers (list with VID:PID match)
  - Which families OpenRGB does **not** cover (relevant to "expected; LCDV2 is AIO hardware out of OpenRGB's typical scope")
  - The match/diverge/missing breakdown per family
  - Any divergence that warrants reconciliation (citing both `OpenRGB/Controllers/...` paths and our `LConnectCore/Products/...` paths)

- [ ] **Step 5:** Validate every OpenRGB citation by URL + line: format like `[OpenRGB/Controllers/LianLiController/<file>:<line>](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/<file>#L<line>)`. Spot-check three URLs by `curl -fI` to confirm they resolve.

- [ ] **Step 6:** Commit.

```bash
git add docs/protocol/cross-device-survey.md
git commit -m "docs(protocol): cross-device survey — OpenRGB cross-validator pass"
```

---

## Task P0e-15: Synthesize the final cross-device-survey.md

**Files:**
- Modify: `docs/protocol/cross-device-survey.md` — fill in section 3 (per-family writeups inline) and section 4 (cross-reference matrix)

- [ ] **Step 1:** Append every `per-family/<family>.md` staging file's content into section 3 of `cross-device-survey.md`. Keep the staging files in place — they remain the editable source-of-truth and the synthesized doc is a regenerable view (or you may delete the staging dir at the end if the team prefers a single doc; document the choice in the commit message either way).

- [ ] **Step 2:** Build the cross-reference matrix tables in section 4. For each subsystem (lighting, LCD frame, fan PWM, sensor handshake, maintenance), produce a table like:

```markdown
### 4.1 Lighting commands across families

| Cmd byte | Galahad2 LCDV2 | Galahad2 Trinity | TLFan | HS I | HS II | TLLCD | SLV3LCD | … | OpenRGB |
|---|---|---|---|---|---|---|---|---|---|
| `0x14` (SetLEDColors) | ✅ per-pixel | ✅ per-pixel | … | … | … | … | … | … | ⚠ diverges payload |
| `0x83` (SetPumpLighting) | ✅ preset | ✅ preset | — | … | … | … | … | … | — |
| … | … | … | … | … | … | … | … | … | … |
```

A cell shows `✅` (family implements this command, same shape as the row's reference family), `⚠` (family implements but with a different payload — link to the divergence note), `—` (family does not implement), `❓` (unclear from static analysis — flagged for P0e-19 probe set).

- [ ] **Step 3:** For every `❓` cell in the matrix, record an entry in the "open questions" running list — these become candidate probes for P0e-19.

- [ ] **Step 4:** Commit.

```bash
git add docs/protocol/cross-device-survey.md
git commit -m "docs(protocol): synthesize cross-device-survey — per-family + matrix"
```

---

## Task P0e-16: Findings & actionable outputs

**Files:**
- Modify: `docs/protocol/cross-device-survey.md` — fill in section 6

This section is the *deliverable* of the survey — actionable input for the Phase 1 plan. Without it, the survey is just a reference doc.

- [ ] **Step 1:** Write section 6 with the following sub-sections:

```markdown
## 6. Findings & actionable outputs

### 6.1 Daemon framing-class extension points

Does any device family use a framing class beyond A (64 B), B (1024/512),
and C (512/512)? Document each. If yes, `lconnect-hid`'s `Framing`
enum needs to accommodate it; if no, a 3-variant `Framing` enum is
sufficient for the foreseeable phases.

### 6.2 Cmd-15 (UploadJpeg) hypotheses sourced from sibling families

Map each ❓ "stored image" / "boot logo" / "alternate display mode"
command found in HS I, HS II, or other LCD-bearing families to a
testable hypothesis on the LCDV2. Each hypothesis has a one-line
description, the family it was sourced from, the cmd byte we'd test,
and the expected behaviour if the hypothesis is correct.

### 6.3 Reusable codec opportunities

Commands that appear byte-identical across ≥3 families — these are
candidates for refactoring into `lconnect-protocol/src/common/`
*before* the Phase 3 second device family lands. Phase 1 should leave
the existing `galahad2_lcdv2/` module untouched; Phase 3's first task
is the "extract common codecs" refactor with these targets pre-listed.

### 6.4 Risky-area flagging for hid-safety-reviewer

Any command guarded by warning dialogs, retry logic, or "do not call
repeatedly" comments in the C# source. Flag these so future
hid-safety-reviewer dispatches know what patterns to watch for.
The Phase 1 implementation plan inherits these flags.

### 6.5 Phase 1 plan inputs

A short bulleted recap: which findings the Phase 1 plan must absorb
(framing enum shape, cmd-15 investigation hypotheses, code organisation
hints).
```

- [ ] **Step 2:** Update `cross-device-survey.md`'s top-of-file Status line to "Phase 0e static survey complete — pending live probes (P0e-20) and post-execution wrap-up".

- [ ] **Step 3:** Commit.

```bash
git add docs/protocol/cross-device-survey.md
git commit -m "docs(protocol): cross-device survey — findings + actionable outputs"
```

---

## Task P0e-17: TDD `verify-galahad probe` subcommand — codec layer

**Files:**
- Modify: `tools/verify-galahad/src/main.rs` — add `Probe` subcommand variant
- Create: `tools/verify-galahad/src/probe.rs` — probe logic (input parsing, framing dispatch, response decoding, hexdump output)
- Create: `tools/verify-galahad/tests/probe.rs` — input parsing + hexdump golden tests

The `probe` subcommand sends a single host→device read-style request and dumps the response as hex. It does **not** mutate device state (caller responsibility — P0e-19's safety review confirms each chosen cmd byte is read-only). Framing class is selected via `--framing a|b|c`; defaults to `a`.

- [ ] **Step 1:** Write the failing test for input parsing.

```rust
// tools/verify-galahad/tests/probe.rs
use verify_galahad::probe::{Framing, ProbeArgs, parse_hex_payload};

#[test]
fn parse_hex_payload_accepts_lower_case_no_prefix() {
    assert_eq!(parse_hex_payload("aabb").unwrap(), vec![0xaa, 0xbb]);
}

#[test]
fn parse_hex_payload_accepts_upper_case_with_spaces() {
    assert_eq!(parse_hex_payload("AA BB").unwrap(), vec![0xaa, 0xbb]);
}

#[test]
fn parse_hex_payload_accepts_0x_prefix() {
    assert_eq!(parse_hex_payload("0xaa 0xbb").unwrap(), vec![0xaa, 0xbb]);
}

#[test]
fn parse_hex_payload_rejects_odd_length() {
    assert!(parse_hex_payload("aabbc").is_err());
}

#[test]
fn parse_hex_payload_rejects_non_hex() {
    assert!(parse_hex_payload("aazz").is_err());
}

#[test]
fn framing_a_buffer_size_is_64() {
    assert_eq!(Framing::A.output_len(), 64);
}

#[test]
fn framing_b_buffer_size_is_1024() {
    assert_eq!(Framing::B.output_len(), 1024);
}

#[test]
fn framing_c_buffer_size_is_512() {
    assert_eq!(Framing::C.output_len(), 512);
}
```

- [ ] **Step 2:** Run the tests; expect them all to fail.

```bash
cargo test -p verify-galahad --test probe 2>&1 | tail -20
```

Expected: compile error (`probe` module doesn't exist) or unresolved imports.

- [ ] **Step 3:** Add the `probe` module skeleton.

```rust
// tools/verify-galahad/src/probe.rs
//! `verify-galahad probe` — read-only cross-device experimentation.
//!
//! Sends a single request to the device using one of the three known
//! framing classes (A/B/C) and dumps the response as hex. Used by
//! Phase 0e's targeted-probe set to confirm or refute hypotheses
//! sourced from sibling device families.
//!
//! # Safety
//!
//! Each invocation should target a command byte that has been
//! reviewed as **read-only** by hid-safety-reviewer. The CLI does not
//! enforce this — the caller is responsible for choosing a safe cmd
//! byte. See `docs/protocol/captures/2026-04-26-cross-device-probes.txt`
//! for the curated probe set.

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Framing {
    A,
    B,
    C,
}

impl Framing {
    #[must_use]
    pub fn output_len(self) -> usize {
        match self {
            Framing::A => 64,
            Framing::B => 1024,
            Framing::C => 512,
        }
    }

    #[must_use]
    pub fn input_len(self) -> usize {
        match self {
            Framing::A => 64,
            Framing::B => 512,
            Framing::C => 512,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProbeArgs {
    pub cmd: u8,
    pub framing: Framing,
    pub payload: Vec<u8>,
}

/// Parse a hex string into bytes. Accepts lower/upper case, optional
/// `0x` prefixes, and whitespace separators. Rejects odd-length input
/// and non-hex characters.
///
/// # Errors
///
/// Returns an error if the cleaned input has odd length or contains
/// any non-hex character.
pub fn parse_hex_payload(s: &str) -> Result<Vec<u8>> {
    let cleaned: String = s
        .replace("0x", "")
        .replace("0X", "")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if cleaned.len() % 2 != 0 {
        return Err(anyhow!("hex payload has odd length: {}", cleaned.len()));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .with_context(|| format!("non-hex digit at offset {i}"))
        })
        .collect()
}
```

- [ ] **Step 4:** Wire `pub mod probe;` into `tools/verify-galahad/src/main.rs` (or `lib.rs` if one exists; create one if not — clap binary crates often expose helpers via a sibling lib for testing).

If no `lib.rs` exists yet:

```rust
// tools/verify-galahad/src/lib.rs
pub mod probe;
```

And ensure `Cargo.toml` exposes both `lib` and `bin` targets for `verify-galahad`. If the existing layout is bin-only, the test file uses `use verify_galahad::probe::...`; either expose the module via a fresh `lib.rs` or move the tests to `#[cfg(test)] mod tests` inside `probe.rs`.

- [ ] **Step 5:** Run the tests; expect them all to pass.

```bash
cargo test -p verify-galahad --test probe 2>&1 | tail -20
```

Expected: 8 passed.

- [ ] **Step 6:** Commit.

```bash
git add tools/verify-galahad/src/probe.rs \
        tools/verify-galahad/src/lib.rs  # if newly created
        tools/verify-galahad/src/main.rs
        tools/verify-galahad/tests/probe.rs
        tools/verify-galahad/Cargo.toml  # if modified
git commit -m "feat(verify-galahad): add probe subcommand codec layer

Read-only cross-device experimentation harness for Phase 0e. Parses
hex payload input, computes per-framing output/input buffer sizes,
and exposes the data structures the CLI plumbing in P0e-18 will
wire up to hidapi."
```

---

## Task P0e-18: TDD `verify-galahad probe` subcommand — CLI plumbing

**Files:**
- Modify: `tools/verify-galahad/src/main.rs` — add `Probe { cmd, framing, payload }` variant to the `Command` enum and the dispatch arm
- Modify: `tools/verify-galahad/src/probe.rs` — add `run_probe(device, args) -> Result<Vec<u8>>` that does the actual HID round-trip

This task wires the probe codec layer to the CLI and to hidapi. The `run_probe` function is the only HID-touching surface; it follows the same `read_until_command` pattern Phase 0d established for stale-response draining.

- [ ] **Step 1:** Add the CLI variant in `main.rs`. Match Phase 0d's clap derive style (look at how `set-led-colors` and `upload-jpeg` are wired).

```rust
// in main.rs's Command enum
/// Send a single read-style request to the device using a chosen
/// framing class and dump the hex response. For Phase 0e cross-device
/// experimentation; the caller is responsible for choosing a cmd byte
/// that has been hid-safety-reviewer-cleared as read-only.
Probe {
    /// Command byte to send (e.g., `0x86`, `134`)
    #[arg(value_parser = parse_cmd_byte)]
    cmd: u8,

    /// Framing class
    #[arg(long, default_value = "a")]
    framing: verify_galahad::probe::Framing,

    /// Hex payload to send after the cmd byte (defaults to empty)
    #[arg(long, default_value = "")]
    payload: String,

    /// Acknowledge that this probe has been reviewed as read-only
    #[arg(long)]
    yes_read_only: bool,
},
```

- [ ] **Step 2:** Add the dispatch arm:

```rust
Command::Probe { cmd, framing, payload, yes_read_only } => {
    if !yes_read_only {
        anyhow::bail!(
            "refusing to probe without --yes-read-only; \
             confirm cmd byte 0x{cmd:02x} has been hid-safety-reviewer-\
             cleared as read-only and re-run with --yes-read-only"
        );
    }
    let payload = verify_galahad::probe::parse_hex_payload(&payload)?;
    let args = verify_galahad::probe::ProbeArgs { cmd, framing, payload };
    let device = open_device()?;
    let response = verify_galahad::probe::run_probe(&device, &args)?;
    println!("response ({} bytes):", response.len());
    print_hex_dump(&response);
}
```

- [ ] **Step 3:** Implement `run_probe` in `probe.rs`.

```rust
// in probe.rs
use hidapi::HidDevice;

/// Perform one probe round-trip.
///
/// Constructs an output buffer of the framing's output size with the
/// command byte at offset 1 (offset 0 is the report ID, fixed per
/// framing class) and the payload starting at offset 2; the rest is
/// zero-filled. Writes once, drains stale responses with a sibling of
/// the existing `read_until_command` helper, and returns the response.
///
/// # Errors
///
/// Returns an error if the HID write fails, the read times out, or
/// the response's command echo doesn't match the expected echo
/// (cmd | 0x80 for A-class, etc.).
pub fn run_probe(device: &HidDevice, args: &ProbeArgs) -> Result<Vec<u8>> {
    let out_len = args.framing.output_len();
    let in_len = args.framing.input_len();
    let report_id: u8 = match args.framing {
        Framing::A => 0x01,
        Framing::B => 0x02,
        Framing::C => 0x03,
    };

    // Output buffer construction: [report_id, cmd, payload..., 0..]
    let mut out = vec![0u8; out_len];
    out[0] = report_id;
    out[1] = args.cmd;
    let payload_room = out_len.saturating_sub(2);
    if args.payload.len() > payload_room {
        return Err(anyhow!(
            "payload {} bytes exceeds framing-{:?} room {}",
            args.payload.len(), args.framing, payload_room
        ));
    }
    out[2..2 + args.payload.len()].copy_from_slice(&args.payload);

    let written = device.write(&out)?;
    if written != out_len {
        return Err(anyhow!(
            "short write: {written}/{out_len} bytes"
        ));
    }

    // Drain up to N stale responses, looking for our cmd echo.
    let mut buf = vec![0u8; in_len];
    let expected_echo = args.cmd | 0x80;
    for _ in 0..4 {
        let n = device.read_timeout(&mut buf, 500)?;
        if n == 0 {
            return Err(anyhow!("read timeout waiting for echo 0x{expected_echo:02x}"));
        }
        if buf[0] == expected_echo || buf[1] == expected_echo {
            buf.truncate(n);
            return Ok(buf);
        }
        // stale response; loop and retry
    }
    Err(anyhow!(
        "drained 4 stale responses without finding echo 0x{expected_echo:02x}"
    ))
}
```

(The exact echo-matching logic — `buf[0]` vs `buf[1]` — depends on framing. Cross-check against the existing `read_until_command` helper in `main.rs` and mirror the same convention.)

- [ ] **Step 4:** Add an integration-style smoke test that opens a fake `HidDevice`-shaped trait... actually, hidapi-rs doesn't trait-ify `HidDevice`, so a real-device test is the only meaningful end-to-end check. Skip the integration test for `run_probe` itself; rely on the manual run in P0e-20 plus the unit tests on the codec helpers.

- [ ] **Step 5:** Build and run the existing test suite; everything should still pass.

```bash
cargo build -p verify-galahad
cargo test -p verify-galahad
```

Expected: clean build, all tests pass.

- [ ] **Step 6:** Run clippy and fmt.

```bash
cargo clippy -p verify-galahad --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: clean.

- [ ] **Step 7:** Dispatch reviewers in parallel (per the standing review preference): spec-compliance + code-quality + rust-practices-verifier + hid-safety-reviewer (because this *is* a HID write path, even if used for read-only cmd bytes).

- [ ] **Step 8:** Address all blocking findings from the reviewers; commit.

```bash
git add tools/verify-galahad/src/main.rs \
        tools/verify-galahad/src/probe.rs
git commit -m "feat(verify-galahad): wire probe subcommand to HID

Adds the CLI dispatch and run_probe round-trip helper. Probe writes
require --yes-read-only to prevent accidental writes of un-reviewed
cmd bytes; the safety boundary is the curated probe set, not the CLI."
```

---

## Task P0e-19: Define probe set + hid-safety-reviewer dispatch

**Files:**
- Create: `docs/protocol/captures/2026-04-26-cross-device-probes.txt` — header section only (results land in P0e-20)

This task curates the read-only probe set. It is **mandatory** that every probe entry has been hid-safety-reviewer-cleared *before* P0e-20 runs anything against the device. No exceptions.

- [ ] **Step 1:** From section 6.2 of `cross-device-survey.md` (the cmd-15 hypothesis list) and the matrix's `❓` cells, draft a candidate probe list. Each entry has:
  - Cmd byte (hex)
  - Framing class
  - Source family (which family this hypothesis came from)
  - Read/write classification (only "read" enters the probe set; "write" entries are deferred to a later phase with separate gating)
  - Why we expect this to be safe (e.g., "in HS II this is documented as a `Get*` request returning device state")
  - Expected response shape if hypothesis holds
  - Failure mode if hypothesis fails (typically "device returns error byte 0xFF or no response")

Write the candidate list as a fenced block at the top of `2026-04-26-cross-device-probes.txt`.

- [ ] **Step 2:** Cap the probe set at **≤8 entries**. More than that risks fatigue + skipped review. Trim to the highest-information entries.

- [ ] **Step 3:** Dispatch hid-safety-reviewer with the candidate probe list.

Reviewer prompt:
````
Review the candidate probe list at
docs/protocol/captures/2026-04-26-cross-device-probes.txt against the
HID safety invariants in your skill.

For each candidate probe, classify as:
- APPROVED — confidently read-only based on the cited C# source; safe
  to send to LCDV2
- REJECTED — has any write side effect (state mutation, mode change,
  reset, firmware operation), or the source citation does not clearly
  establish read-only semantics
- NEEDS-CLARIFICATION — read-only-ness is plausible but not provable
  from the citation alone

For APPROVED entries, include a one-line rationale citing the source.
For REJECTED entries, the rationale must include the specific side
effect identified.

Return a marked-up version of the candidate list with each entry's
classification.
````

- [ ] **Step 4:** Update the probe-set file with the reviewer's classifications. Only APPROVED entries proceed to P0e-20. NEEDS-CLARIFICATION entries are either re-cited and re-reviewed or dropped. REJECTED entries are dropped with the rationale recorded.

- [ ] **Step 5:** **User confirmation gate.** Before P0e-20 runs anything against the device, the user must read the approved probe list and explicitly say "go." Do not skip this gate even if every reviewer approved.

- [ ] **Step 6:** Commit.

```bash
git add docs/protocol/captures/2026-04-26-cross-device-probes.txt
git commit -m "docs(protocol): Phase 0e probe set — hid-safety-reviewer cleared"
```

---

## Task P0e-20: Run probes against LCDV2 (interactive, user-gated)

**Files:**
- Modify: `docs/protocol/captures/2026-04-26-cross-device-probes.txt` — append per-probe results

This task is **interactive**: it runs `verify-galahad probe ...` once per APPROVED entry, captures the response, and the user reports whether anything visible happened on the device. **One probe at a time**, per the standing user preference for device experiments.

- [ ] **Step 1:** Confirm the user has explicitly said "go" on the approved probe list (P0e-19 step 5). If not, halt.

- [ ] **Step 2:** For each APPROVED entry, in order:
  1. Run the probe:
     ```bash
     cargo run -q -p verify-galahad -- probe 0x<cmd> \
         --framing <a|b|c> --yes-read-only
     ```
  2. Capture the hex output to the per-probe results section in the txt file
  3. Wait for the user to report any visible behaviour (LED colour change, LCD update, sensor blip, nothing)
  4. Record the user's report and a one-line interpretation alongside the hex
  5. **Stop and confirm with the user** before running the next probe — do not batch

- [ ] **Step 3:** After all probes have run, write a summary at the bottom of the file:
  - Probes that returned structured data (the most useful — these are confirmed read paths into the device)
  - Probes that returned device-error bytes (`0xFF` or similar — confirmed not implemented or not allowed)
  - Probes that produced no response (timeout — possibly framing wrong, possibly cmd unknown)
  - Any probe that produced *visible* device behaviour despite being classified read-only — **this is a bug in the safety classification and must be flagged**; halt, file an entry in the survey's "risky-area flagging" section, do not run further probes until reviewed

- [ ] **Step 4:** Update `docs/protocol/galahad2-lcdv2.md` if any probe revealed a new fact about LCDV2 (cross-link to the probes file).

- [ ] **Step 5:** Commit.

```bash
git add docs/protocol/captures/2026-04-26-cross-device-probes.txt \
        docs/protocol/galahad2-lcdv2.md
git commit -m "docs(protocol): Phase 0e cross-device probes — live results"
```

---

## Task P0e-21: Cross-link from existing docs + README

**Files:**
- Modify: `README.md` — Status block adds a Phase 0e bullet
- Modify: `docs/protocol/galahad2-lcdv2.md` — top-of-file note pointing to the cross-device-survey for cross-family context

- [ ] **Step 1:** Update README.md's Status block. Add a bullet under the Phase 0d entry:

```markdown
- ✅ Phase 0e: cross-device command survey + OpenRGB cross-validation +
  read-only probe results (`docs/protocol/cross-device-survey.md`,
  `docs/protocol/captures/2026-04-26-cross-device-probes.txt`).
```

- [ ] **Step 2:** Add a one-line top-of-file note to `docs/protocol/galahad2-lcdv2.md` pointing readers at `cross-device-survey.md` for cross-family context.

- [ ] **Step 3:** Commit.

```bash
git add README.md docs/protocol/galahad2-lcdv2.md
git commit -m "docs: cross-link Phase 0e survey from README + galahad2-lcdv2"
```

---

## Task P0e-22: Phase 0e wrap-up + post-execution notes

**Files:**
- Modify: `docs/superpowers/plans/2026-04-26-phase-0e-cross-device-survey.md` (this file) — append a "Post-execution notes" section

- [ ] **Step 1:** Append a "Post-execution notes" section to this plan, mirroring Phase 0d's wrap-up convention. Cover:
  - Final task count + which tasks were merged or split during execution
  - Number of commits in the phase (use `git log --oneline <starting-commit>..HEAD | wc -l`)
  - Total commands documented across all families (sum the rows in each per-family command table)
  - Top three "gold finding" entries (the divergences or commands most likely to inform Phase 1)
  - Cmd-15 hypothesis status — did any sibling family's command shape match ours? Did probing confirm or refute any hypothesis?
  - Plan deviations
  - Phase 1 plan inputs — a clean recap of section 6.5 from the survey, ready to feed into the Phase 1 plan author

- [ ] **Step 2:** Update `cross-device-survey.md`'s Status line to "Phase 0e complete (static survey + OpenRGB validator + read-only probes)."

- [ ] **Step 3:** Commit.

```bash
git add docs/superpowers/plans/2026-04-26-phase-0e-cross-device-survey.md \
        docs/protocol/cross-device-survey.md
git commit -m "docs: Phase 0e wrap-up — post-execution notes + status update"
```

---

## Phase 0e exit criteria

When all of the following pass, Phase 0e is complete and the Phase 1 plan can be written:

- [ ] `docs/protocol/cross-device-survey.md` exists, covers all 12 device families with citations, has a populated cross-reference matrix, and has a section 6 "Findings & actionable outputs"
- [ ] `docs/protocol/captures/2026-04-26-cross-device-probes.txt` exists with the cleared probe set and either live results or a documented "user declined to run probes"
- [ ] `verify-galahad probe` subcommand exists, has unit tests, and has at least one successful round-trip against LCDV2 documented in the probes capture file
- [ ] `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all -- --check` are clean
- [ ] Section 6.5 of the survey has 3-5 concrete bullets the Phase 1 plan author can lift into the Phase 1 plan's "inputs from Phase 0e" section
- [ ] Every HID write path landed in this phase (just `run_probe`) has a hid-safety-reviewer dispatch in this plan's transcript
- [ ] No C# or OpenRGB source has been pasted into the survey doc (reviewer subagent confirms)

When all pass, the Phase 1 plan author begins.
