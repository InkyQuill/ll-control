# LL-Control branding decision

Date: 2026-10-18

## Approved name

The user selected **LL-Control** to replace **L-Connect Linux**. Preserve the exact capitalization and hyphen. The goal is a concise, memorable hardware utility name with a contextual connection to supported LIAN LI devices.

Suggested description: Independent Linux controller for supported LIAN LI hardware.

This is an independent project, not an official LIAN LI application. The preliminary public-web search found no obvious exact-name software conflict; direct trademark-register clearance and similar-mark searches remain incomplete. Do not describe the name as legally cleared.

## Migration status

The visible product name is LL-Control: window/document titles, sidebar, tray menu, file dialogs, autostart label and packaged launcher. The approved charcoal/cyan dial is used for the application icon, sidebar and favicon; the tray uses its two panel variants. Binary/package identifiers, service filenames and configuration paths remain compatible with existing installations (`lconnect-gui`, `lconnectd`, `net.inkyquill.lconnect`).

## Icon exploration

Historical exploration, superseded by the approved dial below. Generate original artwork rather than adapting LIAN LI or L-Connect logos. Prefer a bold double-L monogram, a subtle airflow/rotor association and restrained cyan light on charcoal. The mark should remain understandable at small launcher sizes and admit a simplified monochrome tray version. Avoid tiny fan blades, text labels, rainbow gradients and ornate gaming crests.

Saved concept: [icon-concept-01.png](../../../assets/branding/ll-control/icon-concept-01.png). The generated mark uses one upright white L and one mirrored cyan L on charcoal. Small-size rendering and monochrome tray acceptance have not yet been validated; the installed icon is unchanged.

Generation method: built-in image_gen, logo-brand concept exploration. Production vector reconstruction and launcher/tray size checks follow selection.

### Concept 01 prompt

Create one premium Linux desktop app icon for LL-Control, an independent cooling, RGB lighting and LCD hardware controller. Original bold geometric monogram of two clearly legible capital L shapes, arranged with a subtle quarter-turn airflow/rotor rhythm, but not a swastika, not a four-blade pinwheel. Two main solid shapes only, generous negative space, compact balanced silhouette. Cool ice-white first L and restrained electric cyan second L on a deep charcoal rounded-square tile. Flat vector-like geometry, precision chamfers and gently rounded outer tile corners; clean high contrast, no fine lines. The double-L must be the primary reading. No wordmark, no letters beyond the two L shapes, no tiny controls, no gear, no circuitry, no fan illustration, no rainbow, no 3D perspective, no mockup, no outside shadow. Single centered icon filling about 80 percent of the image, transparent canvas outside the tile. Designed to be recognizable at 32 pixels, modern understated gaming utility, independent original design with no existing company logo imitation.

## Approved tray icon and integration — 2026-10-23

The approved mark is the cleaned circular dial with a cyan quarter arc and a
thick vertical pointer. The double-L and monochrome concepts above are historical
exploration, superseded by this decision. Both panel variants preserve cyan
`#00fbfd`; only the neutral base changes.

Sources: `assets/branding/ll-control/vector-dial/ll-control-clean-on-{dark,light}.svg`.
Regenerate the embedded 64px transparent PNGs with `scripts/generate-tray-icons.sh`
(requires `rsvg-convert`). The SVG pointer is 6 units wide in a 64-unit viewBox,
equivalent to 1.5px at 16px. Launcher artwork is separate from these tray assets.

The tray checks appearance on its existing lifecycle loop (approximately every
two seconds). On KDE it reads the active Plasma desktop theme's Window palette,
using XDG user/system configuration and data search paths. Adaptive themes fall
back to `kdeglobals`; this respects panel themes that differ from window colors.
Other environments, or unavailable palettes, fall back to Tauri's window theme.
The PNG changes only when the detected appearance changes. Cyan is never replaced
with currentColor. Arbitrary panel-background overrides are not sampled.

### Native KDE acceptance

Verified in the live Plasma Wayland session: Thoth-Gently-Dark → breeze-light →
Thoth-Gently-Dark, without restarting the application. StatusNotifierItem retained
its bus identity while its served PNG changed to the corresponding embedded file.
Panel screenshots were inspected for both appearances. The original user theme
was restored. Raw local evidence (desktop screenshots, served PNGs, hashes and
runtime log): `target/tray-kde-20261023/acceptance.json` and adjacent files.

This machine required `WEBKIT_DISABLE_DMABUF_RENDERER=1` for the test GUI's WebKit
renderer. The test launched the locally built GUI; it did not install a package.

### Application branding — 2026-10-23

The launcher uses the approved charcoal dial from `ll-control-clean-on-light.svg`.
`scripts/generate-tray-icons.sh` also exports the 256px app PNG and the frontend SVG.
Tauri embeds `icons/icon.png`; distribution installs matching 256px and scalable
hicolor assets. Documentation and local project skill descriptions use LL-Control.
References to Windows L-Connect and persistent technical identifiers are retained.

Native follow-up: verified the installed `/usr/bin/lconnect-gui` in KDE, including
window title, sidebar and window-decoration icon. The desktop entry now references
`Icon=ll-control` to avoid reuse of the previous cached icon identity. GUI binary,
desktop entry and icon files were locally updated; this was not a full package
release or daemon upgrade. Original installed files are backed up under
`target/tray-kde-20261023/installed-backup/`. Evidence screenshot:
`target/tray-kde-20261023/installed-branding-window-final2.png`.
The running verification instance uses `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

Validation: Svelte check (zero errors/warnings), strict English resources
(1179/1179), production frontend build, GUI library tests (131 passed, 4 ignored),
Clippy with warnings denied, formatting, GUI build and packaging-script syntax.
