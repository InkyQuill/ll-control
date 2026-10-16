# NVIDIA voltage: подтверждённый ABI и границы

Проверено 2026-10-10. Это source research для базового parity;
Optional NV-CONTROL provider реализован; hardware/драйверы не запускались.
[Manifest](evidence/2026-10-10-nvidia-voltage-abi.json).

## Что действительно доступно

Официальный [nvidia-smi changelog](https://docs.nvidia.com/deploy/nvidia-smi/index.html)
сообщает, что graphics voltage сначала стал всегда N/A, затем был удалён
в v580 Update относительно v575. PSU voltage в разделе UNIT ATTRIBUTES
не является напряжением GPU core. Добавление legacy voltage в существующий
восьмиколоночный query не является решением: неизвестное поле может завершить
весь запрос ошибкой и отключить остальные readings.

В [NV-CONTROL header](https://github.com/NVIDIA/nvidia-settings/blob/df684ed9c29fd116c24a68198b681ef69e4f2c53/src/libXNVCtrl/NVCtrl.h#L3353)
есть read-only `NV_CTRL_GPU_CURRENT_CORE_VOLTAGE` (413), текущие microvolts.
`GPUCurrentCoreVoltage` зарегистрирован в parse.c. Это отдельный показатель
от writable `GPUOverVoltageOffset` (412); offset нельзя выдавать за reading
или менять для получения показателя. Наличие атрибута ограничено поддержкой GPU.

В проверенном `NvCtrlAttributesNvml.c` нет реализации атрибута413;
неподдерживаемые integer permissions возвращают NotSupported. Поэтому переход
nvidia-settings на NVML сам по себе не доказывает headless voltage support.
NV-CONTROL требует доступной X-сессии с соответствующим расширением;
доступность через XWayland и конкретный драйвер пока не доказана.

GPU UUID доступен как string attribute52 (`GpuUUID`). CLI позволяет GPU target
specifications; числовой индекс target не является стабильным UUID. Нельзя
соединять outputs разных providers по порядку строк или предполагать, что
nvidia-smi index совпадает с NV-CONTROL target index.

## Контракт реализации и оставшаяся приёмка

- Отдельный optional collector: отказ voltage не отменяет temperature/load/
  VRAM/power/clocks. Основной nvidia-smi query остаётся самостоятельным.
- Query-only helper выбран после проверки startup/config side effects и UUID
  targeting; точные flags и ограничения описаны ниже.
  Не импортировать пользовательскую display authentication в system daemon
  неявно. Session configuration и отсутствие X должны иметь явный статус.
- Получать UUID вместе с voltage и валидировать соответствие тому же GPU;
  ambiguous/duplicate/missing identity отклонять. Перестановка targets и
  исчезновение GPU не должны менять сохранённый binding.
- Преобразовать uV→V ровно один раз. Zero — отдельное допустимое значение;
  unsupported, permission failure, malformed/negative/overflow и stale не
  подменяются нулём или последним успешным reading.
- Ограничить время/размер вывода/число targets и завершать helper при timeout;
  исключить накопление дочерних процессов. Read-only requests, без assignments,
  Coolbits, настройки voltage/clocks или загрузки пользовательского config.
- Проверить fixtures → cache → IPC → source selection → LCD text/preview,
  независимость отказов, timeout/recovery, UUID reorder и restart bindings.
  Реальный X11/Wayland/headless набор остаётся в аппаратной приёмке.

NVIDIA voltage остаётся открытым gap. Отсутствие современного nvidia-smi поля
не означает, что весь Linux provider невозможен, и не закрывает требование
полной parity. NV-CONTROL — реализованный источник с ограниченной применимостью.

## Реализованный Linux путь

Daemon использует отдельный optional NV-CONTROL запрос для каждого UUID из
успешного nvidia-smi discovery. Включение требует **обеих** переменных в окружении
процесса daemon:

- `LCONNECT_NVIDIA_VOLTAGE_DISPLAY`: выбранная X-сессия, например `:0`.
- `LCONNECT_NVIDIA_VOLTAGE_XAUTHORITY`: абсолютный путь к файлу авторизации,
  который уже доступен пользователю daemon.

Автоматического поиска/copy credentials и изменения systemd/environment нет.
Без этих настроек collector отключён. Нужен `/usr/bin/nvidia-settings`;
существующий `/usr/bin/nvidia-smi` остаётся независимым источником остальных метрик.
Эта функция не требует включать Coolbits или менять voltage offsets.

Запрос содержит только `--no-config --terse --ctrl-display ...` и два `--query`:
UUID и `GPUCurrentCoreVoltage` для `[gpu:UUID]`. В pinned NVIDIA source
`nvidia-settings.c` query/assignment branch выходит до чтения config;
`NvCtrlAttributesUtils.c` добавляет UUID в protocol names GPU,
`query-assign.c` сопоставляет имена без учёта регистра.
Ответ принимается только как UUID + одно integer значение; неожиданные строки
или другая GPU отклоняются. ID: `nvidia:GPU-...:core_voltage`, typed Volts,
тот же GPU group для source editor.

Не более32 voltage children, включая ещё не reap-нутые процессы исчезнувших GPU.
Один child на UUID, параллельное чтение с750ms timeout каждого, stdout4096bytes.
После ошибки/отмены отправляется kill; ожидание reap ограничено100ms.
Новый запрос не запускается до подтверждённого reap прежнего процесса. Отказ одного voltage query не отменяет
остальные readings. Ошибка discovery отзывает все прошлые значения; успешное
пустое discovery удаляет IDs. Отсутствующий/неподдерживаемый канал не становится0.

Остаются реальные X11/XWayland/Wayland/headless availability и accuracy;
чистой Wayland/headless альтернативы эта реализация не добавляет. Требование
NVIDIA voltage считается выполненным только для доступного NV-CONTROL источника,
а не для всех драйверов и окружений.

Ограниченное reap/cancellation и cap при исчезновении GPU проверены отдельно:
[636Rust tests и regressions](evidence/2026-10-11-nvidia-bounded-reaping.json).
Uninterruptible driver I/O на реальном железе не воспроизводился.

## Nouveau hwmon и проверка NVML — 2026-10-11

Для nouveau реализован `in0_input`: единственный GPU core voltage channel в
[Linux v6.12 nouveau_hwmon.c](https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/gpu/drm/nouveau/nouveau_hwmon.c).
Kernel переводит microvolts в millivolts, collector — millivolts в Volts.
[nvkm_volt_get](https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/gpu/drm/nouveau/nvkm/subdev/volt/base.c)
может использовать аппаратный getter или таблицу VID→microvolts. Поэтому это
**driver-reported voltage**, без гарантии аналогового измерения. Приложение
не интерпретирует raw VID. Наличие канала зависит от GPU/драйвера.

Проверены units, exclusion соседнего in1, stable physical identity после
hwmon renumbering, выбор источника, zero/invalid/disabled/stale, исчезновение
и восстановление, cache→IPC→LCD text/JPEG preview. Это независимый hwmon путь;
он не закрывает proprietary NVIDIA voltage вне X-сессии. Драйверы не менялись.

В NVML документации с датой обновления 2026-09-09 проверены
[RailMetrics](https://docs.nvidia.com/deploy/nvml-api/latest/api/structnvmlRailMetrics__t.html),
[ObservedMetrics](https://docs.nvidia.com/deploy/nvml-api/latest/api/structnvmlObservedMetrics__t.html),
[PmgrPwrTuple](https://docs.nvidia.com/deploy/nvml-api/latest/api/structnvmlPmgrPwrTuple__t.html)
и [PwrModelOperatingPointPfpp1x](https://docs.nvidia.com/deploy/nvml-api/latest/api/structnvmlPwrModelOperatingPointPfpp1x__t.html).
Их поля описывают частоты, utilization, power и связанные структуры;
voltage getter они не предоставляют. Это ограниченный результат исследования
этих структур, а не доказательство отсутствия всех возможных NVIDIA API.

[Evidence](evidence/2026-10-11-nouveau-voltage.json):27focused/636workspace
Rust tests/93targets, fmt/strict Clippy/docs и отдельное review прошли.
Hardware accuracy/availability и GUI после изменения не проверялись.
