# Manual PWM0: явное значение и legacy «не настроено»

Baseline: Windows L-Connect3 2.1.28.0 / Galahad II LCDV2. Срез2026-10-15
исправляет потерю явно принятого0 в существующем Linux manual API. Private
IPC/DeviceManager и group archive paths проверены программно; реальный HID
не открывался. Это подтверждение сохранения intent и wire values, не измерение
поведения помпы/вентиляторов при нулевом сигнале.

## Контракт

Раньше `pwm_percent == 0` всегда пропускался при attach replay и group Apply,
хотя manual setter принимал и отправлял0. После restart такой явный выбор
терялся. Старые default files тоже содержат0, поэтому считать каждый legacy0
явной командой нельзя.

Fan сохраняет `manual_zero=true` только после успешного manual0. При последующем
ненулевом manual значении marker снимается. Missing/false marker не сериализуется:
`{"pwm_percent":0}` и `{"pwm_percent":64}` сохраняют прежний portable JSON shape
и соответствующий group hashing. Старый неоднозначный fan0 остаётся unset; его
исторический смысл не угадывается. Marker с ненулевым duty отклоняется до записи.

У pump уже есть source: `mcu` или `application` означает явную manual preference,
в том числе при0. Empty source +0 остаётся unset. Legacy nonzero сохраняет прежнее
MCU/default-source поведение; известный legacy `Application` распознаётся при
attach replay. Для новых writes используются прежние canonical source strings.

Manager и group Apply используют общие `has_manual_setting` predicates. Explicit0
отправляется как обычный manual command с сохранённым source и остаётся отдельным
от `CoolingPolicy::Fixed0` путём. Активная cooling policy по-прежнему имеет
приоритет над cached manual preference. Setter не превращается в periodic policy.

Group snapshot/LCGROUP1 codec сохраняет marker и source. Commit/Apply/reapply
различают explicit0 и unset0; повтор Apply не создаёт новый receipt или profile
revision. Прежние payloads без marker принимаются с прежним unset semantics.

## Проверка

- Реальный private IPC/DeviceManager/MockBackend: legacy0
  startup без PWM writes → fan0/pump motherboard0 → restart → pump MCU0 →
  restart → fan67 → restart. В каждой независимой history проверены cmd139/138,
  source/duty bytes и reported applied state. Новый ненулевой fan удаляет marker.
- Legacy TOML/portable JSON shape, ненулевой legacy fan и no-preference pump
  сохраняются. Невалидный marker не заменяет предыдущий durable file.
- Actual group codec → owner commit/Apply/reapply: explicit0 даёт два PWM writes,
  unset0 — ни одного. Source/marker и device/profiles bytes после reapply проверены.

Workspace1021 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Evidence](evidence/2026-10-15-manual-pwm-zero.json).

## Отложенная приёмка

- Фактический PWM0/RPM и поведение MCU/motherboard на устройстве, переход к
  ненулевому PWM, hotplug/sleep и совместная нагрузка. Hardware testing отложено
  пользователем; использовать общий [hardware acceptance](lcdv2-hardware-acceptance.md).
- Full-group IPC/CLI export/import/readback/reapply подключены в следующих
  [срезах](profile-groups.md); GUI и Windows ZIP mapping остаются. Этот срез
  закрывает неоднозначность данных перед публичным group transfer.
