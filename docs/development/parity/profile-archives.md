# Переносимые профили с зависимостями

Baseline: Windows L-Connect3 **2.1.28.0**, Galahad II LCDV2. Срез2026-10-14
реализует **storage, Unix IPC, CLI и GUI собственного Linux формата**. Начальный
GUI workflow проверен в browser и изолированном native окружении с MockBackend.
Расширенная приёмка остаётся в [чеклисте](basic-parity-checklist.md).
Совместимость с Windows archive не заявлена. Device-free `profiles windows-inspect`
добавляет bounded inspection ZIP/metadata2.1.28; `profiles windows-cooling` —
offline RPM policy preparation, без full-group import/Apply; [mapping и границы](profile-groups.md)
остаются отдельными этапами.

## Реализованный контракт

`llcontrol_daemon::profile_archive::ProfileArchives` экспортирует один
`NamedProfile`: Screen theme, ScreenMedia, Fan или Pump. Активные настройки
устройства не меняются. Для Screen включаются все изображения, пользовательские
шрифты и background videos активной страницы и всех сохранённых неактивных
страниц. Общие ключи внутри каждого типа пакуются один раз. Bundled images/video
также включаются: импорт не зависит от совпадения их ключей в другой установке.

Операции синхронные и должны вызываться на blocking worker с отдельным admission:

- `export(profile, writer)` создаёт snapshot и передаёт контейнер в writer.
  Ошибка означает неполный output; клиент обязан его отбросить.
- `export_file(profile, path)` использует временный файл рядом с destination,
  sync и atomic no-clobber publication. Существующий файл не перезаписывается.
- `import(reader, library, expected_revision, slot)` проверяет весь контейнер,
  импортирует зависимости под новыми ключами и заменяет только выбранный слот
  соответствующей группы через существующий revision check/atomic profile store.

Remap изменяет только typed asset fields, включая background, Image layers,
value/label/unit fonts и cached pages. Текст и sensor ID/unit сохраняются точно,
даже если их строка совпадает с прежним asset key. Импорт не подбирает другой
датчик взамен отсутствующего. Названия изображений и роликов сохраняются;
файлы шрифтов переносятся без изменения байтов.

## Формат и ограничения

Magic `LCPROF01`, little-endian u32 длина manifest,32 bytes SHA-256 manifest,
затем strict JSON manifest и последовательные payloads. Manifest содержит
version1, profile и список typed dependencies с key, byte length и SHA-256;
image name optional. Это контроль целостности, не цифровая подпись.

Вход не содержит extraction paths. Payloads пишутся в private tempdir по
порядковым номерам; каждый сверяется с checksum и полностью проверяется своим
валидатором до публикации первого asset. Missing/duplicate/unreferenced entries,
unknown version/fields, неверные длины и trailing bytes отклоняются. Media
проверяется на каждом access unit, включая последний, через текущий LCM reader.

Ресурсные бюджеты собственного формата:

- Manifest ≤1MiB, ≤1024 dependencies, суммарный payload ≤8GiB.
- Image/font ≤16MiB на файл; normalized images не больше960×960.
- Все шрифты темы вместе ≤64MiB, как у текущего renderer/store.
- Media использует существующие256MiB payload/86400-frame и8MiB index limits.
- Copy buffer64KiB; в памяти один image/font/access unit, не полный ролик
  или архив. Полные payloads временно хранятся на диске.

Сначала проверяется initial library revision; после чтения и подготовки остаётся
окончательный CAS. Конкурентный edit не затирается. Ошибка публикации asset либо
конечный conflict могут оставить **валидные, не связанные с профилем imports**;
они сохраняются для обычного cleanup. Неполный профиль не публикуется, старые
assets не заменяются. Прямой storage API не создаёт transfer receipt; IPC использует описанное ниже
атомарное подтверждение импорта.

## Программная проверка и оставшаяся работа

Focused regressions проверяют перенос в пустое хранилище, cached Rain и Basic,
shared images/fonts/video, exact sensor/text identity, имена, перезапуск библиотеки,
сохранение других групп, ScreenMedia/Fan/Pump, atomic no-overwrite export,
missing source, invalid target и stale/concurrent revision. Неверные typed
payloads отклоняются даже при пересчитанном корректном checksum. Indexed video
600 frames/>1MiB проходит в обоих направлениях при read/write chunks≤64KiB;
это не ограниченный diagnostic clip path. Результаты текущего прогона — в
[evidence](evidence/2026-10-14-profile-archive-storage.json).

## IPC transfer и durable receipt

RPC `profile_archive {request}` поддерживает BeginExport/BeginImport, Chunk,
Read, Inspect, Finish, Status, Cancel и Release. JSON action names — snake_case.
ID —32 lowercase hex digits, клиент создаёт новый случайный ID для каждого
нового намерения и сохраняет его до подтверждения результата. Повтор Begin
с теми же параметрами возвращает текущую сессию; другие параметры того же ID
отклоняются. BeginImport фиксирует size, expected_revision, group и target slot.

Chunk принимает1…64KiB с contiguous offset. Точный повтор подтверждённого
блока допустим; изменённый retry/перекрытие не переписывают историю. Export Read
принимает offset и length≤64KiB и возвращает bytes, исходный offset и явный EOF.
Клиент проверяет ID, size, offset, непрерывность и EOF, затем атомарно сохраняет
полученный файл. Inspect проверяет только manifest и target group, не payload.
Finish проверяет весь архив и сохраняет слот; успешный ответ содержит actual
imported profile с новыми asset keys, library и committed_revision.

`ProfileLibrary.archive_imports` содержит последние64 compact receipts. Receipt
и новый slot пишутся одним atomic profiles.json commit. Finish/Status/BeginImport
после потерянного ответа или restart восстанавливают подтверждение без повторной
публикации assets. Если библиотека позже менялась, ответ содержит её текущий
readback, исходный committed_revision и profile:null; старое намерение не
восстанавливает удалённый/сброшенный слот. После вытеснения старого receipt его
исходный expected_revision всё равно не даёт повторно применить старую операцию.

Есть4 сессии,2 admission permits blocking workers и общий reserve для retained
transfer files≤2×MAX_PROFILE_ARCHIVE_BYTES. Для экспорта резервируется максимально
возможный output, после подготовки — фактическая длина. Это ограничение transfer
staging; временная подготовка storage и опубликованная библиотека имеют свои
бюджеты выше. Квота не обещает наличие свободного места на файловой системе.

Передачи живут на анонимных временных files, не в Vec всего архива. Idle expiry
300s проверяется запросами и sweeper каждые15s. Брошенный RPC не освобождает
permit/файл, пока блокирующая операция реально не закончилась. Одновременная
archive transaction возвращает DeviceBusy для retry того же ID; остальные API
и HID actors от этой блокировки независимы.

Cancel закрывает незавершённый transfer, не отменяя уже импортированный слот.
Release удаляет подтверждённую in-memory session; durable receipt остаётся.
Синхронные packing/export preparation и атомарный Finish нельзя отменить посреди
их blocking transaction: клиент ждёт/readback либо получает DeviceBusy. Это
оставшаяся граница интерактивной отмены, не подтверждение мгновенного Cancel.
Незавершённые upload/download sessions не переживают restart; committed receipt
переживает. Не начинать новый import вместо неизвестного исхода старого Finish.

Программная квалификация IPC/CLI: workspace946 passed/2ignored, strict Clippy,
rustdoc и fmt прошли. Real Unix sockets/MockBackend проверили CJK font/image/
cached video в чистом store, lost Finish reply/restart, текущий Status после
удаления slot и запрет resurrection. Atomic write fault tests проверили совместное
подтверждение slot/receipt и повтор sync после ambiguous rename. Actual CLI
subprocesses проверили export/import/no-overwrite/readback и Ctrl+C на pending
upload Chunk. HID writes0. [Evidence](evidence/2026-10-14-profile-archive-transfer.json).

## CLI

```sh
llcontrol profiles export screen 0 night.lcp
llcontrol profiles import screen 7 night.lcp
llcontrol profiles archive-state OPERATION_ID
llcontrol profiles archive-finish OPERATION_ID
llcontrol profiles archive-cancel OPERATION_ID
```

Слоты zero-based: screen0…7, fan/pump0…3. Export никогда не перезаписывает
существующий файл. Import не применяет настройки устройства. CLI пишет ID и
progress в stderr, поддерживает общий `--json`; источник/назначение/slot проверяются
до подключения. Передача файлов идёт блоками64KiB. Ctrl+C до Finish пытается
Cancel/Release; после начала Finish требуется readback по напечатанному ID.
Операция cancel уже committed import не является undo.

## GUI и native FileChooser

В «Сохранённых профилях» выберите слот и откройте «Импорт и экспорт профиля».
Native-команды `profile_archive_start/status/cancel/recover/dismiss` владеют
передачей и файлами; webview получает только статус, счётчики и итоговый профиль,
без полного архива или произвольных путей. Файл выбирается через
[XDG FileChooser](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html):
SaveFile для экспорта, OpenFile для импорта; принимается один локальный file URI.
Ответ portal подписывается до открытия диалога, включая ранний Response.

GUI показывает прогресс и сохраняет ID незавершённой операции в localStorage.
После потерянного ответа или перезагрузки нужен readback исходного ID;
неизвестный исход не разрешает новый импорт. Отмена и переход к commit имеют
одно атомарное решение; начавшийся commit не отменяется. Normal Close ждёт
завершения native worker. Экспорт публикуется атомарно без перезаписи файла.
Импорт обновляет выбранный слот, сохраняя черновик редактора; Load и Apply
выполняются отдельно явными действиями.

Программная квалификация: workspace953 passed/2ignored, browser40,
Svelte0 errors/0warnings, strict Clippy/rustdoc/fmt и production build прошли.
Пять native job tests проверили файловый round-trip, ошибки, отмену/commit,
joined shutdown и отсутствие runtime до резервирования операции. Два private-bus
теста проверили ранний portal Response, URI validation и Close при Cancel.
Изолированный Tauri/WebKitGTK с настоящим Unix IPC и MockBackend прошёл
SaveFile → экспорт → OpenFile → импорт в slot7, remap image/font/cached video,
readback, сохранение черновика, normal Close и restart. После restart импортированная
тема дала preview15229bytes; LCD оставался LocalUI/Idle. Portal был fixture:
реальные системные диалоги, Wayland, parent placement и accessibility ещё не приняты.
Native прогон выявил вызов Tokio из GTK без runtime; команды start/recover
переведены в async, admission защищён проверкой runtime, добавлена регрессия.
[Evidence](evidence/2026-10-14-profile-archive-gui.json).

## Durable ownership отдельных слотов

Finish использует private publication scope и manifest v2 с global target:
operation ID/group/slot/expected library revision/size/source SHA-256. Все raw
payloads, normalized candidates и hard-link ownership лежат в scope до решения.
Перед `profiles.json` commit журнал переводится в armed. Known negative receipt
разрешает identity/reference-protected rollback; confirmed receipt сохраняет файлы.
Потеря caller не освобождает blocking worker до завершения.

Новые receipt атомарно сохраняют `source_sha256` вместе со слотом. Старые receipt
читаются без SHA; startup не считает их достаточным подтверждением принадлежности
и сохраняет файлы с diagnostic. После receipt eviction неизвестный armed outcome
также сохраняет файлы. Status по operation ID возвращает подсказку проверить
библиотеку и использовать новый ID; recovery не сохраняет слот заново и не делает
Apply. [Протокол и границы](import-publication-recovery.md),
[evidence](evidence/2026-10-15-slot-publication-journal.json).

## Полная группа

Для screen/fan+lighting отдельно реализованы typed snapshot, atomic storage restore,
LCGROUP1 file codec, owner snapshot/commit и durable receipt/readback.
Обе группы подключены к actors, Unix IPC/CLI/GUI, explicit reapply и native
file chooser; базовый mock/native round-trip и restart приняты: [source scope, контракт и дальнейшая работа](profile-groups.md).

## Остаётся для пользовательского сценария

- Расширенная native import/export/reconnect/fault/accessibility matrix и реальные
  X11/Wayland FileChooser; текущий native прогон использует private portal fixture.
- Интерактивная отмена длительного packing/preparation и оставшиеся CLI signal/
  fault сценарии. Ctrl+C в ожидающем upload Chunk уже проверен: Cancel/Release
  исходной сессии и сохранение input file. Текущий atomic Finish не прерывается как undo.
- Сопоставление применимого Windows import/export workflow и отдельно его
  формата. Полные screen/fan+lighting group archives реализованы в Linux LCGROUP1;
  Windows ZIP interoperability ещё открыта.

Ни daemon на реальном устройстве, ни аппаратные тесты этот срез не запускает.

## Язык UI архивов, 2026-10-15

Фазы, progress labels и параметризованные сводки/сообщения используют EN/RU
resources; пользовательские имена и source IDs не переводятся. Private native
round-trip, readback и restart проверены на обоих языках. Это не закрывает
все worker errors или настоящий desktop portal picker. Native title/filter labels
теперь локализованы отдельным срезом и проверяются в private portal arguments.
[Общий language contract и оставшаяся приёмка](interface-language.md),
[evidence](evidence/2026-10-15-language-archive-ui.json).

App-owned worker ошибки теперь могут содержать optional `message_code` отдельно
от исходной `message`. EN/RU меняет текст по коду, opaque diagnostics остаются
данными; COW destination guard и explicit Apply не меняются. Native сценарий
включает повторный Export в существующий файл и следующий Import с чистым notice.
[Контракт и границы](interface-language.md),
[evidence](evidence/2026-10-15-archive-structured-notices.json).
