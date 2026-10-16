# Profile persistence: failure и recovery

2026-10-10. Временные filesystem roots, синтетические данные и MockBackend.
Проверки не используют production daemon, пользовательские profiles или HID.

## Контракт

`ProfileStore::edit` удерживает общий lock, читает текущую библиотеку и
проверяет expected revision. Только после validation новая библиотека
записывается во временный файл в том же каталоге: write → file sync → rename
на `profiles.json` → directory sync. Профили не применяются к устройству.

Ошибка до rename оставляет прежнюю библиотеку. Ошибка после rename может
означать уже выполненную запись: клиент обязан перечитать revision и содержимое.
Повтор со старой revision не должен удалить или заменить успешно сохранённый
профиль. GUI требует refresh после ошибочного ответа и сохраняет черновик.

## Сопоставление проверок

| Требование | Текущая проверка | Граница |
|---|---|---|
| Старый файл при ошибке до rename | `profiles.rs` unit `interrupted_atomic_write_preserves_old_or_committed_library_and_recovers`: отказ после write и после file sync, reopen, отсутствие temp leak | Внедрённый возврат ошибки на границе, не ENOSPC внутри syscall |
| Неоднозначный результат после rename | Тот же unit: отказ перед directory sync, reopen новой revision, отклонение stale delete, успешное explicit recovery | Не доказывает durability при отключении питания |
| Независимые slots/groups, restart | `tests/profiles.rs`: save/rename/delete/reset, сохранение соседних записей | Store contract; без HID |
| Concurrent writers | `concurrent_editors_cannot_lose_an_update`: ровно один победитель общей expected revision | Общий store lock в одном daemon |
| Повреждённая/будущая schema | `corrupt_and_future_schema_are_never_overwritten` | Файл не заменяется пустыми defaults |
| Missing/corrupt assets, stale revision | `tests/theme_ipc.rs`, `tests/font_upload_ipc.rs`: отказ без перезаписи библиотеки; рабочая тема сохраняется | In-memory RPC + MockBackend |
| Потерянный ответ и доотправочный обрыв | Native save/rename/reset/delete lost replies и save pre-forward cutoff | Реальный Unix IPC fixture, не crash внутри fsync |
| Поздний ответ и смена канала | Browser profile matrix: pre-commit rejection, late success/error, pump→fan | Синтетический IPC response fixture |

[Native faults](evidence/2026-10-10-native-profile-mutations.json),
[browser response handling](evidence/2026-10-10-profiles-interrupted.json),
[group switch](evidence/2026-10-10-profile-precommit-group.json).

Настройки устройства имеют отдельный asynchronous writer с pending/error/retry
и final flush: [его acceptance](persistence-acceptance.md). Успешный profile
save нельзя использовать как доказательство записи device settings и наоборот.

## Оставшиеся границы

Process kill/power loss, реальные ENOSPC/IO errors внутри каждого syscall и
filesystem-specific durability не имитировались. Native GUI readback и
store stage tests дополняют друг друга, но не являются общим crash-тестом.
Независимость уже активных cooling/lighting policies при их собственных ошибках
остаётся в общей consumer matrix; сохранение profile slot не является Apply.

[Checkpoint](evidence/2026-10-10-profile-persistence.json): **586 Rust tests**,
fmt/strict Clippy/docs и scoped review прошли. Native/browser заново не запускались.

## Shared connection failure и native readback gate, 2026-10-11

`IpcClient` завершает все pending requests после обрыва reader. В GUI одна
сессия используется и для profile mutation, и для health polling. Если polling
также увидел обрыв, `Profiles.svelte` инвалидирует library по connected change;
ошибка старого generation исчезает. Это отличается от mutation-only failure,
когда library остаётся видимой, но needsRefresh запрещает операции.

Старый native harness принимал только `response dropped` в alert и не учитывал
второй корректный путь. Теперь он требует либо этот alert, либо исчезновение
ранее загруженного selector/library. В обоих случаях Load/Save недоступны,
успешного notice нет, черновик сохранён; затем проверяются точное содержимое и
revision daemon library, explicit readback и отсутствие повторного запроса.
Proxy counts остаются обязательными. Тип наблюдённого recovery записывается
в native result отдельно для rename/reset/delete/reject.

Browser matrix дополнена deterministic rejected-disconnect для всех четырёх
операций: отказ до commit → reconnect → поздний reject. Library revision
остаётся1, единственная mutation — первоначальная, alert старого generation
отсутствует, explicit refresh возвращает исходный slot.20 tests прошли.

Первый исторический native timeout содержал пустой alerts, но недостаточно
данных для доказательства точного порядка событий. Новый browser scenario
доказывает ошибочность универсального ожидания alert; это не ретроспективный
trace того запуска. [Новая проверка](evidence/2026-10-11-profile-readback-race.json).
