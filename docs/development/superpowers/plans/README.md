# Legacy acceptance plans

> **Scheduling superseded 2026-10-17:** [R0–R10 milestone plans](../../parity/release-milestones.md) and the [release audit](../../parity/release-audit-2026-10-17.md) now define priorities and cutover. The three plans listed below remain historical acceptance inventories, with every open requirement mapped in the new plan. Their old stage order is not the active roadmap.

Сверено **2026-10-17** в `lconnect-linux`, ветка `master`. Baseline: Windows L-Connect3
**2.1.28.0**, Galahad II LCDV2 **0416:7395**.

| План | Открытых пунктов | Оставшийся объём |
|---|---:|---|
| [Базовый parity](basic-parity.md) | 5 | Voltage coverage, Intel aggregate, Windows typography, native accessibility, итоговая квалификация |
| [M1](2026-09-08-m1-lcdv2-dynamic-display.md) | 5 | Физические LCD/lifecycle проверки и совместная приёмка media/composition |
| [M2](2026-09-09-m2-sensors-and-cooling.md) | 6 | Sensor coverage и физические cooling/RGB/media проверки |

Отдельно сохранён [старый план ENE6K77](2026-04-26-ene6k77-bootloader.md):
это исследование другого семейства fan controllers, вне текущего LCDV2 roadmap.
Его 83 старых unchecked шага не входят в таблицу активных задач и не считаются
завершёнными без отдельной сверки. Другие устройства отложены.

Все три плана содержат незавершённые требования и сохранены. Новых полностью
выполненных планов для удаления нет; десять устаревших планов убраны ранее.
В действующих планах нет закрытых checkbox-пунктов. Пункты M2 и базового плана
пересекаются:5+5+6 не является числом независимых задач или процентом готовности.

Последний [общий Realtime candidate](../../parity/evidence/2026-10-17-realtime-shadows.json)
содержит Windows Control/Screen/current-editor/private preview, final845-source
package и scoped software/native/package checks. Four Sensor video throughput
после text shadow optimization прошёл минимум20fps:23.40fps при nominal24. Кандидат не закрывает оставшиеся
platform/reference/hardware требования трёх планов; ни один план не удалён.

M4–M6 ведутся в [roadmap](../../../../roadmap.md), без дублирующих пошаговых планов.
[Общий чеклист](../../parity/basic-parity-checklist.md) содержит оставшиеся
критерии; [сводка](../../parity/implementation-status.md) — выполненное и evidence.
Исторические audits/specs/captures не являются активными планами.

Mini System Information направлен на виджет KDE Plasma и
расширение GNOME Shell (6 slots/FPS), без отдельного окна. Sway и остальные
окружения отложены; окончательный список поддержки и версии shell пока не
определены. Оконный черновик удалён из продукта; read-only CLI feed подключён,
начальные Plasma/GNOME adapters и opt-in MangoHud FPS source selection подключены; Plasma test host и GJS/GTK
проверены изолированно. GNOME panel/session и расширенная Plasma config/session
acceptance остаются. Далее updater/поставка. Приёмка editor/capture/RGB/profiles, оставшаяся
локализация и platform gates продолжают учитываться в общем checklist.
Дополнительные effects/Style Express — последними.
Подробности и зависимости — только в roadmap.

Screenshot и начальный ScreenRecord workflow уже подключены до явного Save.
ScreenRecord включает pause/resume, clear, multi-interval trim/crop, диалог
лимита и возврат из prepared review к редактированию. Synthetic native
source/review decode, seek, Save/membership и нормальное закрытие приложения
[проверены](../../parity/evidence/2026-10-12-screen-record-return-edit.json).
Полный 180-секундный лимит engine отдельно принят на synthetic source
([evidence](../../parity/evidence/2026-10-12-screen-record-full-limit.json));
native automatic-limit/Cancel/Import/Save принят; реальные portals/PipeWire
и расширенные limit errors/accessibility остаются непроверенными.
Synthetic release resource baseline 320×240/1080p/4K и отдельный полный180с
320×240 приняты; real-capture/worst-case и long large-source resources остаются
в [контракте ScreenRecord](../../parity/screen-record.md).
Record/Pause в видеоредакторе нарезает существующий файл.
Последующий [общий checkpoint](../../parity/evidence/2026-10-16-current-integrated-candidate.json)
прошёл на722 неизменных inputs: Rust/browser/strict checks, native workflows,
production package и GUI0/1mock. Аппаратного прогона не было.

Аппаратная приёмка отложена пользователем; после тестов оставлять LocalUI.

Языковая политика: English — основной язык/default новой установки, русский
обязателен. English default уже подключён с сохранением legacy Russian; EN/RU dynamic/native
qualification остаётся в базовом объёме; остальные языки отложены до полировки. При миграции
сохранять выбранный русский. [Контракт](../../parity/interface-language.md).
