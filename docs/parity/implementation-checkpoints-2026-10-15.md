# Архив checkpoints реализации: 2026-10-15

Исторические срезы; текущие остатки — в [сводке](implementation-status.md),
[roadmap](../../roadmap.md) и [чеклисте](basic-parity-checklist.md).

**Fan+lighting durable coordinator подключён к дереву устройства:** оба freeze/
preflight ACK → owner commit/receipt → оба silent install ACK → Apply. Текущие
manual/RGB edits не перезаписываются повтором ID; неверные параметры/stale CAS
отклоняются. Known HID failures отделены от committed desired intent. Lost client
reply не отменяет commit; unknown owner outcome удерживает оба actors до recovery/
restart. Shutdown после commit откладывает Apply и отклоняет queued imports;
settings owner живёт до завершения coordinator. Сохранены sensor/Dynamic bindings,
inactive RGB modes, global thermal history и neighboring LCD/slots.
Workspace1012 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Контракт](profile-groups.md), [current evidence](evidence/2026-10-15-profile-groups-coordinator.json).
Explicit reapply/recovery, legacy unset/explicit manual0, group transfer IPC/CLI/
GUI и расширенная lifecycle matrix ещё открыты. Предыдущие actor stages и barrier
checkpoints — в [истории](implementation-checkpoints-2026-10-14.md).

