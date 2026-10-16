# LCDV2: протокол аппаратной проверки

Подготовлен 2026-09-08 для M1 и следующих этапов. M0 выполнял анализ, CLI help и host encoding. Первые результаты M1
записаны в [capture](../protocol/captures/2026-09-08-lcdv2-m1.md);
статусы ниже обновляются только по фактическим прогонам.
Исторический JPEG smoke от 2026-04-27 не переписывается как новый результат.

## Условия прогона

Рабочая директория: `/home/inky/Development/lian-li/lconnect-linux`.
Только `0416:7395`. Перед тестом записать фактические device ID/FW, commit,
kernel, версии инструментов, время. Serial в публичных логах маскировать.

Нельзя одновременно запускать daemon и direct `verify-galahad` writer.
Запуск daemon может replay сохранённые настройки: это уже аппаратное
воздействие, а не read-only discovery. Для чтения через CLI использовать
уже работающий daemon или отдельно подготовленный старт с сохранением state.

До любых изменений сохранить копию `$XDG_CONFIG_HOME/lconnectd/` (если XDG
не задан — `~/.config/lconnectd/`), записать текущие LCD mode/brightness/rotation
и иметь исходный показываемый asset. В текущем коде getter отсутствует,
а JPEG bytes вообще не сохраняются: backup TOML не восстанавливает картинку.
Если исходного asset/config нет, явно выбрать восстановление в LocalUI как
ожидаемый результат теста, не обещать восстановить неизвестное содержимое.

Тесты display не меняют fan/pump PWM. PWM acceptance выполняется отдельно
с известными рабочими значениями и наблюдением RPM. Factory reset, firmware
и raw неизвестные opcodes не используются для cleanup.

## Read-only preflight через существующий daemon

До исправления socket defaults задать CLI override; после исправления
отдельно проверить штатный путь без override (A-001).

```bash
export LCONNECTCTL_SOCKET="$XDG_RUNTIME_DIR/lconnectd.sock"
cargo run -q -p lconnect-cli -- --json devices
cargo run -q -p lconnect-cli -- --json firmware
cargo run -q -p lconnect-cli -- --json snapshot
cargo run -q -p lconnect-cli -- lcd available
cargo run -q -p lconnect-cli -- daemon stats
```

Подкоманды проверены по Clap source/help. Firmware/read availability могут
содержать USB request writes для чтения ответа, но не меняют конфигурацию.
Не путать это с отсутствием USB I/O вообще.

CLI help текущего image ошибочно обещает «any format ffmpeg can decode»:
реальная обработка находится в `lconnect-image` и использует Rust image.
Help clear упоминает ~20 секунд, хотя код уже явно отправляет LocalUI.
Исправить тексты в M1, не использовать их как аппаратный контракт.

## Сценарии и ожидаемый результат

| ID | Действие | Проверяемый результат | Статус |
|---|---|---|---|
| A-001 | Daemon + CLI без socket override в одном runtime dir | Успешный devices RPC | pass: M1 2026-09-08 |
| A-002 | Прочитать FW/snapshot/availability | FW и живой timestamp; физический SKU соответствует scope | pass: FW1.6, живая telemetry, available=yes |
| A-003 | Поменять только brightness, затем только rotation | Другие applied LCD поля не сброшены | pass: 70/90 → 100/90, затем rotation0 |
| A-004 | JPEG A, наблюдать 70 секунд | Изображение видно, не возвращается само в LocalUI | pass: >70с, подтверждено пользователем |
| A-005 | JPEG A → JPEG B во время refresh | Показан B; A больше не появляется | pass: кольца появились быстро, стабильны |
| A-006 | Clear во время передачи большого кадра | После completion LocalUI; старые frame writes не возобновляются | partial: clear проверен; точное попадание в середину передачи — только mock |
| A-007 | Короткий C-framed H.264 diagnostic clip | Видимое движение; reply bytes/timeout и framing записаны | pass: движение подтверждено, raw replies записаны |
| A-008 | Animation 60 секунд + snapshots | Видимый прогресс; нет необъяснённого stale telemetry | pass: после исправления 23,35 sent fps, telemetry≤1,03с; пользователь подтвердил отсутствие лагов |
| A-009 | Замена/stop/restart clip на границе и посреди GOP | Нет смешивания поколений; decoder начинает с decodable keyframe | partial: machine evidence pass; визуальное подтверждение коротких переключений pending |
| A-010 | Animation 30 минут | Стабильная память, ограниченная очередь, без зависшего LCD | pass: 1800с, 23,34 sent fps, zero errors, RSS +188KiB; видимый результат подтверждён |
| A-011 | Реальное отключение/подключение USB | Та же physical identity; settings связаны с устройством; старый actor не пишет | not-run: пользователь отложил физическое переподключение |
| A-012 | Restart daemon | Заявленная policy settings/source выполнена | pass: Idle/LocalUI после restart, explicit play работает |
| A-013 | Suspend/resume ОС | Нет накопленной очереди; источник/ошибка соответствуют заявленной policy | not-run |
| A-014 | Cold boot | Autostart, permissions и replay соответствуют package contract | not-run |
| A-015 | Восстановить исходный config/asset | Видимое состояние совпадает с записанным до теста | partial: согласованный LocalUI отправлен; финальный физический вид отдельно не подтверждён |

M1 diagnostic clips можно не сохранять автоматически: после restart/reattach
ожидается Idle и явный повторный play, если именно эта policy зафиксирована
в M1. Это не полное playback lifecycle parity; автоматическое восстановление
постоянного user source остаётся M3/M4. A-012 проверяет заявленную policy,
не подменяет более сильный release gate.

У M1 нет sleep-event integration: A-013 остаётся not-run/unsupported;
до suspend оператор останавливает diagnostic clip. Нельзя утверждать,
что source сам станет Idle после сна. Logind signal integration и её
deterministic tests входят в будущий M3/M4 lifecycle plan.

### JPEG commands для A-004/005/006

После сохранения state и подготовки единственного владельца HID:

```bash
cargo run -q -p lconnect-cli -- lcd image crates/lconnect-image/test-images/01-radial-gradient.png --fit crop --quality 85
cargo run -q -p lconnect-cli -- lcd image crates/lconnect-image/test-images/02-concentric-rings.png --fit crop --quality 85
cargo run -q -p lconnect-cli -- lcd clear
```

Между первыми двумя командами наблюдать минимум 70 секунд; не исполнять блок
слепо подряд. Пользователь подтверждает физический вид LCD. Preview и USB
success доказывают другие уровни цепочки. Hardware reconnect/cold boot/sleep
требуют отдельного действия, а не имитации повторным запуском процесса.

### Animated diagnostic

Источник — собственный testsrc2/счётчик кадров, подготовленный по
[pipeline](../protocol/lcdv2-screen-pipeline-2.1.28.md). Source manifest
фиксирует 480×480, 24 fps и access-unit boundaries; NAL/pipe chunks не
используются вместо packet boundaries. Точные команды нового CLI добавляются
при реализации M1 и сверяются с `--help` до аппаратного запуска.

На spike записать original replies до их интерпретации: C# не проверяет
ACK status. Нулевой read может быть обычным timeout; обязательный echo-ACK
не вводить до доказательства. При новых I/O errors, пропаже telemetry,
неожиданном LCD mode или зависании остановить источник и выполнить A-015.
Не перебирать framing/opcodes «до появления картинки».

Измерять раздельно:

- submitted frames, успешно записанные frame transactions, read timeout/error;
- фактические интервалы отправки, bytes/sec, p50/p95/max transaction duration;
- scheduler lag, pending transactions, skipped raw frames (если существуют),
  discarded whole generations; не удалять произвольные encoded P-frames;
- snapshot age, RPC latency, CPU/RSS daemon;
- видимое движение/артефакты/black frame. Физический FPS нельзя получить
  из write counter; без наблюдения/видеозаписи называть его sent FPS.

Первый 60s gate: изображение движется, источник останавливается, все
transactions завершены или имеют явную ошибку, sensor age при исправном
устройстве не превышает 3s. Для M1 начальная цель sent rate ≥10 fps и p95
snapshot RPC ≤500ms; это выбранные acceptance budgets, не свойство firmware.
30min soak: queue remains bounded, нет линейного роста RSS; после первой
минуты рост RSS не более 32MiB, включая объяснимый cache; иначе исследовать.
Цель 24 fps измеряется отдельно, не заявляется при достижении только 10 fps.

## Firmware reconnect — отложенная qualification

Текущий probe подключён к internal owned job после acknowledged transfer/checked
original close; product Apply/probe UI и qualified resolution ещё отсутствуют.
Эти пункты относятся к будущей квалификации после software integration и
подтверждения revision/recovery, не являются командами для текущего AIO.
Fault injection firmware на единственном AIO не выполнять.

- [ ] Установить, предоставляет ли реальное устройство стабильный serial и
  сохраняется ли он после штатного reset/reconnect; serial-less auto-verification
  сейчас запрещён, recovery policy остаётся открытой.
- [ ] Сравнить OS hotplug events и enumeration: наблюдается ли detach во время
  reset, меняется ли hidraw path. Пропущенный fast reset не должен считаться успехом.
- [ ] При restart/reconnect сверить `lconnectctl --json firmware-recovery`:
  saved operation/guard сохраняются и видны без ordinary DeviceId; read-only
  inspection не открывает HID и не сообщает success по старому snapshot.
- [ ] После joined old HID owner проверить isolated cmd134 на fresh handle:
  точный identity, PN0/optionalPN1, latency и отсутствие keeper/replay writes.
- [ ] Подтвердить, что версия исходной1.6 совпадает с candidate1.6: простое
  совпадение version не является readback hash установленного image.
- [ ] Проверить disconnect/ошибку readonly probe: guard/journal сохранены,
  ordinary replay не возобновляется. Software negative matrix уже есть;
  опасные flash interruption сценарии требуют отдельного recovery оборудования.
- [ ] После реализации resolution проверить restore policy/cooling/RPM и
  окончательный LocalUI. До этого evidence helper не снимает quarantine.

Подробный [firmware contract](firmware-lcdv2.md) содержит software gates.

## Запись результата

Для каждого прогона создать `docs/protocol/captures/<date>-lcdv2-<scenario>.md`:

```text
Scenario ID:
Commit / dirty scope:
Device SKU / firmware / masked identity:
OS / kernel / encoder version:
Initial state and restoration method:
Config and asset SHA-256:
Exact commands:
Expected:
Observed machine evidence:
Physical observation (who/how):
Duration / sent FPS / latency / RSS / errors:
Recovery outcome:
Result: pass | fail | not-run
Limitation / next action:
```

Mock concurrency tests нужны для детерминированного packet ordering и
cancellation. Hardware smoke нужен для фактического LCD результата.
Ни один из этих уровней не заменяет другой.
