# R2 media resource acceptance contract

These engineering limits are fixed before the first R2 measurement. They are
software acceptance targets, not minimum hardware specifications or Windows limits.

## Release workload

`desktop/scripts/media-resource-smoke.py` runs the release MockBackend with
`fixtures/r2-video-four-sensors.json`: nominal 24fps, 480×480 built-in video, four
shadowed sensor widgets and dividers. The owned theme input preserves the earlier
minimum20fps reference workload; all four widgets intentionally bind the same
changing synthetic CPU source. It is a reproducible input, not a saved run report.

After 24 warmup frames, measure 60 seconds without other test/build workloads:

| Measure | Acceptance limit | Measurement |
|---|---|---|
| Completed output | At least 20fps | Reassembled complete mock H.264 frames / actual elapsed time |
| CPU | At most 2 cores averaged | Daemon and codec descendants, including reaped child CPU |
| Resident memory | At most 512 MiB | Summed daemon/codec RSS sampled every 200ms; report sampled peak |
| Sensor age | At most 2000ms and valid | IPC sample age throughout the run |
| Decode | Every captured frame decodes at 480×480 | FFprobe count, compared with completed-frame receipt |
| Stop | No codec child remains | Clear, joined cleanup, then owned daemon shutdown |

Reader and composition queues are separately bounded by source/tests: two
queued output units plus a blocked sender; composition pipe advance uses four
credits. A single-frame placeholder path has capacity one. This queue contract
is not inferred from RSS. Six reader unit tests cover backpressure, blocked first
read cancellation, failure/panic, repeated stop and ownership release.

Build with `cargo build -p lconnect-daemon --example desktop-mock --release --locked`,
then run `/usr/bin/python3 desktop/scripts/media-resource-smoke.py`. The driver
prints its private output root. Keep every result and sample file ignored.
A failed budget remains a failure; changing a budget requires an explicit
recorded rationale and requalification, not adjustment to the observed result.

This short run does not close maximum-duration/size or quota checks, long soak,
physical LCD playback, real sensor providers or Plasma Wayland acceptance.
