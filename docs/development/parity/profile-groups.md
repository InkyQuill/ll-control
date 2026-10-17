# Полные группы профиля LCDV2

Baseline: Windows L-Connect3 **2.1.28.0**, Galahad II LCDV2 **0416:7395**.
Срез 2026-10-15 реализует **storage snapshot/atomic restore и файловый codec**
полной группы собственного Linux формата. Owner snapshot/commit и durable receipt
подключены. Screen runtime commit/Apply/reapply подключён к LCD actor.
Fan+lighting coordinator commit/install/Apply и explicit reapply подключены
к дереву устройства. Публичный snapshot/export обеих групп через IPC/CLI
подключён. Upload/Inspect IPC/CLI выполняет полную проверку в private quarantine;
Finish/Apply, durable readback и explicit reapply подключены к IPC/CLI.
GUI обеих групп подключён и проверен с native WebKitGTK/MockBackend.
Durable publication ownership/startup recovery подключён к full-group Finish;
расширенная приёмка остаётся открытой.
Существующий [архив одного named slot](profile-archives.md) не заменяет этот сценарий.
Дополнительные LCD effects/Style Express остаются в последней очереди.

[Reset FanAndLighting](fan-lighting-reset.md) использует тот же coordinator и
receipt/readback/reapply с source digest отдельных versioned defaults; daemon,
Unix IPC/CLI/GUI подключены. Read-only preview, explicit retry и native baseline
с desktop/daemon restart приняты; расширенная приёмка остаётся открытой.

## Что делает Windows

Контракт перенесён в [Windows profiles](windows-profiles.md#что-делает-windows).

## Windows ZIP: локальная проверка — 2026-10-17

Контракт перенесён в [Windows profiles](windows-profiles.md#windows-zip-локальная-проверка--2026-10-17).

### Windows lighting bank inspection — 2026-10-17

Контракт перенесён в [Windows profiles](windows-profiles.md#windows-lighting-bank-inspection--2026-10-17).

### Windows lighting Linux preparation — 2026-10-17

Контракт перенесён в [Windows profiles](windows-profiles.md#windows-lighting-linux-preparation--2026-10-17).

### RPM: source contract

Контракт перенесён в [Windows profiles](windows-profiles.md#rpm-source-contract).

### Windows cooling policy preparation — 2026-10-17

Контракт перенесён в [Windows profiles](windows-profiles.md#windows-cooling-policy-preparation--2026-10-17).

### Хранилище cooling mode banks — 2026-10-17

Контракт перенесён в [Cooling modes](cooling-modes.md#хранилище-cooling-mode-banks--2026-10-17).

### Device modes: read-only RPC и редактор — 2026-10-17

Контракт перенесён в [Cooling modes](cooling-modes.md#device-modes-read-only-rpc-и-редактор--2026-10-17).

### Device modes: сохранение снимка и CAS — 2026-10-17

Контракт перенесён в [Cooling modes](cooling-modes.md#device-modes-сохранение-снимка-и-cas--2026-10-17).

### Device modes: persisted selection — 2026-10-17

Контракт перенесён в [Cooling modes](cooling-modes.md#device-modes-persisted-selection--2026-10-17).

## Реализованная storage-основа

`profile_group::GroupProfile` — version1 typed snapshot с двумя вариантами:

- Screen: LCD settings, active theme с cached pages либо media selection,
  screen collections и все screen slots с их исходными индексами/именами.
- FanAndLighting: fan/pump state, cooling policies, optional complete cooling mode banks, LED topology/sync, active
  manual bases и inactive mode maps, dynamic policy и source/unit registry,
  все fan/pump slots. Sensor IDs переносятся точно, без автоматической подмены.

Source device identity, library revision и reset/archive receipts в portable
content не включаются. Global thermal alert policy остаётся target-side: Windows
processNewProfile не меняет её в перечисленных группах. Это не полная копия
всех глобальных настроек L-Connect.

`PersistenceStore::snapshot_profile_group` читает library под общим state gate,
используя latest snapshot последовательного settings owner, включая queued deltas.
Возвращаемый target token включает library revision и SHA-256 полного выбранного
снимка. Группа и слоты проверяются строго: версия, размер, duplicates, чужие
slots, cooling bindings/ranges, manual/dynamic controls и безопасные LCD modes.

`commit_profile_group` проверяет target token и через существующий двухфайловый
journal сохраняет настройки и всю выбранную группу слотов. Другая группа и
global thermal policy сохраняются из latest owner snapshot. Отсутствующие в
исходнике slots выбранной группы удаляются; это замена всей группы.
Revision библиотеки увеличивается, source revision галерей не используется:
берётся следующая target revision. Screen reset receipt инвалидируется при
замене screen. Старые target archive receipts сохраняют только подтверждение
прошлой операции и не импортируются из source.

Оба метода — blocking storage primitives. `SettingsUpdate::ReadProfileGroup`
выполняет snapshot на blocking worker последовательно с queued deltas, без flush
или изменения dirty status. `SettingsUpdate::CommitProfileGroup` владеет Arc<PreparedGroup>
через blocking commit/recovery. Import RPC вызывает actor transaction; прямой вызов из IPC в обход
owner запрещён контрактом API. До commit будущий
caller обязан проверить/подготовить assets и удерживать pins; после commit —
согласовать actors и отчёт об Apply. Primitive `commit_profile_group` сам не
читает payloads и не создаёт receipt; owner использует `commit_group_import`,
который добавляет receipt в тот же journal. Ни один из них не вызывает HID.
После ошибки записи journal исход неоднозначен: сначала recovery/readback,
не новый import intent.

`collections=None` означает legacy shared catalog, не пустую галерею. Group codec
материализует его по общим правилам существующей gallery migration. Прямой
storage restore legacy-снимка поверх versioned catalog до materialization
по-прежнему отклоняется: потеря revision protection не разрешается.

## Файловый group codec

`ProfileArchives::export_group/export_group_file/inspect_group/prepare_group`
используют отдельный контейнер **LCGROUP1**. Framing:8-byte magic, u32LE manifest
length,32-byte SHA-256 manifest, strict JSON и последовательные typed payloads.
`LCPROF01` named-slot остаётся совместим со своим прежним форматом. Оба codec
используют общий pipeline staging/checksums/полной typed validation/публикации/remap.
Это собственный Linux формат, не Windows ZIP.

Manifest содержит GroupProfile, полный inventory user image/video uploads и
список dependencies. Screen export пакует:

- active theme и все cached pages либо выбранный media clip;
- все theme/media slots, включая неактивные;
- gallery-only images/watermarks/background videos;
- все прочие managed user image/video uploads общих stores, даже не входящие
  в текущую gallery membership. Неиспользуемые fonts не пакуются.

Один typed key пакуется один раз. Сохраняются имена изображений/роликов и исходные
байты пользовательских fonts. FanAndLighting не переносит screen uploads.
Каталог читается под короткими locks images→media и повторно сверяется после
staging. Перед копированием в caller writer locks освобождаются: большой архив
не удерживает catalog locks до завершения записи. Изменение inventory во время
подготовки возвращает ошибку с предложением повторить export.

Границы: manifest≤1MiB, entries≤1024, общий payload≤8GiB, copy buffer64KiB;
image/font≤16MiB, image≤960×960, media использует текущие limits reader.
Fonts ограничены64MiB **для каждой темы с её cached pages**, а не суммой всех
слотов архива. Manifest-only тест проверяет96MiB в двух48MiB темах; такой тест
не подтверждает чтение96MiB настоящих fonts.

Перед первым import publication проверяются complete header, все payloads и
полный post-remap shape. `inspect_group` проверяет только manifest. Неверные
checksums, отсутствующие/лишние/duplicate entries, unknown fields/versions,
invalid typed payloads, включая последний access unit видео, отклоняются.
Проверяется framing/Annex-B и индекс ролика; это не H.264 pixel decode или
подтверждение видимого воспроизведения на LCD.
Extraction paths отсутствуют, staging files нумерованы в private tempdir.

`prepare_group` возвращает PreparedGroup с новыми typed keys и inventory uploads.
Settings и slots на этом этапе не меняются. Image/media pins живут до drop
PreparedGroup, fonts защищены общей session retention. Caller удерживает
PreparedGroup через owner commit и runtime preparation. Ошибка после начала
publication может оставить валидные unreferenced assets для обычного cleanup;
неполная группа не записывается. File export использует temporary sibling,
sync и atomic no-clobber: существующий пользовательский файл не заменяется.

Protected versioned gallery defaults сохраняют канонические keys и берутся из
bundled assets целевой установки; по контракту этих ключей artwork не меняется.
Они не превращаются в дополнительные пользовательские gallery entries. Поэтому
полная галерея256 entries импортируется без превышения лимита. Если те же builtins
используются в theme, её payload включается и reference remap выполняется как
для остальных theme dependencies. Sensor IDs и обычный текст никогда не remap.

## Commit owner и durable acknowledgement

`DeviceSettings.group_imports` хранит последние64 GroupImportReceipt. В записи
есть ID32hex, group, SHA-256 исходного header архива до remap (header содержит
hashes всех payloads), исходный target GroupRevision, committed profile/collection
revisions и digest сохранённой desired группы. Receipt и новые settings/slots
публикуются одной двухфайловой transaction. Source archive receipts не переносятся.

Owner получает `CommitProfileGroup {operation_id, expected, prepared, reply}`.
Caller может удерживать Arc<PreparedGroup> для retry без повторной публикации
assets. Новый commit включает уже принятые neighboring deltas и делает их
durable вместе с группой. Только Screen commit освобождает прежние owner leases
screen source/collections; FanAndLighting сохраняет screen pins. Это storage message:
для screen runtime вызывается через LCD actor, а для fan+lighting координатор
переключения live actors ещё требуется.

Повтор того же ID с прежними source/group/target parameters возвращает current
readback и `repeated=true`, без записи или повторного сброса. Поздние queued edits,
dirty status и pins сохраняются. Подмена source header, target token или group
под тем же ID отклоняется. После вытеснения receipt старый target revision всё
равно не разрешает повторную публикацию старого намерения.

`ReadGroupImport {operation_id, reply}` восстанавливает подтверждение без
PreparedGroup и без импорта assets. Это первый шаг после lost reply/restart.
Readback содержит исходный receipt, текущие settings/library и флаг
`selected_content_current`: совпадает ли выбранная desired группа со снимком
после commit. Позднее удалённые/изменённые slots не восстанавливаются. Флаг
не подтверждает live Apply, целостность payloads или видимое LCD playback;
для Apply потребуется отдельная runtime preparation/reconciliation.

Перед положительным readback receipt сверяется с диском и повторно sync-ятся
settings/profiles и directories. Missing/future profile revision, невалидный
receipt либо несовместимый journal не подменяются defaults. При неоднозначной
ошибке owner пытается recovery/readback; если доказать результат нельзя,
возвращает RecoveryRequired и прекращает writer **без flush старого снимка**.
Простое stale/invalid request rejection сохраняет queued intent для обычного flush.

## Screen runtime: serialized commit и Apply

`PreparedScreenGroup::prepare` загружает managed images/fonts/media и готовит
первый JPEG либо composition AU до отправки изменения. ThemeRuntime получает
живые SensorCache; для group media descriptor проверяется и закрепляется до
commit. Source может быть Idle, JPEG theme, managed media или composition.
Это internal API; будущий transfer caller ещё должен подключить общий admission.

`LcdCommand::CommitScreenGroup` проходит через единственный LCD actor. Он
проверяет receipt/parameters и preparation ticket, для streaming запрашивает
известную подходящую firmware, затем отправляет CommitProfileGroup owner и ждёт
его результат. Только после durable receipt выполняются joined stop старого
reader/codec, invalidation старой generation/preparation и LCD config/frame.
Таким образом старый отложенный render не может выйти после смены source.
Group Apply не отправляет SettingsUpdate заново и не заменяет imported settings
последующими deltas старого источника.

Повтор операции возвращает receipt и текущий LCD state без повторного Apply.
Уже подготовленный лишний composition worker при повторе/rejection останавливается
с join. Commit result содержит storage acknowledgement и отдельный `lcd.last_error`:
config/frame failure после commit не превращается в повторный import.

`PreparedScreen::prepare` может восстановить runtime из текущего durable readback,
в том числе на свежем actor после restart. Явный `ReapplyScreenGroup` проверяет
receipt kind, `selected_content_current`, expected LCD generation, preparation
ticket и совпадение active source/config с readback. Новые Clear/brightness/theme
или superseded preparation отклоняют старую попытку. Reapply не меняет settings,
слоты или profile revision. Readback по-прежнему не равен автоматическому Apply.

Неизвестный storage outcome останавливает display source и блокирует дальнейшие
mutations до recovery/restart, сохраняя read-only state. Queued group compositions
на shutdown reaped/joined до выхода LCD actor; commit во время teardown сохраняет
intent для replay, не начинает новый source после shutdown signal.

H.264 group media/composition поддерживает0/90/180/270° через software rotation
финального кадра; hardware rotation остаётся0. JPEG использует hardware angle.
Media сохраняет исходный ID и не превращается в synthetic theme в API.
[Контракт rotation и оставшаяся native/physical приёмка](display-rotation.md).
Известная firmware≥1.5 по-прежнему проверяется до group commit.

Проверки screen actor используют private fake HID channel и временные stores;
composition готовится настоящим FFmpeg, без реального устройства. Проверены
первый config/frame после receipt, sensor-only JPEG с новым значением, JPEG
rotation90/restart180, media/procedural composition, lost reply/repeat после Clear,
старые tickets, config/frame errors + explicit reapply, fresh actor reapply,
unsafe firmware/invalid angles, unknown journal и queued shutdown cleanup.
Workspace988 passed/2ignored,11 screen actor tests; strict Clippy/rustdoc/fmt
прошли. [Evidence](evidence/2026-10-14-profile-groups-screen-runtime.json).

## FanAndLighting: общий барьер actors

`GroupBarrierOwner` выдаёт общий ticket cooling и RGB actors. Каждый actor
обрабатывает `FreezeGroup` последовательно после предыдущих команд, HID ACK и
отправки соответствующих `SettingsUpdate`. Coordinator обязан получить **оба**
ACK до commit: snapshot owner после этого учитывает все ранее принятые deltas.
Разные tickets не могут захватить уже удерживаемый actor; повтор того же ticket
идемпотентен. Уже освобождённый ticket отклоняется, включая поздний enqueue после
отмены вызывающей операции.

Во время удержания cooling policies и весь RGB refresh (base/thermal/dynamic)
не отправляют HID. Все обычные setters, Direct, Clear и изменения thermal/dynamic
возвращают Busy без settings deltas. Cooling state и RGB base/thermal/dynamic
readback доступны. Shutdown не ждёт освобождения барьера. Сам барьер не меняет
runtime configuration, settings, hardware outputs или revision.

Owner освобождает оба actors при обычном Drop до durable стадии. Перед commit
coordinator вызывает `seal`: последующий Drop **не** снимает барьер, потому что
исход записи может быть неизвестен. Явный `resume` разрешён после доказанного
rejection либо reconciliation обоих runtimes. Оставшийся sealed ticket удерживает
actors даже после закрытия watch sender; автоматического timeout/unfreeze нет.
Следующие tick/command замечают release; политики продолжают работу с актуальными
датчиками без persistence mutation только из-за снятия барьера.

Три software regressions используют настоящие cooling/lighting actors и private
fake HID queue: отложенный HID ACK и порядок settings до freeze, sensor change/
refresh при удержании, все mutation variants/read-only queries, resume, разные
owners, late cancelled ticket, unsealed/sealed Drop и shutdown. Барьер сам по
себе не выполняет import/Apply; следующие actor stages описаны ниже.
Исходный checkpoint барьера: workspace999 passed/2ignored; strict Clippy/rustdoc
и fmt прошли.
[Barrier evidence](evidence/2026-10-14-profile-groups-barrier.json).

## FanAndLighting: preflight, install и Apply внутри actors

`PrepareGroup` проверяет typed group целиком и совместимость известных sensor
units. Missing sensor допускается с обычным fallback/degraded поведением; известный
неверный unit отклоняется. RGB готовит active bases, topology, inactive mode map,
активный Dynamic и registry с текущими SensorCache. Global thermal config,
история активных предупреждений и текущая grace сохраняются отдельно от группы.
Нельзя удалить base, необходимый сохранённому thermal rule/overlay.

Preparation не меняет активный runtime и не отправляет HID/settings. Новый
неудачный preflight инвалидирует предыдущую подготовку того же owner. Candidate
привязан к общему barrier ticket; stale/foreign/released ticket не может выполнить
Install/Apply. Уже установленную транзакцию нельзя restage тем же ticket.

`InstallGroup` заменяет runtime без HID и без SettingsUpdate. Coordinator должен
вызвать его **после durable commit** для обоих actors до первого `ApplyGroup`.
Cooling сбрасывает anchors/applied claims под новые policies; RGB сбрасывает claims
по новым bases, сохраняя global alert history, включая warning при исчезнувшем
датчике. Сам actor API не доказывает receipt: порядок обеспечивает coordinator,
описанный ниже.

`ApplyGroup` использует прежние PWM/RPM/motherboard, thermal/dynamic RGB и packet
paths. Он не снимает барьер и не отправляет deltas imported settings. Ошибки HID
отражаются в channel/zone state; desired runtime сохраняется для явного retry.
После release фоновые policies/refresh используют свежие SensorCache. Отдельный
Apply можно повторить без новой установки или import/revision. Непроверенный
результат между install обоих actors требует удержания барьера и recovery.

Legacy fan0 без marker остаётся unset. Явный fan0 сохраняет `manual_zero=true`;
pump0 с сохранённым source также explicit. Manager и group Apply различают эти
случаи без подмены manual control на periodic Fixed policy. Snapshot/codec/reapply
сохраняют marker/source. [Контракт и проверка manual0](manual-pwm-zero.md).

Четыре дополнительные regressions проверяют silent prepare/install, actual
private HID Apply, curve realtime refresh, manual motherboard pump, Dynamic sample,
inactive RGB mode, неверные units и failed/restaged/stale tickets, apply failure/
retry без settings deltas и сохранение global thermal warning на missing samples.
Исходный actor-stage checkpoint: workspace1003 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Runtime-stage evidence](evidence/2026-10-14-profile-groups-install.json).
Actor stages используются coordinator ниже и публичным Finish, описанным далее.

## FanAndLighting: serialized durable coordinator

`ControlGroupCommand::Commit` проходит через per-device coordinator из
`DeviceHandles.control_group_tx`. Он получает оба freeze ACK, затем читает receipt
через settings owner. Повтор ID сверяет kind/source SHA/original expected token и
возвращает текущий runtime без повторного import/Apply, даже если позже приняты
manual/RGB edits. Другие параметры того же ID отклоняются. Group CAS учитывает
ранее принятые settings deltas; обычный stale rejection снимает барьеры и оставляет
старый runtime рабочим.

Новый import проходит preflight обоих actors, затем sealed durable commit.
Только положительный owner receipt допускает InstallGroup. Coordinator ждёт **оба**
install ACK до первого Apply: принятые старые policies не могут выйти между
установками. После ответов обоих Apply он возвращает storage result отдельно от
runtime states/errors и снимает барьеры. Известная HID-ошибка сохраняет imported
desired intent и receipt; это не повод создавать новый import или выдавать Apply
за успешный. После release обычные setters могут исправить состояние.

Потерянный client reply не отменяет уже принятую операцию. Потерянное owner ACK,
RecoveryRequired или неопределённая partial install/Apply admission удерживают
оба producers, выставляют `recovery_required` и запрещают новые coordinator
commits до recovery/restart **всего device tree/daemon**. Read-only actor states
сохраняются. Coordinator в этом срезе не умеет автоматически reload/reapply и не
снимает такой барьер по timeout. Ни поздние deltas, ни shutdown flush не должны
вернуть старый snapshot поверх receipt.

Shutdown до commit не начинает импорт. Если owner уже принял commit, coordinator
дожидается его результата; при наблюдаемом shutdown Apply откладывается до replay.
Queued imports получают отказ и не создают новые receipts. Teardown дерева ждёт
coordinator до остановки settings owner, чтобы не оборвать принятую запись.

Девять software regressions используют реальные codec/settings owner/cooling/RGB
actors и приватный HID. Каждый из четырёх config writes видит durable receipt;
удержанный install ACK блокирует первый Apply. Проверены lost client reply,
lost owner ACK, stale CAS/повтор с другими параметрами, newer manual intent,
соседний LCD/global thermal, sparse fan/pump slots, HID errors, corrupt journal,
итоговый flush и shutdown между commit и Apply с queued import.
Исходный coordinator checkpoint: workspace1012 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Coordinator evidence](evidence/2026-10-15-profile-groups-coordinator.json).
Runtime path подключён к Finish/reapply IPC/CLI/GUI; расширенная lifecycle/native/
physical matrix остаются открытыми. Explicit reapply описан ниже.

## FanAndLighting: explicit reapply и защита нового runtime intent

`ControlGroupCommand::Reapply` принимает import ID и `ControlRevision` из текущего
`ControlGroupState`. Revision состоит из двух opaque actor revisions. У каждого
actor — новый instance ID при создании и локальная generation; это не profile
revision и не durable token. Actor отвечает state **вместе с revision одним
сообщением**, чтобы между чтением состояния и версии не прошла новая команда.
Отсутствие одного actor даёт revision unavailable, не выдуманный0.

Новая user mutation admission (включая Direct/Clear, thermal/dynamic setters,
неуспешную validation/HID попытку) и успешный InstallGroup инвалидируют старый
runtime token. Это консервативная защита нового intent. Sensor refresh и read-only
queries его не меняют. Новый instance ID предотвращает ABA после restart/sleep/
reattach, даже если локальные generation численно повторились.

Reapply получает оба барьера, сверяет runtime revision, читает receipt через
settings owner и требует FanAndLighting kind + `selected_content_current`.
Сохранённые manual/RGB/slot edits отвергают старый receipt даже со свежим runtime
token. Несохранённый Direct меняет runtime revision без изменения group hash:
старый token отклоняется; явный reapply со свежим snapshot может восстановить
imported base. Missing/Screen receipt не используется для fan+lighting.

Desired group строится из current owner readback, а не из старого archive payload.
После preflight обоих actors выполняются оба install ACK и Apply под барьером;
новых CommitProfileGroup, settings deltas, receipt или profile revision нет.
Result содержит readback и текущий runtime отдельно. Нормальная HID-ошибка
допускает следующий явный reapply со свежим token. Неопределённый durable/partial
runtime outcome по-прежнему требует device-tree recovery/restart; этот endpoint
не снимает sealed unknown barrier.

Потерянный reapply reply не отменяет принятую операцию. Повтор старого token
отклоняется до output, поэтому после потери ответа нужно читать runtime заново.
Нет автоматического Apply только из-за чтения receipt или повторного import ID.

Шесть дополнительных regressions проверяют восстановление после HID failure
с неизменными байтами device.toml/profiles.json и одним receipt/revision;
Direct при неизменном durable hash; saved manual/RGB supersession; lost reapply
reply без второго Apply; fresh actors/current receipt и pre-restart ABA token;
missing/Screen receipt и release после rejection. Workspace1018 passed/2ignored; strict Clippy/rustdoc и fmt прошли.
[Reapply evidence](evidence/2026-10-15-profile-groups-reapply.json).
Это private software/actor evidence; production DeviceManager sleep/detach,
partial install reconciliation и native/physical qualification остаются отдельно.

## Публичный snapshot и backup export через IPC/CLI

`group_archive` принимает явные `dev`, `group` (`screen`/`fan_and_lighting`) и
строго typed request. Snapshot читает settings owner и возвращает typed summary
слотов/настроек с opaque `expected`: клиент возвращает строку целиком, не вычисляет
hash и не преобразует revision в JavaScript number. Summary не содержит private
filesystem paths; это краткое описание, а полный portable content находится в архиве.

`BeginExport` привязывает32-hex ID к device/group/expected. Новый ID требует
актуальный owner snapshot; stale token отклоняется до packing. Повтор ID с теми
же параметрами возвращает исходный export, даже после новых settings edits;
другой target/token отклоняется. Готовый архив неизменяем. Status/Read/Cancel/
Release работают по сохранённому target, не требуют подключённого устройства.
Экспорт не делает settings commit, не создаёт import receipt и не вызывает Apply/HID.

Packing выполняется в blocking worker в unlinked temporary file, не удерживая
registry/state lock на время staging/hash/copy. Допускаются4 sessions,2 packers,
2 readers и суммарная логическая reservation не более двух максимальных архивов
(каждый до8GiB payload +1MiB manifest +44 bytes framing). Read ограничен64KiB;
absolute offset можно читать повторно. Inactive sessions удаляются после5 минут
idle, проверка выполняется раз в15 секунд.

Cancel немедленно фиксирует запрос; `cancelling` не равен завершению cleanup.
Worker удерживает admission/quota до фактического выхода; отмена проверяется на
границе записи архива. Уже начатый catalog/staging read не прерывается немедленно.
Ready file закрывается в blocking worker; Release удаляет завершённую session,
а для preparing/cancelling запрашивает отмену. Status остаётся доступен во время
заблокированного catalog read. Полная shutdown/recovery и интерактивная отмена
каждой стадии подготовки остаются отдельной приёмкой.

```sh
lconnectctl profiles group snapshot screen --device DEVICE
lconnectctl profiles group export screen backup-screen.lcg --device DEVICE
lconnectctl profiles group export fan-and-lighting backup-cooling-rgb.lcg --device DEVICE
lconnectctl profiles group status screen SESSION_ID --device DEVICE
lconnectctl profiles group cancel screen SESSION_ID --device DEVICE
lconnectctl profiles group release screen SESSION_ID --device DEVICE
```

Без `--device` CLI выбирает устройство только при единственном доступном target.
CLI печатает session ID в stderr, загружает chunks во временный sibling, проверяет
identity/offset/size/EOF/magic, выполняет sync и `persist_noclobber`; существующий
файл или symlink не перезаписывается. Ctrl+C/error запрашивает Cancel/Release с
ограниченным ожиданием cleanup RPC. Прерванный download не публикуется под
назначенным именем; server-side работа может ещё находиться в cancelling.

Приватные RPC/MockBackend и настоящий CLI/Unix socket проверяют immutable export,
stale source/foreign target, повторные/некорректные reads, сохранение manual0 и
legacy Application source, Screen video больше1MiB и gallery-only image,
чтение последнего кадра после clean-store codec prepare и no-overwrite. Отдельно
проверено удержание quota после Cancel заблокированного packer. CLI signal/fault
matrix, GUI и hardware здесь не запускались. [Evidence](evidence/2026-10-15-profile-groups-export.json).

## Upload и предварительная проверка полной группы

`group_upload` имеет отдельные Begin/Chunk/Inspect/Status/Cancel/Release. Begin
привязывает ID к target device/group/expected/size; новый ID требует актуальный
owner snapshot, повтор возвращает прежнюю session без смены intent. Чужие target
и параметры того же ID отклоняются. Token может устареть уже после Begin:
предварительная проверка не заменяет CAS перед будущим Finish.

Для Windows Screen добавлен `begin_windows_screen`: source record и bounded
explicit sensor/font/resource bindings фиксируются вместе с target snapshot.
Native и Windows source не могут переиспользовать один ID. Windows Inspect
удерживает private PNG/H.264 staging и existing resource pins без записи в target
catalog; Finish использует тот же owner, CAS, receipt и publication rollback.
[Контракт](windows-profiles.md#windows-screen-upload-and-explicit-finish) и
[evidence](evidence/2026-10-17-windows-screen-upload.json). Desktop source workflow
пока отдельно от этого IPC пути.

Upload пишет unlinked temporary file последовательно, chunks1…64KiB. Точный
повтор уже принятого offset/bytes возвращает прежний received; изменённый повтор
или частичное перекрытие завершает session с ошибкой. Пропуск offset, размер вне
лимита и Inspect неполного файла отклоняются до I/O. Ошибка записи терминальна:
частично записанный файл не продолжается как корректный архив.

Inspect проверяет тип группы по checksum-checked manifest, затем все payloads,
checksums, typed content, image/font/media decoding и remap через существующий
codec. Проверка выполняется в **отдельном временном хранилище**, которое удаляется
до `prepared`: target uploads/gallery, settings, named slots, receipts и runtime
не меняются. Это важно для legacy shared gallery, где публикация новых uploads
сама изменила бы видимый каталог. Summary содержит логические ключи временного
remap, они не являются target asset IDs. Оригинальный файл и digest manifest
сохраняются в session для дальнейшего Finish; actual target preparation/publication
и commit/Apply выполняются только явным Finish, описанным ниже.

Admission:4 sessions,2 blocking workers, отдельная логическая reservation до двух
MAX архивов для upload pool; export pool имеет свой бюджет. Эти reservations не
являются ограничением всех временных файлов и памяти codec. Пока работа реально
выполняется, её admission/reservation не освобождаются из-за потери RPC future
или Cancel. Metadata Status не ждёт data lock или decoding активного worker. Cancel публикует
`cancelling`, затем освобождает staging на owned worker; уже начатый codec/backend
не прерывается немедленно. Idle sessions истекают после5 минут без активности,
с проверкой раз в15 секунд; активная работа не прерывается по idle timeout.
Поздний Release старого объекта session не удаляет новый объект с тем же ID.

```sh
lconnectctl profiles group inspect screen backup-screen.lcg --device DEVICE
lconnectctl profiles group inspect fan-and-lighting backup-cooling-rgb.lcg --device DEVICE
```

CLI проверяет magic/size до подключения, передаёт файл частями, сверяет каждое
acknowledgement и ждёт результат inspection. `--json` возвращает typed summary,
source digest и target token без binary payload. После readback или ошибки CLI
запрашивает cleanup с ограниченным ожиданием. Inspect не означает восстановление
профиля. CLI signal/fault/restart/native matrix остаётся открытой.

Actual private RPC/MockBackend проверяет incomplete/gap, exact chunk retry,
различающийся повтор, foreign target/group, source type mismatch, corrupt manifest,
валидный Screen с gallery-only image без публикации в target catalog, unchanged
owner snapshot/receipts/PWM и cleanup. Actual CLI загружает и проверяет Screen
архив>1MiB и отклоняет неверную группу. Отдельная блокированная worker regression
проверяет lost waiter/Cancel, удержание reservation и доступный Status до
фактического выхода. [Evidence](evidence/2026-10-15-profile-groups-upload.json).

## Явный Finish, Apply и durable readback

`group_upload.Finish` допускается после Prepared. Session становится `importing`;
Cancel/Release не прерывают уже принятую операцию и возвращают её состояние.
Owned task сохраняет entry/admission после потери ответа или соединения. Finish
имеет2 собственных slots и использует общую `theme_jobs` admission обычных LCD
preparations: новый endpoint не обходит runtime budget. Status остаётся отдельным
metadata query; повтор Finish в importing не запускает второй job.

Сначала settings owner ищет receipt этого ID. Kind/source digest/original expected
должны совпадать; известный повтор возвращает current desired/runtime без повторной
публикации assets, preparation ticket, commit или Apply. Новый Finish повторно
проверяет и подготавливает исходный файл в target stores. Source digest и group
должны совпасть с ранее проверенным архивом. PreparedGroup удерживает pins до
actor transaction. Screen получает ticket до медленной подготовки и reserved
LCD channel slot до runtime preparation; shutdown закрывает и дренирует receiver,
включая ожидаемую reservation. Более поздние LCD edits отклоняют старый ticket.
FanAndLighting использует существующий общий coordinator/barriers.

Оба пути делают durable owner commit/CAS/receipt перед output. Screen выполняет
config/frame через LCD actor; FanAndLighting устанавливает оба runtime до Apply.
Finish возвращает `imported` с typed `committed` только после actor result и
закрытия staging. Терминальная фаза и commit result публикуются одним metadata
lock. Storage acknowledgement отделён от runtime.last_error: известная ошибка
Apply не отменяет сохранённый import. CLI печатает результат и возвращает ошибку
процессу при известном runtime failure.

Known rejection даёт failed; неопределённый outcome даёт recovery_required и
требует readback/recovery, не автоматического повторного Apply. Cancel не откатывает
сохранённую группу. Release удаляет metadata готовой session, но не receipt.
`read_group_import` работает через settings owner после Release и restart daemon,
пока receipt сохранён (последние64). Wrong kind/target, отсутствующий/вытесненный
receipt возвращают ошибку. Current summary и selected_content_current показывают
поздние изменения; runtime читается отдельно и не образует атомарную пару с storage.
Screen runtime использует atomic GroupState с intent revision и фактическим
recovery flag. Состояние ordinary LCD State и его generation не заменяют token.

```sh
lconnectctl profiles group import screen backup-screen.lcg --device DEVICE
lconnectctl profiles group import fan-and-lighting backup-cooling-rgb.lcg --device DEVICE
lconnectctl profiles group readback screen OPERATION_ID --device DEVICE
```

CLI импортирует обе группы после bounded upload/Inspect. ID печатается до передачи
файла; потеря ответа/interrupt после admission не отменяет Finish. Сначала нужно
прочитать результат по тому же ID, включая после restart. Новый import ID означает
новый intent; readback и repeated Finish не выполняют Apply сами по себе.
Explicit transport reapply описан ниже; native GUI подключён, см. раздел ниже.

Actual Unix RPC/DeviceManager/MockBackend проверяет FanAndLighting manual0 и
обе static RGB bases, соседний LCD, Screen config/JPEG theme, сохранение нового
fan67 при повторном Finish, новый brightness52 при stale Screen CAS, cleanup и
readback после daemon restart с утраченными transfer sessions. Actual CLI
импортирует Screen архив>1MiB с indexed video в sparse slot, затем fan/pump0,
читает receipt и сохраняет соседние slots. Отдельная gated test проверяет lost
response, неотменяемость admitted Finish,2-job capacity и terminal unknown/known
rejection. [Evidence](evidence/2026-10-15-profile-groups-finish.json).

Target files до commit теперь принадлежат private publication конкретной операции.
Изображения/видео скрыты из обычных и legacy catalogs до разрешения исхода.
Known failed Finish выполняет scoped rollback после negative owner readback;
accepted/unknown outcome сохраняет файлы. Подробности и границы — ниже.
GUI/native/hardware, raw streaming media Finish и расширенные unknown/partial
runtime/lifecycle faults остаются отдельной приёмкой; internal actor streaming
проверки не заменяют публичный transport workload.

## Явный reapply через IPC/CLI

`read_group_import` и результат Finish возвращают `runtime.revision`: opaque
строку с tagged group и actor intent revision. Клиент возвращает строку без
разбора чисел. Token не равен group/library CAS token, LCD generation, времени
последнего кадра или sensor revision. Он привязан к runtime конкретного device:
у каждого actor новый random instance при создании и локальная generation.
FanAndLighting использует ранее реализованную пару cooling/lighting revisions.

LCD GroupState возвращает state/revision/recovery одним actor message. Обычная
mutation admission, включая validation/HID failure и BeginThemePreparation,
инвалидирует старый token. Reads, live sensors и frame ticks его не меняют.
Checked BeginScreenGroupReapply сверяет token, receipt kind и current contents
на owner, затем выдаёт новый revision/ticket и snapshot для подготовки. Старый
token потреблён уже в этот момент. После подготовки checked Apply повторно
проверяет revision, generation, preparation, current receipt и source/config;
более поздний setter, даже неуспешный, отклоняет candidate с joined cleanup.
Новый instance предотвращает ABA после restart/sleep/reattach.

`reapply_group_import` принимает явный dev/group/operation_id/expected. Wrong
kind/malformed/oversized token отклоняется до работы. FanAndLighting идёт через
существующий coordinator Reapply с обеими barrier/revision checks. Screen идёт
через checked Begin/Apply и готовит saved scene/media через прежний runtime.
Current owner readback, а не старый archive payload, определяет desired source.
Новых CommitProfileGroup, receipt, profile revision или settings deltas нет.
Новые сохранённые edits делают selected_content_current=false и запрещают retry;
Direct RGB меняет runtime token, но допускает явный retry со свежим snapshot.

Operation использует общую theme_jobs admission и owned task до окончания
preparation/actor result/cleanup. Потеря caller/connection не отменяет admitted
retry. Source reservation удерживает LCD shutdown drain до завершения подготовки;
queued checked composition останавливается и joins перед teardown. Повтор старого
token не делает второй Apply: после потери ответа нужно читать state заново.
Unknown durable/runtime barrier не снимается reapply; требуется recovery/restart.
Known Apply failure сохраняет desired profile и выдаёт last_error отдельно;
следующий явный retry требует свежий token.

```sh
lconnectctl profiles group readback screen OPERATION_ID --device DEVICE
lconnectctl profiles group reapply screen OPERATION_ID --device DEVICE
lconnectctl profiles group reapply fan-and-lighting OPERATION_ID --device DEVICE
```

CLI без `--expected` сначала читает current receipt/runtime и требует current
content, затем вызывает reapply с полученным token. Это явное действие пользователя,
не автоматический retry после чтения. Для automation есть `--expected TOKEN`:
CAS исполняется на actor в любом случае. JSON содержит receipt/summary/runtime;
known Apply error даёт nonzero exit после вывода результата.

Actual Unix RPC/DeviceManager и CLI проверяют обе группы, immutable settings/
profiles/receipts, old token rejection, Direct → fresh retry, saved fan67 и invalid
LCD setter. Gated actual RPC проверяет caller disconnect во время asset load,
завершение accepted Apply и отказ повторного старого token. Actor regressions
проверяют late failed mutation между Begin/Apply, known frame failure → fresh
retry без новой записи, current receipt на новом actor и ABA token, queued
H.264 composition shutdown join. [Evidence](evidence/2026-10-15-profile-groups-reapply-transport.json).
Расширенная public streaming/transport fault/workload/physical matrix
остаются; private actor tests не равны приёмке реального LCD.

## Scoped rollback после неуспешного Finish

Actual Unix RPC regression воспроизвёл лишний upload в legacy catalog после
stale Screen CAS. `Publication` теперь получает записи **только из callbacks
эксклюзивного создания** PNG, name metadata, font и indexed media. Directory diff
не используется: обычные uploads и параллельные imports не становятся кандидатами
для удаления. Fingerprint включает filesystem device/inode и удерживает descriptor
исходного файла до разрешения исхода; замена пути не может выдать повторно
использованный inode за прежний файл. Новый name sidecar публикуется без
перезаписи существующего файла; rename использует прежний revision-checked update. Это временные bounded descriptors на новые
файлы/sidecars; их открытие подчиняется обычным OS resource limits.

Новые images/media исключены из list/list_named/with_catalog, включая legacy
migration и export inventory, но доступны подготовке по private key. Media quota
по-прежнему учитывает все физические entries, включая скрытые. Successful commit
раскрывает файлы после actor result; known Apply error также сохраняет accepted
assets. Видимость и descriptor cleanup завершаются на blocking worker до terminal
Finish state/admission release.

После ошибки actor/preparation caller повторно читает receipt через serialized
settings owner. Только доказанное отсутствие receipt разрешает rollback.
Существующий receipt, недоступный owner или неизвестный исход сохраняют assets;
readback failure переводит outcome в recovery_required. Panic/Drop fallback
сохраняет файлы, а не пытается угадать отсутствие commit.

Rollback проверяет собственный pending token, regular root, identity каждого
пути, live image/media leases, pending pins и все durable settings/profile refs,
включая offline devices. Rename metadata, другой файл или symlink по тому же
пути сохраняются. Нечитаемые/новые schema/journal не считаются отсутствием refs:
cleanup сохраняет файлы и добавляет ошибку к исходному результату. Все собственные
pending visibility markers снимаются и при неполной очистке.

Для fonts обычный load отмечает external observation; такой файл сохраняется
до обычной session/GC процедуры. Candidate Screen загружает **только свои новые
fonts** без этой отметки, чтобы собственная неуспешная подготовка не блокировала
откат. Existing/foreign fonts используют прежний load contract. Успешные imports
получают обычное session retention; global cleanup не запускается.

Программные проверки: rejected Finish с image/font/gallery video и более новой
brightness52 очищает свои файлы, сохраняет исходные и прибывший во время операции
upload, legacy membership и shutdown flush. Storage regressions проверяют два
параллельных publication scopes, скрытые catalogs, live pins, external font load,
offline durable refs, unknown-outcome retention, partial publish failure после
PNG/name и foreign blocker, заменённый font path, переименованный image metadata и конфликтующий foreign
sidecar при создании нового upload.
[Evidence](evidence/2026-10-15-profile-groups-rollback.json).

Этот checkpoint квалифицировал live scoped rollback. Теперь ownership дополнен
[durable journal/startup reconciliation](import-publication-recovery.md): private
staging и hard links переживают process death; receipt/phase определяет cleanup
или retain. Расширенная fsync/FD exhaustion/panic/shutdown fault matrix и
named-slot publication cleanup остаются открытыми.

## Проверка и дальнейшая реализация

Storage tests проверяют JSON round-trip обеих групп, замену sparse slots,
сохранение соседней группы и globals, exact bindings, inactive modes/dynamic
registry, restart, stale token после queued same-group edits, сохранение queued
neighbor edits, corrupt target и неправильные входы. Shared journal fault test
останавливает каждую группу после journal/device/profiles publication и проверяет
полное восстановление обоих файлов новым store. [Evidence](evidence/2026-10-14-profile-groups-storage.json).

Group codec проверен clean-store round-trip с actual files, image/font/cached
video и extra uploads, сохранением neighboring settings/slot и restart. Проверены
legacy membership, галерея256 entries, no-overwrite/error paths, pins, indexed
video600frames/>1MiB с64KiB I/O и последним прочитанным frame, source owner queued
deltas и post-checksum typed corruption. [Codec evidence](evidence/2026-10-14-profile-groups-codec.json).

Codec checkpoint: workspace970 passed/2ignored, strict Clippy/rustdoc/fmt
прошли;10 group codec tests. Owner/receipt checkpoint записан отдельно в
[evidence](evidence/2026-10-14-profile-groups-commit.json): workspace977 passed/
2ignored, strict Clippy/rustdoc/fmt прошли;6 owner/storage tests плюс journal
receipt recovery и contended abandoned worker/pins. GUI/native/hardware в этих
срезах не запускались.

Оставшаяся работа для полного SYS-002:

- Расширить native GUI acceptance: real X11/Wayland file portals, lost Finish
  reply/abrupt desktop death, close во время принятого Apply, large streaming,
  expiry/corruption и resource/fsync/codec failures. Базовый native GUI обеих
  групп с progress/Cancel/Inspect/Apply/readback/reapply уже принят.
- Extended publication boundary/fsync/resource/panic/shutdown matrix и
  применение scoped durable ownership к named-slot archive path.
  Live known-failure group rollback, hidden catalogs и reference protection подключены.
- Расширить FanAndLighting partial install/Apply reconciliation, production
  DeviceManager sleep/detach/contended/caller cancellation fault matrix. Physical manual0 qualification — в [контракте](manual-pwm-zero.md).
- Расширенная screen import/reapply lifecycle matrix, включая detach/sleep/contended commit, caller cancellation при preparation/
  enqueue и зависшие codec/filesystem backends. Runtime admission подключён;
  расширить приёмку shared budget/shutdown и публичного streaming media Finish.
- Windows ZIP: container/metadata inspection реализован выше; остаются semantic
  mapping профильных данных/путей/fonts/media, explicit target-bound commit и
  genuine Windows fixtures/round-trip. Переименование `.lcp` в `.zip` этого не даёт.
- Browser/native fault/accessibility matrix; физическая приёмка отложена.

## Native GUI полных групп

В «Настройки → Резервная копия группы настроек» выберите устройство и группу.
Экспорт использует native XDG FileChooser и sibling temporary file с sync,
`persist_noclobber` и sync каталога. JavaScript получает имя файла, summary и
progress, а не archive payload или право открыть произвольный путь.

Импорт передаёт выбранный regular `.lcg` файл по64KiB и выполняет quarantine
Inspect. Предпросмотр показывает слоты, тему/LCD, manual PWM (включая0),
политики охлаждения и active lighting/dynamic binding из summary. Кнопка
«Восстановить и применить группу» отдельно принимает Finish; до неё settings,
profiles и LCD runtime не изменяются. Более новый intent после Inspect приводит
к CAS отказу. Соседняя группа и черновики редакторов сохраняются.

Native job владеет chooser/файлом/IPC worker и completion signal. Progress не
требует ждать transfer mutex. Cancel не публикует terminal state до завершения
worker/remote cleanup; после принятого Apply Cancel запрещён. Закрытие окна
дожидается native workers; Prepared import отменяется и освобождает daemon
staging, а принятый Apply ожидается. Известный завершённый импорт освобождает
upload session; durable receipt остаётся доступным.

ID, device, group и direction сохраняются в localStorage до явного закрытия
сведений. Reload/readback не отправляет Finish повторно: сначала читает durable
receipt, затем, если его нет, проверяет upload session. Только Prepared можно
явно применить. Explicit reapply передаёт opaque runtime revision из прочитанного
результата, проверяет current receipt и не создаёт нового commit. Storage ACK и
ошибка/recovery runtime показываются отдельно. Потерянный ответ на export не
выдаётся за подтверждённый локальный файл.

Программная приёмка: обе группы прошли actual Unix IPC/MockBackend file round-trip,
large font chunking, corrupt Inspect с освобождением failed session, Inspect без commit, отдельный Finish, readback нового native
owner, reapply, no-overwrite, Prepared Cancel/shutdown и stale LCD intent.
В браузере проверены явный target, несовпадение выбранного device, потеря Start
reply, reload с сохранённым ID, runtime errors, draft preservation и отсутствие
автоматического Apply. Native WebKitGTK/private FileChooser проверил обе группы,
изменение64→27→64, readback/reapply без нового commit, normal Close и receipt/state
после restart mock-daemon. Normal Close отправляется через WM_DELETE_WINDOW
(`xdotool windowquit`) и подтверждается markers входа в close handler и join
workers. Нестабильная доставка Alt+F4 в Xvfb не считается проверкой keyboard
shortcut. Это не проверка USB или реального desktop portal.
[Evidence](evidence/2026-10-15-profile-groups-gui.json).

## Барьер перед durable ownership

Ownership callback PNG/name/font/media теперь fallible и вызывается на synced
private candidate перед финальным `persist_noclobber`. Ошибка регистрации или
захвата identity предотвращает публикацию. Metadata failure не удаляет PNG,
если его inode уже заменён. [Проверки](evidence/2026-10-15-import-prepublication.json).
Этот barrier используется durable journal полных групп. Базовый process-death
recovery подключён; расширенная fault matrix и named-slot path остаются открытыми. [Протокол и crash checklist](import-publication-recovery.md).

## Durable journal и startup recovery

Full-group Finish создаёт bound scope с private candidates и lifetime lock;
ownership links/manifest sync предшествуют final publication. Arm sync происходит
до owner commit. Known positive/negative outcomes записывают keep/discard перед
retirement. Unknown outcome/Drop сохраняет durable notice до удаления scope.
Startup сначала завершает settings+profiles redo journal и проверяет receipt.
Preparing/discard допускают reference-protected cleanup; accepted сохраняется;
armed/keep без receipt сохраняется с diagnostic. Receipt eviction не является
negative proof. Factory и manager выполняют сверку до catalogs/replay; corrupt
состояние сохраняет данные и блокирует запуск устройств.
[Протокол и оставшиеся проверки](import-publication-recovery.md),
[evidence](evidence/2026-10-15-import-publication-journal.json).

## Язык UI архивов, 2026-10-15

Фазы, progress labels и параметризованные сводки/сообщения используют EN/RU
resources; пользовательские имена и source IDs не переводятся. Private native
round-trip, readback и restart проверены на обоих языках. Это не закрывает
все worker errors или настоящий desktop portal picker. Native title/filter labels
теперь локализованы отдельным срезом и проверяются в private portal arguments.
[Общий language contract и оставшаяся приёмка](interface-language.md),
[evidence](evidence/2026-10-15-language-archive-ui.json).
