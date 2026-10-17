# M2: оставшееся покрытие и аппаратная проверка

Актуализировано 2026-10-16. [Сводка реализации и проверок](../../parity/implementation-status.md).
Ранний M2 checkpoint: 416 workspace tests, fmt, strict Clippy, docs и release
build прошли. Более поздние checkpoints — в сводке; это не physical cooling acceptance.

## Активные задачи

- [ ] Закрыть подтверждённые gaps providers: CPU voltage вне asusec/opt-in ASUS WMI/поддержанных NCT CPUVC/zenpower SVI2 Core и NVIDIA platform coverage вне доступной X-сессии. Optional NV-CONTROL provider уже реализован и fixture-tested. CPU fan RPM и явный mapping уже проверены software/native. Source inventory общей панели и Settings завершён по XAML2.1.28; LCDV2 payload остаётся восьмипольным. Драйверные ограничения и missing/stale/identity acceptance сохраняются.
- [ ] Завершить соответствие Intel GPU load Windows provider: xe/i915 per-engine counters и явный source mapping уже реализованы; Intel VRAM query уже реализован с unavailable для неоднозначного нуля; определить достоверный общий load или явно выбранный источник, не выдавая сумму/max/mean engines за Windows aggregate. i915 per-engine PMU уже реализован и fixture-tested. AMDGPU load/VRAM и NVIDIA temperature/load/VRAM/power, GPU hwmon power и RAPL CPU power уже реализованы; [inventory](../../parity/sensor-provider-inventory.md). Проверить исчезновение устройства/ошибку источника/stale; не подставлять другую GPU или ноль.
- [ ] Выполнить [cooling acceptance](../../parity/lcdv2-m2-cooling-acceptance.md) после установления рабочих pump duty/source для восстановления. Зафиксировать реальные RPM, fallback, ручную отмену policy и restart.
- [ ] Fan hardware acceptance только при подтверждённом подключении к LCDV2. Со слов пользователя вентиляторы, вероятно, на материнской плате; 0 RPM не доказывает поломку кривой.
- [ ] Физически проверить thermal RGB: CPU/GPU thresholds/priority, manual grace, возврат base, stale input, mode/direct overlap и покрытие всей доступной зоны. В direct режиме сейчас гарантирован только известный prefix.
- [ ] После M3/M4 совместно проверить cooling + thermal RGB + media и lifecycle без периодических зависаний LCD.

## Условия приёмки

Реализованные registry/providers, curves/presets, thermal/dynamic RGB и named
slots описаны в [сводке](../../parity/implementation-status.md).
[Sensor inventory](../../parity/sensor-provider-inventory.md) и
[NVIDIA ABI](../../parity/nvidia-voltage-abi.md) задают границы доступных источников.
Программные результаты не закрывают физическую accuracy/cooling/RGB приёмку.
Аппаратная приёмка отложена пользователем; нужны известные настройки восстановления.
