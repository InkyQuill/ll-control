# Video channel reference vectors

`video-effect-channels.bin` contains numerical outputs generated from the user's
Windows2.1.28 SDK exports in an isolated x86-64 emulator, not vendor code/assets.
Two banks of256 rows ×256 bytes: Contrast then Lightness. Row is amount0..255;
column is input gray channel0..255. Input RGB24 width256,height1; MXCSR0x1f80.
Each call returned to a sentinel; instruction limit prevents incomplete results.
No Windows loader initialization, GUI, sensors or devices were executed.

Provenance, external probe path/hash and DLL SHA256:
[checkpoint](../../../../docs/parity/evidence/2026-10-12-video-adjustments-kernel.json).
The SIMD results do not qualify scalar/tail paths or the Windows video pipeline.
