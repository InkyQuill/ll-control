# Реализованное и границы проверки

Сводка обновлена 2026-10-16, рабочее дерево `lconnect-linux-m1`, ветка
`codex/m1-lcdv2-dynamic-display`. Baseline Windows **2.1.28.0**, Galahad II
LCDV2 **0416:7395**. Это сводка результатов, не очередь задач.
Коллекции, gallery membership и полный Screen reset реализованы в storage,
actor, IPC и GUI. Reset/cancel/LCD failure/reapply прошли native WebKitGTK
с MockBackend; sleep/reattach/restart и in-flight sleep проверены программно.
Открыты расширенная приёмка, editor/capture/effects и остальные функции roadmap.
Здесь актуальная сводка; исторические checkpoints находятся в связанном архиве.

**Firmware:** internal original-HID engine и process-local supervision готовы;
saved restart inspection доступен через read-only IPC/CLI даже без DeviceId.
Owned reconnect job после checked close подключена. Product Apply/live-status UI,
qualified resolution/recovery и restart policy ещё открыты. [Текущая граница и история](firmware-lcdv2.md).

**Последняя сверка планов:** три активных плана сохраняются (5/5/6 открытых
пунктов), полностью завершённых для дополнительного удаления нет. Roadmap и
индекс очищены от повторной истории прогонов.

**Mini переведён на shell-направление:** отдельное Tauri-окно, route, UI-кнопки,
commands/capability и привязка к GUI shutdown удалены из продуктового пути.
Подключён read-only `lconnectctl mini snapshot/watch`: versioned JSON/NDJSON,
cache-only sensors/defaults/display, deadline, offline и reconnect. Private Unix
regressions проверяют отсутствие mutation calls, raw labels и новое показание
после восстановления связи. **Начальный Plasma adapter подключён:** shared TypeScript-модель, шесть
настраиваемых slots, EN/RU, exact source/freshness и bounded Qt helper. Native
package из staged install прошёл private Plasma6.7.5/Qt6.12 test host с настоящим
CLI/synthetic Unix peer, EN/RU screenshots и reaping helper. Qt/Node/QML tests и
qml lint прошли. Начальный GNOME adapter также подключён: GJS helper, общий
renderer, GSettings и GTK preferences. Private GJS/GTK/dconf проверки прошли,
включая сохранение после отдельного процесса/session bus. GNOME panel/session
и расширенная Plasma config/session acceptance ещё открыты; это не real desktop/hardware acceptance. Sway и другие окружения отложены, список поддержки открыт.
[Контракт и остаток](mini-system-info.md).

**English default подключён:** новый store без файла и GUI без допустимого cache
начинают на en-US; обязательный русский сохраняется по явному выбору. Legacy v1
без language читается как ru-RU без изменения файла/revision; новые записи явно
содержат язык. Unit/CAS сохраняются. Private native прошёл fresh English →
Russian → GUI/daemon restart с Russian → English и image preview; пользовательский
draft не переведён. Первые две фазы0 devices/0 HID writes, третья — один fake
LCDV2 с одним mock startup write; физических устройств нет. Dynamic/native
полнота EN/RU остаётся открытой, остальные языки — на этапе полировки.
[Языковой контракт](interface-language.md).
Текущий срез:1099 Rust passed/3 ignored,337 browser passed,39 contracts,
frontend0 errors/0 warnings/build, strict workspace/native Clippy, rustdoc/fmt
и locale768/768 прошли.
[Текущий evidence](evidence/2026-10-15-mini-shell-feed-english-default.json).

**Язык интерфейса и English static catalog подключены:** optional ru-RU/en-US
в display CAS record, CLI partial updates, reactive resources/document lang и cache
последнего подтверждённого языка. Все768 explicit static keys переведены;
strict locale check проходит,882 English resources сохраняют параметры. Известные
статусы media/editor переведены, пользовательские имена и черновики сохраняются.
Три English browser media-сценария и полный browser suite333/333 прошли.
Frontend0 errors/0 warnings, build,39 contracts и strict locale check успешны. Private native прошёл RU→EN→RU,
GUI/daemon restart и joined Close; отдельная фаза с одним MockBackend LCDV2
проверила реальный image decode/rotate/480px preview без публикации.
Первые две native фазы:0 devices/0 HID writes; третья:1 mock HID startup write,
физические устройства не подключались. **English пока partial:**63 dynamic call
sites требуют проверки, EN/RU native dialogs/menus открыты. Остальные6 Windows
locales перенесены в полировку.
Последний Rust checkpoint1095/3 ignored относится к основе языка; этим срезом
Rust suite повторно не запускался. [Контракт и остаток](interface-language.md),
[текущий static report](evidence/2026-10-15-language-static-complete-coverage.json),
[catalog/media evidence](evidence/2026-10-15-language-static-catalog.json).

**Общие °C/°F подключены:** отдельный CAS/flock store и owned IO jobs,
IPC/CLI/GUI без устройства; readings и temperature fields используют общую
единицу, управление хранит canonical°C. Новые LCD sensor/cycle layers следуют
System; explicit°C/°F старых тем сохраняются. Runtime читает atomic unit один
раз на кадр, без повторного Apply, disk reads или перезапуска producer.
Raw40°C → pixels104°F и сохранность explicit°C проверены; Unix preview System
совпадает с explicit°F. Private native прошёл запись, Sensors, System preview,
GUI/daemon restart и joined Close с0 devices/0 HID writes. Workspace1095 passed/
3 ignored (151 suites),36 browser,39 contracts, frontend0/0/build, strict
workspace/native Clippy, rustdoc/fmt прошли. Расширенные editor/platform/hardware
проверки остаются; язык интерфейса не реализован этим срезом.
[Контракт и checklist](display-settings.md),
[evidence](evidence/2026-10-15-display-settings.json).

**System Information / Spec подключён:** read-only Linux inventory шести групп
(CPU/GPU/memory/monitors/motherboard/storage), bounded files/directories, validated
EDID/SMBIOS, explicit unavailable и источники полей. Typed IPC, `lconnectctl spec
[--refresh]` и GUI работают без устройства Lian Li. Кеш30s, один owned blocking
job и потеря caller проверены; динамические sensors/HID не используются.
Private native WebKitGTK/Tauri/Unix fixture принял категории, выбор двух GPU,
unknown DIMMs, refresh и joined Close при0 devices/0 HID writes.
Workspace1090 passed/3 ignored (150 suites),19 browser, frontend0/0/build,
strict workspace/native Clippy, rustdoc/fmt прошли на текущем срезе.
Полнота соответствующих Windows полей, доступность реального DMI/EDID/NVMe,
права, hotplug и desktop/platform accessibility остаются открытыми.
[Контракт и checklist](system-spec.md),
[evidence](evidence/2026-10-15-system-spec.json).

**Актуальный ScreenRecord:** начальный capture → pause/resume/finish → preview,
clear, ordered trim/crop, диалог лимита, review → edit → reprepare → явное Save
и gallery membership реализованы. Начальный synthetic Tauri/WebKit workflow
принят; числовой и визуальный capture region до записи и на паузе реализован. Реальные portals/PipeWire и
расширенная fault/crop/limit приёмка остаются. Полный 180-секундный engine
лимит принят на synthetic source; native automatic-limit/Cancel/Import/Save
также принят. Расширенные limit errors/accessibility остаются открытыми.
[Последний workflow checkpoint](evidence/2026-10-12-screen-record-return-edit.json).

**ScreenRecord recovery после ошибки подготовки принят:** отказ admission,
failed encoding и возврат к исходнику проверены в browser. Native WebKitGTK
прошёл one-shot command rejection → review cleanup → edit → повторная подготовка
→ явное Save/gallery и normal Close с synthetic source/MockBackend. Название и
черновик сохраняются; publishing не отменяется через этот путь. Browser37,
Svelte0/0, frontend/native build успешны. Реальные portals и расширенная fault
matrix остаются открытыми.
[Evidence](evidence/2026-10-13-screen-record-review-recovery.json).

**Desktop runtime/GUI/tray подключены:** native preferences/actual registration,
CAS/retry, draft-safe poll, qualified own tray item, Close→Hide/Show/host-loss
restore и bounded no-host startup fallback. Menu/GUI Exit и Tauri ExitRequested
идут через общий join всех workers + settings IO drain. Single-instance нормальный
запуск активирует существующее окно; --autostart duplicate не поднимает hidden UI.
Private watcher/menu native принял3 GUI starts и регистрацию/disable; media Close
reaped encoder/staging, control reset restart принят. Real file-lock/caller-loss
тест подтвердил settings IO ownership до shutdown. Реальные sessions/login/packages,
AppImage и расширенная failure/accessibility matrix остаются открытыми.
[Контракт](desktop-lifecycle.md), [evidence](evidence/2026-10-15-desktop-runtime.json).
Workspace:1081 passed/3 ignored;23 browser, frontend0/0/build, strict
Clippy (normal/native-smoke), rustdoc/fmt и native tray/media/reset приняты. Первый
media прогон имел AT-SPI timeout; повтор того же кода прошёл. Оба лога сохранены.

**Основа desktop preferences/autostart реализована:** versioned CAS/flock store,
owned XDG entry, durable intent до integration, отдельный actual/unknown status,
ошибка/retry и system Hidden override. Реальный GIO запуск с literal special
characters проверяет исходный executable/argv без shell в private D-Bus session.
Tauri/GUI/tray/lifecycle подключены в следующем checkpoint выше; реальная host
registration при разработке не менялась. [Контракт и checklist](desktop-lifecycle.md),
[evidence](evidence/2026-10-15-desktop-preferences.json).
Desktop all-targets:112 passed/1 existing180-second screen-record test ignored;
strict desktop Clippy/rustdoc/fmt прошли. Полный workspace/frontend/native GUI
в storage-only срезе повторно не запускались.

**GUI полного FanAndLighting reset подключён:** read-only preview defaults и
точного target от daemon, отдельное подтверждение, CPU temperature selection,
owned native group worker, persisted operation ID и receipt-first recovery.
Same-ID retry после потерянного ACK не делает новый commit/Apply; current explicit
reapply использует opaque runtime token. Browser reset/archive regressions и
actual Unix native worker/proxy с lost reply после commit проверены. Подробный
native/remaining checklist — [контракт](fan-lighting-reset.md),
[evidence](evidence/2026-10-15-fan-lighting-reset-gui.json).
Workspace: 1073 passed/3 ignored;19 browser tests, frontend check0/0/build,
strict Clippy/rustdoc/fmt и native reset/archive regression прошли. Native reset
включает два нормальных Close и новый desktop после daemon restart.

**Полный FanAndLighting reset подключён к daemon/Unix IPC/CLI:** defaults
Windows2.1.28 восстановлены для Fan Standard/Pump Quiet, Rainbow, topology/sync,
всех inactive manual modes и8 Linux fan/pump slots. Обе actor barriers и shared
atomic owner сохраняют receipt до runtime Apply; повторный reset не меняет более
поздние intent/slot edits. Actual Unix RPC и CLI проверили defaults, fallback на
missing source и synthetic CPU reading, current readback/reapply, stale/different
parameters, Screen/global preservation и restart. Расширенные native failure/lifecycle scenarios
остаются в приёмке. GUI/native baseline добавлен в checkpoint выше.
[Контракт](fan-lighting-reset.md),
[evidence](evidence/2026-10-15-fan-lighting-reset-core.json).
Workspace: 1070 passed/3 ignored; strict daemon/IPC/CLI Clippy, rustdoc и fmt прошли.

**Durable journal полных групп подключён:** private staging/lifetime lock,
owned hard links и bound manifest до публикации; preparing/armed/keep/discard,
startup receipt reconciliation и durable notice при неподтверждённом результате.
Recovery не повторяет импортный Apply; factory/manager проверяют его до replay.
SIGKILL, реальный commit и64 последующих commits/eviction, foreign/offline refs,
corrupt journals, отказ manifest write и actual manager/Unix RPC проверены.
[Контракт](import-publication-recovery.md), [evidence](evidence/2026-10-15-import-publication-journal.json).
Базовый named-slot durable path также реализован: отдельный global target,
source-bound receipt с чтением legacy, SIGKILL/restart/eviction, Status diagnostic,
known-negative rollback и caller loss. Расширенные IO-boundary/fsync/resource/native
faults остаются открытыми. Последний checkpoint —
[evidence отдельных слотов](evidence/2026-10-15-slot-publication-journal.json).
Workspace: 1067 passed/3 ignored; strict daemon/IPC Clippy, rustdoc и fmt
прошли. Native slot/Screen/FanAndLighting GUI с private portal/MockBackend прошли
normal Close/restart; journal scopes удалены, slot source digest подтверждён. Один ignored — entry point child fixture, который
запускают SIGKILL tests; дополнительные реальные проверки не пропускались.

**Подготовлен барьер перед публикацией файлов импорта:** ownership callback
PNG/name/font/media вызывается на synced private candidate до финального имени
и может veto публикацию. Ошибка захвата identity не публикует файл; metadata
failure сохраняет foreign replacement.9 focused ownership regressions и оба
native group GUI round-trip/restart прошли. Это prerequisite; durable ledger,
расширенная crash matrix остаётся открытой; базовый named-slot durable path
подключён в следующем checkpoint выше.
[Протокол](import-publication-recovery.md), [evidence](evidence/2026-10-15-import-prepublication.json).
Prepublication checkpoint: workspace1046 passed/2ignored; strict daemon Clippy/rustdoc и fmt прошли.

**GUI полных групп подключён:** в «Настройки» доступны Screen и FanAndLighting
с native file chooser, export без перезаписи, upload/Inspect/preview и отдельным
Apply. Readback/reapply используют сохранённый ID/device/group и opaque runtime
token; reload не повторяет Finish, draft сохраняется. Native WebKitGTK/private
FileChooser/MockBackend принял обе группы, изменение64→27→64, readback/reapply без
нового commit, normal Close и receipt/settings после restart. Actual IPC tests
также проверяют large font, no-overwrite, Prepared Cancel/shutdown и stale LCD
intent. Расширенные fault/lifecycle/streaming/real-portal и physical gates открыты.
[Контракт](profile-groups.md), [evidence](evidence/2026-10-15-profile-groups-gui.json).
GUI checkpoint: workspace1043 passed/2ignored; strict GUI Clippy/rustdoc, fmt, Svelte0/0,
frontend/native build,16browser и два native round-trip прошли.

**Явный reapply полной группы подключён к IPC/CLI:** opaque runtime token
приходит с readback/Finish, принадлежит actor instance и инвалидируется новым
intent, включая неуспешный setter. LCD использует atomic GroupState и checked
Begin/Apply; cooling/RGB — существующий coordinator. Повтор старого token не
делает Apply; saved newer edits запрещают старый receipt. Reapply не меняет
settings/profiles/receipt и использует общую owned runtime admission. Actual
RPC/DeviceManager/CLI проверили обе группы, Direct/fresh retry, invalid LCD
setter и caller loss при заблокированной asset preparation. Actor tests приняли
frame failure/retry, current receipt на fresh actor/ABA и queued composition
shutdown join. [Контракт](profile-groups.md),
[evidence](evidence/2026-10-15-profile-groups-reapply-transport.json).
Расширенная public fault/workload/lifecycle/physical приёмка остаётся; базовый GUI принят выше.
Reapply checkpoint: workspace1039 passed/2ignored; strict Clippy/rustdoc, fmt и standalone CLI check прошли.

**Live rollback публикации полной группы подключён:** callbacks создания
учитывают только собственные PNG/name/font/media, временные image/media скрыты
из catalogs. После ошибки только negative owner receipt readback допускает
удаление; accepted/unknown outcome сохраняется. Live/pending pins, offline saved
refs, foreign replacement/rename и параллельные uploads защищены. Ownership
descriptors закрываются до terminal Finish state. Actual stale Screen IPC с
image/font/video сохраняет legacy catalog и новую brightness52 через shutdown.
[Контракт](profile-groups.md), [evidence](evidence/2026-10-15-profile-groups-rollback.json).
Базовый durable ledger/startup recovery подключён в новом срезе выше. Extended
fsync/resource/panic/shutdown и named-slot ownership остаются открытыми.
Rollback checkpoint: workspace1035 passed/2ignored; strict Clippy/rustdoc и fmt прошли.

**Finish/Apply и durable readback полной группы подключены к IPC/CLI:**
Screen и FanAndLighting проходят готовые actor transactions с receipt до output.
Owned Finish переживает потерю соединения; Cancel/Release его не прерывают.
Используется общий runtime budget обычных LCD preparations. Repeated Finish читает
current receipt/runtime без Apply поверх новых PWM/LCD edits; stale CAS отвергается.
Actual Unix RPC/DeviceManager и CLI проверили manual0, static RGB, Screen/JPEG,
архив>1MiB/sparse video slot и readback после restart с утраченными sessions.
Storage commit отделён от runtime error. [Контракт](profile-groups.md),
[evidence](evidence/2026-10-15-profile-groups-finish.json).
Расширенная publication/public streaming/recovery/lifecycle приёмка и named-slot
durable ownership остаются открытыми.
Finish checkpoint: workspace1028 passed/2ignored; strict Clippy/rustdoc, fmt и standalone CLI check прошли.

**Upload/Inspect полной группы подключён к IPC/CLI:** файл передаётся по64KiB,
повторы chunks проверяются, target/group/size/revision привязаны к session.
Полный codec проверяет payloads в private quarantine; target gallery/settings/
receipts/PWM не меняются. Status доступен во время blocking validation; потеря
waiter и Cancel не освобождают reservation до выхода реальной работы. Actual
RPC/MockBackend и CLI проверяют invalid/corrupt/wrong-group, gallery-only image,
Screen>1MiB и отсутствие target mutations. [Контракт](profile-groups.md),
[evidence](evidence/2026-10-15-profile-groups-upload.json).
Расширенная transport приёмка ещё открыта; предварительная
проверка архива не означает импорт или Apply.
Upload checkpoint: workspace1026 passed/2ignored; strict Clippy/rustdoc, fmt и standalone CLI check прошли.

**Snapshot/backup export полной группы подключён к IPC/CLI:** Screen и
FanAndLighting читаются через settings owner; session связана с device/group/
revision, stale source отвергается, повтор Begin возвращает прежний export.
Packing/file reads выполняются в blocking workers, chunks до64KiB; CLI сохраняет
новый файл без overwrite. Private RPC и actual CLI/Unix socket проверили
Screen video>1MiB/gallery-only image, clean-store decode/последний кадр, explicit
manual0 и legacy Application source. Cancel сохраняет quota/admission до выхода
worker; catalog/staging не прерывается немедленно. [Контракт](profile-groups.md),
[evidence](evidence/2026-10-15-profile-groups-export.json).
Расширенная signal/fault/expiry/shutdown и real-portal/physical приёмка остаётся;
базовый native GUI принят выше.
Export checkpoint: workspace1024 passed/2ignored; strict Clippy/rustdoc, fmt и standalone CLI check прошли.

**Fan+lighting commit/Apply/reapply подключены к дереву устройства:** оба barrier/
preflight ACK → durable receipt → оба install ACK → Apply. Storage acknowledgement
отделён от runtime errors; повтор ID не применяет старый intent. Explicit reapply
требует current receipt и парный actor revision, защищает Direct/saved mutations и
restart ABA, не меняет settings/profiles/receipt. State и revision читаются атомарно
в каждом actor. Lost reply не отменяет accepted job; старый retry token не делает
второй Apply. Unknown durable/partial runtime outcome удерживает actors до recovery/
restart. Sensor/Dynamic policies, inactive modes, global thermal и neighboring LCD
сохраняются. Workspace1018 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Контракт](profile-groups.md), [current evidence](evidence/2026-10-15-profile-groups-reapply.json).
Legacy unset/explicit manual0 различаются: fan marker и pump source сохраняются
через private IPC/manager restart и group codec/Apply/reapply. Marker false не
меняет прежний portable JSON shape/hash. [Контракт/evidence](manual-pwm-zero.md). Manual0 checkpoint: workspace1021 passed/2ignored;
strict Clippy/rustdoc и fmt прошли.
Extended partial install/recovery/lifecycle остаётся открытой. История: [actor stages](implementation-checkpoints-2026-10-14.md)
и [coordinator](implementation-checkpoints-2026-10-15.md).

**Screen group runtime и LCD rotation подключены:** prepared group commit/Apply/
reapply выполняется LCD owner с durable receipt до config/frame и защитой поздних
edits. Final H264 frame поворачивается0/90/180/270 вместе с sensor overlays;
wire rotation всегда0, возврат к JPEG восстанавливает hardware angle.
Managed media сохраняет ID без synthetic theme в API, restart/group replay
использует desired angle. Live composition angle сохраняет worker/generation,
video phase и scene state; end-to-end frame budget ограничивает старую очередь.
Direct media0° передаёт reader после join и прогревает decoder от предыдущего
IDR, не отправляя старые кадры. Decoder/config failures восстанавливают direct
source на прежней позиции; shutdown после config ACK не начинает новый AU.
Private IPC проверяет identity/generation/counters и все четыре угла; actual
FFmpeg decoded pixels — направление, sensor updates и непрерывность video phase.
Workspace996 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Rotation contract](display-rotation.md), [current evidence](evidence/2026-10-14-display-direct-rotation.json),
[group contract](profile-groups.md). Реальное устройство здесь не проверялось.
Остаются expanded lifecycle/native gates; базовый GUI принят выше. Fan+lighting coordinator/barriers реализованы в следующем срезе выше. Предыдущие checkpoints сохранены в [истории](implementation-checkpoints-2026-10-14.md).

**GUI архива одного named slot подключён:** native FileChooser/file workers,
progress/Cancel и recovery по сохранённому ID работают без передачи payload в JS.
Импорт сохраняет черновик редактора; Load/Apply остаются явными. Workspace953
passed/2ignored, browser40, Svelte0/0, strict Clippy/rustdoc/fmt и build прошли.
Native WebKitGTK/Unix IPC/MockBackend с private FileChooser fixture принял
экспорт/import с remap image/font/cached video, readback, normal Close/restart
и preview импортированной темы после restart без LCD Apply. Исправлен Tokio
runtime crash GTK-команды; добавлена регрессия до admission.
Реальные portals/accessibility, расширенные native faults, долгий packing Cancel,
полный screen/fan+lighting group archive и Windows interoperability остаются.
[Контракт](profile-archives.md), [evidence](evidence/2026-10-14-profile-archive-gui.json).
Предыдущие storage/IPC результаты сохранены в
[исторических checkpoints](implementation-checkpoints-2026-10-14.md).

**Выбор Static/Dynamic по pump mode и restore подключены:** `modes[].enabled`
сохраняет выбор неактивных режимов. Явный `dynamic_mode=true` восстанавливает
последний source; false/omitted сохраняет прежний manual contract. Pump base,
active policy и registry сохраняются одной settings операцией. Missing source
даёт degraded/manual fallback; невалидный restore отклоняется до writes.
GUI разделяет выбор по режимам; Disable редактора источника обновляет parent.
Workspace927 passed/2ignored, browser43, strict clippy/fmt/rustdoc,
Svelte0/0/build прошли; финальный notice guard повторно проверен4 сценариями.
Native WebKitGTK/Unix IPC/MockBackend принял Meteor CPU ↔ Static generic host RPM,
automatic restore, Disable и restart с Meteor false/Static true и тремя bindings.
Expanded native faults/accessibility, совместная media/cooling/RGB и физическая
приёмка остаются. [Evidence](evidence/2026-10-14-dynamic-rgb-selection.json).

**Dynamic RGB параметры по mode/source/unit подключены:** ranges, low/high
palettes/speed и fixed brightness/direction сохраняются отдельно; optional
legacy fields наследуют manual base. GUI сохраняет черновики источников,
использует native defaults/semantic CPU Load и отклоняет поздние ответы.
Workspace925/2ignored, browser39, strict clippy/fmt/rustdoc, Svelte0/0/build
прошли. Native WebKitGTK/MockBackend принял CPU/GPU Meteor, generic host RPM
Static, applied readback, Disable и точный registry после restart. Выбор
Static/Dynamic по каждому режиму закрыт checkpoint выше;
физическая проверка не проводилась.
[Evidence](evidence/2026-10-14-dynamic-rgb-profiles.json).

**Fan↔pump sync/topology подключены:** manual8…50; sync SKU280→16/360→24
с сохранённым ручным значением. Настройка глобальная для fan zone, отдельно от
mode profiles. GUI/CLI/IPC → sole owner → persistence/restart и thermal sync-off/
restore реализованы. Неверные значения, write failure, legacy omission и
переключение устройств проверены. Исправлен delayed input change прежнего device.
Native sync280/360/manual и restart sync1/count16 с manual31 приняты с MockBackend.
Workspace922/2ignored, browser34, strict clippy/fmt/rustdoc, Svelte0/0/build прошли.
Физические LED/wiring и expanded native gates остаются. Параметры dynamic
mode/source закрыты checkpoint выше.
[Evidence](evidence/2026-10-14-fan-topology.json).

**Manual RGB catalog и отдельные mode profiles подключены:**6 fan/16 pump
controls/defaults подтверждены versioned numeric inventory. GUI разделяет
черновики device/zone/mode; reset не делает Apply. Применённые настройки режимов
сохраняются независимо и восстанавливаются после restart, неактивные modes не
запускаются. Native весь catalog, оба режима обеих зон, defaults/load/restart
приняты с MockBackend. Workspace917/2ignored, browser28 RGB +4 catalog,
strict clippy/fmt/rustdoc и Svelte0/0/build прошли. Параметры dynamic mode/source закрыты
checkpoint выше; расширенная native/аппаратная приёмка остаются.
[Evidence](evidence/2026-10-14-rgb-mode-catalog.json).

**Загрузка RGB базы в GUI подключена:** get_lighting_info читает текущую
manual base через sole owner без HID query. Явная кнопка восстанавливает mode,
всю палитру и controls без Apply. Unset/direct/Clear, failed replacement,
missing/error/retry, legacy black slots и late edit/zone/device ownership
проверены. Native загрузка обеих зон и restart приняты с MockBackend.
Workspace915/2ignored, strict clippy/fmt/rustdoc, browser7 focused +21 RGB,
Svelte0/0/build успешны. Dynamic per-mode/source parameters закрыты checkpoint выше;
расширенная native/аппаратная приёмка остаются открытыми.
[Evidence](evidence/2026-10-14-rgb-base-load.json).

**Полные ручные RGB палитры подключены:** GUI показывает0/1/2/4 color slots
по Windows2.1.28 catalog, CLI принимает повторный `--additional-color`. IPC,
единственный RGB owner и persistence сохраняют все цвета. 2/4-color wire output,
отказ неверной замены, restart и legacy/corrupt settings проверены программно.
Native WebKitGTK/MockBackend подтвердил RGB bytes обеих зон после GUI Apply и
restart. Browser21, strict clippy/fmt/rustdoc и builds прошли. Dynamic per-mode/source parameters закрыты checkpoint выше;
native/hardware gates остаются.
[Evidence](evidence/2026-10-13-rgb-mode-palettes.json).

**Полный Basic/Rain/Snow page-state принят программно:** независимые name,
background color/gradient/image/video, overlays/fonts и weather settings хранятся
в ограниченном `page_cache`. «Без эффекта» и удаление строки Weather возвращают
Basic, сохраняя эффект. Все ссылки защищены от cleanup/deletion; video leases
удерживаются через persistence. Native WebKitGTK/Unix IPC/MockBackend прошёл
preview/Apply → profile → restart с разными именами, цветами, overlays и cached
managed image. Workspace885 passed/2ignored, browser27 passed, strict clippy,
rustdoc, fmt, Svelte0/0, frontend contracts39 и build успешны.
[evidence](evidence/2026-10-13-lcd-page-state.json).
Остаются native video/fault matrix, Windows visual/defaults и аппаратная приёмка.

**MeteorShower подключён:** schema v3, собственный live renderer, общий H.264
owner, preview, persistence, GUI static speed/colors и source-specific dynamic
palettes/ranges, независимая четвёртая страница и slots/restart реализованы.
Native WebKitGTK/MockBackend preview/Apply/profile/restart и cached managed image
приняты; mock owner sleep/wake/Clear и live sensor/preview isolation проверены.
Workspace895 passed/2ignored, browser38+1 passed, strict clippy/fmt/rustdoc,
Svelte0/0, contracts39/build успешны. Аппаратная проверка отложена; native
background-video/fault matrix и Windows stochastic trail/raster/defaults остаются.
[Evidence](evidence/2026-10-13-meteor-integration.json).

**Snow3D подключён:** static intensity1…3, native mode parameters, собственный
perspective renderer, schema v3, общий H.264 owner, GUI и пятая независимая
страница. Native WebKitGTK/MockBackend preview/Apply/profile/restart принят;
mock sleep/wake/Clear и millisecond timeline/preview isolation проверены.
Workspace901 passed/2ignored, browser40 passed, strict clippy/fmt/rustdoc,
Svelte0/0, contracts39/build успешны. Windows sprite/raster/fade/RAF timing/camera
и native video/fault qualification остаются; аппаратная проверка отложена.
[Evidence](evidence/2026-10-13-snow3d-integration.json).

**Fireworks подключён:** шесть static/low/high цветов, source-specific ranges,
numeric oracle и stateful scene; цвета запущенных частиц сохраняются при смене
показаний. Schema v3, общий H.264 owner, preview, GUI PaletteEditor, шестая
независимая страница, slots/persistence/restart реализованы. Mock Apply → restart
→ sleep/wake → Clear и live sensor/preview isolation приняты. Native WebKitGTK
с MockBackend проверил preview/Apply/profile/restart, пять cached pages и stream
после закрытия GUI. Workspace911 passed/2ignored, browser43 passed, strict
clippy/fmt/rustdoc, Svelte0/0, contracts39/build успешны. Visual/trails/timing,
native video/fault/recovery и hardware qualification остаются открытыми.
[Evidence](evidence/2026-10-13-fireworks-integration.json).

**Приоритет изменён пользователем:** перенос оставшихся16 effects/Style Express
отложен до завершения основного функционала. Уже подключённые effects сохраняются;
их текущая программная приёмка не означает full visual или hardware parity.

Исторические checkpoints ранних фаз и отдельных прогонов вынесены в
[архив](implementation-checkpoints-2026-10-14.md). Они сохранены целиком и не
переоткрывают уже выполненную работу. Приёмка относится к указанному snapshot,
не ко всем последующим изменениям.

[Plasma adapter checkpoint](evidence/2026-10-15-mini-plasma-adapter.json):
исходники, строгие TS/C++/QML проверки, staged runtime и EN/RU screenshots.

[GNOME adapter checkpoint](evidence/2026-10-15-mini-gnome-adapter.json):
GJS transport и GTK preferences, private dconf persistence; Shell runtime не проверен.

[Создаваемые имена и описания раскладок](evidence/2026-10-15-language-created-defaults.json):
My screen / Мой экран, Screenshot.png / Снимок экрана.png, Screen recording /
Запись экрана и app-owned font fallback. Смена языка сохраняет введённые имена и
LCD text; layouts сохраняют provider labels. Private native подтверждает новые
имена EN/RU и restart; реальные capture portals остаются открыты.

[EN/RU UI архивов](evidence/2026-10-15-language-archive-ui.json): phases, group labels,
progress accessibility и cooling summaries подключены к явным ресурсам; import
receipt notice обновляет язык без нового импорта. Пройдены private native
named-slot/full-group round-trips EN/RU с restart/readback. Native chooser metadata далее локализованы отдельным срезом; прочие worker
errors и настоящий desktop picker остаются отдельным language gate.

[Native archive chooser EN/RU](evidence/2026-10-15-language-native-archive-chooser.json):
типизированный UI language в native Start; profile/group import/export title/filter
и app-owned file grant errors выбираются явно. Private D-Bus проверяет metadata,
URI grant/cancel и отсутствие передачи path/bytes из GUI. Реальный DE picker и
остальные worker diagnostics остаются открыты.

[Structured archive notices](evidence/2026-10-15-archive-structured-notices.json):
восемь native-кодов и два UI pending guards; Rust error/worker → EN/RU render,
opaque diagnostics и без угадывания кодов из строк. COW/invalid input guards
сохранились. Native slot acceptance включает existing-destination rejection и
следующий успешный import без stale notice. Остальные diagnostics не заявлены
как полностью переведённые.

[Firmware identity checkpoint](evidence/2026-10-15-firmware-identity.json): исправлен
двойной cmd134 и date=PN0. Один actor-owned query классифицирует PN0/PN1;
optional build_datetime/CLI unavailable сохраняют неизвестную дату неизвестной.
[Updater contract](firmware-lcdv2.md) фиксирует Windows source route, ownership,
image/ACK/reconnect dependencies и незакрытую hardware acceptance. Flash-команд
этот срез не добавляет и не выполняет; safe updater ещё отсутствует.

[Firmware GUI preflight checkpoint](evidence/2026-10-15-firmware-gui-preflight.json):
Settings явно читает version/raw date, показывает unavailable и сохраняет raw token
для диагностики. EN/RU, повторные нажатия и late-result guards при смене target,
соединения и вкладки проверены в browser; private Tauri → daemon → MockBackend
проверяет actual bridge/IPC с одним cmd134 и без cmd10/11. Физический readback,
streaming latency, native keyboard/pointer/accessibility и safe updater остаются открыты.

[Firmware image research](evidence/2026-10-15-firmware-image-research.json): найден
официальный catalog route, скачан кандидат1.6/626212bytes с matching MD5 и закреплённым
SHA-256. Offline inspector принимает только exact bytes, без network/HID/flash.
Статический анализ этого образа уточняет chunk echo, error response и activation
branches; это не аппаратно подтверждённые ACK/recovery. Safe updater остаётся открытым.

[Firmware ACK/error codec](evidence/2026-10-15-firmware-ack-codec.json): отдельный
FW1.6 parser проверяет512-byte chunk echo против фактического B/cmd11 request,
отличает cmd2 device errors и raw activation extra flag от reboot/version success.
190 protocol tests, workspace strict Clippy и warning-free protocol rustdoc прошли.
No nonce/unechoed payload limits зафиксированы тестом; update owner и recovery ещё
не реализованы, аппаратные команды не выполнялись.

[Firmware owner transport](evidence/2026-10-15-firmware-owner-transport.json): internal
HID actor transaction для exact candidate/current identity1.6, one request at a time,
permanent attachment fence, no retry, typed partial/attempted progress и joined
cancel/shutdown. MockBackend проверяет corruption/loss/rejection/detach/partial write,
FIFO/pending sensor, drop requester и отсутствие queued I/O. Product updater пока
не подключён: coordinator должен остановить keepers и удержать manager replay до
проверки вернувшегося target/version; durable lifecycle/recovery остаётся.

На этом source snapshot:1133 Rust tests passed/4ignored; отдельный ignored vendor
fixture запущен явно и прошёл619chunks+activation на MockBackend. Workspace fmt,
strict Clippy (включая GUI native features) и warning-free workspace rustdoc прошли.
Нового browser/native GUI или hardware acceptance в этом срезе нет.

[Keeper quiescence](evidence/2026-10-15-keeper-quiescence.json): stop/join sensor,
cooling, LCD, RGB, control group и settings dispatcher теперь отделён от HID/root
shutdown. Joined producers оставляют исходный handle живым; dropped waiter не
отменяет завершение, root detach/shutdown сохраняет полный teardown. Barrier не
заменяет save/cleanup health, cooling hold или IPC admission gate. Coordinator,
durable outcome и manager reconnect/replay verification ещё открыты.

Текущий keeper lifecycle snapshot:1139 Rust tests passed/5ignored; отдельно
запущен vendor-fixture integration — quiesce и621reports на том же MockBackend
handle,11.26s. Workspace fmt/strict Clippy/native-feature Clippy/rustdoc прошли.
Новых GUI/hardware acceptance результатов нет; активные планы остаются открыты.

[Checked cleanup/save preflight](evidence/2026-10-15-quiesce-health.json): LCD stop
ошибки сохраняются после Source→Idle и передаются с active/queued workers до
keeper barrier Failed. Missing report/completion остаётся ошибкой.
`DeviceHandles::quiesce_checked()` различает typed Cleanup и Persistence failures;
проверяет final save_state после joined dispatcher. Global admission/cooling hold,
durable coordinator и manager reconnect/replay gate по-прежнему открыты.

На checked-health snapshot:1141 Rust tests passed/5ignored; vendor fixture отдельно
прошла checked quiesce→621reports на одном MockBackend handle (11.25s). Workspace
fmt, strict Clippy/native features и warning-free rustdoc прошли. Нового GUI или
hardware acceptance нет.

[Firmware requested cooling hold](evidence/2026-10-15-firmware-cooling-hold.json):
internal transfer явно выбирает PumpOnly либо PumpAndControllerFan; перед chunks
запрашивает MCU pump100% и только при owned fan выборе fan100%. Write-only stage
не выдаётся за подтверждённый RPM. Partial/cancel даёт Unknown и не запускает flash
или restore. Hold не меняет persisted cooling policy. Product UI/admission/durable
coordinator/reconnect gate всё ещё открыты.

Hold snapshot:1144 Rust tests passed/5ignored; обе vendor Mock fixtures отдельно
прошли622reports с PumpOnly, checked actor-tree путь сохранил исходный motherboard80%
intent. Workspace fmt/strict Clippy/native features/rustdoc прошли. Аппаратных
и новых GUI acceptance результатов нет.

[Firmware operation journal foundation](evidence/2026-10-15-firmware-operation-journal.json):
atomic no-clobber arm, durable effects/terminal records, validated restore snapshot
и pre-open manager attach/replay quarantine. Corrupt state не заменяется, другой
serial/path не считается success. Живой actor не evict-ится от публикации; replug
и новый manager удерживаются. Transport/journal binding, global admission и
probe-only verified resolution ещё не подключены; product updater отсутствует.

Journal snapshot:1152 Rust tests passed/5ignored, включая8новых journal/manager
cases. Workspace fmt, strict Clippy/native features и warning-free rustdoc прошли.
Существующий transfer fixture заново не запускался: execution wiring journal ещё
отсутствует. Нового GUI/hardware acceptance нет;3активных плана остаются открыты.

[Firmware journal execution binding](evidence/2026-10-15-firmware-journal-binding.json):
mandatory owned token и store scope в HID update; target/hold/file checks до query,
fsynced effects marker до PWM и final outcome до reply. Failure commit остаётся
Unknown даже после всех ACK. Low-level unscoped actor не выполняет update.
Global admission, downgrade/missing-file anchor, reconnect verifier/coordinator
и product UI всё ещё открыты.

Binding snapshot:1157 Rust tests passed/5ignored; обе pinned Mock fixtures отдельно
прошли622reports и durable final outcome. Workspace fmt, strict Clippy/native
features и warning-free rustdoc прошли. Новых GUI/hardware acceptance нет.

### Firmware device-settings anchor — 2026-10-15

До первого hold/flash internal transfer сохраняет fsynced guard в canonical
settings TOML; текущие saved cooling/LCD intent сохраняются. Store scope делит
существующий state gate с persistence/profile transactions. Ordinary writes
и group transactions не могут убрать/перенести guard. Manager до HID open
отказывает и при отсутствующем journal с изменённым serial/path. Старый полный
serde reader с deny_unknown_fields принимает обычный TOML и отвергает guarded.
Guard остаётся после terminal outcomes; resolution API не добавлен.
[Checkpoint](evidence/2026-10-15-firmware-settings-anchor.json).

Global admission, fresh owner snapshot/assets, qualified no-effects/verified
resolution, probe-only reconnect verification и product updater EN/RU остаются.
Armed-only pre-effects stage ещё использует journal без settings anchor.
Ни этот gate, ни mock ACK не означают hardware update qualification.

Verification: workspace1161passed/0failed/5ignored; current daemon lib209passed/3ignored;
12journal regressions, pinned same-handle622report fixture; fmt, strict workspace/native
Clippy и warnings-denied workspace rustdoc passed. Hardware/native GUI не запускались.

### Internal firmware reconnect probe — 2026-10-15

Добавлен probe-only helper: process-local observed absence → unique same serial,
новый handle/bounded drain/один cmd134, exact1.6 identity с optional date,
неизменные journal/guard и повторная enumeration. До наблюдённого отключения HID
не открывается. Ошибки/duplicate serial/Unknown/serial-less остаются quarantined;
helper не запускает keepers, restore, reflash или clearing API.
[Evidence](evidence/2026-10-15-firmware-reconnect-probe.json).

Global IPC/preparation job exclusion и joined old HID owner — обязанности будущего
coordinator, не свойства этого helper. Qualified resolution, serial-less recovery,
пропущенный fast reset и EN/RU product updater остаются. Успешный identity probe
не равен installed image hash или hardware qualification.

Verification: final snapshot1167 Rust tests passed/0failed/5ignored in157groups;
6probe cases +12journal regressions included. Strict workspace/native-feature Clippy,
fmt и warnings-denied workspace rustdoc passed. No hardware/native GUI run.

### Firmware ordinary-work admission — 2026-10-15

Shared process-local Admission входит в PersistenceStore и связывает root RPC,
основные capacity pools, media preparation/finite exports, archive/group workers,
owned monitors/cleanup iterations, display/spec, manager discovery и power.
Seal атомарен с enter; caller/drain waiter cancellation не открывает gate и не
скрывает enrolled worker. Shutdown доступен. Product close/Apply/reopen API нет.
[Checkpoint](evidence/2026-10-15-firmware-admission.json).

Publication/file/data async Drop cleanup ещё не включён; требуется explicit
staging/publication close-and-join, idle session invalidation и bounded drain.
Keepers/HID teardown, fresh snapshot/assets, power cancellation и полная update
job/reconnect/resolution/UI остаются. Active0 не является общей quiescence.

Verification: final unchanged snapshot1174 Rust tests passed/0failed/5ignored
in158groups; targeted admission10passed, manager8passed,32/32sleep/wake repeats.
Fmt, strict workspace/native-feature Clippy и warnings-denied workspace rustdoc passed.
Intermediate stalled manager fixture не засчитан; test-only gate hardened,
исходный trigger не установлен. Hardware/native GUI не запускались.

### Firmware lifetime cleanup owners/staging close — 2026-10-15

Publication и file-backed archive/group/media upload entries владеют admission
lease до окончания Drop cleanup. Worker/monitor ownership сохраняется после
seal и caller/waiter cancellation; I/O/panic cleanup health sticky и checked.
Unique Maintenance capability привязан к исходному store gate. Internal
close_staging запрещает/очищает upload registries и отменяет finite export
lazy reader независимо от polling receiver; чтение без prefetch, final-frame/pin
contract сохранён; underlying blocking reads удерживают
свой owner до фактического завершения. Settings/library/assets не сбрасываются.
[Checkpoint](evidence/2026-10-15-firmware-cleanup-owners.json).

Полного updater ещё нет. Coordinator должен владеть Maintenance, close staging
и join keepers до checked drain, проверить unresolved imports и fresh snapshot/
assets, затем владеть original HID transfer/reconnect/qualified resolution.
Bounded cancellation, shutdown/power и расширенная fault matrix остаются.

Verification: final unchanged snapshot1183 Rust tests passed/0failed/5ignored in158groups;
9new tests. Existing2media-export integrations pass without assertion changes.
Fmt, strict workspace/native-feature Clippy и warnings-denied workspace rustdoc passed.
Первый prefetch вариант дал2media-export failures; заменён lazy reader, failed run
сохранён в evidence. Hardware/native GUI не запускались.

### Firmware serialized manager handoff — 2026-10-16

Добавлены manager queue и scope-bound take_for_firmware: выполнение после текущего
poll/attach/replay, non-Clone ownership target/peers вне ordinary map с исходным
DeviceInfo/HID. Checked quiesce ждёт всех peers и target, в том числе при save
failure. Close/drop/requester loss не возвращает устройства в replay; manager
держит root stop/teardown receipts и joins handed-off HID при shutdown независимо
от жизни owner/close waiter. [Checkpoint](evidence/2026-10-16-firmware-manager-handoff.json).

Шесть обычных Mock cases и отдельный pinned-image fixture проверяют этот путь.
Staging/drain/fresh snapshot/assets, bounded orchestration, import recovery,
probe/resolution/restoration и product UI EN/RU ещё не соединены в единый updater.
Hardware qualification остаётся отложенной; no automatic reopen/reclaim.

Verification completed2026-10-16 Europe/Moscow: unchanged source1189 Rust tests
passed/0failed/6ignored in159groups. Six new normal cases; external pinned manager
fixture separately passed622reports/619chunks on one original handle with durable
AwaitingReconnect. Fmt, strict workspace/native-feature Clippy и warnings-denied
workspace rustdoc passed. Hardware/native GUI не запускались.

### Firmware owned preparation/transfer engine — 2026-10-16

Internal begin_firmware_update соединяет scoped maintenance/serialized handoff,
staging close, checked keeper join/drain, fresh settings+asset ownership, bound
journal arm, original HID transfer и joined close. Scope mismatch отвергается
до seal; drop observer не abort owned job. Cancel, замеченный preparation, не arm-ит operation; поздний до hold может
оставить NotStarted journal, после hold — Unknown с durable quarantine. Snapshot берётся после accepted intent
flush; changed saved intent запрещает marker/hold. No restore/reopen/retry.
[Checkpoint](evidence/2026-10-16-firmware-owned-coordinator.json).

Product RPC/Tauri route не добавлен. Durable job/status/recovery registry,
panic/power/shutdown supervision, extended deadlines/fault matrix, probe и
qualified resolution/restoration/restart policy, UI EN/RU остаются. Outcome
AwaitingReconnect не является firmware success/hardware qualification.

Verification: unchanged Rust snapshot1194 tests passed/0failed/7ignored in160groups;
5new normal tests. External pinned coordinator fixture separately passed4modes
(success, observer loss, pre-hold cancel, post-hold cancel), two619chunk transfers
and one original handle per case. Fmt, strict workspace/native-feature Clippy
и warnings-denied workspace rustdoc passed. Hardware/native GUI не запускались.


### Firmware retained job и shutdown supervision — 2026-10-16

Process-local registry сохраняет status/progress/cancel и completion report одной
job; reattach/repeated wait работают после observer loss. Shutdown сериализован с
begin, явно отменяет и ждёт job, cancellation waiter не abort-ит supervisor.
Main вызывает этот join до manager shutdown. Worker panic переводится в Unknown
после staging/root/tracked cleanup join, без удаления journal/settings quarantine.
Recovery authority не удерживает normal drain count. Restart recovery, extended
fault/power/deadline policy, probe/resolution и EN/RU product UI остаются.
[Checkpoint](evidence/2026-10-16-firmware-job-supervision.json).

Verification:1198 Rust tests passed/0failed/7ignored in160groups,9targeted coordinator
tests и отдельный pinned Mock fixture4modes (two619chunk transfers) passed. Frozen
358 Rust/root Cargo files unchanged across checks. Fmt, strict workspace/all-target
and native-smoke/custom-protocol Clippy, warnings-denied rustdoc passed. Hardware
and native GUI не запускались; три незавершённых плана сохранены.


### Firmware saved restart inspection / Unix IPC / CLI — 2026-10-16

Read-only reconstruction journal/settings guards доступна через
`get_firmware_recovery({})` и `lconnectctl [--json] firmware-recovery`, без DeviceId
и при sealed ordinary admission. Typed states различают journal-only, matching
quarantine, orphan guard, mismatch и unavailable/corrupt state; saved progress не
становится live progress или firmware success. Общий state mutex используется без
redo, no-follow bounded reads не создают/исправляют файлы. Два worker permits
остаются у blocking readers при потере caller. Apply/probe/clear не добавлены.
[Checkpoint](evidence/2026-10-16-firmware-recovery-inspection.json).

Durable restart action policy, owned reconnect/probe/resolution, live-status/product
UI EN/RU и расширенная fault matrix остаются. Mini Shell и языковая политика не
меняются; hardware/native GUI в этом checkpoint не запускались.

Verification:1204 Rust tests passed/0failed/7ignored in162groups;6new tests include
real CLI → Unix IPC → daemon with sealed admission and no device. Frozen386Rust/
Cargo files unchanged across fmt, strict workspace/all-target and native-feature
Clippy, warnings-denied rustdoc and workspace tests. Все проверки прошли; hardware
and native GUI не запускались. Три незавершённых плана5/5/6 сохранены.


### Firmware owned reconnect job — 2026-10-16

Acknowledged transfer + checked original close создают одноразовый reconnect owner
с привязкой backend/scope/target/operation. Explicit internal begin запускает retained
blocking probe job; observer loss и cancelled shutdown waiter не отрывают worker.
30s observation window/200ms cadence, actual absence → same serial/fresh cmd134;
новый path без observed absence не принимается. FreshIdentityObserved — historical
read-only evidence, quarantine и original AwaitingReconnect report сохраняются.
[Checkpoint](evidence/2026-10-16-firmware-owned-reconnect.json).

Product probe/live-status/Apply UI EN/RU, qualified resolution/restoration и
restart/serial-less/missed-reset policy остаются; аппаратная приёмка отложена.

Verification:1209 Rust tests passed/0failed/7ignored in162groups;14targeted coordinator
tests include5new owned-probe cases. External pinned Mock fixture passed4modes,
two619chunk transfers and one end-to-end fresh reconnect query after original close.
Frozen387Rust/Cargo files unchanged across fmt, strict workspace/all-target and
native-feature Clippy, warnings-denied rustdoc and workspace checks. Hardware/native
GUI не запускались. Три незавершённых плана5/5/6 сохранены.


### Zenpower SVI2 Core voltage / CPU temperature — 2026-10-16

Collector читает существующий hwmon `zenpower`: exact in1/SVI2 Core label в mV,
Tdie/Tctl/Tccd temperatures в Celsius. SoC/VID/unknown rails не принимаются за CPU
voltage. Существующие роли, source picker, cache freshness и realtime LCD renderer
используются без новых polling loops, установки модулей или автоматической смены
bindings. Stable physical ID, missing/recovery и stale формат проверены fixtures;
cache→IPC→LCD JPEG показывает0.90V, unavailable и восстановление без изменения темы.
[Checkpoint](evidence/2026-10-16-zenpower-sensors.json), [ABI/границы](sensor-provider-inventory.md).

Остальные voltage platform gaps и Windows-equivalent Intel aggregate остаются;
отдельные engines не объявляются whole-GPU load. Hardware accuracy/driver cadence
и native source-selection acceptance нового provider отложены.

Verification:1211 Rust tests passed/0failed/7ignored in162groups;29host-sensor tests
include2new zenpower cases and expanded IPC/LCD JPEG freshness acceptance. Frozen
387Rust/Cargo files unchanged across fmt, strict workspace/all-target and native
feature Clippy, warnings-denied rustdoc and workspace tests. Hardware/native GUI
не запускались; три незавершённых плана5/5/6 сохранены.


### Opt-in MangoHud FPS / LCD / Mini selectors — 2026-10-16

Явный per-frame logfile source читает только новые records после warmup; mean
по frame durations, session ID по canonical path/dev/inode, unknown schema,
malformed/duplicate/overflow/missing input не становятся valid0. Unit FPS проходит
shared cache/IPC/custom LCD preview. Mini Plasma/GNOME имеют EN/RU explicit source
selector; no shared GPU fallback. Runtime capture enable/disable и exact Windows
PresentedAvg/application semantics ещё не закрыты. [Контракт](fps-source.md),
[checkpoint](evidence/2026-10-16-mangohud-fps.json). Реальные игры не запускались.

Verification:1215 Rust tests passed/0failed/7ignored in162groups,4new provider tests,
expanded cache/IPC/LCD JPEG and4shared-model cases passed. Frontend check/build,
39theme contracts, strict locale check, fmt/strict workspace+native-feature Clippy
and warnings-denied rustdoc passed. Qt/QML3ctest groups and private GJS/GTK feed/
preferences passed, включая FPS keyboard selection/Save/readback. Frozen417sources/
manifests unchanged. Реальные игры, hardware и полные shell sessions не запускались;
три незавершённых плана5/5/6 сохранены.


### Runtime FPS settings / manager shutdown race — 2026-10-16

Основное Settings и CLI сохраняют read enable/disable/absolute log path без
DeviceId и без restart daemon. EN default/RU, explicit Save/Load, revision CAS,
conflict/lost reply и сохранность draft проверены. Saved disable имеет приоритет
над legacy environment после restart. Live watch меняет reader; FPS-only cache
boundary отзывает и запоздалые старые samples, не трогая CPU/GPU.
[Контракт](fps-source.md), [checkpoint](evidence/2026-10-16-fps-runtime-settings.json).
Capture activation/app selection, exact Windows semantics и game qualification
остаются открытыми. Mini — только Plasma widget/GNOME Shell extension, другие
окружения и дополнительные языки отложены.

Первый общий прогон остановлен после debugger-подтверждения старой гонки: запрос
после manager shutdown мог попасть в уже закрывающийся канал и бесконечно ждать
reply. Command wait теперь наблюдает manager completion; deterministic regression
удерживает reply sender после завершения manager. Исходный lifecycle case прошёл
16 повторов. Это не аппаратная проверка сна/USB.

Verification:1221 Rust tests passed/0failed/7ignored in163groups;6new Rust tests
(4store/runtime,1real CLI/IPC,1manager regression). Browser13passed, включая3FPS
flows; Svelte0errors/0warnings, build,39theme contracts,800/800EN static resources,
fmt, strict workspace/native-feature Clippy и warnings-denied rustdoc passed.
Frozen453source/manifest/script files unchanged at final verification. Final native
WebKit/Tauri acceptance прошла2phases с restart GUI+fixture-daemon, EN→RU, enable/
disable и Cyrillic path retention; zero devices/HID writes, CSV unchanged. Private
Xvfb/session D-Bus only; screen-reader/full shell/game/hardware checks не выполнялись.
Три незавершённых плана5/5/6 сохранены.


### ScreenRecord owned child / native 180s — 2026-10-16

GStreamer recorder перенесён в дочерний процесс того же executable; internal mode
выполняется до Tauri/single-instance startup. Parent сохраняет portal Session и
private staging, передаёт выбранный FD/serial через private protocol и контролирует
startup/command deadlines. Cancel/Drop/Close убивают и reap-ят child до освобождения
portal/staging. Broken transport/protocol и timeout не публикуют Ready; finalize
требует успешного child exit и file/frame bounds. Incomplete output не сохраняется.
[Контракт и ограничения](screen-record.md),
[checkpoint](evidence/2026-10-16-screen-record-process.json).

FD inheritance проверена sentinel без CLOEXEC и selected-FD fixture. Startup cancel,
зависший command/finish, crash/oversized reply, owner Cancel/Shutdown и следующая
запись проверены отдельными процессами. EN/RU сообщает о типовых worker failures;
Clear → новый source selection принят в browser. Native harness теперь явно
выбирает RU в private settings; English остаётся default продукта.

Verification:1230 Rust tests passed/0failed/7ignored in163groups со всеми features;
9new tests (8behavioral +1worker entry). Browser40passed, Svelte0errors/0warnings,
frontend build,806/806EN static resources, fmt, strict workspace/all-target/all-feature
Clippy и warnings-denied rustdoc passed. Final456source/script/manifest snapshot
сверен. Static Rust review выявил FD leak boundary; close_range fix и regression
перепроверены. Обычный native workflow и отдельный production180s/4320frame native
limit → Cancel → Import/review/final seek/explicit Save/gallery → Close прошли.
Оба harness завершились0, native staging пуст, all-workers-joined подтверждён.

Реальные plugin/PipeWire/kernel stalls и resource/platform matrix остаются. Linux
close_range CLOEXEC (kernel5.11+) нужен только recorder startup; syscall failure
не запускает capture. Shell/Wayland/screen-reader и hardware qualification не
выполнялись. Три незавершённых плана5/5/6 сохранены.


### Saved firmware recovery in Settings EN/RU — 2026-10-16

Глобальная карточка Saved firmware state читает существующий read-only recovery
RPC только по кнопке, без DeviceId и без требования ordinary connected. Показаны
шесть состояний, пять сохранённых stages, target/ID/guards, nullable counters/
markers и raw JSON diagnostics. Это снимок файлов; no_saved_operation не обещает
отсутствие live job, а markers не означают успех или отсутствие других effects.
Unknown/malformed/read failure очищают прежние поля. Busy deduplication и UI epoch
на unmount/ordinary availability change отбрасывают поздние ответы; новый read
работает при sealed gate. Bridge разрешает только getter; Apply/clear/probe/replay
не подключены. [Контракт](firmware-lcdv2.md),
[checkpoint](evidence/2026-10-16-firmware-recovery-gui.json).

Verification:126GUI Rust tests +5daemon recovery +1real CLI/IPC test passed,
0failed/1ignored; expanded bridge allowlist boundary проверен. Browser22passed,
включая15new recovery cases и7firmware-info regressions. Svelte0errors/0warnings,
frontend build,846/846EN static resources, fmt, strict workspace/all-target/all-feature
Clippy и warnings-denied GUI rustdoc passed. Final462source/script/manifest snapshot
сверен. Новый общий workspace test count не заявляется: проверены затронутые пути.

Native WebKit/Tauri прошёл4private cases: EN empty/guarded с реальным sealed
ordinary admission; RU guard-only/unreadable с open admission. Во всех cases
zero devices,0HID writes/firmware queries/transfer/activation; journal/guards/global
saved files побайтно неизменны после чтения и нормального joined Close. Нет
production daemon, host session или hardware access. Product Apply/live progress,
qualified resolution/restoration и hardware gates остаются. Три плана5/5/6 сохранены.

## Production package и GitHub CI source — 2026-10-16

Добавлен production Linux x86_64/Arch builder: locked release Cargo artifacts
для GUI/CLI/daemon, обязательный custom-protocol и запрет native-smoke, frontend
build, systemd user ExecStart=/usr/bin/lconnectd, LCDV2 udev и desktop launcher/icon.
Исправлена старая license metadata; GPL3-or-later и bundled font notices включены.
Plasma applet/native QML module и GNOME extension/compiled schema устанавливаются
обычным пакетом в общесистемные stores. Определение активного shell, включение
extension/сервиса и изменение панели установщиком не выполняются. Mini не имеет
отдельного окна. GitHub workflow содержит Rust/frontend/browser/package checks;
actionlint прошёл, remote и наблюдённого GitHub run пока нет.

Final694build-input manifest неизменен. Production release/frontend/Qt/GNOME
build,4focused Python regressions, desktop-file/Bash syntax/ShellCheck и workflow
syntax checks passed. Real pacman в private root/db/cache/hooks прошёл install
0.1.0-1→upgrade0.1.0-2→remove:32file hashes/modes, оба Mini stores, ldd трёх
бинарников, CLI version и сохранение user-data sentinel. Repack actual payload
побайтно воспроизвёл tar SHA256. Standard makepkg strip сначала менял production
GUI и был обнаружен hash check; final recipe отключает повторный strip/debug split.

Actual packaged GUI без native-smoke прошёл private Xvfb/session D-Bus/XDG и
AT-SPI: embedded English frontend, IPC к zero-device MockBackend, screenshot и
normal Close exit0. Packaged self-exec recorder entry отвергает missing private
channel до GTK. Никакого production daemon, host installation или HID access.
Новый full-workspace test checkpoint не заявляется: Rust/Svelte sources этим
срезом не изменялись. Real shell/session/autostart, clean-host binary reproducibility,
remote CI execution и hardware остаются открытыми.
[Контракт](package-delivery.md), [evidence](evidence/2026-10-16-package-delivery.json).
