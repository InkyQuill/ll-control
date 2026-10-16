# Gallery defaults и Screen reset: Windows 2.1.28.0

Сверено 2026-10-12 по локальной декомпиляции и ресурсам Windows baseline.
[Evidence с SHA-256 и ffprobe](evidence/2026-10-12-gallery-defaults-reference.json).
Это source-derived поведение и измерение файлов; Windows GUI и LCD не запускались.
Proprietary assets остаются в reference, в Linux repository не копировались.

## Применимые сценарии

Пути ниже относительно `L-Connect 3/LConnect3cs` в reference.

| Сценарий | Источник | Требование |
|---|---|---|
| Отдельные коллекции | `Products/Galahad2Vision/Galahad2VisionProfile.cs:55–59` | Image, image/video и watermark имеют отдельные списки. Единая глобальная библиотека не является требованием Windows |
| ImageVideo background | `.../Galahad2VisionScreenSettingPage_ImageVideo.cs:1049,1093` | Background выбирается из image/video collection; watermark из своей. Нужны оба контекста выбора, не обязательно буквальное копирование структуры списков |
| Начальный background | `...ImageVideo.cs:296–304`; `...Controller.cs:278–299` | Пустая настройка ImageVideo выбирает default video с prefix `a_default_`; image getter выбирает `b_default_` |
| Состав defaults | `...Controller.cs:700–709,729–753` | ImageVideo: logo+video; image: logo+image; watermark: logo. Порядок default entries задаётся индексами0/1 |
| Защита defaults | `Views/UserControls/WMSQ/Gallery.cs:491,731,1058–1060` | `IsDefaultFile` запрещает удаление и editor. GIF также не получает повторный editor. Запрет rename здесь не доказан |
| Полный Screen reset | `...Controller.cs:203–267`; `...Profile.cs:717–738` | Заменяет настройки экрана, три списка и восемь слотов; brightness50, rotation0, пустой config→ApplyDefaultScreenConfigJson. Это не reset одного слота и не добавление отсутствующих defaults |
| Save failure reset | `...Controller.cs:246–251` | Возвращает предыдущий profile и повторно отправляет screen requests. Не подтверждает атомарность физического устройства |

`setDefaultFile` проверяет prefix/extension у заданного индекса перед вставкой;
это не проверка content hash. `RemoveNotExistsFileInfos` удаляет отсутствующие
ссылки из списков. В исследованном reset-пути нет удаления пользовательских
media-файлов с диска; замена списка не равна физическому удалению файлов.

## Измеренные ресурсы

| Ресурс | Данные |
|---|---|
| `assets/hydroshiftlcd/default_video.mp4` | VP9,480×480,yuv420p,24fps,586frames,24.416667s,754223bytes |
| `assets/hydroshiftlcd/default_image.png` | PNG480×480,571364bytes |
| `assets/galahad2vision/screen/lcd_logo.png` | PNG480×480,39577bytes |

Наличие VP9 внутри MP4 не означает VP9 по USB: Linux также должен декодировать
input и выдавать исследованный H.264 transport. Формат контейнера нельзя
приравнивать к input codec. Первоначальный diagnostic путь ограничен10s/240AU/1MiB.
Persistent media v2 уже отделён от него: indexed storage, chunked publication,
bounded reader, actor/replay и GUI import реализованы; diagnostic budgets сохранены.
Синтетический VP9-in-MP4 586frames/24.416667s прошёл native import, полный mock loop
и restart. [Checkpoint](evidence/2026-10-12-native-long-import.json).

## Следующая реализация

1. Реализовать явный **полный Screen reset** отдельной операцией с понятной
   областью сброса, rollback/error state и сохранением файлов пользователя.
   Reset brightness/rotation/slots отличается от восстановления каталога.
2. Определить сохранённый состав коллекций для reset. Контексты выбора
   image/background/watermark реализованы поверх общего хранилища; это ещё
   не отдельные per-device списки Windows. Immutable defaults уже встроены.
3. Проверить расширенную native accessibility и remaining media fault matrix.

## Acceptance

- [x] Импорт24.416667s/586frames VP9-in-MP4 без обрезания, index/frame count точны.
- [ ] Длинный loop, replace/stop, missing/corrupt record, bounded memory/backpressure;
  отмена publication/close/restart не оставляет частично опубликованную запись.
- [x] Pin/reference protection работает для legacy и file-backed clips, в том
  числе saved profile, offline device и pending settings.
- [x] Software sleep/reconnect replay нового reader не возрождает старый stream.
- [x] Builtin entries доступны при первом запуске/restart, bootstrap не требуется.
- [ ] Default delete/editor отклоняются backend и GUI; пользовательские entries
  по-прежнему доступны; restore defaults не сбрасывает profiles/настройки.
- [ ] Full Screen reset: brightness50/rotation0, все8slots и screen intent;
  при save/write/reply failure состояние и способы восстановления явны.
- [ ] Image/video background и watermark выбор работают в одной композиции
  с realtime sensor/text, включая pause/resume/restart и missing sensors.
- [ ] Native keyboard/speech/scaling для каталога и reset.
- [ ] Позже: визуальная и длительная аппаратная приёмка, затем LocalUI.

Отметки выполненного опираются на последующие software/native checkpoints,
а не только на исследование Windows. Физическая приёмка остаётся открытой.

## Встроенные изображения — 2026-10-12

`lconnect-image::defaults` содержит два собственных PNG480×480: прозрачную
«Орбиту» (`builtin-orbit-mark-v1`) и фон «Полночь» (`builtin-midnight-v1`).
Исходный генератор и provenance находятся в `tools/generate-lcd-defaults.py`
и `crates/lconnect-image/assets/defaults/README.md`. Это собственное оформление,
не копия Windows assets; визуальная идентичность не заявляется.

Каталог встроен в бинарник: доступен до создания пользовательской директории,
не требует bootstrap-записи и не дублируется после restart. Theme/profile keys
сохраняются обычным путём. Thumbnail/load разрешают builtin keys до файловой
системы; сторонние PNG/metadata с такими именами не подменяют default.
List/list_named возвращают defaults первыми; cleanup рассматривает только
32-hex пользовательские keys. IPC metadata добавляет `is_default` с legacy
serde default=false. Daemon отклоняет delete и rename, GUI оставляет Add доступным.
Запрет rename — локальное правило неизменяемого каталога, а не доказанное
требование Windows. Будущий editor должен запрещать изменение этих ресурсов;
размещение/размер/opacity слоя в пользовательской теме остаются редактируемыми.

- [x] Image defaults доступны при первом запуске/restart, без disk bootstrap.
- [x] Защита PNG/metadata от подмены, backend rename/delete refusal и cleanup.
- [x] Browser: default можно добавить в draft, rename/delete отключены.
- [x] Video default (см. следующий checkpoint).
- [x] Контексты выбора image/background/watermark.
- [ ] Полный Screen reset и состав коллекций.
- [ ] Native accessibility каталога и hardware output.

[Checkpoint](evidence/2026-10-12-builtin-images.json).

## Встроенный видеофон — 2026-10-12

«Сияние» — собственный периодический фон с двумя движущимися teal/violet
свечениями:576frames,24fps,480×480,400236encoded bytes. Он включён в daemon
как H.264 и индекс AU; на каждый read выделяется только один ограниченный AU.
Runtime FFmpeg/bootstrap/user quota для standalone playback не нужны.
Композиция с overlays использует существующий decoder/renderer/encoder.

Стабильный reserved32hex key совместим с прежними theme/profile/media fields.
Store list возвращает builtin первым, acquire_source читает embedded данные;
подмена JSON/LCM на диске не действует. Import/publication/receipt recovery
отклоняют зарезервированный ключ, delete запрещён; legacy whole-clip RPC
отклоняет длинный builtin и требует streaming path. GUI допускает выбор для LCD
или фона темы и показывает запрет удаления. Старые metadata без is_default
читаются с false. Квота остаётся64 пользовательских ролика.

Генератор, лицензия и ограничения воспроизводимости кодека:
`tools/generate-lcd-default-video.py`,
`crates/lconnect-daemon/assets/defaults/README.md`. Исходные Windows assets не
копировались; оформление и длительность не объявляются визуально идентичными
оригинальному default. [Checkpoint](evidence/2026-10-12-builtin-video.json).

- [x] Полное декодирование576frames, изменение картинки, каждый AU валиден.
- [x] Restart/shadow protection, запрет публикации/удаления и неизменность user quota.
- [x] Browser: builtin доступен для playback/background, delete отключён.
- [x] Выбор image/video background и default logo как watermark.
- [ ] Сохранённый состав отдельных коллекций и его reset.
- [ ] Full Screen reset/recovery и расширенная native accessibility.

Полный native media повтор для этого checkpoint прошёл: builtin video preview,
Apply/Clear, image defaults/delete/focus и ранее существующие import/gallery/
restart/composition/close workflows. Расширенная accessibility и физический LCD
этим не квалифицируются.

## Контексты выбора и статический фон — 2026-10-12

ImageGallery выбирает назначение: слой, фон или водяной знак. «Орбита» доступна
во всех контекстах, «Полночь» — для слоя/фона; пользовательские PNG — во всех.
Фон рисуется под overlays, watermark добавляется последним. Image/video/solid
взаимно исключаются, переключение сохраняет остальные слои и не вызывает Apply.
Четыре режима fit используют общий renderer, включая alpha compositing.

`background_image` — optional extension v1 theme; старые темы без поля читаются
как прежде. Старый daemon с deny_unknown_fields явно отклонит новую тему,
поэтому GUI/daemon нужно обновлять вместе. Image references учитываются при
load, live lease, offline settings, profiles, delete и cleanup.

[Software/native evidence](evidence/2026-10-12-gallery-contexts.json).
Это контексты выбора в общем каталоге, не реализация сохранённого состава трёх
Windows-коллекций. Полный reset/recovery, native accessibility, lifecycle matrix
и физический вывод остаются отдельными gates.

## Подготовка полного сброса: атомарная группа слотов — 2026-10-12

`edit_profile` принимает `reset_group` с `group` и optional `sensor_id`.
Сброс screen создаёт все8slots; fan/pump — все4 с явным температурным датчиком.
Одна проверка revision, один store lock и один atomic write заменяют всю группу.
Соседние группы и media-файлы сохраняются; stale revision требует readback.
CLI: `lconnectctl profiles reset-group screen`; в desktop — «Сбросить все слоты
группы…» с подтверждением. Операция не выполняет device Apply.

Это prerequisite, **не полный Screen reset**. Следующая реализация должна учесть
текущие границы владения:

- ProfileStore сейчас глобальный, DeviceSettings — per-device. Полный reset
  должен явно определять область слотов и коллекций; нельзя молча сбросить
  чужие device profiles под видом операции одного устройства.
- Settings writer применяет независимые deltas с debounce. Нельзя записать
  snapshot на диск из IPC параллельно actor: последующая delta перезапишет его.
- LCD actor сериализует source/config; полный reset должен занять одну такую
  операцию, отменить старую preparation/stream и удержать ресурсы до commit.
- Настройки, слоты и состав коллекций требуют общего durable intent/recovery
  либо единой записи. Цепочка отдельных RPC с best-effort rollback недостаточна.
- Ответ должен различать persisted intent, device apply и recovery required.
  Потерянный reply ведёт к readback, не к слепому повтору. Файлы пользователя
  не удаляются при замене состава коллекций.

Acceptance полного reset остаётся: brightness50/rotation0, согласованные8slots,
default screen intent и коллекции; restart во всех точках commit, write/save/
reply failure, concurrent Apply/profile edit, detached device, соседние cooling/
lighting settings и сохранность user files. Физическую приёмку выполнять позже.

## Durable reset transaction — 2026-10-12

Backend foundation теперь связывает device settings и общие screen slots через
redo journal `screen-reset-transaction.json`. Настройки и ProfileStore используют
один gate (один PersistenceStore на daemon; ProfileStore получаем через
`persistence.profiles()`). До публикации проверяются schema, revision и полные snapshots;
после публикации journal — committed intent. Перед чтением любого из двух
хранилищ recovery заканчивает обе записи, затем удаляет журнал. Несовпадение
текущего файла со старым и новым snapshot останавливает recovery с сохранением
файлов и журнала. Corrupt/future journals не удаляются автоматически.

`SettingsUpdate::ResetScreen` проходит через settings owner: queued cooling/
lighting deltas входят в новый snapshot. Сбрасываются brightness50/rotation0,
media choice→базовая Linux clock theme и8shared slots; соседние profile groups
сохраняются. Receipt последней операции хранится в DeviceSettings. Повтор того
же operation ID не повторяет reset и не теряет более поздние pending deltas.
При неопределимом recovery writer прекращает запись вместо flush старого snapshot.
Disk work/gate wait выполняются через spawn_blocking, owner ждёт их по порядку.

Это ещё **не доступный пользователю полный Screen reset**. Не подключены:

- LCD actor operation подключена (см. следующий checkpoint); внешний IPC/GUI путь ещё отсутствует;
- сохранённый состав image/background/watermark collections и его reset;
- IPC receipt/readback, scope disclosure (8slots пока глобальные), GUI confirmation;
- recovery status и явный повтор применения durable intent после transport error.

Old daemon отвергает новый optional receipt field; обновлять вместе с GUI.
Проверки interruption моделируют остановку между файловыми шагами, не потерю
питания файловой системы. Внешние редакторы не разделяют in-process gate;
обнаруживается изменённый snapshot, но multi-process writer arbitration этим
не реализован. [Checkpoint](evidence/2026-10-12-screen-reset-transaction.json).

## LCD actor reset — 2026-10-12

`LcdCommand::ResetScreen` проверяет заранее подготовленную базовую тему,
дожидается durable commit settings owner и затем одной actor operation
инвалидирует старые preparation tickets, останавливает/join прежний source,
применяет brightness50/rotation0 и первый JPEG, запускает clock runtime.
Режим — **Application**. Предыдущий storage checkpoint ошибочно записывал
внутренний LCDSetting; actor-регрессия обнаружила это, исправление охватывает
persisted config и runtime. После reset обычный brightness patch работает.

Результат различает durable receipt, repeated и lcd.last_error. Ошибка config
или frame не отменяет сохранённый reset. Повтор operation ID возвращает текущее
состояние без повторного Apply; позднейшие настройки не сбрасываются. Явное
повторное применение после transport failure и внешний readback ещё нужны.
Rejected storage edit не трогает текущий source. RecoveryRequired/потеря
внутреннего reply останавливает вывод и блокирует мутации до восстановления
хранилища и restart; State/Stats/Theme/Media readback остаются доступны.

Проверки включают held old renderer (и позднюю ошибку), invalidation tickets,
lost caller reply, config/frame faults, отказ мутаций после неопределимого
recovery. Real FFmpeg composition → reset через MockBackend: codec child PIDs
исчезают до ответа, video frame count перестаёт расти; исходный clip удаляется
до shutdown manager, подтверждая освобождение active/pending pins.
[Checkpoint](evidence/2026-10-12-screen-reset-actor.json).

Остаются состав коллекций, IPC/GUI с областью сброса общих slots, receipt
readback/explicit reapply и общая native/lifecycle приёмка. Hardware не запускался.

## Коллекции: schema и storage reset — 2026-10-12

`DeviceSettings.screen_collections` хранит отдельные image/background/watermark
списки с общей ревизией. Отсутствующее поле означает legacy общий каталог,
а не пустой список. Defaults защищены от удаления из состава; ключи, размер,
дубликаты и версия проверяются перед сохранением. Image references всех трёх
контекстов и video backgrounds учитываются в offline deletion/cleanup guards.

Reset проверяет обе ожидаемые ревизии — profiles и collections — и записывает
новый состав в том же redo journal, что settings и slots. Receipt сохраняет
collection revision; старые receipt читаются с нулевой ревизией. Повтор ID
не повторяет сброс. Сброс состава не удаляет файлы из managed storage.

Это основа хранения, ещё не готовые пользовательские коллекции. Остаются:

- membership read/edit API через settings owner и asset leases до commit;
- миграция legacy общего каталога без потери пользовательских entries;
- GUI выбора коллекции и повторного добавления сохранённого файла после reset;
- внешний reset/readback/reapply и native/lifecycle acceptance.

Проверки: stale collection revision без изменения файлов/slots, round-trip,
offline references, сохранение пользовательских bytes, invalid schema и
исчерпание revision. Hardware и native GUI в этом срезе не запускались.

[Коллекции: storage checkpoint](evidence/2026-10-12-screen-collections-storage.json): 744 tests passed, 1 ignored; fmt, strict Clippy и docs прошли.

## Prepared membership и settings owner — 2026-10-12

`PreparedCollections::prepare` проверяет состав и каждый asset до постановки
изменения в очередь. Для PNG используется лёгкий `ImagePin`: декодируется
только одно изображение за раз, затем пиксели освобождаются. Между decoded
lease и pin нет окна для удаления. Видео использует существующий `MediaPin`.
Ошибка подготовки освобождает все полученные leases.

`SettingsUpdate::SetCollections` сравнивает current/next revision, сохраняет
соседние queued settings и дожидается записи. Потеря ответа не отменяет работу.
При ошибке записи принятое intent и pins остаются у owner для retry;
`ReadCollections` возвращает принятое состояние, **не доказательство записи**.
Будущий IPC должен возвращать также persistence status и не показывать saved
по одному readback. Замена состава и reset освобождают старые leases;
сохранённые ссылки продолжают защищать файлы после остановки owner.

Внешний IPC, миграция legacy общего каталога и GUI ещё не подключены. При
миграции нельзя молча обрезать каталог до лимита или терять элементы,
импортированные конкурентно с подготовкой migration snapshot.

[Membership owner checkpoint](evidence/2026-10-12-screen-collections-owner.json): 748 tests passed, 1 ignored; fmt, strict Clippy и docs прошли. Внешний IPC/GUI ещё открыт.

## Collection IPC — 2026-10-12

- `get_screen_collections({dev})` возвращает `{collections, persistence}` одним
  снимком settings owner. `collections: null` означает legacy общий каталог.
- `set_screen_collections({dev, expected_revision, collections})` заменяет состав
  целиком. `collections.revision` должна быть `expected_revision + 1`; первая
  явная запись использует 0 → 1. Schema и assets проверяются до enqueue,
  CAS выполняется owner после подготовки. Успешный ответ подтверждает запись.
- Ошибка записи не отменяет принятое intent: читать оба поля get-ответа.
  `persistence.saved` означает хотя бы одну успешную запись в текущей сессии;
  при `pending: true` это не подтверждение сохранения нового состава.
- Неверная ревизия/asset отклоняется без изменения текущего состава. Потерянный
  ответ требует readback, а не слепого повтора мутации.

Автоматическая миграция legacy каталога не включена: RPC полной замены не
должен использоваться GUI как неявная миграция обрезанного списка. Миграция
с защитой от конкурентного import и пользовательский интерфейс ещё нужны.

[Collection IPC checkpoint](evidence/2026-10-12-screen-collections-ipc.json): 749 tests passed, 1 ignored; fmt, strict Clippy и docs прошли.

## Legacy migration — 2026-10-12

`migrate_screen_collections({dev})` переносит legacy общий каталог в revision1.
Пользовательские PNG входят в image/background/watermark; builtin PNG сохраняют
свои контексты, video входят в backgrounds. Состав проверяется целиком: превышение
лимита отклоняет перенос без обрезки и записи. Все assets проверяются и pin-ятся.

Непосредственно перед записью inventory повторно сравнивается под блокировками
images → media → settings. Изменение набора ключей до commit отклоняет migration
без изменения intent; клиент перечитывает состояние и повторяет подготовку.
Импорты после принятия migration остаются в managed catalog и должны отдельно
добавляться в нужную коллекцию. Изменения bytes внешним редактором при прежнем
ключе не являются частью этого inventory-контракта.

Ошибка записи после принятия сохраняет intent и leases для retry, как обычный
membership edit. Readback показывает pending/error; shutdown flush сохраняет
также соседние queued settings. Повторная migration уже инициализированных
коллекций отклоняется. Lost reply восстанавливается get_screen_collections.
Пользовательский GUI переноса/редактирования ещё не подключён.

[Legacy migration checkpoint](evidence/2026-10-12-screen-collections-migration.json): 752 tests passed, 1 ignored; fmt, strict Clippy и docs прошли.

## GUI membership panel — 2026-10-12

Display содержит панель «Коллекции LCD»: явный перенос legacy каталога,
переключение image/background/watermark, локальное редактирование состава,
сохранение и повторное добавление из managed catalog. Canonical defaults
защищены. Удаление из списка не вызывает физическое delete и не меняет тему.

Readback после мутации обязателен. Pending save и неизвестный результат
блокируют дальнейшее редактирование до успешной проверки. Закрытие/открытие
панели не снимает блокировку незавершённой записи; settlement инициирует новое
чтение, а устаревшие notice/focus не применяются. После удаления строки focus
возвращается на устойчивую кнопку обновления.

Это панель управления составом. Существующие ImageGallery/MediaLibrary пока
показывают общий каталог: фильтрация выбора темы по сохранённым коллекциям и
добавление нового импорта в выбранный список ещё должны быть подключены.
Также остаются полный Screen reset GUI и native accessibility/acceptance.

[Collection panel checkpoint](evidence/2026-10-12-collections-panel.json): frontend check/build, browser workflows и 20 GUI crate tests прошли. Native ещё не проверен.

## Галереи и import используют membership — 2026-10-12

ImageGallery читает состав выбранного устройства и фильтрует изображения по
image/background/watermark. MediaLibrary показывает video backgrounds этого
устройства. Legacy null сохраняет общий каталог. Ошибка чтения или pending
persistence блокирует выбор вместо fallback на полный каталог. Изменения в
панели коллекций инвалидируют открытые галереи.

Импорт изображения добавляет asset в images до изменения draft; импорт video
добавляет key в backgrounds. Для нового native media job фиксируется исходное
устройство. Обнаруженный после открытия панели чужой/старый job не изменяет
коллекции текущего устройства. Файл публикуется отдельно от membership: при
ошибке файл остаётся в каталоге и доступен для явного re-add через панель.
CAS не повторяется автоматически после ошибки/потери ответа.

Остаются native qualification и полный Screen reset. Для файлов вне коллекций добавлен режим «Показать все файлы … для управления».
Он сохраняет rename/delete, но блокирует выбор в тему/воспроизведение вне
соответствующей коллекции. Conservative bulk cleanup остаётся отдельно.

[Gallery integration checkpoint](evidence/2026-10-12-collection-gallery-integration.json): фильтрация по устройству/контексту, import и управление файлами вне коллекций проверены browser. Native ещё открыт.


## Reset RPC и admission — 2026-10-12

`reset_screen(dev, operation_id, expected_profile_revision, expected_collection_revision)`
возвращает receipt/repeated/LCD; `get_screen_reset(dev)` — receipt, persistence
status и LCD через actor barrier. Operation ID — 32 lowercase hex символа.
Для нового reset actor выдаёт preparation ticket до подготовки default theme;
устаревший ticket отвергается до durable reset. Завершённый duplicate получает
текущее состояние без нового ticket/Apply и не отменяет позднейшую подготовку.
Transport failure сохраняет receipt; успешный durable reset не означает
успешный LCD output. Explicit reapply и GUI остаются следующими шагами.
[Software checkpoint](evidence/2026-10-12-screen-reset-ipc.json).


## Явное повторное применение reset — 2026-10-12

`reapply_screen_reset(dev, operation_id, expected_generation)` повторяет только
LCD config/frame после ошибки первоначального reset; settings/slots/collections
не записываются повторно. `get_screen_reset.reapply_available` сообщает наличие
такой попытки на текущем attachment. Подготовка свежего кадра идёт вне actor;
перед отправкой проверяются operation ID, generation, preparation ticket и
тот же runtime. Новая тема, Clear, config patch или новый reset отменяют retry.
Повтор завершённого reset ID eligibility не меняет.

Ошибку повторной передачи показывает `lcd.last_error`; readback возвращает новое
поколение и доступность следующей попытки. После lost reply сначала читать
состояние, не повторять мутацию автоматически. Success снимает eligibility.
После restart/detach действует обычный replay сохранённого состояния; старый
attachment-local retry недоступен. Это не общий механизм восстановления любого
последующего playback failure.

Остаются GUI, native/accessibility и совместная lifecycle приёмка.
[Checkpoint](evidence/2026-10-12-screen-reset-reapply.json).


## GUI полного Screen reset — 2026-10-12

На странице экрана добавлена отдельная панель сброса. Подтверждение явно
перечисляет settings/коллекции выбранного устройства и восемь общих Screen-слотов;
файлы, cooling/RGB и локальный черновик сохраняются. Начальные revisions берутся
из readback; конфликт требует нового подтверждения. Неопределённый ответ
не вызывает автоматический повтор: сначала readback, затем только явный повтор
с тем же operation ID/revisions. Позднее подтверждение обновляет зависимые
галереи и инвалидирует прочитанные ранее profiles/collections.

Панель остаётся смонтированной при смене вкладки. Пока запись выполняется,
Apply и смена устройства заблокированы. Ошибка чтения блокирует новую мутацию.
Readback с `reapply_available` предлагает повтор только передачи на LCD.
Обновление результата не заменяет локальный черновик темы.

Остаются native/accessibility и combined lifecycle qualification.

[Screen reset GUI checkpoint](evidence/2026-10-12-screen-reset-gui.json).
