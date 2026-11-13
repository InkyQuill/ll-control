# Windows profiles LCDV2

Baseline: Windows L-Connect3 2.1.28.0 / GALAHAD II LCDV2 0416:7395.

## FanAndLighting import: текущий путь

Для поддерживаемых source curves/parameters теперь есть полный target-bound
CLI/Unix IPC и desktop путь. Source inspection/preparation ниже остаются отдельными
read-only командами и сами ничего не импортируют.

```sh
# Проверка source + target proposal без изменения настроек
llcontrol --json profiles windows-control source.zip --record 1 --bindings bindings.json
# Явный commit и Apply; напечатанные ID/token сохранить для recovery
llcontrol --json profiles windows-control source.zip --record 1 --bindings bindings.json --apply
```

Формат bindings — [шесть явных ID/unit roles](../../../crates/llcontrol-profile/README.md#typed-linux-lighting-preparation).
`--device` выбирает target; при нескольких устройствах он обязателен.
`--hysteresis-c` по умолчанию0, `--fallback-percent`100. Windows hardware Metadata
не используется как target. Версия, группа и record выбираются явно, чужие
семейства отклоняются. Все6 lighting roles нужны даже для inactive modes; host
availability не квалифицируется offline. Cooling получает именно CPUTemperature
binding, с unit Celsius, а не первый найденный sensor ID.

Read-only `preview_windows_control_import` выдаёт exact target token и proposed
summary и typed review всех6/16 RGB modes, topology и96 dynamic bindings
без raw provenance. `import_windows_control` принимает original token, operation ID и тот же
bounded normalized draft. Group builder сохраняет target-side named Linux slots,
а Windows8/3 modes идут в отдельные banks; никаких8-to-4 truncation/slot guessing.
Manual PWM preferences заменяются source policies/IDs вместе. Полные6/16 manual
bases и96 dynamic bindings сохраняют per-mode enabled/selected state. Screen,
файлы/assets и global thermal policy остаются target-side.

Исходные RPM settings сохраняются в cooling bank source metadata; полный typed
lighting source report — в optional private `LightingSection.windows_source`.
Он bounded128KiB, object JSON и исторический: runtime не исполняет эти данные.
Legacy None отсутствует в serialization, старые hashes не меняются. Public summary
не раскрывает provenance; Linux full-group backup сохраняет его. После обычных
правок source остаётся историей, а не утверждением о текущих параметрах.

Commit использует existing FanAndLighting actor coordinator, settings/profile redo
journal и durable receipt. Новый HID/settings writer не создаётся. Digest domain
`llcontrol-windows-control-v1` включает source draft, fingerprints и bindings,
но не target-preserved Linux slots: это позволяет прочитать прежний receipt после
их более нового редактирования. Target token отдельно проверяется перед новым
commit. Потеря caller/соединения после admission не отменяет принятую операцию.

При неизвестном результате сначала используйте existing readback:

```sh
llcontrol --json profiles group readback fan-and-lighting OPERATION_ID
# Только явный retry с теми же исходными source/bindings/параметрами:
llcontrol --json profiles windows-control source.zip --record 1 --bindings bindings.json --apply --id OPERATION_ID --expected ORIGINAL_TOKEN
```

CLI печатает ID/token до admission. Retry требует оба параметра и `--apply`;
он не подставляет fresh token. Changed parameters с прежним ID и stale token
с новым ID отклоняются. Receipt/readback/reapply используют общую control-group
защиту; новые manual intent и slot edits не перезаписываются старым импортом.
Storage commit и runtime error различаются; при runtime failure committed result
выдаётся с ненулевым exit status и recovery ID. Автоматического reapply нет.

Приёмка: actual CLI/Unix IPC/MockBackend, потеря ACK после реального commit,
retry/readback, stale target и malformed drafts без mutation; сохранение named
slots/Screen/thermal, source provenance, portable archive round-trip и daemon
restart. Fixture объединяет собственные .NET cooling/lighting DTOs; это не genuine
Windows export. На итоговом source scope прошли1286 Rust tests/9ignored в176
result groups, strict Clippy/rustdoc/fmt;83/83 RPC overrides — inventory.
24 source/test hashes после проверок неизменны. [Evidence](evidence/2026-10-17-windows-control-import.json).

Открыто: genuine exports,
расширенные partial install/shutdown/native faults, неподдерживаемые RPM curves/
source parameters, Screen/fonts/media/assets mapper и integrated package. Реальные
USB/PWM/RPM проверки отложены. Наличие этого пути не означает full interoperability
для любого Windows ZIP и не закрывает всю базовую parity.

## Desktop import — 2026-10-17

В Settings → Settings group backup выберите Fan and lighting и **Choose Windows
ZIP…**. Native FileChooser выдаёт grant приложению; webview не получает путь,
исходный JSON или содержимое ZIP. Затем явно выберите source record и задайте
шесть Linux sensor ID с фиксированными units. Подсказки фильтруются по units;
отсутствующий или устаревший источник показывается как непроверенный. ID не
выбираются автоматически, наличие источника не выводится из Windows metadata.

**Review Windows settings** готовит предложение без commit:8 fan/3 pump modes,
6/16 RGB modes, topology и все96 High/Low bindings доступны для просмотра.
Изменение record/bindings блокирует Apply до новой подготовки. Native worker
держит исходный открытый regular-file descriptor и проверяет fingerprint при
подготовке; после подготовки **Import and apply** использует сохранённый exact
normalized draft и original target token, не перечитывая файл. Новый файл требует
новой операции. Отмена и Close до Apply освобождают grant без импорта.

При неизвестном результате используйте **Read group result**. Явный retry в той
же сессии использует original native draft/ID/token; поля формы не меняют его.
После перезапуска localStorage хранит только identity операции: доступно чтение
receipt, а source grant не восстанавливается и новый импорт не запускается.
Source file не нужен для чтения уже записанного результата. Reapply сохранённой
группы — отдельное явное действие, не второй storage commit.

Actual Tauri/WebKitGTK/private FileChooser/Unix IPC/MockBackend сценарии EN/RU
проверили отмену, новую подготовку, изменение bindings, explicit Apply, сохранность
Linux slots/Screen/local draft, readback/reapply и restart приложения/daemon с
удалённым исходным ZIP. Worker tests отдельно проверили реальную потерю ACK и
retry без второго Apply, повреждение источника и закрытие до admission.
[Evidence](evidence/2026-10-17-windows-control-desktop.json). Это owned fixture,
не genuine Windows export, system portal или physical USB acceptance. Текущий
collections package предшествует этому workflow; нужна общая новая сборка.

## Screen source banks and payload inventory — 2026-10-17

```sh
llcontrol --json profiles windows-screen source.zip --record INDEX
```

Команда работает offline без daemon/HID. Явный record должен принадлежать LCDV2
в Screen archive2.1.28.0. Она читает brightness, rotation enum0..3 → degrees,
отдельный runtime `ScreenConfigJson`, все8 nullable names, selected effect и
Style Express IDs и полные dictionaries `ScreenProfiles`. Exact JSON strings
и SHA-256 сохраняются отдельно для каждого editor setting и runtime config;
empty string означает Windows defaults. Даже неактивный malformed JSON, duplicate
key, неизвестный enum/key или неполный массив блокирует весь report. Это проверка
структуры, не семантики theme parameters или их преобразование в Linux.

Различие подтверждается `Galahad2VisionScreenPage.cs:99,204,221-244`: выводимый
config пишется из `GetConfigs()`, банк — из `Export()`. Для ImageVideo
`Export()` сериализует `ScreenSetting` (`ImageVideoPath`, Text* и Watermark),
а `GetConfigs()` строит `Style/Screens/Theme/Text/Watermark`. Импортёр не должен
трактовать эти JSON как один и тот же документ или восстанавливать одно из другого
с потерей inactive edits. Slot0 — рабочий банк UI; его данные и runtime config
сохраняются независимо, без предположения об их синхронности.

Три галереи сохраняют source paths и `IsDefaultFile` как данные; Linux никогда
не открывает эти пути. Uploaded references сопоставляются по basename с плоскими
`wmsq/` entries; missing payload и external default отмечаются явно. Default
reference не подменяется upload с совпадающим именем. Это candidate mapping,
а не доказательство тождества источников. Вложенные uploads отклоняются вместо
неявного flatten. Все payloads, включая thumbnails и unused uploads, потоково
проверяются до EOF/CRC и получают размер/SHA-256. После этого
`payload_streams_verified=true`, но `media_decoded=false`,
`theme_semantics_validated=false`, `group_import_supported=false`,
`apply_supported=false`; `archive.asset_payloads_validated` остаётся false.

Bounds Linux inspector:512KiB на embedded JSON,256 entries на галерею,
256MiB на payload и1GiB суммарно, плюс общие ZIP/profile limits. Это host-side
бюджеты, не заявленные Windows limits. Проверка backup fingerprint между чтениями
защищает от смешения разных source records; report не является immutable grant
для последующего commit. Исходный ZIP нужно заново подтвердить при подготовке.

[Evidence](evidence/2026-10-17-windows-screen-inspection.json): owned structural
fixture, все8 banks/runtime, payload hashes/CRC corruption, unsafe/nested paths,
size budgets и actual executable с listening Unix peer без connections/writes.
Payloads намеренно не являются декодируемыми media; genuine Windows export,
actual Windows UI, media decode и Apply этим checkpoint не подтверждаются.

### Basic runtime → native proposal

```sh
llcontrol --json profiles windows-screen source.zip --record INDEX --bindings screen-bindings.json
```

[Пример mapping](../examples/windows-screen-bindings.json) требует явное имя темы,
Linux sensor ID/unit/label/precision/temperature preference, выбранный Linux font
для каждого используемого Windows family и exact source path/URI → managed
image/video key. Пример явно заменяет HyperspaceRace на bundled Sans: это выбор
пользователя, не автоматический fallback или подтверждение эквивалентности шрифтов.
Custom font задаётся `font_asset` из существующего font import; активный renderer
не подменяет отсутствующий managed font. `NotoSansTC_Bold` отдельно нужен для
label/unit/date/period в Windows Slider/Duo/Time. Пустое source FontFamily означает
default HyperspaceRace для Image/Color, но Linux mapping всё равно обязателен.
Только реально используемые roles/fonts/resources попадают в used lists; host
availability не выводится из source metadata. IDs не выбираются автоматически.

В `runtime.theme` выдаётся валидный LcdTheme для Full/single screen:

- Image background — Stretch480×480, как p5 `image(...,width,height)`; для video
  создаётся version2 background_video, требующий существующий decoded frame.
- Color — Solid/Horizontal/Vertical/Radial и Flip; source runtime не содержит
  inactive gradient edits, они остаются в editor banks для следующего mapper.
- None/Custom/Time/SliderSensor/DuoSensor/ThreeSensor/FourSensor — native layers
  с сохранением source role order, source colors, loop interval, separate parts,
  opacity/shadow и source-family CSS font sizes. Custom повторяет удаление empty
  newline entries, width314 внутри358px wrapper и line heights25→29/43→58/65→77.
  Time строит Date/12-hour TimeWithoutPeriod/Period без секунд, как Windows JS.
  Grid имеет два цвета (value и label+unit), а не три; Shadow0/3/2 и opacity100.
- Watermark — отдельный верхний image layer, image-only exact resource binding,
  native-size contain,0→22px position convention, source opacity/shadow.

Это **предложение для layout review**, не full Screen import: native Duo/grid
geometry ещё требует сравнения с Windows, Three/Four dividers уже преобразуются в editable native layers, но
font-specific margins/CSS/raster не квалифицированы. Эти различия перечислены в
`review`; `layout_review_required=true`, `pixel_parity_verified=false`,
`resource_payloads_verified=false`, `sensor_binding_verified=false`,
`editor_banks_converted=false`, `apply_supported=false`. Ни source inspection,
ни эта preparation-команда не публикуют assets, не подключаются к daemon и не
делают Apply. Native proposal не должен выдаваться за lossless полный mapper.

Unknown source type/style/field, malformed palette/layout, missing binding,
wrong role unit, duplicate identities, mismatched Duo layouts, unsupported
watermark resource или managed key отклоняются без output/side effects.
Mapping ограничен64KiB, source runtime512KiB. File/URI строки сравниваются точно;
basename matching из inventory не используется для назначения executable assets.

Actual CLI→LcdTheme→renderer сравнил полные480×480 RGB buffers с независимыми
literal text inputs для valid/updated/missing/stale/invalid/wrong-unit/wrong-ID
и следующего cycle source: Fahrenheit/label/unit/identity остаются вместе,
unavailable не превращается в0. Отсутствующие managed image/font и video frame
дают явную ошибку вместо fallback. [Evidence](evidence/2026-10-17-windows-screen-runtime.json).
Это software cache/renderer acceptance, не Windows pixel или аппаратная приёмка.

### Basic editor banks → native proposals

```sh
llcontrol --json profiles windows-screen source.zip --record INDEX --bindings screen-bindings.json --editor-banks
```

Этот режим независим от `ScreenConfigJson`: он готовит существующие ImageVideo/
BackgroundColor entries восьми source array positions. Это explicit research
mode, а не восемь Linux named slots. Сохраняются исходные nullable names,
индексы, выбранные effect/Style Express и text overlay. Для каждого basic bank
выдаются все7 overlay proposals, а не только активный. Неполная либо неверная
неактивная настройка/роль/font/resource binding блокирует весь результат.
Numeric editor enums преобразуются отдельно от runtime strings: sensor1 означает
CPUTemperature, а не lighting SensorType.CPULoad; TextAlignment1=Right/2=Center.

Color preparation сохраняет independent Solid и все три inactive gradients с
их Colors/Flip в каждой native теме. ImageVideo использует точную строку editor
path; runtime URI для того же файла может требовать отдельный explicit mapping.
Отсутствующие банки не создаются; прочие effect dictionaries перечисляются как
`deferred_effects`, без подмены выбранной сцены. Все исходные JSON остаются в source.

Явно пустая settings string материализует frozen constructor defaults. Для
отсутствующего selected basic bank в index0 действует тот же Import(empty),
как Windows ScreenPage; `source_absent_materialized=true` отмечает этот случай.
Другие отсутствующие banks/неприменяемые индексы не создаются.
Slider Shadow различается: Image60/Color50; исходный default10-second sensor order
сохраняется. Image default lookup выбирает первый default `a_default_*.mp4` из
соответствующей source gallery, как Windows getter; source path всё равно требует
explicit managed video binding, Linux install path не угадывается.

`basic_editor_banks_prepared=true` сопровождается фактическими bank/overlay counts,
но `complete_screen_group_supported=false`, `apply_supported=false`. Layout/dividers,
resource actor validation/publication, current-scene attachment и target commit ещё
открыты. Owned fixture даёт16 banks/112 overlays; actual CLI проверил также
отсутствие daemon connection/config/archive writes и отказ при missing inactive
binding. [Evidence](evidence/2026-10-17-windows-screen-editors-and-wpf.json).

### Basic grid divider source audit

Shipped `ga2v/index.html` выбирает `css/app.430a4e44.css` и
`js/app.8fc94db7.js`. ThreeSensor содержит один горизонтальный и один нижний
вертикальный divider; FourSensor — один горизонтальный и два вертикальных.
Цвет берётся из `Layout[0].FontColors[1]`, как label/unit. CSS задаёт толщину2px
и black `0 0 10px` box-shadow; ThreeSensor добавляет horizontal margin-top12px.
Это статическое оформление basic presets, не отложенные procedural effects.
[Source evidence](evidence/2026-10-17-windows-grid-divider-source.json).

Linux mapper создаёт два/три editable Divider layers с source label/unit color,
толщиной2px и shadow; координаты/толщина допускают дробные значения. Обе basic
banks, active/cached themes и portable archives сохраняют настройки. Native
EN/RU проверяет edit → Save → отдельный Apply → actual mock-HID JPEG → restart.
[Implementation/package evidence](evidence/2026-10-17-grid-dividers.json).

Исходные Vue Three/Four components выполнены read-only с owned values для
Image/Color и трёх шрифтов:12 samples. [Geometry report](evidence/2026-10-17-windows-grid-component-geometry.json).
Three default: H(34,240,412), V(241,262,166); Four native default:
V(239,53,169), H(34,242,412), V(239,264,166). Абсолютные Four x positions
в оригинале меняются с font/value-dependent flex reflow; явные native defaults
не доказывают intrinsic layout/pixel parity. Shadow sigma5 лишь соответствует
CSS blur-radius10 по модели; Windows raster/shadow ещё не квалифицированы.
Realtime video+four sensors сначала failed20fps gate с dividers и без них.
После text shadow optimization minimum20fps пройден:23.40fps при nominal24,
весь723-frame stream decoded480×480. [Followup evidence](evidence/2026-10-17-realtime-shadows.json).
Physical/long-run qualification и original intrinsic layout остаются открытыми.

### Storage WPF Color versus runtime RGB: correction

Source audit уточнил важное различие: `Galahad2VisionController.ExportProfile`
(:191-200) и basic editor `Export()` вызывают JsonSerializer без converters.
Converters подключаются только при создании runtime config в ScreenPage и
`serializeScreenConfig`. Поэтому RGB-array synthetic fixtures прежних checkpoints
не доказывают формат сохранённого WPF Color. Это уточнение supersedes такую
интерпретацию старого source evidence; прежние ограничения genuine export остаются.

Новый codec принимает как RGB-array structural fixtures, так и default WPF objects
с byte/linear channels и null ColorContext. Runtime получает byte RGB, как
ColorToIntArray; lighting provenance дополнительно сохраняет исходные Color
property values по JSON pointers. Editor source JSON сохраняется целиком.
Unknown/missing Color fields и non-null profiles отклоняются, profile URI не
открывается. Публичные свойства подтверждены [WPF Color source](https://github.com/dotnet/wpf/blob/main/src/Microsoft.DotNet.Wpf/src/PresentationCore/System/Windows/Media/Color.cs),
а отсутствие специального Color converter — [System.Text.Json resolver](https://github.com/dotnet/runtime/blob/main/src/libraries/System.Text.Json/src/System/Text/Json/Serialization/Metadata/DefaultJsonTypeInfoResolver.Converters.cs).

Owned WPF-shape fixtures проверены через lighting/editor preparation и actual
control CLI/Unix IPC/MockBackend import с lost ACK, source-history persistence,
portable codec и restart. Измеренный полный draft161833 bytes/history105053 bytes
укладывается в прежние256/128KiB bounds. Это эмуляция публичных свойств, не запуск
WPF или genuine Windows export; CLR property-order/ICC/raster qualification
остаётся отдельной приёмкой. [Evidence](evidence/2026-10-17-windows-screen-editors-and-wpf.json).

### Verified static image preparation

```sh
llcontrol --json profiles windows-screen source.zip --record INDEX --prepare-images
```

Подготовка независима от bindings/runtime/editor selection. Source envelope и
backup fingerprint проверяются заново, затем каждый выбранный `wmsq/` member
читается в encoded budget и сверяется с size/SHA-256 из inventory, включая CRC.
Windows file paths не открываются. PNG/JPEG uploads, thumbnails и unused images
декодируются существующим bounded `llcontrol-image` pipeline и нормализуются в PNG
с сохранением alpha/aspect ratio и применением orientation. Signature должна
совпадать с extension. GIF/video/unknown entries перечисляются в deferred; disguised
GIF и APNG отклоняются вместо неявного преобразования в первый кадр.

`PreparedImages` удерживает owned PNG bytes для будущего actor-owned staging;
байты не сериализуются в CLI/webview report. Видны source/normalized hashes,
size/dimensions и deferred entries. До успеха всех выбранных images report не
выдаётся: поздняя ошибка не создаёт partial proposal. Глобальное хранилище не
меняется и managed keys не назначаются. `images_decoded=true` относится только
к выбранным static images; `assets_published=false`,
`complete_screen_group_supported=false`, outer Apply/group-import false.

Bounds: encoded16MiB/image,8192px/64MiB decoder allocation; normalized PNG≤1MiB
и32MiB суммарно для owned staging buffers,960px with480px fallback по existing
normalizer. Это не worst-case CPU/resource qualification. `--prepare-images`
несовместим с `--bindings`/`--editor-banks`; дальнейший builder должен соединить
подготовленные bytes с exact source reference remap и owned transaction.
[Evidence](evidence/2026-10-17-windows-screen-images.json): owned RGBA/JPEG,
source changes/budget/unknown entries, deferred animation и actual offline CLI
с no connection/config/archive mutation. Video/GIF/fonts и полный Screen commit
этим шагом не подтверждаются.

### Verified GIF/MP4 preparation

```sh
llcontrol --json profiles windows-screen source.zip --record INDEX --prepare-media
```

Payload size/CRC/SHA-256 и backup fingerprint проверяются до decoder. Временный
input получает фиксированное имя внутри private directory; JSON Windows paths
не открываются. Используется общий `llcontrol-media` pipeline, вынесенный из
Desktop без изменения defaults: Stretch480×480/24fps, без audio, bounded H.264
units и полная проверка подготовленного clip. GIF готовится как конечный набор
moving frames, а не первый кадр; playback loop остаётся отдельной runtime policy.
PNG/JPEG/fonts/прочие entries перечисляются как deferred, ничего не теряется молча.

Каждый `PreparedMediaItem` удерживает private disk owner; в report идут только
source/encoded SHA-256, frames/fps/size. Byte payloads и staging paths не
сериализуются. До успеха всех кандидатов output не выдаётся. Отмена прерывает
также source reads, затем shared pipeline завершает/reaps media-tool children;
CLI Ctrl+C ждёт blocking worker перед выходом. Drop очищает private staging.
Ни assets, ни managed keys, ни device settings не публикуются.

Per-source пределы прежнего pipeline (один час/256MiB encoded), общий batch:
не более64 GIF/MP4, один час frames и512MiB encoded retained data. Remaining
frame/byte budget передаётся engine до encode, а не проверяется только после
всего batch. Source payload≤256MiB и прежние ZIP budgets сохраняются. Это
Linux limits, не Windows format limits или worst-case resource qualification.

Actual owned GIF и MP4 проходят ZIP→pipeline→H.264→MP4 preview→RGB decode:
первый/последний кадры различаются, количество frames сохраняется при decode,
units валидны и Drop удаляет staging. Actual CLI не подключается к listening
Unix peer/не меняет source/config; bad media не выдаёт partial report. Source
fingerprint/early cancel/late bad member/remaining budget проверены отдельно.
[Evidence](evidence/2026-10-17-windows-screen-media.json). Native owner/UI regression
и ограничения записаны там; genuine Windows export, full resource remap и Screen
commit ещё не закрыты.

### Owned Screen resource plan

```sh
llcontrol --json profiles windows-screen source.zip --record INDEX \
  --bindings linux-bindings.json --resource-map resource-map.json
```

`resource-map.json` связывает **точную** исходную строку path/URI с одним из
строго типизированных targets:

```json
{
  "C:\\owned\\art.png": {"kind": "archive", "entry": "wmsq/art.png"},
  "file:///C:/owned/art.png": {"kind": "archive", "entry": "wmsq/art.png"},
  "C:\\owned\\old.png": {"kind": "existing_image", "key": "11111111111111111111111111111111"},
  "C:\\owned\\clip.mp4": {"kind": "existing_video", "key": "22222222222222222222222222222222"}
}
```

Нет basename matching, implicit Windows default substitution или чтения host
paths из JSON. Existing image допускает также известные immutable Linux builtin
keys; это явный выбор, а не утверждение равенства с Windows default. Один alias
не может одновременно присутствовать в `bindings.resources` и resource map.
Map≤64KiB/256 aliases, итоговые bindings сохраняют общий предел256 references.
Unknown/duplicate fields, unsafe entries и неверные keys отклоняются.

План повторно проверяет source backup/payload fingerprints, готовит все PNG/JPEG
и GIF/MP4 uploads, затем разрешает три галереи, runtime basic scene и все
существующие basic overlays текущего editor index0 (до14). Gallery-only и unused uploads удерживаются отдельно;
unknown entries перечисляются как unprepared. Source gallery order добавляется
после защищённых Linux defaults; aliases, дублирующие членство внутри одной
source gallery, отклоняются. Image/watermark gallery не принимает video.
Missing reference или bad payload отменяет весь результат без partial output.
Пустой runtime и явно deferred effect не заменяются другой сценой; basic banks
всё равно проверяются. Layout-review requirements proposals сохраняются.

Каждый prepared entry получает детерминированный **provisional** key по backup,
имени entry, normalized/encoded hash и типу. Эти keys не опубликованы. Private
PNG buffers и H.264 file owners находятся в `ScreenPlan`, не сериализуются;
Drop удаляет encoded staging. Existing/provisional key collision отклоняется.
Отдельные `existing_*_to_verify` sets перечисляют image/video/font references
для будущего actor pin/validation; наличие files, font metrics и датчиков на
целевой машине этим не доказано. Ctrl+C ждёт owned blocking worker и media
children. Опция конфликтует с отдельными image/media/editor modes.

[Evidence](evidence/2026-10-17-windows-screen-plan.json): owned complete archive,
all banks/gallery aliases, moving MP4 owner lifetime, missing/wrong-kind/duplicate
references, stale source/cancel, strict map admission и actual offline CLI.
Это coherent preparation plan, **не** immutable target grant, publication,
native editor persistence, полный Screen import или device Apply.

### Native basic editor storage

`LcdTheme.basic_editor` хранит выбранный background и отдельные banks для
ImageVideo/BackgroundColor. Каждый присутствующий bank содержит все семь
native overlay templates и свой selected overlay. В template запрещены вложенные
basic/effect caches и procedural effects; duplicate/incomplete banks reject.
Поле отсутствует в legacy serialization, если state не задан. Native active
layers/background остаются единственным runtime scene; inactive video не
включает streaming сам по себе.

`PreparedSlot::native_theme()` создаёт полноценный native named-slot candidate
с обоими подготовленными editors. Source selected effect вне basic возвращает
ошибку вместо подмены. Owned resource plan выдаёт `current_editor_theme` только
для index0 и `current_editor_deferred`; остальные `inactive_editor_indices`1..7
остаются в source inventory без runtime interpretation. `editor_banks.scope`
равен current_editor; --editor-banks отдельно проверяет all_source_entries.
Current runtime остаётся самостоятельной source scene. Автоматическое наложение editor cache на current
runtime и target commit этим шагом не реализованы.

`select_basic_editor` сначала сохраняет edits активного template, затем выбирает
другой background/overlay и переносит effect-page cache. Background image/video,
solid/gradient settings и Windows watermark общие для семи text variants внутри
одного background bank, как в ScreenSetting DTO reference ImageVideo:25-46 /
BackgroundColor:34-53. Text selection переносит эти общие edits во все варианты;
другой background bank остаётся независимым. Обычные native image layers без
watermark_position остаются локальными для template. Название темы не меняется
при выборе варианта. Native dependency
collectors учитывают каждый inactive image/video/font. Portable archive собирает
и remap-ит их как typed references, не меняя совпадающие sensor IDs/text. JSON
named slots и TOML current theme удерживают state. Existing1MiB manifest/settings
budgets сохраняются; число templates bounded: два banks по семь, без рекурсии.
Native image loading ограничивает total retained RGBA256MiB для unique active/
inactive assets; повторный key считается один раз, превышение не оставляет
partial session/live pins. Per-file decoder/normalized dimension limits прежние.
GUI draft resource pins обходят те же templates. При переходе Basic→effect
кэш остаётся в saved Basic page; effect root не получает basic editor state.

[Evidence](evidence/2026-10-17-native-basic-editor-storage.json): native DTO
validation/switching, Windows proposal→native banks, portable image/font/video
transfer в пустое хранилище с restart/TOML и browser Basic→Rain→Basic. Это native
storage/codec/dependency checkpoint. Выбор existing native banks через UI описан
ниже; Windows resource publication/current scene attachment и target-bound import
ещё открыты.

### Windows Screen native publication adapter

`ProfileArchives::prepare_windows_screen[_scoped]` соединяет owned source plan
с native `PreparedGroup`, без изменения settings и без Apply. Source runtime
становится active theme с Application control; current editor/history сохраняются
отдельно. Target Linux named profiles копируются без смены asset keys: Windows
array1..7 не становится библиотекой Linux. Source digest не включает target library,
поэтому её правки не меняют identity исходного импорта.

Все source payloads должны быть подготовлены. Backup/record fence, typed resource
coverage, normalized PNG digest/dimensions, encoded H.264 digest/frame metadata,
существующие dependencies и полная post-remap group проверяются до первой
публикации. Deferred runtime/editor, empty runtime и неизвестные uploads
отклоняются; подмена LocalUI не выполняется. Новые media переписываются потоково
по access units в native container; names берутся из source inventory как labels.
Существующие images/media удерживаются pins, fonts — existing session pin.
Только новые uploads получают generated keys; scoped вариант использует общий
Publication journal/rollback. Подготовка не является target revision grant.

[Software evidence](evidence/2026-10-17-windows-screen-publication.json) проверяет
раздельные runtime/editor, сохранение named library, native storage commit/restart,
portable re-export, actual moving clip units, cancellation/preflight rejection и
rollback. Это backend adapter: Windows source job/IPC/desktop chooser, explicit
preview/Apply/recovery ещё не подключены. Pixel/layout/font qualification и
hardware acceptance остаются открытыми.

### Windows Screen upload and explicit Finish

`group_upload` принимает `begin_windows_screen` с `id/size/expected` и source
options: record, `bindings_json`, `resources_json`. Explicit sensor/font bindings
и exact path/URI aliases разбираются до admission; новый ID требует актуальный
LCDV2 Screen snapshot. Формат и options неизменяемы для ID: native Begin и иные
record/bindings не могут использовать уже принятый Windows ID.

Существующие chunk, inspect, status, cancel и release работают с тем же bounded
registry, worker admission и idle cleanup. Windows Inspect готовит media и полный
Screen group в приватных numbered files; existing target dependencies удерживаются
pins. Каталоги, settings и named library до Finish не меняются. Новые uploads
публикуются только внутри existing full-group Finish с Publication journal и
serialized Screen owner. Перед публикацией private staging проверяется повторно.
Target CAS, durable receipt, readback/reapply и rollback используют native путь.

[Socket evidence](evidence/2026-10-17-windows-screen-upload.json) отделяет backend
от интерфейса: позднейший form/native checkpoint ниже подключает chooser/bindings/Apply; graphical import preview и расширенная fault matrix ещё не закрыты.

### Load imported editor into a draft

`get_imported_lcd_editor(dev)` читает current Screen snapshot через settings owner
и возвращает только native editor theme (или null), без Windows source/history и
без LCD command. Active theme возвращается прежним `get_lcd_theme`; эти документы
не подменяют друг друга. Сохранённые image/video/font refs остаются защищены
persistence reference scan.

В Pump screen есть отдельная кнопка **Load imported editor** / **Загрузить
импортированный редактор**. Она загружает тему и все basic banks в локальный
черновик, очищает выбранный standalone media draft и требует явный Apply. Поздний
ответ после смены устройства или новых правок не заменяет текущий черновик.
Null/error не очищают его. EN/RU browser tests проверяют этот путь; socket tests
проверяют независимость active/editor, отсутствие LCD generation change и чтение
после restart. [Evidence](evidence/2026-10-17-imported-editor-load.json).
На момент этого getter checkpoint native GUI acceptance ещё не выполнялась.
Позднейшие form/private-preview checkpoints ниже принимают кнопку, chooser,
bindings и import preview на owned sources.

### Desktop native Screen source job

`Groups` поддерживает отдельный `windows_screen` mode для Screen. Существующий
Windows ZIP chooser используется через native source grant; выбранный regular
file ограничен archive budget и копируется в unlinked private snapshot на
blocking worker с проверкой cancellation. После выбора оригинальный pathname
или его содержимое больше не используются. До подготовки native job хранит
только inspection/source identity и остаётся в Bindings.

Существующий `group_windows_prepare` принимает два непересекающихся strict
варианта options: прежние Windows control options и Screen source options
(`record/bindings_json/resources_json`). Screen upload берёт новый target snapshot,
использует BeginWindowsScreen и общий chunk/Inspect worker. Finish, recovery и
readback проходят через существующий full-group coordinator. До Finish каталоги
и Linux named library не меняются; после подготовки повторное изменение options
для этого ID запрещено. Для новой подготовки после ошибки нужен новый source ID.

Cancel в Bindings закрывает только локальный grant; Cancel prepared или shutdown
сначала отменяет и освобождает daemon upload, затем закрывает native grant.
Ошибку неподтверждённой очистки нельзя выдавать за успешную отмену.
[Job evidence](evidence/2026-10-17-native-screen-job.json) проверяет injected chooser,
actual Unix IPC/MockBackend, pathname/in-place mutation isolation, prepare/Finish/
recovery, отмену и shutdown, missing record и wire compatibility control options.
Это native job API checkpoint; frontend form и scoped native PNG workflow подключены в следующем checkpoint ниже.

### Screen binding requirements for the import form

`screen_requirements::inspect` возвращает требования текущей runtime scene,
существующих basic banks editor0 (до14 overlays) и трёх галерей. Source1..7 и
inactive procedural effects остаются opaque. Для absent selected basic bank
используются ровно те же scoped constructor defaults, что и при реальной
подготовке; absent inactive banks не создаются.

Runtime и editor проходят общие bounded parser/layout validators с реальной
конвертацией. Внутренняя inspection-ветка собирает имена roles/fonts/references;
служебные значения для проверки формы discarded, их theme/keys/IDs не выходят
из public inspection API. Реальная конвертация по-прежнему требует explicit
bindings. В результате нет назначений ресурсов и нет availability/Apply grant.
Exact reference → image-only flag объединяет background/watermark/gallery uses:
если хотя бы одно использование требует image, video choice недопустим.

Payload choices содержат entry/bytes и extension classification image/video/
unsupported, **не** подтверждение декодирования. Empty runtime default request,
unsupported runtime/current effect и unsupported payloads отмечены deferred;
ничего не заменяется LocalUI. Result≤512KiB, combined references≤256, paths≤32768
без control characters. Unknown/malformed current DTO отклоняется целиком.

Native `group_windows_prepare` принимает read-only options
`{record,inspect_only:true}` только для `windows_screen`/Screen. Он читает тот же
private file snapshot на blocking worker, возвращает status в Bindings с
`windows_screen_requirements`, не создаёт upload и не допускает Finish. Target
selection и cancellation ограждены существующим native job owner. После выбора
реальных bindings используется отдельная Screen preparation прежнего пути.
[Evidence](evidence/2026-10-17-screen-binding-requirements.json).
Frontend controls и native PNG acceptance подключены в checkpoint ниже; inquiry само по себе не является Apply grant.

### Windows Screen desktop form and native acceptance

Settings → Settings group backup → Screen → **Choose Windows Screen ZIP…**
запускает existing native chooser/private snapshot. Record выбирается явно;
**Read screen requirements** только анализирует источник. Форма требует theme
name, отдельные sensor IDs с ожидаемыми units, mapping каждого source font и
каждой exact resource reference. Fonts берутся из existing bundled choices или
известных app custom fonts; images/video — из source payload choices или после
явного refresh native catalog. Нет basename matching и автоматического выбора.

Неизвестный sensor ID можно ввести явно: он показывается unverified, без подмены.
Duplicate/wrong-unit IDs, missing font/reference и video для image-only use
блокируют preparation. Special reference names, включая `__proto__`, остаются
данными и сериализуются без prototype mutation. Deferred source блокирует импорт.

**Prepare screen import** запускает проверку/декодирование через native job.
Options удерживаются native owner и возвращаются как immutable review metadata;
после preparation форма read-only. Отдельное **Import screen and apply** заменяет
current LCD scene/editor/brightness/rotation/galleries; Linux named library и
cooling сохраняются.

**Show prepared frame** строит графический preview до Apply. Можно отдельно
выбрать текущую Windows scene или сохранённый current editor. RPC
`preview_windows_screen_upload {dev,id,source:scene|editor}` принимает только
privately prepared Screen operation. Renderer использует private PNG/H.264,
существующие managed assets/fonts, текущий sensor cache и source LCD rotation.
Для видео показывается первый кадр; непрерывный playback здесь не заявляется.
Preview не публикует файлы, не сохраняет настройки и не посылает команды LCD.
Apply блокируется на время запроса; смена source/target или Cancel отбрасывает
устаревший ответ. Ошибка preview сохраняет prepared operation и требует явного
Apply для импорта. Detached backend owner удерживает admission и private
resources до завершения renderer; Cancel/Finish используют тот же entry barrier.
Browser EN/RU и focused IPC/renderer проверки пройдены. Actual native EN/RU
на owned PNG проверяет оба кадра, сохранность active LCD/library, explicit Apply,
normal Close и restart. Медленный render сохраняется через повторные status
polls; stale/Cancel response отбрасывается.
[Private preview evidence](evidence/2026-10-17-windows-screen-private-preview.json).
Owned MP4/GIF + realtime CPU overlay также проверен через GUI EN/RU: private
scene/editor preview, no publication before Apply, explicit import, normal
Close/restart и cold receipt без ZIP/MP4. Полный H.264 поток из фактических
MockBackend writes декодирован в480×480 до/после restart. Короткая release
проверка имеет ≥20fps gate при target24; debug rasterization этого темпа не даёт.
Malformed MP4 также отклоняется native EN/RU при уже активной LCD теме:
no Apply, сохранены settings/draft/library/catalogs и emitted JPEG bytes;
existing ThemeEngine refresh продолжается, нового H.264 нет. Прямой socket
подтверждает release private upload, normal Close проходит.
[Native media matrix](evidence/2026-10-17-windows-screen-media-matrix.json).
Другие GUI source variations, expanded faults, genuine exports и physical acceptance
остаются открытыми.

[Form evidence](evidence/2026-10-17-windows-screen-form.json): browser EN/RU,
actual Tauri/WebKitGTK/private FileChooser/Unix IPC/MockBackend в EN/RU, owned PNG
source с независимым runtime и default current Color editor, six explicit roles,
source cancel, preflight without commit, one explicit import, preserved local
LCD draft/library, explicit imported-editor load и sensor preview без Apply,
square LCD, normal Close и GUI/daemon restart. ZIP удалён перед cold receipt
recovery. Проверка не включает GUI video-profile import, genuine Windows exports,
Windows pixel/font metrics, real portal sessions или physical hardware.

### Windows Screen array identity correction

**Index0 — current Full Screen editor, не Linux named slot.** Versioned source:
Galahad2VisionScreenPage:201-224 читает/меняет ScreenProfiles[0] и
ScreenEffectType[0]; MainPage:311,520 редактирует/показывает ScreenProfileName[0].
Profile.GetCurrentScreenProfile:675-684 читает только index0. Constructor:716-739
создаёт arrays длины8, но задаёт имя Full Screen только элементу0; остальные
names null, dictionaries пусты. Обращений LCDV2 UI к1..7 как named profiles
в этом reference не найдено; их vendor role дальше не угадываем.

Поэтому native resource plan интерпретирует только index0. Positions1..7 остаются
exact source JSON, даже когда их данные не соответствуют текущему runtime mapper,
и не требуют sensor/font/resource bindings. В research mode --editor-banks можно
явно запросить строгую preparation всех восьми source entries, но это не
assignment Linux slots. Owned rich fixture16banks/112overlays остаётся структурной
проверкой сохранения source data, не доказательством восьми Windows user presets.

Absent selected ImageVideo/BackgroundColor bank в index0 должен пройти
Import(string.Empty), как ScreenPage:201-211. Только он materializes constructor
defaults с source_absent_materialized=true; source inventory остаётся исходным.
Inactive absent banks и positions1..7 defaults не получают. Unsupported current
effect остаётся явно deferred, basic не подставляется.

Пустой ScreenConfigJson вызывает ApplyDefaultScreenConfigJson request
(Controller:260-266); это **не доказательство LocalUI**. Runtime остаётся deferred,
пока default-request renderer path не исследован. Непустой runtime также не
заменяется editor proposal, если они различаются.

[Evidence](evidence/2026-10-17-windows-screen-current-identity.json) supersedes
предположение о native per-slot candidates в прежних preparation checkpoints;
их JSON/hashes сохранены как история. Native Linux named slots и bank UI остаются
реализованными отдельными возможностями. Для Windows group import требуется
сохранить target-side Linux named library и source positions1..7 как bounded
private history, с отдельным active runtime/editor persistence до публикации.

### Independent imported Screen editor storage

`DeviceSettings.screen_editor` / `ScreenGroup.editor` сохраняют imported editor
отдельно от active `theme` / `media_clip`. `StoredScreenEditor` version1 содержит
optional native bank theme, private source_json и typed resource aliases.
None editor theme сохраняет deferred selected effect без подстановки basic.
Существующие get_lcd_theme/runtime replay продолжают читать только active scene.
Новый backend storage ещё не является UI командой загрузки imported editor.

`from_prepared` проверяет backup SHA и source_record_index, сохраняет exact raw
source inspection (включая positions1..7 и inactive effect JSON) в inert envelope.
History≤512KiB, references≤1280 с bounded keys; версия/envelope/fingerprint/native
bank theme проверяются. Это data representation, не immutable admission grant:
provisional keys требуют actor publication/remap перед commit. Source history
никогда не исполняется, не открывается как host path и не становится native slot
library. Existing1MiB settings/manifest budgets продолжают действовать.

Typed aliases разделяют archive:entry и reference:exact-path/URI. Они удерживают
также gallery-less uploads и выбранные existing resources. Cleanup scans,
firmware-handoff reference freeze и group dependencies учитывают image/video/font
refs editor theme и aliases. Произвольный текст/history, совпадающий с native key,
не считается ссылкой. Portable group переносит и remap-ит typed refs; исходный
source_json остаётся неизменным. Legacy отсутствие поля сериализуется прежним
образом. Full Screen reset очищает editor/history, FanAndLighting import сохраняет
их. Group revision включает editor, поэтому metadata edits меняют target token.

[Evidence](evidence/2026-10-17-imported-screen-editor-storage.json): source plan→
private editor/history, wrong backup/record fence, offline restart/JSON/TOML,
actual cleanup, editor-only image/font/video/alias transfer в пустое хранилище,
re-export, independent active scene, control-group preservation/full reset.
Actual settings/LCD actors commit/restart сохраняют editor video, но отправляют
JPEG текущей sensor scene, а не скрытого editor. Это storage/archive/ownership
integration; Windows ZIP group publication, separate read/load-to-draft UI и
source default-request renderer path остаются открытыми.

### Basic editor user workflow

Темы с `basic_editor` показывают в Composition два native select controls:
Background preset и Text preset. Они перечисляют только сохранённые варианты,
без генерации Windows defaults, угадывания sensor bindings или переназначения
ресурсов. Темы без banks используют прежний composition editor.

Выбор сохраняет current edits, независимый background bank и общее имя темы.
Background/gradient и source watermark разделяются всеми семью текстовыми
вариантами своего bank; ordinary native image layers сохраняются в отдельном
composition template. Невалидный/missing target отклоняется без изменения input.
Profile Load и переключение — локальные draft operations, Save записывает slot,
Preview не применяет к устройству; Apply остаётся отдельной кнопкой. Pending
Apply отключает выбор. Existing generation fence удаляет stale preview после
смены темы. Basic→effect→Basic сохраняет весь cache/dependency state.

Контролы, ошибки, layer headings и profile notices имеют EN/RU resources.
Новый блок использует текущие Device Studio tokens и native controls; layout
фона исправлен для1024px без horizontal overflow.

[Evidence](evidence/2026-10-17-basic-editor-ui.json): browser all2×7 selections,
independent custom text/shared gradient/title, profile Save/Load, explicit Apply,
busy/focus и missing/malformed target cases; EN/RU1280/1024. Isolated native
Tauri/WebKitGTK/Unix IPC/MockBackend проверяет14 renderer previews, Save/Apply,
normal Close и cold daemon+GUI restart в EN/RU. Это существующие native banks;
Windows ZIP→private upload/Finish и desktop source workflow подключены и scoped
native PNG/MP4/GIF acceptance пройдена. Expanded faults, reference qualification и
physical acceptance остаются отдельными gates.

Следующие обязательные шаги:

- [ ] Завершить basic ImageVideo/BackgroundColor mapper: runtime proposal уже
  и basic editor proposals уже сохраняют все overlays/gradient edits; ещё нужны
  Windows intrinsic grid layout, расширенная приёмка current/editor workflow и
  source-reference qualification без потери параметров.
  Эффекты остаются в конце
  очереди по решению пользователя.
- [x] Native storage/archive сохраняют current runtime/editor независимо, source
  positions1..7/inactive effects как bounded inert history и typed source resources.
- [x] Prepared Windows Screen подключён к existing storage/owner: target library,
  staged publication/commit, chooser/bindings/Apply/read/load-editor-to-draft и
  private graphical preview приняты на owned native PNG/MP4/GIF fixtures EN/RU.
- [x] Owned source plan подключён к private upload preparation, existing asset pins,
  full Screen group и staged owner transaction с rollback/receipt; socket
  commit/retry/restart/cancel/stale-target проверены. Desktop workflow остаётся ниже.
- [ ] Расширить native source variations, missing managed font/resource,
  source expiry и uncertain-result matrix без повторного импорта. Target-bound
  preview, explicit Apply, source grant и normal restart уже подключены и приняты.
- [ ] Проверить genuine Windows exports и одинаковые reference samples; физический
  вывод/переподключение оставить аппаратной приёмке позже.

## Что делает Windows

Versioned reference: `reference-decompiled/2.1.28-2026-09-08/L-Connect 3/LConnect3cs/`
в соседнем каталоге проекта. Пути ниже относительно этого reference.

| Источник | Наблюдение в source | Следствие для Linux |
| --- | --- | --- |
| `Utils/ProfileManager.cs:47-77,144-170,180-225` | ZIP с entry `backup`: Version, CreatedAt, Type, Datas; Controller record содержит MainType/SubType/Metadata/Data | Bounded ZIP/metadata inspector добавлен; Linux LCPROF01 не является Windows ZIP, semantic mapper/commit ещё нужны |
| `Products/Galahad2Vision/Galahad2VisionController.cs:191-200` | ExportProfile сериализует весь Galahad2VisionProfile для обоих типов | Совпадение названия типа не означает ограниченный экспорт одного слота |
| Тот же controller `:214-231,269-275` | FanAndLighting заменяет Fan/Pump, topology/sync, выбранные LED modes и все их profiles; отправляет четыре configuration requests, сохраняет, при ошибке возвращает предыдущий профиль | Нужны обе cooling группы, active и inactive RGB profiles, выбор Dynamic и последующий runtime reconciliation |
| Тот же controller `:232-267` | Screen заменяет brightness/rotation/current config, source arrays длины8 с names/effect types и три gallery lists; отправляет rotation/brightness/config и сохраняет с rollback | Групповой перенос должен включать текущую сцену и галереи, а не только named themes |
| `Utils/ProfileManager.cs:382-475,1628-1634` | Перед импортом WMSQ отправляется black screen; экспортируются все файлы uploaded directory, в том числе неиспользуемые текущей сценой | Собрать gallery-only и legacy uploads; подготовить staged assets до смены runtime |
| `Utils/ProfileManager.cs:1477-1496,1551-1588` | Controller сопоставляется по TypeID; Metadata device path сохраняется, но этот import loop не выбирает по нему устройство | Linux должен требовать явный target device, не переносить hidraw/runtime identity |

Это source-derived требования, не новый Windows runtime или hardware test.
Имена proprietary assets, их содержимое и Windows profile JSON в Linux repository
не копировались. Hash provenance reference включён в evidence.

## Windows ZIP: локальная проверка — 2026-10-17

`llcontrol-profile` реализует device-free container/metadata inspection для
Windows2.1.28.0. В рабочей release CLI доступно:

```sh
target/release/llcontrol --json profiles windows-inspect exported.zip
```

Daemon не нужен. Команда не распаковывает файлы, не публикует assets и не меняет
настройки. ZIP/ZIP64 preflight ограничивает central-directory size/count до
создания ZIP index; `backup` читается с лимитом и CRC. Unknown version/group,
небезопасные/дублирующиеся paths, encryption/special members, JSON duplicate keys,
malformed base64 и превышение бюджетов отклоняются. Стандартные Stored/Deflated
поддержаны; archive comments/trailing data/SFX исключены из исходно наблюдаемого
формата. Бюджеты и границы — в [README codec](../../../crates/llcontrol-profile/README.md).

`backup` содержит JSON с Version/CreatedAt/Type/Datas. Enum Type0=FanAndLighting,
1=Screen; MainType0=Controller; применимый SubType16846849=Galahad2LCDV2.
Data — default System.Text.Json base64 для byte[]. Оба типа экспортируют весь
JSON Galahad2VisionProfile, а набор переносимых полей определяется ImportProfile.
Инспектор показывает record indices, profile field names и SHA-256; Metadata
Windows device path и пользовательские profile values не выводятся. Несколько
LCDV2 records сохраняются отдельно; автоматического выбора target нет.

Это ещё **не импорт**: `inspection_only=true`, `import_supported=false`,
`asset_payloads_validated=false`. JSON object shape не доказывает корректность
RPM/Color/effect/font/sensor fields; payloads других семейств и ZIP assets не
декодируются. Нужны actual Windows exports, typed mapper выбранной группы,
полная asset validation/remap и target-bound atomic commit через существующий
owner/journal. Неподдержанный nonempty cached effect нельзя молча потерять.

Own fixtures созданы стандартными .NET JSON/ZipArchive с synthetic DTOs/values;
vendor code/assets не использованы. Это BCL serialization/container evidence,
не запуск Windows L-Connect.9 focused Rust/CLI cases, общий workspace и release
проверены в [evidence](evidence/2026-10-17-windows-profile-inspection.json).
Текущий пакет `distribution-20261017-collections` предшествует этой CLI-команде.

Остаток реализации interoperability:

1. FanAndLighting: RPM policy preparation ниже уже реализован для описанного
   подмножества; нужна lossless representation оставшихся curves. Reset8/3,
   Save и selected-mode identity реализованы в [cooling modes](cooling-modes.md).
   Полные RGB banks/thresholds/palettes/topology теперь декодируются offline
   [ниже](#windows-lighting-bank-inspection--2026-10-17), typed Linux preparation
   также реализована. Full FanAndLighting composition/commit с provenance
   и target-side Linux slots подключён выше; остаются неподдержанные cases/GUI.
   Соседние globals и Screen остаются target-side.
2. Screen: current config/editor index0 и исходные arrays длины8 с names/cache,
   gallery, fonts/effects/geometry и
   WMSQ media paths; неизвестное содержимое диагностировать до публикации.
3. Все assets полностью проверить/перекодировать, включая unused uploads;
   не использовать абсолютные Windows paths и hardware identity для Linux IO.
4. Подготовить Linux GroupProfile в private quarantine, затем existing
   Inspect→explicit Apply/CAS/journal/readback/reapply. Для FanAndLighting этот
   путь уже подключён выше. Genuine Windows ZIP fixtures и Screen round-trip
   остаются обязательными для расширения заявленной interoperability.

### Windows lighting bank inspection — 2026-10-17

`llcontrol --json profiles windows-lighting FILE --record INDEX` работает
локально, без daemon. Использует тот же bounded ZIP2.1.28 reader и explicit
record selection, что и cooling preparation. Перенос private Windows Metadata
в report или Linux target не выполняется. Все root fields вне7 lighting fields
перечисляются как unconverted; Screen и Fan/Pump RPM не теряются молча.

Сверено с frozen `Products/Galahad2Vision/Galahad2VisionProfile.cs:88-137,373-673`,
`Galahad2VisionFanLightingSetting.cs`, `Galahad2VisionScreenLEDLightingSetting.cs`,
`Galahad2VisionScreenLEDLightingSubSetting.cs`, `Galahad2VisionScreenLEDSensorSetting.cs`,
Core enums FanLightingMode/PumpLightingMode/SensorType/LightingDirection/FanLEDSize,
`Utils/JsonConverters/ColorToIntArrayJsonConverter.cs:17-24` и controller
`getFanLEDNumberOfLED():385-397`.

Report хранит6 fan и16 pump entries с выбранными IDs, Static/Dynamic selection,
всеми6 sensor entries на каждый pump mode и192 High/Low parameter sets, а также
16 Static и6 fan parameter sets. Проверяются все inactive записи: missing/unknown
mode/sensor, mode-key mismatch, палитры, RGB triplets, направления и диапазоны.
Threshold equality разрешена Windows UI (ScreenLEDPage:452-458) и сохраняется;
Linux evaluator для этого случая ещё предстоит определить. UnitText сохраняется
как presentation, Linux source binding не угадывается. int.MinValue сохраняется
как sentinel, не превращается в придуманное значение slider.

Topology хранит оба значения: manual loop count и size. При sync=false controller
использует loop count, при sync=true —16/24 для280/360. Non-byte count отклоняется;
Linux mapper должен дополнительно проверить текущий supported8..50.

Новая typed inspection — вход для полного mapper, не готовая Linux группа.
`group_import_supported=false`, `sensor_binding_verified=false`; ни asset
publication, ни Apply нет. Windows CPU/GPU/Pump/Coolant roles требуют явных Linux
bindings/units; CoolantTemp нельзя автоматически считать доступным датчиком.
.NET10.0.12 owned fixture подтверждает enum dictionary serialization и ZIP/JSON,
не genuine Windows export/WPF run. [Evidence](evidence/2026-10-17-windows-lighting-inspection.json).

### Windows lighting Linux preparation — 2026-10-17

`profiles windows-lighting FILE --record INDEX --bindings BINDINGS.json`
добавляет typed Linux preparation к source report. Формат bindings описан в
[README библиотеки](../../../crates/llcontrol-profile/README.md#typed-linux-lighting-preparation).
Все6 source roles требуют явных разных ID и unit; duplicate keys, wrong/unknown
roles, control-containing IDs и oversized maps отклоняются до выдачи результата.
Host availability не проверяется, source Metadata не становится Linux target.

Получаются6 fan и16 pump LightingConfig, global topology, все96 bindings в
DynamicRgbProfiles и optional active DynamicRgbConfig. Для каждого режима
selected source идёт последним, enabled хранится независимо; static base не
заменяется dynamic endpoint. Для активного Static dynamic_rgb=None, даже когда
другие сохранённые modes включают Dynamic. Full source report сохраняет Low
brightness/direction/ranges и исходные sentinels, которые runtime не использует.

Source service `Galahad2VisionController.cs:498-598` выбирает яркость/направление
из High, interpolates HSV colors и speed по Low/High. Existing Linux evaluator
имеет эту семантику; CLI-generated config проверен на actual evaluator с low/
midpoint/high значениями. Equal thresholds представляются step без деления на0.
Linux-unrepresentable dynamic speed sentinels/directions/topology/thresholds
отклоняются, не заменяются guessed defaults. min/max chart/source ranges сохранены
в source report; при валидных пределах вокруг Low/High их clamp не меняет
endpoint/interior color/speed результата.

Library использует shared IPC data types; daemon/HID не подключаются.
Report содержит preparation_only=true, group_import_supported=false,
apply_supported=false, sensor_binding_verified=false. Полный FanAndLighting group importer выше уже соединяет результат с RPM banks,
сохраняет provenance/target Linux extension slots и использует existing coordinator.
Отдельная preparation-команда сама не выполняет этот commit. GUI и аппаратная приёмка этим срезом не заявляются.
[Evidence](evidence/2026-10-17-windows-lighting-preparation.json).

### RPM: source contract

По `Products/RPMSetting.cs`, `RPMProfile.cs`, `RPMPhaseKey.cs`,
`RPMPhaseInfoDictionaryJsonConverter.cs` и `RPMPhaseInfo.cs` в frozen2.1.28:

- Выбранный `Mode` и все `Profiles` сохраняются отдельно; перенос только текущей
  кривой теряет пользовательские профили. `Name` сохраняется, `DisplayName`
  исключён через JsonIgnore.
- `PhaseInfos` имеет собственный JSON converter. Его ключ — строка
  `FanSize|ReferenceSource|ControlType` с именами enum, а не числовой tuple.
  Нельзя интерпретировать его обычным serializer enum dictionary.
- `SpeedUnit`, `RPMReferenceSource`, `IsStartStop` и `ControlType` меняют
  семантику значений. Pump RPM нельзя считать процентами PWM; Linux binding
  к CPU/GPU источнику выбирается явно, capability-флаги не должны расширять
  возможности выбранного Linux устройства.
- `ToRPMConfigPhases` добавляет endpoint при curve: начальный `0/MinSpeed`
  при выключенном StartStop и конечный `100/MaxSpeed`; при StartStop добавляет
  нулевую скорость на температуре первой фазы. Для single-phase применяется
  отдельная ветка. Mapper должен проверить эти случаи по одинаковым входам,
  а не сравнить только массив сохранённых точек.

Эти source-derived требования использованы в preparation ниже; полного
FanAndLighting mapper/commit они не заменяют.

### Windows cooling policy preparation — 2026-10-17

```sh
target/release/llcontrol --json profiles windows-cooling exported.zip --record INDEX --cpu-sensor SENSOR_ID
```

Требуются явные zero-based record index и Linux source binding; доступность и
роль датчика offline не подтверждаются. Команда читает тот же bounded ZIP,
проверяет все8 fan/3 pump modes и переводит их в Linux policies. Выбранный mode,
names, все inactive edited built-ins и исходные RPM settings/chart metadata
сохраняются в report. Четыре named fan slots не используются как замена восьми
Windows modes. Motherboard global sync не выводится из capability-флага и не
переносится как часть RPM policy.

Политики проверяются существующим CoolingPolicy validator и actual Linux
evaluator в executable CLI tests: integer Celsius/PWM, RPM conversion и missing
source fallback. Own .NET fixture использует enum-key dictionary serialization;
неверная запись/группа, отсутствующий mode или invalid inactive profile, units,
phase keys, source и bounds блокируют весь report. Generic inspector продолжает
выводить только metadata; новая explicit preparation показывает user names и
settings, но исключает Windows device Metadata.

`policy_conversion_only=true`, `group_import_supported=false`,
`apply_supported=false`, `sensor_binding_verified=false`,
`source_presentation_metadata_validated=false`; никакого daemon/HID/config write.
RGB/Screen fields только перечисляются как unconverted, assets не валидируются.
Непредставимые текущей Linux policy curves не исправляются молча: duplicate или
endpoint knots, decreasing speeds, StartStop, non-CPU reference и oversized names
отклоняются. Точные bounds и CLI defaults — в [README](../../../crates/llcontrol-profile/README.md#cooling-policies-from-a-selected-record).
Chart metadata хранится без семантической квалификации. Для расширения Windows interoperability
ещё нужны расширение curve representation, Screen/assets mapping, GUI и genuine
Windows exports. Fresh-store banks и target-bound FanAndLighting commit уже
реализованы в более новых checkpoints выше.
[Evidence и команды](evidence/2026-10-17-windows-cooling-preparation.json).
Текущий collections package предшествует этой команде; GUI не подключён.
