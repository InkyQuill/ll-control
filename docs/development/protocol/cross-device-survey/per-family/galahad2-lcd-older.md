## Family: Galahad II LCD (older, pre-Vision)

**Source folder:** none — no dedicated `Galahad2LCD/` product folder exists in
`LConnectCore/Products/`. Only `Galahad2Trinity/` and `Galahad2Vision/` are
present in the decompiled tree.
**ControllerTypeID enum value:** `Galahad2LCD = 16846848` cited from
`LConnectCore/Devices/ControllerTypeID.cs:18` (also `ControllerClass.cs:15`
where it equals `17`).
**USB VID:PID:** `0x0416:0x7391` (decimal `1046:29585`) cited from
`LConnectCore/Devices/KnownDevices.cs:39`.
**LCDV2 sibling for comparison:** `0x0416:0x7395` (decimal `1046:29589`) at
`LConnectCore/Devices/KnownDevices.cs:41`.

### Static-analysis findings

A repository-wide search for `Galahad2LCD` (excluding the `Galahad2LCDV2`
substring) in `L-Connect.Core/` and `L-Connect3/` returns hits only in:

- `LConnectCore/Devices/ControllerClass.cs:15` — enum value
- `LConnectCore/Devices/ControllerTypeID.cs:18` — enum value
- `LConnectCore/Devices/ControllerTypeIDDefinitions.cs:43,90` — class constant
  and an OR-mask of all controller-types
- `LConnectCore/Devices/KnownDevices.cs:39` — VID/PID registration
- `L-Connect3/LConnect3cs/Views/Pages/Settings/Galahad2LCDDevice.cs` — a WPF
  `UserControl` shell (no protocol logic; just `InitializeComponent` and a
  `ListView` field)

There is **no** `Galahad2LCDDevice` protocol class, no factory or dispatcher
branch that constructs a Vision/Trinity device for the older LCD enum value,
and no `if (controllerType == ControllerTypeID.Galahad2LCD)` branching inside
`LConnectCore/Products/Galahad2Vision/Galahad2VisionDevice.cs`
(`Galahad2VisionDevice` ctor at line 48 takes only an `IHidDevice`; lines
1-398 contain no `ControllerType` field access or SKU-keyed conditionals).

The hypothesis from the survey plan ("the older LCD is one of the SKUs that
`Galahad2Vision/` serves via ControllerTypeID branching") is **not confirmed
by the decompiled source**. The older `Galahad2LCD` enum value is registered
to a PID and exposed in a UI control list, but the decompiled tree contains
no device-class implementation that handles it.

### Notable divergences from LCDV2

Cannot be enumerated from the decompiled source — there is no older-LCD
device class to compare against `Galahad2VisionDevice`. The wire-format
command set for the older variant is therefore **unknown** from this snapshot
alone.

### Open questions

- Is the older `Galahad2LCD` device-class implementation present in a part of
  the legacy app that is missing from the current decompile (e.g., a separate
  assembly, an obfuscated class, or a build-excluded file)? Worth re-checking
  the full csproj `<Compile Include>` list and any `.dll` references that
  weren't decompiled.
- Does the older variant share the Vision wire format at runtime (i.e., does
  the shipping app actually instantiate `Galahad2VisionDevice` for PID
  `0x7391` via a code path not visible here)? The static evidence neither
  confirms nor refutes this.
- The pre-Vision SKU is not in the user's hands, so live verification (USB
  capture, handshake comparison) is out of scope for Phase 0e.

### Recommendation

Treat the older Galahad II LCD as **unsupported / unknown** for the Linux
port until either (a) decompiled source for its device class surfaces, or
(b) live hardware becomes available. Do not assume LCDV2-compatible framing.
