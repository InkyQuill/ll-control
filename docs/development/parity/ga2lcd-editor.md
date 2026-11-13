# GA2LCD Editor: контракт и реализация

> **Current-status note 2026-10-17:** early sections below are dated checkpoints. Video editing and native image/gallery workflows are now implemented; remaining reference/platform/fault checks are in the [release audit](release-audit-2026-10-17.md). Direct composition-canvas editing is a separate missing workflow (R1), not satisfied by crop dragging.

Baseline: Windows L-Connect 3 **2.1.28.0**, Galahad II LCDV2. Сверка
2026-10-16. Обычный GA2LCD Editor применим; отдельный Screen Animation Editor
для HydroShift не входит в этот контракт.

## Подтверждённое поведение изображений

Reference root (вне Linux repository):
`/home/inky/Development/lian-li/reference-decompiled/2.1.28-2026-09-08/L-Connect Editor/`.
SHA-256 исходного EXE и общая provenance: [матрица](lcdv2-2.1.28-matrix.md).
Все пути ниже относительны `LConnectEditor/`.

| Действие | Source evidence | Требование |
|---|---|---|
| Выбор области | `MainWindow.cs:91–94,155,191`; `Views/Tools/ImageCropTool.cs:getCaptureStatus/getCaptureBitmaps` | GA2LCD single-screen target 480×480; квадратная область, пересчёт display scale в source coordinates; crop → resize |
| Маска | `MainWindow.cs:91–94` | GA2LCD задаёт `ShowCaptureBorderMask=false`; не выводить обязательную круглую маску из формы LCD |
| После crop | `ImageCropTool.cs:confirmButton_Click` | Можно импортировать результат либо перейти к ImageEdit; cancel не публикует |
| Поворот | `ImageEditTool.cs:rotateLeftButton_Click/rotateRightButton_Click` | −90°/+90°; изменения base image накапливаются |
| Отражение | `ImageEditTool.cs:flipHorizontalButton_Click/flipVerticalButton_Click/updateEffectFunctionButtonViews` | GA2LCD показывает обе кнопки; операции должны сохранять порядок |
| Цветовые фильтры | `ImageEditTool.cs:applyEffect` | Hue → saturation(value/25+1) → sharpen(value/5) → brightness(value/100)/contrast(value/100+1) → blur(value/5) |
| Выход ImageEdit | `ImageEditTool.cs:getToolResult` | Для одного экрана resize до screenSize. GA2LCD не попадает в guards, принудительно выключающие PNG; alpha PNG сохраняется |

Диапазоны/defaults подтверждены восстановленным XAML (таблица ниже). Формулы
передачи параметров в Telerik не доказывают эквивалентность самих фильтров.
Не подменять их случайными диапазонами или одноимёнными `image` algorithms.

В `getCaptureBitmaps` видна коррекция края относительно PixelWidth/Height−1.
Linux сохраняет последнюю строку/колонку валидного выделения; воспроизведение
этого возможного off-by-one не заявляется. Fractional selection rounding и
точное WPF/Telerik resampling остаются предметом сравнения.

## Реализованная подготовка geometry

`crates/llcontrol-image/src/editor.rs` предоставляет чистую
`prepare_geometry(source, crop, transforms)`:

- bounded decode с применением EXIF orientation до координат;
- явная квадратная область в пикселях исходника, без неявного clamping;
- crop перед преобразованиями; rotate/flip в порядке действий;
- edit-only без crop сохраняет всё изображение, растягивая до 480×480;
- RGBA PNG 480×480, максимум 1MiB для существующего asset import;
- максимум 64 geometry actions — локальный resource limit, не Windows limit;
- исходные bytes не меняются, записи в assets/HID отсутствуют.

Lanczos sampling отличается от непроверенного WPF raster; PNG используется
как внутренний managed format даже для JPEG source. Подготовка подключена к Tauri `image_editor` и GUI «Импорт с редактированием».
Один blocking worker удерживает admission permit до окончания обработки,
даже если caller перестал ждать; конкурентные запросы не накапливают очередь.

GUI сохраняет полный исходник до crop, показывает ориентированный preview,
поддерживает numeric/pointer selection, rotate/flip, отмену последнего
преобразования и сброс. Preview готовится явно; изменение параметров запрещает
сохранение устаревшего результата. Закрытие отбрасывает поздние ответы и
освобождает URLs. Сохранение использует существующий import → membership путь
и добавляет слой только при прежнем selection и свободном месте в черновике.
При изменении соединения или заполнении 32 слоёв сообщается «сохранено в
галерее», без ложного сообщения о добавлении в draft. Неопределённый ответ
не вызывает автоматический retry. Прямой импорт без редактора сохранён.

Это geometry workflow, **не весь GA2LCD Editor**: повторное редактирование
gallery images подключено; hue/saturation/brightness/contrast реализованы и software-tested
(см. checkpoint ниже), Windows raster ещё не квалифицирован. Sharpen/blur подключены последующим checkpoint; video editor
отсутствует. Native WebKitGTK этого
нового сценария ещё не проверен.

## Оставшаяся реализация и приёмка

- [ ] Расширенная browser/native приёмка pointer crop, undo/reset и keyboard focus.
- [ ] Сквозная native gallery thumbnail → editor → theme preview/Apply;
  error/focus/scaling и сохранность оригиналов.
- [ ] Одинаковые fixtures на Windows/Linux: rounding/sampling/non-96-DPI
  и отличие программного export от экранного GPU preview.
- [ ] GA2LCD VideoCut/VideoEdit/AutoFill: контракт времени, crop, format,
  audio/loop и speed (preview speed не считать export speed без трассировки).
- [ ] Screenshot/record: точный GA2LCD контракт и X11/Wayland workflows.
- [ ] Browser и native keyboard/focus/error/cancel/scaling/accessibility.
- [ ] Позднее аппаратно: импортированный результат → LCD, замена/stop/replay,
  совместно с датчиками и охлаждением; после теста LocalUI.

Software checkpoint: [geometry](evidence/2026-10-12-editor-geometry.json).

[GUI software checkpoint](evidence/2026-10-12-image-editor-gui.json):
22 GUI Rust tests, 166 browser tests, frontend check/build, strict Clippy и docs.
В этом раннем checkpoint не было native WebKitGTK или аппаратной приёмки.
Текущий bounded native gallery workflow описан ниже; физический LCD не проверялся.

## Повторное редактирование галереи

Windows `L-Connect 3/LConnect3cs/Views/UserControls/WMSQ/Gallery.cs:480–535`:
`editButton_Click` запрещает defaults/GIF, запускает ImageEdit, копирует результат,
удаляет старую запись и файл, добавляет и выбирает новую запись. Это **замена**,
а не добавление ещё одной карточки. `updateShowEditButton:1058–1060` подтверждает guard.

Linux `read_theme_asset_for_edit` читает полные bytes нормализованного managed PNG
(не thumbnail и не исходный файл до первого импорта), максимум 1MiB. Чтение
сериализовано с catalog mutations, не добавляет session pins; malformed keys,
builtins, symlinks, corrupt/missing/oversize files отклоняются.

Save в редакторе галереи получает revision snapshot коллекций до импорта нового
asset, затем revision-checked заменяет **только выбранный контекст** и читает
результат. Legacy каталог сначала переносится в коллекции в рамках явного Save.
При потере ответа/конфликте требуется readback, автоматической повторной
публикации нет. Прежний файл остаётся в полном каталоге: его могут использовать
другие коллекции, profiles и активные темы. Это намеренное отличие от удаления
файла в Windows; отображаемая коллекция заменяется, существующие ссылки целы.

Совпадающие references выбранного контекста в текущем draft переключаются на
новый key только при неизменившемся selection. Сохранённые profiles и LCD
не переписываются автоматически. Возврат focus после обновления списка проверен browser. Native focus,
реальное LCD поведение, legacy migration в GUI и совместные внешние mutations
требуют дальнейшей приёмки.

## Точные диапазоны фильтров

Из восстановленного `views/tools/imageedittool.xaml`: slider bounds привязаны
к соответствующему NumberTextBox. Все начальные Value равны 0.

| Control | Minimum | Maximum | Передача в Telerik |
|---|---:|---:|---|
| Hue | 0 | 360 | value |
| Saturation | -25 | 75 | value / 25 + 1 |
| Brightness | -100 | 100 | value / 100 |
| Contrast | -100 | 100 | value / 100 + 1 |
| Sharpen | 0 | 10 | value / 5 |
| Blur | 0 | 10 | value / 5 |

`videoedittool.xaml` задаёт Brightness/Contrast **0..255**, Value0. Это другой
SDK path; переносить туда image ranges/algorithms нельзя. Coercion/rounding
NumberTextBox и pixel algorithms ещё исследуются.
[Source evidence](evidence/2026-10-12-editor-xaml.json): 6/6 BAML восстановлены
из копий, originals hashes неизменны; proprietary XAML находится вне repo.

[Saved-image editor checkpoint](evidence/2026-10-12-gallery-image-editor.json):
780 workspace tests, 1 ignored; 173 browser tests, fmt/strict Clippy/docs,
frontend check/build. Native/hardware ещё не запускались для этого среза.


## Цветовые point filters — checkpoint 2026-10-12

`editor_filters.rs`, Tauri DTO и `ImageEditor.svelte` поддерживают hue,
saturation, brightness/contrast в Windows UI ranges. Validation запрещает
нечисловые/неограниченные значения; изменение controls инвалидирует prepared
preview и Save. Reset возвращает neutral. No-crop путь обрабатывает source
resolution перед итоговым resize, crop путь сначала нормализуется до480×480.

Порядок: premultiply → hue → saturation → brightness/contrast → unpremultiply,
между активными stages UNorm8 quantization. Hue использует reciprocal multiplication
и HSV interpolation max*(1−s*f), saturation — luma(.299,.587,.114), contrast —
((rgb/alpha−.5)*factor+brightness+.5)*alpha. Alpha сохраняется; neutral не меняет
bytes, alpha0 при active filters canonicalized в transparent black.

Reference: Telerik assembly из той же установки Windows2.1.28; EffectHelper
создаёт отдельные Pbgra32 RenderTargetBitmap. Disassembly и собственный offline
interpreter находятся вне repository; hashes и пути в evidence ниже.

Исправлена ошибка первоначального oracle: Python float64 давал red92 для
RGBA[190,80,90,80], controls[90,75,10,25]. f32 constants/register writes дают
после hue premult[46,60,25,80], saturation[29,85,0,80], contrast[34,104,0,80],
после unpremultiply[108,255,0,80]. 12 именованных и128 seeded vectors (seed7395)
проверяют результат с tolerance1/channel. Expected исправлен после проверки
промежуточных регистров, не путём увеличения допуска.

Это offline approximation: mad/dp3/lrp интерпретируются с одним f32 rounding
на instruction, scalar Rust может округлять intermediates иначе. WPF target
rounding, alpha compositing, actual GPU precision/sampling ещё не подтверждены.
Sharpen/blur требуют реализации и edge sampling qualification. Большой source
с64 transforms требует проверки стоимости обработки. Native editor/hardware
не запускались; browser bridge fixtures сами по себе не проверяют Rust pixels.

[Software evidence](evidence/2026-10-12-editor-point-filters.json):79 focused Rust,
13 browser tests, fmt/strict Clippy/docs и frontend check/build прошли.


## Sharpen/blur — software checkpoint 2026-10-12

Оба controls0..10 подключены к той же validation/preview invalidation/reset и
publication. Полный порядок: hue → saturation → sharpen → brightness/contrast
→ horizontal blur → vertical blur. Рабочие pixels premultiplied RGBA8;
перед/между stages квантизация, в конце один unpremultiply. Sharpen сохраняет
alpha; blur обрабатывает все четыре канала. Скрытый RGB полностью прозрачного
source не попадает в цвет растущей размытой области.

Sharpen: centre + (value/5)*NW − (value/5)*SE, Scale1. Blur: 13 samples каждой
оси, symmetric weights centre .199471; distances1…6: .176033, .120985,
.064759, .026995, .008764, .002216. Коэффициенты не перенормированы. Шаг по
нормализованной оси = (value/5)/originalInputSize. OriginalBitmap dimensions
сохраняются при rotate/flip: поворот прямоугольного исходника меняет шаг в
пикселях, а не только геометрию. Linux передаёт pre-transform dimensions;
non-96-DPI WPF logical dimensions ещё требуют отдельного сравнения.

Sampling установлен точнее: Telerik не задаёт explicit sampler mode, WPF
использует Auto. EffectHelper создаёт RenderTargetBitmap; Microsoft относит
[этот путь к software rendering](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/graphics-rendering-tiers).
В [WPF ShaderEffect.cpp](https://github.com/dotnet/wpf/blob/a088618af985b17dfbc2327c7caed84f7758a2ff/src/Microsoft.DotNet.Wpf/src/WpfGfx/core/resources/ShaderEffect.cpp)
Auto выбирает nearest в software и bilinear в hardware. Texture sampler
clamp ограничивает края. Linux использует normalized f32 UV, nearest с
ties-even, clamp. Упрощение в pixel-space даёт другие результаты на half-texel;
тест blur2.5 сохраняет этот случай.

Это source-derived software model; открытый WPF commit не является binary
qualification .NET Framework из Windows установки. MXCSR rounding, actual
coordinate accumulation/half-texel offsets, reciprocal approximation, non-96-DPI
и WPF intermediate compositing остаются в checklist. GPU preview также может
отличаться от экспортируемого изображения. Не заявляется pixel-exact parity.

Память пространственных stages: один дополнительный RGBA8 buffer; blur меняет
местами source/destination между двумя проходами. Исходные decode bounds и
один blocking worker сохранены; release resource samples и оптимизация64 transforms
описаны ниже. Остальные formats/content и worst-case workloads ещё требуют измерений.

Проверки: четыре независимых shader-grid fixtures (sharp, blur, combined и
fractional), полная цепочка после поворота5×3→3×5, alpha-edge/hidden RGB,
invalid values до mutation; browser limits/fractional/empty/reset/stale preview.
84 focused Rust (83 в общем прогоне и1 дополнительная регрессия) tests,14 browser tests, fmt/strict Clippy/docs/frontend check/build.
[Evidence/provenance](evidence/2026-10-12-editor-spatial-filters.json).

После sharpen нейтральный contrast-проход сохраняется: alpha0 RGB очищается
до blur. Независимый transparent fixture и отдельный regression подтверждают
отсутствие распространения невидимого цвета из этого промежуточного stage.

## VideoCut: первый software slice

[Контракт видеоредактора](video-editor.md): ordered temporal intervals при
импорте реализованы, включая repeat/reverse order, duration validation и
sequential preparation. Source preview/scrubbing и выбор границ по текущему кадру
также реализованы. Per-interval spatial crop с GUI/local frame preview и prepared-result playback/confirmation подключены. Record/pause source intervals также реализован.
Gallery re-edit, VideoEdit/AutoFill и native qualification остаются открытыми.


## Ordered geometry and resources — 2026-10-16

The editor now composes ordered90° rotations/reflections before touching pixels.
There are eight possible outcomes; applying one reflection followed by one
rotation needs at most two full-frame operations, even after64 button actions.
Crop/resize order, original spatial-filter input dimensions, RGBA bytes, action
limit and bounded worker ownership remain unchanged. Every action word of
length0..6 on four rectangular/one-pixel-axis sources plus63/64-action sequences
matches sequential operations over all pixels/dimensions (21,852 comparisons).

A dedicated release example accepts an encoded source and writes the480×480
result. Source generation/build must stay outside process-RSS measurement:

```sh
cargo build --locked --release -p llcontrol-image --example profile-editor
# OUTPUT is a separate test file; SOURCE remains the encoded input.
target/release/examples/profile-editor SOURCE OUTPUT 64 all
```

Use wait4/ru_maxrss (Linux KiB) or GNU time around the executable for memory/CPU;
the example prints preparation latency. Point/all presets are deterministic and
include non-neutral filters, which preserve full-source processing before resize.
The output is validated as480×480; this fixture never accesses assets or HID.

| Synthetic input / actions / filters | Before | After | Output |
|---|---:|---:|---|
|3840×2160 /64 /brightness |800ms |316ms |Byte-identical PNG |
|3840×2160 /63 /all six |2187ms |1632ms |Byte-identical PNG |
|8192×2048 /64 /all six |12669ms |3687ms |Byte-identical PNG |

Six before/after public-pipeline output comparisons passed. These are single
samples on the recorded host; they do not establish general timing guarantees.
Peak RSS was about69MiB for4K and138MiB for8192×2048 and did not materially change.
The64MiB decoder allocation limit is not a total process memory cap. Different
formats/content, worst-case decode/output compression, concurrent workloads and
host variation remain open. Windows WPF/GPU/raster/DPI parity and physical LCD
acceptance remain separate. Full Rust checks:1235passed/9ignored, fmt, strict
Clippy and warning-denied docs. [Evidence](evidence/2026-10-16-editor-transform-resources.json).


## Native saved-image gallery and crop input — 2026-10-16

The private native Tauri/WebKitGTK EN/RU harness now uses actual X11 input for
crop controls and Save. From a focused crop checkbox, Space enables crop; Tab
traverses X/Y/side and native typing enters30/20/150. AT-SPI locates the source
button for a real mouse drag outside its bounds, clamping to270/130. Moving the
released pointer back inside must leave that crop unchanged. Each locale accepts
9 trusted key steps and8 trusted input events; Save is triggered by Enter.
Transform/filter controls remain DOM-driven:64 actions, Undo, all six filters,
full native preview pixels and saved PNG match a fixed explicit engine request.
This proves UI mapping/runtime, not a Windows raster oracle.

Trusted input exposed two defects. Default draggable IMG started native image
DnD, interfering with pointer release and keyboard activation. The crop source
alone now sets draggable=false. Drag state also ends before mutation when the
primary button is absent or pointer capture is lost, including a delayed
lostpointercapture event. Four EN/RU browser regressions cover missing release,
explicit capture loss, unchanged crop and a successful fresh drag.45 affected
browser cases, Svelte0/0, resources915/915, fmt and strict GUI Clippy passed.
[Input findings, traces and checks](evidence/2026-10-16-native-image-crop-input.json).

Preview does not publish. Save creates one derivative and replaces the selected
collection entry; original bytes and the active old theme remain unchanged until
Apply. Reopen/edit/Close without Save preserves membership/catalog. Thumbnail and
focus contracts remain checked. The parent compares complete actual MockBackend
JPEG bytes to the applied static image theme. Both locales finish in LocalUI,
close normally with joined workers/empty staging and no PWM/RGB/firmware changes.

```sh
pnpm --dir desktop build
cargo build --locked -p llcontrol-daemon --example desktop-mock
cargo build --locked -p llcontrol-gui --features native-smoke,custom-protocol
python3 desktop/scripts/native-image-editor.py
```

Run native harnesses sequentially. The wrapper creates private Xvfb/Openbox,
D-Bus/accessibility buses and XDG/TMPDIR; its xdotool input targets that display.
Real user desktop/Wayland/fractional-DPI, speech, complete chooser/Tab navigation,
errors/lost replies/concurrent edits, other-context/profile/restart acceptance
remain open. No physical HID is used. The previous DOM-only gallery checkpoint
is retained in [its dated evidence](evidence/2026-10-16-native-gallery-image-editor.json).
The fixed package is [qualified separately](evidence/2026-10-16-crop-production-candidate.json).
