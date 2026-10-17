# Восстановление файлов импорта после process death

Baseline: Windows L-Connect3 2.1.28.0 / Galahad II LCDV2. Сверено2026-10-15.
Базовый durable protocol полных групп Screen/FanAndLighting и отдельных named
slots подключён к Finish и startup. Расширенная fault matrix остаётся открытой. Дополнительные эффекты остаются последними.
[Evidence](evidence/2026-10-15-import-publication-journal.json).

## Ownership и публикация

Каждая операция получает private `import-publications/<nonce>` с lifetime flock.
Manifest v1 связывает nonce, device persistence key, group, operation ID,
исходный target GroupRevision и source SHA. Для каждого PNG/name/font/media
хранятся kind/key/dev/inode; ownership link имеет вычисляемое имя `N.owned`.
Пути из manifest не используются. Схема, digest/key/nonce, duplicates, budgets
и regular file/directory types проверяются. Стартовая сверка откладывается с
ошибкой, пока scope другого writer удерживает lifetime lock.

Scoped candidates создаются внутри каталога операции. Anonymous media payload
по-прежнему освобождается ядром при process death. Полностью записанный/synced
candidate проходит fallible ownership callback: создать hard link через собственный
FD, sync файла/каталога, атомарно записать/sync manifest, затем опубликовать финальное
имя через `persist_noclobber` и sync asset directory. Hard link удерживает исходный
inode после смерти процесса. Observer/identity/link/manifest failure vetoes final
publication. Каталог не содержит произвольные пользовательские файлы.
[Предварительный барьер](evidence/2026-10-15-import-prepublication.json).

Named-slot manifest v2 связывает operation ID, global library revision, group,
slot, размер и SHA-256 исходного manifest. Он не использует device key или
искусственный full-group token. Manifest v1 полных групп сохраняет прежний формат.
Новые slot receipts содержат optional `source_sha256`; старые читаются без него.
Receipt без SHA не доказывает владение: recovery сохраняет файлы и diagnostic,
включая стадию preparing. Slot history хранится отдельно в
`slot-recovery-report.json`; Status сообщает о сохранённых файлах и необходимости
нового ID после проверки библиотеки. Recovery не редактирует слот и не делает Apply.
Декодированные raw payloads также находятся внутри owned scope обеих разновидностей.

Manifest ограничен1MiB/2048 records; лимит unresolved scopes64. Кандидаты скрыты
из обычных catalogs до решения. Существующие references/pins, offline settings,
slots, metadata rename и foreign replacement защищает общий live rollback.
Ошибка метаданных удаляет PNG только при совпадении его identity.

## Решение и recovery

- `preparing`: owner commit ещё не мог быть отправлен. Recovery удаляет только
  свои identity-matching unreferenced assets и owned staging.
- `armed`: записывается/sync-ится до передачи готовой группы на owner commit.
  Сначала завершается existing settings+profiles redo journal. Matching receipt
  означает сохранить assets; source/group/device/expected должны совпасть.
- Без matching receipt `armed` сохраняется консервативно: последние64 receipt
  могли быть вытеснены. Записывается durable `recovery-report.json` с operation ID,
  device/group и причиной. Сохранённые настройки не меняются, Apply из journal
  не выполняется. Readback по такому ID сообщает, что commit не подтверждён,
  файлы сохранены и Apply не повторялся. Report сохраняет историю retention;
  текущий commit status определяется owner receipt.
- `discard`: известный negative owner readback разрешил rollback. Проверяются
  durable references и ownership links; foreign/referenced файлы сохраняются.
- `keep`: решение сохранить файлы. Unconfirmed live outcome и Drop fallback
  сохраняют диагностическую запись **до** retirement журнала, поэтому она
  остаётся доступной даже после завершённой уборки scope.

Resolved directory переименовывается в `done-<nonce>`, sync-ится parent, затем
удаляются private links/candidates и снова sync-ится parent. Startup собирает
прерванный retirement, initial manifest/report temporary и повторяет recovery
idempotently. Unknown/malformed/symlink/binding/link-identity errors сохраняют
данные и блокируют стартовую сверку.

`PersistenceStore::new/open_with_root` выполняет сверку до возврата store.
`DeviceManager::spawn` также проверяет её перед созданием actors/replay, включая
embedded callers; при ошибке не запускает устройства и пишет diagnostic.
`new_with_root` остаётся конструктором для staging/fault fixtures: до доступа
к catalogs/replay нужны `recover_import_publications` либо manager startup.
Нормальный manager replay после успешного recovery использует текущие durable
settings, включая более новые изменения; импортный Apply отдельно не повторяется.

## Проверено программно

- [x] Настоящий SIGKILL дочернего процесса с новым store: private candidate до
  ownership callback, published `preparing`, durable `discard`, `armed`, `keep`
  и после реального settings+profiles+receipt commit. Destructors не выполняются.
- [x] Accepted assets/receipt сохраняются без нового commit; unreferenced
  unaccepted файлы убираются. Повторный recovery не меняет результат.
- [x] После64 реальных последующих owner commits старый receipt вытесняется:
  все assets сохраняются, появляется diagnostic, settings/revision не меняются.
- [x] Offline references и foreign replacements сохраняются; подмена ownership
  link, redirected root, unknown manifest и dangling report symlink дают ошибку.
- [x] Отказ записи manifest после создания hard link не публикует финальный PNG.
  Live unknown outcome и Drop сохраняют notice после retirement.
- [x] Interrupted scope creation/report write/retirement собираются; live scope
  не перехватывается. Corrupt recovery не запускает HID writes через manager.
- [x] Actual Unix IPC/MockBackend: unconfirmed import диагностируется без Apply/
  commit; accepted journal собирается до нормального theme replay, readback
  сохраняет runtime generation и profile revision.

## Оставшаяся приёмка и реализация

- [ ] Завершать процесс в каждой точной IO-границе: hard link и directory sync,
  atomic manifest write/persist/fsync, final asset persist/fsync, armed/enqueue,
  redo journal/device/profiles/ACK, keep/discard, retirement rename и каждый unlink.
  Текущие SIGKILL сценарии покрывают логические стадии; synthesized interrupted
  creation/retirement не является полным boundary fault injection.
- [ ] Расширить настоящие fsync/link/rename/ENOSPC/FD exhaustion/cross-filesystem,
  panic/hanging codec/filesystem и partial cleanup failures. Проверить большие
  media, record/scope/report limits и стоимость recovery на большом наборе refs.
- [ ] Дополнить contended multi-process/live writer, directory/root replacement,
  permissions, неизвестные версии и разные corrupted/missing-link permutations.
- [ ] Native GUI с abrupt desktop/daemon death, потерей Finish reply, preserved
  pending ID, recovery diagnostics, expired sessions и закрытием во время Apply.
  Физические USB/power/sleep/detach проверки отложены пользователем.
- [x] Named-slot durable ownership с отдельным global target/receipt, сохранением
  v1 manifest полных групп и чтением legacy receipts. Настоящий SIGKILL до/после
  commit, legacy receipt,64-commit eviction, repeated startup и Status diagnostic;
  live stale-CAS rollback и caller loss проверены программно.
  [Evidence](evidence/2026-10-15-slot-publication-journal.json).
- [ ] Windows ZIP interoperability — отдельный контракт; LCGROUP1 остаётся
  собственным форматом Linux.
