---
version: 1
slug: "desktop-src-routes-page-svelte"
primary_target: "desktop/src/routes/+page.svelte"
related_targets: ["desktop/src/lib/studio.css", "desktop/src/lib/DeviceOverview.svelte", "desktop/src/lib/Lighting.svelte", "desktop/src/lib/Cooling.svelte", "desktop/src/lib/CoolingModes.svelte"]
---

# Production device studio

Operate mode. Migration of the user-approved prototype into the existing Svelte/Tauri application. Preserve actual daemon capabilities, explicit Apply, profile/media workflows, device identity, reconnect guards and EN/RU. The supported family remains GALAHAD II LCDV2; unknown families must not inherit its editors.

## Direction contract

THESIS: Device-first gaming studio; LCD is one optional device capability.
OWN-WORLD: Graphite surfaces, Exo headings, pale text and restrained lilac actions. Editable RGB belongs to device schematics and LCD content.
STORY: Discover actual devices, select a supported capability, edit a local draft, apply explicitly and inspect the result.
FIRST VIEWPORT: Device navigation at left, an inventory overview with capability links in the main area. Selected-device studios preserve production controls. LCD has common name/profiles and local Composition/Library panels; lighting pairs a schematic with controls. Signature interaction: draft lighting changes update the schematic before explicit Apply. Motion is restrained and respects reduced motion.
FORM: Approved Device Studio direction from prototype, seed 54f8ad9a, adapted to complete production tasks rather than demo controls. Code-led implementation.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

DISPLAY GEOMETRY: GALAHAD II LCDV2 is square, 480×480. Preview and glyph share device metadata; circular clipping applies only to explicitly round LCDs. Geometry is independent of the studio shell and does not imply support for additional hardware families.

COOLING MODE REFINEMENT: Separate saved-device-mode disclosure, read → local draft → explicit Apply. Preserve graph/draft on read failure, raw user names and EN/RU. Browser review at1280/1024 showed no overflow; native RU uses isolated MockBackend only. Bank CRUD/active identity and physical qualification remain open.

Cooling: stored-mode Apply persists requested ID/snapshot with CAS; custom drafts
remain separate. Late Save never relabels a different editor session. Identity in
the daemon summary does not assert measured physical RPM.

Saved device modes expose an explicit fresh-bank initializer after Read confirms
absence. Creating modes never Applies. Factory labels have explicit localization
intent; literal names never translate by value. Completion preserves focus in a
policy the user began editing while initialization was pending.
