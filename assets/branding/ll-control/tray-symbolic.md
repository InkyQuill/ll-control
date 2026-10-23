# Tray symbolic candidate

2026-10-23. User requested a stronger-contrast tray variant and currentColor checks at 16 and 24 px on light and dark backgrounds.

[SVG](ll-control-tray-symbolic.svg) uses only currentColor strokes and a transparent background. A 3-unit stroke in a 24-unit viewBox becomes 2 px at 16 px. The open dial and diagonal pointer remove the filled charcoal body and separate the indicator from the perimeter. This intentionally simplifies the full-color launcher design.

[Preview](tray-check/preview.html), [1x screenshot](tray-check/comparison.png), [verification](tray-check/verification.json).

Chromium verification: the same inline SVG inherits #eff2f6 on #171b22 and #252a33 on #f1f3f5. Changing parent color to #00d5ec updates the stroke without replacing the SVG. Visual inspection confirms visible ring and pointer at both sizes; the 16px version retains less internal detail. The open silhouette can also read as a C, so final visual selection remains pending.

Integration boundary: desktop/src-tauri/src/desktop_runtime.rs currently supplies a PNG through tauri::image::Image::from_bytes and TrayIconBuilder.icon. This does not provide CSS currentColor inheritance. Inline SVG in a web document is verified; an external img also does not inherit the embedding document's color. Native KDE/GNOME theme recoloring requires a suitable symbolic/theme-name or theme-aware raster integration and separate desktop testing. No tray runtime code or installed icon was changed.
