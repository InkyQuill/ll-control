# RGB: программные контракты и оставшаяся parity

Сверка 2026-10-11, Windows baseline 2.1.28.0 / LCDV2. Assertions просмотрены,
27 tests повторно прошли, 0 failed/ignored. [Лог и hashes](evidence/2026-10-11-rgb-acceptance-map.json).
Код не менялся; полный workspace и native GUI в этой сверке не запускались.

Пути в таблице относительно `crates/lconnect-daemon/tests/`. Это контракт
существующего runtime, не утверждение о полноте Windows mode catalog.

| Требование | Проверка и наблюдаемый результат |
|---|---|
| Единственный RGB owner, порядок refresh/manual | `lighting_owner.rs::delayed_refresh_cannot_overtake_manual_and_ack_includes_settings`: queued manual предшествует overdue refresh; ack после write/settings; старый цвет не возвращается |
| Ошибка ручной записи | `failed_manual_keeps_previous_refresh_and_does_not_persist_failed_intent`: error, прежний refresh, failed intent не сохранён |
| Direct/mode transition и Clear barrier | `direct_colors_replace_refresh_until_next_mode_and_clear_is_a_barrier`: Application preparation → cmd20 → refresh; mode заменяет direct; после Clear записей нет; direct transient |
| Partial direct failure | `direct_data_failure_after_preparation_does_not_commit_and_retry_prepares_again`: failed cmd20 не становится refresh base; explicit retry повторяет preparation |
| Thermal threshold и CPU/GPU priority | `thermal.rs::newest_trigger_wins_stale_retains_and_valid_cool_sample_removes`: crossing threshold, последний trigger, stale сохраняет alert/degraded, свежий cool снимает |
| Manual grace и актуальная base | `overlay_grace_restart_latest_base_restore_and_persistence`: через9s base; повторный manual перезапускает grace; после ещё9+2s alert возвращается; disable восстанавливает последний manual цвет |
| Отдельные ошибки зон и retry | `partial_failure_retry_stale_recovery_and_direct_restore_failure`: pump error не объявляется applied, fan не получает чужую ошибку; retry и fresh input восстанавливают работу |
| Direct retained prefix | `direct_alert_restores_exact_prefix_and_short_updates_keep_known_tail`: обе зоны, точные cmd20 bytes для трёх известных LED; короткий update сохраняет известный tail; alert/restore не теряет base |
| Direct overlap и invalid inputs | `direct_transition_during_alert_gets_grace_and_failed_restore_can_retry`, `oversized_or_empty_direct_update_cannot_replace_retained_base`: grace/retry и сохранение base при rejected update |
| Sensor-driven arithmetic | `dynamic_rgb.rs`: integer input, HSV interpolation, high-first равные thresholds, четыре цвета, speed interpolation, endpoint bypass и RPM clamp3600; invalid/missing/stale/nonfinite/wrong unit не превращаются в0 |
| Dynamic/thermal/manual priority | `thermal.rs::sensor_rgb_thermal_priority_restore_manual_cancel_and_stale_fallback`: thermal suppresses dynamic; fan edit сохраняет pump policy; stale возвращает manual base; pump manual отменяет policy и сохраняет отмену |
| Mode validation и wire output | `sensor_rgb_four_wire_colors_speed_retry_and_mode_validation`: static отклоняет4colors/speed; meteor передаёт4RGB и speed2 при50%; retry снимает error; direct отменяет dynamic |
| Binding recovery | `sensor_rgb_replay_retains_binding_and_recovers_when_source_returns`: чужой ID не подставляется, missing сохраняет binding/degraded, возврат исходного ID восстанавливает цвет |
| Durable enable/update/disable | `dynamic_rgb_ipc.rs::dynamic_policy_rpc_updates_replays_and_disables_durably`: три запуска manager, RPC/readback и store; изменённая policy восстанавливается, disabled policy после restart не возвращается |

Дополнение: [native транспортная независимость](transport-consumer-acceptance.md),
[invalid/recovery GUI](native-keyboard-acceptance.md#dynamic-rgb-invalidrecovery--2026-10-11).
Это сохранённые checkpoints, не новые native runs в этой сверке.

## Полные ручные палитры — 2026-10-13

GUI, CLI (`--additional-color` для каждого цвета2–4), IPC и persistence теперь
передают все цвета выбранного режима. Legacy `color` остаётся первым;
`additional_colors` отсутствует в старых настройках и по умолчанию пуст.
При явно переданных дополнительных цветах число слотов должно соответствовать
Windows2.1.28: Rainbow/RainbowMorph —0; Static/Breathing —1; Runway —2;
Meteor —4 для fan и pump. Pump дополнительно: TickerTape/Fluctuation/BigBang/Bounce
—4; Transmit —2; StaticStarryNight/Voice/Burst —1;
ColorfulStarryNight/ColorsMorph —0. `static_color` — совместимый alias `static`.

Два и четыре RGB проходят через единственного owner в cmd133/cmd131; unused
slots остаются black. Неверные размер/цвет сохранённой палитры отклоняются,
прежний файл сохраняется. Native WebKitGTK/Unix IPC/MockBackend проверил четыре
разных цвета в обеих зонах, persistence и точные RGB bytes после restart.
Browser21, focused RPC/replay/legacy-corruption2, CLI проверки, strict
clippy/fmt/rustdoc и builds прошли. Scope полного workspace snapshot указан
отдельно в [evidence](evidence/2026-10-13-rgb-mode-palettes.json).

Это закрывает передачу и сохранение полной ручной палитры. Windows defaults,
mode-specific speed/brightness/direction, восстановление черновиков каждого
режима ещё требуют реализации/проверки. Явная загрузка текущей базы в GUI
реализована следующим срезом.

## Явная загрузка настройки зоны — 2026-10-14

`get_lighting_info.base` возвращает полную базовую настройку из RGB owner:
mode, все RGB slots, brightness/speed, direction, scope и disabled. Это текущая
committed manual base, включая восстановленную после restart; HID query не
выполняется. Alert/dynamic цвета и transient direct output не выдаются за mode
base. Для unset/direct/Clear возвращается None; старые daemons могут не прислать
поле. Ошибка ручной записи не заменяет предыдущую базу.

Кнопка «Загрузить настройки подсветки» явно переносит base в черновик без Apply.
Поздний ответ после редактирования, смены зоны, устройства или reconnect не
перезаписывает черновик. Отсутствие базы и ошибка запроса сохраняют значения;
retry явный. Legacy дополнительные цвета загружаются black, как прежние wire
slots. Автоматического восстановления отдельных черновиков всех modes пока нет.

Workspace915/2ignored, strict clippy/fmt/rustdoc, Svelte0/0, семь focused browser
сценариев и21 прежний RGB сценарий приняты. Native WebKitGTK/Unix IPC/MockBackend
прошёл Apply → изменить черновик → Load для обеих зон; все цвета и restart bytes
сохранены. [Evidence](evidence/2026-10-14-rgb-base-load.json).

## Manual mode catalog и отдельные профили — 2026-10-14

IPC публикует полный versioned catalog:6 fan и16 pump modes. Для каждого
передаются0/1/2/4 цвета, visibility brightness/speed/direction и defaults.
Numeric inventory извлечён из Windows2.1.28 Galahad2VisionProfile; все строки
проверены независимо от GUI. Неиспользуемый Static speed sentinel int.MinValue
представлен0%, как Windows wire conversion; pump scope по умолчанию Inner,
согласно Core PumpLightingSetting. Ни код, ни assets Windows не перенесены.

GUI использует catalog. Черновики разделены по device/zone/mode; reset defaults
меняет только выбранный черновик. Apply сохраняет профиль режима независимо от
других применённых режимов. `lighting.fan_modes`/`pump_modes` ограничены6/16
ключами; legacy settings без maps читаются. Неверный key/mode и повреждённая
палитра не заменяют файл. Owner держит профили и текущую базу атомарно;
воспроизведение после restart отправляет только активный режим. Неотправленные
черновики сохраняются в текущем GUI сеансе, применённые — в daemon settings.

Browser проверил все22 controls/defaults, разделение device/zone/mode,
reset без Apply и восстановление отдельно применённых настроек после reload.
Native WebKitGTK прошёл весь catalog, черновики/defaults/load, Apply Runway и
Meteor обеих зон; после restart проверены обе сохранённые настройки и только
активный RGB output. Workspace917/2ignored, strict clippy/fmt/rustdoc,
Svelte0/0/build приняты. [Evidence](evidence/2026-10-14-rgb-mode-catalog.json).

## Fan↔pump sync и LED count — 2026-10-14

Windows MainPage XAML связывает manual slider/textbox с Core Constants8…50.
Controller использует manual count при sync=false; при sync=true SKU280→16,
SKU360→24. Manual value сохраняется. Defaults: sync=false,360mm,24 LED.
Это глобальные параметры fan zone, не mode profile. Motherboard sync — отдельный
сценарий; этот флаг синхронизирует fan RGB с pump RGB внутри LCDV2.

IPC `fan_topology` содержит sync_to_pump, size280/360 и manual_led_count.
GUI/CLI Apply проходит через единственного owner; cmd133 offsets24/25 содержат
sync/effective count. Пропущенный topology в legacy SetFan сохраняет текущий;
pump update не принимает fan topology. `lighting.fan_topology` — единственный
persisted источник; per-mode snapshots используют актуальную global topology.
Ошибки write/validation не заменяют прежние параметры. Thermal overlay временно
сбрасывает sync, оставляет effective count, затем восстанавливает базу.

GUI проверяет целый диапазон8…50, сохраняет manual count при sync, разделяет
черновики устройств и не сбрасывает topology кнопкой mode defaults. Устаревший
`change` прежнего устройства игнорируется при загрузке новой формы. CLI принимает
`--sync-to-pump true|false`, `--fan-size 280|360`, `--led-count 8..50`; значения
передаются с пробелом после флага. Непереданные поля сохраняются. Старый daemon,
не поддерживающий topology, не получает неподдержанный update.

Workspace922/2ignored, browser34, strict clippy/fmt/rustdoc, Svelte0/0/build
прошли. Native WebKitGTK/MockBackend проверил все переключения и сохранённый
manual31; после restart sync=true/280 воспроизводит cmd133 sync1/count16.
Физические LED и wiring не проверены.
[Evidence](evidence/2026-10-14-fan-topology.json).

## Dynamic RGB параметры по mode/source/unit — 2026-10-14

Реестр сохраняет ranges, low/high palettes/speed и fixed brightness/direction
по canonical mode и точной sensor ID/unit. Enable перемещает обновлённую
binding в последнюю позицию; Disable и manual cancellation сохраняют параметры.
GUI разделяет локальные черновики источников, использует native defaults и
semantic CPU Load; поздний default reply не заменяет изменённый контекст.
Legacy optional fields продолжают наследовать manual brightness/direction.

Workspace925/2ignored, browser39, strict clippy/fmt/rustdoc, Svelte0/0/build
прошли. Native WebKitGTK/MockBackend принял Meteor CPU/GPU и Static generic
host RPM, applied readback, Disable/reload и точный registry после restart.
[Evidence](evidence/2026-10-14-dynamic-rgb-profiles.json). Этот checkpoint ещё
не закрывает сохранённый выбор Static/Dynamic и его восстановление при Apply
другого режима; это закрыто отдельным срезом ниже.

## Выбор Static/Dynamic и restore режима — 2026-10-14

`modes[].enabled` сохраняет отдельный выбор каждого режима. Pump Apply с
`dynamic_mode=true` восстанавливает последний сохранённый source; false/omitted
применяет обычную base. Переключение на другой режим сохраняет выбор прежнего.
Explicit Disable оставляет параметры, выключает текущий mode и не выключает
остальные. Restart активирует только сохранённую current policy, не все enabled
profiles. Pump base/active policy/registry обновляются одним settings сообщением.

Reject ненастроенного mode до writes, write failure без замены base, missing
source → degraded/manual fallback → возврат исходного источника проверены actor
тестами. Три manager lifetimes проверяют mode restore и durable Disable.
Browser проверил отдельные mode drafts/choice, reload без implicit Apply,
синхронизацию Disable с родительским редактором и legacy daemon contract.

Workspace927/2ignored, strict clippy/fmt/rustdoc, Svelte0/0/build и43 browser
прошли; после финального notice guard повторно прошли4 focused scenarios.
Native WebKitGTK/MockBackend принял CPU Meteor ↔ generic host RPM Static,
automatic source restore, явный Disable и точный registry/flags после restart.
[Evidence](evidence/2026-10-14-dynamic-rgb-selection.json).

## Не закрыто этим доказательством

- Расширенная native fault/reconnect/accessibility matrix. Основные manual и
  dynamic controls, mode/source/unit parameters, Static/Dynamic restore и
  fan↔pump sync/topology реализованы; полный RGB parity этим не заявляется.
- Физическое покрытие LED: direct tests гарантируют только известный prefix,
  не все физические LED. Реальные цвета, скорость и alert/restore требуют железа.
- USB/firmware recovery и совместный длительный media/cooling/RGB прогон: M1/M2.
- GUI-wide accessibility и итоговый software checkpoint остаются в базовом плане.

Исходный широкий RGB-критерий сохраняется открытым до оставшейся проверки
каталога и зон. Закрыт только явно перечисленный runtime software контракт.
