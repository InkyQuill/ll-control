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
