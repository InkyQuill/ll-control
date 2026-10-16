# Suspend/resume — LCDV2

Baseline: Windows2.1.28.0, Galahad II LCDV2. Реальные sleep/wake и HID
проверки отложены; текущий этап использует только MockBackend.

## Реализованная основа manager

`ManagerHandle::prepare_for_sleep(bool)` отправляет упорядоченный запрос
manager и ждёт завершения перехода. При sleep он убирает устройства из IPC
map, останавливает discovery, ждёт завершения старых actor trees и текущей
HID записи. Используется существующий final persistence flush; ошибки
записи сохраняют прежнюю обработку/logging и не становятся успешной записью.

Wake запускает discovery заново даже при неизменном USB path. Устройство
получает новый runtime ID; стабильный serial/key определяет настройки и
сохранённую тему. Диагностический source остаётся Idle. Повтор одного
состояния не пересоздаёт actors. Ошибка enumeration возвращается caller,
а обычный discovery продолжает попытки. Отмена caller после постановки
запроса не отменяет выполняющийся переход.

## Программная приёмка

- [x] Задержанная HID запись удерживает sleep acknowledgement до завершения.
- [x] Во время паузы USB остаётся доступным, но actors не открываются снова
  и новые записи не отправляются.
- [x] После wake новые handles восстановили PWM и brightness; diagnostic
  image не возобновилась. Проверка PWM смотрит только записи после wake.
- [x] Тема сброшена на диск при sleep и восстановлена после wake без
  перезапуска daemon; старый runtime ID больше не принимается IPC.
- [x] Дубликаты sleep/wake без лишнего reopen; shutdown во сне завершается.
- [x] Production main запускает logind observer и завершает его при shutdown.
- [x] PrepareForSleep обрабатывается по порядку, включая queued true→false
  при уже false PreparingForSleep; начальный state читается без property cache.
- [x] Delay-inhibitor освобождается после joined teardown, получается заново
  при wake. Заблокированный HID удерживает FD; при превышении бюджета FD
  освобождается, но join в manager продолжается и новый actor его не обгоняет.
- [x] Private-bus fake-logind: initial sleep, denied inhibitor, owner loss,
  повторная сверка state, автоматическое восстановление после смены owner
  при awake и release FD при shutdown observer. Повторное подключение
  принудительно переоткрывает устройства после ненаблюдаемого интервала.
- [x] Кэш host sensors инвалидируется на sleep и wake; результат collection,
  начатой до границы, отклоняется. До следующей свежей выборки значение Stale.
- [x] Полный перезапуск private D-Bus daemon: уничтожен owned process,
  supervisor переподключился к новому адресу, восстановил inhibitor и устройство
  после sleep; runtime ID сменился, старый FD освобождён.
- [x] Отсутствие logind при старте: минимум один неуспешный цикл и retry
  не останавливают устройство; поздний сервис вызывает resync/reopen.
- [x] Зависший health property read не задерживает sleep signal: teardown и
  освобождение inhibitor подтверждены до ответа fake server и раньше RPC timeout.
- [x] Совместная cooling/RGB/realtime-theme приёмка с synthetic CPU:
  hot → sleep/wake → Stale → fresh hot → cool. Post-wake PWM packets
  сначала100/100fallback, затем90/80hot и45/50cool. RGB обеих зон совпадает
  с alert/base packets; LCD передаёт полные JPEG batches для каждого состояния.
  До нового sample ни один старый hot frame не появляется после wake;
  persistence сохраняет тему, policies/rules и base colors, а не overlay.
- [x] На границе sleep/wake сбрасываются CPU/RAPL/network baselines,
  i915/xe counters и cached xe rows. Первая выборка Unavailable; новая пара
  отсчётов восстанавливает rates без усреднения через паузу. Fixtures проверяют
  скачки счётчиков и освобождение PMU handles. Реальный collector с fake async
  query проверяет invalidation во время collection: старый результат отклонён,
  CPU baseline сброшен, последующая свежая пара даёт корректную нагрузку.

## Аппаратная приёмка позднее

- [ ] Sleep/wake с theme, LocalUI и diagnostic source; USB path как
  неизменный, так и новый; ещё отсутствующее устройство возвращается позже.
- [ ] LCD не возобновляет старый diagnostic stream; сохранённая тема
  визуально восстановлена, requested/applied/error соответствуют результату.
- [ ] Pump/cooling/RGB восстановлены; длительный combined run без LCD stalls.
- [ ] Отмена сна и повторные циклы; после тестов оставить LocalUI.

Контракт logind: [официальное описание интерфейса](https://github.com/systemd/systemd/blob/main/man/org.freedesktop.login1.xml).
Delay-inhibitor ограничен системным deadline; сигнал сам по себе не даёт
неограниченного времени на завершение HID операции. Adapter ограничивает ожидание manager от момента обработки события
(maxDelay минус200ms, не более10s), пишет предупреждение при превышении
и отпускает FD. Это не гарантия deadline от момента emission: queued events,
scheduler и блокирующая HID операция могут исчерпать системный бюджет.
Отказ inhibitor означает best-effort preparation с предупреждением.
При потере logind не предполагается wake: supervisor переподключается
через2s и сверяет текущее состояние. Health query раз в секунду отменяется
при поступлении сигнала, чтобы медленный read не задерживал обработчик сна.

Результат foundation:614Rust tests/92targets, fmt/strict Clippy/docs,
7focused lifecycle tests; [source hashes и логи](evidence/2026-10-11-manager-sleep-wake.json).

Production adapter checkpoint:622Rust tests/92targets, fmt/strict Clippy/docs,
88daemon lib tests; [source hashes и логи](evidence/2026-10-11-logind-integration.json).

Recovery checkpoint:625Rust tests/92targets,91daemon lib tests,
fmt/strict Clippy/docs; [source hashes, scope и логи](evidence/2026-10-11-logind-recovery.json).
Production код не менялся. Private bus recovery не является проверкой
реального system bus на хосте.

Combined checkpoint: [точные source hashes и результаты](evidence/2026-10-11-sleep-combined.json).
Final focused test/fmt/Clippy/docs прошли; предыдущий full run626tests/93targets
выполнен до последних четырёх packet assertions.

Counter baseline checkpoint:631Rust tests/93targets, fmt/strict Clippy/docs;
review без замечаний. [Hashes, сценарии и логи](evidence/2026-10-11-sleep-counter-baselines.json).
Это fixtures/private-bus проверки; физический сон и hardware counters не проверялись.

## Selected prepared media — 2026-10-11

Manager/actor software sleep/wake проверен с выбранным persistent clip:
joined teardown, registry empty, no writes во сне, old-ID rejection, fresh-ID
wake и полный frame batch восстановленного clip. Clear после wake снимает
выбор и следующий restart не отправляет H.264. Все вызовы через private fixture;
системный сон и HID не запускались. Это относится к существующим diagnostic
limits prepared clips; будущий disk-backed long-video runtime требует повторной
приёмки. Полный совместный media+logind fault scenario остаётся отдельным.

[Evidence](evidence/2026-10-11-media-sleep-schema.json):645Rust/95targets,
fmt/strict Clippy/docs и review прошли. Production не менялся.
