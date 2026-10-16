> Исторический аудит до M1. Актуальные пробелы — [здесь](../lcdv2-baseline-gaps.md).

# LCDV2: пробелы Rust baseline перед M1

Дата анализа: 2026-09-08. Область: Galahad II LCDV2 `0416:7395`, daemon,
IPC и CLI текущего workspace. Это статический анализ и существующие mock/unit
tests; новых аппаратных прогонов в M0 не было.

## Связь с матрицей и этапами

Стабильные feature IDs определены в [matrix](../lcdv2-2.1.28-matrix.md).

| Gap ID | Пробел | Matrix IDs | Блокирует |
|---|---|---|---|
| GAP-001 | Socket defaults расходятся | LCD-001, FW-001, SYS-001 (общий IPC путь) | M1 Task1 |
| GAP-002 | Patch/get applied LCD state отсутствуют | LCD-012, LCD-013 | M1 Task1/3 |
| GAP-003 | Нет frame transaction | LCD-001, LCD-002, FW-003 | M1 Task2 |
| GAP-004 | Нет clear/replace barrier | LCD-001, LCD-002, LCD-015 | M1 Task3/4 |
| GAP-005 | Persistence key зависит от attach order | SYS-001, SYS-004 | M1 Task6 |
| GAP-006 | H264 readback, recovery не подтверждены live | LCD-002, FW-003 | M1 Task5 |
| GAP-007 | Нет bounded source lifecycle/pacing и проверки GOP | LCD-002, SYS-004 | M1 Task4–7 |
| GAP-008 | Freshness есть в payload, автономной обработки отказов нет | LCD-004, COOL-001, COOL-002 | M2; M1 измеряет sensor age |

M1 task numbers — [план dynamic display](../../superpowers/plans/README.md).

## Что уже доказано

- Статический JPEG имеет интегрированный путь CLI → IPC → `lcd_keeper` →
  `hid_io`. IPC заранее проверяет только допустимый размер кадра и возвращает
  успех после помещения `Set` в очередь (`ipc_server.rs:642-657`). Keeper
  строит B-пакеты cmd 14, пишет их по одному и ждёт результат каждой записи
  (`actor/lcd_keeper.rs:101-132`). Тест подтверждает немедленную отправку и
  периодическую повторную отправку лишь для однопакетного синтетического
  payload (`tests/lcd_keeper.rs:32-95`). Исторический live smoke остаётся
  единственным доказательством видимого JPEG на устройстве.
- H.264 пока является codec layer. B- и C-builder проверены для пустого,
  короткого, граничного, многопакетного и максимального payload, включая PN,
  DataSize и Length (`protocol/tests/galahad2_lcdv2_send_h264_frame.rs:8-124,
  199-296`). Это не проверяет encoder, pacing, очередь daemon или playback.
- `hid_io` последовательно исполняет отдельные `HidCommand` из bounded-очереди
  ёмкостью 64 (`actor/hid_io.rs:38-65`). Rust round-trip сопоставляет report id
  и command echo, пропуская до четырёх stale/timeout ответов по 500 ms
  (`actor/hid_io.rs:137-191`); C-report regression покрыт mock-тестом
  (`tests/hid_io.rs:64-101`). Windows `SendH264Frame`/`SendH264Frame512` после
  записи делает одно чтение до 512 bytes с timeout 20 ms и игнорирует результат
  (`lcdv2-screen-pipeline-2.1.28.md`, раздел «HID framing»). Поэтому это
  обязательный drain-кандидат, но не доказанный echo-ACK или applied status.
- Snapshot — latest-value `watch`: медленный потребитель намеренно пропускает
  промежуточные значения (`actor/sensor_poll.rs:1-16`). В структуре есть
  `temp_valid` и `last_seen_unix_ms` (`lconnect-ipc/src/types.rs:34-42`), но
  после poll/decode failure последнее успешное значение продолжает
  возвращаться (`actor/sensor_poll.rs:53-65`, `ipc_server.rs:382-402`).

## Подтверждённые интеграционные дефекты

### Разные socket defaults

Daemon при заданном `XDG_RUNTIME_DIR` слушает
`$XDG_RUNTIME_DIR/lconnectd.sock` (`daemon/src/main.rs:87-104`), а CLI без
override использует XDG prefix и получает
`$XDG_RUNTIME_DIR/lconnectd/lconnectd.sock` (`cli/src/connection.rs:8-24`).
Обычный запуск daemon и CLI с одной средой поэтому выбирает разные файлы.

Регрессия: создать временный `XDG_RUNTIME_DIR`, запустить server с daemon
resolver и CLI без `LCONNECTCTL_SOCKET`; `list_devices` должен установить
соединение. Отдельно проверить fallback при отсутствующем env. Один общий
resolver должен принадлежать IPC crate или общей config-функции.

### Частичная команда CLI перезаписывает соседние поля LCD

IPC принимает только полный `LcdConfig`; getter отсутствует
(`lconnect-ipc/src/trait_def.rs:43-47`, `types.rs:118-124`). Поэтому команды
brightness, rotation и mode подставляют `Application/100/0` вместо двух
неуказанных полей (`cli/src/commands/lcd.rs:98-150`). Это не гипотеза: такое
поведение явно реализовано и даже сообщается warning.

Регрессии: после applied `{LocalH264, 70, 90}` команда brightness=40 должна
дать `{LocalH264, 40, 90}`; rotation=180 должна сохранить mode и brightness;
mode=LocalUI должна сохранить brightness и rotation. Authoritative state для
M1 должен жить в daemon и различать requested/applied; клиент не должен делать
read-modify-write по потенциально устаревшему snapshot. Нужен getter плюс
patch-операция или отдельные setter-методы с атомарным merge в daemon.

### Многопакетный кадр не является одной HID-транзакцией

`LcdFrameMsg::Set` несёт целые JPEG bytes, но keeper раскладывает кадр и для
каждого пакета отдельно отправляет `HidCommand::Write`, ожидая oneshot перед
следующим (`lcd_keeper.rs:93-132`). В ту же multi-producer очередь пишут
sensor polling, PWM, lighting и IPC (`actor/mod.rs:68-74, 136-180`). Планировщик
может вставить другую команду между пакетами одного JPEG. Сериализация
отдельных HID calls в `hid_io` (`hid_io.rs:48-59`) не обеспечивает
непрерывность frame packet sequence.

Это доказанный дефект границы транзакции; повреждение конкретной firmware ещё
не воспроизведено. Регрессия: одновременно отправить многопакетный кадр и
запустить sensor/PWM writes; в mock history все PN кадра должны идти подряд,
после чего выполняется следующая команда. Для M1 `hid_io` должен принимать
одну frame transaction (весь packet batch и, для cmd 13, исследуемый read/drain),
не блокируя выбор/подготовку следующего кадра.

### `clear_lcd` не имеет барьера с keeper

`clear_lcd` лишь помещает `Clear` в frame-очередь, затем независимо помещает
LocalUI write в HID-очередь (`ipc_server.rs:660-698`). `Clear` не имеет reply,
а keeper может уже выполнять `push_frame`; его пакет может попасть после
LocalUI. Комментарий о предотвращении race (`ipc_server.rs:664-666`) сильнее
гарантии реализации. Аналогично success от `set_lcd_image` означает accepted,
а не applied.

Регрессии: во время длинного frame batch вызвать clear; после подтверждённого
LocalUI в history не должно быть пакетов старого кадра. Затем проверить
replace A→B: после принятия B ни один refresh A не должен начаться. Нужны
generation/cancellation и reply с результатом `applied` либо явный статус
`accepted`; остановка должна ждать завершения/отмены frame transaction.

### Persistence привязана к attach-order ID

Ключ TOML равен runtime `DeviceId` (`persistence.rs:124-168`), а manager при
каждом attach увеличивает process-local counter (`manager.rs:144-178,
308-313`). После detach запись удаляется из maps (`manager.rs:221-247`), и
тот же физический прибор при reattach получает следующий ID. Replay читает
настройки уже по новому ID (`manager.rs:260-294`), поэтому сохранённые для
`:0001` значения не являются настройками того же устройства под `:0002`.
Существующий replay test проверяет только первый attach `:0001`
(`tests/manager.rs:166-242`).

Регрессия: настроить устройство с serial, дождаться persistence, detach и
reattach в том же процессе; ID/persistence identity должны остаться связаны
с физическим устройством, а LCD/PWM settings восстановиться. Для устройства
без serial требуется явно выбранная устойчивая policy; HID path сам по себе
может измениться.

## Подтверждённые пробелы M1, но ещё не воспроизведённые bugs

- Streaming runtime отсутствует: IPC имеет только JPEG `set_lcd_image` и
  `clear_lcd`, `LcdFrameMsg` — только `Set/Clear` (`trait_def.rs:43-47`,
  `actor/mod.rs:76-81`). Нет H.264 submit, framing selection по firmware,
  start/replace/stop lifecycle, pacing, delivered FPS или recovery.
- Frame queue bounded (8), а `send().await` создаёт backpressure
  (`actor/mod.rs:144-153`). Для 24 fps это может накопить устаревшие кадры и
  задержать stop/replace. Однако произвольный latest-wins/drop уже encoded
  H.264 packets недопустим: P-frames зависят от предшествующих кадров внутри
  GOP. M1 должен либо применять backpressure без потерь внутри GOP, либо при
  сбросе backlog явно переходить к следующему keyframe и сбрасывать decoder/
  transport state. Queue overflow сейчас не воспроизводится, поскольку runtime
  producer отсутствует.
- Reply/drain contract cmd 13 не интегрирован. Codec parser требует B echo, а
  daemon не вызывает H.264 builder/parser. Windows после B/C write делает одно
  чтение до 512 bytes с timeout 20 ms и игнорирует buffer; это не доказывает
  echo, ACK или success status. Live spike должен установить: обязательно ли
  чтение, какой report/cmd фактически возвращается, что означает timeout и
  остаются ли stale bytes. Только после этого можно закрепить class-specific
  matching; успешное чтение само по себе не доказывает playback.
- Partial-write recovery явно undefined в codec/keeper. Текущий JPEG keeper
  прекращает batch и на следующем tick повторяет полный кадр
  (`lcd_keeper.rs:96-100, 109-130`), без reset/resync и без ошибки клиенту.
  Без live evidence нельзя утверждать, что повтор безопасно восстанавливает
  firmware transfer state.
- Sensor freshness кодируется timestamp, но daemon не вычисляет stale status
  и не публикует poll/decode error; consumer обязан сам интерпретировать age.
  M1 animation/soak должен измерять cadence при frame traffic и доказать, что
  transaction batching не приводит к неприемлемому голоданию telemetry/PWM.
- Restart/reconnect прекращает actor tree через shutdown watch, но JPEG bytes
  намеренно не persist/replay (`manager.rs:316-327`). Желаемое восстановление
  активного stream source и различие requested/applied/error пока не заданы.
- Настройки применяются на HID до persistence commit (`ipc_server.rs:617-639`),
  а commit при закрытом settings channel только логирует рассинхронизацию
  cache/disk (`ipc_server.rs:200-219`). Replay пишет best-effort и игнорирует
  ошибку (`manager.rs:340-342, 403-424`). Это требует observable applied/error
  state; само по себе не доказывает пользовательскую регрессию.

## Минимальные архитектурные рекомендации для M1

1. Сначала исправить общий socket resolver и daemon-owned частичное обновление
   LCD config; добавить end-to-end regressions без железа.
2. Ввести в HID actor команду уровня frame batch: class, packets и настраиваемую
   post-write read/drain policy. Конкретный echo/timeout contract закрепить
   только после live spike. Между пакетами кадра не допускаются sibling writes;
   между кадрами scheduler обязан давать bounded доступ sensor/PWM.
3. Создать один per-device display controller с поколением потока и состояниями
   `stopped/starting/running/stopping/error`. Replace/stop инвалидируют queued
   старые поколения; stop завершается только после барьера HID и проверенного
   LocalUI write/reply outcome. API отдельно сообщает accepted и applied.
4. Начать с bounded source и собственной короткой анимации. До encoding можно
   coalesce raw frames; после encoding сохранять порядок и не выбрасывать
   P-frames произвольно. При отставании использовать backpressure либо явный
   jump-to-keyframe/reset. Зафиксировать cadence, drops по типу кадра, queue
   depth, transaction latency, sensor age и write/decode errors.
5. Выбирать C-framing только по firmware contract из baseline и live smoke;
   B оставить исследовательской веткой до подтверждения. В spike повторить
   Windows-последовательность «batch → одно read(512, 20 ms)», сохранить raw
   ответы/таймауты и лишь затем определить, нужен ли class+cmd match или drain.
6. До hardware acceptance исправить стабильную device persistence identity и
   определить restart/reconnect policy для активного source. Ошибка replay не
   должна превращаться в молчаливое «applied» состояние.

## Обязательные сценарии проверки M1

| Сценарий | Проверяемая гарантия |
|---|---|
| daemon + CLI без overrides | Оба вычисляют один socket path |
| brightness/rotation/mode по одному | Соседние applied-поля сохраняются |
| большой кадр + polling/PWM | Пакеты кадра contiguous; другие команды выполняются между кадрами |
| A→B при заполненной очереди | После барьера замены A больше не отправляется; B не ждёт stale backlog |
| clear во время batch | После LocalUI нет пакетов старого поколения |
| reply/drain: B/C, timeout, неожиданный buffer | Live spike определяет policy; read queue остаётся пригодной |
| write failure на PN=N | Клиент видит error; recovery выполняет определённый полный restart/resync |
| 60 s и 30 min animation | Видимое движение, delivered FPS, drops, CPU/RAM и sensor age записаны |
| detach/reattach того же устройства | Stable identity, восстановление settings, старый stream не воскресает |
| daemon restart, sleep/wake | Явный requested/applied/error state и определённое восстановление |
