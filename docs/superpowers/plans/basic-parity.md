# Базовый parity: оставшаяся работа

Сверено 2026-10-16 в `lconnect-linux-m1`. Baseline: Windows L-Connect 3
2.1.28.0, Galahad II LCDV2 0416:7395. Здесь только незавершённое;
[результаты](../../parity/implementation-status.md) и
[чеклист](../../parity/basic-parity-checklist.md) разделяют историю результатов и оставшуюся приёмку.
Полная parity шире базы: [roadmap](../../../roadmap.md).

## Reference и platform gaps

- [ ] Расширить подтверждённые voltage providers: CPU вне asusec/opt-in ASUS WMI/поддержанных NCT CPUVC/zenpower SVI2 Core,
  proprietary NVIDIA вне доступной X-сессии. Для каждой платформы нужны достоверный Linux
  источник, идентичность устройства, units и unavailable при отсутствии данных.
  Не подменять напряжение VID/другим rail. [Inventory](../../parity/sensor-provider-inventory.md).
- [ ] Установить соответствие общего Intel GPU load источнику Windows.
  xe/i915 per-engine и ручной mapping уже есть; сумма/max/mean не доказаны
  как Windows aggregate. Нужны Windows report/mapping и сравнение workloads.
  Intel VRAM query реализован; неоднозначный used=0 остаётся unavailable.
- [ ] Сверить locale-specific переносы, Windows font fallback и hinting/raster
  с Windows. CSS em, shaping/bidi, Unicode wrapping, hanging spaces, tabs,
  soft hyphen и bundled fallback уже проверены. Нужны одинаковые входы,
  шрифты/locale и сопоставленные изображения. [Font catalog](../../parity/font-catalog.md).

## Приёмка существующего GUI

- [ ] Завершить расширенную native accessibility: реальное озвучивание
  screen reader, оставшиеся dialogs/editor/error/limit/expanded states,
  Wayland/fractional scaling и другие fonts/themes. Доставка AT-SPI events
  не доказывает озвучивание. Уже принятые X11 keyboard/chooser/zoom/DPI flows
  повторно не планировать как новую реализацию; границы проверок —
  [native acceptance](../../parity/native-keyboard-acceptance.md) и
  [accessibility acceptance](../../parity/accessibility-acceptance.md).

## Итоговый checkpoint

- [ ] На одном состоянии дерева выполнить fmt, strict Clippy, tests, docs,
  release build, frontend check/build/contracts, browser и изолированные
  native UI flows. Сохранить HEAD, dirty snapshot, окружение и ограничения.
  [Текущий общий checkpoint](../../parity/evidence/2026-10-11-integrated-font-sleep.json)
  прошёл:631Rust/119browser/26contracts, release/frontend build и оба native
  harness на329 неизменных файлах. Это квалификация уже реализованных workflows.
  Gate остаётся до закрытия требований выше и повторной проверки их реализации.

Quickstart/troubleshooting уже сведены. Этот план удалять только после
закрытия всех его пунктов. Windows/platform вопросы сохранять явными gates,
но они не блокируют оставшиеся задачи M4 из roadmap. GIF/MP4 → gallery →
loop/replace/stop → restart уже реализован; его повторная аппаратная приёмка
остаётся в M1/M2.

Suspend/resume и физические проверки остаются в M1/M2. Hardware acceptance
отложена пользователем; другие устройства пока вне доступного тестового набора.

Последующие provider/media checkpoints собраны в [сводке реализации](../../parity/implementation-status.md).
Они не закрывают перечисленные выше platform/reference gates.
