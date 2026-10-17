# GUI: IPC, состояния и восстановление

Сверка текущего кода 2026-10-11. Базовый критерий: пять страниц используют
IPC, draft/отправленная команда/applied/persistence/error не подменяют друг
друга, reconnect не применяет черновик автоматически, preview не доказывает
физический вывод. Полная accessibility и аппаратная приёмка — отдельные gates.

## Путь данных

`desktop/src/lib/api.ts::rpc` вызывает Tauri `daemon_call`;
`desktop/src-tauri/src/main.rs::daemon_call` проверяет whitelist и размер,
использует Unix IPC, ограничивает connect3s/request10s. При ошибке соединение
сбрасывается только если это всё ещё тот же client. Записи не повторяются;
при timeout требуется проверка applied state.

`desktop/src/routes/+page.svelte::refresh` сохраняет выбранный исчезнувший
ID, не перенаправляет Apply на другое устройство; offline readings становятся
unavailable. Selection/revision guards отбрасывают поздние ответы. Components
получают `deviceAvailable`, поэтому missing device блокирует controls.

| Страница/состояние | Проверка текущего поведения |
|---|---|
| Display: draft/Apply/Clear/load | `display.spec.ts`, `display-interrupted.spec.ts`, `reconnect.spec.ts`: captured submitted draft, disabled pending/offline commands, late success/error/load не стирают новые правки и не повторяют команды |
| Preview | `preview-recovery.spec.ts`, `live-preview.spec.ts`: single flight, stale results отброшены после draft/device/reconnect, explicit recovery; нет set/clear RPC. UI и alt text явно говорят, что preview не подтверждает физический вывод |
| Датчики и source editor | `electrical-sensors.spec.ts`, `sensor-defaults.spec.ts`: units и stable bindings передаются в LCD theme; selected source configuration через IPC, explicit Apply/reset/conflict/reconnect. Native controls reconnect отдельно подтверждает unavailable→valid с прежним ID |
| Cooling | `cooling.spec.ts`, `control-drafts.spec.ts`, `external-controls.spec.ts`: draft/preset локальны; target/policy в Apply; daemon applied обновляется отдельно; late preset/Apply не стирают новые правки |
| Lighting | `controls.spec.ts`, `lighting-interrupted.spec.ts`, `lighting-serialization.spec.ts`, `dynamic-rgb-reconnect.spec.ts`: manual/thermal/dynamic commands, error/retry, mutual exclusion, submitted snapshots, late answers и сохранение drafts |
| Settings | `controls.spec.ts`, `settings-library-reconnect.spec.ts`, `external-controls.spec.ts`: field-only patches, requested readback, untouched neighboring config, old-device replies отброшены |
| Persistence всех страниц | `persistence-status.spec.ts`, `save-state.spec.ts`: accepted/queued не равны saved; disk error показан отдельно, polling recovery без дополнительного Apply |
| Внешние изменения | `external-controls.spec.ts`: свежий readback не затирает draft; explicit Load берёт актуальное состояние без writes |
| Profile/assets workflows | `profiles-interrupted.spec.ts`, `profiles-group-switch.spec.ts`, `font-recovery.spec.ts`, cleanup suites: draft retention, failures, late replies и explicit recovery; отдельные acceptance документы сохраняют границы |

## Настоящий IPC в изолированном native приложении

Ниже сохранённые предыдущие прогоны WebKitGTK/MockBackend, не новый запуск:

- [Display reconnect](evidence/2026-10-10-native-display-reconnect.json): daemon stop/restart, draft retention и explicit Apply.
- [Остальные страницы](evidence/2026-10-10-native-controls-reconnect.json): offline controls, сохранённые drafts, readings unavailable/recovery; restart/applied.
- [Lost reply](evidence/2026-10-10-native-lost-reply.json): brightness66 принят daemon, ответ потерян; ровно1 mutation, readback66 и explicit recovery65.
- [Timeout](evidence/2026-10-10-native-write-timeout.json): brightness67 response удержан, настоящий10s deadline, ошибка вместо success, readback67, ровно1 mutation.
- [Keyboard/AT-SPI](evidence/2026-10-11-native-dynamic-invalid.json):400 trusted events,12 variants, five-page flows и invalid/correction.
- [Transport failures](transport-consumer-acceptance.md): independent LCD/cooling/RGB state и recovery.

Это сочетание browser fixtures, native Unix IPC и component tests. Оно не
означает native fault injection для каждой операции, физический USB recovery
или exactly-once commit после произвольного crash. Applied означает состояние,
сообщённое daemon после transport write, а не визуально подтверждённый LCD/LED.
Windows-dependent providers/typography, speech/live announcements, zoom/dialogs,
полный итоговый software checkpoint и hardware checklist остаются открытыми.

## Результат текущей проверки

`pnpm check`:0 errors/0 warnings. `pnpm exec playwright test`:111 passed
за3.5min,0failed. Headless Chromium, один worker, bridge fixtures.
[Логи и source hashes](evidence/2026-10-11-gui-state-acceptance.json).
Вместе с перечисленными native checkpoints это закрывает общий software
GUI IPC/reconnect/error/pending/applied критерий. Это не полный итоговый
прогон Rust/release/contracts/native на одном состоянии дерева.
