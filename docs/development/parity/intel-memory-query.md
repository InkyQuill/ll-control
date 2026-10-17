# Intel dedicated VRAM: read-only query provider

2026-10-10. Программная реализация для i915 и xe; реальные драйверы не опрашивались.
Проверки и hashes: [checkpoint](evidence/2026-10-10-intel-memory.json).

## Источники и значение данных

Linux **v6.17**:

- [i915 UAPI](https://github.com/torvalds/linux/blob/v6.17/include/uapi/drm/i915_drm.h):
  `drm_i915_query`, `drm_i915_query_item`, `drm_i915_query_memory_regions`.
- [i915 query](https://github.com/torvalds/linux/blob/v6.17/drivers/gpu/drm/i915/i915_query.c):
  `query_memregion_info`, включая `perfmon_capable()`.
- [xe UAPI](https://github.com/torvalds/linux/blob/v6.17/include/uapi/drm/xe_drm.h):
  `drm_xe_device_query`, `drm_xe_query_mem_regions`, `drm_xe_mem_region`.
- [xe query](https://github.com/torvalds/linux/blob/v6.17/drivers/gpu/drm/xe/xe_query.c):
  `query_mem_regions`, также с `perfmon_capable()`.

i915 query ID4 / DEVICE class1: сумма `probed_size`, расход — сумма
`probed_size - unallocated_size`. xe query ID1 / VRAM class1: сумма
`total_size` и `used`. SYSMEM class0 исключается. Дубли регионов, неизвестные
classes, несогласованные длины, переполнение и used>total дают invalid.
iGPU без dedicated region получает unavailable; общая RAM не выдаётся за VRAM.

**Accounting ограничено в обоих драйверах.** Без CAP_PERFMON/CAP_SYS_ADMIN
xe возвращает used=0, i915 — unallocated=total. Старый i915 также может
не поддерживать accounting. Сам факт успешного ioctl не доказывает ноль.
Текущий provider публикует положительный расход только из текущего ответа;
при нулевой сумме used/load — unavailable, total остаётся доступным.
Это консервативно скрывает и настоящий ноль при полностью свободной VRAM.
Признак достоверности не переносится между опросами; capabilities не выдаются
автоматически. Проверка UID или CapEff внутри user namespace не используется
как доказательство прав в initial namespace. Обработка истинного нуля остаётся
явным ограничением, пока нет надёжного признака accounting.

## Выполнение и привязки

`lconnectd --query-intel-memory` — внутренний одноразовый helper mode.
Он выбирается **до** Tokio runtime, config, persistence, HID и IPC initialization.
Обычный daemon запускает собственный executable с этим фиксированным аргументом,
без shell, в общем sensor collector вне HID/IPC. Отдельный executable устанавливать
не требуется. Fixtures используют явно переданный script; `LinuxSensors::new`
и `Default` не включают self-exec сами — это делает production main.

Новый crate `lconnect-intel-memory` отделяет узкий unsafe ioctl boundary;
в daemon остаётся `forbid(unsafe_code)`, остальные модули нового crate запрещают unsafe.
Запросы только VERSION и memory query. Render node открывается O_RDONLY/
O_NOFOLLOW/O_NONBLOCK; fstat проверяет character device и ожидаемый major/minor,
VERSION проверяет driver открытого FD перед driver-specific ioctl.
Canonical physical device/driver проверяются до и после query.

Discovery ограничен 256 entries / 32 GPUs. Query — два прохода, максимум
32 regions / 2832 bytes, output helper максимум 64 KiB. Timeout 750 ms,
kill + reap до 100 ms; если child ещё не завершён, он сохраняется для reaping,
новый helper не запускается. Ошибка всего helper отзывает прошлые значения;
ошибка отдельного query сохраняет остальные устройства. Исчезновение binding
не вызывает автоматического выбора другой карты.

IDs: `intel-memory:<hex canonical device path>:i915|xe:vram_total|vram_used|vram_used_percent`.
Единицы Bytes/Bytes/Percent, процент — 100×used/total. Source editor классифицирует
used как GpuMemoryUsed в той же `sysfs:<physical>:<driver>` группе, что clocks/hwmon.
Physical identity сохраняется при render-node renumbering; перенос карты в другой
PCI slot может изменить путь. Cache freshness и LCD unavailable применяются штатно.

## Программные проверки

- Decode i915/xe, несколько dedicated regions плюс SYSMEM, iGPU, redacted zero,
  truncated/oversized/duplicate/unknown/inconsistent/overflow replies.
- Query size negotiation и изменение длины между проходами; обычный файл
  отклоняется до ioctl. Фикстуры discovery: перестановка render nodes, две GPU,
  независимая ошибка и rebind/disappearance во время query.
- Helper fixed argument, revoke/recovery/disappearance, timeout/reap и quarantine
  без второго процесса; строгий JSON-контракт и source selection.
- Синтетический helper → collector → cache → IPC → sensor text/JPEG preview;
  удаление helper не ломает cache-only RPC, stale отзывает значение и меняет preview.

[Compile-only C probe](../../../tools/verify-intel-memory-abi.c) сверяет размеры/offsets
с UAPI. Скачанные upstream v6.17 headers положить в отдельный каталог под именами
`i915_drm.h` и `xe_drm.h`, затем собрать с `-D__user= -I<каталог> -I/usr/include/drm`.
На x86_64 получены VERSION `c0406400`, I915_QUERY `c0106479`, XE_QUERY `c0286440`;
region size88, i915 header16, xe header8, total offset8, usage offset16.
Probe только печатает константы, не выполняет ioctl. Это проверка ABI, не driver acceptance.

## Отложенная аппаратная приёмка

Проверить доступ к render node, поддержку query, total и положительный used
на i915/xe discrete GPU, iGPU без dedicated memory, permission-denied/redacted
accounting, multi-GPU mapping, исчезновение/возврат, latency и отсутствие пауз
LCD при совместном опросе. Настоящий ноль сейчас ожидаемо unavailable.
Не менять permissions и не выдавать широкие capabilities в ходе автоматического теста.
Intel xe per-engine utilization также [реализован](intel-engine-utilization.md).
Underlying aggregate GPU load и его равенство Windows остаются открытыми.
