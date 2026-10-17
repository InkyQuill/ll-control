---
name: L-Connect Linux — Device Studio
description: Production device-first graphite studio with lilac actions and capability-specific workspaces.
colors:
  background: "#101115"
  sidebar: "#15161c"
  canvas: "#14161c"
  surface: "#191b22"
  surface-raised: "#232630"
  border: "#30323e"
  border-strong: "#787b8c"
  text: "#eff0f6"
  text-muted: "#adb2c2"
  accent: "#b9a7ff"
  accent-text: "#ded3ff"
  accent-surface: "#2c263d"
  accent-ink: "#211a35"
  primary-hover: "#cbbdff"
  success: "#a0dfbc"
  danger: "#ffb8c5"
  danger-surface: "#3a222d"
typography:
  headline:
    fontFamily: "Exo, Segoe UI, sans-serif"
    fontSize: "32px"
    fontWeight: 600
    letterSpacing: "-0.025em"
  title:
    fontFamily: "Exo, Segoe UI, sans-serif"
    fontSize: "18px"
    fontWeight: 600
    letterSpacing: "-0.025em"
  body:
    fontFamily: "Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
  label:
    fontFamily: "Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
  caption:
    fontFamily: "Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
rounded:
  control: "8px"
  field: "7px"
  surface: "14px"
  circle: "50%"
spacing:
  compact: "8px"
  small: "12px"
  medium: "16px"
  large: "24px"
  workspace: "32px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    rounded: "{rounded.control}"
    padding: "10px 14px"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
  button-secondary:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "10px 14px"
  field:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.text}"
    rounded: "{rounded.field}"
    padding: "9px"
  device-card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.surface}"
  navigation-selected:
    backgroundColor: "{colors.accent-surface}"
    textColor: "{colors.accent-text}"
    rounded: "{rounded.control}"
    padding: "10px 14px"
---

# Design System: L-Connect Linux Device Studio

## Overview

**Creative North Star: "Graphite device workbench"**

The user approved the interactive Device Studio prototype on 2026-10-16 ("Мне нравится. выглядит хорошо."). That visual direction is now migrated into the production Svelte/Tauri interface. This document records the implemented production system; the prototype remains provenance for the direction, not the token source.

Dark graphite surfaces frame pale text and restrained lilac actions. Device schematics provide visual focus without turning the application into a multicolored gallery. The selected device determines its workspaces; LCD is an optional capability rather than the application shell. Interface evidence and previews do not establish physical hardware acceptance.

**Key Characteristics:**
- Graphite tonal layers and restrained lilac actions.
- Self-hosted Exo 2 headings with system body text.
- English default and Russian localization.
- Device capability navigation and explicit application of drafts.
- Stable local editing panels with visible connection and error states.

## Colors

The palette separates the stable lilac interface accent from editable device illumination.

### Primary
- **Accent:** primary Apply buttons, focus outlines, selection and native range controls.
- **Accent text / surface:** selected device navigation and local workspace controls.
- **Accent ink / primary hover:** readable primary-button text and the lighter hover fill.

### Neutral
- **Background / sidebar:** application and navigation shell.
- **Canvas / surface / raised surface:** preview stages, inventory cards and controls.
- **Border:** structural divisions and grouped settings.
- **Strong border:** interactive field and secondary-button boundaries.
- **Text / muted text:** main copy and supporting labels, captions and status detail.

Success and danger colors communicate status alongside text. Errors pair the danger color with its dedicated dark surface.

**The Preview Color Rule.** Editable RGB colors belong to device schematics and content; ordinary navigation keeps the lilac interface accent.

## Typography

Exo 2 Semibold is self-hosted at `desktop/static/fonts/exo2-semibold.ttf`, with its OFL license at `desktop/static/fonts/OFL.txt`. CSS registers it as `Exo` at weight 600 with `font-display: swap`. Headings use Exo with Segoe UI and sans-serif fallbacks. Body controls use Segoe UI, system-ui and sans-serif; English and Cyrillic labels share the same hierarchy.

Page headings use the headline token, compact to 26px above 980px to leave room for the workbench, and become 28px below 760px. Standard section headings use the title token; inventory device names use 24px. Supporting captions use 12px with generous line height, and field labels use 13px. Sensor values use 30px tabular numerals. LCD content typography belongs to the user's composition, not the application heading scale.

## Layout

The desktop shell has a 190px device rail and a flexible main area capped at 1700px with 24px padding. Device capability links sit directly beneath the selected device. Global settings, sensors and system information form a separate navigation group.

Inventory cards pair a schematic column (`minmax(180px,35%)`) with device identity and capability actions; cards have a 240px minimum height and 24px gaps. The LCD workspace pairs a flexible canvas (`minmax(280px,1fr)`) with a bounded editing pane (`minmax(300px,360px)`). Above 980px, the columns have a 12px gap and the canvas stage stays sticky at 16px from the top. Theme identity, saved profiles and peer-panel navigation share a wrapping row above both columns. Lighting keeps its own schematic-and-controls layout.

At 980px and below, the LCD workspace stacks and its stage stops sticking, with a 560px maximum width. At 760px and below, navigation moves above content, inventory cards stack, main padding becomes 16px, and the header and device selector wrap. Forms and action rows wrap without shrinking their body type. The compact layer stack and selected-layer inspector have their own scroll areas; their height must be qualified against the supported desktop windows rather than treated as a universal token.

## Elevation & Depth

The production shell, inventory and previews use tonal layers and thin borders rather than decorative shadows. The LCD preview has an eight-pixel bezel; the prototype's LCD drop shadow is not part of the production system. Keyboard focus uses a two-pixel lilac outline with a three-pixel offset.

The shell introduces no perpetual animation or prototype panel-entry animation. Reduced-motion mode removes animations and transitions. Hardware content may itself animate; the lighting schematic is not an animation simulator.

## Shapes

Large stages and inventory cards use the surface radius; buttons use the control radius and fields the field radius. LCD preview geometry follows the selected device, alongside geometric SVG hardware drawings. Structural dividers separate layers and settings without adding a card around every field.

## Components

### Buttons and fields

Primary Apply actions have lilac fill, dark text and weight 600. Secondary buttons use a raised graphite fill and strong border. Buttons have a 38px minimum height; disabled buttons have 50% opacity and a not-allowed cursor. Secondary hover uses a lighter graphite surface and border. Inputs use inset canvas fill, strong borders and compact padding. Checkboxes and ranges use the accent color; every interactive element retains visible keyboard focus.

### Device navigation and cards

Inventory cards show device identity, connection state, a schematic and direct capability actions. Capability links are grouped under their owning selected device. The implemented registry currently grants display, cooling and lighting editors only to `galahad2-lcdv2`; unknown families receive no editors and show an explanatory message. Device absence and disconnection remain separate from unsupported capability. System-wide screens remain accessible without a device.

**The Capability Rule.** Expose editors from the implemented family registry; do not infer support from a device name or use LCD as a universal shell.

### LCD studio

**Display geometry.** GALAHAD II LCDV2 has a square 480×480 LCD. Shape and resolution come from device capability metadata shared by the preview and device glyph. Preserve square corners; apply a circular mask only to devices explicitly described as round. Do not infer the shape from the product category or add unsupported families just to demonstrate another shape.

The live canvas is the editor on entry: selection, movement and resizing do not require an editing-mode or continuous-preview checkbox. Its toolbar keeps explicit Apply to LCD and whole-screen orientation above the canvas; animation has a separate pause/resume control. Local preview changes remain distinct from confirmed physical output.

Theme name and saved profiles are common controls above three peer panels: Ready-made themes, Composition and Library. Ready-made themes create an editable local draft and then open Composition; choosing a layout does not send it to hardware. Library holds media, collections and fonts.

Composition uses a switchable Layers / Properties pane beside the canvas. Layers are compact rows ordered topmost first, with content names, type, visibility and lock controls; the background remains reachable below the rows. Selection on either the canvas or a row opens Properties for that same layer. One contextual inspector replaces repeated expanded forms. Reorder, duplicate and delete remain explicit controls, and layer names and advanced typography use disclosures rather than competing with common content fields.

Local peer panels remain mounted and are hidden when inactive, preserving their editing state while switching between them. This is local panel continuity, not a promise that every editor persists across application restarts. Preview copy explicitly distinguishes a prepared image from confirmed physical LCD output.

### Lighting studio

The schematic reflects the draft's primary color, brightness and disabled state for the chosen pump or fan zone. It explicitly warns that the fan count and LED arrangement may differ and that the illustration is a color schematic, not an effect simulation. Effect capability controls remain beside the preview. The explicit lighting Apply button uses the primary action treatment.

Lighting mounts lazily on first visit and remains mounted while navigating elsewhere. Device-, zone- and mode-scoped RGB drafts survive navigation in the current page session; inactive lighting receives a false connection signal so its connection-gated reads stop. Draft retention is not disk persistence or proof that settings were applied.

### Cooling modes

Saved device modes use a collapsed disclosure separate from named user profiles
and built-in preset buttons. Read modes on demand, choose one, then load it into
the local editor; only the existing Apply cooling action sends the policy.
Keep missing sensor bindings visible and preserve drafts on read errors. The
loading Read button retains keyboard focus; selection, read-only data and applied
state remain distinct. Local messages follow English/Russian, while mode names
and remote diagnostics remain literal. The graphite/lilac controls wrap at compact
desktop widths. Stored-mode editing uses a separate name and Save row after explicit Load. Save
changes a stored snapshot without Apply. Conflicts require an explicit Read and
a clearly named overwrite; unresolved/pending writes retain a recovery fence.
Saved status distinguishes later draft edits. Stored-mode Apply checks the loaded snapshot revision and persists its requested
identity alongside the active policy. A changed draft uses custom Apply; an
explicit Save can associate its captured snapshot with the same editor session.
The daemon summary reports requested identity separately from applied PWM.

### Status and recovery

Connection state combines a status dot with text. Missing telemetry remains unavailable rather than becoming zero. Errors, persistence status and command notices use readable text and live-region semantics where implemented. A skip link leads to main content. Disabled actions, unavailable-device copy and explicit target selection keep the effect of commands understandable.

**The Explicit Apply Rule.** Distinguish local draft, preview, saved profile and device command; editing a draft must not silently write hardware.

## Do's and Don'ts

### Do:
- **Do** use the production stylesheet and components as the token and behavior source.
- **Do** preserve English and Russian labels, visible keyboard focus and reduced-motion support.
- **Do** keep capability navigation under its device and ready-made themes, composition and library equally reachable.
- **Do** preserve local drafts and distinguish preview, storage and explicit device application.

### Don't:
- **Don't** present schematic output, fixtures or browser screenshots as physical hardware acceptance.
- **Don't** give unknown device families implemented editors or assume every device has an LCD.
- **Don't** spread user-selected RGB colors through ordinary navigation and controls.
- **Don't** reintroduce prototype-only token values, animations or demo Apply semantics into production documentation.

Fresh-bank initialization lives inside Saved device modes after an explicit Read
confirms absence. The source selector starts empty; creation is storage-only,
with8 fan/3 pump policies. Known factory labels translate by an explicit marker;
any name input becomes literal, including text equal to the default label.
Creation completion focuses the new selector unless the user moved to another
field; unresolved persistence keeps Read as the recovery action.
