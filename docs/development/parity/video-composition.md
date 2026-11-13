# Видео и live LCD layers

Baseline: Galahad II LCDV2, Windows L-Connect3 2.1.28.0.
Сверено2026-10-15: **renderer, streaming worker, backend Apply/replay,
preview и GUI выбора video background реализованы**. Видео выбирается в
библиотеке как фон темы; preview и Apply остаются отдельными действиями.
Standalone media сохраняется как отдельный сценарий. Открытые native/platform/
performance gates перечислены в разделе приёмки ниже.

## Реализованный контракт

`llcontrol_image::theme::render_theme_over_frame` принимает owned RGB480×480.
Pixels видео заменяют solid/gradient background темы; ordered layers рисуются
тем же кодом, что и в обычном renderer. Text, sensor, clock, image/watermark,
alpha, shadow, managed fonts и validation не дублируются отдельной реализацией.
Неверный размер кадра отклоняется; это проверка готового canvas, не защита
от allocations декодера до создания RgbImage.

`ThemeRuntime::compose_video_frame` выполняется на blocking worker, снимает
свежие snapshots только с SensorCache и локальное время. Он не обращается к
датчикам, файлам или HID. Elapsed seconds задаёт сессия композиции: начало
очередного video loop не должно сбрасывать sensor-cycle или часы. Существующий
JPEG путь использует тот же общий render и остаётся на своём текущем cadence.

Renderer tests сравнивают обычную тему и те же слои поверх видеокадра,
сохраняют незакрытые pixels и проверяют alpha watermark/clock/размер.
Codec fixture декодирует четыре движущихся H.264 кадра, обновляет synthetic CPU,
проверяет missing/recovery через буквальный text raster, затем перекодирует
композицию в H.264 и декодирует все четыре кадра обратно. Это file/codec test,
не потоковый worker, frame pacing или физическое наблюдение LCD.

## Streaming worker foundation

`MediaComposition` владеет pipeline media reader → FFmpeg decoder → cache-only
renderer → FFmpeg encoder → очередь из двух AU. Blocking render обязательно
дожидается завершения; stop отменяет pipe tasks, завершает и reap-ит оба codec
процесса, дожидается reader и освобождения source pin. Отмена receive сохраняет
исходный десятисекундный deadline. Drop просит отмену, явный stop даёт barrier.

Первый run выявил ошибку allocation limit: FFmpeg проверял aligned512×480
для visible480×480 против лимита230400. Лимит исправлен на262144 (512×512),
фиксированный output остаётся480×480 RGB. `-xerror` завершает decoder при ошибке.
Четыре focused tests проходят: 60 decoded output frames с обновлением CPU40→80
на повторяемом четырёхкадровом видео; отклонение1024×1024 до timeout;
kill/reap зависших процессов с cancelled receive deadline; AUD byte boundaries
и AU budget. Это не 24fps performance или аппаратная приёмка.

Worker подключён к LCD actor, публичному Apply, manager replay, preview и GUI. Нужны дополнительные
malformed/truncated/read-failure сценарии, performance и lifecycle integration.
Исторический timeout сохранён в [аудите](evidence/2026-10-12-plan-audit-worker-wip.json);
исправление — [новый checkpoint](evidence/2026-10-12-video-composition-worker.json).

## Атомарная модель сохранения

Композиция хранится как `LcdTheme` версии2 с обязательным `background_video`
(managed32lowerhex media key). Layers, image/font dependencies и video key
сохраняются в одном theme object, включая обычный `ProfileContent::Screen`.
Версия1 не допускает video key; старый JSON без поля сериализуется без добавок.
Версия2 без video key и версия1 с ним отклоняются. Старый daemon отвергает
неизвестное поле/версию, а не превращает видео в solid background. Внешние
versions settings/profile library не меняются: новый вариант версионирован
внутри theme и неизвестные данные уже защищены от перезаписи.

Persistence scanner учитывает video key в offline theme; profile Save
проверяет media dependency и держит pin до durable edit. Delete учитывает
сохранённые theme references. Screen slot reset снимает эту ссылку. Image/font
references остаются в тех же theme layers и обрабатываются прежними scanners.
Прежняя взаимоисключаемость `settings.theme` и `settings.media_clip` сохраняется:
новая композиция целиком находится в `settings.theme`.

Apply версии2 использует PreparedComposition и H264 actor. Preview теперь
декодирует первый AU и передаёт явный480×480 frame общему renderer; прямой вызов
статического renderer без video frame по-прежнему запрещён. UI пока не создаёт
такие темы.
[Schema checkpoint](evidence/2026-10-12-video-composition-schema.json).

## LCD actor bridge

`PreparedComposition::prepare` загружает media по ключу из самой theme,
создаёт worker и получает первый AU до постановки мутации в очередь. Внутренняя
команда `SetComposition` использует preparation ticket, firmware/rotation guards,
те же H264 frame batches и24fps deadlines. Источник виден как Theme;
запрос Theme возвращает полную video-theme. Публичный IPC Apply теперь вызывает
эту команду после подготовки runtime/media/первого AU.

`ComposedTheme` передаёт один theme object и media pin settings writer.
Clear/replace/failure останавливают активный worker; stale ticket дожидается
его stop и освобождает prepared перед ответом. При shutdown actor закрывает
очередь и дожидается также всех queued composition workers, затем active source.

Реальные FFmpeg fixtures с mock HID проверяют первый frame, Theme readback,
Clear за удерживаемым frame batch, LocalUI после stop, pending-settings pin,
отказ late preparation и shutdown с queued Apply за незавершённым HID batch.
[Actor checkpoint](evidence/2026-10-12-video-composition-actor.json).
Публичный Apply удерживает concurrency permit во время загрузки assets/media,
подготовки первого AU и принятой actor мутации. Worker и supervisor разделяют
lease; Drop RPC waiter не освобождает slot до завершения cleanup. Успешный
первый frame освобождает preparation slot, не останавливая playback.
Manager replay использует тот же PreparedComposition для startup и wake.

[IPC/replay checkpoint](evidence/2026-10-12-video-composition-ipc.json): private
IPC + real FFmpeg + MockBackend проверяют repeated Apply (без утечки slots),
missing dependency без замены текущей темы, restart, sleep/wake с новым device
identity, missing-media restore error без потери сохранённой темы, recovery,
Clear и итоговое удаление. Drop regression проверяет reap обоих stalled codecs
до освобождения permit. Полный IPC disconnect/cancellation fault matrix,
На момент этого IPC checkpoint GUI ещё не был подключён; текущий GUI
принят ниже. Preview добавлен следующим checkpoint.

## Предпросмотр video theme

`preview_lcd_theme` версии2 читает только первый AU под media pin, запускает
общий decoder с `-frames:v 1`, ограничивает stdout691201bytes и ожидает ровно
480×480 RGB. Timeout/invalid size/decode failure вызывают kill/reap до ответа.
После записи AU stdin descriptor закрывается явно: один `shutdown()` не
доставляет EOF H264 parser. Sensor/clock/text layers рисуются общим renderer
с текущим cache и `elapsed_seconds` для позиции sensor cycle, затем кодируются
в JPEG. Это preview первого видеокадра, не seek по timeline или motion preview.

Весь job выполняется внутри owned IPC preparation task: потерянный waiter
не освобождает concurrency slot до завершения decode/render/cleanup. Ни HID,
ни saved settings preview не изменяет. Missing media даёт явную ошибку.

[Preview checkpoint](evidence/2026-10-12-video-composition-preview.json): live
cache/cycle position, декодированные blue background/white text, offline/connected
preview без LCD writes/state change, delete после preview, missing dependency,
а также stalled/oversize stdout decoder kill/reap проверены. Hardware/GUI не запускались.

## Реализованная архитектура и требования дальнейшей проверки

Пункты 1–3 реализованы; они задают инварианты для оставшейся fault/performance
приёмки. GUI-сценарий ниже подключён; расширенная native accessibility остаётся.

1. Decoder/encoder worker владеет media pin, процессами, bounded pipes/queues
   и runtime assets. Decode allocation должен быть ограничен до создания frame:
   только480×480 RGB, фиксированный размер raw frame, bounded metadata/AU.
2. Один LCD actor получает подготовленные H.264 AU и сохраняет frame-batch
   barriers. Renderer/codec/IO не выполняются внутри HID executor. Backpressure
   ограничивает очередь; медленный codec не приводит к накоплению всех кадров.
3. Stop/replace/shutdown/sleep ждут реального завершения worker/child reap.
   Cancellation receive не отменяет начатый HID batch. Текущий media reader
   не должен продолжать работу параллельно новому источнику после barrier.
4. Расширить IPC cancellation/failure приёмку version2 Apply/replay. Основной
   путь, first AU до queue, pending settings pin и stale-ticket stop реализованы;
   отдельные disconnect/reply-loss и slow prepare сценарии ещё проверить.
5. GUI редактор выбирает background video и использует готовый preview с явным Apply; закрытие GUI
   не останавливает daemon composition. Missing media/image/font даёт явную
   ошибку, а не ложный applied или молчаливую подмену фона.

## Приёмка

- [x] Ordered layers поверх RGB video canvas; незакрытые pixels сохраняются.
- [x] Cache updates/missing/recovery влияют на successive decoded frames;
  compose→encode→decode сохраняет число кадров.
- [x] Scripted codec faults: truncated RGB stdout, malformed/oversize encoder
  output и encoder EOF доставляют ошибку; оба child PID исчезают до Stop return,
  preparation lease и source pin освобождаются. Real FFmpeg queue saturation
  → Stop/drain/join/delete и повторный Stop проверены без consumer receive.
  [Evidence](evidence/2026-10-12-composition-codec-faults.json).
- [ ] Decoder/encoder worker: bounded memory/queues, malformed/truncated source,
  AU budget, timeout/kill/reap, cancellation на каждом blocked IO и prefetch.
- [x] Initial release paced workload:720frames/24fps, sensor+clock; no frame more
  than one period late, startup30ms, CPU3.6% of one core, sampled RSS sum130.5MiB.
  [Evidence and scope](evidence/2026-10-12-composition-paced-release.json).
- [ ] 24fps pacing и sensor/clock updates во время нескольких loops; UI закрыт,
  CPU/RSS/queue/latency измерены. Это отдельный performance gate.
- [x] Actor composition first frame/Theme query, Clear за HID batch, pending
  settings pin, stale ticket и queued Apply shutdown проверены с real codec/mock HID.
- [ ] Полная матрица replace/clear во время decode, encode, receive и HID batch: после barrier
  нет старых кадров, decoder/processes/pins освобождены.
- [x] Versioned composition identity, сохранение/profile round-trip, missing media
  dependency и delete protection (включая save/delete race).
- [x] Публичный Apply и manager replay: restart, missing dependency/recovery,
  sleep/wake с новым attachment, Clear и release references (real codec/mock HID).
- [x] Early request EOF отменяет deferred preparation; потеря ответа Apply
  через transport write failure не отменяет подтверждённую вторым клиентом
  новую тему. Поток продолжает кадры; последующие Apply/restart/sleep/Clear
  и delete проходят. [Evidence](evidence/2026-10-12-composition-ipc-faults.json).
- [ ] Расширенная IPC disconnect/cancellation/failure матрица composition.
  В частности, disconnect во время уже запущенного codec prepare ещё не покрыт.
- [x] Backend first-frame video preview: live readings/cycle elapsed, bounded decode
  и cleanup; preview не меняет LCD.
- [x] GUI выбор video background без Apply, preview RPC, screen slot round-trip
  и возврат к static background с сохранением layers/gradient проверены browser tests.
  Missing video не подменяется другим фоном; Apply показывает ошибку.
- [x] Native WebKit/Tauri/private IPC/MockBackend: video draft, cyan preview
  pixels, Apply с video+clock, Clear/persistence и удаление освобождённого ролика.
  [GUI checkpoint](evidence/2026-10-12-video-background-gui.json).
- [ ] Полная native keyboard/speech/scaling приёмка GUI composition.
- [ ] Физическая приёмка всех слоёв с cooling/RGB на доступном LCDV2 позднее;
  после теста LocalUI. Codec/mock результаты этот пункт не закрывают.

[Результаты renderer/codec проверки](evidence/2026-10-12-video-overlay-renderer.json):
699Rust tests/101targets, fmt/strictClippy/docs/production GUI check прошли.
Новых native GUI или hardware наблюдений этот checkpoint не добавляет.


## Воспроизводимое измерение paced composition

Запускать отдельно, без других тестов, в release mode:

```sh
cargo test -p llcontrol-daemon --release --lib composition_paced_acceptance --locked -- --ignored --nocapture
```

30-second workload: 720 H.264 frames, consumer deadlines24fps, повтор
четырёх кадров синего video, sensor40→80°C и clock layer. JSON строка
`COMPOSITION_REPORT` содержит startup, receive wait p95/max, опоздания больше
одного frame period, queue occupancy, CPU ticks и RSS samples.

Это CPU/RSS тестового процесса и обоих codec, включая накопленный encoded
output. RSS — сумма sampled resident pages, shared pages могут учитываться
дважды; это не unique memory peak и не доказательство отсутствия leaks.
Post-run decode и teardown resource costs исключены. Decoder проверяет первый
и720-й кадры и изменение sensor region; визуальный ход часов отдельно не
проверяется. Test asserts correctness/cleanup; timing thresholds автоматически
не ограничиваются. Actor/HID, сложное video, максимальное число layers и
длительная стабильность требуют отдельных измерений.
