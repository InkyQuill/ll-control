# LCDV2 font catalog: Windows 2.1.28.0

Сверено 2026-10-10 с одиннадцатью файлами `Assets/ga2v/fonts` из установки
пользователя. Имена enum не всегда совпадают с реальным начертанием.
[Evidence: hashes, versions, cmap и comparison](evidence/2026-10-10-font-catalog-audit.json).
Это binary/source audit, не сравнение итоговых изображений на LCD.

| Windows choice | Реальный файл: family/style/version | Решение для Linux |
|---|---|---|
| AbrilFatface | Abril Fatface Regular 1.001 | Bundled официальный OFL upstream; renderer/IPC/GUI подключены |
| AgencyGothicCT_Bold | Agency Gothic CT **Condensed** 2.000 | User import соответствующего лицензированного файла; право включения в поставку не установлено |
| BakerieSmooth_WideMediumRegular | Bakerie Smooth Wide Medium 1.000 | User import; metadata требует vendor license |
| Caprasimo_Regular | Caprasimo Regular 1.001 | Bundled официальный OFL upstream; renderer/IPC/GUI подключены |
| GoodTimesRg_Italic | Good Times Italic 4.102 | User import; отсутствие license name13 не является разрешением redistribution |
| HyperspaceRace_BoldItalic | Hyperspace Race Bold Italic 1.000 | User import; право включения не установлено |
| LTCFournierLeJeune_Regular | LTC Fournier Le Jeune Regular 3.000 | User import; metadata ссылается на P22 EULA |
| ManoloMono_Regular | Manolo Mono Regular 1.001 | User import; author Manuel Sanchez, redistribution не установлено |
| NotoSansTC_Regular | Noto Sans TC Regular 2.002, OTF | Bundled OTF2.002, cmap20748; glyph outlines/horizontal metrics совпадают с Windows |
| NotoSansTC_Bold | Noto Sans TC Bold 2.002, OTF | Bundled OTF2.002; то же покрытие и совпадение glyph data |
| Cubic11 | Cubic 11 Regular 1.013 | Bundled официальный **побайтово идентичный** файл1.013 |

## Официальные источники включённых шрифтов

- Abril Fatface: [font](https://github.com/google/fonts/blob/3be1884c48c3e45b52ecc725676a08f87776373e/ofl/abrilfatface/AbrilFatface-Regular.ttf),
  [OFL](https://github.com/google/fonts/blob/3be1884c48c3e45b52ecc725676a08f87776373e/ofl/abrilfatface/OFL.txt).
- Caprasimo: [font](https://github.com/google/fonts/blob/3be1884c48c3e45b52ecc725676a08f87776373e/ofl/caprasimo/Caprasimo-Regular.ttf),
  [OFL](https://github.com/google/fonts/blob/3be1884c48c3e45b52ecc725676a08f87776373e/ofl/caprasimo/OFL.txt).
- Cubic 11 v1.013: [font](https://github.com/ACh-K/Cubic-11/blob/6a74b71d78369961a73a187e4c73ff81d3f51a96/fonts/ttf/Cubic_11_1.013_R.ttf),
  [полный license/attribution](https://github.com/ACh-K/Cubic-11/blob/6a74b71d78369961a73a187e4c73ff81d3f51a96/OFL.txt).
  Сохранять весь файл, включая Cubic/M+ notices до текста OFL.

У Abril/Caprasimo совпадают version strings, Unicode cmap и horizontal metrics
всех Unicode-mapped glyphs. Бинарные файлы и несколько font tables различаются;
это **не** доказательство идентичного raster/kerning/shaping. Cubic совпадает
побайтово, но разные renderer всё равно требуют image comparison.

Bundled Noto OTF2.002 содержат те же20 748 Unicode codepoints, что Windows,
включая **U+2252 (≒), U+25C8 (◈), U+30EDE**, отсутствовавшие в прежних TTF.
Сравнение каждого Unicode-mapped glyph в обоих начертаниях не выявило различий
в horizontal metrics и командах контуров. Бинарные hashes различаются;
это не доказательство одинаковых kerning/shaping, line layout или raster.
Полный OFL встроен в name ID13 без изменения glyph data; offline helper
воспроизводит записанные output hashes. Источник — официальный
[noto-cjk2.002](https://github.com/notofonts/noto-cjk/tree/74981c47522c5b7262fcc344bf01c18e82af5d78/Sans/SubsetOTF/TC).

## Пользовательские шрифты

Готовый путь: Display → «Пользовательские шрифты» → выбрать TTF/OTF → выбрать
его у текста/часов/части sensor → preview → Apply → сохранить профиль.
Импорт принимает отдельный font до16MiB; TTC/WOFF не принимаются. Импорт
сохраняет bytes и managed key, не переименовывает family и не применяет тему сам.
Пользователь выбирает файл, разрешённый его лицензией; наличие файла в Windows
не означает право распространять его с Linux приложением.

Три подтверждённых свободных источника включены с полными лицензиями/provenance.
Осталось сравнить текстовую вёрстку/shaping/raster с Windows. Для шести
остальных families использовать существующий import workflow, не обещая их
наличие в чистой установке.

[Проверка renderer/IPC/GUI/profile/restart трёх новых fonts](evidence/2026-10-10-display-fonts.json).

[Noto OTF: comparison, reproduction, renderer и native acceptance](evidence/2026-10-10-noto-coverage.json).


CSS em sizing [исправлен](text-em-sizing.md): `line_box:true` использует
px-per-em вместо metric height; legacy false неизменён. Полный shaping/raster
и Windows image comparison остаются открытыми.

OpenType shaping и paragraph bidi теперь [реализованы](text-shaping-acceptance.md).
Обычные Unicode/CJK переносы также [проверены](text-wrapping-acceptance.md).
Остаток: locale-specific breaking, Windows-specific fallback и Windows raster comparison.
CSS normal wrapping/whitespace и [bundled glyph fallback](text-shaping-acceptance.md#fallback-отсутствующих-glyphs--2026-10-11) реализованы; это не полное воспроизведение системного font resolver Windows.

## Upload messages and cleanup — 2026-10-17

Success/cancel и local size/offset/missing-asset ошибки имеют reactive EN/RU
resources; внешние diagnostics сохраняются дословно. Cleanup scan/delete failures
не повторяют операцию при смене языка и требуют explicit rescan. Импорт добавляет
файл в каталог, а выбор шрифта и Apply темы остаются отдельными действиями.
[Browser/native evidence](evidence/2026-10-17-library-resources.json) также проверяет
retained statuses, отмену и коллизии текста ошибки с resource key. Это post-package
working tree; расширенная native/session/physical/font raster приёмка открыта.
