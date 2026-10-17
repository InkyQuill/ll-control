# M2 cooling acceptance — partial hardware verification

Only Galahad II LCDV2 0416:7395. M1 display authorization does not establish
working fan/pump PWM. Last M1 observation was pump≈2400 RPM, fan0 RPM; the
fan wiring and working restore duty/source must be established before live PWM.

## Preconditions

One daemon/HID owner; read actual firmware/snapshot, preserve configuration,
use isolated test configuration that replays only agreed settings. Record baseline
PWM/source supplied by the operator and actual RPM/temperature. Keep LocalUI as
the agreed display fallback. Do not treat a saved percentage as a device getter.
No firmware/reset/raw unknown commands are used.

## Cases

| Case | Procedure | Acceptance | Status |
|---|---|---|---|
| M2-01 | `sensors` while normal LCDV2 polling runs | coolant and RPM IDs stable; values agree with snapshot, freshness real | pass: live 11-entry catalog, coolant34.3°C/pump2430RPM/fan0, age620ms |
| M2-02 | Apply fixed policy at known working duty, then a modest higher agreed duty | source/duty state and observed RPM change agree; no cooling loss | not-run |
| M2-03 | Apply curve bound to device coolant, with baseline-safe points and explicit fallback | policy continues without CLI/GUI; curve output matches sensor | not-run |
| M2-04 | Simulated unavailable/stale/wrong-unit source | fallback bypasses hysteresis; wrong-unit persistence is rejected visibly | mock pass; no physical fault injection |
| M2-05 | Apply manual restore value while curve is active | policy cleared, no later curve overwrite, RPM returns to baseline | mock pass; live not-run |
| M2-06 | Restart with policy saved | policy replays without legacy PWM overwrite; missing initial sample uses fallback | mock pass; live not-run |
| M2-07 | Pump motherboard policy (only when wired/authorized) | source1, upper-bound semantics and RPM reflect external PWM | wire mock pass; live not-run |
| M2-08 | With an existing test bank and known safe fixed policy: Load without Apply; restore factory name/policy into draft without Save/Apply; stored Apply; edit/Save bank; restart; then custom Apply/manual restore | Load/Save do not change requested working snapshot; stored ID/policy replay together; edited bank does not replace it; custom/manual clears only its channel ID; actual RPM/source stay consistent with the agreed duty | IPC/MockBackend + native GUI pass; physical not-run |
| M2-09 | With a reviewed Windows FanAndLighting export, known safe restore and explicit source/unit bindings: preview; explicit import; retained Linux slots/Screen/thermal; readback/restart; original-ID retry after newer edits | All8/3 bank modes and IDs match the source; source Dynamic High brightness/direction and chosen bindings are correct; actual duty/RPM/LED state agrees; retry never overwrites newer intent; restore original controls explicitly | CLI/Unix IPC/MockBackend pass on owned DTOs; genuine export/hardware not-run |

## Commands to use after restore settings are supplied

Prepare fixed and curve JSON files using the documented `CoolingPolicy` schema;
record exact selected values and SHA256 before writes. Choose only sensor IDs
reported by `lconnectctl sensors`. Apply with `cooling set fan|pump --policy FILE`,
inspect `cooling state` and `snapshot` each second. Stop on transport/control
errors, stale cooling telemetry or unexpected RPM behavior.

Restore fan via `lconnectctl fan RESTORE_PERCENT`; restore pump via
`lconnectctl pump RESTORE_PERCENT --source RESTORE_SOURCE` (legacy source spelling
mcu/application, where application is motherboard). These manual commands remove
the corresponding test policy atomically with its saved manual setting. Verify
state, actual RPM and temperature, then stop/join daemon. Keep original config
backup intact. Record physical findings separately from write success.


## 2026-09-09 read-only catalog verification

Pavel reports the fans are probably connected to the motherboard. Fan PWM/RPM
acceptance through LCDV2 is therefore deferred; pump restore duty/source is still
unknown. No cooling policy was enabled or manual PWM command issued.

Used the reviewed release build with an empty isolated configuration directory
`/tmp/lcdv2-m2b-readonly-config-20260909`, sensor1Hz and hotplug500ms. There were no
saved device settings to replay. `cooling state` returned null requested/applied
values and no errors for both targets. `sensors` returned 11 valid readings,
including LCDV2 coolant34.3°C, fan0RPM and pump2430RPM at age620ms; host readings
had age157ms. The adjacent legacy snapshot agreed on those device readings.
Firmware version1.6 was read; the legacy firmware RPC returned duplicated PN0
text rather than a date in this run, so no fresh firmware-date claim is made.

Commands: `lconnectctl --json sensors`, `--json snapshot`, `cooling state`,
`--json firmware`; cleanup `lcd clear` and SIGTERM with daemon join. LocalUI was
sent as agreed. Machine evidence: `/tmp/lcdv2-m2b-catalog.json` and
`/tmp/lcdv2-m2b-catalog.log`. Serial-bearing sensor IDs are not reproduced here.
Full physical curve/fallback/RPM acceptance is still not-run.


## RPM follow-up: software evidence only

Pump fixed RPM and Quiet/Performance curves now use the Windows2.1.28 two-stage
conversion. Mock checks cover all29 mapping knots, rounding between knots,
RPM policy IPC/TOML roundtrip and restart with missing sensor→100% configured
fallback instead of stale legacy PWM. CLI preset generation does not connect
to a daemon. Physical target/measured RPM comparison remains not-run; no new
hardware command was issued in this follow-up. Add a bounded RPM-target case
only after the pump restore duty/source is established.


## 2026-10-10 thermal RGB software acceptance

Mock/integration cases now pass for independent rules, latest-trigger priority,
stale warning retention,10-second repeated manual grace, base/scope restore,
partial zone failure/retry, saved rules, IPC/CLI and persistence without alert
color overwriting base. Direct-color overlap is explicitly rejected until
alerts are disabled and restoration completes; full overlap parity is pending.
No physical RGB/PWM/LCD writes were made. Visual threshold/color/restore acceptance
and ongoing LCD playback compatibility remain not-run. Previous live values
above are September evidence, not measurements from this session.


### Direct-color follow-up (same day)

The earlier direct-overlap guard is now superseded by retained-prefix overlay and
restoration. Both zones, short-prefix tail preservation, Application-source entry,
manual grace and partial-write retry have software coverage. `direct_led_count`
reports scope. Whole physical-zone coverage and visible direct/alert transitions
remain not-run; there was no new hardware access.


### Fan preset follow-up, 2026-10-10

Quiet/Standard/High/Full now have source-derived knots and software coverage for
integer Celsius/duty interpolation, fallback and persistence. Mock output uses
existing cmd139 only. This does not verify fan RPM or motherboard-connected
fans; no physical cooling action was taken.

### Fan preset endpoints, 2026-10-17

Source audit при подготовке Windows mapper выявил ошибку нижнего участка:
`RPMPhaseInfo.ToRPMConfigPhases(false)` окружает пять editable knots точками
0°C/MinSpeed и100°C/MaxSpeed. Для LCDV2 Fan MinSpeed=10, MaxSpeed=100.
Теперь все четыре `FanPreset` возвращают семь явных точек, которыми пользуются
CLI, Tauri preset bridge и defaults/reset named slots/full group. Quiet при
10.9°C даёт14%, Standard14%, High24%, Full46%; Windows service сначала truncates
Celsius. Ниже0°C результат10%, выше100°C —100%. Full здесь Windows curve preset,
а явный fixed100% остаётся отдельной политикой.

Существующие сохранённые кривые и custom `fan_curve` автоматически не
переписываются: схема не содержит надёжного признака «старый preset», и
совпадающая кривая могла быть осознанным пользовательским выбором. Для нового
preset intent его нужно заново выбрать и явно Apply; загрузка preset в GUI
сама по себе не отправляет PWM. Pump presets и generic evaluator не менялись.

Regression сначала воспроизвела20% вместо10% ниже первой точки. Затем
проверяются четыре presets, целочисленная интерполяция, actual mock cmd139
14→21, stale fallback/recovery, TOML roundtrip и неизменённые custom endpoints.
CLI executable проверяется с listening fake Unix socket: без connections,
config writes и Apply; JSON пригоден для named Fan slot.
[Команды, hashes и ограничения](evidence/2026-10-17-fan-preset-endpoints.json).
Аппаратная приёмка остаётся открытой; motherboard fans не считаются каналом LCDV2.

## Полный software cooling критерий — 2026-10-11

Это консолидация реализованных сценариев COOL-001–004 для LCDV2.
Windows provenance остаётся в [матрице](lcdv2-2.1.28-matrix.md).
Наличие кода проверено по тестовым assertions и production owner, а не
по названиям тестов. Физические cases M2-02–07 выше от этого не меняют статус.

Все имена ниже относятся к `crates/lconnect-daemon/tests/`:

| Требование | Проверяемое поведение | Доказательство |
|---|---|---|
| Fan Quiet/Standard/High/Full | Все preset knots, clipping за границами, integer Celsius/PWM interpolation; реальный cmd139 и fallback | `cooling.rs::fan_presets_match_reference_knots_truncation_and_limits`, `fan_reference_policy_writes_pwm_fallback_and_retains_serialized_intent` |
| Custom fan/percent curves | Интерполяция, монотонные точки, запрет NaN/неоднозначных входов; fractional knots запрещены для reference fan_curve; TOML roundtrip | `interpolation_rising_response_falling_hysteresis_and_fallback_recovery`, `policies_reject_dangerous_or_ambiguous_inputs`, `fan_reference_curve_rejects_fractional_knots_and_roundtrips` |
| Pump Quiet/Performance и fixed/custom RPM | Все29 RPM→PWM knots, промежуточное округление, two-stage calculation, диапазоны и limits, MCU cmd138, сохранённый RPM intent | `reference_rpm_mapping_and_two_stage_rounding`, `rpm_validation_rejects_fan_out_of_range_and_malformed_curves`, `rpm_policy_uses_existing_mcu_write_path_and_retains_rpm_intent` |
| Hysteresis | Немедленное повышение; удержание при небольшом снижении; снижение на границе; fallback сбрасывает anchor | `interpolation_rising_response_falling_hysteresis_and_fallback_recovery`, `reference_rpm_mapping_and_two_stage_rounding` |
| Missing/stale/recovery | До первого sample и при возрасте4s — configured fallback; чужой ID не подменяет свой; восстановление без Apply | `persisted_curve_uses_fallback_before_first_sample_and_invalid_policy_does_not_write`, `fan_reference_policy_writes_pwm_fallback_and_retains_serialized_intent`; [совместная и native приёмка](sensor-consumer-acceptance.md) |
| Wrong unit / errors | RPM нельзя привязать как temperature; persisted wrong unit виден как ошибка и не пишет PWM; failed HID не даёт ложного applied, intent сохраняется | `persisted_known_wrong_unit_is_disabled_with_visible_error`, `rpm_binding_rejected_and_write_failure_retains_intent_without_applied_claim`; [native transport retry](transport-consumer-acceptance.md) |
| Manual override | Policy отменяется; последующие ticks не возвращают curve; queued manual выигрывает после slow failed write | `curve_writes_fallback_on_stale_data_and_manual_override_cancels_future_ticks`, `queued_manual_wins_after_a_slow_failed_policy_write` |
| Policy persistence/replay | Поля сохраняются; активная policy исключает stale legacy PWM; restart до sensor sample даёт fallback | `cooling_ipc.rs::curve_persistence_roundtrip_preserves_fields_and_legacy_files_default_to_no_policy`, `policy_ipc_manual_override_atomic_persistence_and_restart_do_not_replay_old_curve` |
| Motherboard source и raw replay | Source1/duty80 после restart; MCU67 отменяет policy и переживает следующий restart; fan73 applied восстановлен | `cooling_ipc.rs::motherboard_pump_replays_until_manual_mcu_override_survives_restart`; [исправление owner replay](persistence-acceptance.md#cooling-source-и-ручной-pwm-после-restart--2026-10-11) |
| Работа без GUI и под LCD нагрузкой | Автономный curve tick реагирует на95°C→100%, пока CPU renderer занят; same-stream manual доступен | [Actual CPU render acceptance](theme-preparation-order.md#реальный-cpu-rasterjpeg-и-общий-render-loop-критерий--2026-10-11) |

[Checkpoint этой сверки](evidence/2026-10-11-cooling-acceptance-map.json).
Общий **software** пункт cooling закрыт. GUI accessibility остаётся отдельной
приёмкой: [native pump/RPM numeric controls](native-keyboard-acceptance.md#pump-rpm-native-acceptance--2026-10-11) теперь проверены отдельно; speech остаётся открытым.
Target RPM — вход таблицы преобразования, не обещание измеренного RPM.
Действительное охлаждение, external motherboard PWM, wiring вентиляторов,
права/драйверы sensors и firmware recovery требуют отложенных hardware cases.


## Production GUI cooling и совместный LCD — 2026-10-16

[Evidence](evidence/2026-10-16-production-cooling.json). Actual packaged GUI,
без native-smoke и JS injection; AT-SPI/native keyboard выбирают режим/source,
редактируют PWM/fallback и нажимают Apply. Нет RPC setters. Mock fixture
показывает последний реально записанный cmd138 и число pump writes, отдельно
от requested/applied state. Header и source/duty bytes сверяются независимо.

| Переход | Ожидаемый MockBackend payload `[source, PWM]` |
|---|---|
| Fixed61 | `[0,61]`; до Apply нет pump writes |
| Curve30°C/40% →70°C/100%, synthetic95°C | `[0,100]`; до Apply старая fixed policy |
| Missing / monotonic stale>3s | `[0,87]`, configured fallback, binding сохранён |
| Возврат того же ID | `[0,100]` без Apply |
| Manual fixed61 при stale | `[0,61]`; свежий95°C sample не возвращает curve; штатный fixed refresh остаётся61 |
| Motherboard upper bound55 | `[1,55]`; это не measured duty/RPM |
| Curve, GUI Close, normal fixture restart | `[0,100]`, saved policy replay без Apply |
| Restore fixed61 | `[0,61]`; затем LocalUI и normal Close |

Два native runs прошли по12 cooling состояний каждый: сначала с clock theme,
затем с той же realtime sensor theme, что прошла отдельный production checkpoint.
Каждый cooling state ждёт дальнейшей JPEG передачи; combined run дополнительно
сопоставляет полные cmd14 JPEG с literal text: valid203.0°F и unavailable—.
Всего19 точных JPEG сравнений, включая7 sensor lifecycle состояний и12 cooling
состояний. LCD theme/bindings не меняются; fan policy остаётся null.
Оба restart в combined workflow штатные, не crash/power-loss испытания.

Повторить после сборки mock fixture:
`python3 tools/test-distribution-gui.py target/distribution-20261016-ownerloss --cooling`
или совместно `--sensor-theme --cooling`. Production GUI payload прежний;
новые test-only harness/fixture hashes сохранены отдельно. English workflow,
strict Clippy/fmt,2 fixture tests и4 package tests прошли. Физические M2 cases,
RPM accuracy, fan wiring, media/RGB mixed workload и полная fault/accessibility
matrix остаются открытыми. Никакая реальная cooling policy не включалась.


## Cooling error recovery и критерий refresh — 2026-10-16

[Checkpoint](evidence/2026-10-16-cooling-messages.json) проверяет отказ ответа
после реального commit fixed PWM в MockBackend, сохранённый draft/readback,
реактивный EN/RU context с неизменным raw diagnostic и явный edited retry.
Два native cases дали ровно4 пользовательских `set_cooling_policy` RPC; final
cmd138 source0/PWM64. Poll/runtime/preset errors и local numeric/profile validation
покрыты browser. Расширенная native matrix остаётся.

Cooling owner штатно обновляет даже неизменный PWM после2s. Поэтому отсутствие
нового Apply проверяется числом RPC и сохранением requested policy/source/duty,
а не запретом всех HID writes. Исправлен слишком строгий тестовый счётчик при
редактировании draft/manual override; production12-state regression повторена.
Это не изменение production cooling actor или physical acceptance статусов.
