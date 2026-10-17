# L-Connect Linux

Linux-native client for Lian Li hardware. Current target: **Galahad II LCDV2
(`0416:7395`)**, with **L-Connect 3 2.1.28.0** as the Windows parity baseline.

> **Software preview:** no hardware-ready release is claimed. The [R0 release contract](parity/release-contract.md) declares core candidates, experimental features and unavailable capabilities. First release targets Plasma Wayland after R1–R3 qualification.

## Status (2026-10-16)

The daemon, CLI and Tauri desktop implement image display, GIF/MP4 import and
managed playback, LCD composition/themes, sensors, cooling and RGB policies,
profiles and settings. Screen capture/recording have isolated native acceptance;
real portal/backend qualification remains open. English is the default and
Russian is required. Mini is a Plasma widget and GNOME Shell extension, with
both integrations included in the system package; there is no standalone Mini
window. Broader shell/session and hardware acceptance remains open.
See [desktop usage](../../desktop/README.md).

- M0: versioned feature matrix and protocol research complete.
- M1: JPEG and diagnostic H.264 physically observed on FW1.6; a 30-minute
  sensor-enabled animation soak passed. Reconnect/sleep/cold-boot acceptance
  remains incomplete. Cmd15 activation/offline applicability is unresolved.
- M2: typed hwmon/procfs/device sensors, fan/pump presets and curves,
  persistence, fallback/hysteresis and thermal overlays are software-tested.
  AMDGPU/NVIDIA load/VRAM, Intel per-engine load/memory and RAPL CPU power are
  implemented. Intel aggregate equivalence, further voltage providers and
  physical cooling/thermal acceptance remain open.
- Verification is recorded by slice in [implementation status](parity/implementation-status.md).
  The earlier 416-test M2 result is not the final state of the current tree.
- [Production package tooling](parity/package-delivery.md) includes GUI,
  CLI/daemon, systemd/udev and both Mini integrations. GitHub CI is configured
  in source; remote execution, real autostart/session and release qualification
  remain open. Safe firmware Apply/recovery is still under development.

See the [current roadmap](../../roadmap.md), [active plans](superpowers/plans/README.md)
and [scenario matrix](parity/lcdv2-2.1.28-matrix.md). Completed or superseded implementation plans were removed after results and
remaining tasks were consolidated. Research snapshots and captures are retained.
Software tests and successful USB writes do not establish full physical parity.

## Hardware coverage (current)

| Device | VID:PID | Status |
|---|---|---|
| Galahad II LCDV2 ("Vision") | `0416:7395` | JPEG + diagnostic H.264 observed; sensor/cooling/RGB daemon and CLI implemented, full acceptance pending |
| Everything else from L-Connect 3 | various | future phases |

## Getting started

Use the [quickstart and troubleshooting guide](quickstart.md) for build,
installation paths, daemon/GUI startup, themes, sensor binding and recovery.
It separates offline verification from commands that access the device.
The [full acceptance checklist](parity/basic-parity-checklist.md) records
software results and deferred hardware checks.

## Saved firmware recovery state

`lconnectctl firmware-recovery` (or `--json firmware-recovery`) reads saved
firmware journal/settings guards through the daemon, including when the LCDV2 is
absent or ordinary maintenance admission is closed. It does not query HID,
retry an update, clear quarantine, or replay settings. Last durable progress can
lag actual writes; this report is separate from live job status and update success.
See the [firmware contract](parity/firmware-lcdv2.md) for remaining updater gates.

## License

GPL-3.0-or-later.


## M1 diagnostic LCD control

The development roadmap targets the available **Galahad II LCDV2 (0416:7395)**.
See [roadmap.md](../../roadmap.md) and the [M1 evidence](protocol/captures/2026-09-08-lcdv2-m1.md)
for the current verification status. Other product families are not hardware verified.

The daemon and CLI require an absolute existing `XDG_RUNTIME_DIR` and share
`$XDG_RUNTIME_DIR/lconnectd.sock`. `LCONNECTCTL_SOCKET` remains an explicit CLI override.
Brightness, rotation, and mode commands patch only the selected field.
`requested` records the user configuration; `applied` records the last successfully
sent control configuration, including the internal `LCDSetting` mode used during
diagnostic playback. Neither value is a readback of the physical screen.

```sh
lconnectctl lcd state
lconnectctl lcd brightness 70
lconnectctl lcd rotation 0
python3 tools/prepare-lcdv2-clip.py --output /tmp/lcdv2-clip.json
lconnectctl lcd play-clip /tmp/lcdv2-clip.json --loops 30
lconnectctl lcd playback-stats
lconnectctl lcd clear
```

The diagnostic clip is 480×480, 24 fps, baseline H.264, split into complete
access units. Playback requires firmware >=1.5 and rotation 0; it uses C-framing
and a single 20 ms read observation after each frame. Empty readback is a timeout,
not a failed ACK. Counters describe frames **sent**, not frames visibly displayed.
Sensor requests keep one pending response in the HID actor, allowing frames to
continue while waiting. Valid sensor replies can arrive during a frame observation;
they update telemetry and remain raw readback, without being treated as frame ACKs.
`clear` waits for the current frame and the LocalUI write before returning.

Diagnostic clips and standalone JPEG sources are not persisted (managed theme
assets and themes have separate persistence). Restart/reconnect leaves a diagnostic source Idle;
play again explicitly. Stop a diagnostic clip before suspend: automatic sleep-event
handling is deferred to M3/M4. Physical hotplug and cold-boot evidence remain separate
from mock lifecycle tests.

Settings now use VID/PID plus hex-encoded serial. Serial-less devices use a singleton
key; indistinguishable units are rejected. Replacing a serial-less unit reuses that
singleton's settings. Legacy `galahad2-lcdv2:0001.toml` files are retained but never
automatically assigned: reapply known settings to create the stable-key file.

## Host sensors

`lconnectctl sensors` (or `lconnectctl --json sensors`) reads the daemon's cached
catalog. Providers cover hwmon temperatures and GPU power, procfs CPU/RAM,
AMDGPU load/VRAM/voltage/clocks, NVIDIA temperature/load/VRAM/power/clocks,
RAPL CPU power, CPUFreq and Intel GT clocks, i915 per-engine counters,
network RX/TX rates and mounted filesystem capacity. LCDV2 coolant/RPM are
merged under the attached device identity. Driver and permission restrictions
are documented in the [provider inventory](parity/sensor-provider-inventory.md).
Intel hwmon voltage is implemented for supported i915/xe rails. Intel memory query is implemented for i915/xe with explicit unavailable cases;
Windows aggregate load semantics, CPU voltage on remaining platforms and
broader NVIDIA platform coverage remain incomplete.
Optional NVIDIA core voltage uses NV-CONTROL with explicitly configured
`LCONNECT_NVIDIA_VOLTAGE_DISPLAY` and `LCONNECT_NVIDIA_VOLTAGE_XAUTHORITY`
(absolute path), plus `/usr/bin/nvidia-settings`. It is disabled without both
variables; the daemon does not discover or copy desktop credentials. See
[setup, source semantics and limitations](parity/nvidia-voltage-abi.md).

ASUS WMI CPU core voltage can be explicitly enabled with
`LCONNECT_ASUS_WMI_VOLTAGE=1` in the daemon environment. It requires the existing
`asus_wmi_sensors` driver and an unambiguous `CPU Core Voltage` label. Kernel
documentation records BIOS polling defects on some boards; this extra voltage
query is off by default. No driver is loaded or firmware changed. See the
[provider inventory](parity/sensor-provider-inventory.md) for source semantics
and limits.

Production NVIDIA readings require `/usr/bin/nvidia-smi`; filesystem capacity
requires util-linux `/usr/bin/findmnt` with the JSON columns listed in the
inventory. Missing helpers or unsupported output produce unavailable/invalid
readings. No installation or privilege escalation is performed automatically.
Filesystem total, used, free, available and used percent are separate metrics;
reserved space is not reported as used. Network totals include virtual traffic.

Each reading has an ID, label, unit, optional value, status, observation time
and age. Missing/invalid/stale readings never substitute zero or another sensor.
Freshness expires after three seconds. Stable identity rules differ by provider;
labels and enumeration numbers do not define identity. CPU/network rates need
an initial sample before a delta is available.

One collector publishes snapshots for cache-only IPC. Filesystem/sysfs reads
run in a blocking worker; NVIDIA and storage helpers run asynchronously with
output limits and deadlines, outside the HID actor. Collection is sequential,
with a one-second delay between cycles; it does not guarantee a 1 Hz sample rate.
The `host-sensors` diagnostic example uses the synchronous provider path and
therefore does not demonstrate asynchronous NVIDIA/storage collection.
Cooling curves, fallback/hysteresis, persistence and thermal alerts are already
implemented; remaining acceptance is in the [M2 plan](superpowers/plans/2026-09-09-m2-sensors-and-cooling.md).

## FPS source settings

Settings → FPS source can enable or disable reading an existing MangoHud
per-frame CSV and save its absolute path. Changes apply without restarting the
daemon; disabling retains the selected path. English is the default interface
language, and Russian is supported.

Alternatively, use `lconnectctl fps show --json`, then
`lconnectctl fps set --revision N --enabled true --log-path /absolute/path/game.csv`
with the returned revision. Set replaces the whole record; include the path when
disabling if you want to keep it. Configure MangoHud `log_interval=0` separately;
the app does not enable game logging. Mini uses a selected daemon FPS source in
its Plasma widget or GNOME Shell extension. See the [FPS contract](parity/fps-source.md)
for source identity, stale data, supported input and remaining qualification.

## M2b autonomous cooling

`lconnectctl sensors` now also lists the attached LCDV2's coolant, fan RPM and
pump RPM under its stable device identity. These readings become stale after
three seconds without a successful poll. A curve can bind a host temperature or
the target device's coolant, but cannot bind utilization, RPM, or another AIO.

Inspect state with `lconnectctl cooling state`. Apply a JSON policy with:

```sh
lconnectctl cooling set fan --policy /path/to/fan-policy.json
lconnectctl cooling set pump --policy /path/to/pump-policy.json
```

A fixed-duty policy is `{"mode":"fixed","percent":80}`. A pump-only motherboard
policy is `{"mode":"motherboard","percent":80}`; source1 means motherboard,
not application control (the old `pump --source application` name remains a
compatibility alias). Fixed and curve policies use source0 for the pump.

Example curve schema; replace `sensor_id` with the exact temperature ID from
`sensors`, and choose duties appropriate for the connected cooling hardware:

```json
{
  "mode": "curve",
  "sensor_id": "<temperature ID from sensors>",
  "points": [
    {"celsius": 30, "percent": 40},
    {"celsius": 70, "percent": 100}
  ],
  "hysteresis_c": 2,
  "fallback_percent": 100
}
```

Policies accept 2–16 points with increasing temperatures and nondecreasing duty.
New fan policies use 10–100%; pump policies use 20–100%. These are percent duties,
not calibrated RPM targets. Rising temperature takes effect on the next tick;
falling temperature must cross the configured hysteresis. Missing, stale or
invalid temperature bypasses hysteresis and selects the explicit fallback.

The cooling actor runs without a GUI, serializes manual and policy changes, and
uses the existing HID owner. It stores requested policies with the existing
settings debounce/flush mechanism, rather than persisting every computed duty.
Settings write failures are logged by that mechanism. State separates requested
policy, last successfully applied duty, fallback state and current control error.
A failed initial HID write retains policy intent, reports an error and is retried
on later ticks; the CLI returns unsuccessful status for that initial failure.

`lconnectctl fan <percent>` or `lconnectctl pump <percent> --source mcu` cancels the
corresponding policy after a successful manual write. The policy and legacy
manual setting update together. On restart, a persisted policy owns its target
and suppresses conflicting legacy PWM replay. Missing temperature starts in
fallback; an invalid saved policy remains visible without generating its writes.
Stopping the daemon does not restore earlier physical PWM. For hardware tests,
restore known working manual settings before shutdown.

Pump RPM policies are now available:

```json
{"mode": "fixed_rpm", "rpm": 2200}
```

`fixed_rpm` and `curve_rpm` are pump-only, with targets from 800 to 3600 RPM.
They convert targets using the LCDV2 Windows 2.1.28 duty table; they do not
regulate measured RPM in a feedback loop. Actual RPM still needs hardware
acceptance. The existing `curve` mode retains percent interpolation.

Generate an editable Quiet or Performance pump policy without contacting a daemon:

```bash
lconnectctl cooling preset quiet --sensor 'CPU_TEMPERATURE_ID' > /tmp/pump-quiet.json
# Inspect/edit the file, then apply when appropriate for your hardware:
lconnectctl cooling set pump --policy /tmp/pump-quiet.json
```

Replace `CPU_TEMPERATURE_ID` with the chosen ID from `sensors`. `performance`
is the other preset. Generation defaults to 2°C falling hysteresis and 100%
fallback; both can be set with `--hysteresis-c` and `--fallback-percent`.
Windows preset points reference CPU temperature; Linux also permits an explicit
coolant or other temperature binding, which changes that scenario's behavior.
For Windows arithmetic at each fresh sample, select `--hysteresis-c 0`.

Generated `curve_rpm` JSON contains 2–16 increasing integer `celsius` points
with nondecreasing `rpm`, plus `sensor_id`, `hysteresis_c`, `fallback_percent`.
The daemon truncates accepted Celsius, interpolates and truncates target RPM,
then interpolates and truncates PWM through the reference table. It persists
these RPM points, so restart preserves their meaning. Missing data uses the
explicit percent fallback without converting it from RPM.

Fan presets are available below; physical cooling acceptance remains pending. Thermal RGB alerts are available as described below. See the [M2 plan](superpowers/plans/2026-09-09-m2-sensors-and-cooling.md).


## RGB command ordering (M2c foundation)

Manual lighting modes and direct per-LED colors now share the daemon's lighting
owner with periodic refresh. A successful mode command acknowledges the HID
write and queuing its settings update; disk persistence still uses the existing
debounce/flush mechanism. A failed write retains the prior refresh state.

Direct per-LED output is retained for refresh until replaced, so an older mode
cannot overwrite it on the next tick. It is transient: daemon restart restores
the last persisted mode. The internal clear operation stops refresh without
changing physical LEDs or saved settings. With thermal alerts enabled, clear is rejected until alerts are disabled and the base is restored. Direct updates remain available.


## Thermal RGB alerts (M2c)

`lconnectctl thermal state` reports requested rules, the active source, degraded
inputs, remaining manual grace and per-zone applied layer/error. Applied means
the HID write succeeded; visual acceptance on the physical LCDV2 is pending.

First configure both fan and pump lighting modes through the existing lighting
commands; these form the restorable base. Then create a JSON config using IDs
from `lconnectctl sensors`:

```json
{
  "cpu": {
    "sensor_id": "CPU_TEMPERATURE_ID",
    "threshold_c": 85,
    "color": {"r": 255, "g": 0, "b": 0}
  },
  "gpu": null
}
```

The threshold above is illustrative. Replace the ID and threshold for your
chosen sensor. Both CPU and GPU slots accept an explicit known Celsius binding;
there is no automatic selection or remapping. A null slot disables that rule.

```bash
lconnectctl thermal set --config /path/to/thermal.json
lconnectctl thermal state
lconnectctl thermal disable
```

At or above the threshold, static warning color overrides both zones. The most
recently triggered source wins; GPU wins simultaneous new triggers. Cooling
below threshold removes that source and exposes the previous warning or the
base. Missing/stale/invalid readings retain an already active warning and report
its source as degraded; an initially missing reading does not create a warning.
The loop evaluates at least once a second when I/O is responsive, while retaining
normal RGB refresh cadence. HID failures retry on later ticks and report each
zone separately. Failed configuration application can retain the requested
rules; the CLI prints state and exits unsuccessfully on application errors.

Manual mode changes restore both base zones first (including an old pump scope),
then apply and save the new mode. They give manual lighting ten seconds before
an active warning resumes; repeated changes restart that period. Rule updates
apply immediately. Reapplying identical rules preserves priority and stale
warning history. Alert color never replaces the saved base, and restoration
issues no cooling/LCD command. Rules persist with the existing settings debounce.

Direct per-LED output can also be a base. The daemon enters the Application
source before its first color packet, retains the addressed prefix, and replaces
only that prefix with warning color. `fan.direct_led_count` and
`pump.direct_led_count` in `thermal state` show this coverage; null means a normal
mode base. Unknown physical LEDs beyond the retained prefix are not included in
the warning. This is not a claim of whole-zone or hardware parity.

A shorter direct update replaces the prefix and retains the previous known tail.
Direct updates during an alert restore the old output first, apply the new colors
and restart the ten-second grace period. On warning completion, the complete
retained prefix is restored. Normal direct refresh does not repeat the mode
switch; errors with unknown applied state retry preparation before color data.
Direct colors remain transient and never replace persisted lighting modes.

On restart, saved rules evaluate against fresh cache readings; transient trigger
order and manual grace are not persisted. Invalid saved rules or missing base
modes are reported as blocked. Stop after `thermal disable` has restored the
base if you want ordinary lighting left on the device; shutdown itself does not
promise a final restoration write. Physical RGB acceptance has not been run.


## Fan presets (M2)

Generate Quiet, Standard, High or Full without contacting a daemon:

```bash
lconnectctl cooling preset standard --target fan --sensor 'CPU_TEMPERATURE_ID' > /tmp/fan-standard.json
```

Names are `quiet`, `standard`, `high`, `full`. The existing pump syntax and
`--target pump` use `quiet`/`performance`; omitting `--target` still selects pump.
Invalid target/preset combinations fail before IPC. Generation does not apply
hardware settings. Apply the inspected file with `cooling set fan --policy FILE`
only to fans controlled by this device. Fans connected to motherboard headers
are outside this controller's fan output.

Fan presets generate editable `fan_curve` JSON: five Windows2.1.28 temperature/PWM
points, an explicit sensor binding, falling hysteresis and percent fallback.
The daemon truncates Celsius, interpolates duty with the reference operation
order, then truncates duty. For example Quiet at26.9°C sends21%, while ordinary
`curve` with the same points sends23% because it keeps fractional Celsius and
rounds duty upward. Existing `curve` behavior is unchanged.

`fan_curve` is fan-only, accepts2–16 strictly increasing integer Celsius points
and nondecreasing10–100% duties. The preset points use CPU temperature in Windows;
select that binding for the same scenario. The default2°C hysteresis and explicit
100% fallback are Linux extensions; use `--hysteresis-c 0` for reference sampling
arithmetic. Full still uses the explicitly chosen fallback if its sensor is lost.

Windows UserDefine1–4 start with Quiet's points. Editable JSON provides custom
curves today; four named slots, renaming and Windows profile import remain part
of the later profile workflow. Physical fan RPM acceptance is not yet performed.
