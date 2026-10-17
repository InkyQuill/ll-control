# RGB-004: sensor-driven screen/pump lighting

Baseline Windows **2.1.28.0**, Galahad2Vision/LCDV2. Исследовано 2026-10-10
по локальной декомпиляции `reference-decompiled/2.1.28-2026-09-08`.
Это обычный dynamic mode, отдельный от global thermal alerts.

## Source-derived поведение

| Источник | Наблюдение |
|---|---|
| Service `Products/Galahad2Vision/Galahad2VisionController.cs:192,451–465` | Timer 1s, immediate start; только не-direct mode, dynamic enabled, MCU и не suspended |
| Там же `:499–514` | CPU/GPU load/temperature, PumpRPM, CoolantTemp; cast в int, constrain min/max |
| Там же `:515–545` | До 4 цветов, high branch проверяется первым, затем low, затем interpolation; speed сначала int percent, потом `/25` в wire |
| Там же `:548–568` | Интерполяция в HSV, компоненты HSV целочисленные; обычная линейная hue-интерполяция, не shortest hue arc |
| Core `HSVColor.cs:70–158` | RGB→HSV использует floor; Hue setter ограничивает 0–360. Отрицательный остаток hue clamp в 0, не wrap; HSV→RGB truncates bytes |
| Service `:571–620` | Mode выбран независимо; dynamic brightness/direction берутся из High, не интерполируются; прямой RGB отключает dynamic writes |
| UI `Products/Galahad2Vision/Galahad2VisionProfile.cs:240–275,566–659` | Sensor ranges и mode-specific controls; colors count/speed/brightness/direction определяются режимом |

Celsius и load: range0–100, defaults low30/high60. Pump RPM: range0–3600,
low800/high1200. Equal low/high допустимы в reference: при >=high выбирается
High, ниже Low; промежуточная ветка не делит на ноль. Endpoint RGB берутся
без HSV round-trip. Windows подставляет 0 при missing source; Linux намеренно
возвращает unavailable/degraded и должен показывать известную обычную base,
не выдавая missing reading за реальную низкую температуру.

## Реализовано

`crates/lconnect-ipc/src/dynamic_rgb.rs`: strict configuration, explicit
sensor ID/unit, thresholds, matching 0–4 low/high colors, optional pair of
speed percentages, requested/applied/degraded state contract.
`crates/lconnect-daemon/src/actor/dynamic_rgb.rs`: pure evaluator с freshness
3s, finite value validation, integer input/clamping, reference HSV/color/speed
arithmetic. Никаких timers, settings или HID writes в вычислителе нет.

Четыре focused regression tests прошли: HSV midpoint, fractional sensor input,
equal thresholds, четыре цвета и negative-hue reference quirk; stale/missing/
wrong-unit/nonfinite input; malformed config. Strict daemon/IPC Clippy прошёл.

## Runtime и RPC

`lighting_keeper` — единственный владелец RGB. Dynamic policy меняет до четырёх
color fields и speed выбранного pump mode. Brightness/direction могут быть
фиксированными параметрами источника; при отсутствии этих полей наследуются
из обычной base. Scope/disabled остаются параметрами обычной base. Color count и наличие speed проверяются
по Windows mode catalog. State сообщает base_mode/color_count/speed_supported
для клиентского редактора. Обычный refresh tick не реже 1s; при настроенной
более частой reissue cadence сохраняется её частота.

Thermal overlay имеет приоритет; восстановление использует актуальный sensor
sample. Manual pump/direct-screen после успешной записи очищают dynamic policy
и её сохранённый active intent, сохраняя параметры источников; fan edit не
очищает pump policy. Thermal manual grace
10s сохраняется. Stale/missing input возвращает обычную base и degraded; ошибки
записи очищают applied и повторяются на следующем tick. Настройки режима не
перезаписываются вычисленным цветом на каждом кадре.

DeviceSettings сохраняет `dynamic_rgb`; actor tree восстанавливает policy при
старте. Missing binding не заменяется другой строкой; intent остаётся и
восстанавливает output, когда возвращается тот же sensor ID. Некорректный
persisted config виден как error/degraded и не применяется.

RPC: `get_dynamic_rgb {dev}` и `set_dynamic_rgb {dev,config}`. `config:null`
явно отключает политику; отсутствие поля config — ошибка. Для enable нужны
известная pump mode base и известный sensor с соответствующей unit.
Возвращаемый state различает requested/applied/degraded/suppressed/error;
успешный ответ может содержать ошибку HID, которую клиент обязан показать.

Проверки mock actor подтверждают packets low/mid/high, все четыре color slots,
speed percent→wire, stale fallback, thermal priority/restore, fan edit,
pump/direct cancellation, retry, persisted intent и startup binding recovery.
JSON-RPC integration проверяет getter/disable и rejection отсутствующего config.
Workspace tests/Clippy/docs прошли; последние state metadata дополнения проверены
focused thermal/theme IPC tests и strict daemon/IPC Clippy. Hardware не запускался.

## CLI и GUI

CLI: `lconnectctl dynamic-rgb state --device ID`,
`dynamic-rgb set --device ID --config policy.json`,
`dynamic-rgb disable --device ID`. Читаемый вывод по умолчанию, глобальный
`--json` для automation. JSON ограничен 64 KiB и валидируется до IPC;
ошибка применения даёт ненулевой exit code даже при сохранённом requested intent.

В Lighting раскройте «Подсветка помпы по датчику», загрузите текущий обычный
режим, выберите sensor/thresholds/colors/speed и примените. Редактор учитывает
color count и speed выбранного режима. Polling не перезаписывает draft;
при смене устройства/режима требуется явная загрузка. Цвета читаются по r/g/b,
независимо от порядка JSON keys. Старый poll response не заменяет результат
мутации. UI показывает degraded, thermal suppression и ошибку записи.

Checkpoint 2026-10-10:
- CLI real Unix socket tests: set/state/disable, explicit null, failed write exit,
  malformed/oversized config до подключения.
- Headless GUI: цветовые каналы, draft во время polling, смена устройства,
  invalid thresholds без RPC, error/thermal suppression и disable.
- JSON-RPC + MockBackend: enable → update → shutdown → replay → disable →
  shutdown → disabled replay; settings проверяются после каждого shutdown.
- CLI suite, strict CLI/daemon Clippy, fmt, Svelte check (0 errors/warnings),
  все 10 headless GUI tests и frontend production build прошли.

Позднее native GUI→Unix IPC → MockBackend enable/disable/restart/applied
проверен в [пятистраничном smoke](native-desktop-smoke.md). Полный набор
mode-specific controls и source profiles закрыты checkpoints ниже; native
error/reconnect и физический RGB/LCD run остаются в итоговом checklist.

## Параметры по mode/source/unit — 2026-10-14

`dynamic_rgb_profiles.modes[].bindings[]` хранит отдельные настройки каждого
canonical pump mode и точной пары sensor ID/unit. Последняя binding — последний
применённый источник. Enable обновляет только выбранную binding; Disable и
обычная ручная настройка не удаляют параметры. Ограничения:16 режимов,
64 источника на режим,256KiB JSON; неверная замена отклоняется целиком.

`brightness_percent` и `direction` — optional fixed High settings источника,
без интерполяции. Старые конфигурации без них наследуют manual base. Ошибка
binding/stale возвращает всю обычную base, включая brightness/direction.
GUI удерживает неприменённые черновики отдельно по mode/ID/unit и явно
загружает сохранённый источник. Новые настройки используют native palettes,
speed75 (Runway50), brightness100, Celsius/percent30…60 и RPM800…1200.
CPU Load берётся только из настроенного ID либо однозначного semantic mapping;
поздний ответ после смены источника/режима не заменяет черновик.

Workspace925 passed/2ignored, browser39, strict clippy/fmt/rustdoc, Svelte0/0 и
build прошли. Native WebKitGTK/Unix IPC/MockBackend принял CPU Celsius/GPU
percent Meteor и generic host fan RPM Static с applied/nondegraded readback,
сохранением трёх bindings после restart и Disable. Это программная проверка;
физический pump RPM этим сценарием не проверен.
[Evidence параметров](evidence/2026-10-14-dynamic-rgb-profiles.json).
Выбор Static/Dynamic по каждому режиму закрыт отдельным checkpoint ниже.

## Выбор Static/Dynamic по режимам — 2026-10-14

Windows UI `Galahad2VisionScreenLEDPage.cs:146–171,381–405` меняет и читает
`GetCurrentScreenLEDProfile().IsDynamicMode`; Core `ScreenLEDLightingSetting`
задаёт default false. Linux хранит этот выбор как `modes[].enabled` отдельно от
активной policy. Переход с A на B сохраняет выбор A. Disable выключает текущий
режим, сохраняя его параметры и выбор остальных режимов.

`LightingConfig.dynamic_mode=true` в `set_lighting` явно восстанавливает
последний сохранённый source target pump mode. False/omitted применяет обычную
base и выключает policy этого режима; legacy manual contract сохраняется.
Ненастроенный режим или известная неверная unit отклоняются до HID writes.
Отсутствующий источник допускается при восстановлении: intent сохраняется,
состояние degraded, выводится manual fallback до возврата исходного ID/unit.

`LightingPumpDynamic` атомарно сохраняет manual base, active policy и registry
через settings owner. Ошибка первой записи не меняет committed mode. Ошибка
последующей dynamic записи видна клиенту и допускает runtime retry; successful
base не объявляется отменённой. Direct color остаётся transient; отмена active
policy сохраняется только при фактическом изменении intent/выбора.

`get_lighting_info.dynamic_mode_supported` включает GUI select; base/profiles
возвращают choice в `dynamic_mode`. Применение и загрузка явные, черновики
разделены по режимам. Enable/Disable в редакторе датчика обновляет соответствующий
выбор обычного редактора; смена device/context защищена существующими guards.
Old daemon скрывает select и получает прежний manual request без нового поля.

Workspace927 passed/2ignored, strict clippy/fmt/rustdoc, Svelte0/0/build,
43 browser scenarios (и4 focused повторно после notice guard) прошли.
Native WebKitGTK/Unix IPC/MockBackend проверил Meteor CPU → Static host RPM →
Meteor с автоматическим restore, Disable, возврат и restart: Meteor false,
Static true, три bindings сохранены. Физический pump RPM не проверялся.
[Evidence выбора режима](evidence/2026-10-14-dynamic-rgb-selection.json).
Остаются expanded native faults/accessibility и физическая совместная приёмка.

## GUI diagnostics and temperature grid — 2026-10-17

External read/poll/apply/output errors сохраняются дословно; local validation
resources переводятся при render. Legacy restore-bases hint не скрывает raw cause
и ограничен thermal action. [Error ownership evidence](evidence/2026-10-17-rgb-messages.json).

Пороги — `u16`: Celsius/load0…100, RPM0…3600. Temperature unit presentation не
меняет этот контракт. В°F сетка Celsius порогов32…212 с шагом1,8; EN/RU help
связан с обоими fields.100°F остаётся draft37.77777778°C и не записывается;
87.8/143.6°F отправляются как31/62°C. Display и ARIA используют одно представление.
74 affected browser cases, Svelte0/0/static959/959, native RU Static/IPC и broad
controls/profiles/sensor/restart прошли. [Temperature evidence](evidence/2026-10-17-temperature-grid.json).
Срез включён в [пакет collections](evidence/2026-10-17-collections-integrated-candidate.json); full native
keyboard/speech/Wayland/scaling и physical RGB приёмка остаются открытыми.
