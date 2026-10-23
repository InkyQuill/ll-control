# Theme variants from Vectorize

## Clean geometry revision

2026-10-23, following user review: [clean dark-panel SVG](ll-control-clean-on-dark.svg) and [clean light-panel SVG](ll-control-clean-on-light.svg). These manually cleaned SVGs reconstruct the traced dial using exact concentric circles, a circular cyan quarter arc and a rounded vertical pointer. No redesign into a monochrome/open-ring symbol. Original Vectorize output and previous variants remain preserved below.

Removed the irregular lower highlight and tracing noise. Pointer width is 6 units in a 64-unit viewBox: 1.5px at 16px and 2.25px at 24px. The source cyan #00fbfd and both neutral theme palettes are preserved. Both clean files have identical geometry and differ only in neutral colors.

[Before/after preview](clean-comparison.html) and [1x Chromium screenshot](clean-comparison.png) show 16, 24 and 96px on both backgrounds. Visual inspection confirms a more distinct pointer at 16px, while the full circular dial and cyan arc remain recognizable. Transparent PNG exports: clean-dark-16.png, clean-dark-24.png, clean-light-16.png, clean-light-24.png. Verification metadata: clean-verification.json. Native tray integration and automatic KDE theme switching were subsequently verified; see [the branding integration record](../../../../docs/development/design/ll-control-branding.md).

## Original trace

2026-10-23. User rejected the symbolic redesign and requested tracing original dial 07 through Vectorize, retaining the cyan accent and changing neutral base colors only.

Source: ../icon-concept-07-control-dial.png. Vectorize revision: 27b3ec341ea94fd39a9986d3a8897d22. Settings: colors=6, smoothing=1, transparent=true, trim=true. Returned palette has three colors.

- vectorize-original.svg: untouched Vectorize output.
- ll-control-on-light.svg: original charcoal palette, for light panels.
- ll-control-on-dark.svg: neutral base #212326 -> #465463 and neutral edges #3c3d41 -> #cdd8e4, for dark panels.
- Accent remains Vectorize's #00fbfd in both files. It is the quantized source cyan, not a forced new brand color.
- comparison.html / comparison.png: actual 16px and 24px renders plus a 96px shape reference, Chromium at deviceScaleFactor=1.

Only neutral fill colors change between variants; all path geometry, clipping, framing and cyan paths remain identical. No currentColor conversion. An initial pale filled body reduced cyan contrast, so the dark-panel variant uses a medium slate body with a pale neutral edge instead.

Visual inspection: the original full circular dial identity is retained. Both panel backgrounds have a visible silhouette at 16 and 24px. At 16px the cyan arc remains visible but the thin vertical indicator loses definition. Vectorization retains a slightly irregular lower highlight from the source; these files are traced candidates, not geometrically cleaned production masters. These original traced candidates were superseded by the clean geometry revision above; native integration uses the clean variants.
