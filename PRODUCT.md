# L-Connect Linux

<!-- impeccable:product-schema 1 -->

## Platform

web

The interface is a Svelte 5 / SvelteKit application inside a Tauri desktop shell on Linux. This platform value describes the UI implementation, not a browser-hosted product. Preserve desktop window, keyboard, file chooser and session expectations. Mini has separate Plasma and GNOME integrations.

## Users

Linux gamers who own different types of Lian Li hardware and frequently change available lighting, cooling and profiles, plus LCD content on devices with screens. Design for an average user rather than a technician. Choosing and adapting ready-made content and composing original layouts are equally important.

## Product Purpose

Let users customize supported Lian Li hardware, control cooling and inspect system/device telemetry from Linux. Make repeated changes understandable, efficient and recoverable.

## Positioning

Linux-native hardware control with LCD media/composition, cooling, lighting and persistent profiles. Windows L-Connect 3 is a capability reference; its screen organization is not a design constraint. The user explicitly permits rethinking UX while preserving capabilities.

## Operating Context

The desktop talks to a local daemon. The current hardware target is Galahad II LCDV2 (`0416:7395`); Windows L-Connect 3 2.1.28.0 is the parity baseline. Other hardware families are planned product scope, even though their implementation and hardware acceptance are future work. The product architecture must support devices without LCDs and multiple device families. Media editing applies only to screen-equipped devices; it must not define the application shell.

## Capabilities and Constraints

- LCD shape and resolution belong to the device capability metadata. GALAHAD II LCDV2 has a square 480×480 screen; other devices may have round screens. Both the device schematic and content preview must follow this metadata without clipping square corners.
- Build navigation around devices and their capabilities. Never assume every device has an LCD; absent capabilities should not appear as disabled permanent sections. A disconnected known device is a separate state from an unsupported capability.
- The user favors a device studio specifically inside management of a selected device, not an LCD-dominated application home.
- Preserve LCD media import/editing, composition, galleries, profiles, cooling, RGB, sensors and settings workflows during redesign.
- Distinguish draft/preview, saved configuration and commands applied to a device. A preview must not silently become a hardware write.
- Preserve device identity, disconnected states, missing/stale telemetry, recoverable failures and protection of unsaved work. Never depict unavailable telemetry as zero.
- English is the default interface language; Russian is required. Layout must accommodate both.
- Hardware acceptance, real capture backends and firmware update/recovery remain partly incomplete. Design must not imply that planned capabilities are available or verified.
- Mini is implemented through Plasma and GNOME integration rather than a separate application window.

## Brand Commitments

Current product name: L-Connect Linux. The user prefers a modern gaming identity, probably dark and potentially neon: a sense of adding light to darkness, appropriate to RGB and the pump LCD. Keep controls convenient and understandable to an average user.

User-named positive references: ROG / Armoury Crate / ASUS AI Suite, original L-Connect, JBL Quantum (with less animation), Logitech, SteelSeries and Opera GX. MSI utilities are a negative reference. These are orientation references, not permission to copy their assets or every interaction.

Avoid an immediately overwhelming, multicolored gallery. Motion is welcome in moderation. Exact palette, typography, animation behavior and desktop theme integration remain open.

## Evidence on Hand

- `README.md`: scope and current support status.
- `roadmap.md`, `docs/development/parity/`: capability baseline, contracts and remaining acceptance.
- `desktop/src/`: existing interface and behavior.
- `desktop/README.md`: desktop operation and verification workflows.
- https://github.com/joeytroy/ll-connect3: user-provided reference; its README targets SL-Infinity Hub and explicitly describes mirroring Windows L-Connect 3. This does not establish what hardware its authors own.

## Product Principles

- Prioritize frequent customization of each device through its supported capabilities.
- Preserve capability and behavioral contracts while improving navigation and task flow.
- Keep the selected hardware target and the effect of each action understandable.
- Protect drafts and make failures recoverable.
- Present only support and success that available evidence establishes.

## Open Decisions

Design the multi-device shell and a device studio before refining the LCD workspace. The user favors the studio within a selected device. Exact home composition, global versus device profile scope, visual tokens, theme behavior and accessibility acceptance remain open. Within LCD-capable devices, resolve browsing and composition without privileging either workflow.


## Release decisions — 2026-10-17

The first release targets a usable Galahad II LCDV2 application on KDE Plasma
Wayland: core LCD editing/playback, cooling, RGB, sensors and persistence.
Basic direct canvas selection, movement, resizing and layer ordering are required;
form controls plus a live preview do not complete the visual editor.
Firmware flashing, additional LCD effects and optional integrations follow later.

Windows imports must preserve their intended layout and behavior faithfully.
Linux-authored themes need capability equivalence, not identical Windows layout.
Windows-compatible export is also required as a later milestone, with explicit
handling of Linux-only features that Windows cannot represent.

See [release audit](docs/development/parity/release-audit-2026-10-17.md) and
[milestones](docs/development/parity/release-milestones.md). These decisions do not authorize
hardware testing or change recorded unverified support into a release claim.
