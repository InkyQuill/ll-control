# Сохранение настроек: software acceptance

2026-10-10, временный settings root, MockBackend, JSON-RPC memory transport
и headless browser. Production daemon, HID и пользовательская конфигурация не использовались.

## Проверенный контракт

`patch_lcd_config` подтверждает принятую LCD transaction. Это не подтверждение
записи настроек на диск. `get_lcd_state.persistence` отдельно сообщает:

- `pending`: принятые writer изменения ещё не сохранены;
- `saved`: в этой сессии хотя бы одна запись завершила file + directory sync;
- `last_error`: последняя ошибка writer, очищаемая успешной повторной записью.

`saved=true` не отменяет более новый `pending` или `last_error`. GUI показывает
ошибку раньше pending, pending раньше сообщения о завершении записи.
`LcdState.last_error` относится к LCD transaction, а не к settings writer.

## Сквозные сценарии

`crates/llcontrol-daemon/tests/persistence_acceptance.rs`:

1. Сохранённая исходная конфигурация; directory вместо `<key>.toml.tmp` мешает
   новой записи даже под root. LCD Apply проходит через JSON-RPC и MockBackend,
   но прежний файл остаётся неизменным. Через RPC видны pending/error, без
   ложного saved и без LCD error.
2. Удаление препятствия позволяет автоматическому retry сохранить intent без
   нового Apply; status очищается. Независимое изменение rotation сразу перед
   joined shutdown сохраняется вместе с brightness и прежним fan PWM.
   Новый store и новый manager с другим mock device path восстанавливают
   brightness71 и **applied rotation90** по stable identity.
3. Если препятствие осталось до shutdown, final flush завершает teardown с
   pending/error. Старые bytes не меняются; следующий manager загружает
   последнюю успешную конфигурацию brightness35, а не несохранённую82.

`desktop/tests/persistence-status.spec.ts`: реальный DOM Settings с mocked IPC
проверяет Apply → pending → disk error → successful retry. Предыдущий saved
не скрывает более новый pending/error; draft сохраняется, дополнительной записи
от UI нет. Native WebKitGTK disk fault этим тестом не заявляется.

Существующие `tests/command_dispatch.rs` дополнительно покрывают debounce,
burst coalescing, независимые deltas, retry и final flush writer непосредственно.

## Границы

Это ошибка открытия временного файла, не инъекция ENOSPC на каждой стадии,
не сбой fsync/rename и не отключение питания. Код writer выполняет file sync,
rename и parent-directory sync; устойчивость конкретной FS и физический LCD
проверяются отдельно. Ошибка final flush не превращается в сохранение: после
остановки несохранённый intent в памяти утрачен, прежний durable файл остаётся.
Общий native acceptance и итоговый прогон всей базы всё ещё открыты.

[Итоговый checkpoint](evidence/2026-10-10-persistence-acceptance.json):562 Rust tests,
fmt/strict Clippy/docs, frontend check/build,26 contracts и1 browser case прошли.

## Тема следует за serial, а не USB-путём — 2026-10-11

`manager_lifecycle::theme_follows_serial_to_new_path_without_leaking_to_replacement_unit`
проверяет реальный manager/actor replay с MockBackend и временным store:

1. Устройство A получает тему с отдельным sensor ID и Fahrenheit formatting.
2. После joined detach другое устройство B занимает тот же USB-путь:
   source остаётся Idle, активной темы нет, новых JPEG packets нет.
3. B получает собственную тему; A возвращается на другом USB-пути и
   восстанавливает исходную композицию с неизменным binding. Полный JPEG batch
   совпадает с preview исходной темы, а не только с командой начала кадра.
4. После shutdown оба persistence keys содержат свои темы без перезаписи соседней.

MockBackend сохраняет историю по USB-пути: проверка B использует только
суффикс после joined teardown A. Первоначальный запуск без этой границы
ошибочно учитывал старые пакеты; исправлен тест, production код не менялся.
Проверка не доказывает physical reconnect/resync, доступность самого датчика
или уникальность serial реального firmware. Serialless ambiguity отдельно
покрыта существующим lifecycle regression.

[Evidence](evidence/2026-10-11-theme-identity-replay.json):5 focused lifecycle
tests, fmt/strict Clippy,602 Rust tests (0failed/ignored,92targets) и docs
прошли; companion review без замечаний.

## Cooling source и ручной PWM после restart — 2026-10-11

Новый IPC regression `motherboard_pump_replays_until_manual_mcu_override_survives_restart`
выполняет три запуска manager с независимыми MockBackend и одним временным store:

1. Manual fan73% и motherboard pump policy80%; successful applied и source.
2. Restart восстанавливает motherboard80%; явный MCU67% отменяет pump policy.
3. Restart восстанавливает MCU67% без возвращения motherboard policy; fan73%
   и отсутствие fan policy сохраняются на всех запусках.

Проверяются IPC state, реальные mock cmd138 source/duty и durable settings
после joined shutdown. Отдельные mock histories исключают ложное подтверждение
replay старыми пакетами. Готовность applied ожидается с timeout, без фиксированной
паузы, предполагающей успешное завершение replay.

Первоначальный regression завершился timeout: manual PWM после restart
отправлялся напрямую в HID, обходя cooling actor; IPC applied оставался пустым.
Manager теперь использует `CoolingCommand::Manual` для fan/pump replay.
Проверка диапазона, успешная запись, applied state и сохранение выполняются
одним владельцем. Активные policies по-прежнему исключают legacy raw replay.

[Результаты проверок](evidence/2026-10-11-pump-source-replay.json).
Это software MockBackend acceptance; реальные PWM/RPM/source и firmware
поведение не проверялись. Общий cooling checklist сохраняется до сверки
остальных критериев; повреждённые image/font на restart — отдельный остаток.
