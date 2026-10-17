# Intel engine utilization и выбор GPU load

2026-10-10. i915 и xe per-engine readings реализованы; реальный PMU не открывался.
[Checkpoint](evidence/2026-10-10-xe-pmu.json) фиксирует источники, tests и hashes.

## Windows baseline

Локальная декомпиляция Windows **2.1.28.0**:
`L-Connect-Service/LConnectService/Utils/HWiNFO/Managers/HWiNFOGPULoadSensorManager.cs`.
`GetValue` выбирает один sensor из пользовательского SensorMapping либо DefaultId
и ограничивает его значение диапазоном 0–100. Кандидаты — usage/utilization
GPU/GPU_CORE, default names — GPU Core Load, затем GPU Utilization.
`HWiNFOWrapper.updateGPULoad` и `SystemInformation` передают это значение далее.
Никакой суммы/max/mean engine counters в этом пути нет.

Поэтому Linux предоставляет явный выбор подписанного engine для роли GpuLoad.
Рекомендуемые настройки **не назначают** Intel engine как автоматическую общую
загрузку GPU. Пользовательский binding сохраняет physical GPU membership.
Это не доказывает равенства отдельного engine underlying HWiNFO aggregate:
такой источник и аппаратное сравнение остаются отдельным открытым требованием.

## xe discovery и PMU ABI

[Исследованный xe_pmu.c](https://github.com/torvalds/linux/blob/3857c2fe5449541e24afc5efdb0f81a8a8f9a3a0/drivers/gpu/drm/xe/xe_pmu.c),
SHA-256 `64c02c2acf2d5e6bdc6ac91bfd6521ed4239fb8821289e354277e31dc5ca785a`.
Точная версия установленных driver/GuC и наличие событий проверяются runtime;
номер kernel не считается доказательством поддержки.

`lconnectd --query-intel-engines` выбирается до runtime/config/HID. Helper
использует [DRM_XE_DEVICE_QUERY_ENGINES](https://github.com/torvalds/linux/blob/v6.17/include/uapi/drm/xe_drm.h)
на проверенном xe render FD: read-only VERSION/ENGINES, bounded two-pass allocation,
character-device/rdev/driver check и повторная проверка physical binding.
Discovery и timeout/reaping используют тот же контракт, что
[VRAM helper](intel-memory-query.md); stdout limit для topology — 128 KiB.
Нет guessed card numbers или перебора всех возможных class/instance.

ENGINES header8, record32: class u16 offset0, instance offset2, GT offset4.
Принимаются hardware classes0–4; VM_BIND и неизвестные классы отклоняются.
Максимум 32 devices / 32 engines на устройство. GT≤15, instance≤255, без
duplicate coordinates; canonical physical path должен соответствовать PCI address.

PMU `xe_<PCI с ':'→'_'>` должен объявлять:

| Поле | Формат |
|---|---|
| event | config:0-11 |
| engine_instance | config:12-19 |
| engine_class | config:20-27 |
| function | config:44-59 |
| gt | config:60-63 |

Event IDs берутся из `events/engine-active-ticks` и `engine-total-ticks`,
а не фиксируются в коде. В исследованном kernel это0x02/0x03. Function=0;
суммирования VF/PF нет. CPU выбирается из online ∩ cpumask, если mask опубликован.

## Семантика измерения и lifecycle

На каждый engine — два persistent `perf_event` counter. Сбор и открытие
происходят в spawn_blocking, вне executor/HID/IPC; query helper ограничен
750 ms + до100 ms reap и не запускается повторно, пока старый child не завершён.
Транспорт helper выделен в общий внутренний QueryProcess; VRAM сохраняет свой
отдельный 64 KiB output limit и прежний parsing contract.

Загрузка = **100 × Δactive_ticks / Δtotal_ticks**. Наносекунды perf используются
только для проверки completeness/skew sampling windows. Первый sample,
zero total delta, permission failure, stale>3s, multiplexing или разные
sampling windows — unavailable; reset/backwards counters и >101% — invalid.
Отдельные FD читаются последовательно: допускается ≤1% skew, значения100–101%
ограничиваются100%. Точность этого допуска на железе пока не подтверждена.
Valid active delta=0 при положительном total означает реальный0%.

Query failure отзывает прошлые значения и закрывает counters. Read failure,
исчезновение engine/GPU, изменение PMU type/config/CPU закрывают старые handles;
новое подключение проходит warmup. В каждом interval используется актуальный
DRM catalog. Отсутствующий источник не заменяется другой GPU.

IDs:

- i915: `intel:<physical>:i915:engine:<config hex>:utilization` — прежний ID.
- xe: `intel:<physical>:xe:gt<N>:class<N>:instance<N>:utilization`.

Source editor относит их к `sysfs:<physical>:<driver>` и сохраняет label engine.
Оба типа доступны для явного выбора; recommended reset оставляет load пустым,
если у этой GPU есть только per-engine источники. Unit — Percent, LCD использует
обычный cache freshness/unavailable формат. Никакого нового «aggregate» датчика нет.

## Проверки и оставшиеся gates

Fixtures: query binary layout, topology bounds/duplicates/misbinding, объявленные
PMU formats/event IDs, GT/class/instance encoding, CPU mask, tick ratio/zero/reset/
stale/multiplex/skew, permission/read failure, PMU type change, исчезновение и
повторный warmup, две GPU, fixed helper argument, cache→LCD formatting.
Отдельно проверен явный i915/xe source mapping, отсутствие автоматического
aggregate и отказ cross-GPU binding. Compile-only C probe проверяет ENGINES ABI.
Реальные counters в этих тестах заменены Fake; production helper не запускался.

Отложено: сравнение PMU с известной нагрузкой render/compute/video, support и
permissions/GuC, hotplug/CPU migration/sleep, idle power (PMU driver держит
runtime-power/GT forcewake refs), cadence/RSS и отсутствие LCD stalls.
Native выбор i915/xe engine + VRAM, значения и restart/readback [проверены](evidence/2026-10-10-native-intel-sources.json).
Physical accuracy остаётся checklist gate.
Underlying HWiNFO aggregate ещё не установлен; per-engine результат не закрывает
этот пробел полной parity.

## Установленная граница SDK, 2026-10-11

Повторная сверка выполнена именно по
`reference-decompiled/2.1.28-2026-09-08`, не по старому соседнему экспорту.
[Hashes и PE evidence](evidence/2026-10-11-intel-load-boundary.json).

Цепочка: `HWiNFOWrapper.readHWiNFOSensorValue` (811) →
`Core/Native/HWiNFO.GetSensorValue` (596–604) → native HWiNFO64.dll ordinal373
(RVA0x0030b82c у исследованного binary). Обновление SDK выполняется через
ordinal561 (RVA0x0030b694). PE export table содержит ordinal entries без имён;
соответствие функций устанавливается по `bindFunction` (526–540).
Wrapper проверяет успешность вызова и `SENSOR_READ_RESULT.IsValid`; manager
выбирает один reading и ограничивает0–100. Экспорт DLL не раскрывает формулу
Intel load; её reverse engineering здесь не выполнен. Сумма/max/mean engines
не становятся эквивалентом от совпадения имени «GPU Core Load».

Для следующей сверки нужны либо восстановленный native SDK путь для
конкретного Intel adapter, либо Windows catalog и сопоставление readings:

- `%ProgramData%/Lian-Li/L-Connect 3/hwinfo-report.xml` — hardware identity;
- `hwinfo-sensors.json` в том же каталоге — sensor/reading locations и IDs;
- выбранный GPULoad `SensorMapping` и отображаемое имя из Settings;
- Intel model/driver/SDK version и controlled render/compute/video workloads
  с timestamped выбранным reading. Начальный catalog не заменяет time series.

Пути подтверждены `LConnectApp.cs:263,310–311`; запись export —
`HWiNFOWrapper.cs:426–453`. В inspected installation tree таких runtime exports
нет; service logs и frontend assets не являются их заменой. Никаких данных
с реальной машины в этой проверке не собиралось. Эти пункты включены в
аппаратный checklist; per-engine mapping остаётся доступным сейчас.


Сверка2026-10-16 повторно подтвердила hashes исходного Windows2.1.28 GUI/service
и выбор одного reading в `HWiNFOGPULoadSensorManager.GetValue`. В предоставленном
`winver/L-Connect 3` runtime exports по-прежнему отсутствуют; это ограниченный
поиск внутри installation tree, не всей Windows-системы. Формула native SDK и
соответствие Linux engines не установлены. [Audit evidence](evidence/2026-10-16-sensor-gap-audit.json).
