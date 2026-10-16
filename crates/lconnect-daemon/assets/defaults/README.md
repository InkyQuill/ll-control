# Original builtin video

Copyright 2026 Pavel Obruchnikov <me@inkyquill.net>.
Distributed under the repository license.

`aurora-v1.h264` (Сияние) is an original ambient background: two slowly orbiting
teal/violet lights, dark enough for white sensor overlays. It contains 576 frames
at 24 fps, 480×480, baseline H.264 level3.0, yuv420p, IDR every24frames,
AUD and repeated SPS/PPS. The 24-second motion is periodic. It is not a copied
Windows asset or a claim of visual equivalence with LIAN LI defaults.

Regenerate with `python3 tools/generate-lcd-default-video.py` (Python standard
library + FFmpeg/libx264). The generator also writes `aurora-v1-index.rs`.
Encoder-version differences can change bytes: committed encoded bytes and index
are authoritative. Never change existing versioned content in place; add a new
key/version for different artwork or encoding. Current encoder: FFmpeg n9.0.2.

The daemon embeds bytes/index; playback allocates and validates one AU per read.
No runtime encoder, bootstrap directory, user quota or entire-video copy is
required. The reserved 32-hex key is compatible with existing themes/profiles.
Disk files under that key cannot shadow the builtin; upload/delete are rejected.
