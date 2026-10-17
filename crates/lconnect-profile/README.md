# Windows profile inspection and preparation

Device-free codecs for Windows L-Connect3 **2.1.28.0** exports targeting GALAHAD II
LCDV2 (`0416:7395`, ControllerTypeID16846849). The library opens no daemon, HID
backend, user font, source media path or URL. Explicit import/Apply belongs to the
existing daemon transaction. Shared IPC models describe validated native proposals.

## Local container inspection

```sh
lconnectctl --json profiles windows-inspect exported.zip
```

Reports version, group, eligible record indices, field names and fingerprints.
Controller Metadata and user settings values are omitted. This is envelope-only
inspection: `inspection_only=true`, `import_supported=false` and
`asset_payloads_validated=false`. No extraction or asset publication occurs.

## Cooling policies from a selected record

```sh
lconnectctl --json profiles windows-cooling exported.zip --record INDEX --cpu-sensor SENSOR_ID
```

Requires an explicit zero-based record and CPU source ID. All8 fan/3 pump modes,
selected IDs, nullable-name origin and source RPM/chart data are retained. Output
contains native `cooling_config` and separate `cooling_banks`; named Linux slots
are not used as an8-to-4 substitute. Empty/default names retain factory origin;
explicitly typed names remain literal. CPU availability is not verified offline.

Options: `--hysteresis-c` defaults0 (0..10), `--fallback-percent` defaults100
(20..100). The explicit sensor ID must be nonblank, contain no control characters
and fit4096 bytes. Unsupported source endpoints/duplicate knots, decreasing
speeds, StartStop, non-CPU references and unrepresentable profile sizes reject the
whole preparation, including inactive modes. RPM conversion uses the source
phase contract and native policy validation/evaluation.

`policy_conversion_only=true`, `group_import_supported=false`,
`apply_supported=false`, `sensor_binding_verified=false`,
`source_presentation_metadata_validated=false`. Screen/assets remain unconverted;
source chart metadata is history, not an executable Linux policy.

## Lighting bank inspection

```sh
lconnectctl --json profiles windows-lighting exported.zip --record INDEX
```

Reads complete6/16 static banks, selected modes/topology, all96 sensor High/Low
settings and per-mode selection/enabled state. Missing/unknown keys, wrong enum
IDs, malformed palettes/thresholds or invalid inactive data reject the report.
Source int.MinValue sentinels and source ranges remain distinct from real values.
No Linux source binding or group import is implied.

Stored WPF Color objects differ from runtime RGB arrays: profile/editor exports
use default serialization; the runtime path explicitly adds ColorToIntArray.
Both forms normalize to byte RGB. Original WPF property values are retained in
`source_color_objects` with JSON-pointer keys, outside executable policies. RGB
triplet fixtures remain supported but do not prove the default WPF storage shape.
Non-null color contexts, missing fields and unknown color properties reject
instead of being read from the host or replaced with black. Genuine export and
Windows deserialization/raster qualification remain separate.

## Typed Linux lighting preparation

```sh
lconnectctl --json profiles windows-lighting exported.zip --record INDEX --bindings bindings.json
```

The bounded16KiB binding document requires all six distinct explicit roles:

```json
{
  "CPULoad": {"sensor_id":"CPU_LOAD_ID","unit":"percent"},
  "CPUTemperature": {"sensor_id":"CPU_TEMP_ID","unit":"celsius"},
  "GPULoad": {"sensor_id":"GPU_LOAD_ID","unit":"percent"},
  "GPUTemperature": {"sensor_id":"GPU_TEMP_ID","unit":"celsius"},
  "PumpRPM": {"sensor_id":"PUMP_ID","unit":"rpm"},
  "CoolantTemp": {"sensor_id":"COOLANT_ID","unit":"celsius"}
}
```

IDs/units are never inferred from Windows device Metadata. All inactive mode
bindings are required. High brightness/direction and supported color/speed ranges
follow the source/native evaluator contract; the selected binding is retained.
Unsupported directions, sentinels, thresholds or topology outside Linux8..50
reject without clamping or discarding modes. Output has `source` and `lighting`,
`preparation_only=true`, `group_import_supported=false`, `apply_supported=false`
and `sensor_binding_verified=false`. Host source availability is not established.

## Complete FanAndLighting preview/import

```sh
lconnectctl --json profiles windows-control exported.zip --record INDEX --bindings bindings.json
lconnectctl --json profiles windows-control exported.zip --record INDEX --bindings bindings.json --apply
```

The default validates locally and requests a target-bound read-only proposal.
Explicit Apply uses the existing daemon control-group coordinator/journal/receipt.
Source8/3 cooling banks,6/16 RGB banks,96 dynamic bindings and requested IDs travel
together. Linux named extension slots, Screen/assets and global thermal settings
are preserved. Cooling receives exactly the explicit CPUTemperature binding.
Original RPM and lighting data, including WPF color metadata, remain private
history rather than executable settings.

The CLI prints operation ID/original target token before admission. After an
uncertain result use `profiles group readback fan-and-lighting ID`. Explicit retry
requires `--apply --id ID --expected ORIGINAL_TOKEN` and unchanged source/bindings;
it never substitutes a fresh token. Admitted caller loss does not cancel commit.
Newer manual intent/slot edits are not overwritten by original-ID receipt reads.

Draft admission is bounded256KiB; private lighting history128KiB and total settings
write1MiB still apply. Metadata does not bypass these limits. Desktop native
chooser/bindings/preview/Apply/recovery are qualified with private EN/RU fixtures.
Genuine Windows exports, unsupported curves and extended lifecycle/platform gates
remain open. See the [current contract](../../docs/development/parity/windows-profiles.md).

## Screen banks and uploaded payload inventory

```sh
lconnectctl --json profiles windows-screen exported.zip --record INDEX
```

Retains brightness/rotation, all eight nullable names/selected effects/Style Express
IDs, complete editor dictionaries and exact embedded JSON strings/hashes. Runtime
`ScreenConfigJson` and editor settings are different documents. Empty strings
retain Windows default intent; nonempty JSON must be bounded objects without
duplicate keys. This inspection does not validate inner theme semantics.

The three galleries retain source references without opening host paths. Uploaded
basenames identify flat `wmsq/` candidates; missing payload/default references stay
explicit. A default never inherits an upload with the same basename. Every
payload, including unused uploads and thumbnails, is streamed through CRC/SHA-256
without extraction. Nested uploads reject instead of being silently flattened.
`payload_streams_verified=true`, `media_decoded=false`,
`theme_semantics_validated=false`, `group_import_supported=false` and
`apply_supported=false`. This explicit report contains user text/media paths but
not controller Metadata. A report is not an immutable grant or admission token.

## Basic Screen runtime proposal

```sh
lconnectctl --json profiles windows-screen exported.zip --record INDEX --bindings screen-bindings.json
```

The [mapping document](../../docs/development/examples/windows-screen-bindings.json) supplies
an explicit native name, roles/IDs/labels/units/precision/temperature choices,
Linux fonts and exact source path/URI→managed resource keys. Substitution of a
bundled font is a user choice, never inferred equivalence. Source defaults still
require explicit Linux mappings. Managed fonts/images and video frames must exist
for rendering; they are never silently replaced.

`runtime.theme` is a valid LcdTheme for Full Image/Color with None/Custom/Time/
Slider/Duo/Three/Four sensor overlays and watermark. It preserves source role
order, cycle interval, source colors/family CSS sizes, opacity/shadow and typed
units. Images stretch480×480; watermarks use native contain/center conventions.
Custom removes empty newline entries; Time uses date/12-hour/no-seconds/period.

Layout review remains mandatory: native Duo/grid geometry, divider decorations,
font-specific margins and Windows raster are not qualified. The result retains
`layout_review_required=true`, `pixel_parity_verified=false`,
`resource_payloads_verified=false`, `sensor_binding_verified=false`,
`editor_banks_converted=false`, `apply_supported=false`. It is not a full Screen
importer and does not reconstruct inactive settings from runtime output.

## Basic editor bank preparation

```sh
lconnectctl --json profiles windows-screen exported.zip --record INDEX --bindings screen-bindings.json --editor-banks
```

Explicit research mode prepares existing ImageVideo/BackgroundColor entries for
all eight source array positions. These are not Linux named-profile slots.
Every bank keeps all seven overlay proposals, its selected overlay and original
settings hash; names/indices/effect selection remain source data. Every
inactive overlay is validated and requires its explicit bindings. All four Color
settings, including inactive palettes/Flip and independent Solid, survive in every
native Color proposal. Only an absent selected basic bank at source index0
materializes defaults, matching Windows page Import(empty); source_absent_materialized
distinguishes that from an explicitly empty settings string. Other missing banks
are not fabricated. Other effects stay
explicitly deferred in the report and source inspection.

An explicitly empty settings string materializes versioned constructor defaults.
ImageVideo follows the source default-video gallery lookup, but the resulting
source path still requires an explicit prepared-resource binding. Editor path
strings are used exactly; runtime URIs may need a separate explicit alias. This
preparation does not assume a Windows install directory or open a source path.
It can run independently of an empty/deferred active runtime config.

`basic_editor_banks_prepared=true` does not mean complete Screen import:
`complete_screen_group_supported=false`, `apply_supported=false`. Layout, asset
validation/remap, native editor/persistence integration and target-bound Screen
commit remain open. [Contract](../../docs/development/parity/windows-profiles.md).

## Verified static image preparation

```sh
lconnectctl --json profiles windows-screen exported.zip --record INDEX --prepare-images
```

Revalidates the source backup and selected payload size/CRC/SHA-256, then uses the
existing lconnect-image decoder/normalizer for PNG/JPEG uploads, including unused
images and thumbnails. No source JSON path is opened. The library holds owned
normalized PNG buffers, but reports only hashes/dimensions/size; buffers are not
serialized. No files are published or managed keys assigned. All selected images
must succeed before a CLI result is emitted. GIF/video/unknown entries stay
explicitly deferred; disguised GIF and APNG reject instead of losing animation.

Encoded input≤16MiB, decoder8192px/64MiB, normalized PNG≤1MiB and total owned
buffers≤32MiB. `images_decoded=true` qualifies only selected static images;
`assets_published=false`, `complete_screen_group_supported=false`. This is not
an immutable admission grant, full media qualification, resource remap or Screen
commit. The option is independent of and conflicts with runtime/editor bindings.
[Contract](../../docs/development/parity/windows-profiles.md#verified-static-image-preparation).

## Verified GIF/MP4 preparation

```sh
lconnectctl --json profiles windows-screen exported.zip --record INDEX --prepare-media
```

Uses the shared device-free lconnect-media engine after source backup/payload
fingerprint verification. Fixed private input files feed Stretch480×480/24fps
preparation; no Windows path/URI is opened. GIF motion survives as multiple frames.
PreparedFile owners retain encoded H.264 privately, while report serialization
omits files/bytes/paths. Source/encoded hashes, frames and sizes remain visible.
No publication, key assignment, device writes or partial output on failure.

Cancellation is checked during archive reads and media-tool work; the CLI joins
its blocking worker before returning from Ctrl+C. Per-clip existing bounds remain;
batch limits64entries, one hour of frames and512MiB retained encoded data use the
remaining budget in the same engine. Static images/other entries remain deferred.
This still does not create a complete Screen group or verified resource remap.
[Contract](../../docs/development/parity/windows-profiles.md#verified-gifmp4-preparation).

## Owned resource-remap plan

`screen_plan::inspect_and_prepare` verifies and owns all image/video uploads,
resolves exact Windows path/URI aliases to provisional keys or explicit native
resources, and validates galleries/current basic runtime/all editor banks as one
all-or-error result. Existing files/fonts need actor pins; nothing is published
or applied. Private bytes and file owners are omitted from JSON.
CLI: `windows-screen source.zip --record INDEX --bindings bindings.json --resource-map resources.json`.
[Contract and map example](../../docs/development/parity/windows-profiles.md#owned-screen-resource-plan).

## Native basic editor candidates

`PreparedSlot::native_theme()` retains available basic background banks and all seven
independent overlay templates in `LcdTheme.basic_editor`. Selected deferred effects
reject rather than substitute. `ScreenPlan` prepares only source index0 as
`current_editor_theme` / `current_editor_deferred`; `inactive_editor_indices`
lists1..7, retained opaque in source. Its editor_banks scope is current_editor.
The explicit --editor-banks research mode retains all_source_entries scope.
No source array position becomes a Linux named slot. Native collectors, portable
remapping and JSON/TOML preserve inactive assets/state. Current source runtime
remains separate; no publication/target commit is implied. Existing native bank
UI does not establish a Windows array→named-profile correspondence.
[Contract](../../docs/development/parity/windows-profiles.md#native-basic-editor-storage).

## Bounds and format

Generic envelope:8GiB archive/declared content,4096 ZIP members,4MiB central
directory,16MiB backup and8MiB controller JSON. Directory extent/counts are checked
before indexing, including ZIP64. Only Stored/Deflated; unsafe/case-colliding
names, encryption/special entries, comments, trailing data and prepended archives
reject. Backup CRC, strict JSON and base64 are checked. Generic inspection does
not decompress media; Screen inventory additionally checks bounded payload streams.

Screen limits:512KiB per embedded document,256 references per gallery,256MiB per
payload,1GiB total payloads and64KiB bindings. These are host budgets, not declared
Windows limits. Typed parsing rejects unknown/duplicate fields and excessive
nesting. Existing total settings/storage constraints still apply at commit.

Stored cooling banks carry all8/3 modes, selection identity and source metadata
(64KiB/channel) independently of named slot libraries and active policies.
Persistence, native load-to-draft/Save/Apply/restore, Linux archives and restart
are documented in [cooling modes](../../docs/development/parity/cooling-modes.md).

ZIP4.6.1 declares Rust1.82 support; the current workspace toolchain is Rust1.96,
not a fresh minimum-toolchain qualification. Deflate uses the Rust backend.
Fixtures are owned synthetic .NET DTO/ZIP and Rust/Python structural values.
WPF property-shape emulation is not an actual WPF application export. No vendor
assembly, proprietary asset or physical device is required for these tests.
