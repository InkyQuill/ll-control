> Снимок перед очисткой 2026-10-10; не активный roadmap.

# Roadmap: LL-Control для Galahad II LCDV2

Актуализировано: 2026-09-08. Windows baseline: локальная установка
**L-Connect 3 2.1.28.0**. Доступное железо: **Galahad II LCDV2, 0416:7395**.
Исторически проверена firmware 1.6; перед новыми hardware tests читать её заново.

## Цель

Полная функциональная эквивалентность L-Connect 3 для нашего LCDV2:
управление содержимым LCD, охлаждением, подсветкой и сохранёнными профилями.
Экран — центральная возможность продукта: пользователь должен выводить
собственные изображения, видео и динамические композиции с датчиками,
часами, текстом и графикой, настраивать их и переключать без Windows.

Parity оценивается по применимым к LCDV2 пользовательским сценариям
2.1.28, а не по наличию похожих настроек или совпадающему набору команд.
Функции других моделей не входят в текущую цель. Неизвестная применимость
не означает «не требуется»: её необходимо установить по reference.

Этот файл — основной roadmap. Старые Phase 0–5 specs и планы используются
как reference; их ограничения v1 и порядок работ не определяют эту цель.
Подробные implementation plans создаются по отдельным этапам после
разрешения их исследовательских зависимостей.

## Подтверждённая основа

- [x] Rust workspace: protocol, HID backend, daemon, IPC, CLI, image preparation.
- [x] Кодеки LCDV2: sensors, PWM, RGB, LCD и B/C H.264 framing.
- [x] Исторический вывод JPEG на реальный LCD через cmd 14; daemon refresh
  удерживал изображение более 60 секунд. Это уже работающий вывод картинки.
- [x] Исторические live-проверки части RGB, telemetry и persistence replay.
- [x] Проверка 2026-09-08: 299 тестов, Clippy и форматирование прошли.
- [x] 2.1.28.0 декомпилирована: десять основных сборок, SHA-256 manifest.
- [ ] GUI, dynamic rendering, полноценное GIF/video playback.
- [ ] Linux CPU/GPU providers и автономные cooling curves.
- [ ] Полная hardware acceptance: restart, hotplug, sleep/wake, cold boot.

Codec tests не подтверждают playback. Исторические результаты не заменяют
новый hardware run. Проверенная отправка JPEG не означает, что cmd 15,
offline content или firmware update уже работают.

## Последовательность этапов

### M0 — точный baseline и карта функций

**Результат:** матрица LCDV2 2.1.28 и техническое описание пути до экрана.

- [x] Проследить LCD UI → settings/requests → service → renderer/encoder → HID.
- [x] Зафиксировать изображения, media, sensor layouts, часы, text/graphics,
  редактор, preview, библиотеку, capture/record и переключение содержимого.
- [x] Составить аналогичную карту cooling, RGB, thermal alerts, profiles,
  firmware и lifecycle; отделить применимые функции от общих enum.
- [x] Описать параметры encoder, frame boundaries, pacing, ACK/timeout,
  B/C selection, suspend/resume и повторную инициализацию.
- [x] Сверить существующий код/тесты/captures; выписать конкретные gaps.
- [x] Подготовить воспроизводимый baseline smoke и отдельный план M1.

Первый план: [M0 — baseline и LCD pipeline](../../superpowers/plans/README.md).

M0 завершён 2026-09-08. Результаты:

- [Матрица: 33 сценария](../lcdv2-2.1.28-matrix.md).
- [LCD pipeline 2.1.28](../../protocol/lcdv2-screen-pipeline-2.1.28.md).
- [Пробелы реализации](../lcdv2-baseline-gaps.md).
- [Hardware acceptance protocol](../lcdv2-hardware-acceptance.md).
- [Исполнимый план M1](../../superpowers/plans/2026-09-08-m1-lcdv2-dynamic-display.md).

Открытые вопросы M0 получили следующие проверки: startup/offline остаётся
applicability-unknown (LCD-016), H.264 readback/decoder acceptance проверяется
ограниченным M1 spike. Это не заявления о готовом hardware playback.


### M1 — надёжная основа вывода на LCD

**Зависит от:** M0. **Результат:** управляемый поток меняющихся кадров
через daemon и CLI, работающий одновременно с telemetry и PWM.

- [x] Устранить обнаруженные дефекты интеграции, начиная с socket paths
  daemon/CLI и чтения текущих LCD settings без сброса соседних полей.
- [x] Повторить JPEG smoke, переключение изображений и возврат LocalUI.
- [x] Проверить C-framing H.264 на текущей firmware с короткой собственной
  тестовой анимацией; B-framing выбирать только по подтверждённому контракту.
- [ ] Реализовать frame-source lifecycle, bounded buffering, pacing,
  остановку/замену потока и восстановление после disconnect.
- [ ] Проверить отсутствие перемешивания пакетов разных кадров и голодания
  sensor/PWM-команд; измерить реальный FPS и загрузку системы.

**Текущий статус:** JPEG и H.264 подтверждены на LCDV2 FW1.6. Исправлено
голодание кадров во время ожидания sensor reply: 60s повтор дал23,35 sent fps
при живой telemetry, пользователь подтвердил отсутствие лагов. 30min soak
прошёл: 23,34 sent fps, ноль ошибок, sensor age≤0,96с, RSS +188KiB после
первой минуты. Проверены stop/replace и restart на уровне daemon/CLI;
визуальное подтверждение коротких переключений pending. Физический USB
reconnect отложен пользователем. Полный lifecycle gate M1 ещё не закрыт.

**Готово, когда:** движущееся изображение выводится на физический LCD,
заменяется и останавливается; daemon остаётся отзывчивым; ошибки и recovery
имеют тесты и live evidence. Cmd 15 не блокирует transient streaming.

### M2 — датчики и автономное охлаждение

**M2a, 2026-09-09:** реализован host-каталог с IPC/CLI `sensors`: CPU/GPU/DIMM
температуры поддерживаемых hwmon-драйверов, CPU utilization, memory usage.
Проверены реальные host readings, стабильные ID, invalid/unavailable/stale,
кэш без HID I/O и IPC без USB-устройства. NVIDIA proprietary provider,
GPU utilization/VRAM, общий каталог с LCDV2 coolant/RPM и cooling policy
ещё не были реализованы на этапе M2a (coolant/RPM и policy добавлены ниже в M2b). Подробности — в
[плане M2](../../superpowers/plans/2026-09-09-m2-sensors-and-cooling.md).

**M2b, 2026-09-09:** добавлены стабильные coolant/RPM bindings, per-device
cooling actor, fixed/curve/motherboard policies, интерполяция, гистерезис,
явный fallback и persistence/replay. Ручные PWM-команды отменяют политику
только своей цели. Wire/actor/IPC проверки пройдены (387 workspace tests); live-каталог из
11 датчиков проверен без PWM. Аппаратная проверка помпы ожидает рабочих
PWM/source; вентиляторы, по словам пользователя, вероятно подключены к материнке.
**Дополнение M2b:** реализованы pump `fixed_rpm`/`curve_rpm`, Quiet/Performance
и двухэтапное преобразование Windows 2.1.28 с точным порядком округления.
CLI генерирует редактируемые JSON-пресеты; persistence сохраняет RPM-точки.
391 workspace tests, Clippy, fmt, docs и release build прошли; ревью без замечаний.
Фактические RPM ещё не измерены; это reference mapping без обратной связи.
На этом этапе fan presets и температурный RGB alert ещё оставались впереди. Для alert уточнены
priority/restore и 10-секундный manual grace; шаги реализации записаны в плане M2.

**M2c, подготовка:** ручные RGB-команды, direct LED colors и refresh теперь
сериализуются одним lighting actor с подтверждением записи и принятия
settings delta. Устранена гонка со старым refresh; direct-данные удерживаются
до смены режима, без сохранения на диск. 397 тестов и полные проверки прошли.
**M2c, 2026-10-10:** подключён температурный RGB overlay: независимые CPU/GPU
bindings, threshold/color, приоритет последнего срабатывания, удержание при stale,
10-секундный manual grace, восстановление базовых режимов, per-zone errors/retry,
IPC/CLI и persistence. 408 workspace tests, fmt, Clippy, docs и независимое ревью прошли; детали в плане M2.
Аппаратная RGB acceptance не проводилась. Добавлено взаимодействие с direct per-LED: переключение Application source,
retained-prefix overlay/restore и сохранение хвоста при короткой записи.
Число охваченных LED видно в state. Покрытие всей физической зоны неизвестно;
визуальная совместимость требует аппаратной проверки.

**M2b, fan presets, 2026-10-10:** Quiet/Standard/High/Full генерируются через
`cooling preset NAME --target fan`. `fan_curve` сохраняет исходные точки и
целочисленную арифметику Windows, не меняя существующую `curve`. Hysteresis,
fallback, binding validation и persistence используют готовый cooling actor.
Физические RPM и четыре именованных UserDefine-слота ещё не подтверждены/реализованы.

**Зависит от:** M0 и стабильного daemon; может выполняться независимо от media UI.

- [ ] CPU/GPU/memory providers с identity, units, validity и freshness.
- [ ] Общий реестр bindings для LCD и cooling; без синтетических значений
  при отказе реального датчика.
- [ ] Fixed/curve/presets и применимые источники температуры, pump limits,
  MB sync; расчёт в daemon, сохранение и replay.
- [ ] Hysteresis и определённое поведение при stale/unavailable sensor.
- [ ] Температурный RGB alert, его приоритет и восстановление настроек.

**Готово, когда:** охлаждение работает без GUI, bindings доступны экрану,
потеря источника и restart проверены, фактические RPM измерены.

### M3 — desktop и полезный динамический экран

**Зависит от:** M1; датчики поступают из M2.

- [ ] Tauri 2 + SvelteKit/Svelte 5 + Tailwind v4; использовать согласованное
  направление Aurora, обновив старый GUI-план под реальный IPC.
- [ ] Display: собственное изображение, preview, fit/crop/rotate, выбор
  композиции, запуск/остановка и понятный applied/error status.
- [ ] Renderer: sensor/clock/text/image/graphics layers, параметры,
  bindings и условия, переключение композиций без перезапуска daemon.
- [ ] Dashboard, Cooling, Lighting, Settings; живое состояние, отсутствие
  устройства, reconnect и восстановление draft/applied settings.

**Готово, когда:** пользователь устанавливает приложение и настраивает
живой экран с часами/датчиками; закрытие GUI не останавливает показ.
Проверки frontend mock, реального Tauri IPC и hardware разделены.

### M4 — полный LCD workflow

**Зависит от:** M1/M3 и матрицы M0.

- [ ] GIF/видео: decode, fit/crop, trim и speed там, где применимо,
  playback/loop и переключение media без зависаний.
- [ ] Библиотека материалов и тем, импорт/экспорт, thumbnails и preview.
- [ ] Пользовательский редактор композиции и эквиваленты применимых
  Windows layouts; собственные assets вместо копирования фирменной графики.
- [ ] Screen capture/record для Linux desktop sessions; явно проверить
  поддерживаемые Wayland/X11 сценарии.
- [ ] Display defaults, brightness/rotation, startup/offline content,
  если их поддержка подтверждена для LCDV2. Отдельно исследовать cmd 15.

**Готово, когда:** каждая применимая строка LCD matrix имеет end-to-end
проверку, включая восстановление после сна и смену источника во время playback.

### M5 — остальные функции LCDV2 и удобство использования

- [ ] Полный применимый RGB mode/zone/sync набор и thermal-alert conflicts.
- [ ] Profiles: сохранить, выбрать, экспортировать, импортировать и восстановить.
- [ ] Совместимость Windows settings/assets formats — отдельные тесты;
  невозможность импорта не скрывать под наличием своего формата.
- [ ] Tray/autostart, единицы, локализация, диагностика и user-facing recovery.
- [ ] Закрыть применимые дополнительные sensor/system workflows из M0.

### M6 — firmware и выпуск с заявленным parity

- [ ] Исследовать update/recovery именно LCDV2: target validation, firmware
  format, transport, exclusive ownership, progress и проверку результата.
- [ ] Реализацию и опасные fault-injection tests планировать отдельно;
  единственное рабочее AIO не использовать для эксперимента с прерыванием flash.
- [ ] Проверить весь baseline на физическом LCDV2: GUI закрыт/открыт,
  cold boot, daemon restart, hotplug, sleep/wake, media + cooling + RGB.
- [ ] Измерить устойчивость и ресурсы; закончить CI, packaging, install,
  upgrade/uninstall и исправить metadata/installation inconsistencies.

Полезные промежуточные релизы допустимы после M3. Название «full parity»
допустимо только после закрытия всех применимых сценариев, включая firmware,
либо при явном указании более узкой границы релиза.

## Архитектурные ориентиры

- Daemon владеет HID, cooling loop и активным display pipeline; GUI — клиент.
- Протокол остаётся чистым codec layer. Renderer/media не пишут в HID напрямую.
- API различает requested, applied и unavailable/error state.
- Firmware quirks и ограничения не подменяются общими assumptions.
- Условия и пользовательские параметры тем остаются требованием проекта.
- Новые driver abstractions вводятся по реальной потребности, без реализации
  неподтверждённых семейств «на будущее».

## Отложено до появления железа

SL/AL/SL-INF, Strimer, другие AIO/LCD, wireless, FLEX, несколько устройств
и экранов. Исследовать источники можно; заявлять hardware support нельзя.
ENE bootloader план не является updater для LCDV2.

`joeytroy/ll-connect3` — reference для UX, Linux sensors и будущего SL-INF.
Его kernel/proc architecture и simulated sensor fallback не переносятся
в наш daemon.

## Документы и доказательства

- [Первичный аудит](../../2026-09-08-parity-audit-roadmap.md) — исторический
  снимок; последовательность этапов теперь определяется этим roadmap.
- [Reference 2.1.28 и ll-connect3](../../2026-09-08-reference-2.1.28-and-ll-connect3.md).
- [Протокол LCDV2](../../protocol/galahad2-lcdv2.md).
- [Исторический smoke](../../protocol/captures/2026-04-27-phase-1-smoke-run.txt).
- [GUI design](../../superpowers/specs/2026-04-27-phase-2-gui-tauri.md) —
  направление стека/визуального языка; прежняя GUI-local curve не применяется.

Каждый этап имеет отдельный plan и acceptance evidence. Исследовательские
неизвестные разрешаются до фиксации зависимого implementation contract.
