# Bundled Noto Sans TC

Regular and Bold OTF version2.002, each with20,748 Unicode cmap entries,
from official noto-cjk commit `74981c47522c5b7262fcc344bf01c18e82af5d78`,
`Sans/SubsetOTF/TC/NotoSansTC-{Regular,Bold}.otf`.
Source URLs, input/output hashes and fonttools version are pinned in
`NotoSansTC-provenance.json`. Files are not copied from Windows and are not
byte-identical to the Windows assets.

The full upstream SIL Open Font License and notices are preserved in
`NotoSansTC-OFL.txt` and embedded in name ID13 of both files. Only license
metadata is changed; glyphs and metrics are not subsetted or converted.
Keep these notices when redistributing the application.

Offline regeneration requires Python with fonttools==4.66.1 and a directory
containing both original OTF files downloaded from the manifest URLs:

```sh
python3 tools/prepare-noto-tc.py /path/to/original-otfs --output /tmp/noto-rebuilt
```

Omit `--output` to write the bundled assets. The helper validates both inputs,
license hash and fonttools version before writing, preserves timestamps and
checks each output against the recorded hash. It does not rewrite provenance,
access the network, install fonts or query fontconfig.

Coverage includes U+2252, U+25C8 and U+30EDE, missing from the previously bundled
Google variable-font instances. Renderer regressions require real glyph outlines
for these characters in both weights. Coverage is not all of Unicode; select
Noto Sans TC explicitly for Traditional Chinese. No automatic fallback or
complex-script shaping is introduced. Equal glyph data does not establish
pixel-identical browser layout or rasterization.
