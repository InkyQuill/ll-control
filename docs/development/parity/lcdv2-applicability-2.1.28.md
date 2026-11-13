# LCDV2 applicability audit: Windows 2.1.28.0

This is a source audit of the versioned 2026-09-08 decompilation described in
[reference provenance](../2026-09-08-reference-2.1.28-and-ll-connect3.md), not a
new hardware test. Paths below are relative to that decompilation root. The
scope is Galahad II LCDV2 (Galahad2Vision), not every product in L-Connect.

## Startup and offline uploads

`L-Connect.Core/LConnectCore/Products/Galahad2Vision/Commands.cs:20-27`
names `UploadJpeg=15`, `UploadAvi=18`, `UploadH264=19`,
`UploadH264Info=21` and `UploadStartupAviMovie=22`. A search for these
identifiers across the decompiled C# finds only the enum entries for this product
and the corresponding HydroShift enum. The Galahad2Vision device interface
and implementation expose H.264 streaming but no named upload method for these
commands. This is evidence of no **identified named caller in the examined
source**, not proof that numeric, native, dynamically loaded or firmware paths
cannot exist. The product service starts screen streaming in
`L-Connect-Service/LConnectService/Products/Galahad2Vision/Galahad2VisionController.cs:1278-1281`.

Decision: exclude selectable offline-content upload from the *currently
supported Linux scope*, pending a demonstrated Windows caller or physical proof.
Do not claim that firmware cannot support it, that a codec is a feature, or that
LocalUI is equivalent. Reopen on a versioned caller/capture and verify selected
content after cold boot without the daemon; USB success alone is insufficient.

## Global motherboard RPM sync

`L-Connect 3/LConnect3cs/Views/CustomControls/MotherboardRPMSyncToggleSwitch.cs:96-152`
subscribes a click, sends `SetMotherboardRPMSync`, and reads service state back.
`L-Connect 3/LConnect3cs/Products/TLFan/TLFanProfileMainPage.cs:22,150`
contains an instance; `MainWindow.cs:761` synchronizes its shared state.
`L-Connect-Service/LConnectService/MainService.cs:2008-2028` persists the
service-wide boolean. The Galahad2Vision service's CPU-reference pump timer
selects Motherboard versus MCU from that boolean (`Galahad2VisionController.cs:363-381`);
the product page disables pump profile selection under sync
(`L-Connect 3/LConnect3cs/Products/Galahad2Vision/Galahad2VisionMainPage.cs:142`),
and the product profile allows pump sync but disallows fan sync
(`Galahad2VisionProfile.cs:154,743`). These are source-derived observations,
not physical verification of all mode transitions.

Linux explicitly adapts this as a **per-device** pump source/policy, exposed in
the cooling editor, rather than a global switch affecting other product
families. The existing source/persistence path is integration-tested; global
cross-device propagation and physical transition parity are not claimed. The
Windows caller is resolved; complete global UI equivalence remains outside this
single-device scope.

## Firmware before 1.5

The service defaults to `SendH264Frame`, then switches to
`SendH264Frame512` and starts availability polling only when the initially
read version is at least 1.5 (`Galahad2VisionController.cs:1253-1277`). This
establishes the Windows caller and gate, not visible Linux playback. The
recorded physical result is FW 1.6 / C512; B transport on FW <1.5 has no
physical playback qualification. Linux support claims therefore cover the
observed FW 1.6 configuration only; legacy transport remains experimental and
unqualified, including boot, reconnect and sustained playback. Qualify it with
an identified FW <1.5 device and repeatable playback/recovery evidence before
expanding that claim.
