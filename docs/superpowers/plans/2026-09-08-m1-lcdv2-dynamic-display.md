# M1: оставшиеся проверки LCD runtime

Актуализировано 2026-10-12. Реализация transport/runtime завершена в рамках
диагностического slice; полный аппаратный lifecycle не закрыт.
[Сводка реализации](../../parity/implementation-status.md),
[результаты](../../protocol/captures/2026-09-08-lcdv2-m1.md),
[acceptance protocol](../../parity/lcdv2-hardware-acceptance.md).

## Активные задачи

- [ ] Физический USB detach/reattach (A-011): отложен пользователем. Проверить stable identity, replay settings и отсутствие возрождения старого diagnostic stream.
- [ ] Дополнить A-009 визуальной проверкой коротких stop/replace/restart; подтвердить финальный LocalUI. Машинный barrier уже проверен.
- [ ] Подтвердить видимое изменение яркости и четыре rotation; сохранение соседних полей уже проверено программно.
- [ ] Проверить cold boot и sleep/wake с выбранной политикой восстановления, затем повторить с media/composition M3/M4.
- [ ] Подтвердить firmware recovery после partial frame/write failure отдельным ограниченным сценарием; mock ошибки не доказывают физический decoder resync.

## Условия приёмки

Исторические JPEG/H.264 наблюдения и программные lifecycle checkpoints
сохранены в [сводке](../../parity/implementation-status.md) и
[M1 capture](../../protocol/captures/2026-09-08-lcdv2-m1.md).
Они не заменяют проверку нового media/composition пути на устройстве.
Legacy B/H.264 на FW<1.5 физически не проверен; перед тестами прочитать firmware.
Аппаратная приёмка отложена пользователем. После тестов оставлять LocalUI.
GIF/video editor и startup/offline остаются в [roadmap](../../../roadmap.md).
