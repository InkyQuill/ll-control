# LCDV2: оставшиеся пробелы parity

Актуализировано **2026-10-12**, Windows **2.1.28.0**, USB **0416:7395**.
Сверка текущего кода и сохранённых tests/captures; новых hardware runs не было.
[Матрица сценариев](lcdv2-2.1.28-matrix.md) содержит source provenance.
[Исходный аудит до M1](archive/2026-09-08-baseline-gaps.md) сохранён отдельно.

## Закрытые исходные gaps

GAP-001 (socket), GAP-002 (LCD patch/state), GAP-003 (frame transaction),
GAP-004 (clear/replace barrier), GAP-005 (stable persistence identity) закрыты
реализацией и регрессиями. GAP-007 (bounded diagnostic source/pacing) закрыт
для diagnostic clip, но не для произвольных GIF/MP4. GAP-008 (freshness и
автономная реакция) закрыт в registry/cooling/thermal; LCD consumer теперь реализован в theme runtime (1 Hz).
В GAP-006 подтверждены C/512 playback и длительный прогон на FW1.6;
physical recovery/lifecycle остаются открытыми. Не переоткрывать выполненные
задачи из-за старых unchecked boxes в архиве.

## Активные gaps

| ID / этап / сценарии | Сейчас и доказательство | Затрагиваемые области, зависимости | Проверяемый результат |
|---|---|---|---|
| PAR-01 / M3 / LCD-004–008,014 | Renderer/editor/runtime/replay есть: image/text/sensor/clock, °C/°F, Focus/Duo/Triple/Quad/Cycle/Clock. Triple/Cycle реализованы; font selection/shadow/opacity и live preview реализованы; gradient backgrounds, line boxes и bundled Noto Sans TC проверены; user font store/chunked RPC, managed references/renderer и backend profile/replay готовы, GUI import/выбор подключён; font cleanup, recovery и shared-store failure/race checks проверены; native cleanup matrix закрыта; Noto coverage/horizontal metrics/outlines сопоставлены; CSS em sizing, OpenType shaping/bidi, normal wrapping, hanging spaces, tab stops и manual soft hyphen проверены software; bundled glyph fallback добавлен; locale/Windows-specific fallback/raster, оставшиеся Windows controls и аппаратный вывод тем требуют сверки | Существующие theme модули daemon/image/IPC и desktop; UI parity inventory, cancellation/failure tests | Картинка + датчик + часы + text/watermark видны на LCD и preview; обновляются с закрытым GUI; stale и °C/°F корректны |
| PAR-02 / M3 / LCD/COOL/RGB/SYS UI | Tauri/Svelte desktop и пять страниц реализованы; browser mocked flows есть; native Display/Sensors/Cooling/Lighting/Settings → Unix IPC → MockBackend happy paths и restart прошли; browser interrupted/external updates, native recovery и основные keyboard workflows проверены; расширенная native accessibility и итоговая приёмка остаются | `desktop/src`, `desktop/src-tauri`, mock Unix daemon; isolated WebKitGTK acceptance | Display/Dashboard/Cooling/Lighting/Settings используют реальные states; disconnect/error не выглядит успехом |
| PAR-03 / M4 / LCD-002,009–011 | Diagnostic H.264 hardware-verified; bounded GUI GIF/MP4 import, сохранённая library, loop/stop/restart и software sleep replay реализованы. Named media slots и delete с active/pending/offline/profile reference protection реализованы; v2 storage/chunked upload/reader→actor и profile/restart/sleep реализованы; native long-video import подключён и проверен на586frames; video+sensor composition backend Apply/replay/first-frame preview проверены, GUI composition подключён; встроенные image/video defaults защищены; контексты выбора и image geometry/import/gallery editor реализованы; point filters software-tested, Windows raster/native pending; sharpen/blur software-tested; ordered temporal cut, per-interval spatial crop с GUI/local frame preview и source scrubbing при импорте реализованы; полный video editor/capture не завершены; selected-clip thumbnail browser/native-tested, полный gallery lifecycle открыт | Media decode/encode, gallery/assets, editor, capture; PAR-01/02 | GIF/MP4 импорт/loop/replace, crop/cut/autofill, screenshot/record; preview/thumbnail/LCD согласованы |
| PAR-04 / M4 / LCD-003,015 | Полный набор 23 страниц/эффектов отсутствует. Rain/Snow schema/renderer/H.264/GUI и persistence подключены, расширенная приёмка открыта; MeteorShower, Snow3D и Fireworks schema/renderer/theme/daemon/H.264/GUI/page-state/slots/restart подключены; mock/native happy path принят, video/fault/visual qualification открыта ([каталог](lcd-effects.md)). Screen reset реализован (storage/actor/IPC/GUI), native keyboard/reapply и mock lifecycle проверены; расширенная приёмка остаётся | Renderer/themes/defaults; PAR-01/03 | Каждый применимый preset работает, reset восстанавливает собственные defaults/assets |
| PAR-05 / M2 / LCD-004, COOL, RGB | hwmon/procfs/LCDV2 registry есть; AMDGPU load/VRAM добавлены; NVIDIA temperature/load/VRAM/power, GPU hwmon/RAPL power, CPUFreq/NVIDIA/Intel clocks, AMDGPU/Intel/nouveau hwmon voltage, generic fan RPM с ручным CPU mapping, network RX/TX и filesystem capacity добавлены; i915 per-engine PMU реализован; optional NVIDIA NV-CONTROL voltage реализован; zenpower SVI2 Core voltage/CPU temperature реализованы; CPU voltage вне asusec/opt-in ASUS WMI/поддержанных NCT CPUVC/zenpower, proprietary NVIDIA coverage вне X-сессии, Intel VRAM query реализован (неоднозначный used=0 остаётся unavailable); xe/i915 engine utilization и ручной mapping реализованы; соответствие общего GPU load Windows source остаётся | `daemon/src/sensors.rs`, `ipc/src/sensors.rs`, provider tests; Windows binding inventory | Подтверждённые поля имеют provider и верные unit/identity/error; unsupported не подменяются чужими значениями |
| PAR-06 / M2 / COOL-001–004 | Presets/curves/RPM mapping/source mock-tested; physical RPM не подтверждены | Cooling actor, hardware acceptance; известный pump restore и реальное подключение fan | Реальные duty/RPM/source, fallback/manual override/replay соответствуют ожидаемому поведению |
| PAR-07 / M5 / RGB-001–004 | Manual catalog/defaults6/16, applied mode profiles, palettes/load и global fan↔pump sync/SKU/manual count8…50 реализованы; modes/direct есть; high/low sensor-driven pump runtime и IPC реализованы; CLI/GUI, focused regressions и native enable/disable/restart/applied есть; browser interrupted mutations проверены; dynamic mode/source graph и expanded native/hardware coverage остаются | `actor/lighting_keeper.rs`, IPC/desktop, sensor registry; каталог Windows controls | Threshold transitions, fan↔pump sync, LED count и все применимые modes видимы на доступных зонах |
| PAR-08 / M2,M5 / RGB-005 | Thermal priority/grace/restore integration-tested; direct restore известного prefix | `actor/thermal.rs`, lighting owner; физическая LED topology и известный base | Alerts перекрывают требуемые зоны и восстанавливают base; нет конфликта с direct/video; непокрытая зона явно видна как ограничение |
| PAR-09 / M5 / COOL-002, SYS-001–003 | Persistence policies/config и активной theme/assets есть; named slots/library и IPC/CLI CRUD/apply реализованы; GUI CRUD/load draft/slot reset есть; cleanup с session retention реализован; атомарный reset группы слотов реализован; полный device Screen reset с коллекциями/IPC/GUI реализован; archive import/export и полный fan+lighting runtime reset не закрыты, расширенная Screen reset приёмка остаётся | Persistence, IPC, profile/asset storage, desktop; PAR-01/03 | Rename/select/restart/reset по группам и archive round-trip; Windows format interoperability исследована отдельно |
| PAR-10 / M1,M3,M4,M6 / SYS-004 | Stable identity/barriers/restart policy tested; physical reconnect отложен; production logind adapter/inhibitor, joined reopen, cache invalidation и сброс counter baselines реализованы; private-bus failure/recovery и combined cooling/RGB/theme проверены, v2 media profile/restart/sleep проверены с MockBackend; video+overlay composition Apply/restart/sleep/wake и first-frame preview проверены с real codec/MockBackend; GUI composition подключён; расширенная native приёмка и физический lifecycle остаются | Manager/display lifecycle, logind events; сохранённый source PAR-01/03 | Hotplug/sleep/cold boot без старых frames, потери settings и ложного applied; совместно media+cooling+RGB |
| PAR-11 / M6 / FW-001–003 | Version/C512 на FW1.6 наблюдались; safe updater отсутствует; date readback требует проверки | Identity GUI/IPC/CLI, pinned image/ACK codec, fenced transport и joined keeper lifecycle; owned coordinator/process-local supervision и saved recovery inspection IPC/CLI готовы; owned reconnect job готов; product Apply/probe UI/qualified resolution/restart policy остаются | Верные version/date; validated target/image, progress/exclusive ownership, post-update verification и исследованный recovery |
| PAR-12 / M5,M6 / desktop/delivery | systemd/udev/xtask есть; полный desktop/install UX и release gates не проверены | Packaging/CI/desktop/settings, manifest metadata | Install→autostart→upgrade→uninstall, tray/units/localization/diagnostics и воспроизводимые artifacts |

Пути `daemon/`, `ipc/` и `actor/` в таблице сокращают соответствующие
`crates/lconnect-daemon/` и `crates/lconnect-ipc/`.
Текущее покрытие и checkpoints: [сводка реализации](implementation-status.md).

## Research gates, не готовые требования реализации

- **LCD-016 startup/offline:** cmd15 codec есть, activation/runtime и применимый Windows caller не установлены. Найти caller/guard; затем добавить сценарий или обоснованно исключить. Не считать USB ACK доказательством offline content.
- **Editor:** image controls/ranges и geometry установлены; point filters прошли software проверки, Windows raster/native остаются. Video cut/edit/autofill/capture/record применимы, их time/format/audio/speed semantics требуют дальнейшей трассировки. [Контракт](ga2lcd-editor.md).
- **RGB-003/004:** manual LED-loop range8…50 извлечён из versioned XAML/Core Constants; sync/SKU/retention реализованы и mock/native-tested. Физический LED coverage и dynamic mode/source graph остаются. RGB-004 sources/cadence/HSV и color-count/speed catalog уже извлечены: [исследование](sensor-driven-rgb.md).
- **COOL-004:** pump source реализован; глобальный UI caller MB sync ещё исследовать. Fan MB sync/start-stop исключены product guards.
- **LCD-017:** отдельный Screen Animation Editor исключён для LCDV2 product guard; не смешивать с обычным GA2LCD Editor.
- **Legacy FW<1.5:** codec есть, аппаратного B/H.264 playback evidence нет. Текущая проверенная конфигурация — FW1.6/C512.

Profiles/slot reset/cleanup реализованы с описанной session retention.
Карточки видео приняты программно: [checkpoint](evidence/2026-10-12-media-gallery.json).
Video+overlay composition уже реализована и проверена backend/browser/native с MockBackend. Контексты выбора background/media/watermark и protected defaults реализованы. Screen reset, состав коллекций и recovery реализованы; prepared-result preview/confirmation подключены; gallery video cut/crop/fit re-edit подключён; AutoFill stretch реализован; VideoEdit brightness/contrast UI/preview/native реализованы; native single-range и range UI реализованы, check/build и42 browser tests прошли; native fresh/gallery single-range preview/seek/Save принят ([evidence](evidence/2026-10-12-native-gallery-video-edit.json)); оставшиеся Windows/native gates открыты; record/pause исходных интервалов уже реализован. Оставшаяся reset приёмка: native ambiguous reply/scaling/speech и прямые in-flight detach/shutdown. Точечное image delete и image/video defaults реализованы и проверены backend/browser/native; расширенная accessibility/fault приёмка остаётся. Bulk cleanup сохраняет отдельный консервативный контракт session retention. Карточки image/video уже реализованы; единая глобальная gallery не требуется.
Bounded GIF/MP4 import → library → loop/stop/restart уже реализован и software-tested.
Оставшиеся providers, Windows controls/typography и native accessibility сохраняются
в [базовом плане](../superpowers/plans/basic-parity.md); reference/platform gates
не блокируют независимый M4. RGB-004 software slice завершён. Hardware gates
остаются открытыми, пока нет соответствующего наблюдения. Порядок — [roadmap](../../roadmap.md).

## Уточнение capture backlog после сверки 2026-10-12

Screenshot уже проходит portal → image editor → Save с synthetic native
приёмкой. ScreenRecord включает negotiation/engine/Tauri owner, admission,
metadata, HTTP preview, start/pause/resume/finish/clear, multi-interval trim/crop,
диалог лимита и review → edit → reprepare → явное Save с gallery membership.
Начальный native workflow проверен на synthetic source. Новой реализацией
остаётся изменение области захвата между интервалами;
stalled source, расширенная native fault/crop/limit
матрица, реальные portals/PipeWire и hardware приёмка не закрыты.
Полный 180-секундный engine лимит принят на synthetic source
([evidence](evidence/2026-10-12-screen-record-full-limit.json)).
[Контракт](screen-record.md), [последний workflow checkpoint](evidence/2026-10-12-screen-record-return-edit.json).
