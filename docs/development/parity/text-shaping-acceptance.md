# OpenType shaping и bidi LCD text

2026-10-11, software checkpoint для CSS режима `line_box:true`.

Renderer использует rustybuzz для OpenType GSUB/GPOS и unicode-bidi для
визуальных runs. Измерение переноса, alignment и raster placement получают
одни и те же shaped glyphs. Bidi-контекст вычисляется для всего абзаца и
сохраняется при мягком переносе. Shadow использует тот же layout.

Новые темы и presets включают CSS режим. Сохранённые темы без `line_box`
или с `false` сохраняют старое посимвольное поведение; миграции нет.

## Проверки

- Лигатура ffi, арабский lam-alef, порядок Hebrew с парными скобками,
  смешанные LTR/RTL строки, одинаковые glyphs и pixels для é/e+combining mark.
- Измерение переноса использует shaping; RLO сохраняется на следующей строке.
- Независимый headless Chromium с тем же DejaVuSans40px: fractional advances
  совпали с `geometricPrecision` с допуском0.001px.
- Обычный canvas `textRendering:auto` округляет advances иначе. Это не
  доказательство равенства default browser metrics или Windows pixels.
- fmt, strict Clippy, docs прошли;596 Rust tests,0 failed/ignored,92 targets.
- frontend check:0 errors/warnings; build прошёл;26 themes прошли Rust schema.
-9 browser tests: presets с CSS mode, expanded accessibility и layer keyboard.

Oracle: `desktop/scripts/text-shaping-oracle.mjs`.
API references: [rustybuzz](https://docs.rs/rustybuzz/0.20.1/rustybuzz/),
[unicode-bidi](https://docs.rs/unicode-bidi/0.3.18/unicode_bidi/struct.BidiInfo.html).
[Evidence](evidence/2026-10-11-text-shaping.json) содержит hashes и browser oracle.

## Остаток

[Обычные Unicode переносы](text-wrapping-acceptance.md), CJK punctuation и
неразрывные слова проверены. Полная Windows typography остаётся открытой:
locale-specific/browser line-breaking,
locale/script extensions и Windows-specific fallback, trailing newline semantics,
letter-spacing, hinting/antialiasing и визуальное сравнение Windows.
Worst-case realtime стоимость повторного shaping при подборе переносов
не измерена. Native GUI и физический LCD на этом checkpoint не запускались.

## Fallback отсутствующих glyphs — 2026-10-11

CSS mode (`line_box:true`) сохраняет выбранный bundled/managed шрифт для
поддерживаемых clusters. Если cluster не может быть shaped без `.notdef`,
renderer пробует bundled DejaVu Sans, затем Noto Sans TC Regular. Если ни
один не поддерживает cluster целиком, остаётся missing glyph основного шрифта.
Legacy mode не меняется. Отсутствующий/повреждённый managed font file остаётся
ошибкой подготовки темы; fallback glyphs не скрывает потерю ресурса.

Выбор делается для целого grapheme cluster. Соседние clusters одного шрифта
и script объединяются перед shaping; bidi order и paragraph context остаются.
Для canonical decomposition используется shaping probe: `e+acute` сохраняет
Abril, если он способен отобразить готовый `é`. Measurement/wrapping/alignment
и shadow/raster используют тот же glyph font и его em scale. Основной шрифт
по-прежнему задаёт line box и baseline.

Независимый oracle `desktop/scripts/text-fallback-oracle.mjs` загружает точные
bundled bytes и явный stack Abril→DejaVu→Noto в headless Chromium. Проверены
одиннадцать строк: кириллица, CJK, смешанные Latin/CJK и Latin/Arabic, Hebrew,
precomposed/decomposed accent, Cyrillic combining mark и Arabic SHY/word joiner.
Невидимый word joiner наследует соседний font run; SHY выбирается отдельно,
как в browser stack. Дополнительная проверка с одним DejaVu отличает
само shaping от переключения шрифта. Fractional widths
совпадают с geometricPrecision в пределах0.001px. Regression проверяет
неразрывность combining cluster, CJK wrapping и точные outline pixels Noto
при fallback, а также отсутствие изменения legacy mode.

Это детерминированный Linux fallback для доступных bundled glyphs, а не
воспроизведение Windows system-font resolver. Полный font inventory Windows,
locale-sensitive CJK selection, emoji/color fonts, complex-script contextual
fallback и динамический рост line box требуют отдельной сверки. Native UI проверен последующим checkpoint ниже; физический LCD
для этого изменения ещё не запускался.

[Source hashes, browser oracle и verification](evidence/2026-10-11-text-fallback.json):
42 image tests,601 workspace tests,fmt/strict Clippy/docs прошли; review без
оставшихся замечаний.

Native follow-up: [WebKitGTK→IPC→MockBackend](evidence/2026-10-11-native-text-fallback.json)
прошёл. Два неподдерживаемых основным Sans CJK символа дают разные previews;
Abril `é`/`e+acute` дают одинаковые JPEG. Mixed Cyrillic/CJK + tabs/SHY тема
прошла GUI Apply, сохранение и restart. Это transport/integration acceptance,
не визуальное равенство Windows raster или аппаратный LCD.
