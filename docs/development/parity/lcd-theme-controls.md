# LCDV2 theme controls: Windows → Linux

Сверка 2026-10-10, baseline Windows 2.1.28.0. Reference root:
`reference-decompiled/2.1.28-2026-09-08/L-Connect 3/LConnect3cs` рядом с репозиторием.
Это source evidence и software coverage, не новый hardware результат.

| Windows control | Source относительно reference root | Linux / оставшаяся проверка |
|---|---|---|
| Watermark image, Horizontal, Vertical, Shadow, Opacity | `Types/WMSQ/WMSQScreenWatermarkConfig.cs:5–15`; `Views/UserControls/WMSQ/WatermarkLayoutConfigurationPanel.cs:147–177` | Managed image layer, x/y/rectangle, opacity; alpha shadow добавлен. Browser semantics установлены ниже; optional watermark_position добавляет center anchor |
| Custom text и small/medium/large | `Products/Galahad2Vision/Galahad2VisionScreenSettingPage_ImageVideo.cs:130–146,440–458` | Text layer, editable size (включая 25/43/65 px из Windows), color/font/shadow, wrap/line height/vertical anchor реализованы; Image/Color wrapper geometry установлена ниже; glyph/line-box equivalence остаётся |
| Time: три LabelColor, font, horizontal/vertical/text alignment, Shadow | Тот же файл `:110–127,422–438,828–900` | Clock layers date/time_without_period/period дают три independently styled элемента; Clock preset обновлён |
| Sensor layouts, в том числе three/four/slider | Тот же файл `:538–825`; `Private/TextSliderSensorBase.cs` | Focus/Duo/Triple/Quad/Cycle, binding order, interval, °C/°F. Полный provider inventory ведётся отдельно |
| Fonts | `WMSQScreenFontFamilies` в перечисленных config fields | DejaVu Sans/Serif/Mono, Noto Sans TC Regular/Bold, Abril/Caprasimo/Cubic11; остальные6 families через user import, Noto coverage и точные metrics остаются |

## Image shadow contract

`image.shadow` — optional object с теми же полями, что `TextStyle.shadow`:
`x/y` −32…32 px, `blur` 0…16, RGB `color`, `opacity` 0…100.
Если поле отсутствует или null, поведение прежнее; при serializing None поле
не добавляется, version 1 остаётся совместимой со старыми темами.

Renderer строит маску alpha уже вписанного/cropped изображения на canvas
480×480, размывает её, применяет offset и рисует под image layer. Transparent
pixels не создают прямоугольную тень; opacity слоя умножает opacity тени.
Выход за canvas отсекается. Это собственный bounded renderer; equivalence
Windows CSS Shadow по одному совпадению названия поля не утверждается.

## Остаток controls audit

1. Проверить визуальную геометрию на LCD позднее: contain_native сохраняет
   небольшой asset без увеличения; center/percentage и Clock parts реализованы.
2. Закрыть font catalog coverage. Перенос, line height и vertical anchor
   реализованы; Windows-specific margins/точная геометрия остаются визуальной сверкой.
3. BackgroundColor page сверена с ImageVideo: отдельные gradient controls
   подтверждены и реализованы (см. checkpoint ниже). Физический вывод
   gradient backgrounds остаётся в hardware checklist.
4. Включить подтверждённые controls в mock/native acceptance, а визуальную
   проверку на LCD оставить в hardware checklist.

## Установленный browser baseline

Service `Galahad2VisionController.cs:1093` открывает AssetsWMSQDirectory/index.html;
Core `LConnectApp.cs:373` задаёт `Assets/ga2v`. В пользовательской Windows-копии
`winver/L-Connect 3/Assets/ga2v/index.html` подключён **app.8fc94db7.js**.
Соседний app.9d60cfd9.js не является подключённым baseline и не используется
для этих выводов. SHA-256:

| Файл | Hash |
|---|---|
| index.html | `598e0b01927b25fd54fb2eb162f7d0ba693a9d551708546ea8e8da544c622fb8` |
| js/app.8fc94db7.js | `f3691c15dbe62d1248abee444dc94c82a34c46e45697bda4ccad77825c6e878a` |
| css/app.430a4e44.css | `c0f1d6d5bb1febfe0503b78a8ee23c3416cee0c044a3d4513e4f9342fa4357ec` |

Minified JS, line 1, symbol `TextTimeShowMode`: local date YYYY/MM/DD,
12-hour hh:mm, separate A.M./P.M.; FontColors[0/1/2] respectively date/time/period.
Update timer 500ms. Text shadow uses 3px/3px/10px black with configured opacity.
Linux Clock preset implements these three contents and independent style/color;
1Hz daemon renderer remains sufficient for seconds/minute display, not an
assertion of identical browser cadence or glyph geometry. Old Clock layers retain
combined time/AM-PM formatting. All parts in a frame use one local timestamp.

`WatermarkShowMode`: positive Horizontal/Vertical are percentages; non-positive
values fall back to 22px. Opacity and Shadow are percentages, negative fallback
75%. Shadow is black CSS drop-shadow with fixed 5px/5px/5px and variable alpha.
CSS `.waterMark_img` uses contain, max-width/max-height 100%, height auto,
translate(-50%,-50%) center anchor and z-index 20. Linux offers equivalent
shadow inputs and optional center/percentage placement (rounded to integer pixels),
while retaining explicit size/layer order. Gaussian rasterization is not claimed pixel-identical.

Proprietary JS/CSS/assets remain outside the Linux repository; only behavioral
findings and hashes are recorded here.

## Watermark position contract

Optional image.watermark_position = {horizontal, vertical}, each 0…100.
Positive values place the fitted image center at that percentage of 480px;
zero places it at 22px, matching the baseline fallback. Coordinates round to
integer pixels. x/y are ignored in this mode but retained for switching back.
Without this field the previous top-left rectangle contract is unchanged.

Width/height remain 1…480. Contain centers the actual fitted image rather than
its empty letterbox. Off-canvas pixels are clipped at final composition;
alpha shadow uses a bounded source-local padded mask (maximum 576×576), retaining
invisible source pixels that can cast visible shadows. Render-time pixel tests
cover contain geometry, 0/50/100 placement, edge clipping and off-canvas shadow.
Current UI exposes explicit sizing/fit. contain_native avoids upscaling the
managed asset; subpixel browser rasterization equivalence is not claimed.

## Sensor parts and native image size

`Sensor` and `SensorCycle` accept optional `parts: {label: TextStyle, unit: TextStyle}`.
Layer `style` then controls the numeric value only; parts styles control binding
label and unit independently. With no parts, legacy combined text is unchanged.
Missing/stale/invalid value remains —; label and unit are semantic metadata and
remain visible. Cycle swaps all three parts from the same selected binding.
Nested styles have the same bounds and font/shadow rules as ordinary text.
The active app.8fc94db7.js uses FontColors[0/1/2] for Type/Value/Unit.

`fit: contain_native` fits within width/height without enlarging the decoded
managed asset; ordinary contain still scales up. Center placement works with
actual fitted dimensions. Import normalization may already downscale large
input to the managed limit; this is not recovery of original file dimensions.
Windows CSS intrinsic sizing and max constraints motivated this option.

## Custom text layout and fonts

Active `app.8fc94db7.js`, `TextCustomShowMode`, supplies wordBreak=break-word
and per-size line heights. Image/Color defaults: size25→29px, 43→58px,
65→77px. CSS `.text_wrap.wrap_custom_large` has width/max-width358px;
other effect layouts use distinct widths/margins. Wrapper positions use
horizontal/vertical alignment; text alignment within it is separate.
Explicit Windows newline handling drops empty entries. Linux preserves empty
lines intentionally, with an optional max_width and line_height rather than
silently changing legacy output. Word wrapping prefers whitespace boundaries
and splits oversized words only between Unicode graphemes. This is not full
browser shaping or pixel-identical CSS layout.

TextStyle adds optional max_width (1…480), line_height (8…320), and
vertical_align top(default)/center/bottom relative to y. Width limits line
layout rather than clipping individual glyphs; a grapheme wider than the box
is retained whole and clipped only at the canvas. Defaults retain old JSON
and renderer behavior. All text parts share this contract, including sensor
parts and clock. Main-layer GUI exposes these controls.

Windows `WMSQScreenFontFamilies.cs` lists: AbrilFatface, AgencyGothicCT_Bold,
BakerieSmooth_WideMediumRegular, Caprasimo_Regular, GoodTimesRg_Italic,
HyperspaceRace_BoldItalic, LTCFournierLeJeune_Regular, ManoloMono_Regular,
NotoSansTC_Regular, NotoSansTC_Bold, Cubic11. Linux offers bundled
DejaVu Sans/Serif/Mono plus Noto Sans TC Regular/Bold (checkpoint below).
The remaining catalog and exact Windows font-version equivalence are open. Do not
copy Windows font files into the project based only on their presence in the
installation; establish licensing or provide a user-managed import path.


## BackgroundColor: подтверждённый пробел, 2026-10-10

[Source evidence и hashes](evidence/2026-10-10-background-color-audit.json).
`Galahad2VisionScreenSettingPage_BackgroundColor.cs:24–103` задаёт SolidColor,
HorizontalGradient, VerticalGradient, RadialGradient и отдельные настройки
каждого типа: Colors и Flip. Defaults: solid #34e6d1, gradient #34e6d1 → #1a7369.
`GetConfigs` (:385–553) передаёт Theme.Type=Color, ColorType, Colors, Flip.
Остальная сборка overlay configs (начиная с `object obj`) идентична ImageVideo
после удаления комментариев декомпилятора: sensor/text/clock/watermark не
требуют второго renderer. Это сходство выходной структуры, не всех defaults:
например SliderSensor Shadow здесь50, в ImageVideo60.

Подключённый `app.8fc94db7.js`, line1, `ColorShowMode` (offset около484422):

- SolidColor заполняет canvas первым цветом; пустой массив даёт чёрный.
- HorizontalGradient: первый цвет слева, второй справа; коэффициент x/width.
- VerticalGradient: первый цвет сверху, второй снизу; коэффициент y/height.
- RadialGradient: центр (width/2,height/2), радиус до угла. Внешние круги имеют
  второй цвет, внутренние — первый; радиус уменьшается с шагом2px, цвет
  интерполируется по radius/maxRadius. Центр и углы важнее CSS названия: это p5
  canvas drawing, не CSS radial-gradient с радиусом до ближайшей стороны.
- Flip разворачивает массив цветов перед рисованием. Настройки разных типов
  сохраняются независимо; переключение типа не должно стирать выбранную пару.
- Draw завершает `noLoop()`: сам цветной фон статичен, overlays обновляются отдельно.

Linux gap подтверждён в `LcdTheme.background: [u8;3]`, `render_theme_at` и
единственном input «Фон» в desktop. M1 тестовый градиент был готовым изображением
и не является поддержкой этого пользовательского workflow.

Следующий implementation slice:

1. Совместимое optional описание gradient с тремя режимами, двумя RGB и Flip;
   старые темы сохраняют solid background, unknown/malformed inputs отклоняются.
2. Renderer строит background до layers; preview и daemon loop используют один
   путь. Проверки направлений, Flip, center/edge/corner radial, одинаковых цветов,
   overlay composition и отсутствия изменения старой solid темы. Точную p5
   rasterization/antialiasing не объявлять доказанной только по sample pixels.
3. Editor переключает тип, редактирует цвета и Flip, сохраняет настройки разных
   типов; explicit Apply, profile save/load и restart сохраняют выбранный фон.
4. Browser/native scenario и hardware checklist; реальный LCD проверить позднее.

Fonts, margins и точная геометрия text layouts остаются отдельными задачами;
этот аудит не закрывает весь controls inventory.


### Linux gradient implementation

`background_fill` добавлен как optional v1 field; старый RGB остаётся fallback
для отсутствующего поля и solid mode. Три типа сохраняют собственные две RGB
и Flip; редактор «Фон LCD» меняет только draft до обычного Apply.

Renderer рисует gradient до остальных layers: horizontal x/480, vertical y/480,
radial hypot(x−240,y−240)/hypot(240,240). RGB линейно интерполируется и округляется
к ближайшему целому. Flip меняет местами endpoints. Это непрерывная интерполяция
с теми же направлениями/радиусом; p5 nested circles с шагом2 и antialiasing
не воспроизводятся пиксельно. Для solid старый raster path сохранён.

[Software acceptance checkpoint](evidence/2026-10-10-gradient-backgrounds.json):
527 all-target Rust tests, fmt/strict Clippy/docs, frontend check/build,
20 contracts, 34 browser regressions + новый gradient flow и native WebKitGTK
Apply/profile/restart прошли. Новый browser test проверяет независимые цвета/
Flip при смене типов и отсутствие автоматического Apply. Hardware отдельно.
Предыдущий раздел фиксирует исходный gap до этой реализации.


## Image/Color custom text geometry, 2026-10-10

[Source evidence](evidence/2026-10-10-custom-text-geometry.json), тот же активный
JS/CSS baseline. `TextCustomShowMode` выбирает `wrap_custom_large` для Image и
Color: width/max-width358px. Общий `.text_wrap` имеет padding22px, **но inline
padding w(effectMode,FontSize) для Image/Color возвращает0 и переопределяет его**.
Значит, ширина content box358px; нельзя автоматически вычитать44px из неё.
Для этих двух режимов max-height и letter-spacing остаются initial.

Позиция wrapper и TextAlignment дочерних строк независимы. Horizontal:
Left→left0, Center→left50%/translateX(−50%), Right→right0. При ширине358 на canvas
480 получаются левые границы0/61/122. Vertical: top0, center50% со сдвигом на
половину фактической высоты, bottom0; комбинацию center/center обслуживает
отдельный combined translate. TextAlignment выбирает text-start/center/end
внутри wrapper; размеры25/43/65 используют line-height29/58/77.

Существующий Linux max_width=358 и отдельные x/y позволяют выразить положение
строк без нового wrapper schema. Для левой границы блока L и line alignment:
left→x=L, center→x=L+179, right→x=L+358; y/vertical_align задают вертикальный anchor.
Это соответствие координат, не утверждение pixel-equivalence: высота блока в
нашем renderer использует font height +(N−1)×line_height, а browser CSS line boxes
и метрики оригинальных шрифтов ещё требуют сравнения. Browser text-align-last
у horizontal-left также следует учитывать при точной проверке последней строки.
Windows убирает пустые newline entries, Linux сохраняет их намеренно.

Режимы иных эффектов имеют собственные padding/max-height/letter-spacing и
не должны менять общие defaults Image/Color. Их сверка относится к реализации
соответствующих effects. Не вводить guessed global margin22 или обрезание высоты
из другого эффекта. В basic scope остаются font/TC coverage и проверка вертикальных
line boxes; аппаратная визуальная проверка выполняется позднее.


### Line-box option

`TextStyle.line_box=true` добавляет half-leading над/под glyph metrics и
высоту блока N×line_height; checkbox «Высота блока по строкам (CSS)» доступен
для основных text/sensor/clock layers. Formula следует принципу
[CSS2 leading/half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading).
Default false сохраняет прежнюю геометрию. Center anchor не сдвигает симметричное
содержимое; top/bottom учитывают дополнительное пространство по краям.

Это закрывает отсутствующий выбор line-box модели. Полная browser glyph
эквивалентность не заявлена: ab_glyph scale и browser font-size/em metrics
могут различаться, исходные Windows шрифты не перенесены. Для точного font
сопоставления нужны соответствующие font assets и отдельные reference images.


Line-box [software checkpoint](evidence/2026-10-10-text-line-box.json):
529 all-target Rust tests,35 browser scenarios,20 theme contracts и native
checkbox/load/profile/restart прошли. Top/bottom/center и negative leading
проверены pixel tests. Exact original-font comparison и hardware остаются.


## Bundled Noto Sans TC

Regular400 и Bold700 добавлены из официального Google Fonts, commit
`3be1884c48c3e45b52ecc725676a08f87776373e`. Это полный repertoire (20,745 cmap
entries в каждой static instance), без subsetting. [Provenance](../../../crates/llcontrol-image/assets/NotoSansTC-provenance.json),
[OFL](../../../crates/llcontrol-image/assets/NotoSansTC-OFL.txt) и
[подготовка](../../../crates/llcontrol-image/assets/NotoSansTC-README.md) находятся
рядом с файлами. Full OFL также встроена в font metadata бинарных assets.

Выбор доступен для основных text/clock/sensor styles и label/unit parts;
legacy DejaVu и absent→sans не меняются. Не добавлен автоматический fallback:
пользователь явно выбирает Noto Sans TC. Неподдерживаемые этим repertoire
символы, shaping, остальные9 Windows font choices и точное сравнение версии
Windows остаются отдельными задачами. Наличие шрифта с тем же именем не
доказывает одинаковые outlines с пользовательской Windows установкой.


[TC software checkpoint](evidence/2026-10-10-noto-sans-tc.json): native font
selection/profile/restart,530 Rust tests,35 browser scenarios,22 contracts;
два font binaries повторно получены побайтово. Это закрывает bundled TC slice,
а не весь Windows font catalog.

## Font catalog audit, 2026-10-10

[Полная таблица](font-catalog.md) уточняет реальные начертания и источники.
AgencyGothicCT_Bold фактически использует Condensed. Официальный Cubic11 v1.013
побайтово совпадает с Windows; Abril/Caprasimo совпадают по Unicode metrics,
но не bytes. Bundled Noto недостаёт3 Windows codepoints: U+2252/U+25C8/U+30EDE.
Это новый подтверждённый gap, не отмена пройденных TC sample tests.


CSS em sizing [исправлен](text-em-sizing.md): `line_box:true` использует
px-per-em вместо metric height; legacy false неизменён. Полный shaping/raster
и Windows image comparison остаются открытыми.
