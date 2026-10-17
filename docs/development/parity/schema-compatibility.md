# Совместимость сохранённых данных

Политика для текущей версии Linux, 2026-10-10. Windows archive format сюда
не входит: его совместимость исследуется отдельно.

| Данные | Поддержка сейчас | При несовместимости |
|---|---|---|
| Device TOML `schema_version=1` | Чтение и атомарная запись | Ошибка, исходный файл не заменяется |
| Legacy device TOML без `schema_version` | Читается как v1; отсутствующие известные секции получают defaults | Чтение не переписывает файл; нормализация только при следующем явном сохранении |
| LCD theme `version=1` | Обязательные поля и известные optional defaults | Неизвестная версия/поле или invalid geometry отклоняются |
| Profile library `version=1` | Слоты и optimistic revision; embedded themes проходят validation | List/edit возвращают ошибку; файл и revision сохраняются |

Новые optional поля в рамках v1 допустимы только с defaults, сохраняющими
прежнее поведение: например Sans/opacity=100/no-shadow и legacy clock formatting.
Изменение смысла существующего поля или обязательных данных требует новой
версии. Старый бинарник может не прочитать новые optional поля: неизвестные поля
отклоняются, а не удаляются при последующей сериализации. Downgrade без потери
данных важнее молчаливого сброса к defaults.

## Граница миграции

Поддерживаемой v2 сейчас нет, поэтому преобразование v2→v1 не выдумывается.
Единственная текущая нормализация — legacy device TOML без версии и defaults
для отсутствующих полей v1. Theme и profile версии должны быть явными.

При появлении новой версии её implementation plan должен включать:

1. Fixtures прежнего формата с themes, bindings, profiles и asset references.
2. Чистое преобразование в памяти с validation результата, без изменения sensor
   identity и без автоматического Apply на устройство.
3. Сохранение исходных bytes в отдельной резервной копии до замены файла;
   атомарный commit нового формата и восстановление после ошибки/перезапуска.
4. Повторный запуск миграции без повторного преобразования уже мигрированных данных.
5. Проверку missing assets/sensors, readonly/disk failure и отказа downgrade.

Это требования к будущей миграции, не заявление о наличии migration engine.

## Отказ и восстановление

Per-device read допускает только обычный файл до 1 MiB. Неподдерживаемая схема,
неизвестные поля и невалидная embedded theme возвращают ошибку. Manager не
создаёт actor для устройства с таким файлом: replay defaults не происходит.
Ошибка attach записывается в daemon log; отдельного GUI repair editor пока нет.

Перед каждой записью store повторно читает существующий файл. Если он стал
несовместимым или повреждённым после attach, запись отказывается его заменять;
persistence status показывает ошибку, retry не стирает исходник. Это не compare-
and-swap с произвольным внешним редактором: внешнюю правку одновременно с daemon
не следует выполнять. Live Apply и запись на диск — разные результаты.

Для восстановления остановить daemon, сохранить копию проблемного файла и
использовать совместимую версию программы либо восстановить заведомо корректную
копию. Намеренный сброс — отдельное действие пользователя после резервного
копирования; автоматического удаления или понижения версии нет. Assets не
удалять: conservative cleanup также отказывается от несовместимых корней.

Проверки: `crates/lconnect-daemon/tests/persistence.rs`, `tests/profiles.rs`,
`crates/lconnect-ipc/tests/theme.rs`; theme IPC и persistence writer regressions
проверяют сохранение рабочего состояния и обработку ошибок. Общая итоговая
software приёмка ведётся в [checklist](basic-parity-checklist.md).


Volts/Hertz добавлены как варианты typed sensor unit без изменения envelope v1.
Старый binary не сможет прочитать тему с `volts`/`hertz`; такая тема не считается
совместимой с downgrade. Существующая strict rejection сохраняет файл и
останавливает replay, вместо подмены настроек defaults. Старые units читаются
без изменения; миграция сохранённых температурных тем не требуется.

`bytes_per_second` — ещё один новый вариант unit v1, с теми же downgrade
ограничениями: старый binary отклоняет такую тему без перезаписи; новая версия
читает старые units без миграции.


## Sensor defaults contract

`SensorDefaults` — отдельная version1 schema (revision, CPU sources и optional
GPU с physical device_id). Version/revision/cpu обязательны; optional bindings
могут отсутствовать либо передаваться null (не выбран источник). Unknown
fields/version и invalid IDs отклоняются. Контракт поддержан `SensorDefaultsStore` и production get/set RPC. Read-only
запрос и запуск daemon не создают файл; первая успешная запись создаёт его.
Migration не заявлена.
Загрузка допускает отсутствующее железо; разрешение reading требует exact ID,
соответствующий unit, valid finite value и age≤3s. Duplicate ID неоднозначен.
Provider membership должен проверяться при Apply в daemon: unit сам по себе
не доказывает принадлежность reading выбранной CPU/GPU. Explicit theme/policy
bindings остаются самостоятельными и не переписываются этим resolver.


### Sensor defaults storage

`SensorDefaultsStore` читает `sensor-defaults.json` из явно переданного root.
Только отсутствующий файл даёт пустые defaults; load ничего не записывает.
Regular-file/64KiB bounds, strict schema и version проверяются и перед заменой.
Corrupt/future/unknown-field files не заменяются пустыми defaults. Файл-symlink
отклоняется; root должен находиться в закрытом settings directory daemon,
защита от конкурентной подмены файлов недоверенным local writer не заявлена.

Save принимает revision прочитанного snapshot, под общим для clones mutex
перечитывает текущий файл и увеличивает revision с checked overflow. Запись:
При каждой записи root создаётся при необходимости, затем синхронизируется
вся цепочка его ancestors. Это также завершает синхронизацию directories после
неудачной первой попытки: существование каталога не доказывает durable entry. Затем temporary file в том же directory → file fsync →
rename → directory fsync.
Ошибка после rename может означать сохранённые bytes: клиент должен перечитать
состояние вместо auto-retry. Отменённый RPC не должен освобождать lock выполняемой
blocking операции; production интеграция должна держать один store и передавать
его clones. Независимые store instances не являются multi-process writer lock.

Слой хранения проверяет структуру, но не принадлежность sensor выбранному GPU.
Mutation RPC выполняет эту проверку через server-owned candidate registry. Старые bindings
можно читать при отключённом устройстве; unavailable не стирает сохранённый ID.


### Sensor defaults RPC

`get_sensor_defaults {}` возвращает config/revision, server-classified candidates
и 13 role values. У каждого value сохранён sensor_id; reading=null при missing,
stale, invalid, ambiguous, wrong unit/role/GPU либо отсутствии выбора. Catalog
читается после disk load, чтобы блокировка I/O не выдавала старый snapshot за fresh.
Ручная правка файла с wrong-GPU binding не заставляет read RPC выдавать чужое значение.

`set_sensor_defaults {config}` принимает только config со старой revision,
проверяет schema и membership на server cache; неизвестные параметры запроса
отклоняются. Store повторно проверяет revision под общим mutex после validation.
Write/read идут в spawn_blocking с существующим лимитом disk/theme jobs; owner
permit остаётся у blocking worker после отмены RPC. Config сохраняется отдельно
от theme/cooling/RGB и не переписывает explicit bindings. Readback обязателен после
ambiguous error; старый запрос повторно не применяется поверх новой revision.

Desktop Sensors подключён через allowlisted get/set RPC. Очистка выбора меняет
только черновик; запись выполняет explicit Apply. `get_sensor_defaults` также
возвращает `recommended`: свежие допустимые источники из текущего каталога,
actual clocks предпочтительнее requested, одна GPU по стабильному порядку ID.
Кнопка рекомендуемого выбора переносит предложение в черновик. Polling не
переписывает сохранённые bindings. Это Linux policy, а не идентичность Windows
Reset All; browser/native reset workflow прошёл [приёмку](evidence/2026-10-10-recommended-sensor-defaults.json).
Отдельного eager-reset RPC нет.


## Gradient backgrounds

Optional `background_fill` сохраняет `mode` (solid/horizontal/vertical/radial)
и три независимых настройки `horizontal`, `vertical`, `radial`. Каждая содержит
ровно две RGB-тройки `colors` и boolean `flip`; все поля вложенного объекта
обязательны, неизвестные поля/enum, неверная длина массива и значения вне u8
отклоняются. Это bounded typed schema без client paths и новых assets.

Отсутствующее/null поле сохраняет прежний `background: [r,g,b]`; None не
сериализуется. Режим solid также использует прежний RGB, сохраняя gradient
настройки для возврата. Старый binary отклонит тему с новым полем без перезаписи
(обычная v1 downgrade policy), но темы без gradient остаются совместимыми.
Preview, daemon frames, profiles и persistence используют одну LcdTheme.


## Text line boxes

Optional boolean `TextStyle.line_box` по умолчанию false и не сериализуется при
false. True считает высоту блока как N×line_height и распределяет разность
line_height−font_height поровну над/под glyph metrics; отрицательное значение
разности допустимо. Без явного line_height используется прежний font height+
line gap. Все text/clock/sensor parts используют один TextStyle/render path.

False сохраняет прежнюю высоту font_height+(N−1)×line_height. Unknown/неboolean
значения отклоняются. Это additive v1 с обычной strict downgrade policy:
старый binary отклоняет тему с новым полем без удаления данных.


## Noto Sans TC choices

`TextStyle.font` дополнен `noto_sans_tc_regular` и `noto_sans_tc_bold`. Старые
sans/serif/mono и absent→sans не меняются. Как и для других новых enum values,
старый binary отклоняет незнакомый font без перезаписи темы. Полные static
fonts включены в renderer; пользовательские пути и внешняя font installation
не нужны. Provenance/OFL — в `crates/lconnect-image/assets/NotoSansTC-*`.


## User font assets, chunked RPC и theme references

`lconnect_image::fonts::validate_font` принимает одиночные TTF/OTF до16MiB,
используя parser renderer. TTC/WOFF/WOFF2 не принимаются; corrupt/invalid
metrics отклоняются. Variable font пока означает default instance, без выбора
осей. Bytes не нормализуются и не subset: copyright/license и glyph data
сохраняются. Проверка файла не является проверкой права пользователя на шрифт.

`ThemeFontStore` хранит оригинал как generated32-lower-hex`.font` в отдельном
private root. Temp file→file fsync→no-clobber persist→directory fsync; ancestors
синхронизируются и после retry создания root. Load принимает только regular file
и снова проверяет лимит/parser. Missing/corrupt не заменяются bundled font;
path/symlink/directory inputs отклоняются. Защита от adversarial replacement
внутри writable shared directory не заявлена; root должен принадлежать daemon user.

Production `font_upload` RPC принимает `{request: {action, ...}}`: `begin(size)`,
`chunk(id, offset, bytes)`, `status(id)`, `finish(id)`, `cancel(id)`.
Лимиты: font1..16MiB, chunk1..64KiB, до4 сессий на server instance, idle300s.
Неизвестные request fields отклоняются. Новые bytes принимаются строго с
`offset == received`; точный повтор уже принятого диапазона допустим, изменение
bytes, gap, overlap с новыми bytes и overflow отклоняются.

Finish валидирует и сохраняет файл в `theme-fonts` persistence root. Повторный
Finish живой сессии возвращает тот же asset key; retry после rename проверяет
существующие bytes и повторяет fsync без создания второго файла. Cancel освобождает
сессию, но не удаляет уже сохранённый asset. Завершённые сессии занимают слот до
cancel/expiry, чтобы поддерживать повтор Finish. Idle cleanup выполняется при
следующем RPC запросе, а не background timer. Сессии не переживают daemon restart;
сохранённые файлы переживают. Retry гарантия ограничена живой сессией.

Большой CJK font прошёл production dispatch через memory transport с MockBackend
и побайтовой проверкой файла. Глобальный8MiB frame limit не увеличен, repertoire
не сокращён. [Evidence](evidence/2026-10-10-font-upload.json).
`TextStyle.font_asset` — optional managed32-lower-hex key, включая отдельные
sensor label/unit styles. При наличии он имеет приоритет над bundled `font`;
при отсутствии JSON и прежний raster не меняются. Неверный key отклоняется
validation; missing/corrupt файл не заменяется bundled fallback. Старый binary
с deny_unknown_fields отклонит новое поле, сохранив исходный файл.

Preview, Apply, profile save и startup replay загружают уникальные font keys
через ThemeFontStore до установки runtime. Общий лимит source bytes64MiB на тему,
каждый файл до16MiB; повторные ссылки дедуплицируются. Parsed FontArc принадлежит
runtime: чтения файлов на кадр нет, исчезновение файла не прерывает уже подготовленный
экран. Новая подготовка и restart явно завершаются ошибкой; restore сохраняет
repairable theme intent. Это не обещание автоматического reload изменённого файла.

Полный CJK upload→preview→profile проверен production RPC; managed rendering
совпадает с тем же bundled font для text/clock/sensor/cycle и отдельных частей.
[Evidence](evidence/2026-10-10-theme-fonts.json). GUI выбора/import подключён: File→64KiB chunks, explicit selection и Apply.
Если Finish потерял ответ, GUI читает Status того же id; новый upload
автоматически не создаётся. Отмена/disconnect/unmount запрещают позднее
добавление шрифта в список; сессия освобождается Cancel best effort. Потерянный
Begin без id очищается по idle expiry. Имена imported файлов живут в текущем
GUI session, после reload у managed key показана короткая подпись; сам key
сохраняется в теме/профиле. `cleanup_theme_fonts` — отдельный RPC с `{delete:false, keep:[]}` по умолчанию;
ответ использует AssetCleanupReport. `keep` ограничен1024 managed keys.
Перед scan/delete перечитываются все device settings (включая offline) и
screen profiles; corrupt/unreadable/unknown schema блокируют удаление.

PersistenceStore владеет общим ThemeFontStore: успешные load и import pin keys
до restart; pin import устанавливается до записи и сохраняется даже при
неоднозначной ошибке fsync. Import/load и scan/delete сериализованы одним mutex.
Поэтому accepted/prepared jobs и debounced settings не теряют файл после
устаревшего сканирования references. Повторно созданный PersistenceStore
означает новую сессию; production manager/IPC используют clones общего store.

Cleanup сначала полностью сканирует каталог и только затем удаляет candidates.
Подходят только regular32-lower-hex`.font`; symlinks/directories/unknown filenames
пропускаются, scan ограничен10000 entries. Ошибки unlink возвращаются в failures.
Referenced/session/draft keys сохраняются. GUI передаёт keys текущей темы и
списка импортов, требует scan перед delete и отбрасывает report при изменении
keys/connection. Image cleanup по-прежнему не затрагивает font root.
[Checkpoint](evidence/2026-10-10-font-cleanup.json).

## Отклонение IPC-запросов при активной теме — 2026-10-11

`crates/lconnect-daemon/tests/theme_ipc.rs` проверяет настоящий JSON-RPC
server/dispatcher с MockBackend и активной sensor-cycle темой:

- future version, unknown theme field, RGB component256, negative font size;
- traversal/missing font reference и повреждённый font file;
- traversal/missing image asset, zero image width и некорректный image upload;
- malformed JSON получает parse error −32700;
- строка без newline длиннее8MiB завершает только своё соединение с InvalidData.

После отвергнутых запросов сохранены theme intent и generation producer;
второе клиентское соединение продолжает читать состояние. Существующее
завершение этого же теста проверяет flush сохранённой темы и manager restart.
Framing проверяется через in-memory duplex, использующий тот же connection
handler, что Unix listener; это не проверка socket permissions или физического
LCD. Дополнительные schema/bounds cases — IPC tests `theme` и `bounded_input`.

## Prepared media selection — 2026-10-11

Device v1 допускает optional media_clip (default None, отсутствует при записи
None):32lowercase hex managed key. Одновременные theme и media_clip отклоняются.
Существующие файлы без поля сохраняют прежнее поведение. Media/theme deltas
взаимно очищают альтернативу; named profile library расширена ниже.
Старые readers с deny_unknown_fields не поддерживают файл с media_clip; не
следует считать это обратной совместимостью с более старым executable.

## Media screen slots — 2026-10-12

Profile library v1 поддерживает `content: {kind: "screen_media", value: KEY}`,
где KEY —32lowercase hex prepared clip key. Это группа screen и те же слоты0–7,
а не ещё восемь слотов: theme и media с одинаковым slot конфликтуют; Save
заменяет содержимое этого слота, Reset создаёт прежнюю базовую тему. Fan/pump
не меняются. Старые v1 libraries с themes читаются без миграции и перезаписи.

Старый executable с enum без screen_media не прочитает такую библиотеку и
не должен подменять её пустой. Это расширение читаемых данных новым бинарником,
не обещание чтения нового variant старым бинарником. Archive interoperability
с Windows остаётся отдельной задачей.

RPC Save валидирует наличие/содержимое ролика и удерживает MediaPin до конца
атомарной публикации profiles.json. Delete media читает всю библиотеку профилей
строго; corrupt/unknown library запрещает unlink. Rename/Delete/Reset работают
по group=screen. Клиент загружает snapshot, а Apply вызывает set_lcd_media;
отсутствующий asset не создаёт ложного applied state.

## Имена изображений — 2026-10-12

PNG и ключи theme/profile references не меняются. Необязательный sidecar
`<key>.name.json` содержит `version:1`, положительную `revision` и `name`.
Отсутствующий sidecar означает legacy asset без имени, revision0; UI показывает
короткий ключ. Новые GUI imports передают имя файла; старый IPC import без
`name` продолжает работать. Legacy `list_theme_assets` по-прежнему возвращает ключи;
новый `list_named_theme_assets` возвращает metadata. Новый GUI требует этих RPC.

Rename требует совпадающую `expected_revision`, сериализован с import/cleanup
и атомарно заменяет только sidecar с file/directory fsync. Имена ограничены256
UTF-8 bytes, не могут быть пустыми/содержать control characters. Unknown version,
corrupt/oversize/symlink metadata отклоняются без перезаписи. Ошибка fsync/потеря
ответа не доказывает отсутствие commit: GUI перечитывает библиотеку без autoretry.
Cleanup изображения также удаляет sidecar; сбой удаления metadata виден отдельно.
Старый daemon игнорирует sidecars; его cleanup может оставить orphan metadata.
Это не совместимость с Windows archive и не миграция содержимого PNG.


## Persistent media v2 store — 2026-10-12

Старые `<key>.json`v1 не мигрируют автоматически. Новый `<key>.lcm` использует
[version2 container](persistent-media-v2.md); list/open/delete распознают оба
формата в общем namespace/quota. Если оба extension имеют один key, операции
отклоняют неоднозначность, не выбирают и не удаляют одну запись автоматически.
Profile `screen_media` хранит тот же key, формат контейнера не меняет schema профиля.
Legacy `load_prepared_clip` не возвращает v2 в byte-array ответе: ему необходим
streaming consumer. До подключения actor/import v2 не объявляется пользовательской
поддержкой длинных видео. Старый binary не умеет читать/воспроизводить `.lcm`.

## Media upload RPC — 2026-10-12

Добавлен отдельный `media_upload` с strict tagged request и состоянием
receiving/published/cancelled/failed. Старые diagnostic manifest и
`load_prepared_clip` не расширяются. Новый случайный32hex upload id становится
asset key; опубликованный v2 индекс позволяет восстановить receipt после
перезапуска без отдельной mutable receipt schema. До commit сессия volatile.
Повторный Begin требует совпадения name/frames/bytes; Chunk поддерживает только
точный retry последнего принятого AU. Finish/Cancel после commit не создают
копии и не удаляют asset. Failed state требует проверки клиентом даже при
успешном JSON-RPC response. Полный [контракт](persistent-media-v2.md).

## Язык display settings, 2026-10-15

Новая установка без `display-settings.json` получает en-US. Существующий v1
record без `interface_language` продолжает означать ru-RU: раньше serializer
мог опускать даже явно выбранный русский. `Deserialize` сохраняет это legacy
значение отдельно от конструктора нового `DisplaySettings::default()` (English).
Новые записи явно сериализуют обе локали; нормализация выполняется только при
явном Save с обычной CAS revision. Чтение не переписывает файл и не меняет unit.
GUI без допустимого cache использует English; допустимый ru-RU cache сохраняет
выбор при offline startup. [Языковой контракт](interface-language.md).
