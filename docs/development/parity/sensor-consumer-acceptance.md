# Missing/stale sensor consumers: software acceptance

2026-10-10. Ниже сопоставлены реакции уже реализованных consumers с текущими
тестами. Использованы синтетические readings/cache и fake HID; это не
измерение реального охлаждения, цвета LED или точности датчиков.

| Consumer | Ожидаемое поведение | Доказательство |
|---|---|---|
| Fan PWM curve | Stale/missing → configured fallback; чужой ID не заменяет binding; возврат своего ID восстанавливает PWM без Apply | `tests/cooling.rs`: `curve_writes_fallback_on_stale_data_and_manual_override_cancels_future_ticks`, `fan_reference_policy_writes_pwm_fallback_and_retains_serialized_intent` |
| Pump curve | До первого sample — fallback; неизвестная единица запрещает policy; missing/recovery не меняет binding | `tests/cooling.rs`: `persisted_curve_uses_fallback_before_first_sample_and_invalid_policy_does_not_write`, `persisted_known_wrong_unit_is_disabled_with_visible_error`; native active-pump checkpoint |
| Thermal RGB | Потеря данных не отменяет активную тревогу; degraded явно виден; свежий cool sample снимает тревогу и возвращает base | `tests/thermal.rs`: `newest_trigger_wins_stale_retains_and_valid_cool_sample_removes`, `partial_failure_retry_stale_recovery_and_direct_restore_failure` |
| Dynamic RGB | Невалидный источник не становится zero; возвращается base, retained binding восстанавливается при появлении того же ID | `tests/dynamic_rgb.rs`: `unavailable_wrong_unit_stale_nonfinite_and_missing_binding_never_become_zero`; `tests/thermal.rs`: `sensor_rgb_thermal_priority_restore_manual_cancel_and_stale_fallback`, `sensor_rgb_replay_retains_binding_and_recovers_when_source_returns` |
| LCD sensor | Missing/stale/invalid/wrong unit/wrong ID рисуются как unavailable; настоящий zero отличается; восстановление без пересоздания theme | `tests/theme_runtime.rs`: `live_sensor_frames_revoke_missing_stale_invalid_and_wrong_binding_without_fake_zero` проверяет cache→готовый JPEG |
| LCD Cycle | Смена label/value/unit на границе interval; missing slot не пропускается, loop сохраняется | `lconnect-image/tests/theme.rs`: `sensor_cycle_switches_on_boundaries_keeps_missing_slot_and_wraps` |

Все пути указаны относительно `crates/lconnect-daemon`, кроме явно указанного
image crate. [Native pump missing/recovery](evidence/2026-10-10-native-active-sensor.json)
покрывает GUI и configured fallback на MockBackend. LCD runtime дополнительно
проверяет обновление от cache без GUI и остановку producer при ручной замене
изображением (`live_theme_updates_and_manual_image_replaces_its_producer`).

Новая LCD регрессия сравнивает unavailable JPEG с кадрами пяти ошибочных
входов; отдельные valid0/valid75 frames различаются. После возврата valid75
получается прежний кадр при неизменном theme/binding. В ней нет clock layer,
поэтому сравнение не зависит от текущего времени суток.

## Native совместная потеря источника, 2026-10-11

[Evidence](evidence/2026-10-11-native-consumer-recovery.json): pump/fan curves,
thermal и dynamic RGB используют один fixture CPU ID. Missing даёт configured
fallback обоим каналам, сохраняет активную тревогу в двух зонах и binding RGB.
GUI показывает thermal ownership, не обещая base lighting при активной тревоге.
Возврат того же ID восстанавливает cooling без Apply, убирает degraded; тревога
остаётся, пока threshold явно не восстановлен. После этого dynamic RGB продолжает
работу. Unsent threshold drafts и LCD theme сохранены. Full native restart passed.

Исправление UI: сообщение degraded учитывает suppressed_by_thermal. Browser
проверка проходит переходы degraded+suppressed → degraded → recovered без writes.
4 RGB browser tests, frontend check/build и native build/smoke прошли.

## Native aging и hot→cool, 2026-10-11

[Evidence](evidence/2026-10-11-native-aged-consumers.json): fixture публикует95°C,
затем останавливает публикацию всего synthetic cache. IPC, actors и stats работают.
По настоящему monotonic age>3000ms чтение становится stale/value:null. Pump/fan
переходят на fallback; тревога остаётся в обеих RGB зонах; dynamic degraded/suppressed.
После свежего40..49°C sample тревога снимается, cooling и dynamic RGB восстанавливаются
без Apply. Requested конфигурации четырёх consumers, LCD theme и GUI drafts сохранены.
Fault markers сняты; обычный fan55 восстановлен; полный native restart прошёл.
Rust fmt/Clippy/doc и587 tests прошли; fixture review без замечаний.

## Native RGB transport failure, 2026-10-11

[Evidence](evidence/2026-10-11-native-rgb-transport.json): private MockBackend
wrapper отклоняет только A-class SetPumpLighting (cmd131) с ShortWrite0.
Во время удерживаемого fault dynamic RGB показывает last_error, binding
сохранён; fan PWM61 применяется без ошибок обоих cooling channels.
Успешные JPEG start packets растут15→18, fan writes9→10, RGB failures0→3.
После снятия fault RGB восстанавливается без Apply; fan55 возвращён, полный
native workflow и restart/replay прошли. Rust fmt/Clippy/doc и587 tests прошли.

Это один направленный сценарий: ошибка pump RGB не останавливает LCD/cooling.
Счётчик начала JPEG не доказывает физический кадр. Ошибки LCD/PWM, direct-color
и partial-frame decoder resync этим тестом не покрыты.

## Итог software consumer matrix

Missing/same-ID, aged-stale/hot→cool и три направления одиночных transport
ошибок сопоставлены и проверены. [Transport acceptance](transport-consumer-acceptance.md)
фиксирует LCD/PWM/RGB recovery и дополнительное component покрытие direct/H264.

Остаются физические PWM/RPM, LED coverage, accuracy и long-run realtime theme,
а также общий disconnect/cancel/preparation lifecycle из базового чеклиста.
Сценарии одновременного отказа нескольких consumers и физический resync
этими isolated tests не подтверждаются. Cycle restart/GUI-close и полный
набор theme controls остаются в общем чеклисте.

[Checkpoint](evidence/2026-10-10-sensor-consumers.json): 587 tests полного Rust
прогона прошли. После уточнения только fan-test timing финальные fmt/Clippy
и affected regression прошли повторно; review без оставшихся замечаний.

## Общий software контракт readings → LCD — 2026-10-11

Граница: типизированные readings поддержанных providers и их consumers.
Это не утверждение, что все Windows voltage sources или Intel aggregate
уже реализованы; соответствующие пункты provider inventory остаются открытыми.

| Критерий | Проверенное доказательство |
|---|---|
| CPU/GPU temperature, CPU load, RAM | `host_sensors.rs`: physical hwmon identity/renumber; failed/fault/disabled; CPU counter deltas без guest double count; memory kB→bytes и malformed/available bounds |
| GPU load/VRAM | AMDGPU missing/recovery/impossible values и NVIDIA query fixtures; [provider inventory](sensor-provider-inventory.md). Intel engine ≠ aggregate, отдельный открытый критерий |
| Coolant/pump/fan RPM | `host_sensors.rs::device_bindings_use_stable_key_and_invalid_coolant_does_not_hide_real_rpm`: stable device key, invalid coolant не скрывает RPM, настоящий fan0 остаётся valid |
| IPC/cache/freshness | `catalog_roundtrip_without_any_usb_device_uses_cache`, `stale_cache_keeps_identity_and_timestamp_but_revokes_value`: cached readings/units, age и timestamp, stale→value:null |
| Binding/selection | `sensor_selection.rs`: physical GPU membership, wrong unit/role, ambiguous sources, order-independent recommendations, явный fan mapping и Intel engine selection |
| Sensor rendering | `theme_runtime.rs::live_sensor_frames_revoke_missing_stale_invalid_and_wrong_binding_without_fake_zero`: missing/invalid/stale/wrong-ID/unit → unavailable; zero отличается от unavailable; recovery без Apply |
| Совместные host/device readings | Новый `mixed_host_and_lcd_sensors_render_units_and_recover_independently`: девять readings, °C/°F/%/W/RPM, независимое устаревание host, invalid/unit/identity ошибки LCD и recovery |
| Автономные consumers и GUI | Предыдущие native missing/stale/recovery выше; [render-loop](theme-preparation-order.md) и [cooling](lcdv2-m2-cooling-acceptance.md) checkpoints |

Новый тест сравнивает JPEG динамической темы с JPEG **literal text** композиции,
не вызывает production sensor formatter для вычисления ожидаемых строк.
Fresh содержит CPU104.0°F/GPU50.0°C/CPU25.0%/GPU60.0%/RAM75.0%/Power65.0W/
Coolant34.0°C/Pump2400.0RPM/Fan0.0RPM (пробелы единиц проверяются в literals).
Host age4s заменяет только шесть host строк на unavailable. Invalid coolant,
pump с Percent unit и чужой fan ID заменяют только три LCD строки. Возврат
исходных readings даёт исходный JPEG; composition/bindings не меняются.

[Evidence](evidence/2026-10-11-mixed-sensor-rendering.json).
Общий software контракт датчиков закрыт совместно с указанными provider/IPC/
identity regressions. Hardware accuracy/wiring, Windows Intel aggregate,
coverage voltage вне поддержанных платформ, права реальных drivers и
оставшиеся GUI/typography gates не закрываются этой консолидацией.

## Production GUI → sensor theme → mock HID, 2026-10-16

[Evidence](evidence/2026-10-16-production-sensor-theme.json). Actual packaged
production GUI (custom-protocol, no native-smoke/JS injection) управляется через
AT-SPI/native keyboard. Удалены оба default layers, выбран точный Fixture CPU,
изменены name/label и Fahrenheit, выполнены read-only preview и один явный Apply.
Это English workflow; предыдущие RU/debug checkpoints сохраняют собственные границы.

Opt-in mock fixture собирает только полные contiguous cmd14 B-packets и сохраняет
атомарный receipt sequence+JPEG. Expected preview использует literal text,
не production sensor formatter; clock отсутствует. Все7 transmitted JPEG480×480
совпадают с ожидаемыми:95°C→203.0°F; missing→—; return→203.0°F;
реальный monotonic age>3s→—; fresh→203.0°F; после GUI Close producer продолжает
отправлять новые кадры; restart fixture восстанавливает theme/binding и тот же
кадр без Apply. Повторно открытый GUI загружает сохранённую тему и возвращает
LocalUI. Saved theme неизменна на каждом переходе. Clock production regression
также повторена; fixture tests и strict Clippy/fmt прошли.

Повторяемая команда после сборки `desktop-mock`:
`python3 tools/test-distribution-gui.py target/distribution-20261016-ownerloss --sensor-theme`.
Новые helper/fixture источники записаны отдельно: сам production payload не
пересобирался, application sources совпадают с его manifest. Это не real-provider
accuracy, физический LCD, Cycle matrix или одновременно несколько consumers.


[Production combined cooling checkpoint](evidence/2026-10-16-production-cooling.json)
adds12 pump states beside the same sensor theme, with exact JPEG comparisons at
every state. Missing/stale drives fallback87 and unavailable LCD; recovery returns
curve100 and203.0°F. Manual fixed61 leaves the LCD stale display independent, then
fresh data updates the LCD without reactivating the old pump curve. Motherboard
source, GUI Close and normal fixture restart preserve the LCD theme. Fan remains
unconfigured. These are synthetic source/MockBackend results, not measured duty,
RPM or a real display test.
