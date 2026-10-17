# CSS pre-wrap: Unicode переносы LCD text

2026-10-11. Windows2.1.28 active CSS `Assets/ga2v/css/app.430a4e44.css`:
`.text_wrap` задаёт `white-space:pre-wrap`; word-break/overflow-wrap/word-wrap
в этом stylesheet отсутствуют. Это соответствует normal opportunities,
а не принудительному разрыву каждого длинного слова.

## Исправление

В `line_box:true` renderer использует Unicode line-break opportunities:
CJK closing punctuation не отрывается от предыдущего символа, opening
punctuation от следующего; NBSP не создаёт перенос. Слишком длинное
неразрывное слово выходит за заданную ширину. Пробелы сохраняются и при
выборе переноса в конце строки не требуют дополнительной ширины.
Shaping и paragraph bidi сохраняются через переносы.

`line_box:false` сохраняет старые word/grapheme breaks. Сохранённые темы
не мигрируются. API/schema не менялись.

## Независимая проверка

`desktop/scripts/text-wrapping-oracle.mjs` загружает тот же bundled Noto OTF
в headless Chromium, создаёт настоящий DOM block40px/line-height1 с
pre-wrap и группирует символы по Y координатам DOM Range.

| Текст | Ширина | Строки |
|---|---:|---|
| 溫度，正常 |80| 溫 / 度， / 正常 |
| 溫（正常）度 |120| 溫（正 / 常）度 |
| ABCD |40| ABCD — overflow без разрыва |
| A NBSP B space C |60| A NBSP B space / C |
| AA BB |65| AA space / BB |
| RLO AB CD PDF |65| RLO AB space / CD PDF |

Все девять browser cases совпали с renderer regression. Legacy grapheme,
CSS em sizing и bidi continuation tests также прошли. Отдельно проверены
U+2028, NEL и standalone CR: при достаточной ширине Chromium не создаёт
принудительный перенос; Mandatory из UAX не применяется как CSS hard break.
Workspace fmt/strict Clippy/docs прошли;597 tests,0 failed/ignored,92 targets.
[Evidence](evidence/2026-10-11-text-wrapping.json).
Алгоритм использует [unicode-linebreak0.1.5](https://docs.rs/unicode-linebreak/0.1.5/unicode_linebreak/).

## Открытые границы

Это не полная CSS Text реализация: locale
tailoring, другие CSS segment-break комбинации, trailing-newline box semantics,
Windows hinting/raster требуют отдельной
сверки. Fixture coverage не доказывает все комбинации Unicode. Физический LCD
и native GUI в этом checkpoint не запускались.

## Выравнивание концевых пробелов

Следующий software checkpoint 2026-10-11 различает три случая pre-wrap:
после автоматического переноса пробелы исключаются из alignment width;
перед явным newline и в конце текста сохраняются; если последние пробелы
выходят за ширину блока, только избыток не участвует в alignment.
NBSP не обрезается. Legacy mode не меняется. Glyph positions сохраняются,
поправка применяется к ширине для left/center/right и общей shadow placement.

Независимый `desktop/scripts/text-alignment-oracle.mjs` измеряет DOM Range
для27 комбинаций: три align, soft/explicit/end, несколько пробелов,
Hebrew, NBSP и whitespace-only. Rust regression сравнивает реальные pixels
`AA BB` с `AA\nBB` при width100; явный `AA \nBB` и `AA ` сохраняют отличие
для center/right. Conditional overflow проверен для латиницы и Hebrew.

Поведение согласуется с [CSS Text phase II](https://www.w3.org/TR/css-text-3/#white-space-phase-2);
browser geometry не доказывает Windows pixel raster equivalence.
fmt/strict Clippy/docs прошли;598 workspace tests,0 failed/ignored,92 targets.
[Evidence](evidence/2026-10-11-text-alignment.json). Bidi controls
после trailing spaces и более сложные whitespace sequences требуют дальнейшей сверки.

## Табуляция

В CSS режиме tab теперь создаёт отступ к следующей позиции через восемь
пробелов выбранного шрифта; если до позиции осталось меньше половины
пробела, используется следующая. Отсчёт начинается заново на каждой строке.
Tab не передаётся как отсутствующий glyph и не рисует квадрат. Wrapping,
alignment и shadow используют тот же layout; legacy mode сохранён.

`desktop/scripts/text-tabs-oracle.mjs` записывает21 DOM case (семь строк,
три align). Rust regression сравнивает шесть координат visible glyphs с
допуском0.04px к browser quantization, включая последовательные tabs,
Hebrew до/после tab и почти достигнутую позицию. Отдельно проверены
reset на новой строке, soft wrap и пустое изображение для одиночного tab.
fmt/strict Clippy/docs прошли;599 workspace tests,0 failed/ignored,92 targets.
[Evidence](evidence/2026-10-11-text-tabs.json).
Сложные bidi override/control sequences и font fallback остаются отдельными
границами; это не доказательство полной Windows typography.

## Мягкий дефис

CSS `U+00AD` теперь остаётся невидимым без переноса и перед явным newline/
концом текста. На выбранной discretionary boundary shaping заменяет его
видимым `U+2010` (same-font ASCII fallback при отсутствии glyph). Width,
alignment и raster получают один shaped result. При выборе break учитывается
ширина дефиса; rejected opportunity пересматривается после более раннего
переноса. Помещающееся целиком слово не переносится из-за ширины невидимого
в этом случае дефиса. Legacy mode не меняется.

`desktop/scripts/text-hyphen-oracle.mjs` проверяет10 DOM cases на bundled
DejaVu40px. Для символа сразу после SHY берётся последний client rect:
bounding rectangle может включать дефис на предыдущей строке и ошибочно
приписать символ ей. Rust regression сравнивает шесть line distributions
и pixels для трёх align: discretionary break, unbroken word, explicit LF и
text end. Перенос с видимым дефисом совпадает с явным `ab‐\ncd`.

[Evidence](evidence/2026-10-11-text-hyphen.json). Это manual soft hyphen,
не dictionary-based automatic hyphenation. Locale-specific replacements,
сложные bidi controls и межшрифтовый fallback не квалифицированы этим тестом.
Native GUI и физический LCD на этом checkpoint не запускались.
