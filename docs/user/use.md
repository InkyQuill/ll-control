# Use LL-Control

Choose your device, then open **Pump screen**, **Cooling** or **Lighting**.
Unavailable sensor values appear as a dash; they are not zero.

## Put content on the LCD

1. Choose a ready-made layout, or add an image/video from **Library**.
2. Edit text, clocks, sensor bindings and appearance in **Composition**.
3. Select a layer on the live canvas or in **Layers**, then drag, resize or edit its
   properties. **Layers** shows the compact stack; **Properties** shows the selected
   layer’s controls. Use the eye to hide a layer, the lock to protect it, and
   **Undo edit** to restore a draft.
4. Choose **Apply to LCD** when the composition is ready.

Canvas arrows move a layer; Shift + arrows resize it. Escape cancels a drag.
Use the **?** beside the canvas for shortcuts. Hover an icon to see its action. Animation plays automatically;
**Pause animation** freezes it while you edit.

**Screen orientation** rotates the whole canvas. Brightness is under **Display
settings** (sliders icon). These settings reach the device only when you choose **Apply to LCD**.
Brightness does not dim the editing canvas.

Advanced controls open in floating panels; press Escape or click outside to close them.
The font selector includes installed system fonts. Selecting one copies it into the
theme library, so the theme keeps its font. TTF/OTF are supported; TTC collections are not.

**Save** stores a profile. **Load** puts a profile into the editor.
**Apply** changes the device. These are separate actions.
**Return LocalUI** stops the software display and returns the device's own screen.

Imported media is managed by the application. Deleting an asset can be refused
while a saved profile or active composition still uses it. Gallery re-editing
keeps existing references safe.

## Cooling and lighting

Select a preset or edit a curve, check its temperature source, then apply it.
A fan connected to the motherboard is not controlled through the LCDV2 fan channel.
Lighting offers manual modes, sensor-driven changes and thermal alerts.

Closing the window does not stop the background daemon or its cooling/LCD work.
GUI startup and tray behavior are in application settings. Language and
Celsius/Fahrenheit are also configurable there.

Screenshot/recording, Windows ZIP import, procedural effects and Mini/FPS are
experimental. Firmware flashing is unavailable. If an operation's result is
uncertain, read its current state before retrying; see [help](troubleshooting.md).
