# CSS font size: исправление масштаба LCD text

2026-10-11, Windows2.1.28.0 reference. Active `Assets/ga2v/js/app.8fc94db7.js`
передаёт `FontSize` в CSS px; default43px. SHA256:
`f3691c15dbe62d1248abee444dc94c82a34c46e45697bda4ccad77825c6e878a`.
[CSS font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop) задаёт масштаб
em square. `ab_glyph0.2.32` напротив трактует PxScale как ascent-minus-descent;
его локальный `Font::pt_to_px_scale` документирует преобразование через
`height_unscaled / units_per_em`.

Раньше CSS line-box режим задавал правильную высоту блока, но использовал
metric-height размер глифа. В результате Noto40px был меньше CSS40px, а два
полноширинных символа ошибочно помещались в79px. Теперь при `line_box:true`
один общий scale применяется к измерению переноса, horizontal alignment,
baseline metrics и контурам. Half-leading учитывает уже пересчитанные metrics;
допускается отрицательный leading при маленьком line-height.

## Совместимость

`line_box:false` или отсутствие поля сохраняет прежний renderer scale.
Существующие темы с `line_box:true` меняют размер глифов и могут иначе
переноситься: это исправление их CSS режима. JSON schema/enum не меняются;
настройка общая для text, clocks и sensor parts, включая managed fonts.
Автоматическая миграция размеров или изменение сохранённых тем не выполняется.

## Проверки

- Red regression: bundled Noto Sans TC Regular,40px, `溫度`, width79 — прежний
  renderer выдавал одну строку вместо двух.
- Green: width79 даёт две строки,80 — одну; legacy mode остаётся прежним.
- Независимый headless Chromium156.0.8078.4: FontFace с тем же локальным OTF,
  canvas `40px ParityNoto`, `measureText('溫度').width == 80`.
- CSS top/center/bottom anchors совпадают для блока из трёх строк, включая
  пустую, при line-height80 и20; legacy block-height проверен отдельно.
- [Проверки и hashes](evidence/2026-10-11-css-em-sizing.json).

## Открытый остаток text parity

CSS em sizing и [OpenType shaping/bidi](text-shaping-acceptance.md) реализованы
и проверены программно. [Обычные Unicode/CJK переносы](text-wrapping-acceptance.md)
также проверены. Остаются locale-specific breaking, whitespace layout/fallback,
letter-spacing, hinting/raster и сравнение с Windows. Полное pixel-identical
соответствие не заявляется. Физический LCD не запускался.
