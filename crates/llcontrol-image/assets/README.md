# Renderer fonts

Bundled faces are available offline to the daemon renderer:

- DejaVu Sans/Serif/Mono: unmodified host `ttf-dejavu` files;
  [notices](DejaVuSans-LICENSE.txt).
- Noto Sans TC Regular/Bold: pinned OTF2.002 with reproducibly embedded OFL;
  [provenance](NotoSansTC-provenance.json), [preparation](NotoSansTC-README.md),
  [OFL](NotoSansTC-OFL.txt). Windows typography comparison remains documented in
  [catalog](../../../docs/development/parity/font-catalog.md).
- Abril Fatface Regular, Caprasimo Regular, Cubic 11 v1.013: unmodified official
  upstream files. [Pinned URLs and SHA-256](DisplayFonts-provenance.json).
  Complete notices: [Abril](AbrilFatface-Regular.OFL.txt),
  [Caprasimo](Caprasimo-Regular.OFL.txt), [Cubic/M+](Cubic11-OFL.txt).

Desktop also ships the complete notices in `desktop/src/lib/assets/font-licenses.txt`, exposed
inline in the font importer through Vite raw import, including offline builds. Keep this aggregate synchronized with these source files.
No proprietary Windows fonts are bundled. Coverage depends on the selected
face; complex-script shaping and pixel-identical browser metrics are not claimed.
