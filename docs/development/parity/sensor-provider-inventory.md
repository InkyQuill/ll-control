# LCDV2 sensor inventory

Baseline: Windows L-Connect3 2.1.28.0. Текущая сводка обновлена2026-10-16; датированные секции ниже сохраняют историю.
Reference root: соседний `reference-decompiled/2.1.28-2026-09-08`.
Это source inventory и программное покрытие, не измерения реального оборудования.

## Применимость Windows

- Core `Types/WMSQ/WMSQScreenSensorType.cs`: CPU/GPU temperature/load,
  PumpRPM, CoolantTemp (и Unknown).
- UI `Products/Galahad2Vision/Private/TextSliderSensorBase.cs:10–21`:
  те же шесть источников для сменяемого текста, default LoopSecond=10.
- UI `Galahad2VisionScreenSettingPage_Transform.cs:94–111`: те же шесть
  для dynamic visual effects.
- Service `Products/Galahad2Vision/Galahad2VisionController.cs:499–514`:
  те же шесть для RGB; `:990`: default screen Sensors.
- Service `:1203–1236`: screen sensor update дополнительно передаёт
  RAMLoad и CPUPower. CPUPower используется Steampunk; RAMLoad сериализуется как RAM и используется
  Robot/Radio. Product UI callers и browser consumers подтверждены ниже.
- Core `SensorSettingTypes.cs` и `SystemResource.cs` также содержат clock,
  power, voltage, VRAM и другие общесистемные поля. Наличие поля в общем enum
  не доказывает его доступность в LCDV2 layout. Service
  `SystemInformation.cs:220–225` вычисляет VRAM load как used/size.

## Linux coverage

| Источник | Provider / unit | Состояние |
|---|---|---|
| CPU core voltage | asusec in0; opt-in ASUS WMI exact CPU Core Voltage; NCT6779/6791/6792/6793/6795/6796/6797/6798/6799 in0 CPUVC; zenpower in1 с exact SVI2 Core label / Volts | cache→IPC→LCD и identity/selection проверены fixtures; точность на платах и другие drivers остаются |
| GPU voltage | AMDGPU in0/in1, Intel i915 in0/xe in1, nouveau in0; optional NV-CONTROL / Volts | Driver-specific rails и units; nouveau core driver-reported, не гарантия analog measurement. Proprietary NVIDIA вне X остаётся открытым; [ABI и ограничения](nvidia-voltage-abi.md) |
| CPU temperature | hwmon k10temp/coretemp/zenpower / Celsius | Реализован; explicit channel ID |
| GPU temperature | hwmon amdgpu/nouveau/i915/xe / Celsius | Реализован при наличии driver channel; NVIDIA через bounded nvidia-smi query |
| CPU load | proc/stat deltas / Percent | Реализован, warmup/reset unavailable |
| GPU load | AMDGPU gpu_busy_percent / Percent | Реализован и fixture-tested; NVIDIA реализован; i915/xe engine readings и явный выбор реализованы; Windows-equivalent общий Intel load остаётся; [точная SDK boundary](intel-engine-utilization.md#установленная-граница-sdk-2026-10-11) |
| Pump RPM | LCDV2 sensor response / Rpm | Реализован, cache freshness |
| Coolant | LCDV2 response / Celsius | Реализован, temp_valid учитывается |
| RAM load/used | proc/meminfo / Percent, Bytes | Реализован через total−available |
| CPU power | RAPL package energy delta / Watts | Реализован; driver/permissions зависят от хоста, Windows consumer Steampunk подтверждён |
| GPU power | GPU hwmon powerN_input/powerN_average / Watts | Реализован при наличии атрибутов amdgpu/nouveau/i915/xe; NVIDIA proprietary power.draw также реализован; hardware accuracy не проверена |
| VRAM used/total/load | AMDGPU sysfs, NVIDIA query, Intel DRM query / Bytes, Percent | Реализованы; Intel i915/xe dedicated regions, неоднозначный zero usage unavailable; [контракт](intel-memory-query.md) |

## Уточнение открытых gaps — 2026-10-16

[Source audit](evidence/2026-10-16-sensor-gap-audit.json) не добавляет providers,
а устанавливает проверяемые границы следующих изменений:

- Proprietary NVIDIA headless voltage: `nvidia-smi -q` graphics voltage уже
  deprecated/removed по official documentation; это не готовый fallback.
  [Подробности](nvidia-voltage-abi.md#проверка-headless-fallback--2026-10-16).
- Intel aggregate: hashes исходных Windows2.1.28 GUI/service совпали с baseline;
  GPU load manager выбирает один HWiNFO reading, не вычисляя engine aggregate.
  В предоставленном installation tree нет `hwinfo-report.xml`/`hwinfo-sensors.json`.
  [SDK/runtime prerequisites](intel-engine-utilization.md#установленная-граница-sdk-2026-10-11)
  остаются обязательными до добавления общего Linux load.
- CPU voltage: pinned upstream nct6683 table имеет generic VIN/supply labels,
  не гарантированный CPU rail; atk0110 names приходят из firmware. Нельзя
  расширять CPU role по номеру in0 или похожей label без доказанного mapping.

Достоверный новый provider требует physical identity/rail/units, failure/freshness
contract и cache→IPC→LCD acceptance; real accuracy остаётся в hardware checklist.
Исходники приложения не менялись: все719 inputs текущего readiness package совпали,
пересборка пакета из-за этой документальной сверки не требуется. Gates остаются
открытыми; неподтверждённый источник не подменяется числом.

## AMDGPU

Provider обнаруживает `amdgpu` через существующий hwmon catalog и читает
`device/gpu_busy_percent`, `device/mem_info_vram_used`,
`device/mem_info_vram_total`. Единицы подтверждены
[документацией kernel для load](https://docs.kernel.org/gpu/amdgpu/thermal.html)
и [VRAM](https://docs.kernel.org/gpu/amdgpu/driver-misc.html).

ID: `amdgpu:<hex canonical device path>:utilization|vram_used|vram_total|vram_utilization|power_draw|graphics_clock|memory_clock`.
Идентичность привязана к топологии хоста, не номеру hwmonN; перемещение карты
в другой PCI slot может изменить ID. Label содержит имя физического device,
чтобы различать несколько GPU.

Load ограничен 0–100. VRAM — unsigned bytes, used>total даёт invalid.
Нулевой total не даёт процента загрузки; отсутствие отдельного атрибута
возвращает unavailable. Реальные load=0 и used=0 допустимы. Исчезнувший
device исчезает из каталога; потребитель сохраняет binding и считает его
недоступным, не подбирает другую GPU. Общий catalog limit и freshness
сохраняются. Все операции read-only вне HID queue.

Проверены fixtures: значения/единицы, несколько карт, duplicate identity,
hwmon renumbering, malformed/missing fields, recovery/disappearance.
14 host sensor tests и полный daemon suite прошли, strict daemon Clippy,
warning-free docs и fmt прошли. Реальные GPU не опрашивались.

## Осталось

1. Проверить NVIDIA provider на реальном драйвере позднее; программная реализация и fixtures завершены.
2. Intel соответствие общего load Windows source. [xe/i915 per-engine](intel-engine-utilization.md)
   и ручной выбор реализованы; dedicated VRAM query i915/xe также реализован, с ограничением
   нулевого/redacted accounting: [подробности](intel-memory-query.md).
3. Resource/Settings source inventory уже завершён; Mini/FPS остаются отдельными
   desktop workflows. [Spec inventory](system-spec.md) реализован; его статические
   характеристики не являются realtime readings. См. [inventory](system-info-inventory.md).
4. Оставшееся coverage CPU voltage вне asusec/opt-in WMI/поддержанных NCT CPUVC/zenpower SVI2 Core и proprietary NVIDIA вне доступной X-сессии;
   реальные значения/частота/ошибки drivers — позже в hardware checklist.

## NVIDIA provider

Daemon collector запускает `/usr/bin/nvidia-smi` без shell, с фиксированными
read-only аргументами:
`--query-gpu=uuid,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw,clocks.current.graphics,clocks.current.memory`
и `--format=csv,noheader,nounits`, locale C.
[Официальный query/UUID contract](https://docs.nvidia.com/deploy/nvidia-smi/index.html).
NVIDIA рекомендует NVML для совместимости между версиями; здесь процесс
выбран для изоляции зависшего driver query и возможности kill/reap. Изменение
CSV layout отклоняется как invalid, а не превращается в неверные readings.

Deadline чтения750ms, stdout64KiB, максимум32GPU по восемь readings. Stderr
не накапливается; failed/timeout/cancelled child получает kill, reap ждёт не
более100ms. Дескриптор остаётся у provider до подтверждённого завершения:
следующий запрос возвращает unavailable и не запускает новый процесс, пока
прежний не reap-нут. Отмена collection также сохраняет этот запрет overlap.
Host sysfs catalog остаётся ограничен 256 строками, NVIDIA добавляет до 256
строк плюс discovery error. Общая monotonic freshness учитывает время query;
опрос идёт через 1s после завершения предыдущего сбора, не гарантированно 1Hz.

ID `nvidia:GPU-<UUID>:temperature|utilization|vram_used|vram_total|vram_utilization|power_draw|graphics_clock|memory_clock`.
CSV enumeration index не используется. Memory MiB преобразуются в Bytes;
temperature — Celsius, load — Percent, power_draw — Watts. N/A/Not Supported → unavailable,
nonfinite/out-of-range/contradictory memory → invalid.
Ошибка всего запроса сохраняет последние известные IDs без значений плюс
`nvidia:discovery`. Успешное пустое перечисление удаляет исчезнувшие GPU;
consumer не переназначает binding. При отсутствии executable показывает
discovery unavailable; CPU/hwmon продолжают обновляться.

`LinuxSensors::new` оставляет NVIDIA выключенным для изолированных filesystem
fixtures. Production `Default` включает его в async `sensors::spawn`;
синхронный `collect()` и существующий host-sensors example собирают sysfs/procfs.

Проверки: CSV values/units/N/A/invalid, multi-GPU reorder/disappearance,
duplicate UUID, oversized output, failure/recovery, timeout и отсутствие
оставшегося child, collector→cache→shutdown. Полный daemon suite, затем
27 library tests после дополнительных regressions, strict daemon Clippy,
fmt и warning-free docs прошли. Реальные GPU/HID не использовались.


## CPU package power

Production collector читает package zones `/sys/class/powercap/intel-rapl:N`
с `name=package-N`. Только whole-package domains, без суммирования
перекрывающихся core/uncore/DRAM и без MMIO-дубликатов.
ID `powercap:rapl:package-N:power` сохраняется при перенумерации zone path.
Число package — до 32, discovery — до 256 записей; ambiguous identity
отклоняется. Hardware replacement может изменить смысл package-N.

Из `energy_uj` вычисляются средние Watts: delta microjoules / 1e6 / monotonic
seconds. Это средняя мощность за интервал, не instantaneous power.
Первое чтение, интервал >3s, сброс/переполнение/изменение range, недоступный
counter не публикуют значение. Reset и wrap по двум точкам неразличимы,
поэтому один такой интервал пропускается; следующий нормальный delta
восстанавливает чтение. Никаких writes/reset/power limits/permission changes.
[Kernel ABI](https://docs.kernel.org/power/powercap/powercap.html).

Registry/IPC/theme поддерживают `watts`, LCD показывает ` W` с выбранной
точностью. °F conversion не применяется. Мощность доступна для sensor layers,
но исключена из dynamic RGB selector и Celsius-only cooling/thermal policies.
На хосте без RAPL или read permission power недоступен. Provider не обещает
эквивалентность всех vendor-specific CPU power estimates Windows.

Проверены energy→W, warmup, zero power, malformed/missing counter, reset,
recovery, длинный интервал, duplicate package, zone renumbering и disappearance;
theme formatting/unit round-trip/stale и RGB selector exclusion.
Workspace tests/strict Clippy/docs, Svelte check, 10 GUI tests и frontend build
прошли; дополнительные power/theme/UI regressions также прошли.
Аппаратный RAPL не опрашивался.

## Закрытая сверка LCD sensor payload — 2026-10-10

Проверена цепочка product UI → service → активный browser bundle Windows
2.1.28.0. Это source-derived evidence; не физический прогон.

| Payload key | Product consumer | Linux type/provider |
|---|---|---|
| CPUTemperature | Text sensor layouts, effects, RGB | Celsius / hwmon |
| GPUTemperature | Text sensor layouts, effects, RGB | Celsius / hwmon, NVIDIA |
| CPULoad | Text sensor layouts, effects, RGB | Percent / proc stat |
| GPULoad | Text sensor layouts, effects, RGB | Percent / AMDGPU, NVIDIA; Intel explicit engine source, aggregate open |
| PumpRPM | Text sensor layouts, effects, RGB | Rpm / LCDV2 |
| CoolantTemp | Text sensor layouts, effects, RGB | Celsius / LCDV2 |
| RAM | RobotShowMode, RadioShowMode | Percent / proc meminfo |
| CPUPower | SteampunkShowMode | Watts / package RAPL when available |

Service `Galahad2VisionController.cs:45–115` defines all eight payload fields;
`RAMLoad` has JsonPropertyName("RAM"). `:1200–1237` fills them and invokes
`setSensorValue`; interval at `:210` is **2 seconds**. Browser clock timers are
separate and do not establish sensor sampling frequency. Linux's independent
collector and 1 Hz theme render are not intended to duplicate that interval.

UI `Galahad2VisionScreenPage.cs:178,181` selects Robot/Steampunk pages;
`Galahad2VisionScreenSettingPage_StyleExpress_Robot.cs:63` and corresponding
Steampunk `:63` emit the effect names. Radio page `:99` emits RamColors.
Core `Types/WMSQ/WMSQScreenSensorType.cs` and product `Private/TextSliderSensorBase.cs`
limit the ordinary selector to the six non-RAM/non-power fields above.

Active `Assets/ga2v/js/app.8fc94db7.js` (line 1), SHA-256 verified again:
`f3691c15dbe62d1248abee444dc94c82a34c46e45697bda4ccad77825c6e878a`.
Consumers RobotShowMode and RadioShowMode destructure RAM; SteampunkShowMode
reads CPUPower and renders its value/unit. This confirms applicability beyond
merely finding a field in an enum or sample JSON. These animated effects
remain unimplemented M4 work even though their data types are supported.

The service payload has no clock-rate, voltage, GPU power or VRAM fields.
Thus these are not missing LCDV2 screen bindings in this baseline. They remain
separate desktop dashboard requirements to audit, not globally excluded features.
Windows effects turn some zero/non-positive values into “--”; Linux deliberately
keeps valid zero separate from unavailable. RAPL package Watts do not promise
identical vendor power estimates to HWiNFO on every CPU.

## Intel provider: установленный ABI и остающаяся реализация

Source audit 2026-10-10. [Pinned revision и SHA-256 файлов](evidence/2026-10-10-intel-kernel-sources.json)
фиксируют исследованный kernel source; наличие интерфейса в нём не доказывает
его поддержку установленным kernel/driver. На реальном GPU ничего не запускалось.

| Данные | Источник | Контракт реализации |
|---|---|---|
| i915 engine utilization | PMU `*-busy` counters | Delta busy nanoseconds относительно времени измерения; отдельно каждый engine |
| xe engine utilization | PMU engine-active-ticks / engine-total-ticks | Delta active / delta total; не делить ticks на host nanoseconds |
| i915 dedicated memory | MEMORY_REGIONS query | DEVICE regions: total из probed_size; used из total−unallocated только при достоверном учёте |
| xe dedicated memory | MEM_REGIONS query | Только VRAM regions; total_size и used, без SYSMEM в VRAM denominator |

Источники: [i915 PMU](https://github.com/torvalds/linux/blob/3857c2fe5449541e24afc5efdb0f81a8a8f9a3a0/drivers/gpu/drm/i915/i915_pmu.c),
[xe PMU](https://github.com/torvalds/linux/blob/3857c2fe5449541e24afc5efdb0f81a8a8f9a3a0/drivers/gpu/drm/xe/xe_pmu.c),
[i915 query UAPI](https://github.com/torvalds/linux/blob/3857c2fe5449541e24afc5efdb0f81a8a8f9a3a0/include/uapi/drm/i915_drm.h),
[xe query UAPI](https://github.com/torvalds/linux/blob/3857c2fe5449541e24afc5efdb0f81a8a8f9a3a0/include/uapi/drm/xe_drm.h).

i915 документирует, что unallocated_size без CAP_PERFMON/CAP_SYS_ADMIN (или
на старом kernel) может равняться total независимо от реального расхода.
Следовательно, успешный query сам по себе не разрешает публиковать used=0.
Без подтверждения достоверного accounting used/load должны быть unavailable;
total можно показать отдельно. xe также скрывает used без этих прав. Не менять системные permissions автоматически.
Текущий [memory provider](intel-memory-query.md) реализует эти ограничения;
при полностью нулевом расходе usage/load остаются unavailable.

i915 интегрированный PMU называется `i915`, дискретный включает PCI address;
xe PMU включает PCI address. Привязку нельзя строить по порядковому cardN.
Для legacy `i915` требуется однозначное сопоставление с физическим устройством;
ambiguity должна отклоняться. Engine identity включает driver/device/GT/class/
instance, без подмены другой GPU после hotplug.

[DRM fdinfo](https://docs.kernel.org/gpu/drm-usage-stats.html) описывает работу и
память отдельного клиента. Shared/duplicated descriptors требуют deduplication,
shared buffers и inaccessible processes не дают достоверной суммы всей карты.
Поэтому сканирование доступных /proc/*/fdinfo не выбрано как источник глобальной
GPU load/VRAM. Частоту GPU также нельзя использовать вместо utilization.

Остающиеся implementation шаги в M2:

1. Read-only discovery DRM/PMU с ограничениями числа устройств/engines и размера
   ответов. Проверять поддержку events/format/query в runtime, а не по номеру kernel.
2. Изолированный сбор PMU и memory queries вне HID и IPC; bounded shutdown,
   отсутствие оставшихся helper processes/FD после timeout/reconnect.
3. Публиковать engine-specific readings. Правило общего GPU load ещё нужно
   согласовать с Windows provider semantics: сумма параллельных engines может
   превышать 100%, а max/mean без оговорки не доказывают Windows equivalence.
4. Fixtures: multi-GPU reorder, legacy ambiguity, malformed query size/region,
   permission denied, absent PMU, zero denominator, counter reset/warmup,
   disappearance/recovery и stale; iGPU без dedicated VRAM не получает fake total.
5. Сквозной registry→IPC→theme regression и отдельная последующая аппаратная
   сверка. Этот audit не закрывает Intel provider implementation.

## i915 engine provider — реализация 2026-10-10

`daemon/src/sensors/intel.rs` подключён к production LinuxSensors; fixture
constructor не включает PMU. Collection выполняется в существующем blocking
collector вне HID queue. Используется безопасный API
[perf-event 0.4.9](https://docs.rs/perf-event/0.4.9/perf_event/struct.Counter.html);
`forbid(unsafe_code)` в daemon сохранён. Новых helper processes нет.

Discovery: `/sys/class/drm/card*/device`, canonical physical path, driver i915;
PCI-qualified PMU либо однозначный legacy i915. CPU выбирается из пересечения
online и PMU cpumask, если такой атрибут существует. Изменение CPU/type/config
пересоздаёт counter и требует warmup. Доступ запрещён или событие не поддержано
→ unavailable; provider не меняет privileges/perf_event_paranoid.

ID: `intel:<hex canonical device path>:i915:engine:<hex config>:utilization`.
Config кодирует engine class/instance; cardN и display label не входят в identity.
Каждый engine — отдельный Percent binding, пригодный для LCD и dynamic RGB.
Общий Windows-equivalent GPU Load пока не публикуется: правило aggregation ещё
не установлено. xe и Intel VRAM queries уже реализованы; см. текущую
coverage table и [Intel engine document](intel-engine-utilization.md).

Busy ns делятся на delta perf running ns; неполный/multiplexed интервал
отклоняется. Первый sample, нулевой denominator и interval >3s unavailable;
counter reset invalid с новым baseline. Valid zero остаётся zero. До 1% skew
между отдельными busy/perf timestamp reads ограничивается 100%; больше invalid.
Bounds: 256 directory entries, 32 GPUs, 32 busy engines/GPU, scalar input64KiB,
CPU list4096 entries. Malformed/duplicate events не дают частичный успех.
Missing/failed handles закрываются; после восстановления нужен новый warmup.

Fixtures покрывают warmup→load, valid zero, card renumber/reconnect, permission
и read failures, multi-GPU ambiguity, encoded-event/unit validation, PMU type
смену, oversized catalog, cpumask/migration, stale/reset/multiplexing/skew.
Сам kernel PMU API в тестах не вызывается; аппаратная accuracy ещё не проверена.


## Общая панель: подтверждённый срез 2026-10-10

[System Information inventory](system-info-inventory.md) подтверждает 11 Mini
picker/factory tiles и ResourcePage clock/storage controls. Missing Linux:
на момент ранней сверки включал voltage/clocks/network/storage. Clocks,
network и storage теперь реализованы; текущий остаток — platform-specific
CPU/NVIDIA voltage, Intel aggregate semantics и FPS и Mini shell-интеграции. Mini реализуется как виджет KDE Plasma и расширение
GNOME Shell, без отдельного окна; Sway и остальные окружения отложены. Полный
список поддержки пока не определён; [контракт](mini-system-info.md).
Windows GPULoad manager выбирает один HWiNFO sensor; max/mean engines не является
установленным Windows aggregation algorithm. Intel ABI equivalence ещё не доказана.


## GPU hwmon power — 2026-10-10

Collector поддерживает `powerN_input` и `powerN_average` у уже распознаваемых
GPU hwmon drivers `amdgpu`, `nouveau`, `i915`, `xe`, если атрибуты присутствуют.
[Kernel hwmon ABI](https://docs.kernel.org/hwmon/sysfs-interface.html#power)
задаёт microWatts: деление на 1e6 даёт Watts. Input и average имеют разную
семантику и отдельные IDs: `hwmon:<hex physical path>:<driver>:powerN_input`
либо `powerN_average`. Label сохраняет channel label и вид измерения; cap,
max/min, energy и averaging interval не выдаются за измеренную мощность.

Отрицательное/нечисловое значение invalid; корректный zero valid. Optional
powerN_enable=0 unavailable, invalid enable flag invalid; чтение только read-only.
Удалённый канал исчезает из catalog, binding не переключается на average или
другую GPU. При hwmon renumber identity сохраняется; общие capacity, scalar
read limits, duplicate identity и 3s freshness действуют как для температуры.

Это driver-reported power отдельного канала: не обещается whole-board/TGP и
не суммируются перекрывающиеся channels; особенно важно для APU. NVIDIA
proprietary nvidia-smi power добавлен ниже; hardware accuracy остаётся открытой. Физические
sysfs/PMU/HID не опрашивались. Watts уже поддерживается UI и LCD renderer;
регрессия проверяет fixture → cache → IPC → theme text/JPEG, fresh и stale.


## NVIDIA power — 2026-10-10

Существующий bounded query теперь включает `power.draw`; stable UUID binding
`nvidia:GPU-<UUID>:power_draw` доступен как Watts для Sensors и LCD themes.
Используется давнее поле `power.draw`, без требования новых `.average`/`.instant`.
Вид усреднения зависит от GPU/driver, поэтому название не обещает instantaneous
и показатель не объединяется с hwmon channels.
[Документация NVIDIA](https://docs.nvidia.com/deploy/nvidia-smi/index.html).

N/A/Not Supported → unavailable; отрицательные/NaN/inf → invalid; 0 → valid.
Неисправное power value не портит остальные поля GPU. Fixtures проверяют
аргументы процесса, CSV layout, units/formatting, стабильность IDs, cache delivery,
failure/recovery и bounded shutdown. Реальный NVIDIA driver не запускался.


## AMDGPU voltage and clocks — 2026-10-10

Read-only hwmon `in0_input` (GPU) / `in1_input` (Northbridge) → Volts через
millivolts/1000; `freq1_input` (GFX) / `freq2_input` (memory) → Hertz.
[Документированный AMDGPU ABI](https://docs.kernel.org/gpu/amdgpu/thermal.html).
Provider принимает только эти подтверждённые AMDGPU channels; motherboard
scaling и CPU voltage из них не выводятся. Memory clock не заменяется effective
data rate; Northbridge не называется CPU core voltage. Kernel label сохраняется,
при отсутствии используются названия rails/clocks из ABI.

IDs используют physical device identity + driver + in0/in1/freq1/freq2.
Renumber не меняет binding; disappearance не переназначает его. Optional enable
0 даёт unavailable, malformed/negative → invalid, 0 при enable1 → valid.
Сохраняются scalar/catalog bounds и freshness. В IPC units `volts`/`hertz`;
GUI и LCD форматируют V/MHz, с пределом двух decimals существующего theme
contract. Fahrenheit не преобразует электрические единицы.

CPU/NVIDIA/Intel voltage и platform-specific accuracy
остаются открытыми. Наличие provider не гарантирует наличие файла на каждом GPU.


## CPU frequency — 2026-10-10

Production collector читает `/sys/devices/system/cpu/cpufreq/policy*` в том же
blocking worker, без HID и без writes. Три независимых источника:
`cpuinfo_cur_freq` — hardware current; `cpuinfo_avg_freq` — hardware average;
`scaling_cur_freq` — driver reported (часто requested P-state, не обязательно
физическая текущая частота). kHz → Hertz в catalog, MHz в GUI/LCD.
[Kernel CPUFreq policy ABI](https://docs.kernel.org/admin-guide/pm/cpufreq.html#policy-interface-in-sysfs).
Limits/max/setspeed/governor не выдаются за текущую частоту и не изменяются.

ID `cpufreq:cpus-<sorted logical CPU IDs separated by dots>:<attribute>`
строится из `related_cpus`, включая offline CPUs. Номер policy directory и
`affected_cpus` не задают identity. Изменение состава группы создаёт новый ID;
исчезновение policy удаляет её строки, consumer не выбирает замену. Это identity
топологии хоста, не переносимый hardware serial и не общий CPU max/average.

Максимум 128 directory entries и 128 CPU IDs на policy, 384 readings. CPU IDs
валидируются как u16; malformed/duplicate/overlapping groups и overflow
отклоняют catalog с discovery invalid. Ошибка discovery — unavailable. Scalars
ограничены общим 64KiB reader; отрицательное/нечисловое invalid, valid zero
сохраняется. Для существующей policy неподдерживаемое/недоступное поле остаётся
unavailable без fallback на другой источник. Общая monotonic freshness 3s.
`LinuxSensors::new` не включает реальные providers; fixture root задаётся через
`with_cpufreq_root`. CPU voltage и hardware accuracy остаются открытыми.


## NVIDIA current clocks — 2026-10-10

Bounded read-only query включает `clocks.current.graphics` и
`clocks.current.memory` в дополнение к temperature/load/VRAM/power. По
[официальной документации](https://docs.nvidia.com/deploy/nvidia-smi/index.html#clocks)
эти readings — текущие частоты GPU domains в MHz. Catalog конвертирует их
в Hertz, GUI/LCD показывает MHz. UUID suffixes `graphics_clock` и `memory_clock`
не меняют предыдущие IDs. Это не max/applications/locked clocks и не memory
transfer rate; никакого управления частотами не добавлено.

Восемь CSV columns обязательны; старый/неожиданный layout отклоняется.
N/A/Not Supported unavailable, negative/nonfinite/overflow invalid, valid zero
сохраняется. Ошибка одного clock не портит другой domain или прочие readings.
Timeout 750ms, stdout64KiB, limit32GPU и kill/reap сохранены. Voltage NVIDIA
и фактическая поддержка/accuracy драйвера остаются отдельными задачами.


## Intel i915/xe GT clocks — 2026-10-10

Отдельный read-only provider перечисляет DRM card nodes и определяет driver
по canonical device/driver. i915: `cardN/gt/gtN/rps_act_freq_mhz` и
`rps_cur_freq_mhz`; xe: `device/tileN/gtN/freq0/act_freq` и `cur_freq`.
Первое поле — actual frequency, второе — requested frequency; оба MHz → Hertz
в catalog и MHz в LCD. Requested никогда не подставляется вместо actual.
Ноль допустим: например, xe actual может быть 0 в RC6.

Sources: [i915 attributes and getters](https://raw.githubusercontent.com/torvalds/linux/v6.17/drivers/gpu/drm/i915/gt/intel_gt_sysfs_pm.c),
[i915 GT registration](https://raw.githubusercontent.com/torvalds/linux/v6.17/drivers/gpu/drm/i915/gt/intel_gt_sysfs.c),
[xe frequency ABI](https://docs.kernel.org/gpu/xe/xe_gt_freq.html),
[xe getters/scaling](https://raw.githubusercontent.com/torvalds/linux/v6.17/drivers/gpu/drm/xe/xe_guc_pc.c).

При отсутствии directory `gt` i915 читает legacy `gt_act_freq_mhz`/
`gt_cur_freq_mhz` с отдельным `legacy` domain ID. Legacy card-wide значения
могут агрегировать несколько GT; это не alias отдельного GT. Наличие modern
GT исключает fallback на legacy даже при ошибке чтения. Max/boost/limits
не выдаются за current и не изменяются.

ID `intel-clocks:<hex canonical physical device>:<driver>:<domain>:actual_frequency|requested_frequency`;
xe domain содержит tile и GT. Card renumber не меняет ID, удалённый GT исчезает;
между driver/GT/layout bindings нет auto-remap. Существующий i915 PMU provider
не менялся: clock чтения не требуют perf-event permissions и не дают utilization.

Bounds: 256 entries на перечисляемую директорию, 32 устройства, 512 readings;
scalar read64KiB, MHz проверяется как u32. Duplicate device/overflow invalid;
ошибка enumeration unavailable; missing/invalid metric не портит соседний.
Production включён, fixtures имеют explicit root и не обращаются к настоящим
sysfs/PMU/HID. Intel utilization/VRAM/voltage и hardware accuracy остаются.


## Network RX/TX rates — 2026-10-10

Production collector читает `/sys/class/net/*/statistics/rx_bytes|tx_bytes`
в blocking worker. [Linux statistics ABI](https://docs.kernel.org/networking/statistics.html)
определяет счётчики байтов; monotonic delta/elapsed даёт BytesPerSecond,
GUI/LCD — B/s. Это не link speed, не bits/s и не application throughput.
Первый отсчёт, reset, смена ifindex, нулевой/обратный/>3s interval → unavailable
с новым baseline. Ошибка одного направления сбрасывает только его baseline.

Permanent address (`addr_assign_type=0`) + canonical physical device задают
устойчивый ID при rename/reboot. У виртуального или рандомизированного интерфейса
ID `network:boot-<boot UUID>:if-<ifindex>` живёт в пределах загрузки/namespace
daemon; после reboot такую тему надо перепривязать. Эти ID не обещают сохранение
при явном переиспользовании ifindex администратором.
[Network identity ABI](https://raw.githubusercontent.com/torvalds/linux/v6.17/Documentation/ABI/testing/sysfs-class-net).
Дубликаты physical identity отклоняются; интерфейсы с одним physical device и
одинаковым permanent MAC требуют дальнейшего dev_port/dev_id disambiguation.

Limit128 entries, 256 interface readings + 2 totals, scalar64KiB. Повторный
ifindex read предотвращает смешивание разных интерфейсов при повторном
использовании имени во время сбора. Discovery failure очищает baselines.
Missing interface исчезает; stale cache остаётся unavailable.

Windows `SystemInformation.cs:241–290` суммирует NetworkStatus deltas всех
перечисленных WMI NetworkInterface entries (hash в system-info source manifest).
Linux totals `network:total:rx_bytes_per_second|tx_bytes_per_second` суммируют
все текущие sysfs interfaces, включая loopback/bridge/VPN; label это указывает.
Это общий interface traffic, не уникальный Internet traffic: stacked interfaces
могут считать одни пакеты несколько раз. Если хотя бы один текущий интерфейс
не готов/ошибочен, итог недоступен; пустой catalog также unavailable. Linux
и Windows перечисления интерфейсов не заявлены идентичными. Удалённые интерфейсы
не продолжают вносить старые deltas. Никаких interface writes или real net reads
в fixture tests; реальную driver accuracy проверить позже.


## Filesystem capacity — 2026-10-10

Production async collector запускает `/usr/bin/findmnt` (util-linux) без shell:
`--json --bytes --list --df --real --nofsroot`
`--output=SOURCE,FSTYPE,UUID,FSROOT,MAJ:MIN,TARGET,SIZE,USED,AVAIL`.
Это каталог смонтированных real filesystems, включая доступные сетевые FS;
не точная копия Windows `DriveInfo` Fixed/Removable enumeration. Псевдо-FS
отбираются правилами findmnt; собственный disk I/O provider здесь не добавлен.

[util-linux v2.41 source](https://raw.githubusercontent.com/util-linux/util-linux/v2.41/misc-utils/findmnt.c)
задаёт SIZE через blocks, USED через blocks−bfree, AVAIL через bavail.
Readings: total, used, free=total−used, available и used_percent=100×used/total.
Free и available разделены: резерв файловой системы не выдаётся за used.
Bytes отображаются в GiB, заполнение — в %. Zero total даёт unavailable percent;
противоречивые/нечисловые значения invalid, отсутствующие unavailable.

ID содержит UUID (fallback SOURCE), FSTYPE и FSROOT в hex. TARGET — только
label; смена mountpoint с UUID не меняет binding. Fallback SOURCE не обещает
стабильность при переименовании устройства. Bind mounts с одним ID/MAJ:MIN
дедуплицируются по первому lexical target; одинаковый UUID на разных MAJ:MIN
отклоняет catalog. Subvolume roots отдельны; общей суммы дисков нет, поскольку
их capacity может пересекаться. Исчезнувшие volumes удаляются после success.

Bounds: stdout256KiB, 128 rows/640 readings, query750ms, reap100ms. Если helper
остался в uninterruptible kernel syscall, collector хранит Child и не запускает
новые helpers до его завершения. Это ограничение ожидания, не гарантия немедленного
завершения kernel syscall. Ошибка снимает старые значения и публикует discovery
status; общий monotonic freshness3s сохраняется. `LinuxSensors::new` не включает
storage, synchronous collect его не опрашивает; fixtures используют async collector.

[Проверки](evidence/2026-10-10-storage-capacity.json): reserved space, identities,
duplicate/subvolume/clone, empty/recovery, limits/malformed, timeout/reap/retained
helper, collector→cache→LCD text и GUI binding. Реальный findmnt для capacity,
сетевая FS, USB/HID и аппаратная точность не проверялись.


## Global defaults candidate registry — 2026-10-10

`daemon/sensor_selection.rs` классифицирует только catalog, созданный daemon
providers, по их документированным внутренним IDs. Label/unit без известного
provider ID не доказывают роль. Client-supplied candidates не должны использоваться
для Apply; production RPC строит их из server cache.

CPU: coretemp/k10temp temperature, proc utilization, CPUFreq clocks и RAPL
package power. Generic hwmon fan RPM допускается только для явного CPU mapping;
рекомендуемый выбор никогда не назначает fan автоматически. CPU voltage остаётся. GPU
sysfs identity объединяет canonical physical path + driver: hwmon и AMDGPU
load/VRAM, Intel GT clocks. NVIDIA использует provider UUID. Requested/actual
Intel и hardware/driver CPUFreq clocks сохраняются отдельными choices.
GPU core clock не подменяется memory clock; GPU voltage исключает Northbridge
rail; engine PMU и VRAM utilization не становятся общим GPU load.

Candidate содержит role, optional gpu_id и исходное reading с status. Duplicate
IDs и несовместимые units исключаются. Новые bindings требуют membership
текущего catalog и выбранной GPU. Неизменённый отсутствующий binding сохраняется,
чтобы отключённое оборудование не мешало правке другой настройки; противоречивые
present readings проверяются заново. Clear разрешён и при отключённой GPU.
Unavailable readings могут быть выбраны явно, но resolver не выдаёт их за valid.
Никаких автоматических переносов между GPU или подмены default source нет.

Registry необходимо менять вместе с ID formats providers; новый неизвестный
provider не становится candidate только из-за подходящего unit. Четыре tests
проверяют multi-GPU/roles/loss/recovery/ambiguity и настоящий hwmon collector
на synthetic filesystem с двумя AMDGPU и CPU. Production RPC теперь подключён; UI и аппаратное измерение остаются впереди.

## CPU fan source, XAML audit и реализация 2026-10-10

Windows SensorSettingPage содержит отдельный CPUFanSpeed picker. Linux schema
и source editor имеют CpuFan. Read-only hwmon fanN_input теперь собирается,
candidate registry предлагает generic hwmon fan для ручного назначения CPU fan.
Это eligibility, не доказанная физическая роль: UI просит выбрать канал по подключению.
Known GPU drivers amdgpu/nouveau/i915/xe и LCDV2 readings не предлагаются как CPU fan.
Рекомендуемый выбор оставляет cpu.fan=None независимо от label и свежих RPM.
Сохранённый cpu.fan является явным user mapping; schema не менялась.
Неизвестный новый ID отклоняется, прежний missing ID сохраняется без переназначения.
[Windows source evidence](evidence/2026-10-10-system-info-xaml.json).

### Реализован сбор hwmon RPM, 2026-10-10

`LinuxSensors` принимает стандартные fanN_input от hwmon drivers с валидным
именем и physical device identity, включая motherboard chips вне temperature
allowlist. Остальные неизвестные temp/in каналы по-прежнему не принимаются.
ID: `hwmon:<hex physical path>:<driver>:fanN`, label только для отображения.
RPM уже учитывает driver divisor/pulses; дополнительного масштабирования нет.
Ноль — valid stopped; negative/malformed/oversized — invalid; fault1/enable0
или ошибка чтения — unavailable. Alarm сам по себе не отменяет измерение.
Duplicate identity — invalid, overflow отменяет весь hwmon catalog. Bounded
scalar reads65 536 bytes, catalog256 и scan4096 files/chip. Missing channel
исчезает, stale cache отзывает value и сохраняет identity/time.

[Kernel ABI](https://docs.kernel.org/hwmon/sysfs-interface.html#fans),
[evidence](evidence/2026-10-10-hwmon-fan-rpm.json). Read-only: PWM и fan targets
не записываются. Это readings для Sensors/LCD; явное назначение CPU fan добавлено
через существующий source editor. Реальные драйверы не запускались.

CPU mapping проверен backend/browser/native: manual selection, zero RPM,
durable reload, missing/same-ID recovery (backend), DOM Apply/RPM и restart
(native). [Evidence](evidence/2026-10-10-cpu-fan-mapping.json).
Generic hwmon eligibility не доказывает motherboard принадлежность: пользователь
подтверждает wiring выбором. Реальные PWM/targets не изменяются.

## Intel hwmon voltage, 2026-10-10

Поддержаны `i915/in0_input` (GT voltage) и `xe/in1_input` (package voltage):
целые millivolts делятся на1000 и выдаются как typed Volts. Источники kernel6.17:
[i915 hwmon](https://raw.githubusercontent.com/torvalds/linux/v6.17/drivers/gpu/drm/i915/i915_hwmon.c)
(`hwm_info`, `hwm_in_is_visible/read`) и
[xe hwmon](https://raw.githubusercontent.com/torvalds/linux/v6.17/drivers/gpu/drm/xe/xe_hwmon.c)
(`CHANNEL_PKG`, `REG_GT_PERF_STATUS`, `xe_hwmon_get_voltage`). i915 разрешает
атрибут на DG1/DG2; xe — package channel1 на DG2. Sysfs уже переводит регистр
в millivolts; повторно применять register scale2.5mV нельзя.

Другие inN не принимаются: xe card channel0 не становится package voltage;
произвольный motherboard in0 не становится CPU voltage. Kernel label сохраняется,
при отсутствии label используются GPU GT voltage / GPU package voltage.
IDs сохраняют canonical physical path, driver и inN; GPU defaults registry
объединяет rail с той же physical GPU. Нет auto-remap между i915/xe.

Missing/permission failure остаются unavailable/отсутствующим ID; нет обещания
voltage для всех Intel GPU. Zero допустим, negative/malformed/disabled и stale
не дают fake zero. API/UI units и schema не менялись. Fixture→cache→IPC→LCD
format/JPEG preview дополнены обоими rails. Физическая accuracy не проверена.

556 Rust tests, fmt/strict Clippy/docs прошли; [evidence](evidence/2026-10-10-intel-voltage.json).

## NVIDIA voltage ABI, 2026-10-10

[Исследование](nvidia-voltage-abi.md): legacy nvidia-smi graphics voltage удалён;
подтверждён read-only NV-CONTROL attribute413 в microvolts. Optional collector
с explicit session и UUID identity реализован; NVML/headless support
этим не доказан. Существующий восьмиколоночный query не менялся.

## ASUS EC CPU core voltage, 2026-10-10

Добавлен `asusec:in0` как CPU voltage. Baseline Linux v6.17:
[asus-ec-sensors.c](https://github.com/torvalds/linux/blob/v6.17/drivers/hwmon/asus-ec-sensors.c)
содержит единственный тип hwmon_in (CPU Core) в обеих register families,
`find_ec_sensor_index` нумерует channels этого типа с нуля,
`scale_sensor_value` оставляет voltage в исходных mV, registration name — asusec.
[Документация драйвера](https://docs.kernel.org/6.17/hwmon/asus_ec_sensors.html)
подтверждает CPU core voltage; наличие канала зависит от поддерживаемой платы.
Source SHA256: `a72adc33c5d3f4e6db52b814b724401fcd3bd6eb4e7c39216e598a00aa3dde69`.

Provider использует canonical physical device ID, существующий bounded read,
scale mV→V и freshness. Role берётся из driver/channel, не изменяемой label.
Рекомендуемый CPU voltage может выбрать этот подтверждённый источник;
`cpu0_vid`, asusec in1 и произвольные motherboard in0 не подменяют его.
Generic motherboard inputs могут требовать board-specific resistor scaling
([hwmon ABI](https://docs.kernel.org/hwmon/sysfs-interface.html)); их поддержка
остаётся отдельной задачей. Пользовательская подпись не решает калибровку.

Synthetic fixtures проверяют membership/recommendation/selection, renumber,
missing/recovery, zero/disabled/invalid/stale и cache→IPC→LCD text/JPEG preview.
Физическая accuracy, доступность на конкретной плате и EC polling latency
не измерялись; kernel driver/host sensors не запускались.


## xe per-engine provider — 2026-10-10

[Реализация и source-selection semantics](intel-engine-utilization.md): DRM ENGINES
query, persistent active/total PMU counters, explicit i915/xe mapping; без
автоматического whole-GPU aggregate. Native source selection/restart проверены;
physical accuracy остаётся отложенной.

## ASUS WMI CPU core voltage — 2026-10-11

Дополнительный opt-in provider вне asusec. [Kernel documentation](https://docs.kernel.org/hwmon/asus_wmi_sensors.html)
подтверждает CPU Core Voltage и отдельно CPU SOC/DRAM. [Linux v6.17 source](https://raw.githubusercontent.com/torvalds/linux/v6.17/drivers/hwmon/asus_wmi_sensors.c)
регистрирует имя `asus_wmi_sensors`; read_string возвращает firmware sensor name,
а scale_sensor_value преобразует исходные microV в sysfs mV. Linux collector
делит sysfs mV на1000 один раз. Source SHA256:
`dafc154c8b1e7279db1d6891f744508d5b6695a7c1b9fe9345a361c9c32551fd`.

Включение: передать daemon `LCONNECT_ASUS_WMI_VOLTAGE=1`. Другие значения и
отсутствие переменной оставляют дополнительный voltage collector выключенным.
Драйвер не загружается автоматически. Это явно выбранный дополнительный опрос:
kernel docs описывают дефекты некоторых BIOS при частых WMI запросах; перед
реальным использованием нужны подходящий BIOS и аппаратная проверка. Настройка
не отключает уже существующее чтение fan RPM этого драйвера.

Принимаются только `inN_input` с точной kernel меткой `CPU Core Voltage`.
SOC/DRAM/VID, неизвестные метки и тот же текст у другого драйвера не становятся
CPU voltage. Semantic ID `hwmon:<physical>:asus_wmi_sensors:cpu_core_voltage`
переживает hwmon и channel renumber. Если firmware даёт несколько одинаковых
CPU rails без различимых source IDs, collector помечает общий binding invalid,
а не выбирает произвольный SIO/EC channel. Это ограничение текущей поддержки.

Fixtures проверяют opt-in, масштаб, role/recommendation/selection, перестановку
каналов, missing/recovery, duplicate ambiguity, zero/negative/malformed и stale.
Автоматическое чтение host sysfs, WMI calls и измерение accuracy не выполнялись.
Other motherboard coverage, duplicate-rail disambiguation и physical polling
latency остаются открытыми; этот slice не закрывает весь CPU voltage parity.

[ASUS WMI checkpoint](evidence/2026-10-11-asus-wmi-voltage.json):588 Rust tests,
fmt/strict Clippy/doc прошли; review без замечаний.


Дополнительная сквозная проверка WMI: synthetic hwmon → collector → cache →
list_sensors → theme formatting → preview_lcd_theme JPEG.900mV отображаются
как0.90V, независимо от выбора °F; после cache age>3s выводится unavailable,
JPEG меняется. Свежий sample возвращает прежний JPEG с тем же binding без
редактирования темы. [Evidence](evidence/2026-10-11-asus-wmi-preview.json).
Это IPC/renderer acceptance без физического LCD и native GUI.

## Nuvoton CPUVC — 2026-10-11

Поддержаны только `in0_input` у `nct6779`, `nct6791`, `nct6792`, `nct6793`,
`nct6795`, `nct6796`, `nct6797`, `nct6798`, `nct6799`. В
[Linux v6.12 nct6775-core.c](https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/hwmon/nct6775-core.c)
эти variants назначают `NCT6779_REG_IN` в `REG_VIN`; первый вход таблицы
обозначен CPUVC. `in_from_reg` учитывает chip scaling, а `show_in_reg`
возвращает результат. [Kernel documentation](https://docs.kernel.org/hwmon/nct6775.html)
определяет единицу напряжения как mV: Linux collector делит на1000 один раз.

Физический device path + точный driver + in0 определяют ID и CPU voltage role;
label не выбирает роль. Без kernel label используется CPU core voltage (CPUVC).
VID и другие pins исключены; NCT6775/6106/6794, неизвестные NCT и IT87 не
добавлены по сходству имени или пользовательской метке. Значение0 остаётся
нулём, malformed/negative — Invalid, disabled — Unavailable, старый cache — Stale.
Смена hwmonN не меняет ID; исчезновение не подставляет ноль или иной rail.

Source подтверждает назначение входа чипа, а не разводку/точность каждой платы.
Hardware acceptance требует сверки CPUVC с BIOS/Windows для конкретной платы.
Никакие kernel modules не загружаются, настройки/регистры не записываются.
[Software evidence](evidence/2026-10-11-nct-cpu-voltage.json).

## Ограниченное завершение GPU helpers — 2026-10-11

NVIDIA CSV и NV-CONTROL больше не ждут `child.kill().await` без deadline.
Общий helper ограничивает чтение750ms, reap100ms; если kernel ещё не завершил
процесс, provider сохраняет Child и не запускает следующий запрос в этом слоте.
Отмена collection синхронно запрашивает kill и сохраняет ownership. Тот же
контракт применён к существующим Intel memory/xe query helpers.

Voltage запросы остаются параллельными и привязанными к UUID. Лимит32 включает
unreaped процессы исчезнувших GPU: при насыщении новые UUID получают unavailable,
а существующие независимые readings не подменяются и не смешиваются.
После reap capacity освобождается, запросы восстанавливаются.

Четыре новые регрессии проверяют cancellation/recovery, запрет overlap,
исчезновение32GPU с последующим recovery и независимость одного quarantined
UUID от другого, включая перестановку enumeration. Focused60sensor tests,
полный636Rust/93targets, fmt/strict Clippy/docs и отдельное ревью прошли.
[Evidence](evidence/2026-10-11-nvidia-bounded-reaping.json).

Фактический uninterruptible driver I/O не воспроизводился: тесты используют
owned fixture children и проверяют admission/reap/cancellation contract.
Host GPU/sensors/HID не открывались; новых voltage sources эта правка не добавляет.


## Zenpower SVI2 Core / CPU temperature — 2026-10-16

Дополнительный read-only источник при **уже загруженном** hwmon driver `zenpower`.
ABI сверено по [ocerman/zenpower](https://github.com/ocerman/zenpower/blob/5e2f56fabeba0c909edb90221eeace4a3d726dbb/zenpower.c)
и [zenpower3](https://github.com/PutinVladimir/zenpower3/blob/27eb87fb058dbf10d46133d5aa563986fc9db196/zenpower.c).
Оба регистрируют имя `zenpower`: in0 скрыт, in1 — SVI2 Core в mV, in2 — SoC.
Collector принимает только in1 с label `SVI2_Core`, `cpu0 SVI2_Core` или
`cpu1 SVI2_Core`; неизвестная/отсутствующая label, SoC, VID, in0 и driver alias
`zenpower3` не подменяют CPU voltage. Driver-reported mV делятся на1000 один раз.

CPU temperatures также доступны в Celsius с сохранёнными labels Tdie/Tctl/Tccd.
Sensor role/selector используют существующие CpuVoltage/CpuTemperature; ID
`hwmon:<physical>:zenpower:in1` переживает hwmon renumbering. Source replacement
не переписывает существующие bindings; missing/stale не выбирают другую CPU.
Модуль не устанавливается/не загружается, k10temp не отключается, SMN напрямую
не читается. Accuracy и соответствие конкретному Windows reading требуют
отложенной проверки; это не утверждение о поддержке всех AMD CPUs/драйверов.

SHA256 исследованных sources: zenpower
`7e14d63e878bb7372249db893eb35ca3a9f827ec9b0f4c1e68b8e939ebcabdf8`,
zenpower3 `5758cfcb2e6963f93bbdc46bd5e2f6c86add99248cd8ec65c74aa6196c938712`.
[Checkpoint](evidence/2026-10-16-zenpower-sensors.json): fixture values/labels/
identity/selection, faults/missing/recovery и cache→IPC→LCD JPEG fresh/stale/recovery.
Реальный host sysfs, kernel modules и LCD не использовались.


## FPS logged frames — 2026-10-16

[Opt-in MangoHud source](fps-source.md) публикует frames_per_second из новых
per-frame CSV records; existing benchmark, unchanged/stale/invalid input не valid0.
Collector вне HID, cache→IPC→LCD, explicit file-session ID и EN/RU Mini picker
подключены. Это mean записанных frame durations, не доказательство совпадения
HWiNFO PresentedAvg или автоматически активного процесса. Runtime read settings
подключены; capture activation/app selection, releases/
schema/game/shell/IO qualification остаются. [Checkpoint](evidence/2026-10-16-mangohud-fps.json).


### Runtime FPS configuration — 2026-10-16

Основное приложение EN/RU и CLI сохраняют enable/disable/absolute log path с
revision. Live watch меняет reader без restart; смена record немедленно отзывает
только FPS и не даёт старому collector восстановить его. Warmup пропускает
исторические frames. Saved disable имеет приоритет над legacy environment после
restart. Game logging activation/active-app selection и Windows PresentedAvg
semantics остаются открытыми. [Контракт](fps-source.md),
[software/native evidence](evidence/2026-10-16-fps-runtime-settings.json).
