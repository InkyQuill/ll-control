# Shared media preparation

Device-free GIF/MP4→480×480/24fps H.264 preparation, cut/range validation,
GA2LCD video adjustments, thumbnails and immutable MP4 preview owners. Runs on
blocking workers; it owns/reaps FFmpeg children and private scratch directories.
No Tauri, daemon, publication or HID dependency. Desktop retains its existing
`media_prepare`, `video_range` and `video_adjustments` APIs through re-exports.

`prepare_file` uses the original per-clip frame/byte bounds. The new
`prepare_file_budgeted` takes remaining frame/byte limits for a batch and validates
those before source access; all decoding/encoding/packet validation is shared.
Cancellation must remain admitted until the blocking worker returns, not merely
until a request future is dropped. `PreparedFile`/preview leases keep their own
files alive; dropping the final owner cleans its scratch directory.

The `test-support` feature exposes the staging-path accessor for ownership tests;
it is not needed by production desktop/CLI consumers. Numerical channel fixtures
and all original preparation/range/adjustment tests moved with the implementation.
They are owned reference vectors, not vendor code or media.

```sh
cargo test --locked -p llcontrol-media
```

Tests require FFmpeg/ffprobe/libx264 (and the existing VP9 fixture encoder).
[Preparation contract](../../docs/development/parity/media-preparation.md) and
[Windows resource pipeline](../../docs/development/parity/windows-profiles.md).
