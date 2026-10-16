# Asset cleanup: software acceptance

2026-10-10. Native WebKitGTK → Unix IPC → MockBackend, private temporary roots.
Ниже только проверенный объём; аппаратные устройства не использовались.

| Сценарий | Проверка | Evidence |
|---|---|---|
| Image orphan + profile/offline references | GUI scan/delete удаляет только orphan; active theme сохраняется; retained images рендерятся после restart | [Image cleanup](evidence/2026-10-10-native-image-cleanup.json) |
| Font orphan + profile/offline references | Два seeded fonts не импортированы в текущую сессию; GUI сохраняет их по ссылкам; bytes и image/text rendering проверены после restart | [Font retention](evidence/2026-10-10-native-font-retention.json) |
| Draft-only → session-only image/font | После удаления профиля scan защищает по одному asset черновика; после preview и возврата к активной теме — по одному asset сессии, без draft references | [Draft/session](evidence/2026-10-10-native-draft-session-retention.json) |
| Offline TOML повреждён после scan | Delete отклонён, stale preview сброшен, seeded bytes не изменены; restore→fresh scan→delete проходит | [Corrupt root](evidence/2026-10-10-native-cleanup-corrupt.json) |
| Corrupt profile JSON | Image Delete и font scan отклонены настоящим daemon; bytes не изменены; после восстановления explicit scan/delete проходит | [Corrupt profiles](evidence/2026-10-10-native-cleanup-profiles.json) |
| Partial unlink image/font | Один orphan удаляется, второй получает EACCES; GUI показывает subset и key; fresh scan/retry удаляет только остаток, active theme сохранена | [Partial retry](evidence/2026-10-10-native-cleanup-partial.json) |

Backend `tests/asset_cleanup.rs` также проверяет отказ при corrupt profile
JSON; browser tests `assets.spec.ts`/`font-cleanup.spec.ts` проверяют draft
keys и отчёты. Это не выдаётся за native coverage.

Перечисленная native cleanup matrix закрыта. Ошибка unlink внедряется через
`native-unlink-fault.c`, скомпилированный в private root и загруженный только в
`desktop-mock`. Production daemon/GUI не получают этот preload. Это synthetic
EACCES на уровне libc, не проверка реальных ACL или аппаратного устройства.
Итоговый совместный software прогон остаётся отдельным gate.
