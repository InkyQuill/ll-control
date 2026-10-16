# Архив checkpoints реализации

Перенесено из implementation-status.md 2026-10-14 без удаления результатов.
Это исторические snapshots отдельных срезов; текущие остатки — в
[сводке](implementation-status.md), roadmap и общем чеклисте.

## Исторические checkpoints

**Общий барьер fan+lighting actors реализован:** freeze ACK ждёт предыдущие HID
и settings deltas. Оба refresh и все mutations приостановлены, read-only state
доступен; unsealed cancellation освобождает actors, sealed unknown outcome
оставляет их удержанными до recovery/restart. Проверены предыдущий I/O/settings,
обе очереди, все mutation variants, sensor refresh, late/foreign tickets, Drop и
shutdown.
[Контракт/evidence](profile-groups.md#fanandlighting-общий-барьер-actors).
**Actor preflight/install/Apply готов:** проверяются typed settings и known sensor
units; подготовка и установка не отправляют HID/settings. Apply сохраняет imported
curve/manual source, inactive RGB/Dynamic и global thermal history при missing
samples. Ошибки записи отражены в state; retry не создаёт settings deltas.
Workspace1003 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Runtime-stage evidence](evidence/2026-10-14-profile-groups-install.json).
Связь с durable commit/receipt, reapply и group transport ещё не подключена;
legacy unset/explicit manual0 и расширенная lifecycle matrix остаются открытыми.


**Fan+lighting barrier, до staged runtime:** общий ticket, ожидание старого HID/
settings, запрет mutations/refresh, read-only state, unsealed/sealed Drop и
shutdown. Три actor regressions; workspace999 passed/2ignored, strict Clippy/
rustdoc/fmt. [Evidence](evidence/2026-10-14-profile-groups-barrier.json).


**Screen runtime и H.264 software rotation start/replay/group подключены:**
Prepared group commit/Apply/reapply остаётся serialized LCD transaction с receipt
до первого frame и защитой поздних edits. Final composition поворачивается
0/90/180/270 перед encoder вместе с live sensor overlays; wire H264 rotation0.
Managed media сохраняет ID и не становится synthetic theme в API. Обычные IPC
start и manager restart replay применяют угол; brightness сохраняет orientation.
Возврат к JPEG восстанавливает hardware angle. Current source identity/receipt
reapply после restart проверены для90/180/270.
Workspace992 passed/2ignored, strict Clippy/rustdoc/fmt прошли. Actual decoded
H264 quadrant/overlay pixels проверены для четырёх углов; private manager/IPC
replay и group actor проверили identity и protocol packets. Это не hardware acceptance.
**Live angle существующего composition подключён:** typed atomic angle меняется
без пересоздания worker, generation и decoder. Общий render→AU budget ограничивает
накопление старых кадров в codec/OS pipes. Actual decoded64-frame video проверяет
непрерывность фазы и ограниченный хвост старого угла после паузы потребителя;
private IPC проходит все четыре угла без лишнего LCDControl и с restart replay.
Workspace993 passed/2ignored; strict Clippy, rustdoc и fmt прошли.
[Live rotation evidence](evidence/2026-10-14-display-live-rotation.json).


Статусы «ещё отсутствует» ниже относятся к моменту конкретного checkpoint;
актуальные остатки — в roadmap и чеклисте.

**HTTP shutdown усилен и проверен:** после воспроизведения двух новых сбоев
listener явно останавливается через shutdown перед drop, затем сервер ждёт
клиентов и возвращает возможную ошибку. Регрессия доказывает, что остановка
работает даже при удерживаемой копии дескриптора;20 полных library-прогонов
по61 тесту прошли. Конкретный процесс, удерживавший listener при исходном
сбое, не установлен: fork/exec остаётся гипотезой, а не доказанной причиной.
[Evidence](evidence/2026-10-12-media-http-shutdown.json).
Добавленная зависимость GStreamer сама по себе не является recorder:
ScreenRecord UI отсутствует; recording engine добавлен следующим checkpoint. Hardware/native GUI не запускались.

**Screenshot portal подключён:** capture возвращает bounded raw bytes в image
editor; preview и Save остаются отдельными действиями. Cancel/error сохраняют
прежний draft. Private-bus tests покрывают ранний ответ, cancel/reject/timeout
Close; file admission и browser workflow проверены. Реальный compositor,
compositor selection/lifecycle ещё требуют приёмки. Native raw-byte IPC →
editor/preview/Save, portal Cancel, editor Close и WM close с ожиданием cleanup
[прошли с подменным порталом](evidence/2026-10-12-native-screenshot.json).
Owned capture ID, ранняя отмена, unmount/Close cleanup и shutdown joins уже
реализованы;56 library +2 binary Rust tests и12 browser tests прошли вместе
с strict Clippy/docs/fmt и frontend check/build.
[Cancellation checkpoint](evidence/2026-10-12-screenshot-cancellation.json).
ScreenRecord — отдельный сценарий ниже. [Screenshot contract](screen-capture.md).

**ScreenCast negotiation реализован:** выбор одного advertised monitor/window,
CreateSession/SelectSources/Start, owned PipeWire FD, стабильный serial дляv6,
cancel/Close и монитор Session::Closed. Private-bus tests покрывают success,
cancel, malformed stream metadata, remote failure и закрытие во время Start.
Recording engine добавлен последующим checkpoint; GUI пока отсутствует. [Контракт](screen-record.md).

**ScreenRecord engine реализован:** GStreamer worker, owned portal FD/stable
stream target, MP4 staging, pause/resume/stop, frame/active-time и output-size
budgets, LimitReached pause, EOS/Null cleanup и joined cancellation.
Synthetic FFprobe decode проверяет число кадров и отсутствие pause gap;
sparse updates, повторный Resume, shortened limit, size failure и SessionClosed
также проверены. Tauri/GUI, region/clear/review/publication, completely stalled
source, реальные portals и полный180s ещё не приняты.
[Checkpoint](evidence/2026-10-12-screen-record-engine.json).

**ScreenRecord job owner и Tauri commands подключены:** один caller-known ID,
резервация до открытия portal, status recovery, bounded controls и независимый
cancel, удержание admission до join, готовый MP4 до явной отмены. Оба builders
и main capability зарегистрированы; app Close ждёт локального cleanup.
Synthetic tests покрывают потерянные ответы, Selecting cancellation, повторные
Start/Finish, controls, failed open, удаление готового файла и shutdown.
Remote Close errors сохраняются в Failed status/log; actual portal/native
qualification остаётся. GUI и пользовательский Save workflow пока отсутствуют; native bridge и
preview добавлены следующими checkpoints. [Checkpoint](evidence/2026-10-12-screen-record-owner.json).

**ScreenRecord → prepared review подключён:** синхронный admit удерживает
готовый MP4 без промежуточной копии/дискового I/O; отдельный Prepare запускает
кодирование только принятого задания. Capture cancellation не удаляет уже
удерживаемый input, который освобождается после подготовки; ReviewReady не
публикует asset до media_confirm. Отмена admission отклоняет поздний Prepare.
Существующие gallery leases и mutable upload разнесены по InputStorage.
GUI, редактирование и native Save workflow ещё не подключены; backend
original-source preview добавлен следующим checkpoint.
[Checkpoint](evidence/2026-10-12-screen-record-review.json).

**ScreenRecord original-source preview реализован:** Ready-only lease, bounded
HTTP Range чтение готового MP4 и metadata после FFprobe. Регрессия проверяет
точные байты диапазона, отзыв URL после cancel и очистку metadata.
Итоговая проверка desktop Rust:76 library +2 binary tests, strict Clippy,
default check и rustdoc прошли. Это synthetic/backend результат; GUI, реальный
PipeWire и аппаратный LCD не проверялись. Прерывание именно внутри metadata
probe не имеет отдельного детерминированного fault test.
[Checkpoint](evidence/2026-10-12-screen-record-preview.json).

**Frontend lease для ScreenRecord подготовлен:** типизированный клиент
восстанавливает Begin/Start по ID, сериализует Close после pending admission,
отбрасывает поздний status и позволяет повторить неудачный cleanup. Шесть
mock IPC regressions и frontend check прошли. Svelte GUI ещё не подключён; пользовательский сценарий остаётся открытым.
Отдельный RecordingReview client добавлен следующим срезом: admission/readback,
prepare, explicit confirm и close с ожиданием admission. Всего10 mock IPC
проверок и frontend check прошли; UI/native workflow ещё не проверен.
[Review client checkpoint](evidence/2026-10-12-screen-record-review-client.json).
[Checkpoint](evidence/2026-10-12-screen-record-client.json).

**Начальный ScreenRecord GUI подключён:** библиотека роликов → start/pause/
resume/finish → исходный MP4 → fit/name → prepared review → явное Save и
membership. Clear с подтверждением отменяет обе leases, stale completions
после clear отбрасываются. Region/edit, автоматический review на180s и native
workflow ещё открыты; предыдущие упоминания отсутствующего GUI — checkpoints
до этой интеграции. Реальный desktop/LCD не захватывались.

**Актуальная сводка editor:** image geometry/шесть filters, video cut/crop,
record/pause, source/review, gallery re-edit и AutoFill stretch реализованы.
VideoEdit brightness/contrast подключены к функции подготовки целого ролика
и фрагментов; native admission/DTO, UI controls и локальный preview подключены. Windows/native qualification остаётся.
Разделы ниже фиксируют состояние на момент каждого checkpoint: фразы
«ещё не подключено» в ранних разделах не отменяют последующие реализации.
Нового общего software/native/hardware прогона при аудите планов не было.

**VideoEdit range UI завершён:** свежий upload проходит inspect/readback,
после чего пользователь выбирает непрерывный диапазон; gallery использует
managed metadata. Подготовка и сохранение — отдельные действия с review.
Исправлены declaration-order ошибки и реактивность owned job ID. Frontend
check/build и41 browser tests прошли; guards, потеря inspect reply, cancel
и unmount проверены. [Checkpoint](evidence/2026-10-12-video-range-ui.json).
Native WebKitGTK/Windows SDK/physical qualification остаётся.
Предыдущая [сверка WIP](evidence/2026-10-12-plan-audit-range-wip.json)
сохраняет состояние до исправления, а не текущее состояние frontend.

**Native prepared preview исправлен:** вместо custom scheme UI получает
scoped loopback URL с токеном,4 clients,8KiB headers,5s network deadlines и
256KiB disk reads. Полные HTTP ranges стримятся без загрузки всего файла в RAM;
shutdown ждёт обслуживаемые запросы. Fresh VideoEdit GTK chooser → metadata →
fractional range/brightness → review → явное Save прошёл в WebKitGTK2.54.1
с software decoder. Сохранённый файл:12frames, RGB255/255/255 в центре.
Полный native media harness завершён,52Rust/41browser/check/build/Clippy/docs
прошли. [Checkpoint](evidence/2026-10-12-native-video-edit-http.json).
Последующий gallery checkpoint ниже закрывает source/re-edit и seek; fault matrix, большие native media workloads,
Windows qualification и hardware остаются. Предыдущий
[failed run](evidence/2026-10-12-native-video-edit-failure.json) сохранён как история.

**Native gallery VideoEdit принят:** managed source → decode/seek → диапазон
0.1…0.35 с → prepared review/seek → Save заменяет запись коллекции и сохраняет
исходный файл. Проверены шесть кадров с красным центром, возврат фокуса и
удаление нового файла после снятия ссылки коллекции. Устранено лишнее обновление
каталога после завершённого задания, которое мешало вернуть фокус.
Frontend check/build, 42 browser tests и полный изолированный native media
harness прошли. [Evidence](evidence/2026-10-12-native-gallery-video-edit.json).
Record/pause/crop interaction, faults, большие workloads, Windows и hardware
не закрыты этим прогоном; нового полного workspace checkpoint нет.

**Native Record/Pause принят в ограниченном сценарии:** реальный source playback
формирует два интервала; draft блокируется во время записи, pause фиксирует
время исходника. Склеенный результат декодируется, длительность соответствует
интервалам с допуском двух выходных кадров; cancel не публикует файл.
Последующие single-range Save/replacement и весь media harness также прошли.
[Evidence](evidence/2026-10-12-native-record-pause.json). Сохранение именно записанной
нарезки, автоматический stop/лимит, spatial pixel equivalence и faults остаются.
AT-SPI chooser flakiness наблюдалась в неуспешных попытках и не считается исправленной.

**Native recorded-cut Save и source-end stop приняты:** тестовый исходник
имеет синюю левую и красную правую половину. Первый фрагмент завершается ручной
паузой, второй автоматически по концу исходника. После подготовки/Save файл
прочитан из daemon store и декодирован целиком: все синие кадры предшествуют
красным. Подтверждены независимые numeric crop, порядок, длительность, замена
membership, сохранность исходника и cleanup. Полный native media harness и
native build прошли. [Evidence](evidence/2026-10-12-native-record-save.json).
Лимит180с, pointer/keyboard crop, fault matrix, Windows и hardware остаются.
Предыдущий раздел описывает более раннюю приёмку review/cancel.

**Исторический image point-filter checkpoint:** hue/saturation/brightness/contrast реализованы;
79 focused Rust tests, 13 browser tests, strict Clippy/docs и frontend check/build
прошли. Ошибка float64 shader-oracle исправлена, 140 f32-векторов проверены.
[Evidence](evidence/2026-10-12-editor-point-filters.json). Предыдущие 780 Rust/173
browser относятся к gallery checkpoint; нового полного workspace прогона нет.
Sharpen/blur добавлены последующим [checkpoint](evidence/2026-10-12-editor-spatial-filters.json):
84 focused Rust (83 в общем прогоне и1 дополнительная регрессия) и14 browser, strict Clippy/docs/frontend check/build. Windows
raster, non-96-DPI и native editor остаются открытыми.
История смешанного чеклиста вынесена в [checkpoints](basic-parity-checkpoints-2026-10-12.md);
[активный чеклист](basic-parity-checklist.md) теперь содержит только остатки.

## Gallery video re-edit: подготовлен reader

Конечный pinned source stream для legacy/v2 media реализован; 14 integration
и125 library tests прошли (1 ignored), strict Clippy/docs/fmt прошли.
Admission ограничен двумя потоками; ошибка/EOF/drop освобождают pin, отмена
не прячет продолжающийся blocking read. IPC подключён последующим checkpoint; native staging/gallery UI ещё
не подключены. [Evidence](evidence/2026-10-12-gallery-video-export.json).
Это отдельный checkpoint, без нового общего workspace/native/hardware прогона.
Следующий [IPC checkpoint](evidence/2026-10-12-gallery-video-export-ipc.json):
56 IPC tests +3 focused daemon tests, workspace check, strict Clippy/docs/fmt.
Проверены finite transfer586units, disconnect под backpressure, source corruption
и explicit failure. Полного workspace test/native/hardware прогона не было.

## Gallery source download

Desktop finite receiver пишет исходник на диск, валидирует framing/order/totals
и возвращает PreparedFile только после Complete. Dedicated IpcClient теперь
останавливает reader/writer при Drop, включая full notification queue.
Реальные12frames сохранены побайтово и remuxed в MP4; failure cases не выдают
partial source. State/commands и gallery UI ещё не подключены.
[Checkpoint](evidence/2026-10-12-gallery-video-download.json):30 GUI tests,
57 IPC tests и daemon export integration прошли; strict Clippy/docs/fmt и
workspace check прошли. Native/hardware не запускались.

## Managed gallery source

Source state, cancel/readback/shutdown, Tauri commands и bounded source range
protocol подключены. Cancel не освобождает admission раньше worker; Ready cancel
удаляет файлы и отзывает URL. Close ждёт source вместе с imports/thumbnails.
[Checkpoint](evidence/2026-10-12-gallery-source-state.json):34 GUI tests,
strict Clippy/docs/fmt и native-smoke compilation прошли. UI selection/edit/save
ещё не подключены; native runtime и hardware не запускались.

## Gallery source → подготовка/review

Native lease сохранённого MP4 подключён к общему prepare_review через
media_source_prepare; source preview можно закрыть после admission без удаления
нужного input. Результат использует существующие confirm/publication.
[Checkpoint](evidence/2026-10-12-gallery-source-prepare.json):35 GUI tests,
strict Clippy/docs/fmt и native-smoke compilation прошли. Real12frame source
после выбора250..500ms даёт6frames, вход освобождается после подготовки.
UI галереи и replacement ещё открыты; native/hardware не запускались.

## Gallery video editor UI

Повторное редактирование пользовательского видео подключено к source preview,
cut/crop/fit, prepared review и explicit confirm. Revision-checked replacement
меняет membership; старые assets/references сохраняются. Конфликт не повторяет
мутацию, новый файл остаётся в каталоге. Builtin edit запрещён.
[Checkpoint](evidence/2026-10-12-gallery-video-editor-ui.json):32 browser tests,
frontend check/build,1 focused range Rust test и strict Clippy прошли.
Native gallery editor/hardware и оставшиеся VideoEdit/AutoFill controls открыты.

## AutoFill placement

По Windows GA2LCD source добавлен Fit::Stretch: весь кадр растягивается480×480,
SAR1; отдельная опция и canvas preview в import/re-edit. MP4/GIF pixel tests
отличают stretch от crop/pad. [Checkpoint](evidence/2026-10-12-video-autofill.json):
36 GUI Rust tests,21 browser, frontend check/build и strict Clippy/docs/fmt.
Native/Windows timing и pixel qualification открыты; Linux storage H26424fps
не заявляет совместимость с Windows VP9/GIF archive. VideoEdit SDK effects остаются.

## VideoEdit channel kernel

Восстановлены SDK Contrast→Lightness, f32 и nearest-even SIMD quantization.
131072 numerical values сверены с экспортами установленной MFP64 DLL в
изолированном emulator; Rust LUT совпадает. FFmpeg raw RGB тест5 пар также
совпал. [Checkpoint](evidence/2026-10-12-video-adjustments-kernel.json):2 focused
Rust tests, strict Clippy/docs/fmt. Preparation/UI ещё не подключены;
scalar/tail, Windows pipeline/native/hardware qualification остаются.

## VideoEdit effects в кодировании

`prepare_file_edited` применяет Contrast→Lightness до crop/resize ко всему
исходнику или каждому выбранному интервалу. Нулевые параметры не добавляют
цветовое преобразование. Real codec regression сравнивает результат с SDK
oracle, проверяет каждый кадр двух фрагментов, чёрные поля и побайтовую
эквивалентность neutral path. [Checkpoint](evidence/2026-10-12-video-effects-preparation.json):
37 GUI library tests, all-target/all-feature tests, strict Clippy/docs/fmt.
Admission/DTO/UI остаются следующим шагом; native/hardware не запускались.

## VideoEdit native admission

Оба native commands (`media_begin`, `media_source_prepare`) принимают optional
`adjustments`: brightness/contrast 0..255, default0. `EditOptions` фиксируется
при admission, общий worker применяет фильтры к upload и leased gallery source.
Невалидные числа/типы/unknown fields отвергаются Rust DTO. Старые вызовы
сохраняют нейтральную обработку. Real-codec tests проверяют готовое видео
для upload и cut leased source после отзыва исходного preview.
[Checkpoint](evidence/2026-10-12-video-effects-admission.json):38 library +2 binary
Rust tests, all features/targets, strict Clippy/docs/fmt. UI ещё не передаёт параметры;
нативное окно и железо не запускались.

## VideoEdit UI и source preview

Цветокоррекция включается отдельно от выбора фрагментов; brightness/contrast0..255
имеют sliders/numeric inputs, reset и валидацию. Whole-video и gallery re-edit
передают immutable native options; обработка требует review и explicit Save.
Source canvas применяет f32 LUT до crop/resize, не затрагивая чёрные поля.
Preview LUT совпадает с131072 SDK oracle values и composed cases. Browser
coverage проверяет pixels, validation, reset, payload, review/cancel и gallery cuts.
Для надёжного первого кадра source player использует preload=auto.
[Checkpoint](evidence/2026-10-12-video-effects-ui.json). Native WebKitGTK, Windows
colorspace/codec/raster и большие кадры остаются отдельной квалификацией.

## VideoEdit time semantics

Source audit уточнил continuous slider без frame snapping; tick используется
для range guards и reverse playback. VideoEdit default stop=duration−1/FPS,
export minimum=2/FPS, SDK получает Start/Stop без явного+tick. Эти guards пока
не реализованы в отдельном Linux VideoEdit workflow и не распространяются на
VideoCut/AutoFill. Regression проверяет fractional interval1101..1600ms на
MP4 30000/1001FPS со start_time0/+5s:12 ожидаемых синих кадров.
[Checkpoint](evidence/2026-10-12-video-time-boundaries.json).

## Native video source metadata

Общий ffprobe теперь отдаёт dimensions, source duration и раздельные rational
average/nominal frame rates. `inspect_file` пригоден для owned staged input;
preparation использует тот же probe. Нет подстановки output24fps вместо
неизвестной исходной частоты. MP430000/1001 со start0/+5s подтверждает duration2.002,
rates, cancellation и сериализацию. 40 library +2 binary tests, strict Clippy,
docs/fmt прошли. [Checkpoint](evidence/2026-10-12-video-source-metadata.json).
Managed metadata state/readback/Tauri/UI ещё не подключены.

## Gallery metadata readback

Managed source теперь выполняет inspect после remux и публикует metadata
в status атомарно с Ready. Cancel очищает поля, новый/failed source не выдаёт
старые сведения. Ownership/reap удерживаются до окончания worker. Real fixture
проверяет480×480,0.5s,24/1 и статус после cancel/следующей ошибки.
[Checkpoint](evidence/2026-10-12-gallery-source-metadata.json):40 library +2 binary
tests, strict Clippy/docs/fmt. Fresh-upload metadata admission и range UI открыты.

## Fresh upload inspection state

MediaImports.inspect добавляет SourceReady/metadata без encoding и держит input
для дальнейшей подготовки. Configure разрешён только до worker admission;
readback не перезапускает probe. Cancel/error/shutdown очищают input/metadata,
blocked worker удерживает admission до завершения. Real upload→inspect→configure→
review проверен по декодированным pixels; добавлены ownership/failure regressions.
[Checkpoint](evidence/2026-10-12-upload-source-inspection.json):42 library +2 binary
tests, strict Clippy/docs/fmt. Tauri/UI и VideoEdit single-range ещё открыты.

## Inspected source Tauri commands

`media_inspect` и `media_prepare_inspected` зарегистрированы для main window в
обоих builders/ACL. Owned task сохраняет завершение при lost invoke; final options
и input забираются атомарно, duplicate request не переписывает работающий job.
Standalone configure_source удалён. Native библиотека и команда всегда требуют
review перед публикацией. UI распознаёт SourceReady, но single-range workflow
ещё не подключён. [Checkpoint](evidence/2026-10-12-inspected-media-commands.json):
43 library +2 binary tests, strict Clippy/docs/fmt, frontend check/build.
Browser/native runtime/hardware в этом checkpoint не запускались.

## VideoEdit native single-range

Continuous range DTO/guards и отдельное кодирование подключены к inspected-upload
и gallery-source commands. Range несовместим с VideoCut; источник проверяется
native probe, unknown FPS не подменяется24. Maximum duration−tick/minimum2ticks,
обычный1hour resource budget. Invalid inspected range сохраняет source для retry.
Linux average→nominal FPS policy требует отдельной Windows/VFR qualification.
[Checkpoint](evidence/2026-10-12-video-edit-range.json):46 library +2 binary tests,
strict Clippy/docs/fmt; реальные дробные range и effects проверены. UI ещё открыт.

## Реализация

| Область | Уже есть | Где проверять |
|---|---|---|
| Foundation / M0 | Framing/codecs, HID, CLI/IPC, versioned Windows provenance и 33 сценария | `crates/lconnect-protocol`, [матрица](lcdv2-2.1.28-matrix.md), [baseline](../2026-09-08-reference-2.1.28-and-ll-connect3.md) |
| M1 | JPEG refresh, diagnostic C/512 H.264, один HID owner, frame batches, bounded queues, barriers, joined teardown, stable device identity | `crates/lconnect-daemon/src/actor`, [capture](../protocol/captures/2026-09-08-lcdv2-m1.md) |
| M2 sensors | hwmon CPU/GPU/DIMM temperatures, procfs CPU/RAM, AMDGPU/NVIDIA load/VRAM/power, RAPL, CPUFreq/NVIDIA/Intel clocks, AMDGPU voltage, network RX/TX, filesystem capacity, LCDV2 coolant/RPM, typed units, stable bindings, 3s freshness | `crates/lconnect-daemon/src/sensors.rs`, `crates/lconnect-ipc/src/sensors.rs`, tests/host_sensors.rs |
| M2 cooling | PWM/RPM fixed/curves, pump Quiet/Performance, fan Quiet/Standard/High/Full, reference arithmetic, fallback/hysteresis, motherboard pump source, replay/manual override | daemon `actor/cooling.rs`, IPC `cooling.rs`, daemon tests/cooling*.rs |
| M2 thermal RGB | CPU/GPU alerts, приоритет, 10s manual grace, restore/retry, mode/direct overlap, один RGB owner | daemon `actor/thermal.rs`, tests/thermal.rs и lighting_owner.rs |
| Theme renderer | Version 1, background, image/text/sensor/clock layers, 480×480, Unicode DejaVu Sans/Serif/Mono + Noto Sans TC Regular/Bold + Abril/Caprasimo/Cubic11 + user fonts, gradients, line boxes, alignment/size/color/shadow, alpha, crop/contain/stretch, °C/°F | IPC `theme.rs`, image `theme.rs`, tests/theme.rs |
| Theme runtime | Daemon-owned JPEG 1 Hz из свежих caches, preview без HID, apply/replace/clear, persistence/replay активной темы, явная ошибка missing asset | daemon `theme_runtime.rs`, `theme_assets.rs`, `actor/lcd_keeper.rs`, tests/theme_runtime.rs и theme_ipc.rs |
| Import / CLI | Bounded image normalization, managed PNG assets, theme import/preview/apply/state | image `assets.rs`, CLI `commands/theme.rs`, [workflow](lcd-theme-workflow.md) |
| Desktop | Tauri 2 + Svelte 5, Display/Sensors/Cooling/Lighting/Settings, explicit Apply, restricted IPC bridge, image import, one-shot/live preview и persistence status | [desktop](../../desktop/README.md), `desktop/src`, `desktop/src-tauri` |
| Layouts | Редактируемые Focus/Duo/Triple/Quad/Cycle/Clock, явный выбор и порядок sensor bindings | `desktop/src/lib/theme-presets.ts`, `desktop/tests`, `desktop/scripts/preset-contract.ts` |

[Политика schemas](schema-compatibility.md) определена: legacy v1 читается с defaults,
неизвестные версии/поля отклоняются без перезаписи; future-version migration engine не заявлен. Активная тема и библиотека профилей сохраняются отдельно; slots store/IPC/CLI
уже добавлены, GUI CRUD/load draft/slot reset и консервативный asset cleanup реализованы. [Profiles](profiles.md). Preview
поддерживает ручное и непрерывное обновление. Есть Triple preset и sensor_cycle со сменяемыми показателями. Восемь bundled fonts, user TTF/OTF import/selection, managed references и font cleanup, opacity и shadow уже доступны; эквивалентность каждому Windows font не заявлена. Focus/Duo/Triple/Quad/Cycle/Clock не заменяют
23 страницы Windows effects/Style Express. Thermal alert не заменяет RGB-004.

Обычный sensor-driven RGB реализован в daemon/IPC/persistence и CLI/GUI;
focused клиентские/RPC tests и native enable/disable/restart/applied прошли.
Browser interrupted mutation acceptance закрыт; hardware и полный mode/sync
каталог остаются открытыми. [Поведение и проверенные сценарии](sensor-driven-rgb.md).

## Сохранённые результаты проверок

Исторический software checkpoint — [font cleanup](evidence/2026-10-10-font-cleanup.json):
546 Rust tests, 42 browser tests, fmt/strict Clippy/docs, frontend check/build
и native smoke. Сверка планов подтвердила 15/15 source hashes и наличие native
artifact; тесты повторно не запускались. Ниже перечислены более ранние
результаты с собственными границами проверки.

- M1: JPEG >70 секунд и замена, видимое движение H.264, исправление периодических
  лагов sensor polling. 30 минут: 42 018 frames, 23,34 sent fps, zero errors,
  без пауз >100 ms, sensor age ≤0,96 s, RPC p95 3,90 ms, RSS +188 KiB после
  первой минуты. Пользователь подтвердил плавность. [Полная запись](../protocol/captures/2026-09-08-lcdv2-m1.md).
- M2 checkpoint: 416 workspace tests, fmt, strict Clippy, docs и release build.
  Это исторический checkpoint до theme/desktop изменений, не итоговая проверка дерева.
- Theme replay checkpoint: 430 workspace tests, fmt, strict Clippy и docs.
  Mock JSON-RPC проверяет preview без apply, invalid theme без замены, restart,
  replay, отмену сохранённой темы ручным изображением и missing asset после restart.
- Desktop checkpoint: native build, Svelte check без ошибок/предупреждений,
  frontend production build, GUI strict Clippy и два Rust bridge tests.
  Последний preset checkpoint: шесть headless Playwright flows с mocked Tauri
  invoke, четыре frontend preset проходят настоящий Rust theme validator;
  image tests и focused Clippy прошли. Это не native WebKitGTK → IPC → mock HID E2E.
- Полная цепочка проверок на окончательном состоянии ещё нужна. Числа тестов
  разных checkpoint не складываются и не доказывают hardware parity.

## Исторические результаты ранних фаз

Phase 0a–0e и Phase 1 реализовали основу; подробные captures сохранены в
[protocol/captures](../protocol/captures), codecs и findings — в
[galahad2-lcdv2](../protocol/galahad2-lcdv2.md) и
[cross-device survey](../protocol/cross-device-survey.md).
Старые standalone наблюдения PWM/RGB не заменяют приёмку нынешнего autonomous
cooling/thermal runtime. Апрельское описание fan wiring расходится с поздним
сообщением пользователя: текущее подключение требуется установить заново.
Cmd15 принимал данные без видимого экрана; production offline support не заявлен.
Ранние предположения о самоотключении LCD не являются текущей lifecycle policy.

Результаты M1/M2 и активные остатки перенесены из старых implementation plans.
Phase 2 GUI plan заменён текущей desktop архитектурой, а не объявлен выполненным:
curves/rendering принадлежат daemon. Specs оставлены как design references.
ENE 6K77xx plan соседнего checkout отложен вне LCDV2; updater другой семьи
не подтверждает готовность LCDV2 firmware updates.

Аппаратные проверки новых композиций, охлаждения, RGB и lifecycle остаются
в [чеклисте](basic-parity-checklist.md). Реальные тесты отложены пользователем.

## Исторические checkpoints

Следующие записи описывают состояние на момент отдельного изменения; фразы
«ещё не реализовано» в них не переопределяют актуальную сводку выше.

### Profiles foundation checkpoint, 2026-10-10

Добавлены durable named slots store (screen8/fan4/pump4), bounded schema,
revision conflicts, IPC list/edit и CLI CRUD/apply. Workspace tests прошли;
после последующего CLI error handling — два focused CLI tests, итоговые strict
workspace Clippy/all-targets/all-features, docs warnings denied и fmt прошли.
Границы: reset/asset cleanup ещё нужны; реального HID не было.

### Profiles GUI и slot reset, 2026-10-10

GUI Display/Cooling поддерживает list/save/load draft/rename/delete/reset.
Reset одного слота восстанавливает reference Quiet с explicit sensor binding
или собственную базовую screen theme. Соседние slots и active device state
не меняются. Это не полный Windows reset групп/фирменных assets.
Хранение после restart, ошибка без binding, revision conflicts и отсутствие
автоматического apply проверены в Rust и headless UI. Asset cleanup остаётся
открытым. Промежуточные profiles записи удалены из активного плана.

Reset checkpoint: workspace tests, strict workspace Clippy all-targets/all-features,
docs warnings denied и fmt прошли; Svelte check 0 errors/warnings, 8 headless
UI tests и frontend production build прошли. Логи `/tmp/reset-{workspace,clippy,
doc,ui,build,check}.log`. Native GUI и реальные HID tests не запускались.

### Theme cancellation prerequisite, 2026-10-10

Preview/preparation/accepted Apply теперь владеют semaphore permit до
окончания background job или LCD actor reply. Отмена waiter не снимает лимит;
подготовка, завершившаяся после отмены, не запускает следующую device mutation.
Добавлены regressions на отмену ожидания, передачу permit между стадиями и
освобождение после ошибки. Focused theme integration и полный daemon test
suite, strict daemon Clippy/all-targets/all-features, docs warnings denied и
fmt прошли. Логи `/tmp/theme-cancel-{tests,daemon,clippy,doc}.log`.
Это необходимая часть lifecycle; asset cleanup ещё не реализован.

### Asset cleanup, 2026-10-10

Добавлены IPC `cleanup_theme_assets`, CLI `theme cleanup [--delete] [--keep KEY]`
и GUI dry-run/delete. Strict roots scan сохраняет references всех profiles и
device settings, включая offline устройства. Manager и IPC разделяют session
pins: import/load удерживают keys до завершения процесса. External drafts
после restart требуют keep или сохранённого профиля; это описано в workflow.
Cleanup работает только с импортируемыми 32-hex PNG, не следует symlinks,
возвращает partial delete failures. Corrupt/unknown roots блокируют удаление.

Проверки: offline profile/device references после restart, current session
и draft retention, dry-run, corrupt roots и гонка load/delete. Workspace tests,
strict workspace Clippy all-targets/all-features, fmt, docs warnings denied
прошли; Svelte check без ошибок/предупреждений, 9 headless UI tests и production
frontend build прошли. Логи `/tmp/assets-gc-{workspace,clippy,doc,ui,check,build}.log`.
Реальный daemon/HID не запускался.

### RGB-004 evaluator, 2026-10-10

Windows source traced: 1Hz, six sensor sources, integer input/ranges, 0–4 HSV
color slots and interpolated speed; brightness/direction fixed from High.
Strict config and pure evaluator added; four focused regressions, IPC suite,
strict daemon/IPC Clippy, warning-free docs and fmt passed.
[Detailed provenance and remaining owner integration](sensor-driven-rgb.md).
Runtime/IPC control/GUI/persistence are still pending; no RGB hardware writes.

### RGB-004 owner integration, 2026-10-10

Dynamic pump policy подключена к sole RGB owner, settings delta/replay и
get/set RPC. Mode-specific color count/speed validation; thermal precedence,
manual pump/direct cancellation, fan independence, stale fallback и retry.
Mock packet assertions проверяют четыре RGB slots и wire speed, startup
binding recovery; JSON-RPC missing config не отключает политику.
Workspace tests/strict Clippy/docs прошли; после state/error metadata доработок
15 thermal tests и theme IPC integration, strict daemon/IPC Clippy и fmt прошли.
Логи `/tmp/dynamic-owner-{workspace,clippy,doc,final-tests,final-clippy}.log`.
CLI/GUI integration и hardware acceptance ещё открыты.

### RGB-004 clients checkpoint, 2026-10-10

Исправлены RGB JSON key order, привязка draft к устройству и защита mutation
result от старого poll response. Добавлены два CLI tests, один headless UI flow
и RPC/MockBackend enable/update/restart/disable/restart test.
CLI suite, strict CLI/daemon Clippy, fmt, Svelte check без diagnostics,
10 headless UI tests и frontend build прошли. Логи
`/tmp/dynamic-client-{tests,suite,clippy,daemon-clippy,ui,ui-check,build,ipc}.log`.
Никакого реального HID; native E2E остаётся открытым.

### AMDGPU provider checkpoint, 2026-10-10

Read-only collector дополнен GPU utilization и VRAM used/total/load из sysfs.
Stable physical IDs, bounded catalog, unavailable/invalid и freshness сохранены.
14 host sensor tests и полный daemon suite прошли; strict daemon Clippy,
warning-free docs и fmt прошли. Логи `/tmp/amdgpu-{tests,daemon,clippy,doc}.log`.
[Inventory и незакрытые providers](sensor-provider-inventory.md).
Реальный daemon/GPU/HID не запускался.

### NVIDIA provider checkpoint, 2026-10-10

Добавлен read-only bounded nvidia-smi subprocess provider, UUID bindings,
temperature/load/VRAM и typed unavailable/invalid. Deadline/output/catalog
limits, kill/reap и сохранение ID без stale значений при ошибке.
Полный daemon suite, затем 27 library tests с дополнительными provider regressions,
strict daemon Clippy, warning-free docs и fmt прошли. Логи
`/tmp/nvidia-{tests,daemon,clippy,doc,final-tests,final-clippy}.log`.
[Контракт и ограничения](sensor-provider-inventory.md). Hardware не запускался.

### CPU power checkpoint, 2026-10-10

RAPL package energy deltas → Watts добавлены в production collector;
registry/IPC/LCD theme/desktop принимают новую единицу. Средняя мощность,
warmup/reset/wrap/missing/stale без fake values; RGB/cooling temperature
consumers не принимают Watts. [Контракт](sensor-provider-inventory.md).
Workspace tests, strict workspace Clippy, warning-free docs, fmt,
Svelte check, 10 GUI tests и frontend build прошли; дополнительные focused
power/theme/UI regressions также прошли. Логи `/tmp/power-*.log`.
Hardware не запускался; Intel GPU и остальные исследовательские gates открыты.

### LCD Triple/Cycle checkpoint, 2026-10-10

Добавлены Triple layout и sensor_cycle (1–16 bindings, 1–3600s, default10s).
Daemon monotonic switching, CLI/IPC/persistence через существующий theme path,
GUI редактирование bindings/interval/labels/precision/units. Render tests
проверяют точные границы переключения, wrap и missing binding; RPC проверяет
replay циклического слоя. 12 UI tests и шесть frontend presets через Rust
validator прошли; Svelte check без ошибок, frontend build прошёл.
Логи `/tmp/cycle-*.log`.

Исправлена ошибка предыдущего CPU power отчёта: тогда frontend check не прошёл
из-за отсутствующего Watts suffix в formatSensor, хотя build был успешен.
Suffix теперь добавлен; текущий `/tmp/cycle-ui-check.log` подтверждает 0 errors/warnings.

Итоговый Triple/Cycle workspace tests, strict workspace Clippy, warning-free
workspace docs и fmt также прошли; дополнительный cycle RPC replay test прошёл.

### Live preview checkpoint, 2026-10-10

Добавлены optional timed-preview RPC, runtime frame_at и GUI polling с
monotonic elapsed, single in-flight, 250ms edit debounce, stale reply discard,
URL cleanup и stop на tab/disconnect/toggle. Новые preview не пишут на LCD.
13 GUI tests, Svelte check (0 errors/warnings), frontend build, workspace
tests/strict Clippy/docs и fmt прошли. Дополнительный test ухода/возврата
Display проверен отдельно; RPC test различает кадры Cycle на 0/10s и
подтверждает Idle до Apply. Логи `/tmp/live-*.log`.
Native IPC/WebKitGTK и hardware acceptance остаются отдельными gates.

### Text styling checkpoint, 2026-10-10

TextStyle получил backwards-compatible font/opacity/shadow; bundled DejaVu
Sans/Serif/Mono и bounded shadow mask, GUI настройки для всех текстовых слоёв.
Default JSON shape и прежний вид тем сохраняются. Renderer tests проверяют
различие шрифтов, тень, нулевую прозрачность, bounds; IPC test — old schema
round-trip; GUI test — выбор serif/shadow/opacity и отправленный Apply payload.
Workspace tests/strict Clippy/docs/fmt, Svelte check, 13 GUI tests, шесть
frontend→Rust theme contracts и frontend build прошли. Логи `/tmp/fonts-*.log`.
Точные 11 Windows fonts/CSS shadow остаются full-parity исследованием;
базовые настройки доступны. Реальное устройство не использовалось.

### Persistence retry checkpoint, 2026-10-10

Исправлена потеря dirty state после отказа debounced write: повтор через1s
без нового Apply, final shutdown flush и merge независимых deltas. Добавлен
parent-directory fsync после rename. Apply теперь сообщает queued save.
Regression tests блокируют devices path обычным файлом, затем устраняют отказ
и проверяют retry/последний flush. Daemon suite, strict daemon Clippy/docs,
fmt, Svelte check и пять Display/presets UI tests прошли.
Логи `/tmp/save-retry-*.log`. IPC/GUI pending/saved/error ещё требуется.

### Persistence status checkpoint, 2026-10-10

Dispatcher публикует pending/saved/last_error через watch; get_lcd_state
добавляет cached persistence snapshot отдельно от LCD actor state. GUI
показывает ожидание, отказ с retry и последнюю завершённую запись; начальное
состояние не выдаётся за подтверждённое сохранение. Это observation writer,
не barrier/ack конкретного Apply. Старые ответы без поля допустимы.
Failure/recovery regression проверяет состояние writer; IPC проверяет новое
поле без изменения LCD результата; GUI — error→saved и сохранение source Theme.
Workspace tests/strict Clippy/docs/fmt, Svelte check, 14 GUI tests и frontend
build прошли. Логи `/tmp/save-state-*.log`. Hardware не использовался.

### Native Display checkpoint, 2026-10-10

Tauri/WebKitGTK → production Rust bridge → Unix IPC → MockBackend прошёл
import/preview/DOM load-edit-Apply/synthetic sensor update. После выхода GUI
поступили ещё два JPEG frame starts; новый процесс daemon восстановил Theme
и конфигурацию с asset binding. [Evidence и воспроизведение](native-desktop-smoke.md).
Strict workspace all-targets/all-features Clippy и tests daemon/GUI прошли.
Native file picker, остальные страницы, reconnect/errors и hardware не проверены.

### Повторная сверка планов, 2026-10-10

Убраны устаревшие требования повторно реализовать NVIDIA, Triple/Cycle,
fonts/shadow, live preview и Display native smoke. Матрица, roadmap, README
и базовый план согласованы с кодом и сохранёнными результатами. В активной
директории остались три незавершённых плана: basic parity, M1 lifecycle и M2
providers/hardware. Ранее удалённые implementation plans не восстановлены;
их незакрытый объём (включая прежний Phase 2 GUI) сохранён в roadmap/gaps.
Новых запусков устройства или изменения production code эта сверка не делала.

### Display reconnect/draft checkpoint, 2026-10-10

Выбранное устройство сохраняется при исчезновении из catalog; Apply/LocalUI
и device controls недоступны до возврата или явного выбора другого устройства.
Поздний status старого selection не заменяет текущий LCD state; poll до Apply
не перезаписывает результат более нового Apply. Загрузка active theme не
затирает локальные правки, сделанные во время ожидания. Connection error
очищается при восстановлении связи отдельно от ошибки пользовательской операции.

17 headless GUI tests прошли, включая disconnect→missing device→reconnect,
редактирование во время theme load и смену устройства до ответа state RPC.
Svelte check: 0 errors/warnings; production frontend build и diff check прошли.
Native Display smoke повторно прошёл (`/tmp/lconnect-native-6a7vrf9b`).
Логи: `/tmp/reconnect-ui-final.log`, `/tmp/reconnect-check.log`,
`/tmp/reconnect-build.log`, `/tmp/reconnect-native.log`.
Промежуточный UI run пересёкся со сборкой и получил reload/reset fixtures;
итоговый UI run выполнялся отдельно без изменений файлов и прошёл полностью.

Native reconnect, keyboard/focus audit всех страниц и hardware acceptance
не закрыты этими результатами. Реальный daemon/HID не запускался.

### Image watermark shadow checkpoint, 2026-10-10

Windows 2.1.28 WatermarkConfig/Panel подтверждают наличие Shadow alongside
position/opacity. Добавлен optional image.shadow в version 1, bounded alpha-mask
renderer и GUI controls. Старые JSON без поля сохраняют прежнюю сериализацию.
Pixel regression проверяет alpha contour, общую прозрачность и blur; schema
regression — legacy round-trip и недопустимые offsets/blur/opacity.

Workspace tests, strict all-targets/all-features Clippy, warning-free docs,
fmt, Svelte check (0 errors/warnings), frontend build и 17 UI tests прошли.
Native smoke с image.shadow прошёл, включая GUI close/restart/theme equality:
[evidence](evidence/2026-10-10-watermark-native.json).
Логи `/tmp/watermark-*.log`; hardware не использовался.
[Controls inventory](lcd-theme-controls.md) фиксирует оставшиеся Windows CSS,
clock elements/colors и font/anchor вопросы без заявления полной equivalence.

### Clock parts and browser source checkpoint, 2026-10-10

Найден подключённый Assets/ga2v/app.8fc94db7.js; index/service/Core path и hashes
записаны в [controls inventory](lcd-theme-controls.md). Time содержит local
YYYY/MM/DD, 12h hh:mm, A.M./P.M. с тремя цветами. Watermark CSS semantics
установлены; center/percentage placement ещё требует Linux UX/renderer mapping.

Добавлены optional ClockFormat date/time_without_period/period с сохранением
legacy time, три Clock preset layers и independent style controls. Runtime
использует один local timestamp для всех частей. Pixel tests проверяют дату,
midnight/noon, leap year и неверные даты; schema — old JSON и enum validation.
18 UI tests, шесть frontend→Rust preset contracts, Svelte check, frontend build,
workspace tests/strict Clippy/warning-free docs/fmt прошли. Native smoke с date
layer прошёл GUI close и restart/replay: [evidence](evidence/2026-10-10-clock-native.json).
Логи `/tmp/clock-*.log`; аппаратного прогона не было.

### Watermark center placement checkpoint, 2026-10-10

Optional watermark_position.horizontal/vertical (0…100) добавляет Windows
center placement: положительные значения — проценты canvas, 0 — fallback22px.
Старый x/y rectangle сохранён по умолчанию; contain центрирует fitted image.
Off-canvas source alpha сохраняется до shadow blend в bounded padded mask,
поэтому тень невидимой части изображения может попадать на LCD.

Pixel regressions проверяют contain/0/50/100/clipping и off-canvas shadow;
schema regression — limits/round-trip, GUI — draft fields и Apply payload.
Workspace tests, strict all-targets/all-features Clippy, warning-free docs/fmt,
Svelte check, frontend build и 18 UI tests прошли. Native smoke с watermark
position прошёл GUI close и restart/theme equality:
[evidence](evidence/2026-10-10-watermark-position-native.json).
Логи `/tmp/position-*.log`. Реальный daemon/HID не запускался. Intrinsic-size
поведение Windows и subpixel browser equivalence не закрываются этим slice.

### Sensor parts and native image fit checkpoint, 2026-10-10

Optional Sensor/SensorCycle parts дают отдельные label/unit TextStyle, основной
style управляет value. Cycle меняет все части из одного binding; stale/missing
значение остаётся — с видимой подписью и единицей. Legacy combined JSON не
меняется. GUI exposes independent colors/position/size/font/alignment.
contain_native уменьшает слишком большой managed image, но не увеличивает
маленький, сохраняя центрирование и aspect ratio; обычный contain не изменён.

Pixel regressions: Fahrenheit value/unit split, разные цвета, missing value,
Cycle RPM→Watts label/unit switching, native-size/downscale image geometry.
Nested-style bounds и legacy serde tested. Workspace tests/strict Clippy/docs,
fmt, Svelte check, frontend build, 18 UI flows и 6 Rust preset contracts прошли.
Native smoke с parts + contain_native прошёл GUI close/restart/replay:
[evidence](evidence/2026-10-10-sensor-parts-native.json).
Логи `/tmp/parts-*.log`; hardware не запускался.

### Text layout checkpoint, 2026-10-10

TextStyle получил optional max_width/line_height/vertical_align с сохранением
legacy output/JSON. Wrap предпочитает whitespace, длинные слова делятся по
Unicode graphemes; явные пустые строки сохраняются. Vertical anchor top/center/
bottom учитывает весь block. Unicode segmentation1.13.3 уже присутствовал в
lock как transitive dependency, теперь image crate использует его напрямую.

Tests: word/space/combining/emoji/blank-line segmentation, rendered equivalence
wrapped↔explicit lines, center/bottom anchors, schema bounds и UI Apply payload.
Workspace tests/strict all-targets/all-features Clippy/warning-free docs/fmt,
Svelte check, frontend build, 18 UI flows и 6 frontend→Rust preset contracts
прошли. Native smoke с text layout прошёл GUI close/restart/replay:
[evidence](evidence/2026-10-10-text-layout-native.json).
Логи `/tmp/wrap-*.log`. Реальный daemon/HID не запускался.
Windows font catalog установлен (11 families); exact catalog/TC glyph coverage
остаются открытыми, bundled DejaVu не считается их эквивалентом.

### Native controls checkpoint, 2026-10-10

Native smoke расширен на Sensors/Cooling/Lighting/Settings: real DOM inputs,
preset→draft→Apply, pump/fan applied, LCD patch isolation, manual pump/fan
bases, thermal config, dynamic enable/disable/enable. Restart проверяет полные
requested configs и resumed cooling/dynamic applied без fallback/errors.
[Результаты и границы](native-desktop-smoke.md).

Первый прогон корректно отклонил thermal config без fan lighting base; GUI
теперь объясняет настройку обеих зон для restore. Full existing UI suite18
прошёл, затем focused controls suite3 (включая новый error/retry regression)
прошёл. Svelte check, frontend build, native build/smoke и diff check прошли.
Изменены smoke scripts и GUI copy; новый полный workspace suite не запускался.
Логи `/tmp/controls-*.log`. Реальный daemon/HID не использовался.

### Control drafts checkpoint, 2026-10-10

Cooling отбрасывает поздние Apply/preset ответы после смены device; preset
также сохраняет правки, сделанные во время загрузки. Lighting сохраняет thermal
draft при reconnect и запрещает Apply к другому device до загрузки его правил.
Generation/revision guards отделяют poll и mutation; ошибки старого устройства
очищаются при смене connection/device.

Сохранённые логи: full UI suite 22 passed (`/tmp/control-drafts-ui.log`), после
последней правки очистки ошибок check 0 errors/warnings, frontend build и focused
control-drafts/controls 6 passed. Native build и пятистраничный smoke повторно
прошли: [evidence](evidence/2026-10-10-control-drafts-native.json), включая
2 JPEG frames после GUI close и restart replay/applied. Логи
`/tmp/control-drafts-*.log`, fixture `/tmp/lconnect-native-bo6ac51o`.
Это результаты предыдущего запуска, проверенные при аудите планов; тесты в
аудите заново не запускались. Native reconnect и аппаратные сценарии не проверены.

### LCD binding applicability checkpoint, 2026-10-10

Прослежены восемь полей LCDV2 service payload до active browser consumers.
CPU Power нужен Steampunk, RAM — Robot/Radio; enum из шести значений не задаёт
полный набор данных экрана. Product UI callers подтверждают применимость этих
эффектов. [Inventory и source evidence](sensor-provider-inventory.md#закрытая-сверка-lcd-sensor-payload--2026-10-10).
Все восемь типов имеют Linux providers, но Intel load coverage, реальные
драйверы и полный desktop dashboard inventory остаются открытыми. Effects
не объявляются реализованными. Это source audit без запуска daemon/HID/tests.

### Intel ABI research checkpoint, 2026-10-10

Определены отдельные PMU контракты i915 busy ns и xe active/total ticks,
memory region queries и критическое ограничение unprivileged i915 accounting.
Сканирование fdinfo отклонено как источник глобальной загрузки/VRAM: это
client-scoped статистика. Источники закреплены revision/hash manifest;
implementation steps и регрессии добавлены в inventory/M2/checklist.
Provider ещё не реализован; hardware и daemon не запускались. Docs-only audit.

### i915 engine PMU checkpoint, 2026-10-10

Добавлен production read-only i915 per-engine collector на безопасном perf-event
API, без изменения forbid(unsafe_code). Stable physical/config IDs, PMU cpumask,
bounded discovery, warmup/stale/reset, unavailable при denied/read failure;
handles закрываются при исчезновении/ошибке/reconfiguration. No global load
aggregation, xe или VRAM claims: они остаются в активном M2.

Read-only Rust review выявил выбор неверного CPU на старых kernels и возможность
принять non-busy config; оба замечания исправлены и покрыты regression fixtures.
Все 176 daemon tests (включая 7 новых Intel tests), strict all-targets/all-features
Clippy для daemon, warning-free daemon docs, fmt и diff check прошли.
Логи `/tmp/i915-{daemon-tests,clippy,doc}.log`. Только fake counters/filesystem;
реальные perf syscalls/daemon/HID не запускались. Полный workspace/final acceptance
остаётся отдельным gate. Подробный контракт — в sensor-provider-inventory.md.

### Settings/library request lifecycle checkpoint, 2026-10-10

Settings использует connection/device generation: запоздавшие success/error не
показываются на другом устройстве; brightness/rotation draft сохраняется.
Profiles инвалидирует library при reconnect/group change и отбрасывает старые
refresh/edit responses; имя и screen draft сохраняются. Cleanup инвалидирует
scan при изменении набора keep assets или connection, требует нового scan.
Эти guards не отменяют уже принятую daemon мутацию; итог читается refresh.

Svelte check 0 errors/warnings, frontend production build, все 26 UI flows и
повтор native happy-path smoke прошли. Первый UI run выявил неверный exact-label
locator нового теста ориентации; заменён на combobox role, второй полный run
зелёный. [Native evidence](evidence/2026-10-10-library-races-native.json), логи
`/tmp/library-races-*`; native fixture `/tmp/lconnect-native-d3jzjsvj`.
Native error/reconnect и profile/cleanup flows этим smoke не закрываются.
Изменения только GUI/tests/docs; kernel PMU и реальный HID не использовались.

### Dynamic RGB connection generation checkpoint, 2026-10-10

Mutation responses проверяются по generation устройства/connection/open-state,
а не только по device ID. Старый success/error после A→B→A либо disconnect/
reconnect больше не заменяет текущий state и notice. Draft остаётся локальным;
poll после восстановления читает актуальный daemon state. Старые errors очищаются
при смене context. Это не отмена уже принятой команды.

Svelte check 0 errors/warnings, frontend build и три focused Dynamic RGB browser
теста (два новых плюс существующий workflow) прошли. Native happy paths/restart
повторно прошли: [evidence](evidence/2026-10-10-dynamic-races-native.json).
Логи `/tmp/dynamic-races-*`. Full UI suite не повторялся: в предыдущем checkpoint
26 тестов прошли; здесь проверен изменённый компонент. Native reconnect, concurrent
manual/dynamic matrix и аппаратные RGB сценарии остаются открытыми.

### Lighting UI mutation coordination checkpoint, 2026-10-10

DynamicRgb busy передаётся в Lighting через explicit bindable prop. Общий guard
блокирует manual/thermal Apply во время dynamic mutation; parent busy блокирует
dynamic mutations и инвалидирует его polling context до следующего fresh read.
Draft inputs сохраняются. Повторные вызовы защищены и в обработчиках; finally
освобождает блокировку после success/error. Это один GUI panel, не lock против
других IPC clients.

Два новых browser сценария проверяют dynamic success/failure → manual Apply,
обе стороны блокировки, свежий выключенный dynamic state и сохранённый threshold.
Все 30 UI tests, Svelte check 0 errors/warnings, production frontend build и
повтор native happy-path/restart smoke прошли:
[evidence](evidence/2026-10-10-lighting-serialization-native.json).
Логи `/tmp/lighting-serial-*`; fixture `/tmp/lconnect-native-6084q6bz`.
Native concurrency/error/reconnect и реальные LEDs остаются отдельной приёмкой.

### Native profile lifecycle checkpoint, 2026-10-10

WebKitGTK smoke расширен DOM операциями screen save/rename/load/reset/delete/
resave и независимыми pump/fan save/load. Проверены полные содержимые профилей,
восстановление изменённого локального draft при load и отсутствие device Apply
от операций библиотеки. Python после restart сравнивает всю library, включая
revision и все три profile groups, с результатом GUI.

Пересобраны desktop-mock и feature GUI; расширенный native run прошёл:
[evidence](evidence/2026-10-10-native-profiles.json), fixture
`/tmp/lconnect-native-hkv6fv6i`, логи `/tmp/native-profiles*`.
GUI-close frames, theme и controls replay/applied gates также прошли.
Frontend implementation не менялась, browser suite здесь не повторялся.
Profile conflict/missing assets/sensors, native reconnect и cleanup остаются
незакрытыми. Реальный HID и пользовательский display не использовались.

### Native profile revision conflict checkpoint, 2026-10-10

Smoke моделирует отдельного IPC client, который переименовывает screen slot
после загрузки library в GUI. Устаревший GUI rename получает реальную ошибку
revision conflict. Проверены отсутствие overwrite внешней правки, сохранение
theme/name drafts, отсутствие device Apply; refresh очищает ошибку и получает
новую revision, повторный rename проходит. Последующий общий native suite,
profile/controls/theme restart replay и applied gates также прошли.

[Evidence](evidence/2026-10-10-native-profile-conflict.json), fixture
`/tmp/lconnect-native-_nxuugho`, логи `/tmp/profile-conflict-*`.
Изменён только feature smoke JS, пересобран GUI, diff check прошёл.
Browser suite и hardware не запускались; profile missing assets/sensors и
native connection failures остаются незакрытыми.

### Native missing profile dependencies checkpoint, 2026-10-10

Native bridge/RPC: profile save и theme Apply с несуществующим asset отклоняются,
полная library/revision и active theme не изменяются. Native DOM: pump slot с
unknown sensor загружается как draft с явным unavailable selector; Apply получает
реальную validation error и сохраняет working policy. Загрузка допустимого slot
и повторный Apply восстанавливают нормальный workflow; временный slot удаляется.

Первое ожидание fallback для нового неизвестного binding было неверным: actor
валидирует наличие Celsius source до принятия новой policy. Тест исправлен по
этому контракту. Потеря уже принятого источника остаётся отдельным fallback test.

Расширенный native run, все предыдущие profile/theme/control сценарии и restart
прошли: [evidence](evidence/2026-10-10-native-profile-missing.json), fixture
`/tmp/lconnect-native-tjag90bd`, логи `/tmp/profile-missing-*`.
Изменён только smoke JS; GUI пересобран, diff check прошёл. Frontend/Rust behavior
не менялось; browser suite и реальное железо не запускались.

### Keyboard navigation checkpoint, 2026-10-10

Root GUI получил skip-link к focusable main, aria-current для активной страницы
и общий видимый focus-visible outline, включая дочерние controls. Browser test
управляет реальной клавиатурой: Tab/Enter skip, Shift+Tab к Settings, load current,
slider ArrowLeft → brightness Apply и select ArrowDown → rotation Apply. Проверяет
focus/outline/current page и точные IPC patches без мыши.

Svelte check 0 errors/warnings, production build, полный 31-test UI suite и
native regression smoke прошли. [Native evidence](evidence/2026-10-10-keyboard-native-regression.json),
логи `/tmp/keyboard-*`, fixture `/tmp/lconnect-native-ngsuldo5`.
Native smoke подтверждает прежние DOM/IPC сценарии, не native keyboard navigation.
Остальные страницы/editor keyboard, contrast и assistive technology acceptance
остаются открытыми. Реальный HID не использовался.


### Layer keyboard и повторная сверка планов, 2026-10-10

В текущем дереве editor использует keyed layers и восстанавливает focus после
reorder/delete: заголовок перемещённого/оставшегося слоя либо кнопка добавления
текста после удаления последнего. Сохранённые `/tmp/layer-focus-check.log`,
`/tmp/layer-focus-build.log`, `/tmp/layer-focus-ui.log` подтверждают Svelte check
без ошибок/предупреждений, production build и один focused Playwright test.
Последующая регрессия зафиксирована в checkpoint ниже.

Повторная документальная сверка убрала выполненные LCD binding inventory,
обычное сохранение screen profiles и native profile happy/error paths из
формулировок оставшейся реализации. Basic plan содержит только открытые задачи;
M1/M2 сохранены из-за незакрытых gates. Нового software/hardware прогона при
этой сверке не выполнялось. Исторические checkpoints выше не являются текущим
списком задач и не заменяют итоговую приёмку дерева.


### Layer focus full regression checkpoint, 2026-10-10

После изменения layer focus весь browser suite прошёл: 32 теста за 36,6 s
(`/tmp/layer-full-ui.log`). Пересобраны desktop-mock и GUI с native-smoke и
custom-protocol; изолированный WebKitGTK → Unix IPC → MockBackend smoke прошёл.
Проверены прежние theme/control/profile сценарии, ошибки профилей, продолжение
JPEG после закрытия GUI и theme/control/profile restart replay/applied.

[Evidence с hashes затронутых файлов](evidence/2026-10-10-layer-focus-native-regression.json),
fixture `/tmp/lconnect-native-v1no0gy1`, логи `/tmp/layer-native-*.log`.
Native smoke не проверяет keyboard navigation; полный workspace acceptance,
native reconnect и аппаратные gates остаются открытыми. Новых изменений
поведения в этом прогоне не было.


### Native Display reconnect checkpoint, 2026-10-10

Smoke сохраняет несохранённый theme draft и помечает готовность через fixture
screen profile. Python останавливает собственный daemon, выдерживает 4 s
(более двух GUI polling intervals) и запускает его с прежним isolated storage.
GUI обязан увидеть offline, disabled Apply/load и сохранённый draft, затем
reconnect. Проверяется полный прежний saved theme без автоматического Apply
черновика; explicit load и Apply через восстановленный bridge проходят.
Маркер профиля затем убирается. Harness завершает GUI process group и при
неожиданной ошибке, не только при timeout.

Повторный прогон после уточнения status selector и cleanup прошёл вместе со
всеми предыдущими native сценариями и последующим restart/replay/applied.
[Evidence](evidence/2026-10-10-native-display-reconnect.json), fixture
`/tmp/lconnect-native-q6pukark`, логи `/tmp/reconnect-native-*.log`.
Менялись только smoke scripts, GUI пересобран; production behavior не менялось.
Browser suite повторно не запускался. Другие страницы, in-flight mutation
interruption, USB reconnect и физическое железо этой проверкой не покрыты.


### Five-page native reconnect checkpoint, 2026-10-10

Harness теперь последовательно перезапускает свой isolated MockBackend daemon
при открытой каждой из пяти страниц. Display проверяет load/Apply после reconnect;
Cooling сохраняет draft fan PWM=56 при requested=55; Settings сохраняет draft
64%/90° при requested=65%/180°. Lighting сохраняет manual color, CPU threshold=81
и dynamic high=61, не меняя сохранённые thermal=80 и dynamic=60. Все соответствующие
Apply offline отключены. Sensors показывают unavailable/—, затем valid °C с тем
же fixture:cpu identity. Ни один черновик не применяется автоматически.

Расширенный native suite прошёл вместе с profile/theme/control error paths,
GUI-close JPEG stream и последующим restart/replay/applied. [Evidence](evidence/2026-10-10-native-controls-reconnect.json),
fixture `/tmp/lconnect-native-9za55opl`, логи `/tmp/controls-reconnect-*.log`.
Менялись только smoke scripts; GUI пересобран, Python/JS syntax и diff check
прошли. Browser suite повторно не запускался. In-flight mutation interruption,
USB hotplug и hardware не покрыты. Сохранение manual RGB draft проверено в DOM;
полный wire replay manual config этим отдельным assertion не заявляется.


### Native lost mutation reply checkpoint, 2026-10-10

Добавлен test-only Unix proxy между GUI и isolated MockBackend daemon.
Он пересылает JSON-RPC/hello, для brightness=66 ждёт успешный daemon response,
затем закрывает клиентский socket без доставки ответа. GUI показывает response
dropped, сохраняет draft и не показывает success. Последующее чтение через
восстановленный bridge и GUI polling подтверждают requested=66. Proxy считает
ровно одну такую запись и один потерянный ответ: автоматического retry нет.
Явное Apply=65 успешно восстанавливает настройку и workflow.

Первый запуск выявил несовпадение fixture root и XDG_RUNTIME_DIR. Стенд исправлен:
GUI получает собственный помеченный isolated root рядом с daemon root; native
safety assertion сохранён. После исправления расширенный native suite, пять
page reconnects, profile error paths и финальный restart/replay/applied прошли.
[Evidence](evidence/2026-10-10-native-lost-reply.json), fixture
`/tmp/lconnect-native-njgkiexg`, логи `/tmp/lost-reply-*.log`.
Менялись только smoke scripts; GUI пересобран. Browser suite не повторялся.
Timeout и потеря ответа других mutation families, USB и hardware не проверены.


### Native write timeout checkpoint, 2026-10-10

Test-only Unix proxy удерживает один успешный ответ brightness=67, продолжая
пересылать остальные ответы/чтения по тому же соединению. GUI действительно
дожидается production bridge deadline 10 s (native assertion >=9,5 s), показывает
Request timed out; applied state must be checked, сохраняет draft и не показывает
success. RPC и GUI polling подтверждают requested=67: timeout не означает rollback.
Proxy считает одну запись и один удержанный response; auto-retry отсутствует.
Explicit Apply=65 после timeout проходит и возвращает исходную настройку.

Расширенный native suite, включая потерянный ответ, reconnect пяти страниц,
profile scenarios, GUI close и restart/replay/applied, прошёл.
[Evidence](evidence/2026-10-10-native-write-timeout.json), fixture
`/tmp/lconnect-native-s9y61gv2`, логи `/tmp/write-timeout-*.log`.
Изменены только smoke scripts; GUI пересобран, Python/JS syntax и diff check
прошли. Browser suite не повторялся; hardware не использовался. Это проверка
общего bridge через LCD patch, не всех отдельных mutation workflows.


### Schema compatibility and preservation checkpoint, 2026-10-10

Устранён сброс неизвестной device schema к defaults. Read теперь возвращает
ошибку для неподдерживаемых/повреждённых/необычных/oversized files, валидирует
embedded theme; write валидирует outgoing schema и перечитывает существующий
файл перед заменой. Неизвестные поля settings sections, CoolingConfig и Rgb
отклоняются вместо потери при сериализации. Read legacy v1 без version не меняет
bytes; defaults добавляются в памяти и сохраняются только при явной записи.

Regression fixtures проверяют точную сохранность исходного файла после отказа,
в том числе будущую theme version, неизвестные вложенные поля, dangling symlink
и размер >1 MiB. Manager attach получает ошибку вместо defaults/replay; recovery
и ограничения внешних параллельных правок описаны в [политике](schema-compatibility.md).

Финальная Rust chain прошла: fmt, strict Clippy all-target/all-feature,
486 tests (0 failed/ignored), docs. Read-only review не нашёл оставшихся blocking
findings после исправления CoolingConfig/Rgb. Native smoke с прежними profile,
reconnect/lost reply/timeout и restart scenarios также прошёл.
[Evidence](evidence/2026-10-10-schema-protection.json), fixture
`/tmp/lconnect-native-ga4uihvj`, логи `/tmp/schema-review-*-final.log` и
`/tmp/schema-native-*.log`. Это checkpoint, не итоговый release build/полная
приёмка цели. UI implementation не менялась; hardware не использовался.


### System Information source inventory checkpoint, 2026-10-10

[Source audit](system-info-inventory.md) подтверждает Mini picker/factory из
11 tiles и ResourcePage clock/storage controls. В Linux остаются GPU power,
CPU/GPU voltage/frequency, network rates, storage и FPS/mini-window workflows.
Они записаны в планы, не исключены из parity из-за отсутствия в LCD payload.
GPULoad wrapper Windows выбирает один HWiNFO sensor, не агрегирует engines;
точная underlying driver equivalence по-прежнему неизвестна. BAML string tokens
использованы только для corroboration, не как полная декомпиляция bindings.

Сохранены hashes 11 source artifacts. Remaining Spec/settings/XAML inventory
явно открыт. Код не менялся, software/hardware tests не запускались; ссылки и
diff check прошли. Это source evidence, не runtime observation Windows GUI.


### GPU hwmon power checkpoint, 2026-10-10

Добавлены отдельные input/average Watts bindings для GPU hwmon amdgpu/nouveau/
i915/xe при наличии standard power attributes. Read-only microWatt conversion,
physical identity, explicit disabled/invalid/zero, отсутствие fallback между
измерениями и общие cache freshness/size limits. Proprietary NVIDIA power,
физическая accuracy и whole-board equivalence этим не закрываются.

16 host sensor tests прошли, включая новые provider cases и расширенный
cache→IPC→formatted Watts→JPEG preview fresh/stale regression. Read-only Rust
review без findings; fmt, strict workspace Clippy, 488 tests (0 failed/ignored)
и docs прошли. [Evidence](evidence/2026-10-10-gpu-hwmon-power.json).
UI/IPC schema не менялись: Watts уже поддерживался; native GUI и hardware
в этом checkpoint не запускались. Остальные sensor gaps сохранены в плане.


### NVIDIA power and plan reconciliation checkpoint, 2026-10-10

NVIDIA bounded query добавляет `power.draw` как Watts со stable UUID ID.
Unsupported/invalid/zero различаются; отдельная ошибка power не портит
температуру/load/VRAM. Fixtures проверяют process arguments, CSV и LCD text,
collector cache и shutdown. Исправлена опечатка UUID в новом cache assertion.
Итоговая независимая Rust review: без findings; fmt, strict workspace Clippy,
490 tests (0 failed/ignored) и docs прошли.
[Evidence](evidence/2026-10-10-nvidia-power.json). Реальный driver/GUI/HID
не запускались, hardware accuracy остаётся открытой.

Активные M1/M2/basic планы сверены: выполненные schema policy, native reconnect
и GPU power не числятся задачами реализации. Индекс очищен от повторяющихся
промежуточных записей. Все три плана ещё содержат открытые задачи; дополнительных
полностью закрытых планов для удаления нет. Общая software приёмка и полный
roadmap остаются незавершёнными. Локальные ссылки и diff whitespace проверены.


### AMDGPU voltage/clocks checkpoint, 2026-10-10

Документированные in0/in1 и freq1/freq2 добавлены как Volts/Hertz: отдельные
GPU/Northbridge rails, GFX/memory clocks, stable physical identity и explicit
missing/disabled/invalid/zero. Catalog Hz отображается в MHz в GUI/LCD; V
не зависит от temperature units. Existing typed schemas принимают новые units,
старые binaries с такими темами несовместимы и должны сохранять strict rejection.

Fixtures покрывают renumber/disappearance, отсутствие ложного CPU binding,
cache→IPC→formatted value/JPEG, stale/unit mismatch; UI проверяет Sensors и
выбор обоих bindings для Duo с последующим Apply. 493 Rust tests, fmt, strict
Clippy, docs прошли. Frontend check (0 errors/warnings), build и 10 cross-language
themes прошли; 32 прежних UI tests плюс новый focused test прошли. Новый UI test
первоначально искал английские navigation labels; исправлены на реальные русские.
[Evidence](evidence/2026-10-10-amdgpu-voltage-clocks.json). Native WebKitGTK и
аппаратный driver здесь не запускались. CPU/другие GPU coverage остаются в плане.


### CPUFreq checkpoint, 2026-10-10

Read-only CPUFreq provider подключён к production collector; constructor fixtures
остаются изолированными, explicit root позволяет тестировать без настоящего sysfs.
Три разных Hz binding hardware current/average/driver reported сохраняют
семантику источника. Identity определяется sorted related_cpus с offline CPUs;
policy renumber не влияет, topology change не переназначает старые IDs.
Catalog/scalars bounded; ambiguity/error/missing не превращаются в fake zero.

496 Rust tests (0 failed/ignored), fmt, strict Clippy и docs прошли; независимая
review без findings. Интеграция проверяет cache→IPC→MHz и JPEG preview CPU/GPU
по отдельным IDs; stale CPU clock даёт unavailable. Исправлен прежний test selector,
который выбирал первую строку Hertz вместо ожидаемого GPU.
[Evidence](evidence/2026-10-10-cpufreq.json). Frontend/IPC schema не менялись;
GUI и реальные CPUFreq/HID не запускались. CPU voltage, остальные GPU providers
и оставшаяся software acceptance остаются в базовом плане.


### NVIDIA current clocks checkpoint, 2026-10-10

Bounded query добавляет current graphics/memory clocks с отдельными UUID IDs.
MHz → Hertz в catalog → MHz в LCD/GUI; прежние IDs не меняются.
Fixtures проверяют exact query args, layout, multi-GPU identity, missing/error/
valid zero и overflow, process recovery и collector cache→LCD text.
497 Rust tests (0 failed/ignored), fmt, strict Clippy и docs прошли.
Review без подтверждённых findings: предполагаемый floating-point overflow
уже обрабатывает общий reading constructor как Invalid/None. Точное boundary
добавлено в existing test после fullchain; focused regression и fmt прошли.
[Evidence](evidence/2026-10-10-nvidia-clocks.json).
Никаких real GPU/GUI/HID запусков; NVIDIA voltage, Intel coverage и остальные
пункты базового плана остаются открытыми.


### Intel GT clocks checkpoint, 2026-10-10

Read-only i915/xe frequency provider подключён к collector отдельно от PMU:
per-GT actual/requested, xe tile domains, отдельный legacy i915 card-wide
binding. Canonical physical device identity сохраняет card renumber; отсутствующее
actual не подменяется requested. Unsupported/missing/invalid/zero различаются.

501 Rust tests (0 failed/ignored), fmt, strict Clippy и docs прошли; независимая
review без findings. Fixtures покрывают оба драйвера, несколько domains,
renumber/disappearance/recovery, legacy isolation, duplicates/overflow и
cache→IPC→MHz/JPEG preview. [Evidence](evidence/2026-10-10-intel-clocks.json).
Native GUI, реальные sysfs/PMU/HID не запускались. Intel utilization/VRAM и
voltage coverage, hardware accuracy и общая приёмка остаются открытыми.


### Network RX/TX checkpoint, 2026-10-10

Read-only sysfs network provider добавляет per-interface RX/TX и totals,
BytesPerSecond в registry/IPC/GUI/LCD. Monotonic deltas, reset/warmup/stale/
ifindex change, independent direction errors, permanent physical/boot-scoped
identity, bounded discovery и missing-interface removal проверены fixtures.
Windows source sum подтверждён SystemInformation.cs241–290. Linux totals явно
включают virtual traffic и не обещают уникальную Internet throughput или
одинаковый состав интерфейсов с Windows; partial/empty catalog unavailable.

506 Rust tests (0 failed/ignored), fmt, strict Clippy и docs прошли; review
без findings. Frontend check0errors/warnings, build, 12 cross-language theme
contracts и 33 UI tests прошли. GUI fixture проверяет B/s в Sensors и
Network RX binding в Triple с последующим Apply.
[Evidence](evidence/2026-10-10-network-rates.json). Native WebKitGTK, реальные
network/sysfs/HID не запускались. Software unit/provider закрыт; реальные
величины и multiport identity edge cases остаются ограничениями inventory.


### Storage capacity и сверка планов, 2026-10-10

Добавленный filesystem provider разделяет total/used/free/available/used_percent,
имеет UUID/root bindings и bounded async helper с защитой от повторного запуска
при зависшем kernel syscall. Windows Fixed/Removable и Linux real mounts имеют
разный состав; точная аппаратная эквивалентность не заявлена.

512 Rust tests (0 failed/ignored), fmt, strict Clippy и docs прошли; review без
findings. 16 frontend→Rust theme contracts и один focused browser flow
Sensors→Quad→Apply (V/MHz/B/s/GiB) прошли. Полный frontend/native suite в этом
checkpoint не повторялся. [Evidence](evidence/2026-10-10-storage-capacity.json).
Реальные storage/GPU/HID не опрашивались.

Планы сверены повторно: frequencies/network/storage исключены из будущей
реализации, README исправлен; три оставшихся плана содержат открытые задачи.
Полная parity всё ещё требует M3 acceptance, M4 media/editor/effects, M5 RGB/
profiles/desktop и M6 lifecycle/delivery. Software проверки не закрывают железо.


### System Info callers audit, 2026-10-10

Spec подтверждён как read-only inventory шести групп с detail selection;
Sensor Settings — persisted CPU source mapping и общий выбор GPU с reset draft.
Сверка текущих IPC/Settings/Sensors выявила отсутствие глобальных defaults:
per-layer/per-policy bindings не закрывают этот Windows workflow. Добавлены
задача базового плана и acceptance criteria; Spec сохранён в полном M5 scope.
[Audit](system-info-inventory.md#spec-и-sensor-settings-подтверждённые-callers-2026-10-10),
[hash manifest](evidence/2026-10-10-spec-sensor-settings.json).
BAML strings не объявлены реконструированным XAML; точная visibility остаётся.
Изменения только документационные, аппаратные/GUI тесты не запускались.


### Sensor defaults contract checkpoint, 2026-10-10

Добавлена strict version1 схема default CPU sources/selected GPU и exact-ID
resolver: unit/status/finite/freshness/duplicates проверяются, missing ID не
переназначается. Три regression tests покрывают round-trip, source loss/recovery,
wrong unit/stale/invalid/ambiguous readings и malformed schema.
515 workspace tests (0 failed/ignored), fmt, strict Clippy и docs прошли;
companion review без итоговых findings. [Evidence](evidence/2026-10-10-sensor-defaults-contract.json).
Это foundation: provider membership, atomic store, RPC/dashboard/GUI и native
acceptance ещё отсутствуют. Глобальные defaults в плане остаются открытыми.


### Sensor defaults store checkpoint, 2026-10-10

Добавлен отдельный store: bounded strict read, revision check под shared mutex,
atomic tempfile/file fsync/rename/directory fsync. Несовместимые файлы не
перезаписываются; concurrent editors имеют одного победителя. First-save и
retry синхронизируют ancestors: оба durability edge найдены и исправлены review.
Тесты покрывают nested first-save/restart/reset, stale writes, malformed/future/
unknown/oversized/symlink/directory, concurrency/lost-reply retry, write failure
и revision exhaustion. Power-loss injection не проводился.

До последнего исправления helper — 519 workspace tests, fmt/strict Clippy/docs.
После него — fmt, strict workspace Clippy, четыре focused store tests и docs.
[Evidence](evidence/2026-10-10-sensor-defaults-store.json).
Store ещё не подключён к production RPC; provider membership и UI остаются
открытыми. Тесты использовали temporary files, без оборудования и production daemon.


### Sensor selection checkpoint, 2026-10-10

Daemon candidate registry классифицирует собственные provider IDs, сохраняет
physical GPU grouping между hwmon/AMDGPU/Intel modules и отдельные NVIDIA UUID.
Apply validator отклоняет cross-GPU/role/unit и ambiguous choices; не стирает
неизменённый отсутствующий binding, но перепроверяет present metadata. Memory
clock/Northbridge/engine utilization не выдаются за GPU core sources.
Четыре tests, включая реальный hwmon collector на synthetic filesystem CPU +
две GPU, прошли. Полная цепочка: 523 tests, 0 failed/ignored, fmt, strict Clippy,
docs; review без findings. [Evidence](evidence/2026-10-10-sensor-selection.json).
Следующий шаг — shared store/RPC wiring с server-owned candidates и consumer/UI.
Это не законченный пользовательский global settings workflow; железо не читалось.


### Sensor defaults RPC checkpoint, 2026-10-10

Production get/set RPC подключены к shared store; daemon читает candidates
из своего cache и проверяет выбранную роль/GPU. Get возвращает сохранённые IDs,
candidates и resolved values со freshness/status/membership; corruption/read
errors не маскируются пустой конфигурацией. Disk jobs ограничены semaphore и
не теряют permit после отмены ожидающего RPC; revision повторно проверяется
при atomic save. Explicit theme/cooling/RGB bindings не изменяются.

Memory transport IPC integration с MockBackend проверяет cross-GPU rejection,
save/revision/lost-reply retry, forbidden client metadata, новый RPC instance
с reload, missing/recovery, hand-edited wrong membership и corrupted-file
preservation. 524 workspace tests (0 failed/ignored), fmt/strict Clippy/docs;
review без findings. [Evidence](evidence/2026-10-10-sensor-defaults-rpc.json).
Desktop bridge/UI, reset defaults UX и native acceptance остаются открытыми.
Реальный daemon/HID/providers не запускались.


### Sensor defaults desktop checkpoint, 2026-10-10

Sensors содержит отдельный editor CPU sources/selected GPU и сохранённые
resolved values. Fields фильтруются по server role/GPU; unavailable saved IDs
видимы. Poll не переписывает draft; navigation сохраняет editor, reconnect
отзывает отображаемые значения и блокирует Load/Apply до fresh read. Apply
также отзывает старые values; late reads/writes защищены generation/readRevision.
External revision conflict требует explicit reload; clear меняет только draft.
Factory default suggestion/reset ещё отсутствует и не приравнивается к clear.

Svelte-check0errors/warnings, production build, 16 theme contracts, все 34 browser
flows прошли. Workspace fmt/strict Clippy и 2 Rust GUI tests прошли. Review
нашло два freshness edge, исправлены и повторно проверены статически + browser
suite. [Evidence](evidence/2026-10-10-sensor-defaults-ui.json).
Новая UI не проходила native WebKitGTK acceptance; железо не использовалось.


### Native sensor defaults checkpoint, 2026-10-10

Новый source editor прошёл actual WebKitGTK→Unix IPC→MockBackend flow:
GPU B DOM selection/apply, сохранённое35% reading, clear-only draft, navigation/
reconnect retention, explicit reload и config equality после restart. Явная
LCD-тема не меняется; закрытие GUI сохраняет stream, прежние control/profile/
reconnect/lost-reply/timeout сценарии также прошли. fmt/strict Clippy и обе
native build commands успешны. [Evidence](evidence/2026-10-10-native-sensor-defaults.json).
CPU source selection, factory defaults и ambiguous source mutations остаются;
реальных аппаратных проверок не было.


### Сверка активных планов и незавершённый reset slice, 2026-10-10

Повторная сверка исходников и сохранённых evidence: три активных плана остаются
незавершёнными; десять старых implementation plans уже удалены/заменены.
В очередь не возвращаются реализованные source editor/store/RPC и providers.

`recommended()` и кнопка рекомендуемого выбора уже существуют: fresh finite
candidates, actual/average/requested clock priority, одна GPU по stable ID,
нет записи до Apply. Сохранённые логи `/tmp/source-recommend-focused.log`
показывают 5 selection tests + 1 IPC test; `/tmp/source-recommend-check.log`
и `/tmp/source-recommend-build.log` — успешные check/build. Это логи предыдущего
запуска, не новый прогон аудита; они не подтверждают все текущие изменения.
Изменённые browser/native сценарии reset и CPU выбора ещё требуют запуска.
Предыдущий native checkpoint подтверждает только перечисленный там GPU flow.
Исторические формулировки «ещё не подключено» выше относятся к своим checkpoint;
актуальный остаток — в [плане](../superpowers/plans/basic-parity.md).


### Recommended sensor defaults acceptance, 2026-10-10

Read-only daemon proposal выбирает только fresh valid candidates, предпочитает
actual clock перед requested clock, выбирает одну GPU по stable ID без смешивания
показателей. Кнопка меняет только draft; обычный polling не меняет bindings.
Это Linux policy, не воспроизведение Windows HWiNFO эвристик.

Native WebKitGTK → Unix IPC → MockBackend подтвердил CPU45°C proposal,
GPU A proposal → явный выбор GPU B35%, отсутствие записи до Apply,
сохранение CPU/GPU, независимость LCD theme, draft clear/navigation/reconnect
и equality config после финального restart. Fixture использует только synthetic
readings, GUI работает на отдельном Xvfb. Все 34 browser flows, 16 theme
contracts, 536 Rust tests (0 failed/ignored), fmt, strict Clippy и docs прошли.
[Evidence с hashes и native результатом](evidence/2026-10-10-recommended-sensor-defaults.json).
Ambiguous source mutation recovery остаётся; общий базовый план ещё не закрыт.


### Native source lost-reply recovery, 2026-10-10

Unix proxy пропускает `set_sensor_defaults` с revision1 и получает успешный
ответ daemon (сохранена revision2); только затем закрывает соединение до
доставки ответа GUI. Native DOM подтверждает error без success, сохранённый
GPU A draft, fresh readback/revision conflict и disabled stale Apply. Explicit
reload загружает сохранённый GPU A; ручной выбор GPU B и Apply сохраняют
revision3, CPU binding остаётся прежним. Финальный restart сохраняет всю config.

Proxy фиксирует ровно1 target request/1 drop и3 source writes всего:
initial save, faulted save, explicit recovery. Auto-retry отсутствует. Тема LCD
и requested cooling/thermal/dynamic snapshots не меняются. Прежние native
profiles/reconnect/LCD lost-reply/timeout и restart сценарии также прошли.
[Evidence](evidence/2026-10-10-native-source-lost-reply.json).
Production код не менялся; JS/Python syntax и native build/run прошли.
Физическое устройство не использовалось; полноценная software приёмка остальных
workflows остаётся в базовом плане.


### BackgroundColor source audit, 2026-10-10

Windows page и подключённый GA2V JS подтверждают три gradient background режима,
две RGB, Flip и сохранение отдельных настроек типов. Overlay configs совпадают
с ImageVideo; p5 radial использует radius до угла canvas. Linux пока имеет
только solid RGB. Это конкретный implementation gap базовых themes, не часть
отложенных 23 effects и не закрытый M1 image test. [Audit и acceptance](lcd-theme-controls.md#backgroundcolor-подтверждённый-пробел-2026-10-10),
[evidence](evidence/2026-10-10-background-color-audit.json). Код и железо не запускались.


### Gradient backgrounds implementation/acceptance, 2026-10-10

Optional strict `background_fill` v1 хранит выбранный режим и независимые
horizontal/vertical/radial colors/Flip. Старый solid theme JSON и output
сохранены. Shared renderer рисует фон перед overlays; редактор меняет draft,
позволяет менять цвета/Flip и переключать тип без потери других настроек.
Continuous interpolation использует Windows directions/corner radius, не
утверждает pixel-identical p5 raster.

Результаты: 527 all-target Rust tests (0 failed/ignored), fmt/strict Clippy/docs;
frontend check/build,20 theme contracts,34 browser regressions и1 новый gradient
flow. Native WebKitGTK меняет horizontal→radial/Flip через DOM, сохраняет тему,
профили и проверяет equality после restart; прежние recovery checks также проходят.
Review без открытых findings после исправления Svelte ownership/item order.
[Evidence](evidence/2026-10-10-gradient-backgrounds.json). Физический вывод не
проверен; fonts/margins и остальные строки базового плана ещё открыты.


### Custom text geometry source checkpoint, 2026-10-10

Image/Color custom wrapper width358 и inline padding0 подтверждены активным
JS/CSS: общий padding22 не применяется. Source audit зафиксировал horizontal/
vertical anchors, отдельный TextAlignment и line heights. Existing Linux
coordinates/max_width выражают горизонтальные позиции; renderer line-box height
и font metrics не объявлены эквивалентными. [Подробности](lcd-theme-controls.md#imagecolor-custom-text-geometry-2026-10-10),
[evidence](evidence/2026-10-10-custom-text-geometry.json). Это static evidence;
новых runtime/hardware тестов не было. Basic font/geometry пункт остаётся открытым.


### Text line-box implementation/acceptance, 2026-10-10

Optional bool `TextStyle.line_box` включает N×line_height и half-leading;
false/отсутствующее поле сохраняет старый raster/JSON. Shared renderer поддерживает
top/bottom/center, blank lines и negative leading. Editor checkbox меняет draft.
Native GUI включает checkbox для clock, загружает sensor с флагом, сохраняет
полную тему/профиль и проверяет equality после restart.

529 all-target Rust tests,35 browser tests,20 contracts,fmt/strict Clippy/docs,
frontend check/build и native smoke прошли. Review без открытых findings.
[Evidence](evidence/2026-10-10-text-line-box.json). Font metrics/catalog/TC
и hardware остаются; это не объявление pixel-equivalent browser rendering.


### Noto Sans TC acceptance, 2026-10-10

Noto Sans TC Regular400/Bold700 добавлены из pinned официального Google Fonts
source, с полной OFL рядом и внутри font metadata. Полный cmap20745 entries;
воспроизводимая offline генерация fontTools4.66.1 проверена побайтово в отдельном
каталоге. Windows installation fonts не копировались. Enum/API/оба editor font
selectors и shared renderer подключены. Legacy DejaVu без изменений.

Glyph test подтверждает nonzero IDs/outlines TC sample и разные Regular/Bold
rasters. Native выбирает Bold для clock через DOM, загружает TC-text Regular,
сохраняет profiles и проверяет theme equality после restart. 530 all-target Rust
tests,35 browser flows,22 contracts,fmt/strict Clippy/docs и frontend check/build
прошли; review без findings. [Evidence](evidence/2026-10-10-noto-sans-tc.json).
Другие9 Windows fonts, exact font-version equivalence и hardware не закрыты.


### User font validation/store foundation, 2026-10-10

Добавлен single-face TTF/OTF validator до16MiB с parser renderer, finite metrics
и exact byte preservation. ThemeFontStore сохраняет generated key/font через
fsync/no-clobber, перечитывает с validation; missing/corrupt/oversize/path/symlink
и directory не подменяются default font. First-directory ancestors синхронизируются.

535 all-target Rust tests,fmt/strict Clippy/docs прошли; review без findings.
[Evidence](evidence/2026-10-10-font-store.json). Это пока не production font import:
RPC/managed references/renderer/retention/UI не подключены. Большие fonts требуют
chunked transfer из-за8MiB JSON frame. Hardware и frontend не запускались.


### Chunked user font RPC checkpoint, 2026-10-10

Production RPC теперь передаёт fonts до16MiB порциями до64KiB; общий pool4 сессии,
idle300s с lazy expiry. Проверены offsets/overflow, identical retry, incomplete
Finish, cancel, capacity и exact-byte передача полного Noto Sans TC через
production dispatch/MockBackend. Повтор Finish возвращает тот же key; recovery
после rename не создаёт копию и не заменяет файл иным содержимым.

Сохранённый полный прогон:539 all-target tests, fmt/strict Clippy/docs.
После последних правок повторены fmt/Clippy/docs,3 integration tests и отдельно
новый1 replay unit test; это не новый полный прогон540 tests. Review без findings.
[Evidence](evidence/2026-10-10-font-upload.json). Theme references, renderer,
retention и GUI import ещё не реализованы. Frontend/native/hardware не запускались.

### Повторная сверка планов после font RPC, 2026-10-10

Проверены все4 файла в `docs/superpowers/plans` (3 активных плана и индекс),
корневой roadmap, baseline gaps и acceptance checklist. Десять старых планов
уже удалены из рабочей очереди; новых полностью закрытых планов нет. M1 сохраняет
hardware/lifecycle, M2 — sensor coverage и физическую приёмку, basic-parity —
незавершённую theme/font интеграцию и software acceptance. Выполненный chunked
upload убран из очереди реализации, результат перенесён в checkpoint/checklist.
Исторические audits/specs/captures сохранены как evidence, не как активные планы.
Сверка документации не является новым запуском общей acceptance matrix.


### Managed theme fonts backend, 2026-10-10

Optional TextStyle.font_asset включает managed font для текста, часов, sensor/
cycle values и отдельных label/unit. Renderer использует owned parsed FontArc;
preload дедуплицирует keys и ограничивает source bytes64MiB на тему. Preview,
Apply, profile save и startup replay подключены. Missing/corrupt preparation
отклоняется до замены runtime/library, а restore failure сохраняет intent.
Подготовленный runtime продолжает работать после удаления файла.

Проверены managed/bundled raster equality для всех text roles, strict keys и
legacy serialization, полный CJK upload→preview JPEG equality→profile через
production dispatch, Apply/restart/missing recovery на MockBackend. Полный
Rust run543 tests, fmt/strict Clippy/docs прошёл; добавленный позднее64MiB
boundary/dedup test прошёл в отдельном5-test suite, не включён в число543.
[Evidence](evidence/2026-10-10-theme-fonts.json). GUI import/выбор и font cleanup
остаются, frontend/native/hardware в этом checkpoint не запускались.


### User font GUI import/selection, 2026-10-10

Display получил File upload TTF/OTF до16MiB с64KiB chunks/progress/cancel.
Загрузка добавляет выбор, но не меняет тему; managed/bundled выбор меняет только
draft. FontSelect используется для основного style и sensor label/unit; bundled
selection удаляет font_asset. Fonts текущей темы доступны другим слоям, имя
imported файла хранится в GUI session, после reload используется короткий key.
Lost Finish проверяется через Status той же сессии без повторного Begin; Cancel
в finally освобождает слот, unmount/abort запрещает поздний onimport.

Frontend check0/build,23 Rust↔TS contracts,39 browser tests прошли. Native
WebKitGTK DOM File→production bridge→Unix RPC→MockBackend загрузил шрифт,
выбрал для text layer, сохранил Apply/profile и восстановил после restart.
GUI close сохраняет renderer activity. Bridge unit tests2 passed; fmt/strict
Clippy проверены отдельно. [Evidence](evidence/2026-10-10-font-gui.json).
Font cleanup и расширенные recovery cases остаются; аппаратного прогона нет.


### Font recovery acceptance, 2026-10-10

Добавлены2 browser tests: удержанный chunk, отключение daemon, отмена без
следующего chunk/Finish, сохранение draft, reconnect и explicit retry;
ошибка валидации Finish и успешный повторный импорт без auto Apply. Оба прошли.
Native WebKitGTK с production RPC отвергает File из3 invalid bytes, сохраняет
активную тему и edited draft, затем успешно принимает корректный TTF через тот
же input. DOM выбор managed font для sensor label/unit сохраняется после Apply,
profile/restart; существующая full native matrix также прошла.

[Evidence](evidence/2026-10-10-font-recovery.json),
`/tmp/lconnect-native-tsc53bvc/acceptance.json`. Изменены только tests/harness/docs;
нового полного Rust/browser прогона не заявляем. Production остаётся без изменений.
Native disconnect во время upload/lost Finish и потеря уже сохранённых dependency
остаются отдельными cases; font cleanup — следующая реализация. Hardware не использовался.


### Font cleanup/session retention, 2026-10-10

PersistenceStore владеет общим ThemeFontStore; upload/import, preview/load,
profile save и manager replay используют clones одного session mutex/pin set.
`cleanup_theme_fonts` перечитывает offline device settings и profiles, отклоняет
corrupt/unreadable inputs, сохраняет referenced/session/draft keys. Сканирование
ограничено10000 entries и завершается до удаления; eligible только regular
32-hex`.font`, unlink errors возвращаются в failures. GUI имеет отдельный
scan/delete, invalidate stale report при изменении draft keys/connection.

546 all-target Rust tests, fmt/strict Clippy, frontend check/build и42 browser
tests прошли; исправлена TS type assertion в предыдущем recovery test.
Native WebKitGTK GUI удалил seeded prior-session orphan, затем успешно прошёл
font import/retry/parts/profile/restart и прежнюю native matrix.
[Evidence](evidence/2026-10-10-font-cleanup.json),
`/tmp/lconnect-native-ce3trh7a/acceptance.json`. Hardware не использовался.
Concurrency stress/partial unlink failure и остаток native recovery ещё требуют
явной приёмки; базовый план остаётся открытым.

### Native font lost Finish, 2026-10-10

Тестовый proxy обрывает socket после успешного font Finish. Native GUI
восстанавливает результат через Status той же upload session; итоговый
font_asset совпадает с потерянным ответом. Счётчики: 2 Begin/2 Finish
(сначала invalid file, затем valid retry), 1 dropped response/1 recovery Status;
после удаления prior-session orphan сохранён ровно один font файл.
DOM draft/Apply, profile и restart equality, GUI-close rendering и прежние
native control/reconnect сценарии прошли. [Evidence](evidence/2026-10-10-native-font-lost-finish.json).
Изменены только Python fixture/proxy; production код и JS harness не менялись.
Это не проверка partial-upload disconnect, missing saved font или железа.

### Native partial font upload, 2026-10-10

После invalid-file rejection harness отправляет валидный файл и теряет
ответ на первый принятый64KiB chunk. GUI показывает error, сохраняет draft
и active theme, не добавляет interrupted font в choices. Proxy подтверждает
ровно один chunk и успешный Cancel без asset; последующих chunks/Finish этой
сессии нет. Явный retry проходит вместе с lost-Finish recovery и всей прежней
native matrix, включая profile/restart. Всего3 Begin/2 Finish, один font файл.
[Evidence](evidence/2026-10-10-native-font-partial-upload.json).
Python/JS syntax, native rebuild и isolated MockBackend smoke прошли.
Production код не менялся; missing/corrupt saved dependencies и cleanup
failure/concurrency остаются. Это transport disconnect, не USB hardware test.

### Native saved dependency recovery, 2026-10-10

Python harness удаляет либо повреждает файл уже сохранённого screen profile
(отдельно font/image). Native DOM загружает профиль и пытается применить
изменённый draft: видны error и отсутствие success; draft, library/revision,
active theme и source=Theme сохранены. Возврат исходных bytes → явный Apply
очищает error и восстанавливает workflow. Все4 случая и прежняя native matrix
прошли, включая partial upload/lost Finish и итоговый profile/restart.
[Evidence](evidence/2026-10-10-native-saved-dependencies.json).
Проверены Python/JS syntax, native rebuild и isolated MockBackend smoke.
Это live-runtime recovery; daemon restart при отсутствующем файле отдельно
этим прогоном не проверяется. Production код не менялся, железо не использовалось.

### Native active pump sensor loss/recovery, 2026-10-10

Private marker удаляет fixture:cpu из SensorCache, сохраняя альтернативный CPU
reading. На уже принятой pump curve observed applied_percent становится
configured fallback; GUI показывает недоступный original ID и fallback.
Requested policy, LCD bindings и несохранённая правка curve не меняются.
Возврат same ID восстанавливает curve output без Apply draft.
[Evidence](evidence/2026-10-10-native-active-sensor.json).
Mock fixture/native rebuild, Python/JS syntax и полный isolated native smoke
прошли. Изменён только тестовый fixture/harness; physical PWM/RPM и прочие
stale/fan/thermal cases этим результатом не закрываются.

Rust companion verification: fmt, strict Clippy,546 all-target tests и docs
прошли; замечаний к добавленному marker нет. Existing sync stats I/O fixture
отмечено отдельно, без наблюдавшегося сбоя; production путь не затрагивался.

### Font cleanup concurrency/partial failure, 2026-10-10

Private unlink seam даёт deterministic failure-before-success/retry без
расширения public API. Shared-store load/import/delete запускаются через
barrier32 раза; успешно загруженный/imported font остаётся pinned, cleanup-first
допускает ошибку load удалённого orphan. Browser показывает partial result и
failed key, требует rescan для retry.548 Rust tests, fmt/strict Clippy/docs
и2 focused browser tests прошли; review без замечаний.
[Evidence](evidence/2026-10-10-font-cleanup-races.json). Это sampled schedules
и injected unlink failure, не все interleavings и не аппаратная проверка.
Закрытый font recovery/failure блок удалён из активного базового плана.

### Font catalog binary audit, 2026-10-10

Проверены metadata/hashes всех11 Windows font assets и официальные pinned
Abril/Caprasimo/Cubic11. [Результаты и следующие действия](font-catalog.md).
Выявлены Condensed вместо enum Bold и3 отсутствующих codepoints в bundled
Noto. Новые assets не включались; runtime/hardware tests не запускались.

### Bundled display fonts, 2026-10-10

Abril Fatface/Caprasimo/Cubic11 добавлены в IPC enum, renderer и общий FontSelect.
Официальные bytes и полные licenses закреплены provenance; Cubic1.013 совпадает
с Windows. Native font importer показывает notices inline, без изменения CSP.
Каждый bundled render сравнивается с managed render того же файла; Cubic CJK
проверяется glyph outlines и raster.549 Rust tests, fmt/Clippy/docs, frontend
check/build,26 contracts прошли.43 browser tests до license-only исправления;
после него6 focused cases и полный native smoke прошли. Native выбирает все3
через DOM и проверяет Apply/profile/restart. [Evidence](evidence/2026-10-10-display-fonts.json).
Три missing Noto codepoints, Windows metrics и hardware остаются открытыми.

### Повторная сверка планов, 2026-10-10

[Аудит](evidence/2026-10-10-plan-audit.json) сопоставляет сохранённые checkpoints
с текущими hashes. У display-fonts совпали7 из11 путей; четыре изменены после
checkpoint. Native artifacts трёх проверенных прогонов доступны; оба пути
font-cleanup-races совпали. Это не новый тестовый прогон.

Текущие Noto OTF имеют20748 codepoints, все три ранее отсутствовавших символа
и совпадающие с manifest hashes. Сохранённый focused renderer test прошёл;
общая приёмка после замены остаётся. Из очереди исключены уже проверенные
live saved-dependency и active-pump-sensor recovery; сохранены остальные
fault cases и restart с отсутствующим dependency. Три активных плана имеют
открытые требования, поэтому целиком не удаляются. Архивы/evidence сохранены.

### Noto TC2.002 coverage, 2026-10-10

Bundled Regular/Bold заменены на pinned official OTF с полным OFL в name ID13.
Восстановлены U+2252/U+25C8/U+30EDE; каждый из20 748 mapped glyphs сравнен
с Windows: horizontal metrics и outline commands совпали в обоих весах.
Offline helper воспроизвёл exact output hashes; обновлены asset инструкции.

549 Rust tests, fmt/strict Clippy/docs, frontend check/build,26 contracts,
6 focused browser cases и полный native smoke прошли. Отдельный bundled Noto
слой с тремя символами сохраняется при Apply/profile/restart; managed text
слой по-прежнему проверяет импорт. [Evidence](evidence/2026-10-10-noto-coverage.json).
Vite dev выдаёт существующее предупреждение о raw import notices из static;
production build и native inline notices проходят. Pixel-identical layout/
shaping/raster и hardware этим результатом не подтверждены.

### Resource/Settings XAML inventory, 2026-10-10

Из448 копий BAML version2.1.28 восстановлены448 XAML без ошибок. Hash оригинальной
assembly совпал с baseline manifest; исходные reference BAML не изменены.
Resource/Settings controls, bindings, progress ranges и navigation/visibility
сверены с C# callers и общими styles. Подтверждены CPU fan picker и отсутствие
отдельных VRAM/GPU fan/drive readings controls в этих общих страницах.
[Выводы](system-info-inventory.md), [manifest](evidence/2026-10-10-system-info-xaml.json).
Source inventory исключён из очереди; CPU fan provider/mapping gap добавлен явно.
Production code не менялся, тесты/Windows GUI/железо не запускались.

### hwmon fan RPM collector, 2026-10-10

Generic fanN_input readings появились в host catalog с physical IDs и typed RPM.
Нет записи PWM/target, повторного divisor scaling или догадки о CPU role по label.
Четыре fixture regressions закрывают stopped0, fault/enable/invalid, renumber,
missing/recovery/stale, duplicate identity и catalog overflow.553 all-target Rust
теста, fmt/strict Clippy/docs прошли; companion review без замечаний.
[Evidence](evidence/2026-10-10-hwmon-fan-rpm.json).
CPU-fan mapping/GUI/restart acceptance остаются; hardware не использовался.

### Explicit CPU fan mapping, 2026-10-10

Generic hwmon fan RPM можно назначить CPU fan вручную; known GPU drivers
и LCDV2 readings исключены из кандидатов. Рекомендуемый выбор оставляет поле
пустым; описание UI и aria-describedby объясняют необходимость выбрать канал
по подключению. Schema не менялась: сохраняется cpu.fan ID.

554 Rust tests, fmt/strict Clippy/docs, frontend check/build,26 contracts,
focused browser source flow и полный isolated native smoke прошли. Backend
проверяет stopped0, missing/new-ID rejection, durable reload и same-ID recovery.
Native DOM выбирает канал, видит1234 RPM, сохраняет mapping через lost-response
recovery и все последующие fixture restarts. [Evidence](evidence/2026-10-10-cpu-fan-mapping.json).
Read-only мониторинг: wiring и accuracy физически не проверены; CPU fan loss
проверен RPC, не отдельным native hot-unplug сценарием.

### Intel hwmon voltage, 2026-10-10

Поддержаны i915 in0 (GT) и xe in1 (package) с преобразованием mV→V,
точным driver/rail matching и stable physical IDs. Два новых fixture tests
проверяют units/membership, renumber/missing, zero/disabled/malformed/stale;
оба rails также проходят cache→IPC→LCD text/JPEG preview.
556 Rust tests, fmt/strict Clippy/docs прошли; companion review без замечаний.
[Evidence](evidence/2026-10-10-intel-voltage.json). Hardware и frontend/native
заново не запускались; наличие voltage зависит от GPU/kernel.

### NVIDIA voltage source gate, 2026-10-10

[Source audit](nvidia-voltage-abi.md) установил удаление graphics voltage из
nvidia-smi и отдельный NV-CONTROL read-only attribute413/uV. Pinned official
sources проверены по hashes; подготовлены UUID/session/failure acceptance
критерии. Provider не реализован; GPU/driver commands и тесты не запускались.
Это уточнение пути реализации, не закрытие NVIDIA voltage gap.

### Optional NVIDIA NV-CONTROL voltage, 2026-10-10

Добавлен explicit-session provider `/usr/bin/nvidia-settings`: read-only запрос
по UUID, сверка UUID ответа, uV→V и отдельный core_voltage binding. Основной SMI
query не менялся. Per-GPU ошибки независимы; output/time/child lifetime ограничены.
Четыре новых regressions проверяют units/zero/invalid/timeout/reaping, flags,
две GPU/reorder/missing/recovery, defaults и cache→LCD text/stale.
560 Rust tests, fmt/strict Clippy/docs прошли, companion review без замечаний.
[Evidence](evidence/2026-10-10-nvidia-voltage.json), [настройка](nvidia-voltage-abi.md).
Реальные NVIDIA команды/GUI не запускались; XWayland/headless availability и
физическая accuracy остаются открытыми.

### Settings persistence acceptance, 2026-10-10

Два сквозных Rust tests закрыли JSON-RPC Apply → disk error → retry → joined
shutdown → новый manager/replay, а также failed final flush с сохранением
старых bytes. Browser flow проверяет pending/error выше исторического saved,
сохранение draft и отсутствие дополнительной mutation при retry.
562 Rust tests, fmt/strict Clippy/docs, frontend check/build,26 contracts и
focused browser case прошли. Review без оставшихся замечаний; исходная ошибка
неполного test seed исправлена до итогового прогона. Production code не менялся.
[Evidence](evidence/2026-10-10-persistence-acceptance.json),
[контракт и границы](persistence-acceptance.md). Native/physical/power-loss не заявляются.

### Native missing-dependency restart и nullable IPC, 2026-10-10

Native harness теперь проверяет missing font/image при запуске и возврат файлов
с повторным запуском: Idle/error/no active theme, zero mock JPEG, сохранность
всех device settings/profile/draft, автоматический replay после восстановления.
Прогон обнаружил ошибку serde result:null → None; исправлены response presence
и завершение malformed-response callers. Два protocol regressions проверяют
валидные/невалидные envelopes и продолжение работы соединения.
564 Rust tests, fmt/strict Clippy/docs, syntax checks, native/mock builds и
полный isolated native smoke прошли. Review замечания (fresh stats и error:null)
исправлены до окончательного прогона. [Evidence](evidence/2026-10-10-native-restart-dependencies.json),
[контракт](restart-dependency-acceptance.md). Hardware не использовался.

### Повторная сверка активных планов, 2026-10-10

Проверены все три активных implementation plans, roadmap, scenario matrix,
baseline gaps и сохранённые checkpoints. NVIDIA NV-CONTROL исключён из очереди
реализации: остаётся platform coverage вне X-сессии. Исправлены устаревшие
строки matrix о незавершённом binding/font inventory и native RGB happy paths.
Новых полностью закрытых планов нет: M1/M2 сохраняют hardware/lifecycle и
provider gaps, базовый план — software acceptance и оставшуюся реализацию.
Ранее удалённые старые планы не возвращены; specs/captures/history сохранены.

Hashes всех 4 файлов NVIDIA checkpoint и 8 файлов restart checkpoint совпали
с рабочим деревом. Изменённые файлы старых checkpoints отмечены отдельно;
сохранённый результат не объявляется новым полным прогоном. После native
checkpoint был перенесён raw-import лицензий в source assets, поэтому прежнее
утверждение «frontend source не менялся» убрано. Новые Rust/browser/native
и аппаратные тесты в этой документальной сверке не запускались.
[Evidence](evidence/2026-10-10-plan-reconciliation.json).

### Display interrupted replies, 2026-10-10

Исправлена публикация позднего Apply/clear/load ответа после наблюдаемого
разрыва IPC: selection generation теперь инвалидируется при disconnect,
старые success/error не заменяют свежий status и draft. Ошибки от предыдущего
выбранного устройства также игнорируются. Отправленная команда не повторяется;
загрузка активной темы остаётся отдельным действием пользователя.

6 новых browser cases проверяют принятый Apply/clear и начатый Load при
reconnect с поздним success/error, сохранение последующих правок, один вызов
и явный readback. Вместе с соседними regressions прошли 13 tests.
Frontend check (0 errors/warnings), build,26 contracts, mock/native builds и
полный isolated WebKitGTK smoke прошли. Rust suite повторно не запускался:
Rust-код не менялся. [Evidence](evidence/2026-10-10-display-interrupted.json).
Native smoke подтверждает существующие flows; новую native fault injection
для Apply/clear/load, profile interruptions и физические проверки не заменяет.

### Native profile save: потерянный ответ, 2026-10-10

Unix proxy разрывает GUI socket после успешного edit_profile save ответа
от daemon. Проверено: ровно один запрос и commit (revision +1), сохранён полный
submitted snapshot; последующие правки и активная тема не изменились, GUI
показал ошибку без ложного успеха. Явный refresh не повторяет сохранение и
сохраняет оба draft; Load получает принятый профиль, Save восстанавливает
исходную тему библиотеки. Production code не потребовал изменений.

Python compile, JS syntax, native build и полный isolated native smoke прошли,
включая последующие restart/replay. [Evidence](evidence/2026-10-10-native-profile-lost-reply.json).
Rust/browser suites заново не запускались: менялся только native harness.
Rename/delete/reset, interruption до commit и hardware этим не закрыты.

### Native theme replacement/clear: потерянный ответ, 2026-10-10

Proxy теряет успешные ответы set_lcd_theme и clear_lcd. Native DOM Apply
меняет имя и цвет gradient; полная принятая тема сохраняется, последующие
правки не применяются, ложного успеха нет. Readback явно загружает принятую
композицию. Clear с потерянным ответом даёт null theme/Idle, сохраняет draft
и library; явный Apply восстанавливает исходную полную тему. Proxy подтвердил
по одному target request и dropped response; auto-retry отсутствует.

Python/JS syntax, native build и полный isolated native smoke прошли.
Первый прогон остановлен после обнаружения неверного имени навигационной
кнопки в harness; окончательный прогон после исправления прошёл.
Production code не менялся; Rust/browser suite не повторялись.
[Evidence](evidence/2026-10-10-native-theme-lost-reply.json).
Ни physical firmware resync, ни cancellation during preparation этим не доказаны.

### Порядок theme preparation и Clear, 2026-10-10

Исправлена гонка: theme assets могли готовиться в IPC, пока другой client
завершал Clear; поздняя SetTheme возвращала streaming и старые settings.
LCD actor теперь выдаёт preparation token. Новая подготовка и replace источника
аннулируют старый token; stale SetTheme отклоняется до render/persist/HID.
Startup replay остаётся последовательным и не требует внешнего token.

Детерминированная регрессия проверяет оба порядка завершения подготовок,
Clear, отсутствие stale frames/settings и новый явный Apply. Review потребовал
отдельно проверить старую подготовку до готовности новой; сценарий добавлен.
565 Rust tests, fmt/strict Clippy/docs и независимый review прошли. Mock build
и полный native smoke также прошли; hardware не использовался.
[Контракт](theme-preparation-order.md), [evidence](evidence/2026-10-10-theme-preparation-order.json).

### ASUS EC CPU core voltage, 2026-10-10

Добавлен source-confirmed asusec in0: Linux v6.17 определяет единственный
voltage sensor CPU Core в mV; collector переводит его в Volts и классифицирует
как CPU voltage. Defaults/recommendations и существующие LCD bindings получают
этот канал. VID, другие rails и arbitrary motherboard in0 не подставляются.

Новая и расширенные fixtures проверяют units/role/recommendation/selection,
renumber/label changes, missing/recovery, zero/disabled/invalid/stale и
cache→IPC→LCD text/JPEG preview. 25 focused sensor tests и 566 all-target Rust
tests прошли; fmt/strict Clippy/docs и независимый review также прошли.
[Evidence](evidence/2026-10-10-asus-cpu-voltage.json),
[source и границы](sensor-provider-inventory.md#asus-ec-cpu-core-voltage-2026-10-10).
GUI/native и реальные sensors не запускались; coverage других плат остаётся.


## 2026-10-10 — Intel dedicated VRAM query

i915/xe read-only memory provider реализован: dedicated total/used/percent,
SYSMEM исключён, stable physical IDs и source editor GpuMemoryUsed mapping.
Внутренний helper mode выбирается до config/HID; timeout/reap/quarantine
изолируют зависший query от основного daemon. Unsafe ограничен новым DRM
query crate; запрет unsafe в daemon сохранён.

Оба драйвера скрывают usage без нужных прав: полностью нулевой расход остаётся
unavailable, total доступен независимо. Это также скрывает настоящий ноль;
ограничение явно сохранено в [контракте](intel-memory-query.md).

6 query tests + 5 collector tests, включая helper→cache→IPC→JPEG/stale,
полная Rust chain: **577 tests**, fmt/strict Clippy/docs, review без оставшихся
замечаний. C probe против v6.17 headers подтвердил native ABI x86_64.
[Evidence и hashes](evidence/2026-10-10-intel-memory.json).
Ошибка нового preview fixture (missing temperature_unit) исправлена до
финального успешного прогона. GUI/native и реальные drivers не запускались.

Intel xe utilization/aggregate load, остальные providers, text layout и
общая software acceptance остаются открытыми. Все три активных плана сохранены.


## 2026-10-10 — xe PMU и явный Intel engine source

xe ENGINES query дополнил изолированный helper; persistent per-engine counters
считают Δactive_ticks/Δtotal_ticks вне executor/HID. Runtime проверяет formats,
event IDs, PCI/GT/class/instance, CPU mask; ошибки закрывают handles, recovery
проходит warmup. Общий QueryProcess сохранил bounded timeout/reap/quarantine
Intel memory helper. i915/xe engine теперь можно явно выбрать как GpuLoad;
recommended reset не выдаёт выбранный engine за автоматическую общую загрузку.

Windows2.1.28 GPULoad manager выбирает один HWiNFO sensor, а не агрегирует engines.
Underlying HWiNFO aggregate и эквивалентность на железе не установлены и остаются
открытыми. [Контракт и acceptance](intel-engine-utilization.md).

Финальная Rust chain: **585 tests**, fmt/strict Clippy/docs, review без замечаний.
[Evidence/source hashes](evidence/2026-10-10-xe-pmu.json). C probe подтвердил
ENGINES layout x86_64. Native GUI и реальные PMU/DRM/HID не запускались.
Один промежуточный focused run поймал нестабильный существующий NVIDIA oversize
тест (Unavailable вместо Invalid); итоговый полный прогон прошёл без изменения
NVIDIA. Причина этого промежуточного сбоя не установлена, запись сохранена в evidence.

Все три плана остаются открыты. Native engine selection добавлен в software
checklist; physical accuracy/support/idle-power/cadence — в отложенную приёмку.

## 2026-10-10: native Intel sources

Закрыта GUI-приёмка i915/xe engine и VRAM: readable labels, GPU isolation,
значения 27/47% и 1/2 GiB, сохранение и restart/readback обоих bindings.
Recommended reset выбирает Intel GPU, но оставляет engine load пустым и
не сохраняет draft автоматически. Явные theme/cooling/RGB policies неизменны.
Полный native smoke прошёл с шестью reconnect markers, прежними lost-reply/
dependency сценариями и restart после закрытия GUI. 585 Rust tests,
fmt/strict Clippy/docs прошли; review без оставшихся замечаний.
[Evidence](evidence/2026-10-10-native-intel-sources.json). Реальные providers и
HID не запускались; physical accuracy и Windows aggregate остаются открытыми.

## 2026-10-10: interrupted profile mutations

После ошибки mutation Profiles сохраняет поля имени/слота, но запрещает
загрузку и повторную запись до явного чтения библиотеки. Ошибка ответа
не означает, что daemon не выполнил запись. Browser matrix покрывает
save/rename/delete/reset: lost reply после commit и late success/error после
reconnect, 12 новых случаев без auto-retry/Apply и с сохранением draft.
14 focused browser tests, check/build, 26 contracts и полный isolated native
smoke прошли. Native подтвердил совместимость с конфликтом ревизий и lost save
reply. Первая попытка скрывала поля после ошибки; native выявил это, итоговый
вариант сохраняет их видимыми. [Evidence](evidence/2026-10-10-profiles-interrupted.json).
Pre-commit/group-switch и native fault injection остальных операций остаются.

## 2026-10-10: profile rejection и смена канала

Дополнена browser matrix: четыре отказа до изменения library/revision и
два pump→fan сценария с поздним success/error. Пока ответ pump save удержан,
fan save завершается; поздний ответ не заменяет fan library/draft. Explicit
load возвращает pump70/fan35, запросов применения cooling нет.
18 profile tests и 2 group-switch tests прошли; Svelte check без diagnostics.
Production code не менялся; native/Rust повторно не запускались.
[Evidence](evidence/2026-10-10-profile-precommit-group.json).
Native fault injection оставшихся операций и обрыв до persistence открыты.

## 2026-10-10: native profile mutation faults

Unix-socket proxy теряет ответы успешных rename/reset/delete (каждый ровно
один раз) и разрывает соединение для save до передачи daemon. Native GUI
сохраняет draft, блокирует stale load/retry до refresh, читает результат без
повтора и не меняет активную LCD-тему. Pre-forward failure оставляет всю
библиотеку и revision неизменными. Полный native smoke и final restart прошли.
[Evidence](evidence/2026-10-10-native-profile-mutations.json).
Это не crash во время fsync; backend persistence guarantees и независимость
cooling/lighting надо сопоставить с общей matrix. Production code не менялся.

## 2026-10-10: profile persistence boundaries

Atomic write helper сохраняет прежнюю последовательность write→file sync→
rename→directory sync. Private observer позволяет вернуть ошибку на границе
этапов без смены permissions/гонок filesystem. Unit проверяет old/committed
reopen, temp cleanup, stale revision rejection и explicit recovery.
586 Rust tests, fmt/strict Clippy/docs прошли; review без замечаний.
[Acceptance mapping](profile-persistence-acceptance.md),
[evidence](evidence/2026-10-10-profile-persistence.json). Это returned-error
fixture, не power loss/process kill. Native/browser отдельно не повторялись.

## 2026-10-10: stale/missing consumer mapping

[Матрица](sensor-consumer-acceptance.md) связывает cooling/thermal/dynamic RGB/
LCD поведение с текущими тестами. Fan stale fallback дополнен wrong-ID и
same-ID recovery; paused time + два State replies доказывают обработку
тика перед проверкой. LCD runtime проверяет cache→JPEG для unavailable,
реального zero, stale/invalid/wrongunit/wrongID и восстановления без Apply.
Полный Rust suite: 587 pass. После test-only timing correction прошли
affected test/fmt/Clippy; review без замечаний.
[Evidence](evidence/2026-10-10-sensor-consumers.json). Native/hardware не запускались.
Остаётся native совместная consumer matrix, а не повторная реализация fallback.

## 2026-10-10: native image cleanup retention

Три seeded image assets из прежнего запуска: profile-only, offline-device-only
и orphan. Реальные GUI dry-run/delete удаляют только orphan; active theme
и referenced assets сохраняются. После final restart оба retained assets
успешно проходят preview_lcd_theme. Полный isolated native smoke прошёл.
[Evidence](evidence/2026-10-10-native-image-cleanup.json).
Остаток: native draft/session-only, corrupt roots, partial failure/retry и
font retained-reference matrix. Production code не менялся, железо не использовалось.

## 2026-10-10: native cleanup corruption/recovery

После успешного dry-run fixture повреждает offline TOML. Реальный GUI Delete
получает ошибку, убирает stale report/delete action и не заявляет успех.
Python сверяет исходные bytes всех seeded assets, затем восстанавливает
settings. Fresh scan снова находит orphan; explicit delete проходит.
Active theme не меняется; final restart и retained image rendering прошли.
[Evidence](evidence/2026-10-10-native-cleanup-corrupt.json). Полный native smoke
прошёл; production code не менялся. Это не corrupt profile JSON или partial unlink.

## 2026-10-10: native font retention

Seeded profile-only и offline-device-only fonts сохраняются через native GUI
scan/delete; orphan удаляется. После final daemon restart оба файла совпадают
с исходными bytes и используются renderer для image/text themes. Полный
native smoke прошёл. [Evidence](evidence/2026-10-10-native-font-retention.json),
[консолидированная cleanup matrix](cleanup-acceptance.md). Остались native
draft/session-only, corrupt profile JSON и partial unlink/retry.

## 2026-10-10: сверка очереди и завершённый draft/session smoke

Сохранённый `/tmp/lconnect-native-esxn5bko/acceptance.json` подтверждает полный
native smoke, включая раздельную защиту image/font только черновиком и только
сессией. [Evidence](evidence/2026-10-10-native-draft-session-retention.json).
При аудите результат прочитан, тест повторно не запускался; hardware не использовался.

Активный базовый план сокращён до открытых требований. Три плана сохраняют
6/6/11 открытых пунктов; полностью завершённых среди них нет. Прежние удаления
10 устаревших планов сохранены; старый checkout и provenance не изменялись.
[Аудит](evidence/2026-10-10-plan-reconciliation-final.json).

## 2026-10-10: native cleanup с corrupt profile JSON

Native GUI image Delete и font scan вызывают настоящий daemon при временно
повреждённом profiles.json в private fixture. Оба вызова отклонены; библиотека,
image/font bytes и активная тема сохранены. Восстановление исходных bytes и
explicit scan возвращают workflow; image orphan удаляется только после него.
Полный native smoke и GUI build прошли; JS/Python syntax проверены.
[Evidence](evidence/2026-10-10-native-cleanup-profiles.json).
Production код не менялся; Rust/browser suites повторно не запускались.
Hardware не использовался. Остаток native cleanup — partial unlink/retry.

## 2026-10-10: native partial cleanup/retry завершён

Для image и font независимо seeded два orphan. Test-only libc unlink shim
возвращает EACCES одному точному private path; второй действительно удаляется.
GUI показывает успешный subset и failed key, требует fresh scan. После снятия
fault остаётся один кандидат, explicit retry удаляет его без ошибки. Активная
тема сохранена. Полный native smoke/restart и build прошли; C -Wall/-Wextra/-Werror,
JS/Python syntax проверены. [Evidence](evidence/2026-10-10-native-cleanup-partial.json).
Production код не менялся, hardware не использовался. Cleanup matrix закрыта;
в basic-parity осталось 10 открытых пунктов, M1/M2 — по 6. Планы не завершены.

## 2026-10-11: keyboard workflows и контраст полей

Tab/Enter/Space/стрелки проверены для cooling fan PWM, RGB brightness и CPU
source selection, плюс прежние Settings/layer focus tests. Все 7 tests прошли.
Исправлен общий border color полей: воспроизведённый 1.967:1 повышен до ≥3:1
к заливке и фону в проверенных состояниях пяти страниц. Check/build прошли;
native keyboard/AT, expanded states и full contrast/reflow остаются.
[Evidence](evidence/2026-10-11-keyboard-contrast.json), [matrix](accessibility-acceptance.md).

## 2026-10-11: reflow узкого desktop viewport

Воспроизведено переполнение842px при viewport320/640. LCD workspace теперь
становится одноколоночным, sidebar переходит наверх, controls/actions переносятся;
минимум grid source selectors ограничен доступной шириной. Все пять страниц
и expanded text/curve/thermal состояния прошли проверку viewport/controls.
9 browser tests, check0/0 и build прошли; screenshot cooling320 просмотрен.
[Evidence](evidence/2026-10-11-desktop-reflow.json). CSS-only change, IPC/state
не изменены; native zoom и hardware не проверялись. Accessibility gate остаётся.

## 2026-10-11: native shared-source consumer recovery

Одновременный missing fixture CPU проверен для pump/fan curves, thermal и
sensor RGB. Fallback каналов, сохранение тревоги/привязок, same-ID recovery
без Apply и последующее возобновление dynamic RGB прошли native. Исправлено
ложное GUI сообщение о base lighting, когда degraded режим перекрыт thermal.
4 browser tests, frontend check/build и полный native build/smoke прошли.
[Evidence](evidence/2026-10-11-native-consumer-recovery.json).
Aged stale, hot→cool sample, transport faults и hardware остаются отдельно.

## 2026-10-11: native aged sample и hot→cool recovery

Fixture теперь умеет95°C и остановку synthetic cache publication без остановки
actors/IPC. Проверена настоящая monotonic stale граница>3s: value:null, fallback
pump/fan, удержание thermal alert и dynamic suppression. Свежий cool sample
восстанавливает consumers без Apply, сохраняя intent и drafts. Полный native
build/smoke прошёл; Rust fmt/strict Clippy/doc и587 tests прошли; review чистый.
[Evidence](evidence/2026-10-11-native-aged-consumers.json). Производственный Rust
код не менялся, hardware не использовался. Остались transport independence gates.

## 2026-10-11: повторная сверка всех активных планов

Roadmap очищен от устаревших повторов profile/cleanup и missing/stale acceptance.
Три активных плана сохраняются: M1/M2 по6 открытых пунктов, basic9 после
объединения дублирующего transport gate. Новых полностью закрытых файлов нет;
прежние удаления устаревших планов сохранены. Реализация не менялась,
тесты и hardware не запускались. Проверены hashes текущего aged-native fixture
и reflow checkpoint; старые snapshots не объявляются проверкой всего дерева.
[Evidence](evidence/2026-10-11-plan-reconciliation.json).

## 2026-10-11: независимость от pump RGB transport failure

Private native backend отклоняет только report1/cmd131. За fault interval
зарегистрированы3 ошибки RGB, JPEG starts15→18, fan writes9→10 и applied PWM61.
Requested binding не меняется; после снятия fault RGB сам восстанавливается,
fan55 возвращён. Полный native smoke/restart прошёл; fmt/strict Clippy/doc и587
Rust tests прошли, review без замечаний. Production код не менялся, железо
не использовалось. [Evidence](evidence/2026-10-11-native-rgb-transport.json).
Обратные LCD/PWM failures, direct RGB и физический resync остаются открытыми.

## 2026-10-11: обратные LCD/PWM transport failures

Native ShortWrite0 для JPEG и fan PWM проверены. LCD останавливает producer,
показывает ошибку и active theme=null, сохраняя тему в TOML; explicit Apply
после снятия fault восстанавливает Theme. Fan PWM отбрасывает applied claim,
сохраняет curve и автоматически восстанавливается. В обоих сценариях остальные
consumers продолжают записи. Full native smoke/restart и587 Rust tests прошли;
fmt/Clippy/doc/review без ошибок. Первый native запуск выявил неверную проверку
active-vs-persisted theme в тесте; ожидание исправлено и сохранённая тема
проверена отдельно. [Evidence](evidence/2026-10-11-native-lcd-pwm-transport.json).
[Консолидированная transport matrix](transport-consumer-acceptance.md) закрывает
базовую software independence. Hardware не использовался; планы ещё открыты.

## 2026-10-11: preview recovery при смене контекста

Добавлены6 browser regressions: late success/error после draft/device/reconnect,
current error/explicit retry, single-flight, актуальный target/draft и отсутствие
set/clear. Полный выбранный набор13 tests прошёл при последовательной проверке;
svelte-check0/0. В первом запуске12/13, старый reconnect test сбросил draft при
параллельном sync; повтор без него прошёл, точная причина не доказана.
[Evidence](evidence/2026-10-11-preview-recovery.json). Production код не менялся;
Rust/native/hardware не запускались. Реальный asset-I/O preparation gate остаётся.

## 2026-10-11: удерживаемое asset I/O и закрытие software ordering gate

Два отдельных Unix IPC сценария удерживают настоящий read private PNG в
production loader, пока другой client завершает Clear или JPEG replacement.
Поздний Apply получает superseded, не меняет source/generation/error, frames
и persisted theme. Явный следующий Apply успешен. C strict compile и Python
syntax прошли. Production код не менялся, Rust/GUI suites не повторялись.
[Evidence](evidence/2026-10-11-preparation-read-gate.json),
[консолидированная ordering matrix](theme-preparation-order.md).
Basic plan теперь8 пунктов; M1/M2 сохраняют по6. Полностью закрытых планов нет.
Итоговый общий software прогон и hardware остаются отдельно.

## 2026-10-11: исправлен focus при удалении показателя Cycle

Регрессия воспроизвела потерю keyboard focus после удаления последней строки
цикла. После DOM tick фокус переводится к ближайшему оставшемуся source;
проверка identity/isConnected защищает заменённый draft. Покрыты3→2 и2→1,
идентичность sensor и следующий Tab.13 browser tests, check0/0 и build прошли.
[Evidence](evidence/2026-10-11-cycle-keyboard.json). Изменён только frontend;
Rust/native/hardware не запускались, общий accessibility gate остаётся.

## 2026-10-11: opt-in ASUS WMI CPU voltage

Поддержан documented CPU Core Voltage у asus_wmi_sensors, с mV→V, явным
LCONNECT_ASUS_WMI_VOLTAGE=1 и semantic identity независимо от номера канала.
SOC/VID не принимаются, duplicate labels invalid. Fixtures проверили source
selection/recovery/renumber, zero/invalid/stale.588 Rust tests, fmt/strict Clippy/doc
прошли, review без замечаний. [Evidence](evidence/2026-10-11-asus-wmi-voltage.json).
Новых host sysfs/WMI/HID queries не было; BIOS/accuracy acceptance отложена.
Другие CPU voltage providers и NVIDIA platform coverage ещё открыты.


## 2026-10-11: актуальная очередь после ASUS WMI и ordering acceptance

Повторно сверены все три активных плана: M1 —6, M2 —6, basic —8 открытых
пунктов. Новых полностью выполненных файлов нет, поэтому дополнительных
удалений нет; прежние удаления сохраняются. Из roadmap и текущего checklist
убраны устаревшие pending-пометки transport/preparation/cancel/replace.
Исторические checkpoints сохранены с их исходными границами.

[Текущий аудит](evidence/2026-10-11-plan-reconciliation-current.json) фиксирует
counts, hashes планов и совпадения выбранных source checkpoints. Это проверка
документации и сохранённых доказательств, не новый запуск тестов. Полная
same-tree приёмка, дополнительные sensor/text semantics, GUI accessibility/
concurrent mutations и M4–M6 остаются открыты. Hardware не использовался.


## 2026-10-11: ASUS WMI через IPC и LCD renderer

Существующий cache-only integration scenario расширен synthetic WMI rail.
Проверены900mV→0.90V, actual preview RPC/JPEG, age>3s unavailable и восстановление
прежнего JPEG свежим sample без изменения theme binding.26 focused host-sensor
checks и588 workspace tests прошли; fmt/strict Clippy/doc и review без замечаний.
[Evidence](evidence/2026-10-11-asus-wmi-preview.json). Production код не менялся;
native GUI и hardware не запускались. Общая приёмка и остальные gaps открыты.


## 2026-10-11: Cycle software acceptance

Добавлены cache/runtime и IPC disconnect/restart сценарии. Real stale>3s,
missing/recovery, раздельные стили и reorder/remove сравниваются с single-sensor
JPEG. После teardown IPC connection и manager restart проверены обе полные
последовательности mock JPEG reports. Первый тест ошибочно наблюдал diagnostic
sent_frames; oracle исправлен, production код не менялся.7 focused daemon и2
renderer tests прошли;590 workspace tests, fmt/strict Clippy/doc, review чистый.
[Сводная matrix](cycle-acceptance.md), [evidence](evidence/2026-10-11-cycle-acceptance.json).
Software строка Cycle закрыта; native Cycle GUI close и hardware отложены.


## 2026-10-11: исправлен CSS em scale в renderer

Windows FontSize передаётся в CSS px; ab_glyph PxScale обозначает metric height.
Для line_box:true введено единое преобразование size*height/units_per_em для
wrapping, alignment, baseline и outlines. False/omitted сохраняет legacy scale.
Noto40px regression сначала воспроизвёл неверный перенос, затем прошёл79/80px
boundary; headless Chromium с тем же OTF независимо дал80px.32 image tests,
591 workspace tests, fmt/strict Clippy/doc и review прошли.
[Совместимость и открытые ограничения](text-em-sizing.md),
[evidence](evidence/2026-10-11-css-em-sizing.json). Native/hardware не запускались;
полный Windows shaping/raster parity не заявляется.


## 2026-10-11: внешние изменения controls сохраняют GUI draft

4 новых browser сценария проверили cooling/thermal/dynamic polling, explicit
Load и блокировку Apply при смене RGB base mode; set-записей нет. Вместе с
существующими reconnect/late-reply tests прошли9; svelte-check0/0.
[Матрица и границы](external-control-acceptance.md),
[evidence](evidence/2026-10-11-external-controls.json). Production код не менялся,
Rust/native/hardware не запускались. Остаток error/draft gate сохраняется.


## 2026-10-11: общий frontend/release/native checkpoint

Check0/0, production frontend build,26 frontend→Rust theme contracts и все87
browser tests прошли. Release workspace build прошёл. Native GUI/MockBackend
пересобраны с CSS em correction. Первый native smoke остановился на ожидании
`reject lost reply visible`, alerts были пусты. Добавлены только diagnostics:
DOM body и profile proxy counters в failure.json. Повтор с неизменными assertions
прошёл все native scenarios, final GUI close и restart/replay. Причина первого
timeout не установлена; retry не объявляется исправлением.
[Evidence с288 source hashes и native result](evidence/2026-10-11-combined-software-checkpoint.json).
Аппаратных запросов не было; итоговая приёмка остаётся открытой.


## 2026-10-11: profile readback после общего socket failure

Deterministic rejected-disconnect случаи для save/rename/delete/reset показали,
что reconnect может законно удалить старый alert вместе с загруженной library.
Native oracle принимает оба readback-required состояния и по-прежнему требует
недоступности Load/Save, отсутствия success, сохранности draft/LCD, точной
library revision/content и единственных proxy writes.20 browser tests,
check0/0, native build/smoke прошли. В этом native запуске все4 faults дали
mutation-error; альтернативный путь подтверждён browser fixture.
[Evidence](evidence/2026-10-11-profile-readback-race.json). Production код не менялся.
Первый исторический timeout совместим с этим порядком, но trace не доказывает
его причину; diagnostics сохранены. Аппаратных запросов не было.


## 2026-10-11: EXIF orientation и static import

Shared decoder применяет EXIF orientation до normalization/fit, сохраняя
лимиты декодирования. Все восемь преобразований, отсутствие повторного поворота
и PNG/JPEG/WebP/BMP проверены. Старые assets требуют повторного импорта оригинала.
34 image tests и общий Rust-прогон: 593 passed, 0 failed/ignored, 92 targets;
fmt, strict Clippy и docs прошли. Review без замечаний.
[Acceptance](image-import-acceptance.md), [evidence](evidence/2026-10-11-image-orientation.json).
Native checkpoints предшествуют исправлению; физический LCD не проверялся.

## 2026-10-11: внешние Display/Settings updates

Три новых browser сценария подтверждают сохранение несохранённой темы и обоих
LCD полей при external polling updates. Explicit Load берёт новые значения;
после external Clear сообщает отсутствие темы без потери черновика. Проверяется
отсутствие записывающих RPC. Production код не менялся. Check0/0 и14 browser
tests (external controls, reconnect, settings/library reconnect) прошли.
[Evidence](evidence/2026-10-11-external-display-settings.json). Native/two-client
concurrency и hardware не запускались; manual/thermal interrupted mutations остаются.

## 2026-10-11: interrupted manual/thermal browser acceptance

12 новых сценариев покрывают принятую команду с потерей ответа, отказ до
принятия, A→B→A/reconnect с late success/error. Отправлено91, новое draft94
сохраняется; один RPC, нет ложного success и auto-retry, fresh polling/readback
работает. Вместе с2 serialization cases —14 passed; check0/0. Production код
не менялся. [Матрица](lighting-interrupted-acceptance.md),
[evidence](evidence/2026-10-11-lighting-interrupted.json). Native faults и
физические изменения цвета этим checkpoint не подтверждаются.

## 2026-10-11: expanded accessibility и curve point focus

Исправлена воспроизведённая потеря клавиатурного фокуса после удаления точки
кривой охлаждения: после DOM flush фокус получает ближайшее оставшееся поле.
Раскрытые5 layer types/profile/font/cleanup прошли accessible-name и static
text contrast проверки, layers — reflow320.12 browser tests, check0/0,
production build прошли. [Evidence](evidence/2026-10-11-expanded-accessibility.json).
Native keyboard/AT, forced colors и полный expanded error/cancel matrix остаются.

## 2026-10-11: native keyboard на пяти страницах

Добавлен отдельный isolated Xvfb/Openbox стенд с настоящими X11 key events,
trusted keyup checks и Unix IPC readback.132 steps прошли: skip-link,
Settings99/90, fan PWM79, manual grace, CPU source и text Apply. Test-only
команда/permission отсутствуют в production handlers. [Acceptance](native-keyboard-acceptance.md),
[evidence](evidence/2026-10-11-native-keyboard.json).

В сопутствующем Rust-прогоне выявлено ненадёжное ожидание PID-файла NVIDIA
fixture. Тест теперь получает PID из child handle, проверяет наступление750ms
и reaping. Существующая production логика timeout/read/kill вынесена без
изменений в helper. Первый failure сохранён; его исходная причина не доказана.
Окончательная цепочка fmt/strict Clippy/593 tests(92 targets)/docs прошла,
review без замечаний. Native keyboard повторён на окончательном стенде.
AT, переключение окон, visual focus/zoom и полная parity остаются открытыми.

## 2026-10-11 — OpenType shaping и сверка планов

CSS LCD text получил GSUB/GPOS shaping и paragraph bidi; новые editor/preset styles включают CSS режим. Legacy themes не мигрируются.596 Rust tests,fmt/strict Clippy/docs,frontend check/build и26 schema contracts прошли. [Границы и evidence](text-shaping-acceptance.md).

Активные планы сведены к оставшейся работе: M1 —6, M2 —6, базовый —7. Удалять ещё один план оснований нет. Roadmap и реестр очищены от повторяющихся устаревших сводок.

## 2026-10-11 — CSS normal text wrapping

Исправлены принудительный разрыв длинных слов и отделение CJK punctuation
в CSS text mode. Unicode opportunities, NBSP и preserved trailing spaces
при выборе переноса проверены на девяти независимых DOM cases с тем же
Noto font; legacy layout сохранён.38 image tests и597 workspace tests прошли;
fmt/strict Clippy/docs прошли, review без незакрытых замечаний к изменению.
[Acceptance и границы](text-wrapping-acceptance.md).

Физическая приёмка по-прежнему отложена. Базовый план остаётся открытым:
эта проверка не закрывает остальные whitespace/locale/raster требования.

## 2026-10-11 — CSS trailing-space alignment

Renderer различает hanging spaces после soft wrap и conditional hanging
перед explicit/end break. Это исправляет center/right без изменения legacy
тем и glyph positions.39 image tests и598 workspace tests прошли,
fmt/strict Clippy/docs прошли; независимый browser oracle
записал27 сочетаний align/whitespace/bidi. [Acceptance](text-wrapping-acceptance.md#выравнивание-концевых-пробелов).

Остаток text parity не закрыт: tab stops, soft hyphens, locale/fallback и
Windows raster comparison требуют следующих проверок. Hardware отложен.

## 2026-10-11 — CSS tab stops

В CSS режиме tabs изолируются от shaping glyph runs и создают отступ
с шагом8spaces, с half-space minimum advance и reset каждой строки.
Legacy mode сохранён.40 image tests и599 workspace tests прошли,
fmt/strict Clippy/docs прошли;21 browser DOM case сохранён,
шесть координат сопоставлены с renderer. [Acceptance](text-wrapping-acceptance.md#табуляция).

Остаются soft hyphens, locale/fallback, сложные bidi/control combinations и
Windows raster comparison. Аппаратная приёмка отложена.

## 2026-10-11 — quickstart и восстановление

[Единый quickstart](../quickstart.md) связывает сборку, systemd/udev, GUI,
image/theme/realtime workflow, backup и troubleshooting. Исправлено
несовпадение cargo install root и ExecStart: оба используют ~/.local/bin.
README больше не смешивает сборку с низкоуровневыми hardware probes и
старым udev scaffold. Уточнены Intel VRAM и managed-vs-diagnostic persistence.

Собран текущий CLI;10 help paths проверены без IPC; пример theme прошёл
Rust schema validation. Relative links и diff whitespace проверены.
Установка, production daemon и hardware commands не запускались. Базовый
план теперь содержит6 открытых пунктов; M1/M2 по6. Ни один из трёх планов
пока полностью не закрыт.

## 2026-10-11 — font cancel keyboard focus

Исправлена воспроизведённая потеря focus после keyboard Cancel загрузки
шрифта: возврат к file input после DOM flush с guards против unmount,
disconnect и перехвата другого focus target.10 browser regressions прошли,
check0/0 и production build прошли. Focused Svelte self-review без замечаний;
новых effects/IPC нет, lint не настроен.
[Acceptance](accessibility-acceptance.md#отмена-загрузки-шрифта-с-клавиатуры-2026-10-11).
OS picker/native AT и остальные expanded workflows остаются открытыми.

## 2026-10-11 — общий software checkpoint

На неизменившемся snapshot303 source files прошли fmt/strict Clippy,
599 Rust tests (0failed/ignored,92targets), docs и release build workspace.
Frontend check/build актуальны на тех же исходниках;26 generated themes
прошли Rust schema, полный browser suite —110/110. Native WebKitGTK→Unix IPC→
MockBackend smoke прошёл, включая restart/recovery/resources и продолжение
rendering после закрытия GUI. Отдельный native keyboard —132 trusted steps.

[Source hashes и результаты](evidence/2026-10-11-integrated-software.json),
[полный native readback](evidence/2026-10-11-integrated-native.json),
[keyboard readback](evidence/2026-10-11-integrated-keyboard.json).
Применены private XDG roots/Xvfb и synthetic sensors; реальное железо не
использовалось. Незакрытые sensor/text/accessibility требования остаются: это
checkpoint текущего состояния, а не завершение basic parity или hardware gates.

## 2026-10-11 — Intel GPULoad SDK boundary

Pinned2.1.28 C# и PE export table устанавливают native boundary:
GetSensorValue ordinal373/RVA0x0030b82c, ReadDataFromSensor561/RVA0x0030b694.
Wrapper проверяет IsValid, manager выбирает один reading; формула Intel
aggregate находится за SDK boundary и не восстановлена. Установлены точные
пути Windows runtime exports и состав недостающей сверки; checklist дополнен.
[Evidence](evidence/2026-10-11-intel-load-boundary.json).

Provider inventory очищен от устаревших заявлений о незавершённых xe/VRAM,
clocks/network/storage и native engine selection. Новой sensor implementation
или hardware accuracy этот source audit не заявляет.

## 2026-10-11 — ручной мягкий дефис и сверка планов

CSS soft hyphen отображается только на выбранном мягком переносе;
учитывается ширина дефиса и повторный поиск переноса после более раннего
разрыва. Невидим в целой строке, перед явным newline и в конце текста.
Browser oracle и сравнение пикселей для трёх выравниваний прошли.
На соответствующих source hashes: fmt, strict Clippy, 600 Rust tests
(0 failed/ignored, 92 targets) и docs прошли; Rust review без замечаний.
[Evidence](evidence/2026-10-11-text-hyphen.json). Этот срез не имеет нового
native/hardware прогона; предыдущий общий checkpoint содержит 599 тестов.

Повторно сверены все три активных плана. Полностью завершённых среди них
нет; ранее удалённые десять устаревших планов не восстановлены. Из текущих
формулировок roadmap/gaps убраны уже закрытые cleanup, interrupted mutations
и типографические подзадачи. Открытые software/hardware gates сохранены.

## 2026-10-11 — отклонение IPC без замены активной темы

Расширен integration test с активной sensor-cycle темой: invalid version,
unknown fields, color/geometry/font/image references, malformed JSON и
oversized server input. Проверены неизменные theme/generation, доступность
другого клиента, сохранение и replay. Missing-font использует корректный
отсутствующий ключ; zero-width проверяет именно geometry error.
Закрыта соответствующая software строка общего checklist.

Fmt/strict Clippy/600 Rust tests (0 failed/ignored,92 targets)/docs прошли;
review без оставшихся замечаний. Production code не менялся.
[Evidence](evidence/2026-10-11-ipc-theme-rejection.json),
[границы проверки](schema-compatibility.md#отклонение-ipc-запросов-при-активной-теме--2026-10-11).
Native/physical acceptance этим прогоном не заявляется.

## 2026-10-11 — CSS glyph fallback

Primary→DejaVu Sans→Noto Sans TC fallback реализован для целых grapheme
clusters. Canonical composition сохраняет основной шрифт, invisible word
joiner сохраняет Arabic joining, SHY следует проверенному browser stack.
Measurement/wrapping и raster/shadow используют один выбранный glyph font.
Missing managed file остаётся ошибкой; legacy layout не меняется.

11 browser widths, CJK outline pixels, combining cluster, wrapping и legacy
regressions прошли;42 image tests и601 workspace tests (0failed/ignored,
92targets),fmt/strict Clippy/docs прошли. Review без оставшихся замечаний.
[Evidence](evidence/2026-10-11-text-fallback.json).
Windows system font resolver, locale/emoji/contextual coverage и native/LCD
acceptance остаются отдельными требованиями, не закрытыми этим срезом.

## 2026-10-11 — native acceptance актуального renderer

Расширен изолированный native smoke: CJK fallback различает glyphs, canonical
accent сохраняет Abril, mixed Cyrillic/CJK/tab/SHY layer проходит DOM Apply и
сохраняется при profile/restart. Общий сценарий recovery/resources/controls
прошёл; после закрытия GUI daemon отправил ещё3 JPEG frames.

На неизменных305 source files прошли frontend check0/0/build,26 contracts,
сборки mock daemon/native GUI и полный native smoke.
[Source hashes и readback](evidence/2026-10-11-native-text-fallback.json).
Физическое устройство/host sensors не использовались; native AT/window/zoom
и финальная приёмка всех остальных gaps остаются открытыми.

## 2026-10-11 — native переключение окон

Изолированный keyboard runner теперь открывает второе окно и проверяет
реальный Alt+Tab round trip: X11 active IDs, trusted WebKit blur/focus и
сохранение focus на Settings brightness. Далее прошли132 application key
steps пяти страниц с IPC readback. Native GUI rebuilt; JS/Python syntax и
diff checks прошли. Production code/capabilities не менялись.
[Evidence](evidence/2026-10-11-native-window-focus.json).
AT/zoom/dialogs, другие compositors и expanded states остаются отдельными
gates; AIO/host sensors не использовались.

## 2026-10-11 — platform accessibility tree

Optional AT-SPI fixture mode запускает private bus, обходя недоступную
systemd activation в isolated session. Read-only probe проверяет именно
lconnect-gui subtree: navigation/Settings roles/names, enabled/focused slider
после Alt+Tab. После probe прошли132 trusted keyboard steps/IPC readback.
Python syntax и diff checks прошли; production code/capabilities не менялись.
[Полное дерево и evidence](evidence/2026-10-11-native-atspi.json).
Screen reader speech/live regions, остальные страницы/expanded states и zoom
остаются открытыми. Host accessibility bus, sensors и HID не использовались.

## 2026-10-11 — native AT-SPI main controls пяти страниц

Fixture-only optional probe selector ограничен пятью именами страниц.
Проверены роли/имена/focus/enabled основных controls Settings/Cooling/
Lighting/Sensors/Display, затем132 trusted key steps с IPC readback прошли.
Rust fmt/strict Clippy/601 tests (0failed/ignored,92targets)/docs прошли,
review без замечаний; production capabilities не менялись.
[Полные деревья, hashes и результаты](evidence/2026-10-11-native-atspi-pages.json).

Выявлена незакрытая numeric semantics: WebKitGTK PWM number field имеет
role entry/computed-role textbox и не экспортирует Value interface.
Имя/focus/редактирование работают; screen-reader min/max/current требуют
проверки. Этот gap явно сохранён в checklist. Expanded states, speech/live
announcements, zoom и hardware остаются открытыми.

## 2026-10-11 — исправлен fixed PWM numeric bridge

Cooling fixed PWM/RPM теперь явно передаёт spinbutton/current/min/max/invalid;
reactive values не создают effects или RPC. Scoped redundant-role suppression
обоснован WebKitGTK experiment: без роли не было Value, с ролью native
экспортирует79/min10/max100 после Down, Apply подтверждён IPC.

11 browser regressions, frontend check0/0/build, native build и132 trusted
steps пяти страниц прошли. Svelte self-review без замечаний; lint не настроен.
[Evidence](evidence/2026-10-11-numeric-accessibility.json).
Curve/прочие numeric fields и speech/live announcements остаются открытыми;
аппаратных обращений не было.

## 2026-10-11 — числовая доступность Cooling curve

Temperature/PWM/RPM points, hysteresis и fallback получили проверенный
WebKitGTK spinbutton bridge с reactive bounds/current/invalid. Native fan
curve экспортирует все6 numeric controls через Value;30→31 отображается
в AT-SPI, daemon сохраняет fixed policy до явного Apply.

147 trusted keyboard steps пяти страниц,11 browser tests,check0/0/build и
native build прошли. Focused Svelte self-review без замечаний; lint не настроен.
[Source hashes и результаты](evidence/2026-10-11-curve-numeric-accessibility.json).
Native pump/RPM, numeric других страниц, speech/announcements и hardware
остаются отдельными acceptance; новые HID/RPC/effects не добавлялись.

## 2026-10-11 — изоляция темы по physical serial

Добавлена регрессия manager lifecycle: другой serial на прежнем USB-пути
не получает тему/JPEG, исходный serial на новом пути восстанавливает
композицию с sensor binding и полный ожидаемый JPEG batch. После shutdown
обе сохранённые темы принадлежат своим ключам. Production код не менялся.

5 focused lifecycle tests, fmt/strict Clippy,602 Rust tests (0failed/ignored,
92targets), docs и companion review прошли. Первоначальная ошибка теста
с историей MockBackend по пути исправлена через baseline после joined teardown.
[Evidence](evidence/2026-10-11-theme-identity-replay.json).
Физический reconnect остаётся отложенным; общий software checklist не закрыт.

## 2026-10-11 — LCD controls во время периодического рендера

Исправлена блокировка LCD State/Clear/manual image до завершения periodic
render. Owner обслуживает команды во время ожидания; generation/source guard
отбрасывает старый результат и ошибку. Shutdown joins pending render; готовый
кадр имеет приоритет над очередью входящих команд. HID batch остаётся атомарным.

3 controlled-future tests,25 focused integration tests,fmt/strict Clippy,
605 Rust tests (0failed/ignored,92targets),docs и companion review прошли.
[Evidence и ограничения](evidence/2026-10-11-responsive-lcd-render.json).
Первичный Apply и общий IPC/cooling/RGB slow-encode критерий остаются открытыми;
physical latency и native UI этим прогоном не проверялись.

## 2026-10-11 — первый render вне LCD actor

`PreparedTheme` связывает runtime с проверенным первым JPEG. IPC выполняет
load/render под existing job permit до enqueue, manager использует тот же
путь для replay. LCD owner проверяет ticket перед записью, поэтому Clear/newer
intent инвалидирует подготовленный кадр. Regression с занятым blocking worker
подтвердила State/Clear до завершения raster и отсутствие stale JPEG.

93 focused tests,fmt/strict Clippy,606 Rust tests (0failed/ignored,92targets),
docs и companion review прошли. [Evidence](evidence/2026-10-11-initial-theme-preparation.json).
Остался отдельный IPC gap: последовательная обработка запросов одного stream
может задерживать команды за долгим Apply; общий slow-encode checklist открыт.

## 2026-10-11 — отзывчивость одного IPC соединения

Apply/preview получили ordered admission и bounded deferred reply. LCD ticket
резервируется до чтения следующего wire request; shared writer сохраняет целые
JSON lines и ID. Лимиты:4 transport replies и прежние2 daemon jobs. Остальные
RPC последовательны. Cancel connection drops waiter; actual work сохраняет permit.

Unix read-gate smoke на одном socket подтвердил State/Clear/image, fan PWM61,
pump RGB, busy при занятом daemon limit, superseded/recovery и preview без Apply.
Fresh MockBackend counters подтвердили PWM/RGB writes и отсутствие stale JPEG;
старый diagnostic sent_frames oracle исправлен. 607 Rust tests (0failed/ignored,
92targets),fmt/strict Clippy/docs,26 contracts и review прошли.
[Evidence](evidence/2026-10-11-deferred-theme-ipc.json).
Полный heavy CPU render criterion, peer EOF-specific regression, native GUI
и physical latency этим checkpoint не объявляются закрытыми.

## 2026-10-11 — закрыт software render-loop acceptance

Новый isolated runner создаёт actual CPU load обычным32-layer renderer:
preview/Apply3,63–3,83s wall,same-stream control replies0,07–0,23ms;
fan curve реагирует на95°C →100% во время рендера. Fresh mock counters
подтверждают PWM/RGB и отсутствие stale JPEG. Periodic Clear0,96ms,
оставшаяся CPU работа3,44s; после drain/recovery ровно2 ожидаемых JPEG.

[Evidence](evidence/2026-10-11-render-load.json). В чеклисте закрыт общий
software render-loop пункт с прежними snapshot/no-GUI checkpoints.
Rust production не менялся; syntax/diff/docs проверены,607-test checkpoint
не выдаётся за новый прогон. Hardware и длительный stress остаются отдельно.

## 2026-10-11 — исправлен applied state ручного cooling replay

Трёхэтапный IPC regression обнаружил, что manager восстанавливал raw PWM
прямо через HID, оставляя cooling applied state пустым. Replay fan/pump теперь
идёт через cooling owner: motherboard80 → restart → MCU67 → restart;
fan73, packet source/duty и durable policy cancellation проверены.

[Evidence](evidence/2026-10-11-pump-source-replay.json): fmt, strict Clippy,
608 Rust tests (0failed/ignored,92targets), docs; companion review без замечаний.
Успешный replay повторно ставит сохранение в очередь и нормализует source string.
Реальное железо и native GUI не запускались. Corrupt dependencies на restart,
общие acceptance строки и итоговая базовая приёмка остаются открытыми.

## 2026-10-11 — закрыт software persistence критерий

Corrupt managed PNG и font при restart дают Idle/restore error/no JPEG;
возврат точных bytes и restart восстанавливают тему и полный исходный JPEG
batch без Apply. Fresh store/backend на каждый запуск, сохранённые bindings
и весь DeviceSettings неизменны (соседние поля в fixture имеют defaults).
Production исправлений не потребовалось.

[Evidence](evidence/2026-10-11-corrupt-restart.json):3 focused tests,
fmt/strict Clippy,609 Rust tests (0failed/ignored,92targets), docs; review
без замечаний. Общий software persistence закрыт вместе с прежними native
missing-dependency, identity и schema/disk checkpoints. Аппаратная приёмка,
power loss и итоговая проверка всей базы этим не закрываются.

## 2026-10-11 — консолидирован software cooling acceptance

Общий cooling пункт закрыт по [матрице требований и assertions](lcdv2-m2-cooling-acceptance.md#полный-software-cooling-критерий--2026-10-11):
presets/custom curves/RPM, hysteresis/fallback, manual/source и replay.
Повторный targeted прогон: 16 tests passed,0failed/ignored.
[Evidence и source hashes](evidence/2026-10-11-cooling-acceptance-map.json).
Код не менялся;609-test checkpoint не выдаётся за новый полный прогон.
Hardware duty/RPM/source и native pump/RPM accessibility остаются открытыми.

## 2026-10-11 — native pump/RPM numeric acceptance

203 trusted keyboard steps, private Xvfb/Openbox/AT-SPI, MockBackend:
pump fixed RPM2201→applied55%, RPM curve31°C/2201→daemon requested/applied;
AT-SPI Value/ranges/focus и draft без Apply проверены. Прежние five-page/fan/
Alt+Tab сценарии прошли. [Evidence](evidence/2026-10-11-native-pump-rpm-accessibility.json).
Production UI/Rust не менялись; scripts syntax и native build прошли.
Speech/live regions, другие numeric/expanded states и zoom/dialogs остаются.

## 2026-10-11 — Lighting numeric semantics и thermal validation

Thermal thresholds и Dynamic RGB thresholds/speeds имеют spinbutton/Value/
invalid semantics; пустой/out-of-range thermal draft отклоняется до RPC.
Дробный thermal threshold согласован с HTML step=any; sensor options keyed.
17 browser tests, svelte-check0/0, frontend/native build и235 native keyboard
steps/9AT-SPI probes прошли. Native thermal threshold86/focus/range и IPC
readback проверены. [Evidence и промежуточные failures](evidence/2026-10-11-lighting-numeric-accessibility.json).
Повторный стабильный browser run прошёл после завершения параллельной сборки.
Native Dynamic RGB Value, speech/live regions и zoom/dialogs остаются открытыми.
Аппаратных writes нет; Rust suite не запускался повторно для frontend изменения.

## 2026-10-11 — native Dynamic RGB numeric acceptance

320 trusted steps/11AT-SPI probes прошли: breathing→Celsius31/61 и speed1/99,
RPM max3600, draft без записей, Apply/readback, disable→requested=null.
[Evidence](evidence/2026-10-11-native-dynamic-rgb-accessibility.json).
Scripts syntax/native build прошли; production не менялся. Общий Rust/browser
suite повторно не запускался. Speech/live regions, native invalid announcements,
zoom/dialogs остаются; hardware не использовалось.

## 2026-10-11 — mixed realtime sensors и общий software контракт

Девять host/device bindings проверены сравнением JPEG с literal Text layers:
units/°F, настоящий0RPM, независимые stale/invalid/wrong-unit/wrong-ID и recovery
без Apply. Общий sensor software критерий сопоставлен с отдельными
provider/IPC/identity tests; Intel aggregate/voltage gaps остаются открытыми.
[Evidence](evidence/2026-10-11-mixed-sensor-rendering.json):7 focused tests,
fmt/strict Clippy,610 Rust tests (0failed/ignored,92targets), docs; review без
замечаний. Production не менялся; hardware/host drivers не использовались.

## 2026-10-11 — общий software theme controls критерий

[Матрица](theme-controls-acceptance.md) связывает backgrounds, fits/alpha/shadow,
fonts/text/layout, sensors/clock/Cycle, порядок/clipping и editor/preview с
конкретными проверками. Добавлены portrait/landscape exact fit bounds и
watermark overlap/reorder/zero-opacity/edge clipping. Production не менялся.

[Evidence](evidence/2026-10-11-theme-controls.json):22 focused image tests,
fmt/strict Clippy,612 Rust tests (0failed/ignored,92targets), docs;10 browser
сценариев Display/background/presets/keyboard/live preview. Review без замечаний.
Первоначальный out-of-bounds ordinary image fixture заменён поддерживаемым
watermark positioning. Windows typography, GUI accessibility, M4 effects и
физическая приёмка остаются открытыми; native GUI повторно не запускался.

## 2026-10-11 — native Dynamic RGB invalid/recovery

Исправлен вызов AT-SPI Text API в test harness. 400 trusted keyboard steps /
12 AT-SPI variants прошли: invalid31/30, unchanged daemon requested config,
INVALID_ENTRY и текст ошибки, correction31/61 и снятие ошибки; остальные
five-page/RPM/thermal/Alt+Tab flows также прошли. Python/JS syntax checks pass.
[Evidence](evidence/2026-10-11-native-dynamic-invalid.json) сохраняет snapshots,
source hashes и причины двух предыдущих oracle failures. Production не
менялся; полный Rust/browser suite не повторялся. Speech/live announcements,
zoom/dialogs и hardware acceptance остаются открытыми.

## 2026-10-11 — RGB software acceptance mapping

27 targeted daemon tests прошли (0 failed/ignored): owner ordering, manual/
direct failure, retained-prefix restore, thermal priority/grace, dynamic
arithmetic/mode validation/binding recovery и durable enable/update/disable.
[Матрица assertions](rgb-software-acceptance.md) отделяет принятый runtime
контракт от открытых Windows mode catalog/sync/LED loops и физических зон.
Production не менялся; workspace/native/hardware не запускались повторно.
Полная RGB parity этим не объявляется достигнутой.

## 2026-10-11 — общий GUI software контракт

[Матрица](gui-state-acceptance.md) связывает пять страниц, bridge/Unix IPC,
draft/readback/applied/persistence/error, reconnect и preview с assertions
и отдельными историческими native checkpoints. Полный текущий browser suite:
111passed; svelte-check0errors/0warnings. [Evidence](evidence/2026-10-11-gui-state-acceptance.json).
Production не менялся; native/hardware/Rust заново не запускались. Общий GUI
software критерий закрыт. В базовом плане осталось5открытых пунктов;
accessibility, platform/reference gaps и итоговый checkpoint сохраняются.

## 2026-10-11 — контраст текста, статусов и ошибок

Shared browser helper и новый сценарий покрывают main-текст пяти страниц,
Settings/Display transport errors, curve validation/recovery и success
Cooling/Lighting/Sensors.11focused tests passed; svelte-check0/0.
[Evidence](evidence/2026-10-11-text-status-contrast.json) фиксирует исправленную
ошибку предположения fixture и ограничения метода. Production не менялся;
общая native accessibility, input values/popups/forced-colors и итоговый
checkpoint остаются открытыми.

## 2026-10-11 — Cooling numeric validation и диапазоны

Исправлены клиентские проверки PWM/RPM/curve/fallback и расхождение ranges
с daemon (температура −50…150°C, гистерезис0…10°C). Invalid draft отклоняется
до RPC; допустимые дроби pump PWM temperature/hysteresis сохранены; график
согласован с диапазоном.13browser tests, svelte-check0/0, frontend/native build,
400trusted native steps/12AT-SPI variants прошли. Review без замечаний.
[Evidence](evidence/2026-10-11-cooling-numeric-validation.json).
Rust validator не менялся; full Rust suite не повторялся; hardware не использовалось.

## 2026-10-11 — единый checkpoint после Cooling validation

Все проверки выполнены на неизменном source snapshot (328files; hashes до/после
совпали): fmt, strict Clippy all-targets/all-features,612Rust tests/92targets
(0failed/ignored), docs, workspace release build, frontend check0/0 и build,
26frontend→Rust theme contracts,113browser tests. Затем native400keyboard
steps/12AT-SPI variants и полный native smoke через Unix IPC/MockBackend:
profiles/assets/fonts, reconnect/lost reply/timeout, sensor/transport recovery,
cleanup и restart/applied. Оба native harness завершились успешно.

[Полный evidence, окружение, dirty status, source hashes и логи](evidence/2026-10-11-integrated-after-cooling-validation.json).
HEADbcaf7a2, незакоммиченные изменения включены в snapshot. Ничего не
устанавливалось, production daemon/железо не запускались. Общая цель ещё
не закрыта: voltage/platform coverage, Intel Windows aggregate, typography
и расширенная accessibility остаются. Итоговый gate после их завершения
сохраняется; этот checkpoint не заменяет проверку будущих изменений.

## 2026-10-11 — connection/persistence live regions

Добавлены стабильные polite atomic containers для connection и pending/saved;
persistence error остаётся отдельным alert.14browser tests, check0/0,
frontend/native build и400trusted steps/12AT-SPI variants прошли; review без
замечаний. Browser проверяет неизменность node и отсутствие повторных DOM
mutations на одинаковом polling; native проверяет connection live attributes/text.
[Evidence](evidence/2026-10-11-live-regions.json). Это последующее изменение
после общего checkpoint, не новый полный прогон. Speech/live events и другие
accessibility gates остаются открытыми. Hardware не использовалось.

## 2026-10-11 — native AT-SPI live events

Добавлен изолированный event listener: pending/saved polite и Dynamic RGB
validation assertive подтверждены реальными bus events.400keyboard steps /
12snapshots прошли,24live events сохранены. Python syntax/native build pass.
[Evidence](evidence/2026-10-11-native-live-events.json). Исправлен oracle для
child-added events; production не менялся. Connection reconnect event и
screen-reader speech остаются непроверенными; Orca в окружении отсутствует.
Hardware не использовалось; весь workspace повторно не прогонялся.

## 2026-10-11 — исправлены connection live events в WebKitGTK

Plain text/keyed span не давали captured live events при успешном UI
reconnect. Keyed paragraph в стабильном polite atomic region передаёт
children-added offline/online; декоративная точка скрыта от AT. Добавлены
fixture-only stop/restart probes.402keyboard steps/12snapshots/37events,
13browser tests/check0/0/builds прошли. Rust612tests/fmt/Clippy/docs и focused
reviews без замечаний. [Evidence](evidence/2026-10-11-native-connection-events.json).
Speech/zoom/dialogs и platform/reference gaps остаются. Hardware не использовалось.

## Native zoom acceptance — 2026-10-11

WebKit200%/400%, окно1280×960, viewport640/320CSSpx: по403trusted steps,
12AT-SPI variants; пять страниц без horizontal overflow, IPC readback и
упорядоченный offline→online event. Исправлена гонка завершения теста до
доставки final AT-SPI event. [Evidence](evidence/2026-10-11-native-zoom.json),
[границы](accessibility-acceptance.md#native-webkit-zoom--2026-10-11).
OS scaling/dialogs/speech/прочие expanded states остаются; планы не закрыты.

## Native image chooser — 2026-10-11

Настоящий GTK dialog: cancel возвращает focus без изменения темы; один PNG
дважды выбран клавиатурой, draft-only imports → explicit Apply → JPEG preview;
разные managed keys имеют одинаковые normalized bytes.434trusted steps,
12AT-SPI variants,34live events. Build/fmt/Clippy/syntax/review passed.
[Evidence](evidence/2026-10-11-native-image-chooser.json). Font/scaled dialogs,
focus после успешного импорта, speech и hardware остаются; планы не закрыты.

## Image import focus — 2026-10-11

Исправлены потеря фокуса после import и disabled-picker boundary32layers.
Новый focus/смена страницы не затрагиваются.13browserpassed, check0/0,
frontend/native build, scoped Svelte review. Native success/reselect focus
подтверждён после GTK chooser: 434steps/12variants/44events.
[Evidence](evidence/2026-10-11-image-import-focus.json). Native errors/limit,
font/scaled dialogs и hardware остаются отдельно; планы не закрыты.

## PNG chooser при WebKit zoom400% — 2026-10-11

Совместный сценарий завершён: настоящий GTK chooser при WebKit zoom400%
и viewport320CSSpx. Cancel, повторный выбор PNG, восстановление focus,
draft-only import, Apply и JPEG preview прошли;434keyboard steps,
12AT-SPI variants,41live events. Все пять page probes без горизонтального
переполнения. Frontend, mock и native builds прошли.
[Evidence](evidence/2026-10-11-image-chooser-webkit-zoom.json).

Масштабирование самого GTK dialog и OS font scaling этим не проверены:
изменён только WebKit zoom. Font chooser, native error/limit states,
speech и Wayland остаются открытыми. Измерения reflow относятся к page
probes, не ко всем раскрытым редакторам. Hardware не использовалось;
полный software suite не повторялся. Production код не менялся.

## Manager sleep/wake foundation — 2026-10-11

Добавлен упорядоченный sleep/wake API: pause discovery → remove IPC map →
joined teardown; wake → fresh actors/IDs → replay durable settings/theme.
Diagnostic stream не возобновляется. Два новых integration tests проверяют
blocked HID write, дубликаты, паузу при видимом USB, post-wake PWM,
brightness/theme, старый ID и shutdown во сне.614Rust tests/92targets,
fmt/strict Clippy/docs прошли; reviewer finding об oracle исправлен.
[Evidence](evidence/2026-10-11-manager-sleep-wake.json),
[оставшийся adapter и acceptance](suspend-resume-acceptance.md).
Production logind ещё не подключён; никаких hardware/host sensor reads.

## Production logind integration — 2026-10-11

Production main запускает sleep observer: delay-inhibitor, ordered signals,
joined teardown/reopen, reconnect с принудительной сверкой и host cache
invalidation. Семь private-bus tests проверяют FD release, blocked HID,
deadline, queued true/false, initial sleep/denial, owner loss и автоматический
supervisor recovery/shutdown. Дополнительно проверен cache publish barrier.
622Rust tests/92targets, fmt/strict Clippy/docs прошли; review findings
исправлены. [Evidence](evidence/2026-10-11-logind-integration.json).
Системная шина/сон/HID не использовались. Full bus restart, stalled RPC,
combined post-wake providers и hardware остаются в
[acceptance](suspend-resume-acceptance.md); планы не закрыты.

## Logind failure acceptance — 2026-10-11

Закрыты private-bus restart, отсутствие logind при первом запуске и stalled
health query. Тесты подтверждают реальные owned process kill/wait, supervisor
retry/reopen и освобождение inhibitor раньше RPC timeout без ответа сервера.
625Rust tests/92targets,91daemon lib tests, fmt/strict Clippy/docs прошли;
review без замечаний. [Evidence](evidence/2026-10-11-logind-recovery.json).
Production source hashes и lockfile совпадают с предыдущим checkpoint.
Системная шина/железо не использовались; combined post-wake приёмка остаётся.

## Combined sleep/wake — 2026-10-11

Synthetic CPU hot→sleep/wake→Stale→hot→cool прошёл через полный actor tree:
PWM fallback100/100→90/80→45/50, thermal RGB обеих зон, полные LCD JPEG batches
и persistence base colors/rules/theme. Final focused test, fmt/Clippy/docs
прошли; предыдущий full run626tests/93targets предшествовал четырём последним
packet assertions и не выдаётся за финальный общий прогон.
[Evidence](evidence/2026-10-11-sleep-combined.json). Production код не менялся.
Найден оставшийся gap: invalidation кэша не сбрасывает rate/counter baselines
внутри providers. Он остаётся в M1 acceptance. Hardware не использовалось.

## Sleep counter baselines — 2026-10-11

Закрыт gap предыдущего checkpoint: CPU/RAPL/network history и i915/xe counters
сбрасываются при смене sleep boundary; cached xe rows очищаются. Collection,
пересекающий границу, не публикуется; новая пара отсчётов восстанавливает rates.
Четыре provider regressions и один interrupted async collector test прошли.
Полный Rust-прогон:631tests/93targets, fmt/strict Clippy/docs; review без замечаний.
[Evidence](evidence/2026-10-11-sleep-counter-baselines.json).
Hardware/system bus/host sensors не использовались; GUI заново не проверялся.
Активные планы: M1 —5, M2 —6, базовый —5 открытых пунктов; полностью
закрытых планов для дополнительного удаления нет.

## Native font chooser — 2026-10-11

Настоящий GTK chooser в private Xvfb/DBus/WebKitGTK прошёл cancel → select →
повторный select того же TTF → выбор нового font asset клавиатурой → Apply.
Каждый импорт ждёт новый managed file и завершение upload; два сохранённых
файла побайтно совпали с fixture. IPC подтверждает выбранный font key, preview
возвращает JPEG; импорт и отмена сами не меняют активную тему.
Исправлена воспроизведённая потеря focus после успешного upload. Новый target
пользователя не перехватывается; уход со страницы и отмена покрыты browser tests.
6browser tests, check0/0, frontend/native builds, fmt и GUI strict Clippy прошли.
Нативный прогон: 539 trusted steps. [Evidence](evidence/2026-10-11-native-font-chooser.json).
Scope:100%/X11/C.UTF-8/MockBackend. Font chooser при WebKit zoom400%, OS
font/dialog scaling, native error/limit states, другие compositors и speech
остаются; физический LCD не проверялся.

## Font chooser при WebKit zoom400% — 2026-10-11

PNG и TTF cancel/select/reselect → focus → explicit font selection → Apply
и JPEG preview прошли вместе при WebKit zoom400%/320CSSpx:539 trusted steps,
12 AT-SPI variants и53 live events. Два managed font файла совпали с fixture.
[Evidence](evidence/2026-10-11-font-chooser-webkit-zoom.json).
Исходники совпали с предыдущим checkpoint; production не менялся.
Масштаб самого GTK dialog/OS font не менялся. Reflow измерен на page probes,
не во всех раскрытых editors. Speech, OS scaling, error/limit states и
другие compositors остаются; hardware и полный software suite не запускались.

## Native font errors/recovery — 2026-10-11

GTK chooser проверен с повреждённым TTF и sparse TTF размером16MiB+1.
Обе ошибки видны и отправлены как assertive AT-SPI events; focus возвращён
к picker, options и активная тема не изменились. Затем тот же корректный TTF
дважды импортирован и явно выбран/применён; ровно два managed файла совпадают
с fixture, ошибки очищены, daemon сформировал JPEG preview.
При WebKit zoom400% раскрытые error/import states имеют width320/viewport320.
543trusted steps/12variants/46live events, native build, fmt, GUI strict Clippy и syntax checks прошли.
[Evidence](evidence/2026-10-11-native-font-errors.json). Production не менялся.
OS/GTK font scaling, speech, другие editors/error/limit states и hardware
остаются отдельно. Это не полный software checkpoint.

## Общий software checkpoint после sleep/font — 2026-10-11

На неизменных329 source/config/asset файлах прошли631Rust tests/93targets,
119browser tests,26frontend→Rust theme contracts, fmt, strict Clippy, docs,
Svelte check0/0, frontend build и workspace release build. Mock и native
бинарники пересобраны; оба изолированных native harness прошли. Keyboard:
543trusted steps/12AT-SPI variants/51live events при WebKit400%.
[Исходники, окружение, команды и хеши логов](evidence/2026-10-11-integrated-font-sleep.json).
Это актуальный совместный checkpoint реализованных workflows, включая
sleep counter reset и font errors/recovery. Он не закрывает оставшиеся
Windows/source, OS scaling/speech и hardware требования; итоговая приёмка
после их выполнения остаётся в базовом плане. Rust1.82 не запускался;
проверен Rust1.96. Ничего не установлено и не опубликовано.

## NCT CPUVC voltage — 2026-10-11

Добавлен in0 CPUVC для девяти source-confirmed NCT6779/679x variants,
mV→V, CPU source selection и стабильная physical identity. VID/other pins
не объявляются CPU voltage.27host-sensor tests и полный632Rust/93targets,
fmt/strict Clippy/docs прошли; review без замечаний.
[Evidence и pinned kernel source hash](evidence/2026-10-11-nct-cpu-voltage.json).
Board wiring/accuracy остаются hardware gate. Общий631Rust/119browser/26contracts
checkpoint предшествует этому provider change; GUI не менялся и не запускался.

## X11 OS text scaling — 2026-10-11

Private XSettings DPI96/192, GTK chooser glyph19→37px, WebKit DPR1/2/4
и viewport1280/640/320 проверены; каждый из трёх прогонов прошёл543keyboard
steps/12AT-SPI variants/46events, включая PNG/TTF errors/retry/Apply/preview.
Изменён только harness; Mock/native builds и syntax checks прошли.
[Evidence и ограничения](evidence/2026-10-11-native-os-text-scaling.json).

## NVIDIA bounded reap/cancellation — 2026-10-11

Исправлено неограниченное ожидание kill в CSV/NV-CONTROL queries; Child остаётся
в provider до reap, cancellation не теряет quarantine. Voltage slots ограничены32
включая disappeared GPU. Общий helper также используется Intel memory/xe.
60focused/636workspaceRust/93targets, fmt/strict Clippy/docs и review прошли.
[Evidence](evidence/2026-10-11-nvidia-bounded-reaping.json). Hardware и GUI suite не запускались.

## Nouveau GPU voltage — 2026-10-11

Добавлен source-confirmed nouveau in0: driver-reported core voltage, mV→V,
стабильный binding и source selection; cache→IPC→LCD text/JPEG проверены.
27focused/636workspace Rust tests/93targets, fmt/strict Clippy/docs и review
прошли. [Evidence](evidence/2026-10-11-nouveau-voltage.json).
Proprietary NVIDIA вне X остаётся открытым; hardware и GUI не запускались.

## M4 media preparation — 2026-10-11

GIF/MP4 → contain/cover480×480 → H.264 manifest реализован в preparation tool.
7real-FFmpeg tests с decoder и private-socket CLI contract прошли; CLI build
и Python syntax прошли. [Контракт и продолжение](media-preparation.md).
Diagnostic limits сохранены; gallery/managed playback/restart ещё впереди.
Hardware и GUI не запускались; Rust production code не менялся.

## Prepared media library — 2026-10-11

Daemon IPC import/list/load и атомарное хранение version1 prepared clips
реализованы.4focused tests проверяют reload после restart без устройства,
corrupt/unknown-version/path rejection и concurrent quota. Полный Rust suite:
640tests/94targets,0failed/ignored. [Контракт](media-preparation.md).
Сохранённые clips ещё не привязаны к профилю/автозапуску; GUI gallery и удаление
с references остаются впереди. Никакие устройства или production daemon не запускались.

[Verification evidence](evidence/2026-10-11-prepared-media-store.json):
640Rust/94targets, fmt/strict Clippy/docs и companion review прошли.

## Prepared library CLI — 2026-10-11

Добавлены lcd import-clip/clips/play-saved-clip. Реальный CLI→private Unix IPC→
daemon store→restart→LCD actor→MockBackend проверен с полным frame packet batch
и LocalUI write/state после Clear. Исправлен UTF-8 panic в letterbox colour.
8focused/643workspace tests/95targets, fmt/strict Clippy/docs и review прошли.
[Evidence](evidence/2026-10-11-media-library-cli.json). GUI и hardware не запускались;
выбор media в профиле, autoplay/replay и GUI gallery ещё не реализованы.

## Persistent media selection/replay — 2026-10-11

set/get_lcd_media и CLI select-clip/selected-clip добавлены; device settings
сохраняют media key взаимоисключающе с theme. Actor повторяет выбранный clip
без конечного loop counter; manager восстанавливает его до публикации device.
Missing asset сохраняет key с restore error; ремонт/restart восстанавливает
playback, Clear снимает autoplay. Stale preparation не заменяет новый source.
644Rust tests/95targets, fmt/strict Clippy/docs и review прошли.
[Evidence](evidence/2026-10-11-media-selection-replay.json). Named media slots,
GUI gallery, media sleep/wake и hardware остаются; diagnostic limits сохранены.

## Prepared media sleep/schema acceptance — 2026-10-11

Software manager sleep/wake с выбранным clip проверен: no-write barrier,
stale attachment rejection, fresh-ID/full-frame replay и Clear без возврата
после restart. Legacy/media schema и сохранение файла при invalid/conflicting
writes проверены.645Rust/95targets, fmt/strict Clippy/docs и review прошли.
[Evidence](evidence/2026-10-11-media-sleep-schema.json). Production unchanged;
physical sleep, GUI и совместная media/logind fault приёмка остаются.

## Prepared media GUI selection — 2026-10-11

Display показывает сохранённые ролики и текущий выбор, позволяет select/stop
с возвратом LocalUI, сохраняя черновик темы. Поздние ответы старого device,
потеря устройства и error/retry проверены browser fixtures. Bridge разрешает
только metadata/selection. 122 browser tests, check/build и 645Rust/95targets,
fmt/strict Clippy/docs и Rust review прошли.
[Evidence](evidence/2026-10-11-media-library-gui.json). Native WebKit/AT-SPI
новой панели и hardware не проверялись. GUI import/progress/cancel,
thumbnails/delete, named media slots и long-video runtime остаются.

## Native GIF/MP4 preparation backend — 2026-10-11

Rust desktop library готовит GIF/MP4 в validated H.264 с contain/cover,
phase progress и cancellation/reap. Реальные синтетические источники
декодируются в480×480 с pixel checks; duration/dimension bounds, malformed
input и cancellation проверены.650Rust/96targets, fmt/strict Clippy/docs
и review прошли. [Evidence](evidence/2026-10-11-native-media-preparation.json).
Backend пока не подключён к GUI: upload/job/admission/publication — следующий
шаг. Нового browser/native/hardware прогона нет, UI не менялся.

## Managed GUI GIF/MP4 import — 2026-10-11

GUI загружает файл частями в один native job, показывает upload/prepare/publish,
позволяет отменить подготовку и обновляет библиотеку без Apply. Owned worker
переживает потерю invoke response; cancel удерживает admission до reap. Normal
window close ждёт cleanup либо publication; неоднозначная ошибка публикации
не запускает автоматический retry.

125browser tests, check/build, production-feature cargo check прошли.
Rust:655tests/96targets в полном прогоне, затем1добавленный publication-shutdown
test отдельно; fmt/strict Clippy/docs и review прошли.
[Evidence](evidence/2026-10-11-managed-media-import.json). Native end-to-end
chooser→FFmpeg→daemon и close behaviour ещё не прогонялись. Hardware отложен.
Следующий шаг — native import qualification, затем gallery thumbnails/delete,
named media slots и long-video runtime.

## Native media import qualification — 2026-10-11

Отдельный Xvfb/Openbox harness прошёл реальные GTK chooser cancel/bad/MP4/GIF,
WebKit→bounded native upload→FFmpeg→private daemon→GUI library без Apply.
После daemon restart сохранённые H.264 реально декодированы в480×480 с
contain/cover pixel checks. Cancel и обычный WM close завершают owned encoder
и удаляют staging. Harness failure cleanup проверен отдельной инъекцией сбоя.

656Rust/96targets, fmt/strict Clippy/docs/review, frontend/native builds и
прежний native smoke прошли. [Evidence](evidence/2026-10-11-native-media-import.json).
Native publication faults/close while Publishing, полный keyboard/focus/reflow/
AT-SPI media panel и hardware остаются. Основной следующий функциональный шаг —
gallery thumbnails/delete и named media slots.

## Selected media thumbnail — 2026-10-12

Реализован bounded native first-frame decoder144×144 с отдельным admission,
cancel/reap/shutdown и guard при abort ожидающей задачи. GUI выбранного ролика
показывает preview без Apply, отбрасывает старые ответы и освобождает Blob URL.
Исправлена гонка browser assertion: details toggle асинхронен, проверка ждёт cleanup.

128browser tests прошли; Rust review chain658tests/fmt/strictClippy/docs,
добавленный abort regression отдельно1pass, production check и native build прошли.
Native preview обоих форматов и fresh preview после restart прошли со второго
запуска: первый остановился на пустом AT-SPI chooser tree. Эта нестабильность
harness не объявляется исправленной. [Evidence](evidence/2026-10-12-media-thumbnail.json).
Gallery/delete/slots, media publication faults/accessibility и hardware остаются.

## Media deletion store foundation — 2026-10-12

Добавлены scoped MediaPin и атомарный acquire/delete с проверкой durable
references под gate. Четыре регрессии проверяют shared pins, отказ при
неизвестных ссылках, acquire/delete race, corrupt/symlink и освобождение quota.
Workspace:663tests/96targets, fmt/strictClippy/docs прошли.
[Evidence](evidence/2026-10-12-media-delete-store.json).
API ещё не подключён к runtime/IPC/GUI: до этого нужны pins в actor и pending
settings, strict offline/profile scan. Gallery deletion остаётся открытой.

## Media delete runtime/IPC/CLI/GUI — 2026-10-12

MediaPin проходит через preparation/actor/replay и settings queue; write failure,
retry/final flush и Clear до durable commit покрыты. Strict offline settings scan
отклоняет неизвестную/повреждённую схему и symlinks. Delete RPC/CLI и GUI confirmation,
lost reply/readback без автоматического retry, focus и отказ по ссылкам реализованы.

665Rust/96targets,131browser,fmt/strictClippy/docs, frontend check/build и native
прогон прошли. Последующий test-only clear-before-commit assertion проверен отдельно.
Native delete/reimport сохраняет LCD source; restart/cancel/close также прошли.
[Evidence](evidence/2026-10-12-media-delete.json). Hardware не запускался.
Named media slots, gallery cards/defaults, native deletion faults/accessibility
остаются; остальные active plans не закрыты и не удалены.

## Named media screen slots — 2026-10-12

screen_media занимает один из восьми screen slots, сохранение pinned до commit;
media deletion учитывает profiles.json. GUI save/load не меняют LCD и сохраняют
черновик темы; отдельный Apply и CLI profiles apply используют set_lcd_media.
Rename/reset/delete, shared-slot validation, missing asset, restart, sleep и
конкурентный Save/delete проверены.666Rust/96targets,133browser,fmt/Clippy/docs,
check/build и native workflow прошли. [Evidence](evidence/2026-10-12-media-slots.json).

Native harness потребовал PN0 responder в MockBackend и ожидание enabled reset
после Clear. Эти изменения проверены отдельно после общего Rust/browser chain.
GTK/AT-SPI chooser flakiness остаётся. Hardware не запускался; активные M1/M2/
basic планы остаются незавершёнными. Следующий M4 объём — gallery cards/defaults,
long-video runtime и video+sensor composition, затем editor/capture/effects.


## Повторная сверка планов и незавершённая gallery — 2026-10-12

Активны три плана (basic:5, M1:5, M2:6 открытых пунктов); новых полностью
закрытых документов нет. Выполненные thumbnail/delete/media-slot шаги убраны
из очереди индекса; история checkpoints убрана из хвоста roadmap и остаётся здесь.
Исторические audits/specs/captures сохранены.

В текущем дереве есть MediaGallery и общая очередь media-thumbnails.
Существующий `/tmp/media-gallery-browser.log`:9 passed /1 failed из10;
сценарий смены страницы/закрытия ожидал один dispatch, получил два.
Причина не установлена; gallery не принята. `/tmp/media-gallery-check.log`:
0 errors/0 warnings. Новый полный browser/native checkpoint отсутствует.
Аудит не менял implementation и не запускал тесты/железо.


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


## Defaults research и уточнение очереди — 2026-10-12

[Evidence](evidence/2026-10-12-gallery-defaults-reference.json) фиксирует source
hashes и ffprobe оригинального default_video:VP9-in-MP4,480×480,24fps,
586frames/24.416667s. Текущие10s/240AU не принимают даже этот baseline asset.
Уточнены три отдельные коллекции, protected defaults и полный Screen reset
50%/0°/8slots с profile rollback при save failure. Обязательная глобальная
единая gallery исключена как неподтверждённое требование; контексты background/
media/watermark остаются. [Дальнейшая реализация и acceptance](gallery-defaults.md).
Код и hardware не менялись; программные проверки этого turn не запускались,
кроме read-only ffprobe/PNG headers и document validation. Планы остаются открыты.


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

## Bounded media reader worker — 2026-10-12

`MediaReader` читает PinnedMedia вне async/HID actor с bounded prefetch,
циклическим read и cancellation-safe joined stop. После panic завершённый
handle снимается до возврата JoinError; повторный stop безопасен. Stop запрещает
выдачу кадров немедленно и очищает очередь после join, включая send через
permit, полученный до close. Drop сигнализирует cancel, но не является barrier.

Проверены шесть unit cases и два integration-теста с реальным v1/v2 store:
586 frames/loop, worker-only и independent pending-save pins, truncation after
open. Review выявил race outstanding permit и маскировку worker pin тестом;
оба исправлены, повторный read-only review без замечаний.

[Evidence](evidence/2026-10-12-media-reader.json):682Rust tests/98targets,
fmt, strict Clippy, docs и production GUI check прошли. GUI/hardware не запускались.
Actor/replay/sleep и chunked/native import ещё не подключены; пользовательский
лимит10s/240frames/1MiB не изменился. Следующий шаг — LCD lifecycle integration,
сохранив generation/frame-batch barriers и firmware gate.

## Persistent media: LCD actor/replay integration — 2026-10-12

Сохранённый source передаётся как PinnedMedia через IPC и manager replay;
Source::Media читает v1/v2 через blocking worker. Stop/replace/fail и штатный
выход actor ждут reader; periodic receive допускает команды/shutdown, а
HID batch остаётся неделимым. Диагностический H264Clip и его limits не менялись.

[Checkpoint](evidence/2026-10-12-media-actor.json):685Rust tests/98targets,
fmt, strict Clippy, docs и production GUI check прошли; read-only review без
замечаний. Actor fixture сравнивает все586frames и начало следующего loop,
Clear за незавершённым frame, media→JPEG replace, отсутствие старых AU и pins.
CLI/private Unix IPC/MockBackend v1/v2 проходят profile selection, restart,
missing/recovery, sleep/wake с новым attachment, Clear и protected delete.

Chunked publication, native long-video import и thumbnail остаются открытыми;
user import по-прежнему10s/240frames/1MiB. Дополнительная slow-read/first-frame
и corruption-during-playback qualification остаётся в checklist. GUI/native
и hardware не запускались; это не новый codec или physical playback result.


## Chunked persistent-media upload — 2026-10-12

`media_upload` передаёт один AU за запрос, сохраняет bounded index и last-AU
retry, публикует по постоянному upload id. Реализованы order/byte/frame bounds,
concurrent Finish, cancel/no-delete после commit и receipt после restart.
Payload стал anonymous; собственный publication namespace защищён directory
flock и orphan cleanup. Guard явно Unlock при Drop, включая inherited/duplicate fd.

[Checkpoint](evidence/2026-10-12-media-upload.json):693Rust tests/101targets,
fmt, strict Clippy, docs и production GUI check прошли. RPC fixture586AU
превышает8MiB совокупного JSON, но каждый запрос bounded; lost Finish reply
и fresh-service receipt проверены. Child-process kill без Drop проверяет
cleanup и сохранность saved asset/чужих файлов/symlink.

Первый lost-reply fixture ошибочно закрывал handshake; исправлен. Первый
общий прогон выявил flock lifetime при сохранённом duplicate fd; добавлен
explicit unlock и детерминированная regression, полный повтор прошёл.
Review orphan staging finding исправлен, повторный review без замечаний.
Native import/thumbnail/UI outcome ещё не подключены; hardware не запускался.
Активные планы остаются открытыми, следующий шаг — native v2 integration.

## Сверка планов и первого кадра v2 — 2026-10-12

Roadmap очищен от хронологии выполненных срезов и повторяющихся очередей.
Три активных плана содержат 5/5/6 открытых пунктов; новых полностью завершённых
планов нет. Десять устаревших планов были убраны ранее. Исторические checkpoints,
исследования и аппаратные gates сохранены.
[Результат сверки](evidence/2026-10-12-plan-audit-final-backlog.json).

В текущем коде `load_media_first_frame` читает один AU legacy/v2 в blocking job
под source pin; native thumbnail использует его вместо full-clip load.
Подготовлен native-сценарий v2 thumbnail без playback. Начатые до аудита
Rust fmt/strict Clippy/tests/docs/production GUI check и browser suite завершились
успешно; frontend check/build также прошли. Native v2-сценарий **ещё не запускался**.
Полный пользовательский long-video import остаётся следующим шагом:
действуют прежние 10s/240frames/1MiB. Во время аудита код не менялся,
новые software/hardware прогоны не запускались.

## Native v2 thumbnail и file preparation — 2026-10-12

[Первый кадр v2](evidence/2026-10-12-media-first-frame.json) проверен через
native WebKitGTK → private IPC → MockBackend: synthetic586-frame asset,
cyan144px preview, отсутствие неявного playback и delete после thumbnail.
Этот seed повторяет один IDR; он не является проверкой native long-video import.

`prepare_file` возвращает owned H.264 file и bounded index вместо полного
H264Clip. Synthetic VP9 586frames/24,416667s перекодирован, прочитан по AU и
декодирован обратно во все586frames; payload>1MiB. Byte budget/cancel/cleanup
проверены. Legacy adapter сохраняет10s/240AU/1MiB; MediaImports ещё использует
его. Подключение file owner к chunked publication — следующий шаг.

[Checkpoint](evidence/2026-10-12-media-file-preparation.json):695Rust tests/101targets,
fmt, strict Clippy, docs, production GUI check и native harness прошли.
Review без замечаний. Два native-прогона выявили гонку самого harness:
после v2 delete он фокусировал input до завершения GUI refresh/tick;
поздний возврат фокуса перехватывал Enter. Добавлено ожидание post-delete focus;
пересобранный native-прогон прошёл. Оба сбоя и cleanup зафиксированы.
Нового hardware run не было. Все три активных плана остаются открытыми.

## Native long-video import → v2 — 2026-10-12

MediaImports использует prepare_file и передаёт один AU/RPC. Transferring
можно отменить; commit переводит job в Publishing под mutex. При потерянном
ответе читается Status того же ID, при ошибке/отмене — Cancel с проверкой
Published receipt. Новые IDs и повторные assets автоматически не создаются.
Prepared-file cleanup завершён до terminal status; GUI обновлён до1hour/256MiB.

[Checkpoint](evidence/2026-10-12-native-long-import.json): workspace696tests/101targets,
затем финальные20GUI tests после дополнительной cancellation regression;
fmt/strictClippy/docs/production check, frontend check/build,140browser и native
прошли. Native chooser импортировал synthetic VP9 586frames, полный mock loop
дошёл до587frames без failed transactions; Clear, profile/delete/reimport,
restart, thumbnail, cancel и close с активным encoder прошли. Оба сохранённых
v2-файла декодированы после restart; MP4 содержит все586frames.

Review выявил terminal-before-cleanup при отмене успешной preparation;
исправлен explicit drop под mutex, добавлена real-file regression.
Новых findings нет. Это не hardware acceptance. Native socket faults/close
во время Publishing и полная accessibility остаются в чеклисте.
Следующий основной slice — video+realtime overlays; три активных плана остаются открытыми.

## Video overlay renderer foundation — 2026-10-12

Общий renderer умеет рисовать ordered theme layers поверх owned RGB480×480;
video pixels заменяют background, остальные semantics сохраняются.
ThemeRuntime берёт cache-only readings и clock для каждого compose, не вызывает
providers/HID. Старый JPEG renderer использует тот же общий путь.

[Checkpoint](evidence/2026-10-12-video-overlay-renderer.json):699Rust tests/101targets,
fmt, strictClippy, docs и production GUI check прошли; read-only review без
замечаний. Реальные четыре H264 кадра декодированы, скомпонованы с CPU updates,
missing/recovery и снова encoded/decoded без потери кадров. Первое ожидание
теста содержало лишний пробел перед°C; fixture исправлена, повтор прошёл.
UI/hardware не запускались; это основа, не готовое пользовательское video+theme.

[Контракт и оставшаяся приёмка](video-composition.md): bounded codec worker,
actor/barriers, persistence/profile schema, GUI preview/Apply и hardware.
Три активных плана остаются открытыми.


## Сверка планов и незавершённого worker — 2026-10-12

Повторно сверены все три активных плана: basic — 5, M1 — 5, M2 — 6
открытых пунктов; пересечения не являются независимыми задачами. Новых
полностью закрытых планов нет. Ранее удалённые устаревшие планы не восстановлены;
исторические audits/specs/evidence сохранены. Roadmap и индекс отражают
завершённый native long-video import и уже реализованную основу overlays.

Новый streaming composition worker пока не принят: последний focused run
дал 2 passed / 1 failed, реальный pipeline завершился `composition frame timed out`.
Причина не установлена; actor/IPC/persistence/GUI integration отсутствует.
699 passing tests выше описывают предыдущий renderer checkpoint, а не текущее
дерево с worker. Следующее действие — исправление и квалификация worker,
затем интеграция composition. [Evidence аудита](evidence/2026-10-12-plan-audit-worker-wip.json).
В этой сверке код не менялся, новые tests и hardware runs не запускались.


## Streaming composition worker foundation — 2026-10-12

Исправлен timeout предыдущего checkpoint: decoder отвергал aligned512×480
allocation при лимите230400. Теперь limit262144 с фиксированным480×480 output,
а `-xerror` завершает decoder при ошибке. Receive deadline сохраняется между
отменёнными ожиданиями; stop дожидается reader/render/codec cleanup и child reap.

[Checkpoint](evidence/2026-10-12-video-composition-worker.json): четыре focused
регрессии и 336 daemon tests/42 targets прошли; fmt, strict Clippy и daemon docs
прошли. Реальная композиция CPU40→80 прошла encode/decode60frames; oversize
input, stalled child reap, cancelled receive и AUD boundaries проверены.
Companion review без замечаний после исправления deadline. Изменений frontend
и аппаратных прогонов нет. Actor/IPC/persistence/GUI integration и расширенная
failure/performance приёмка остаются; три активных плана не закрыты.


## Version2 video-theme identity — 2026-10-12

`LcdTheme` версии2 хранит managed `background_video` вместе со всеми layers;
settings и screen profile сохраняют один объект. Legacy version1 не меняет
сериализацию. Version/key mismatch отклоняется. Profile Save проверяет media
и держит pin до commit; offline settings и profiles защищают video от delete.
Статический renderer явно отказывает без decoded frame, не подменяет фон.

[Checkpoint](evidence/2026-10-12-video-composition-schema.json): 706 Rust tests/
101 targets прошли; финальные10media tests после исправления review также
прошли. Fmt, strict Clippy, docs и production GUI check прошли. Review выявил,
что race test с одним IPC соединением сериализовал запросы; исправлено на два
соединения к shared server. Повторный review без замечаний. Это допускает
перекрытие операций, но не задаёт детерминированный barrier pin→commit.

Apply/replay и preview/GUI composition пока не подключены. Новый вариант
возвращает явную ошибку при статическом preview/Apply. Hardware/frontend/native
не запускались. Все три активных плана остаются открытыми.


## LCD actor composition bridge — 2026-10-12

Внутренний `SetComposition` принимает заранее подготовленный первый H264 AU,
использует существующие frame batches/firmware guards и возвращает полную тему.
Settings delta передаёт theme и media pin одним сообщением. Clear/replace/failure
joins активный worker; stale ticket stop/drop выполняется до ответа.

Review выявил queued-worker gap при shutdown: исправлены close/drain очереди
и awaited stop всех queued compositions до teardown. Регрессия удерживает
текущий image HID batch, ставит composition в очередь, запрашивает shutdown,
затем проверяет отсутствие queued writes и освобождение source. Дополнительно
исправлено освобождение pin при полном Replace настроек с удалённым video key;
неизменный snapshot сохраняет pin.

[Checkpoint](evidence/2026-10-12-video-composition-actor.json): 339 daemon tests/
42 targets и дополнительная финальная pin regression прошли; fmt, strict
Clippy прошли, daemon docs собраны до последнего внутреннего pin fix. Первый
финальный Clippy выявил лишний import тестового helper; удалён, повтор прошёл.
Повторный review без замечаний. Публичные IPC Apply/replay, preview/GUI и
cancellation-slot ownership подготовки ещё открыты. Hardware не запускался;
три активных плана остаются незавершёнными.


## Composition public Apply и replay — 2026-10-12

`set_lcd_theme` версии2 готовит runtime, media source и первый AU перед
`SetComposition`. Preparation permit передаётся blocking load, затем worker и
его supervisor; отменённый waiter не освобождает slot до cleanup. Успешный
первый HID frame освобождает slot без остановки playback. Manager использует
тот же подготовленный источник для startup и wake; v1 путь сохранён.

[Checkpoint](evidence/2026-10-12-video-composition-ipc.json): 341 daemon tests/
43 targets, fmt/strict Clippy/docs и production GUI check прошли. Real codec +
private IPC + MockBackend: повторные Apply, missing dependency без замены темы,
restart, sleep/wake с новым attachment, missing-media restore/recovery и Clear
прошли. Drop/stop lease regression проверяет reap до освобождения slot.
Первое ожидание нового теста использовало lowercase enum вместо Theme/Idle;
исправлена fixture, повторный focused и общий daemon прогоны прошли.
Review без замечаний. Preview/GUI, расширенная IPC fault matrix и performance
ещё открыты. Hardware не запускался; активные планы не закрыты.


## First-frame video-theme preview — 2026-10-12

Публичный preview версии2 читает один AU, декодирует ограниченный480×480 RGB
frame и рисует текущие layers через общий renderer, затем возвращает JPEG.
Cycle использует переданный elapsed_seconds; это первый видеокадр, не timeline
seek или animated preview. Owned IPC job удерживает slot до cleanup, media pin
сохраняется до окончания render. Ошибка/timeout/output overflow вызывают kill/reap.

Первый прогон выявил зависание на EOF: shutdown stdin не закрывал descriptor;
явный drop после записи AU исправил timeout. [Checkpoint](evidence/2026-10-12-video-composition-preview.json):
343 daemon tests/43 targets, fmt/strict Clippy/docs и production GUI check
прошли. Focused6tests и private IPC проверили sensor cache/cycle, video pixels
и text, offline/connected preview без LCD writes/state changes, освобождение
pin, missing media и decoder timeout/oversize reap. Review без замечаний.
GUI selector/flow, расширенные faults/performance и hardware остаются открытыми.
Новые hardware/native/browser прогоны не выполнялись. Три активных плана открыты.


## GUI video background — 2026-10-12

Библиотека роликов добавляет выбранный video в черновик темы без LCD writes.
Слои и gradient settings сохраняются; возврат к static background снимает video
key и возвращает schema v1. Screen slots сохраняют полную v2 composition.
Preview явно описан как первый кадр с текущими readings. Missing video
показывает ошибку Apply, фон не заменяется молча.

[Checkpoint](evidence/2026-10-12-video-background-gui.json): frontend check/build,
27 Rust schema contracts и142 browser tests прошли. Native isolated WebKitGTK
прошёл chooser/import/library и новый composition workflow: cyan pixels preview,
Apply/readback video+clock, Clear/persistence и последующее удаление video.
Read-only Svelte review без замечаний. Backend код не менялся, Rust workspace
tests повторно не запускались. Mock daemon/native GUI собраны.

Полная native accessibility, expanded faults/performance и физическая приёмка
остаются открытыми. Ни один активный план полностью не закрыт.


## Composition codec faults — 2026-10-12

[Checkpoint](evidence/2026-10-12-composition-codec-faults.json): восемь focused
codec tests, fmt и strict daemon Clippy прошли. Добавлены truncated decoder
stdout, malformed/oversize encoder output, encoder EOF: ошибка до frame timeout,
оба child reaped, preparation lease/source pin освобождены. Отдельно real FFmpeg
заполняет bounded queue; Stop без receive очищает очередь, join завершается,
ролик удаляется, повторный Stop безопасен. Это queue saturation, не доказательство
blocked third send. Review устранил scheduling race тестового decoder EOF;
финальный повтор прошёл, замечаний не осталось. Production Rust не менялся.

Полная IPC/cancellation matrix и измерения performance остаются. Hardware не
запускался; новых полностью закрытых планов нет.


## Composition IPC faults — 2026-10-12

[Checkpoint](evidence/2026-10-12-composition-ipc-faults.json): private IPC
проверяет early request EOF и потерянный Apply response. Transport wrapper
пропускает handshake/request, затем возвращает BrokenPipe на записи ответа.
Новая тема с отличающимся именем видна второму клиенту; frames продолжаются
после закрытия соединения. Последующие Apply и исходные restart/sleep/missing
media/Clear/delete сценарии прошли. Strict daemon Clippy/fmt прошли.

Review устранил ложноположительный oracle: исходно повтор той же темы не
доказывал новый commit. Финальный тест использует отличающуюся identity.
Early EOF не доказывает cancellation во время активного codec prepare.
Эта матрица и performance остаются; production код и железо не менялись.


## Initial composition performance — 2026-10-12

[Release checkpoint](evidence/2026-10-12-composition-paced-release.json):
720frames с deadlines24fps, sensor40→80°C и clock layer; 0frames позднее
одного периода, startup30.02ms, receive wait p95 0.0062ms/max0.0079ms.
CPU test harness+codecs1.08s за29.99s (~3.6% одного ядра), sampled RSS
sum peak130.5MiB. Queue<=2; первый/720-й кадры декодированы, sensor region
изменился на475pixels. Stop завершён, source pin освобождён и media удалён.

Это начальная оценка простого blue video loop. Buffered receive wait не
является latency renderer/sensor→LCD; RSS sum не является unique peak memory.
Максимальные layers, сложное video, actor/HID и длительный soak остаются.
Release acceptance, fmt и strict daemon Clippy прошли; review без блокирующих
замечаний. Production runtime не менялся, hardware не запускался.

## Сверка очереди и точечное image delete — 2026-10-12

Точечное удаление PNG реализовано через IPC/GUI с revision и draft guard.
Shared image leases удерживают decoded assets в runtime и pending settings;
сохранённые настройки и profiles проверяются до удаления. Bulk cleanup по-прежнему
консервативно удерживает session keys. Backend 3 tests и strict workspace Clippy
прошли ранее; текущий focused browser повтор: 7 passed. Native image delete
ещё не принят; полный 145-test browser run включал изменение frontend во время
прогона и не объявляется qualification неизменного snapshot.

Актуализированы roadmap, индекс трёх активных планов, gaps, checklist и media/defaults
контракты. Новых полностью закрытых планов нет: basic — 5, M1 — 5, M2 — 6
открытых пунктов с пересечениями. GUI composition, выполненные codec/IPC faults,
начальное performance измерение и реализация image delete исключены из очереди
разработки; оставшиеся acceptance gates сохранены отдельно.
[Сверка](evidence/2026-10-12-plan-audit-after-image-delete.json). Железо не запускалось.

## Embedded image defaults — 2026-10-12

В библиотеку включены собственные «Орбита» и «Полночь»: PNG480×480, стабильные
versioned keys, загрузка/thumbnail/metadata из бинарника. Каталог не требует
disk bootstrap; user files не подменяют defaults. Daemon запрещает delete/rename,
cleanup не рассматривает builtin keys, GUI показывает маркер и оставляет Add.
Это image часть defaults; video/context/reset ещё не реализованы.

Focused Rust15/browser8, frontend check/build, strict workspace Clippy, fmt/docs
прошли. Генератор воспроизводит PNG побайтно. Rust/Svelte review без замечаний.
Native достиг sequence11 после builtin thumbnails/protection и user image
Delete/readback/focus; затем AT-SPI не нашёл path/Open в последнем chooser_slow.
Полный native workflow не закрыт, acceptance.json отсутствует. Аппаратные тесты
не запускались. [Evidence](evidence/2026-10-12-builtin-images.json).

Общий повтор Rust workspace/all-targets/all-features завершился:719passed,
103targets,1ignored (отдельный paced performance test). Первый прогон завершился
SIGTERM/143 после362passed без failed assertions; причина не установлена.
Повтор имеет exit0 и является результатом этого checkpoint.

## Embedded video default и native media checkpoint — 2026-10-12

Добавлен собственный видеофон «Сияние»:576frames/24fps/480×480,400236bytes.
Immutable embedded index+H264 обслуживается через обычный pinned source/reader;
на read копируется один AU. Встроенный key защищён от delete/publication/receipt
recovery и disk shadow, не расходует64user quota. IPC `is_default` и GUI marker/
delete guard добавлены; playback/background selection доступны. Legacy whole-clip
RPC явно отклоняет длинный builtin. Генератор и provenance сохранены в repository.

Финальный workspace:721passed/104targets/1ignored, exit0. Strict Clippy,
fmt/docs, frontend check/build и8 focused browser tests прошли. Первые два CLI
падения были ожиданием пустого каталога; исправлены на builtin+user entries,
CLI и полный workspace повторены успешно. Rust/Svelte review без замечаний.

Полный native media harness прошёл в `/tmp/lconnect-native-media-6tvhnd9d`:
builtin preview/Apply/Clear/protection, image defaults/delete/focus, chooser/import,
586-frame long loop, gallery/profile/restart, composition и close-worker reap.
Тест теперь ждёт enabled Stop после Apply, AT-SPI helper обрабатывает события
GLib перед опросами. Предыдущие Clear/chooser failures сохранены в evidence;
этот успешный прогон не закрывает speech/scaling или все native faults.
[Checkpoint](evidence/2026-10-12-builtin-video.json). Hardware не запускался.

На момент builtin-video checkpoint контексты галерей ещё оставались; они реализованы следующим срезом ниже. Full Screen reset/recovery остаётся. Ни один из трёх
активных планов пока не закрыт полностью.

## Контексты галерей и статический фон — 2026-10-12

Реализованы image/background/watermark selection, optional v1 background_image,
четыре fit, alpha compositing под overlays и watermark поверх. Переключение
image/video/solid сохраняет overlays; selection меняет только draft. Background
references защищены при preload/live lease/settings/profiles/delete/cleanup.
Старые темы совместимы; старый daemon отклоняет новое поле явно.

Fmt, strict Clippy, workspace tests (724passed/104targets/1ignored), docs,
Svelte check (0errors/0warnings), build, 11 focused browser tests и31 cross-language
contracts прошли. Rust/Svelte review: actionable findings нет.

Native static background/watermark preview→Apply→Clear прошёл как начало обоих
прогонов. Полный первый прогон остановился на Clear после profile Apply:
harness кликал до снятия disabled. Добавлено ожидание enabled. Повтор остановился
на GIF chooser: AT-SPI windows/controls пусты. Полного успешного native прогона
на этом состоянии дерева нет; retry не скрыт. Hardware не запускался.
[Evidence](evidence/2026-10-12-gallery-contexts.json).

Планы повторно сверены: остаются basic parity (5), M1 (5), M2 (6), ни один
не завершён целиком. Defaults/selection исключены из будущей реализации;
полный Screen reset, сохранённый состав коллекций и recovery остаются.

## Атомарный сброс группы профилей — 2026-10-12

Добавлен ProfileEdit::ResetGroup, CLI `profiles reset-group` и desktop
подтверждение сброса всей группы. Screen8slots/cooling4slots заменяются одной
revision и одной атомарной записью. Соседние группы и user media сохраняются;
Apply устройства не вызывается. Cooling reset требует явного датчика.

Проверены restart/readback, stale revision, binding validation, CLI request
sequence и browser cancel/lost reply/readback без повторной мутации. При ошибке
фокус переходит на обновление списка (исправлено по review). 727 Rust tests,
104targets/1ignored, strict Clippy/fmt/docs, Svelte check/build,31contracts и2
focused browser tests прошли. Native новой кнопки/hardware не запускались.
[Checkpoint](evidence/2026-10-12-profile-group-reset.json).

Полный Screen reset остаётся открытым: per-device settings и глобальные слоты
сейчас имеют разных owners; нужны общая durable операция, коллекции и recovery.
Требования записаны в [контракте](gallery-defaults.md). Планы не закрывались.

## Screen reset: journal и settings owner — 2026-10-12

Добавлен общий redo journal для device settings и profiles. Настройки и профили
читаются/изменяются через shared gate с recovery. Published intent доводится до
обеих записей; внешний snapshot conflict и corrupt journal сохраняются для
явного recovery. Настройки reset: brightness50/rotation0, базовая clock theme,
очистка media choice и8shared slots, без изменения cooling/lighting/других групп.

Settings owner принимает reset по своей очереди, учитывает pending deltas,
сохраняет receipt и не повторяет операцию после потери ответа. Дубликат не
снимает dirty flag и leases более поздних изменений. Неопределимое recovery
останавливает writer без stale flush. Reset/recovery, обычный flush и attach read
перенесены в blocking workers; owner ordering остаётся последовательным.

Финальный workspace:736passed/105targets/1ignored; strict Clippy, fmt и docs
прошли. Rust review: blocking finding исправлен и перепроверен. Добавлены
interruption/conflict/restart, lost-reply+pending update, mixed-store race,
fail-closed writer и current-thread contention regressions.
[Evidence](evidence/2026-10-12-screen-reset-transaction.json).

LCD actor, коллекции и IPC/GUI полного reset ещё не подключены. Native/hardware
этого среза не запускались; ни один из трёх активных планов не закрыт полностью.

## LCD actor Screen reset — 2026-10-12

Reset теперь исполняется владельцем LCD: prepared default validation → durable
settings barrier → invalidate preparation/stop/join → Application50/0 → JPEG
и clock runtime. Duplicate ID не выполняет Apply повторно и сохраняет новые
настройки. Typed rejected/recovery-required разделяют обычный отказ и ситуацию,
где продолжать мутации нельзя. Transport error сохраняет receipt/intent и
возвращается как lcd.last_error; потеря caller reply не отменяет работу.

Actor regression выявила ошибку предшествующего storage slice: LCDSetting
не разрешён для обычного config. Persisted/runtime reset используют Application;
следующий brightness patch и повтор ID проверены. Held old frame/error после
reset отбрасываются. Real FFmpeg composition reset через MockBackend проверил
reap codec PIDs до ответа и освобождение pins до shutdown manager.

Финальный workspace:741passed/106targets/1ignored, strict Clippy/fmt/docs прошли.
Rust review без actionable findings; выявленная граница active-composition
coverage дополнена реальным codec-тестом. [Evidence](evidence/2026-10-12-screen-reset-actor.json).
Коллекции, внешний IPC/GUI, explicit reapply и combined lifecycle приёмка
остаются. Hardware/native GUI полного reset не запускались; планы не закрыты.

[Коллекции: storage checkpoint](evidence/2026-10-12-screen-collections-storage.json): 744 tests passed, 1 ignored; fmt, strict Clippy и docs прошли.

[Membership owner checkpoint](evidence/2026-10-12-screen-collections-owner.json): 748 tests passed, 1 ignored; fmt, strict Clippy и docs прошли. Внешний IPC/GUI ещё открыт.

[Collection IPC checkpoint](evidence/2026-10-12-screen-collections-ipc.json): 749 tests passed, 1 ignored; fmt, strict Clippy и docs прошли.

[Legacy migration checkpoint](evidence/2026-10-12-screen-collections-migration.json): 752 tests passed, 1 ignored; fmt, strict Clippy и docs прошли.

[Collection panel checkpoint](evidence/2026-10-12-collections-panel.json): frontend check/build, browser workflows и 20 GUI crate tests прошли. Native ещё не проверен.

[Gallery integration checkpoint](evidence/2026-10-12-collection-gallery-integration.json): фильтрация по устройству/контексту, import и управление файлами вне коллекций проверены browser. Native ещё открыт.


## Сверка активных планов и reset WIP — 2026-10-12

Все три оставшихся плана проверены: basic parity — 5 открытых пунктов,
M1 — 5, M2 — 6. Новых полностью завершённых планов нет; прежние удаления
сохранены. Коллекции и gallery/import membership сняты с очереди реализации;
устаревшие открытые software пункты чеклиста согласованы с последующими evidence.

Внешние reset_screen/get_screen_reset добавлены, но не приняты: default
preparation до LCD admission позволяет старому reset заменить более новый
Apply/Clear. Нужны receipt-aware admission, stale-ticket rejection до durable
commit и регрессии для duplicate во время нового Apply. Explicit reapply,
GUI и native/lifecycle acceptance остаются. Аудит не менял код и не выполнял
новых software/native/hardware прогонов.


## Screen reset IPC/admission — 2026-10-12

Указанная выше гонка исправлена: admission выполняется до подготовки,
проверка ticket — до durable transaction. Duplicate возвращает receipt без
инвалидации нового Apply. Ordered get_screen_reset возвращает receipt,
persistence status и LCD state. Проверены stale revision, invalid ID,
повтор операции и сохранение пользовательских файлов.

754 workspace tests passed, 1 ignored, 109 targets; fmt, strict Clippy и docs
прошли. Focused Rust review без actionable findings.
[Evidence](evidence/2026-10-12-screen-reset-ipc.json).
GUI/explicit reapply/native/lifecycle остаются; hardware не запускался.


## Screen reset explicit reapply — 2026-10-12

`reapply_screen_reset` повторяет только transport после reset failure;
`get_screen_reset.reapply_available` отражает доступность попытки. Подготовка
свежего кадра вынесена за actor, применение проверяет operation/generation/ticket
и runtime identity. Новые display intents отменяют eligibility; duplicate reset
не отменяет. Никакого повторного сброса profiles/collections.

757 workspace tests passed, 1 ignored, 109 targets; fmt, strict Clippy и docs
прошли; focused Rust review без замечаний. Тесты покрывают config/frame failure,
успешное восстановление, новую яркость/pending theme, повторную ошибку и lost
success reply/readback. [Evidence](evidence/2026-10-12-screen-reset-reapply.json).
GUI и native/lifecycle acceptance остаются. Hardware не запускался.


## Полный Screen reset GUI — 2026-10-12

Панель с областью сброса и подтверждением подключена к reset/readback/reapply.
Она сохраняет локальный черновик, переживает смену вкладок, блокирует повторные
мутации до чтения результата и повторяет неопределённый reset только явно,
с тем же ID/revisions. Definitive revision rejection требует нового подтверждения.
Поздний receipt инвалидирует зависимые списки. Collection save не инвалидирует
собственный редактор; reset использует отдельный сигнал обновления.

Frontend check/build прошли; весь browser suite — 161 passed (3.9m),
GUI crate — 20 tests passed и strict Clippy/fmt. Первые 3 browser failures
исправлены и сохранены в evidence, полный финальный прогон прошёл на неизменных
исходниках. Focused Svelte/Rust review без оставшихся замечаний.
[Checkpoint](evidence/2026-10-12-screen-reset-gui.json).
Native/accessibility и combined lifecycle остаются; hardware не запускался.


## Screen reset: software lifecycle — 2026-10-12

Проверен reset через реальный manager/RPC implementation с MockBackend:
sleep/wake, changed-path reattach и fresh-store/daemon restart восстанавливают
receipt, применённую конфигурацию и базовую тему. Слоты/коллекции согласованы,
соседние настройки и user image сохранены; duplicate ID не повторяет сброс.
Отдельно injected config failure перед sleep: replay успешен, старый retry
недоступен. Lifecycle запускается после ответа reset; interruption in-flight
и native/hardware этим тестом не покрыты.

12 focused tests passed, daemon strict Clippy и fmt прошли.
[Evidence](evidence/2026-10-12-screen-reset-lifecycle.json).


## Reset во время sleep — 2026-10-12

Добавлен управляемый MockBackend gate на reset config write после durable commit.
При потерянном caller sleep дожидается записи и joined teardown, старая очередь
закрывается, wake восстанавливает committed default/receipt. Проверка ожидания
использует bounded timeout на том же sleep handle, который затем завершается.
3 lifecycle tests, daemon strict Clippy и fmt прошли.
[Evidence](evidence/2026-10-12-screen-reset-inflight-sleep.json).
Это не power-loss тест; in-flight detach/shutdown и native/hardware отдельно.


## Screen reset native keyboard/AT-SPI — 2026-10-12

Полный изолированный keyboard harness повторён в WebKitGTK/Tauri с private
IPC и MockBackend. Добавлены cancel/reset, injected JPEG failure после durable
commit, explicit reapply и возвращение focus. Receipt/slots/config проверены
через реальный IPC; readback/reapply controls доступны через AT-SPI.

440 trusted key steps, 13 AT-SPI snapshots, 37 live events. Native builds,
GUI strict Clippy/fmt и синтаксис harness прошли.
[Evidence](evidence/2026-10-12-screen-reset-native.json).
Проверка draft ограничена именем, файлов — image catalog; полная speech,
expanded reset scaling, ambiguous native reset reply и hardware остаются.


### GA2LCD Editor geometry, 2026-10-12

Чистая подготовка square crop → ordered rotate/flip → 480×480 PNG добавлена
в image crate. Bounds/alpha/EXIF/непереставляемый порядок и полный portrait
проверены; strict Clippy, crate tests и warning-free docs прошли.
[Контракт и оставшаяся интеграция](ga2lcd-editor.md),
[evidence](evidence/2026-10-12-editor-geometry.json). GUI/publication/filters
ещё отсутствуют; native/HID не запускались.


### GA2LCD image import editor, 2026-10-12

Geometry подготовка подключена к Tauri/GUI: исходник до crop, numeric/pointer
selection, ordered rotate/flip, undo/reset, explicit preview и managed
publication → collection → draft. После изменения соединения или заполнения
слоёв возвращается saved-only outcome, без ложного draft success.
[Результаты](evidence/2026-10-12-image-editor-gui.json). Gallery re-edit,
цветовые filters, video editor и native qualification остаются в editor checklist.


### Saved gallery image editor, 2026-10-12

Read-only full asset RPC и повторное geometry редактирование подключены.
Save заменяет запись выбранной коллекции с revision/readback, сохраняет старый
файл для references и меняет matching current draft только при прежнем selection.
Builtin guard, invalid source, context switch, конфликт/потеря ответа и focus
проверены. [Checkpoint](evidence/2026-10-12-gallery-image-editor.json).
Native/hardware и color filters/video editor остаются. Точные image/video
slider ranges извлечены из XAML: [контракт](ga2lcd-editor.md).

## VideoCut import — 2026-10-12

Необязательные ordered intervals в GUI → native upload → sequential preparation
→ existing publication. Source-derived180s total; local64interval cap. Реальный
codec test подтверждает reverse/repeat и цвета36frames; default import сохранён.
Полный video editor, native и hardware не заявлены.
[Контракт](video-editor.md), [evidence](evidence/2026-10-12-video-cut.json).

## Source video preview — 2026-10-12

Локальный source playback/scrubbing, capture start/end из player.currentTime,
explicit import, unsupported-codec fallback и object URL cleanup реализованы.
В текущей сверке прошли 10 browser tests, включая реальное декодирование кадров.
[Evidence](evidence/2026-10-12-video-source-preview.json). Native WebView и
результат spatial crop/concat этим не квалифицированы.

## Per-interval video crop backend — 2026-10-12

Optional validated normalized rectangle на каждом Segment, crop перед fit.
Реальный codec regression подтверждает разные области двух интервалов.
GUI/result preview ещё не подключены; Windows pixel equivalence не заявлена.
[Evidence](evidence/2026-10-12-video-crop-backend.json).

## Video crop GUI — 2026-10-12

Numeric/pointer selection для каждого фрагмента, keyboard move/reset focus,
strict percent validation → normalized DTO и local crop/fit frame preview
подключены. Prepared-result preview, native и physical qualification остаются.
[Evidence](evidence/2026-10-12-video-crop-ui.json).

## Prepared video review backend — 2026-10-12

Remux того же H.264 в seekable MP4, bounded disk range reads и ReviewReady
ownership с однократным подтверждением/cancel/shutdown реализованы. Tauri
transport и GUI пока не подключены; обычный import путь сохранён.
[Evidence](evidence/2026-10-12-video-prepared-preview-backend.json).

## Prepared video player/confirmation — 2026-10-12

Tauri range protocol/current-job scoping/CSP и UI ReviewReady → explicit
confirm/cancel подключены. Подтверждение публикует тот же prepared owner
в owned task; reload/lost reply восстанавливаются через status. Browser
проверяет real MP4 ranged decode/seek, explicit confirm, close/cancel и
lost confirmation reply. Native protocol/codec/seek ещё не квалифицированы.
[Evidence](evidence/2026-10-12-video-review-integration.json).

## VideoCut record/pause — 2026-10-12

Source playback record/pause с отдельным списком, per-interval crop snapshot,
EOF/pause, unexpected seek/error discard и180s automatic preparation добавлены.
Ручной draft сохранён. Исправлено millisecond EOF rounding; реальный25-frame
MP4 принимается с browser end1.042s, более поздний1.044s отклоняется.
[Evidence](evidence/2026-10-12-video-record-pause.json). Native/physical и
полный VideoEdit/AutoFill остаются открытыми.

[ScreenRecord GUI checkpoint](evidence/2026-10-12-screen-record-ui.json): frontend check/build и40 focused tests прошли; scope и ограничения указаны в evidence.

## ScreenRecord: предложение завершения на лимите

При LimitReached автоматически открывается modal Finish/возврат к паузе.
Escape сохраняет запись и возвращает focus на Finish; повторный polling не
открывает диалог снова. Finish не запускает Prepare/Save автоматически.
14 focused tests и frontend check прошли; это synthetic UI state, не полный
180s engine/native прогон. Region и Windows operation-choice/edit остаются.
[Evidence](evidence/2026-10-12-screen-record-limit-ui.json).

## ScreenRecord: trim/crop готовой записи

Готовая запись использует общий VideoSourcePreview; добавлены один временной
интервал VideoCut, числовая область кадра и указание границ по playhead.
Неверные поля блокируют Prepare, точный cut/crop DTO проверен browser test.
15 focused tests и frontend check прошли. Это post-recording edit; изменение
области во время записи и multi-interval workflow ещё не реализованы.
[Evidence](evidence/2026-10-12-screen-record-crop-ui.json).

## ScreenRecord: несколько фрагментов готовой записи

Добавлены1…64 фрагмента с независимыми crop и временными границами, выбор,
удаление, перестановка и общая проверка180000ms. Это заменяет прежнее ограничение
одним фрагментом.16 focused tests и frontend check прошли; decoded/native output
в этом срезе не проверялся. Изменение capture region во время записи остаётся.
[Evidence](evidence/2026-10-12-screen-record-fragments-ui.json).

## ScreenRecord: декодированный browser preview

Synthetic128×64 H.264 проверен в настоящем browser video: decode, seek0.5s,
playhead→trim, синие/красные pixels при смене crop фрагментов и освобождение
player при Close.17 focused tests прошли. Native IPC замокан; реальный capture,
WebKit и итоговый encoded crop по-прежнему требуют проверки.
[Evidence](evidence/2026-10-12-screen-record-browser-decode.json).

## ScreenRecord: native prepared crop decode

Synthetic GStreamer запись прошла admission→capture cancel→Prepare→MP4 decode.
24 кадра показывают красный crop, затем синий; source lease освобождается после
подготовки, review не публикует asset, Cancel отзывает preview. Постоянные полосы
не доказывают выбор исходных timestamps; это отдельный оставшийся критерий.
[Evidence](evidence/2026-10-12-screen-record-native-crops.json).

## ScreenRecord: native temporal oracle

Кадры результата сопоставлены с меняющимся шумом исходной записи без trim/concat
в oracle. Проверены24 кадра в порядке12…23, затем0…11 после native
recording/admission/prepare. Это закрывает пробел предыдущего статического
паттерна для24fps и границ500/1000ms; VFR/произвольные границы/Windows rounding
не квалифицированы. Read-only Rust review не нашёл actionable issues.
[Evidence](evidence/2026-10-12-screen-record-temporal.json).

## ScreenRecord: первый isolated native workflow

Tauri/WebKit + GStreamer synthetic source + MockBackend: source decode/seek,
prepared decode, явное Save, membership в явной коллекции и нормальное закрытие
окна во время второй записи прошли. Exit0, пустой staging, сохранённый clip
подтверждены до cleanup harness. Реальный portal/устройство не использовались;
native crop/edit/limit/fault matrix ещё открыта.
[Evidence](evidence/2026-10-12-screen-record-native-ui.json).

## ScreenRecord: возврат к редактированию

ReviewReady можно отменить к исходной записи, сохранив drafts; новая подготовка
получает новый ID. Ошибка отмены блокирует Save и допускает повтор возврата.
19 focused browser/client tests прошли, включая исправление доступного имени
списка размещения. Native повторная подготовка проверяется отдельным checkpoint.
[Evidence](evidence/2026-10-12-screen-record-return-edit.json).

## ScreenRecord: прекращение доставки кадров, 2026-10-12

Synthetic source прекращает доставку buffers после первых шести, оставаясь
отзывчивым на команды. Проверены независимый active-time limit, запрет Resume,
Finish с декодированием MP4 и удалением staging, а также Cancel с join worker.
Зависший plugin, реальные PipeWire keepalive и сохранение визуального хвоста
последним кадром этим не квалифицированы.
[Evidence](evidence/2026-10-12-screen-record-stopped-delivery.json).

## ScreenRecord: выбор действия, 2026-10-12

Ручное завершение сначала приостанавливает запись и предлагает Cancel/Edit/Import;
тот же выбор доступен при лимите. Отмена сохраняет запись на паузе, Edit открывает
исходник, Import готовит запись целиком со stretch и отдельным Save. Подготовка
ждёт Ready; Clear сбрасывает ожидающее действие. Устаревший ответ polling больше
не закрывает диалог. 26 целевых тестов, frontend check/build и synthetic native
manual Edit/Import с реальным декодированием прошли; дополнительные кадры после
Resume теперь проверяются native. Реальные portals и native automatic-limit/fault
матрица остаются открытыми.
[Evidence](evidence/2026-10-12-screen-record-operation-choice.json).

## ScreenRecord: capture-region backend, 2026-10-12

Смена нормализованной области реализована в engine и существующем control IPC,
только на паузе; status сохраняет последнюю принятую область. RGBA crop → scale
фиксирует размер MP4 и SAR1:1. Реально декодированные кадры подтверждают исходный
полный кадр и красный/синий интервалы после пауз; неверные phase/ID/coordinates
не меняют readback. GUI selection/preview и выбор до начала записи остаются.
84 обычных Rust-теста, strict Clippy/fmt/docs, default cargo check, отдельный
180s engine тест и существующий synthetic native workflow прошли. Native region
selection не проверялся, поскольку ещё не подключён к GUI.
[Evidence](evidence/2026-10-12-screen-record-region-engine.json).

## ScreenRecord: числовая область на паузе, 2026-10-12

GUI задаёт X/Y/ширину/высоту в процентах, проверяет прямоугольник и показывает
backend readback. Неприменённый draft блокирует Resume; можно вернуть принятые
значения или задать полный источник. Отказ и lost reply не выглядят успешным
применением без readback. Исправлено сравнение дробных процентов после round-trip.
30 целевых тестов, frontend check/build и synthetic native смена двух областей
с последующим Resume/decode прошли. Выбор/preview области мышью и настройка до
первого кадра ещё отсутствуют.
[Evidence](evidence/2026-10-12-screen-record-region-ui.json).

## ScreenRecord: snapshot полного источника, 2026-10-12

Engine хранит последний полный RGBA-кадр до crop; по запросу на паузе отдаёт
bounded JPEG через raw Tauri IPC. Проверены пиксели вне текущего crop, ограничение
640px/2MiB, неизменность времени/кадров и owner gates. 86 обычных Rust-тестов,
strict Clippy/fmt/docs и native IPC/WebKit decode прошли. Отдельного полного180s
прогона в этом срезе не было. Пользовательский preview/визуальный выбор области
пока не подключён; реальные PipeWire и оборудование не использовались.
[Evidence](evidence/2026-10-12-screen-record-source-snapshot.json).

## ScreenRecord: визуальный выбор области на паузе, 2026-10-12

Полный JPEG кадр до crop показывается на паузе. Выделение мышью, перемещение
стрелками и числовые поля редактируют общий черновик; Apply подтверждается
readback, Resume блокируется при неприменённых изменениях. Ошибку snapshot
можно повторить; поздний ответ после Resume не возвращает старый preview.

Сохранённые результаты: 33 целевых browser/client теста, Svelte check без
ошибок/предупреждений, frontend/native build и isolated Tauri/WebKit workflow
прошли. Native проверяет preview, DOM keyboard event, red/blue region Apply,
Resume, source/review decode, Save/membership и normal Close. Это synthetic
source, не захват рабочего стола; OS keyboard/native pointer, speech/scaling,
настройка до записи и реальные portals остаются открытыми. Preview — уменьшенный
последний полный кадр до паузы, не live preview и не pixel-exact результат.
[Evidence](evidence/2026-10-12-screen-record-visual-region.json).

## ScreenRecord: подготовка до первого кадра — движок, 2026-10-12

`Recorder::prepare` открывает источник в `Setup`: bounded full-source snapshot
доступен, кадры не проходят в videorate/encoder и active time остаётся нулевым.
`set_region` сохраняет выбранную область; первая команда Resume устанавливает
crop/output geometry до открытия frame gate. Временные метки начинаются с
первого допущенного кадра. Источник и portal session не переоткрываются.
Обычный `Recorder::start` сохраняет прежнее немедленное начало записи.

Synthetic regression ждёт в Setup дольше односекундного recording budget,
подтверждает нулевые counters/bytes, выбирает красную область, начинает запись,
делает pause/resume и проверяет **все** decoded frames, включая первый.
Результат23–24 кадра/0.95–1.05с; ожидание и пауза в файл не входят. Отдельно
проверены отсутствие source frame, отказ Resume и joined cancellation.

86 library +2 binary tests прошли, один180s тест оставлен ignored; fmt,
strict Clippy (all targets, native-smoke) и rustdoc прошли. Реальные PipeWire
segments/timestamps, изменение геометрии и hardware не проверялись.
**Tauri owner и GUI ещё используют немедленный start**: подключение Setup,
начального выбора/preview и явной кнопки начала записи остаётся открытым.
[Evidence](evidence/2026-10-12-screen-record-setup-engine.json).

## ScreenRecord: область до начала записи — Tauri/GUI, 2026-10-12

Production и native-smoke owner открывают `Setup`. «Выбрать экран или окно»
открывает источник; preview/числовые поля/Apply доступны до явного «Начать
запись экрана». Неприменённый или неверный черновик блокирует Start. Backend
отклоняет Pause/Finish в Setup без уничтожения recorder; отмена joins worker.
Сохранение остаётся отдельным действием после review.

87 library +2 binary tests прошли (один180s ignored), fmt/strict Clippy/rustdoc,
Svelte check,35 browser/client tests и frontend/native build прошли. Изолированный
Tauri/WebKit с synthetic source подтвердил initial preview/region readback,
нулевые frame/time counters до Start, последующую запись/смену регионов,
review/re-edit/Save/membership и normal Close. Поздние snapshot ответы отдельно
проверены в Setup и Paused. Это не приёмка настоящего системного capture portal,
изменения geometry/segments, fractional scaling или физического LCD.
Предыдущие сообщения об отсутствии owner/GUI Setup являются историческими.
[Evidence](evidence/2026-10-12-screen-record-setup-ui.json).

## ScreenRecord: повтор последнего кадра при отсутствии обновлений, 2026-10-12

Live `imagefreeze` с `allow-replace` формирует24fps из последнего исходного
кадра. При responsive delivery gap видео продолжает показывать его; новые
кадры заменяют удерживаемый. Upstream EOS и Session::Closed остаются ошибками.
Output admission barrier исключает кадры после Pause, CFR timestamps основаны
на числе допущенных кадров. Finish отправляет EOS после imagefreeze, чтобы
генератор повторов не задерживал завершение MP4.

89 library +2 binary tests, fmt, strict Clippy и rustdoc прошли. Двухсекундный
stopped-source тест проверяет длительность и decoded held pixels; отдельный
тест подтверждает возврат меняющихся кадров, finite-source тест — ошибку EOS.
Temporal-cut oracle допускает разные индексы только для одинаковых decoded
pixels и требует минимум12 различных кадров на24, сохраняя проверку порядка.
Изолированный native WebKit workflow с synthetic source/MockBackend прошёл.

Полный continuous synthetic прогон:4320frames,180s,35335378bytes,180.90s elapsed.
Первый запуск завершён SIGTERM после90s по неизвестной причине и не засчитан;
после подтверждения остановки отдельный повтор прошёл. Это обновлённая
квалификация текущего timing path, не результат прежнего videorate pipeline.
Реальные portals/PipeWire, source resize/segments, native deadlock и LCD
остаются открытыми. [Evidence](evidence/2026-10-12-screen-record-held-frame.json).

## ScreenRecord: полный native limit workflow, 2026-10-12

Изолированный Tauri/WebKit с production180s budget и synthetic source прошёл:
4320frames/180000ms → автоматический диалог → Cancel с возвратом фокуса к
Finish → стабильные counters и отсутствие повторного диалога → отказ Resume →
Import целиком → decoded review180s → seek179.5s → явное Save → membership.
Python harness подтвердил нормальный app Close, пустой staging и сохранённый
ключ в MockBackend. Запуск: `python3 desktop/scripts/native-recording.py --full-limit`
после frontend/native-smoke build. Обычный harness сохраняет короткий режим.

Первый прогон прошёл проверки, но поле отчёта duration сталоnull после video
teardown. Измерение сохранено до teardown; повторный полный прогон прошёл с
review_duration180. fmt, strict Clippy, build и JS/Python syntax прошли.
Реальные capture portals, OS keyboard/speech, limit errors и LCD не проверялись.
[Evidence](evidence/2026-10-12-screen-record-native-full-limit.json).

ScreenRecord contract консолидирован: актуальные возможности и открытые gates
отделены от [исторических checkpoints](screen-record-checkpoints-2026-10-12.md).

## Rain/Snow: native-scale parameters и renderer, 2026-10-13

Добавлены static1…3 и sensor identity/unit/low/high, извлечённые Rain/Snow
native-scale mapping, freshness/unavailable fallback и собственный bounded
480×480 renderer. `WeatherScene` интегрирует пройденный путь по предыдущей
скорости; sensor band transition не телепортирует снежинки. Неверное время или
canvas не меняют scene state. Stateless helper используется для fixed preview.

127 targeted tests и871 workspace tests прошли (2 ignored), fmt,
strict Clippy и rustdoc affected crates прошли; offline preview просмотрен.
Ревью выявило смену положения при speed transition; исправление принято с
pixel continuity/future-motion/rewind regression. Theme/H.264/GUI/persistence
integration ещё отсутствует; визуальная эквивалентность Windows и hardware
не доказаны. [Каталог/критерии](lcd-effects.md),
[evidence](evidence/2026-10-13-weather-foundation.json).


## Rain/Snow: theme, H.264 worker и GUI, 2026-10-13

Weather входит в theme v3 как первый слой между background и overlays;
v1/v2 сохраняют совместимость. Static/Dynamic переключение сохраняет выбранные
sensor identity/unit/low/high. Preview не продвигает live scene. GUI выполняет
Apply явно, сохраняет draft при ошибке и загружает активную тему.

Общий 24fps composition worker поддерживает procedural Weather без video
reader/decoder/media pin и Weather поверх video. Runtime использует миллисекунды
и WeatherScene с интеграцией предыдущей скорости. Live sensor переходы проверены
по decoded H.264 кадрам. Исправлено сохранение procedural темы в actor:
без media она теперь проходит через PinnedTheme, а не сбрасывает saved theme.
Private IPC/MockBackend проверяет Apply → restart → sleep/wake → Clear.

Это software integration checkpoint. Native slot/replay/fault matrix, Windows
visual comparison и независимые threshold dictionaries для каждого источника
остаются открытыми. Аппаратная проверка отложена.
Результаты — в [evidence](evidence/2026-10-13-weather-integration.json).

Проверено: workspace877 passed/2 ignored, затронутые crates525 passed/1 ignored,
fmt, strict Clippy и rustdoc; Svelte0 errors/0 warnings,39 frontend themes через
Rust validator,245 Playwright tests и production frontend build. Native weather
GUI в этом срезе не запускался.


## Rain/Snow: per-source ranges и native slots, 2026-10-13

Тема хранит текущий binding и отдельные inactive ranges с ключом(ID, unit).
Backend валидирует все ranges, duplicate identity и предел64 источника.
Старые v3 JSON без нового поля читаются. Static/Dynamic и source switching
сохраняют edits; смена unit создаёт новый диапазон, прежний остаётся сохранён.
Defaults30/60 и800/1200 сопоставлены с Windows helper.

Read-only review нашёл скрытое сохранение invalid range при переключении.
Исправлено: смена требует корректного текущего диапазона, rejected event
возвращает dropdown к текущей identity. Регрессия blank/reversed → switch → fix
прошла. Неиспользуемые ranges не влияют на runtime выбранного датчика.

Native WebKitGTK → Unix IPC → MockBackend: preview480×480, explicit Apply,
managed image background/text overlay, Save/load screen slot, отсутствие Apply
при загрузке черновика, непрерывный video transport после выхода GUI и restart
replay темы/слотов. Проверка `/tmp/lconnect-native-weather-t_j02zp6` принята.
Она не доказывает Windows visual equivalence или физическую LCD приёмку.
Native video-background/fault matrix, Windows bounds/semantic default и
раздельные настройки страниц Rain/Snow остаются открытыми.
[evidence](evidence/2026-10-13-weather-ranges-native.json).

Проверено:879 workspace passed/2 ignored;7 targeted weather Playwright tests;
Svelte0/0;39 frontend themes в Rust validator; production frontend build;
fmt, strict Clippy (включая native-smoke/custom-protocol GUI) и rustdoc.
Native harness повторно принят после проверки inherited disabled fieldsets.


## Rain/Snow: независимые параметры страниц и GUI bounds, 2026-10-13

WeatherEffect.presets хранит одну неактивную WeatherPreset без вложенных
presets. Каждая имеет intensity, mode, selected sensor/ranges и seed. Backend
проверяет обе страницы через общий borrowed validator; runtime применяет
только активную. Старые JSON без presets читаются. GUI переключает параметры
без скрытого кеширования invalid active range; explicit Apply и profile Save
проверяют active input bounds0…100/3600 перед IPC.

Source reference: Rain/Snow VisualEffectSetting + Import/Export отдельных
ScreenSetting; applyDynamicSetting задаёт Min/Max из WMSQUIHelper.
Windows допускает low=high; текущая Linux политика low<high остаётся открытым
сопоставлением. Backend legacy v3 i32 ranges сохраняют совместимость.
Background/overlays между страницами и переход через «Без эффекта» ещё требуют
полного page-state хранения. Semantic default CPU Load ещё не сопоставлен.
[evidence](evidence/2026-10-13-weather-pages.json).

Проверено:880 workspace passed/2 ignored,10 weather browser tests и4 background/
video regressions;39 frontend themes через Rust schema; Svelte0/0; production
frontend build; fmt, strict Clippy включая native-smoke/custom-protocol и rustdoc.
Native `/tmp/lconnect-native-weather-ussn4nnc` хранит Snow active + Rain inactive
в текущей теме и слоте; restart сохраняет оба набора и продолжает video packets.


## Rain/Snow: начальный semantic CPU Load, 2026-10-13

Новый Dynamic черновик выбирает сохранённую global CPU Load identity, иначе
один daemon-classified CPU Load candidate. Не использует label, порядок каталога
или GPU load. Global defaults не изменяются; существующие weather bindings
сохраняются. Missing saved ID не подменяется available candidate.

Unresolved/ambiguous/read failure оставляют Dynamic chooser; Apply/profile
Save не отправляют incomplete theme. Request counter, owner/effect identity
и onDestroy защищают от позднего ответа, ручного выбора и выключения режима.
Read-only Svelte review не выявил блокирующих замечаний.

Mock fixture публикует synthetic `proc:cpu:utilization` только с marker-файлом
`weather-cpu-load`; другие native fixtures сохраняют прежний каталог.
[evidence](evidence/2026-10-13-weather-cpu-load.json).

Проверено:33 targeted browser scenarios, включая18 weather;8 daemon source/default
checks; Svelte0/0;39 frontend contracts; production frontend build; fmt и strict
Clippy daemon/native-smoke GUI. Native `/tmp/lconnect-native-weather-hzsfxgws`
подтвердил CPU Load initial selection, profile/theme replay и transport continuity.
Workspace880/2 ignored относится к предшествующему checkpoint; заново не повторялся.


## Rain/Snow: равные пороги, 2026-10-13

Windows NumberTextBox поддерживает low=high. Actual installed mapping functions
из `/winver` выполнены offline в `/tmp`, без копирования vendor code в repo:
threshold0/40/100, below/equal/above. Rain1/2/6; Snow10/NaN/50. NaN при equality
входит в speed/velocity arithmetic. Linux допускает равные ranges, совпадает
с finite branches и использует минимальный Snow speed10 при exact equality.
GUI явно объясняет это правило; static settings и per-source/page caches
принимают equality, inverted ranges отклоняются.

Renderer regression проверяет движение и recovery после пересечения equality;
browser проверяет Apply/readback/page switching. Общий прогон обнаружил race
в существующем durability test: file visibility опережает parent fsync/status.
Тест теперь ждёт durable watch acknowledgement и проверяет сохранённый PWM67
без новой команды; read-only review подтвердил контракт.
[evidence](evidence/2026-10-13-weather-equal-thresholds.json).

Проверено:882 workspace passed/2 ignored;20 weather browser tests; Svelte0/0;
39 contracts и production build; fmt, strict Clippy (native-smoke GUI также)
и rustdoc. Native `/tmp/lconnect-native-weather-9pvdaurm` подтверждает Rain40/40
и Snow45/45 readback; Snow использует constant45 synthetic CPU Load. Theme/profile
restart и video continuity приняты. Hardware не выполнялся.

## Archive storage и IPC/CLI до подключения GUI

**Архив одного named slot через Unix IPC/CLI подключён:** file-backed64KiB
transfers, bounded admission/quota, idle expiry, exact Begin/Chunk retry и
Cancel/Release реализованы. Durable receipt сохраняется атомарно со slot,
переживает lost Finish reply/restart и не восстанавливает позднее удалённый slot.
Status возвращает текущую library revision и исходный committed_revision;
ambiguous rename resync проверен. CLI file export/import, --json/readback и
Ctrl+C pending Chunk проверены отдельными процессами. Workspace946 passed/
2ignored, strict clippy, rustdoc/fmt прошли. Unix RPC перенёс CJK font/image/cached
video в чистый store; HID writes0. GUI/native, длительный packing Cancel,
полный screen/fan+lighting group archive и Windows interoperability остаются.
[Контракт](profile-archives.md), [evidence](evidence/2026-10-14-profile-archive-transfer.json).

**Storage-основа переносимых профилей подключена:** собственный контейнер
пакует NamedProfile со всеми image/font/video dependencies active/cached pages,
checksums и сохранёнными именами. Import полностью валидирует staging, выдаёт
новые asset keys и меняет только typed references; sensor/text identity сохраняется.
Profile revision CAS защищает соседние/конкурентные edits; export_file не
перезаписывает destination. Indexed video600frames/>1MiB проверен с buffers≤64KiB.
Workspace935 passed/2ignored, восемь archive regressions, strict clippy,
rustdoc и fmt прошли. IPC/CLI, quota/expiry/cancel и durable retries закрыты следующим checkpoint выше;
GUI/native flows, полный group archive и Windows interoperability остаются.
[Контракт](profile-archives.md), [evidence](evidence/2026-10-14-profile-archive-storage.json).


## Group storage до подключения файлового codec

**Storage полных групп профиля подключён:** version1 Screen/FanAndLighting
snapshots включают active scene/collections/slots либо cooling/topology/все RGB
mode/source profiles. Target revision+digest защищают от stale edits; двухфайловый
journal восстанавливает settings и slots вместе, сохраняя соседнюю группу/globals.
Daemon434 passed/1ignored, strict Clippy/rustdoc/fmt прошли;6 group storage tests
и fault test обеих групп на трёх journal boundaries приняты.
Это storage foundation: group payloads/legacy uploads/remap, owner messages,
receipt/runtime Apply, IPC/CLI/GUI и Windows ZIP mapping ещё не подключены.
[Контракт](profile-groups.md), [evidence](evidence/2026-10-14-profile-groups-storage.json).


## Group codec до подключения owner commit/receipt

**Полные группы: storage/file codec и source owner snapshot подключены:**
LCGROUP1 переносит active/cached/slot/gallery-only и все managed user image/video
uploads; legacy catalogs материализуются общими migration rules. Typed remap
сохраняет sensor/text identity, default gallery keys и лимит256 members.
PreparedGroup держит pins; подготовка не меняет settings/slots. Owner snapshot
включает queued deltas без flush. LCPROF01 использует тот же payload pipeline.
Workspace970 passed/2ignored,10 group tests, strict Clippy/rustdoc/fmt прошли.
Clean-store files/restart, no-overwrite, malformed payloads при верной checksum,
последний AU,600frames/>1MiB/64KiB I/O и pins проверены. Это не LCD playback test.
Далее commit owner/receipt/runtime integration, group IPC/CLI/GUI, Windows ZIP
mapping и expanded native/hardware qualification.
[Контракт](profile-groups.md), [evidence](evidence/2026-10-14-profile-groups-codec.json).
Storage checkpoint сохранён в [истории](implementation-checkpoints-2026-10-14.md).


## Group owner receipt до подключения screen runtime

**Полные группы: storage/file codec, owner commit и durable receipt подключены:**
LCGROUP1 переносит active/cached/slot/gallery-only и legacy/user uploads.
CommitProfileGroup владеет Arc<PreparedGroup>/pins через blocking commit/recovery.
Receipt сохраняется вместе с settings/slots, связывает ID/source/target и переживает
lost reply/restart; current readback не возвращает удалённые slots и не затирает
поздние queued edits. Другая группа/globals сохраняются. Неизвестный journal
останавливает writer без flush старого снимка; повреждённые receipt revisions защищены.
Workspace977 passed/2ignored, strict Clippy/rustdoc/fmt прошли.6 owner/storage tests,
оба group journal paths на трёх границах и contended abandoned worker/pins приняты.
Runtime freeze/barriers/Apply/recovery, group IPC/CLI/GUI и Windows ZIP mapping
ещё открыты; live LCD/cooling/RGB этими storage сообщениями не переключаются.
[Контракт](profile-groups.md), [evidence](evidence/2026-10-14-profile-groups-commit.json).
Предыдущие storage/codec checkpoints сохранены в [истории](implementation-checkpoints-2026-10-14.md).


## Screen group runtime до software H264 rotation

**Screen group runtime commit/Apply/reapply подключён к LCD owner:**
PreparedScreenGroup готовит JPEG/media/composition до mutation; durable group
receipt предшествует joined replace, config и первому frame. Старые tickets/
renders инвалидируются; repeat не применяет старую сцену поверх нового Clear.
Live sensor cache продолжает менять sensor-only JPEG после import. Config/frame
failure сохраняет receipt и допускает отдельный generation/ticket-bound reapply,
включая fresh actor после restart. Unknown outcome блокирует mutations; queued
composition cleanup и shutdown во время config ack проверены.
Workspace988 passed/2ignored,11 screen actor tests, strict Clippy/rustdoc/fmt прошли.
Fake HID/private files и actual FFmpeg preparation; это не hardware acceptance.
Остаются fan+lighting runtime/barriers, H.264 nonzero rotation (Windows поворачивает
bitmap перед encoder), bounded admission/group IPC/CLI/GUI и expanded lifecycle.
[Контракт](profile-groups.md), [evidence](evidence/2026-10-14-profile-groups-screen-runtime.json).
Storage/codec/receipt checkpoints сохранены в [истории](implementation-checkpoints-2026-10-14.md).

