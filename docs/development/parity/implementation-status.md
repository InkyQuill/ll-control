# Реализованное и границы проверки

> **Release audit 2026-10-17:** [current audit](release-audit-2026-10-17.md) distinguishes base canvas gaps from implemented form editing and retained runtime evidence. [R0–R10](release-milestones.md) now own scheduling; the old three plans remain acceptance sources. First release: direct canvas + stable core + actual Plasma Wayland/LCDV2 qualification.

Сверено **2026-10-17**. Baseline — **Windows L-Connect3 2.1.28.0**,
**Galahad II LCDV2 0416:7395**, квадратный LCD480×480. Другие семейства
не квалифицированы; metadata круглого LCD не означает поддержку нового устройства.
Основные цепочки ниже реализованы, но полный Windows parity ещё не подтверждён.

Порядок оставшейся работы — [roadmap](../../../roadmap.md), все открытые критерии —
[чеклист](basic-parity-checklist.md), Windows provenance —
[матрица](lcdv2-2.1.28-matrix.md). Подробные checkpoints перенесены без изменения
содержимого в [архив по2026-10-16](implementation-checkpoints-through-2026-10-16.md).
Он сохраняет историю и ограничения каждого среза, не заменяя текущий статус.

## Текущий программный кандидат

`target/distribution-20261017-realtime` supersedes `-dividers`:845 frozen inputs,
33 payload files, production `custom-protocol` GUI без `native-smoke`, обе Mini
integrations в общих системных stores. Включает cooling mode banks, Windows
Control/Screen import, independent current scene/editor/basic banks, private
preview/recovery и редактируемые Three/Four Sensor dividers.
[Manifest, source hashes, команды и ограничения](evidence/2026-10-17-realtime-shadows.json).

- Full workspace:1373 Rust passed/9ignored, doctests включены; отдельно4 example
  tests. Strict Clippy/rustdoc, Svelte0/0, static1144/1144 и release прошли.
  Final full browser:758 passed на4 workers. 50 dynamic outputs — inventory,
  не автоматическое подтверждение всех flows.
- Native EN/RU:14 basic previews на язык, дробные координаты divider,
  Save отдельно от Apply, фактический reassembled mock-HID JPEG, normal Close
  и GUI/daemon restart с сохранением обеих banks/library.
- Private package install→upgrade→remove проверил hashes/modes/ABI/retention и
  оба Mini stores. Actual packaged GUI0/1mock без JS injection:7 sensor phases,
  12 pump states, explicit Apply/LocalUI, normal Close и replay без Apply.
- Из этого пакета проверены Qt3 targets/shared model4 cases, GJS feed/actual CLI
  IPC/preferences и private Plasma host. Реальный GNOME Shell/desktop session
  не заявлен; GNOME51 — candidate. Host install не выполнялся.
- **Realtime minimum20fps gate пройден после оптимизации:** RGB text shadow
  masks заменены одним идентичным каналом; alpha-zero blends пропускаются.
  Регрессия проверяет полное побайтовое480×480 совпадение с прежним RGB renderer.
  Release MockBackend: video+four sensors+three dividers,722 новых кадра за30.849с,
  23.40fps при nominal24; весь723-frame H.264 stream декодируется480×480.
  Это один короткий software run, не физическая или long-run квалификация.
  Предыдущие failed19.75/19.64fps runs сохранены в Divider evidence.
- Новый package проверен отдельно после единственного image renderer source delta.
  Frontend/native divider workflows относятся к неизменённому UI; новые production
  binaries повторили package/Mini/GUI0/1mock sensor/pump/Close/restart/replay.

Три активных плана сохраняют открытые требования. Hardware, real desktop/portal/
Wayland/speech/scaling, Windows/provider/reference, expanded native faults,
GitHub/clean-host/reproducibility gates остаются открытыми.
[Ранние срезы этого дня](implementation-checkpoints-2026-10-17.md) и
[архив по2026-10-16](implementation-checkpoints-through-2026-10-16.md) сохраняют
историю; их старые описания packages/неподключённого GUI не являются текущим backlog.

## Windows source workflow

Control импортирует проверенные fan/pump/RGB banks с явными sensor bindings.
Screen сохраняет current Windows runtime независимо от current0 editor,
поддерживает2 background banks ×7 overlays и source history. Linux named slots
сохраняются. Explicit source/font/resource choices → private preparation →
scene/editor preview → отдельный Apply → receipt/readback/restart.
[Контракт и source provenance](windows-profiles.md).

[Native media matrix](evidence/2026-10-17-windows-screen-media-matrix.json) принимает
owned PNG/MP4/GIF workflows EN/RU, CPU sensor overlay, decoded complete H.264
480×480 до/после restart без исходных файлов. Повреждённый MP4 отклоняется с
сохранением уже активной LCD theme, emitted JPEG bytes, draft/library/catalogs
и joined cleanup. Расширенные managed font/resource/expiry/uncertain faults и
original Windows exports/raster ещё не квалифицированы. Статические Three/Four
Sensor dividers реализованы и редактируются; source font/value-dependent flex
reflow и Windows shadow/raster остаются открытыми. [12 original-component samples](evidence/2026-10-17-windows-grid-component-geometry.json),
[реализация и исходные performance результаты](evidence/2026-10-17-grid-dividers.json).
[Последующая shadow optimization](evidence/2026-10-17-realtime-shadows.json) закрыла
minimum20fps software gate; physical/long-run проверка остаётся отдельно.

## Реализация по направлениям

Текущий пакет включает FanPreset endpoints Windows0°C/10% и100°C/100%,
сохраняя existing custom/saved curves. [Контракт](lcdv2-m2-cooling-acceptance.md#fan-preset-endpoints-2026-10-17),
[evidence](evidence/2026-10-17-fan-preset-endpoints.json).

| Направление | Реализовано и проверено | Что остаётся |
|---|---|---|
| Sensors / Spec | Typed cache с freshness, stable IDs, явным unavailable; поддержанные CPU/GPU/RAM/device providers, source selection, Spec inventory и optional MangoHud feed. [Inventory](sensor-provider-inventory.md), [consumer matrix](sensor-consumer-acceptance.md) | Accuracy/права на реальных drivers, [voltage/Intel source gaps](evidence/2026-10-16-sensor-gap-audit.json) вне доказанных ABI и FPS app identity; расширенная Spec/native приёмка |
| LCD themes | Image/text/clock/sensor/cycle, fonts/styles/parts, watermark, palette/background controls; explicit preview/Apply, per-layer и global C/F, square geometry. Daemon-owned realtime refresh/replay. [Workflow](lcd-theme-workflow.md), [controls](lcd-theme-controls.md) | Windows filters/raster/typography semantics, expanded controls/errors/accessibility, real LCD и lifecycle |
| Media / capture | Managed image/video gallery, import/edit/review/Save, GIF/MP4/H.264, video+sensor overlays, rotation и joined cancellation. Screenshot cleanup/owner-loss и ScreenRecord child isolation/recovery; [cleanup fences/native lost reply](evidence/2026-10-16-screen-record-review-cleanup.json). [Video](video-editor.md), [capture](screen-capture.md), [record](screen-record.md) | Реальные portals/PipeWire routing, region/DPI/parent window, Windows frame/colorspace semantics, large-source/resource и оставшаяся native fault matrix |
| Cooling | Pump/fan PWM и curves, pump RPM reference mapping, source/presets, fallback/recovery, manual override, stored mode ID/requested snapshot и persistence; actual mock cmd138/139. [Acceptance](lcdv2-m2-cooling-acceptance.md) | Реальные duty/RPM, wiring/source/restore baseline и совместная приёмка. Fans на motherboard не управляются каналом LCDV2 |
| RGB | Manual catalogs, per-mode settings/palettes, Static/Dynamic, sensor bindings, thermal arbitration, sync/topology и replay. [Sensor RGB](sensor-driven-rgb.md) | Expanded native faults/accessibility, реальная LED coverage и совместная media/cooling нагрузка |
| Profiles / reset |8 screen и4 fan slots;4 pump slots — Linux extension. Draft/load отдельно от Apply; full Screen/FanAndLighting archives/reset, receipts/readback/retry/reapply и durable import recovery. [Profiles](profiles.md), [groups](profile-groups.md), [reset](fan-lighting-reset.md) | Genuine Windows exports, intrinsic grid reflow и extended boundary/fsync/partial install/lifecycle/native error matrix |
| Desktop / languages | English default с сохранением legacy Russian, обязательный RU, tray/autostart/units; Mini как Plasma widget/GNOME extension. [Language](interface-language.md), [Mini](mini-system-info.md), [lifecycle](desktop-lifecycle.md) | Полная EN/RU dynamic/native qualification, реальные KDE/GNOME/login/upgrade/accessibility; shell version matrix. Другие DE и дополнительные языки отложены |
| Firmware | Version/date parsing, scoped preparation/transport/ownership/jobs/reconnect и saved recovery inspection; guarded ordinary work. [Contract](firmware-lcdv2.md) | Safe end-to-end updater, durable coordinator/recovery и extended supervisor/quiescence/fault gates. Core scaffolding не означает готовый update workflow |

## Открытые границы

Реальные проверки отложены пользователем; после них оставлять **LocalUI**.
[Hardware checklist](lcdv2-hardware-acceptance.md) требует firmware/identity snapshot,
одного HID owner и согласованных pump duty/source для восстановления. По сообщению
пользователя вентиляторы, вероятно, подключены к motherboard; это нужно уточнить
до любой физической fan-проверки через LCDV2.

[Package delivery](package-delivery.md) ещё требует GitHub execution, clean-host
binary reproducibility, real-session install/autostart/upgrade/uninstall и Mini
session qualification. GNOME51 — только candidate, не объявленная версия поддержки.
Полный standalone Mini window не разрабатывается; Sway/другие окружения отложены.
Дополнительные эффекты/Style Express и остальные языки — последними.

Три [активных плана](../superpowers/plans/README.md) сохраняются: basic5, M1:5,
M2:6 открытых пунктов; они пересекаются и не образуют процент готовности.
Удалять план только после закрытия всех его требований. Подробные новые checkpoints
следует хранить в evidence/архиве; здесь — актуальное состояние и границы.
