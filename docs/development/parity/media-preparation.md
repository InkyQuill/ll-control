# M4: подготовка GIF/MP4 — 2026-10-11

> Статус сверен 2026-10-17: GUI import, persistent library, gallery contexts,
> select/loop/stop/restart, long video и video+sensor composition реализованы.
> Ниже сохранена история checkpoints; ранние «следующие шаги» не являются
> текущим планом. Актуальные GUI recovery rules — в [video-editor](video-editor.md),
> очередь — в [roadmap](../../../roadmap.md), приёмка — в
> [общем чеклисте](basic-parity-checklist.md).


## Current thumbnail rendering — 2026-10-17

Native decode и browser image decode — разные этапы. Broken image bytes теперь
дают понятный EN/RU status и explicit retry, включая compact cards. URL старого
image принадлежит effect и освобождается при retry/unmount; key-specific image
nodes и source guard отбрасывают obsolete error events. Derived primitive clip
key сохраняет preview/error при metadata polling и смене языка, не запуская новый
decoder. Request cancellation, bounded busy retry и выбор без LCD Apply сохранены.
Alt/fallback labels переводятся; пользовательские имена остаются literal.
[57 regression/41 final browser cases, native scopes и ограничения](evidence/2026-10-17-library-resources.json).
Срез включён в [пакет collections](evidence/2026-10-17-collections-integrated-candidate.json).

Первый реализованный путь: локальный GIF/MP4 → H.264 access units → существующий
CLI `lcd play-clip`. Это ещё не gallery workflow: хранение assets, GUI import,
GUI gallery и расширение transport limits остаются задачами M4; выбор
prepared clip с постоянным loop/restart replay реализован ниже.

## Подготовка без устройства

```sh
python3 tools/prepare-lcdv2-clip.py --input animation.gif --output /tmp/animation.json
python3 tools/prepare-lcdv2-clip.py --input video.mp4 --fit cover --output /tmp/video.json
```

`contain` (default) вписывает с чёрными полями; `cover` заполняет с центральной
обрезкой. Выход480×480, square pixels, baseline H.264/level3.0/YUV420P/24fps,
GOP24 без B-frames. Frame delays GIF преобразуются по timestamps; embedded
loop игнорируется, повторением управляет daemon. Audio/subtitles не выводятся.
Без `--input` остаётся прежняя двухсекундная diagnostic animation.

Нужны FFmpeg/FFprobe с libx264 и scale `reset_sar`; проверено на FFmpeg n9.0.2.
[FFmpeg scale](https://ffmpeg.org/ffmpeg-filters.html#scale-1) описывает сохранение
display aspect ratio с square pixels при proportional scaling. Anamorphic
input проверен отдельно, чтобы `setsar` не скрывал растянутую картинку.

Ограничения существующего diagnostic transport не сняты: до240кадров/10s,
до1MiB encoded bytes, до128KiB на access unit. Вход до256MiB, dimensions до4096
по каждой оси; каждый subprocess имеет timeout60s. Форматы ограничены GIF/MP4,
file protocol; output count и file size ограничены при encode и повторно
валидируются до публикации. Слишком длинный/большой файл отклоняется целиком.
Качество/лимиты не считаются совпадающими с Windows editor.

Manifest и metadata создаются эксклюзивно; существующие файлы не заменяются.
Metadata содержит параметры, версии инструментов и hashes output. Полной
транзакционной crash durability двух файлов этот CLI helper не обещает.

После подготовки manifest пригоден для существующего `lcd play-clip PATH --loops N`.
Эта команда уже обращается к daemon/устройству; в текущем этапе она проверяется
только с private Unix-socket fixture. Разовый diagnostic playback не восстанавливается после restart; явный
select-clip сохраняет выбор отдельно (см. ниже).

## Проверки и следующий шаг

```sh
cargo build -p llcontrol-cli --locked
python3 -m unittest discover -s tools/tests -v
```

7tests: реальная кодировка/декодирование MP4/GIF, contain/cover pixels, display
aspect ratio, changing GIF frames с delays .25/.75s, embedded loop, diagnostic
48frames, malformed/long rejection, сохранение существующих outputs; CLI
validator и IPC payload проверены через private socket. Ни daemon, ни HID
не запускались. [Evidence](evidence/2026-10-11-media-preparation.json).

Далее нужен bounded disk-backed playback поверх managed media assets,
чтобы заменить diagnostic limits осмысленными пользовательскими ограничениями.
После этого — import/progress/cancel, gallery select/delete/thumbnail,
loop/replace/stop и restart/sleep replay через единственного LCD actor.
Windows editor controls и допустимые media limits сверяются отдельно.

## Daemon-managed prepared clips — 2026-10-11

Добавлены IPC `import_prepared_clip {name,clip}`, `list_prepared_clips {}` и
`load_prepared_clip {key}`. Они не требуют DeviceId и не запускают LCD.
PersistenceStore хранит библиотеку в `prepared-clips/`: один version1 JSON
с названием и clip на случайный32hex key. Импорт валидируется до filesystem
writes, staging file fsync → no-clobber publication → directory fsync.
Чтение после restart повторно проверяет schema/version/name/clip limits.
Файлы не сливаются с device settings и не меняют активную тему.

Метаданные списка: key/name/fps/frames/encoded_bytes. Повреждённая запись даёт
явную ошибку списка с key; чтение остальных записей по key остаётся доступным.
Unknown filenames не импортируются автоматически. Paths, symlinks и nonregular
records отклоняются; библиотека принадлежит пользователю daemon. Это не sandbox
против одновременной внешней подмены filesystem владельцем каталога.

Временный prepared-clip каталог ограничен64записями и1024directory entries;
одна JSON record до8MiB, с прежним1MiB encoded payload. Clones одного store
разделяют mutex, а IPC операции — bounded blocking admission с theme jobs.
Изолированные PersistenceStore для одного root не предназначены для нескольких
одновременно пишущих daemon. Ошибка directory fsync может оставить уже
опубликованный файл: клиент должен перечитать библиотеку после такого отказа.

Это хранение подготовленных клипов, не завершённая галерея. CLI import/list/play по ключу реализован ниже. Ещё нужны GUI
controls, delete с учётом references, thumbnails и named profile slots для media. API load возвращает
исходный validated clip для существующего play_lcd_clip; автоматического
воспроизведения при импорте нет. Restart replay включается явным select-clip.

[Verification evidence](evidence/2026-10-11-prepared-media-store.json):
640Rust/94targets, fmt/strict Clippy/docs и companion review прошли.

## CLI library workflow — 2026-10-11

После подготовки manifest:

```sh
llcontrol lcd import-clip /tmp/animation.json --name 'Мой ролик'
llcontrol lcd clips
llcontrol lcd play-saved-clip KEY --loops 2 --device DEVICE_ID
llcontrol lcd clear --device DEVICE_ID
```

Import выводит key и metadata; import/list не требуют подключённого LCD.
Play получает сохранённый clip, повторно валидирует его и отправляет через
существующий play_lcd_clip. Сам clip сохраняется, но его выбор не включает
автозапуск. Диагностические лимиты и firmware eligibility продолжают действовать.
Ошибка load/validation не отправляет play; invalid key/loops и malformed
local manifest отвергаются до обращения к устройству.

Сквозной тест запускает реальный CLI через private Unix socket к IpcServerImpl:
импорт без устройства → новый manager/store → список → play по key → Clear.
MockBackend отвечает на firmware query после записи команды. Проверяется полная
последовательность512-byte H.264 packets исходного unit, затем точный LocalUI
packet и Idle/applied LocalUI. Synthetic NAL fixture проверяет транспорт,
не decode/visible playback; реальные FFmpeg decode tests описаны выше.
Hardware/production daemon не запускались.

[CLI verification evidence](evidence/2026-10-11-media-library-cli.json):
8focused/643workspace tests/95targets; fmt/strict Clippy/docs и review прошли.

## Сохранённый выбор и непрерывный повтор — 2026-10-11

```sh
llcontrol lcd select-clip KEY --device DEVICE_ID
llcontrol lcd selected-clip --device DEVICE_ID
llcontrol lcd clear --device DEVICE_ID
```

`select-clip` выбирает asset для непрерывного повтора и сохраняет media_clip key
в device settings. Это отличается от разового play-saved-clip --loops N.
`selected-clip` возвращает key или null; live/source/error и persistence status
по-прежнему доступны через lcd state. Transport source пока обозначается
DiagnosticClip: формат/лимиты и firmware guard остаются прежними.

Смена источника принадлежит LCD actor. Перед загрузкой резервируется intent
из того же счётчика, что у theme preparation. Более новый source request
отклоняет запоздалую загрузку. Theme и media_clip взаимно исключаются одним
settings delta. Clear, mode change, image, theme и разовый clip снимают
сохранённый media choice; brightness-only сохраняет выбор. Запись использует
существующий debounce/final flush: accepted Apply не является подтверждением fsync.

Manager загружает сохранённый clip до публикации device в registry. Missing,
corrupt или неподдерживаемый asset/firmware оставляет устройство доступным,
source Idle и явную restore error; выбранный key не теряется. После ремонта
asset следующий attach/restart повторяет восстановление. Повторное чтение и
validation происходят вне async executor. Все кадры отправляет прежний actor.

Проверен software путь через пять запусков CLI/daemon fixture: offline import,
явный выбор, missing-asset restart без H.264 отправок, восстановление файла и
непрерывный replay, Clear и следующий restart без autoplay. Отдельно проверен
stale preparation ticket после нового изображения. Физический вывод, named
profile slots и disk-backed long-video playback остаются. GUI selection добавлен ниже. Software sleep/wake
для текущего prepared-clip runtime проверен ниже.

[Replay verification evidence](evidence/2026-10-11-media-selection-replay.json):
644Rust/95targets, fmt/strict Clippy/docs и review прошли.

## Software sleep/wake и schema acceptance — 2026-10-11

CLI/daemon fixture после восстановления selected clip вызывает настоящий
ManagerHandle prepare_for_sleep: teardown подтверждён пустым registry;
история MockBackend не меняется в течение150ms, больше трёх frame periods.
Команда со старым attachment ID во сне отклоняется без новых writes.
Wake выделяет новый ID, сохраняет media key и отправляет полный сохранённый
frame batch после границы сна. Clear после wake и следующий restart не
возрождают clip. Это проверка manager/actor lifecycle; реальные logind/USB
и новый совместный media+private-bus fault run не выполнялись.

Schema regression подтверждает roundtrip media key с соседним fan PWM,
legacy default None, отказ theme+media conflict и invalid keys без перезаписи
валидного TOML. Production source в этом checkpoint не менялся.

[Evidence](evidence/2026-10-11-media-sleep-schema.json):645Rust/95targets,
fmt/strict Clippy/docs и review прошли. Production не менялся.

## Выбор сохранённого ролика в GUI — 2026-10-11

В Display добавлена раскрываемая «Библиотека роликов»: имя/длительность,
текущий сохранённый выбор, явный select для непрерывного повторения и
stop→LocalUI. Тематический черновик сохраняется. Доступность кнопок учитывает
соединение, выбранное устройство и текущую операцию. Поздний ответ старого
устройства отбрасывается; ошибка списка показана с возможностью retry.

Панель читает данные только открытой и при связи с daemon. Tauri bridge
разрешает list_prepared_clips/get_lcd_media/set_lcd_media; raw clip loading
и diagnostic playback в bridge не разрешены. В первом checkpoint подготовка и импорт выполнялись через CLI; GUI import добавлен ниже.
Thumbnails/delete, named slots и длинное видео остаются отдельными задачами. Панель проверена headless browser; native
WebKit/keyboard/AT-SPI и аппаратный вывод этого изменения ещё не проверены.

[Evidence](evidence/2026-10-11-media-library-gui.json).

## Native preparation backend — 2026-10-11

`crates/llcontrol-media/src/media_prepare.rs` реализует общую подготовку GIF/MP4 внутри
Rust desktop crate. Backend подключён к GUI import в следующем checkpoint.
Python CLI helper остаётся доступным.

Backend использует установленные ffmpeg/ffprobe с libx264 и scale reset_sar,
сохраняет source timestamps, игнорирует встроенный GIF loop, выполняет
contain/cover→480×480→24fps и проверяет contiguous packet coverage через
общий validate_h264_clip. Ограничения прежние:256MiB input,4096px dimensions,
10s/240frames и1MiB encoded clip. Длинное видео не считается реализованным.

Progress сообщает этапы Inspecting/Encoding/Validating без выдуманного процента.
Cancellation перед/между этапами и во время процесса возвращает ошибку без
готового clip. Процесс убивается и reap завершается до возврата; каждый tool
ограничен60s, stdout/stderr читаются с лимитом128KiB. Рабочие файлы принадлежат
TempDir. Подготовка не публикует asset и не меняет источник LCD.

Caller должен запускать prepare в blocking worker и удерживать общий admission
permit до фактического завершения worker, включая отмену/закрытие окна.
После kill wait может задержаться при uninterruptible kernel I/O: нельзя
освобождать permit по одному лишь UI timeout и запускать параллельные imports.
Интеграция владения, chunk upload, состояния import и публикации через
daemon описана ниже. Нельзя вызывать prepare прямо в async/UI executor.

Tests требуют ffmpeg/ffprobe/libx264 и python3. Реальные синтетические MP4/GIF
проверяют decode480×480 и contain/cover pixels, неизменность source, reject
слишком длинного/широкого source до encoding, malformed/cancelled input.
Running-child cancellation проверяет отсутствие собственного PID в /proc
после возврата, включая zombie; это Linux-only fixture. Cleanup пока основан
на TempDir RAII и review, отдельного fault injection для temp cleanup нет.

[Verification](evidence/2026-10-11-native-media-preparation.json).

## Управляемый GUI import — 2026-10-11

«Библиотека роликов» принимает GIF/MP4 через обычный file chooser. Пользователь
выбирает contain/cover; frontend отправляет файл частями не более256KiB с
последовательным offset. Native ограничивает общий размер256MiB, имя256UTF-8
bytes, тип GIF/MP4 и допускает только один активный импорт. Staging использует
собственный TempDir, исходный путь из webview не принимается.

После загрузки owned task вызывает Rust preparation в blocking worker,
показывает Inspecting/Encoding/Validating, затем импортирует clip в daemon.
Webview получает только metadata/status/key. Raw import_prepared_clip остаётся
запрещённым в общем daemon_call allowlist; доступен только внутреннему publisher.
Ни импорт, ни появление key в списке не применяют ролик к LCD.

Отмена upload удаляет staging. Отмена preparation удерживает admission до
возврата worker и reap child. Cancel и переход в Publishing упорядочены одним
lock. После начала публикации отмена отклоняется; потеря ответа помечается как
неоднозначный результат с просьбой проверить библиотеку перед повторным импортом.
Автоматического повторного import нет. Повторный append со старым offset и
повторный start не создают вторую публикацию. Последний статус доступен при
повторном открытии панели. Закрытие панели отменяет подготовку, включая begin,
ответивший уже после unmount; начатая публикация завершается отдельно.

Обычный CloseRequested окна закрывает admission и ждёт cleanup/reap либо
результата publication перед app.exit. Ожидание идёт вне GUI/executor; mutex,
который может удерживаться во время FS I/O, также не берётся в sync GUI handler.
Forced process termination/crash и OS logout здесь не квалифицированы. При
непрерываемом kernel I/O окно может ждать reap дольше deadline FFmpeg.

Программные проверки: real synthetic MP4 upload→prepare; ordered/chunk bounds,
реальный staging cleanup при success/error/cancel; cancel держит admission;
shutdown ждёт worker и publication, ambiguous result остаётся явным. Browser
fixtures проверяют chunk offsets, progress, refresh library без Apply,
validation/cancel/retry и delayed begin после unmount. Сквозная native WebKit→FFmpeg→private daemon проверка добавлена ниже;
аппаратная приёмка остаётся отдельно.

[Verification](evidence/2026-10-11-managed-media-import.json).

## Native import acceptance — 2026-10-11

`python3 desktop/scripts/native-media.py` запускает native-smoke/custom-protocol
binary на отдельном Xvfb/Openbox, private D-Bus/AT-SPI и MockBackend daemon.
Реальные GTK file choosers открываются доверенными X11 keys; focused path/Open
проверяются через AT-SPI. Сценарий проходит chooser cancel, повреждённый MP4,
MP4 contain и GIF cover. Production Tauri commands выполняют chunk upload,
настоящие ffprobe/FFmpeg, daemon publication и обновление GUI library без Apply.

После restart daemon оба key сохранены. H.264 прочитан из сохранённой библиотеки
и реально декодирован в480×480; black top pixel для contain и red top pixel
для cover подтверждены. Realtime/device source при импорте не меняется.

Отмена и обычный WM close проверены с принадлежащим GUI encoder process,
задержанным fixture wrapper перед exec FFmpeg. Это детерминированная проверка
управления процессом, не моделирование зависшего codec/kernel I/O. После обоих
действий PID исчезает и upload/preparation temp directories удалены до cleanup
harness. При close приложение завершилось с code0; новый import не опубликован.

Дополнительно `--fail-after-encoder` намеренно ломает harness при живом encoder.
Каждый запущенный процесс имеет собственную process group; failure cleanup
останавливает группу GUI с дочерними процессами и удаляет только private staging.
Проверены отсутствие PID и staging после ожидаемого failure. Это recovery
самого harness, не приёмка forced-exit/crash приложения.

Повторный прогон потребовал явного ожидания focused GTK location entry перед
вводом path: отображение диалога ещё не гарантирует готовность поля. Это
исправление harness readiness; production UI не менялся.

Native ambiguous publication/disconnect, close во время publication,
полный focus/reflow/AT-SPI/speech media panel, Wayland и физический playback
ещё не квалифицированы. [Evidence](evidence/2026-10-11-native-media-import.json).

## Миниатюра сохранённого ролика — 2026-10-12

Native bridge загружает validated clip из daemon и декодирует первый кадр
в144×144PNG. Один decoder, lease удерживается до reap; cancel/window shutdown
и abort waiter не освобождают admission раньше worker. GUI показывает только
выбранный ролик, не меняет LCD; stale replies игнорируются, Blob URL освобождается.
Ошибка preview допускает retry и не блокирует выбор для LCD.

[Checkpoint](evidence/2026-10-12-media-thumbnail.json):128browser tests,
Rust chain658tests до добавленного abort regression; новый regression отдельно
прошёл. Native MP4/GIF preview и fresh preview после restart прошли. Первый
native run остановился на GTK/AT-SPI chooser readiness; повтор без изменения
кода прошёл. Полная gallery, cache, delete/slots и hardware остаются вне slice.

## Удаление media: защита хранения — 2026-10-12

`MediaAssetStore::acquire` атомарно загружает ролик и возвращает `MediaPin`.
Копии pin удерживают файл до завершения последнего потребителя. Проверка
ссылок и unlink в `delete_unreferenced` сериализованы с acquire/import/load;
ошибка чтения ссылок запрещает удаление. Корректно адресованный испорченный
regular record можно удалить, symlink/невалидный key нельзя. Удаление освобождает
quota; transient thumbnail load не удерживает ролик после чтения.

На первом checkpoint это была только внутренняя основа. Следующий checkpoint
ниже подключил её к runtime и IPC/CLI/GUI. Требования интеграции:

- [x] передача pin через preparation → LCD actor и manager restart replay;
- [x] копия pin в очереди settings и retention до durable commit/final flush,
  включая retry/ошибку записи; Clear не должен освобождать pending reference;
- [x] строгий scan media references всех offline settings;
- [x] scan media slots и pin при Save до durable publication (checkpoint ниже);
- [x] проверка active/sleep references, save failure/retry и clear-before-commit;
- [ ] native transport loss во время удаления;
- [x] IPC/CLI/GUI delete с явным подтверждением выбранного ролика,
  readback при ambiguous result и сохранением текущего LCD source.

Все операции одного root должны использовать клоны общего store. Два
независимых экземпляра или внешний редактор файлов не синхронизируются этим
process-local guard. `is_referenced` выполняется под gate и не должен повторно
обращаться к store; писатель новой ссылки обязан удерживать pin до commit.

## Удаление media через GUI и CLI — 2026-10-12

`delete_prepared_clip(key)` использует общий store, runtime pins и strict offline
reference scan. Выбор/replay получает pin атомарно с load; actor передаёт копию
в settings queue. Clear снимает in-memory reference, но durable scan продолжает
защищать прежний ключ до записи. Нечитаемый settings файл запрещает удаление.

`llcontrol lcd delete-clip KEY` удаляет только managed record. GUI подтверждает
имя выбранного ролика; изменение выбора отменяет подтверждение. После результата
список перечитывается, потерянный ответ не приводит к автоматическому повтору.
Focus возвращается в список или кнопку обновления. Исходный файл не затрагивается.

[Checkpoint](evidence/2026-10-12-media-delete.json):665Rust/131browser,
fmt/strictClippy/docs, check/build и native delete/reimport/restart. Первый native
run прошёл. Media slots ещё не реализованы; добавление требует расширить reference
scan и удерживать pin до публикации профиля. Карточки/defaults и hardware открыты.

## Named media slots — 2026-10-12

ProfileContent::ScreenMedia использует существующую группу screen и слоты0–7.
Сохранение ролика заменяет содержимое выбранного слота; theme/media в одном
slot одновременно не допускаются. RPC Save приобретает pin до profiles.edit,
который отпускается только после возврата durable write, включая ошибки.
Media delete сканирует profiles.json строго; corrupt/unknown library запрещает
удаление. Lock order не инвертирован: Save приобретает pin до profile lock,
Delete проверяет pin под media gate и затем читает profile store.

GUI сохраняет выбранный library clip в слот, загружает snapshot отдельно от
черновика темы и применяет только отдельной кнопкой. CLI profiles apply screen
использует set_lcd_media для media variant. Reset заменяет слот базовой темой;
если ролик ещё выбран устройством, это не снимает device reference.

[Checkpoint](evidence/2026-10-12-media-slots.json):666Rust/133browser и native
save/load/Apply/Clear/reset/delete/reimport. Race Save/delete не создаёт dangling
profile; missing clip отклоняется без изменения revision. Native fixture теперь
отвечает на запрос PN0 firmware; harness ждёт enabled reset после Clear.
Первый native run имел известный GTK/AT-SPI chooser failure; он не исправлен.
Открыты gallery cards/defaults, long video, video+sensor composition и hardware.


## Карточки видео и очередь миниатюр — 2026-10-12

Карточки страницами по8; выбор синхронизируется со списком без Apply.
Metadata refresh сохраняет просматриваемую страницу. Общая очередь
декодирует по одному запросу; отменённая активная задача удерживает admission
до завершения native invoke, queued задачи удаляются без dispatch.
При размонтировании освобождаются object URLs. Compact error допускает retry.

135browser tests, Svelte check(0/0), frontend build, native GUI/MockBackend build
и isolated native media workflow прошли. Native проверил реальные144px FFmpeg
миниатюры двух карточек, выбор без playback, возврат к selected preview,
profile/delete/restart/cancel/close. [Evidence](evidence/2026-10-12-media-gallery.json).
Первоначальный browser failure исправлен ожиданием реального teardown hidden
DOM после asynchronous details toggle; production cancellation для этого не менялась.

Общая image/video gallery/defaults, long video, video+sensor composition,
полная native keyboard/screen-reader приёмка и hardware остаются открытыми.
Историческая GTK/AT-SPI chooser flakiness не объявляется исправленной.


## Сохранённые изображения: gallery и повторное использование — 2026-10-12

`list_theme_assets` перечисляет только managed32hex PNG, исключая symlinks и
неизвестные имена; scan ограничен10000entries. `thumbnail_theme_asset` читает
bounded bytes и выдаёт PNG не более144px с сохранением aspect/alpha.
Preview не создаёт session pin: browsing не меняет eligibility очистки.
RPC выполняет blocking image work под существующим theme_jobs permit.

GUI показывает страницы по8, отдельную ошибку каждого preview и явное добавление
в черновик без Apply. При закрытии/смене страницы поздний результат игнорируется,
дальнейшие запросы старой страницы не отправляются, object URLs освобождаются.
Исходные имена исторически не сохранялись: карточки подписаны коротким ключом.

[Checkpoint](evidence/2026-10-12-image-gallery.json):667Rust/96targets,
fmt/strictClippy/docs/production check,frontend check/build,137browser tests;
isolated native WebKit проверил import→gallery→draft без изменения LCD и
повторил video/profile/delete/restart/cancel/close. Hardware не запускался.
Первый focused browser run имел неверное название существующей кнопки Apply;
после исправления test locator оба новых сценария прошли.

Остались единая image/video gallery, имена/rename, defaults и точечное удаление
изображений; консервативная bulk cleanup уже есть. Полная native accessibility,
long video/composition и физическая приёмка открыты. Три active plans не закрыты.


## Имена изображений и rename — 2026-10-12

GUI import сохраняет File.name; отдельный sidecar version1/revision/name не
меняет PNG/key и ссылки theme/profile. Legacy без sidecar читается как name=null,
revision0. Rename сериализован с import/cleanup, требует expected_revision и
пишет metadata через temporary→fsync→rename→directory fsync. Неизвестные/corrupt
metadata не перезаписываются. Cleanup также удаляет metadata изображения.

GUI показывает имя и редактирует его отдельно от LCD; conflict/lost reply
сохраняет введённое значение и вызывает readback без автоматической повторной
записи. После ошибки требуется заново выбрать переименование на актуальной
ревизии. Legacy list/import API сохранены; named list — новый RPC.

[Checkpoint](evidence/2026-10-12-image-names.json):669Rust/96targets,
fmt/strictClippy/docs/production check,frontend check/build,139browser tests.
Native WebKit проверил import name→rename revision2→restart и прежние media
workflows. Первый run остановился на GTK/AT-SPI GIF chooser с пустым tree;
чистый retry прошёл без изменений кода. Chooser flakiness остаётся открытой.
Hardware не запускался. Единая gallery/defaults, точечное image delete,
long-video/composition и полная accessibility остаются.


## Persistent media v2 — контейнер, 2026-10-12

[Формат и интеграционные gates](persistent-media-v2.md): indexed single-file
storage, atomic no-clobber staging, bounded file reader по AU. Общая проверка
AU выделена из diagnostic clip; старые240AU/1MiB не ослаблены.
[Checkpoint](evidence/2026-10-12-media-file.json):672Rust/97targets,
fmt/strictClippy/docs/production check. Синтетический VP9 MP4480×480/24fps/586frames
перекодирован и восстановлен через file reader; decoder подтвердил586frames,
payload>1MiB. Полная GUI/native/hardware проверка не запускалась: UI не менялся.

Это ещё не пользовательский long-video runtime: container не подключён к
MediaAssetStore, chunked IPC, actor, thumbnail/profile/replay или GUI import.
Следующие gates — owner-controlled publication и bounded blocking reader worker
с pins/joined teardown. Три активных плана остаются открытыми.


## Media store v1/v2 и pins — 2026-10-12

Store распознаёт `.json`v1 и `.lcm`v2, возвращает общий metadata list, отклоняет
дубли ключа и делит quota64. `create_file`/`publish_file` поддерживают internal
staging с повторной quota check при commit; public chunked IPC ещё отсутствует.
`PinnedMedia` владеет legacy clip либо file reader и удерживает pin до закрытия;
cloned pin сохраняет protection для pending persistence. Profile Save использует
общий acquire_source и держит pin до commit; IPC delete проверяет saved references.

[Checkpoint](evidence/2026-10-12-media-store-v2.json):674Rust/97targets,
fmt/strictClippy/docs/production check. Mixed formats/restart/read/delete, quota
race, duplicate keys, corrupt records и IPC v2 saved-profile protection проверены.
При редактировании тестовый файл был случайно перезаписан; восстановлен из
истории с точным SHA-256 прежнего checkpoint, затем новые тесты добавлены отдельно.
Все прежние тесты сохранены. Browser/native/hardware не запускались: UI не менялся.

Actor/replay/thumbnail/native import ещё не используют v2; GUI limits прежние.
Следующий шаг — chunked publication и bounded blocking reader worker→actor.

## Текущая интеграция v2 — 2026-10-12

После store foundation подключены reader→LCD actor/profile/replay/sleep и
`media_upload` chunked IPC. [Актуальный контракт и gates](persistent-media-v2.md)
заменяют очередь предыдущих checkpoint: остаются native preparation/upload,
thumbnail, GUI outcome/progress/cancel и fault acceptance. Native import пока
использует прежний diagnostic manifest с10s/240frames/1MiB; новое хранилище и
RPC сами по себе этот пользовательский лимит не снимают.

## File-backed preparation — 2026-10-12

`crates/llcontrol-media/src/media_prepare.rs::prepare_file` готовит persistent media
в owned temporary H.264 file. В памяти остаются ограниченный packet report/index
и один AU; `read_unit` выполняется на blocking worker. Бюджеты: вход256MiB,
encoded256MiB,86400frames/1hour, AU128KiB, packet report16MiB. Probe исходника
ограничен60s, encoding и packet probe —3600s каждый; cancellation kill/reap
сохраняется. Это бюджеты Linux, не измеренные Windows limits.

Encoder допускает один дополнительный кадр и размер выше payload budget,
после чего код отклоняет превышение; это предотвращает принятие молча
обрезанного по budget ролика. Packet ranges непрерывно покрывают файл;
каждый AU проверяется перед возвратом результата и при последующем чтении.
File descriptor закрывается до удаления временного каталога.

Синтетический VP9-in-MP424fps/586frames/24,416667s проходит prepare_file,
покадровую сборку и реальное декодирование всех586frames; payload>1MiB.
Byte-budget rejection, cancel на стадии validation и Drop cleanup проверены.
Legacy `prepare` остаётся адаптером с10s/240AU/1MiB. MediaImports и GUI пока
используют этот legacy adapter; следующий шаг — file owner через publication
и поэлементный `media_upload` с outcome/readback.

## GUI long-video integration — 2026-10-12

File preparation подключена к MediaImports и chunked IPC. Пользовательский
import теперь до1hour/256MiB source и256MiB encoded; старый diagnostic adapter
сохраняет10s/240AU/1MiB. Native chooser→VP9 586frames→v2→полный mock loop/clear,
restart и decoder586frames [проверены](evidence/2026-10-12-native-long-import.json).
Transferring отменяем; Publishing ждёт результата, потерянный ответ проверяется
по тому же ID через Status/Cancel. Native publication fault/close/accessibility
и physical acceptance остаются открытыми. Следующий functional slice — overlays.
