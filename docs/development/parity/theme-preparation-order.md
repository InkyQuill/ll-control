# Порядок подготовки и замены LCD-темы

Windows baseline остаётся 2.1.28.0 / LCDV2. Это Linux concurrency contract,
а не наблюдение Windows runtime или физического LCD.

До исправления set_lcd_theme загружал assets/fonts перед отправкой команды
LCD actor. Другой IPC client мог завершить Clear за это время; поздняя тема
затем снова включала streaming и сохраняла старое намерение пользователя.

Теперь IPC сначала получает у LCD actor токен подготовки. Новая подготовка
аннулирует предыдущий токен, не останавливая текущую рабочую тему. Замена
источника (включая Clear/mode patch/image/clip) также аннулирует токены через
общий replace. После загрузки assets actor проверяет токен перед первым
рендером, записью settings и отправкой frame. Устаревшая тема получает
DeviceBusy с объяснением superseded; текущий источник остаётся неизменным.

| Порядок событий | Ожидаемый результат |
|---|---|
| Begin A → Begin B → готова A, B ещё готовится | A отклонена, ни frames, ни settings от A |
| Begin A → Begin B → Apply B → готова A | B остаётся активной; A не меняет persistence |
| Begin A → Clear подтверждён → готова A | Idle и пустая active theme; A не возобновляет вывод |
| Clear → Begin C → Apply C | Новый явный запрос успешно включает тему |
| Caller task отменён во время фоновой подготовки | Результат и permit освобождаются после окончания работы; deferred enqueue отсутствует |

Последняя строка опирается на существующий join_theme_job и структуру IPC
метода; отмена waiter и удержание capacity проверены отдельно. Закрытие
socket **не равно** отмене Rust future: IPC dispatcher может уже исполнять
запрос. Принятая LCD actor команда завершается владельцем устройства даже
после потери ответа. Native сценарии dropped reply проверяют это отдельно.

Manager replay передаёт None вместо токена: это уже подготовленная тема в
последовательности startup до публикации device. Внешний IPC всегда использует
токен. Brightness/rotation patch без mode не отменяет подготовку темы.

Детерминированная actor-регрессия находится в
`crates/llcontrol-daemon/tests/theme_runtime.rs`,
`superseded_preparation_cannot_restore_a_cleared_or_newer_theme`.
Она управляет очередью командами/oneshot replies, без timing-dependent sleeps.
Реальная загрузка assets с удерживаемым I/O проверена ниже; transport failures
сопоставлены в [отдельной matrix](transport-consumer-acceptance.md). Физическое
отсутствие старых кадров и firmware resync остаются аппаратными gates.

## Preview UI: late replies и recovery, 2026-10-11

[Evidence](evidence/2026-10-11-preview-recovery.json): шесть browser сценариев
проверяют late success/error после изменения draft, смены device и reconnect.
Старый ответ не возвращает image/error/success notice. Актуальная ошибка видна;
explicit preview retry очищает её и использует текущие device/theme. Одновременно
работает не более одного preview request; set/clear не отправляются.
Вместе с существующим live-preview stop/coalescing и interrupted Display набором
прошли13 tests. Svelte check0/0. Производственный код не менялся.

Первый общий запуск дал12 pass/1 fail в старом reconnect test при параллельном
svelte-kit sync. Повтор без sync прошёл13/13; перезагрузка dev-страницы — возможная,
но не доказанная причина. Browser acceptance не заменяет real I/O preparation
race, native cancellation или аппаратный frame barrier.

## Реальный asset I/O: Clear и image replacement, 2026-10-11

[Evidence](evidence/2026-10-11-preparation-read-gate.json): отдельный private
fixture принимает `set_lcd_theme` через Unix IPC. Linux preload удерживает
`read` только известного PNG внутри `ThemeAssetStore::load` и сигнализирует,
что чтение действительно началось. File metadata/decode/runtime идут обычным
production путём; FIFO или подмена результата prepare не используются.

Другой IPC client выполняет Clear либо set_lcd_image и получает ответ до
освобождения read. Старая рабочая тема продолжает быть активной до этой замены.
После освобождения pending Apply возвращает superseded. Source/generation/error
сохраняют результат замены, sent_frames не увеличивается, active theme=null,
после flush в TOML нет старой theme. Новый явный Apply той же темы проходит.

Запуск без GUI, реального HID и host sensors:

```sh
cargo build -p llcontrol-daemon --example desktop-mock --locked
python3 desktop/scripts/theme-preparation-smoke.py
```

Нужны Linux, cc и Python3.11+. Скрипт компилирует shim с strict warnings,
создаёт private root, запускает только desktop-mock и останавливает его в finally.
Gate имеет10s deadline; обычный процесс/GUI не получает LD_PRELOAD. Итог и logs
сохраняются в `/tmp/llcontrol-preparation-*/`. Набор не меняет пользовательские assets.

## Закрытие software ordering gate

| Граница | Подтверждение |
|---|---|
| Late preview после правок/device/reconnect, retry |6 browser cases + live-preview и interrupted Display,13 pass |
| Background waiter отменён, capacity удержана до завершения | ipc_server unit tests `cancelled_theme_waiter_retains_capacity_until_background_work_finishes`, `preparation_hands_capacity_to_next_stage_and_failure_releases_it` |
| A/B preparation, поздний A не заменяет B, Clear invalidation | actor test `superseded_preparation_cannot_restore_a_cleared_or_newer_theme` |
| Настоящее чтение PNG удерживается во время Clear/image replacement |2 Unix IPC scenarios выше |
| Asset/font invalid/missing preparation сохраняет рабочую тему | `theme_ipc::preview_does_not_apply_and_failed_theme_preserves_active_composition`, native dependency recovery checkpoints |
| Frame batch/replace/clear, ошибка transport | [transport matrix](transport-consumer-acceptance.md), lcd_keeper/hid_io tests |
| Принятая mutation завершилась, ответ потерян | native theme/clear lost-reply и browser interrupted Display checkpoints |

Базовый software disconnect/cancel/replace gate закрыт совокупностью этих
проверок. Socket disconnect не объявляется отменой принятой команды. Общий
итоговый прогон на одном состоянии дерева и физические USB/pixels/lifecycle
остаются в checklist; результаты разных checkpoints не заменяют этот прогон.

## Команды во время периодического render — 2026-10-11

Выявлен отдельный runtime gap: `lcd_keeper` ожидал `runtime.frame()` внутри
ветки таймера, поэтому LCD State/Clear/replace ждали окончания рендера.
Теперь periodic refresh обслуживает команды во время ожидания render future.
Перед отправкой результата проверяется generation: отменённый кадр и его
поздняя ошибка не меняют новый источник. HID frame batch остаётся неделимым.

Три controlled-future regression в `actor/lcd_render_tests.rs` проверяют:

- State и Clear завершаются до снятия render gate; поздние success/error
  отбрасываются, LocalUI сохраняется и старый JPEG не отправляется.
- Manual image отправляется до снятия gate; после завершения старой работы
  в HID остаётся только полный batch нового изображения.
- Shutdown ждёт завершения render job, затем выходит без отправки кадра.

Один periodic job сохраняется до завершения, даже после Clear/replace:
это предотвращает накопление detached encoder jobs при смене темы.
Новый periodic refresh ожидает завершения старого job; первичный SetTheme
по-прежнему подготавливает первый кадр последовательно. Проверки используют
управляемое ожидание на границе render future, а не замер реального CPU encode.
Начальный Apply, объединённый IPC/cooling/RGB slow-encode сценарий и аппаратная
latency остаются отдельными проверками; общий render-loop gate ещё не закрыт.

[Evidence](evidence/2026-10-11-responsive-lcd-render.json):3 controlled-future
tests и25 focused integration tests прошли; итоговый fmt/strict Clippy,605
Rust tests (0failed/ignored,92targets), docs и companion review — pass.
Ready render имеет приоритет над входящими командами; политика выбора
таймера во внешнем actor loop не менялась.

## Первый кадр до очереди LCD-команд — 2026-10-11

`ThemeRuntime::prepare` возвращает `PreparedTheme`: runtime и соответствующий
ему уже отрендеренный/проверенный JPEG нельзя независимо подменить через
публичный API. IPC резервирует preparation ticket, затем загружает assets и
рендерит первый кадр внутри существующего `join_theme_job`, сохраняя permit
до завершения фоновой работы даже при отмене ожидающего запроса. Только после
этого подготовленная тема попадает в LCD actor. Startup replay использует
тот же prepared тип; restore errors сохраняют прежний видимый контракт.

Actor проверяет ticket перед persistence/HID. Поэтому Clear или более новый
intent во время первого рендера делают его результат устаревшим; старый Apply
не возвращает тему на экран. В actor больше нет ожидания первого raster/encode.
HID batch и сохранение принятого intent остаются последовательными.

Регрессия `first_frame_preparation_keeps_controls_available_and_clear_invalidates_apply`
занимает единственный blocking worker изолированного runtime, запускает реальную
подготовку кадра и получает State/Clear до освобождения worker. После фактического
рендера старый ticket отклонён, дополнительных HID packets нет. Это задержка
очереди raster/JPEG, не benchmark CPU. Existing active-theme IPC failures,
preparation ordering и replay regressions также сохранены.

Граница: проверены LCD command channels. JSON-RPC server пока обрабатывает
запросы одного соединения последовательно; доступность второго запроса того же
IPC stream при долгом Apply этой регрессией не доказана. Общий IPC/cooling/RGB
slow-encode критерий остаётся открытым и должен включать это ограничение.

[Evidence](evidence/2026-10-11-initial-theme-preparation.json):93 focused tests,
fmt/strict Clippy,606 Rust tests (0failed/ignored,92targets),docs и companion
review прошли. Hardware не использовалось.

## Apply/preview не блокируют один IPC stream — 2026-10-11

Transport теперь отдельно принимает `set_lcd_theme` и `preview_lcd_theme`:
валидация и admission идут в wire order, затем bounded future отвечает по ID
через общий writer. Apply резервирует LCD ticket до чтения следующего запроса;
Clear и более новый Apply сохраняют право отменить старую подготовку. Остальные
RPC остаются последовательными. Legacy implementations trait по умолчанию
завершают старый метод inline и сохраняют прежний порядок.

На соединение допускаются максимум4 deferred replies, включая ожидающие запись
ответа; общий daemon limit остаётся2 theme jobs. Переполнение возвращает busy
без блокировки последующих controls. При teardown JoinSet отменяет ожидающие
ответы; actual work удерживает daemon permit через `join_theme_job` до завершения.
Уже принятая LCD-команда по-прежнему выполняется owner-ом.

`theme-preparation-smoke.py` теперь использует один Unix socket для задержанного
PNG Apply и следующих действий: get theme/state, fan PWM61, pump RGB, Clear или
manual image. После release все устаревшие Apply дают superseded; текущие
source/generation/error и persistence сохранены, явный новый Apply проходит.
Отдельно задержанный preview не мешает Clear и сам ничего не применяет.
Два занятых daemon slots отклоняют третий job, но controls продолжаются.

Подтверждены fresh MockBackend fan_pwm/pump_rgb write counters до снятия gate;
отсутствие stale JPEG проверено по fresh `jpeg_starts`. Ранее использовавшийся
`sent_frames` относится к diagnostic clip и для этой JPEG-проверки недостаточен.
Transport regression дополнительно проверяет4 deferred slots, ответы по ID,
busy пятому, доступность list_devices и освобождение future guards при abort
connection handler. Во время проверки не открываются real HID/host sensors.

Это реальная задержка asset I/O, не замер тяжёлого raster/encode. Первичный
raster queue и periodic render boundary покрыты отдельными regressions выше;
общая строка slow-encode checklist остаётся открытой до сопоставления полного
сценария. Нового native GUI или physical latency результата этот тест не даёт.

[Evidence](evidence/2026-10-11-deferred-theme-ipc.json):607 Rust tests
(0failed/ignored,92targets),fmt/strict Clippy/docs,26 contracts,review и
повторный isolated Unix IPC/read-gate smoke прошли.

## Реальный CPU raster/JPEG и общий render-loop критерий — 2026-10-11

`theme-render-load-smoke.py` запускает только private desktop-mock без preload
и без GUI. Тема из32 текстовых слоёв с shaping/fallback/wrapping рендерится
обычным production runtime. Для preview и Apply измерены3,63–3,83s wall и
3,56–3,59s process CPU. State/sensors/PWM/RGB/Clear на том же socket отвечают
за0,07–0,23ms в этом прогоне; после Clear остаётся2,78–2,97s CPU работы.
Это доказывает, что команды завершились до конца рендера, а не после него.

При CPU нагрузке fixture:cpu переходит к95°C; fan curve без нового Apply
самостоятельно применяет100%, без fallback. Проверены свежесть reading
(4/189ms), фактические mock PWM/RGB writes и отсутствие JPEG от отменённого
Apply. Preview возвращает JPEG, не меняя источник.

После успешного Apply той же тяжёлой темы periodic render отменяется Clear
за0,96ms; CPU работа продолжается ещё3,44s. Новая лёгкая тема применяется,
после drain старой работы остаются ровно2 JPEG начала: исходный Apply и recovery,
без устаревшего periodic кадра. Итоговый Clear сохраняет Idle.

[Evidence с полной темой, measurements и hashes](evidence/2026-10-11-render-load.json).
Это один debug-run и software responsiveness criterion, не performance SLA.
Process CPU прочитан только у запущенного fixture; host sensors/HID не использованы.
Production Rust не менялся после607-test IPC checkpoint; повторять этот набор
для нового Python runner не требовалось. Syntax/diff/document checks прошли.

Общий software render-loop критерий теперь сопоставлен целиком:
actual CPU load выше покрывает IPC/cooling/RGB/HID progress; `theme_runtime`
integration tests покрывают свежий sensor snapshot/missing/stale; native renderer
checkpoint и Unix fixture без GUI подтверждают daemon-owned runtime. Hardware
latency, длительный совместный прогон и platform scheduling qualification
остаются отдельными аппаратными пунктами checklist.
