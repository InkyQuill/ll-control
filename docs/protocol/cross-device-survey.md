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
