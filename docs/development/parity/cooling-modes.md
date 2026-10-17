# Cooling modes: LCDV2

Windows baseline2.1.28.0, Galahad II LCDV2 `0416:7395`. Реализованные операции
хранят режимы отдельно от active requested policy, локального editor draft и
четырёх named user slots. English — основной язык, Russian обязателен. Другие
модели не квалифицированы. Новый source checkpoint не заменяет production package.

| Действие | Stored banks | Рабочая policy/ID |
|---|---|---|
| Read / Load | Чтение, policy копируется только в draft | Без изменения |
| Initialize | Создаёт стандартные8/3 только при отсутствии bank | Без изменения |
| Save | CAS имени/снимка policy существующего ID | Без изменения |
| Stored Apply | Чтение актуального ID по CAS | Сохраняет ID + requested snapshot |
| Custom Apply / manual PWM | Без изменения | Снимает ID своего канала |

## Standard bank initialization

После явного Read, подтвердившего отсутствие banks, UI предлагает
`initialize_cooling_modes(dev,sensor)`. Пользователь выбирает температурный ID;
daemon требует известный Celsius source из host cache либо cache этого LCDV2.
Автоматического выбора CPU/GPU/coolant нет. Отсутствующее значение у известного
source не подменяется выдуманными данными. Генератор pure; не делает file/HID IO.

| Канал | IDs | Начальные параметры |
|---|---|---|
| Fan | Quiet, StandardSpeed, HighSpeed, FullSpeed | Соответствующие FanPreset; runtime endpoints0°C/10% и100°C/100% |
| Fan | UserDefine1…4 | Quiet curve, как в source default profile |
| Pump | FixRPM |2200 RPM — Windows factory value, а не2300 RPM старой owned fixture |
| Pump | PWMPerformance, PWMQuiet | Шесть source knots соответствующего PumpPreset |

Для curves используются explicit sensor, hysteresis0 и fallback100%. Initializer
не назначает active policy или selected ID. Сохраняются существующие PWM/source,
политики, RGB, Screen, named slots и globals. Historical source metadata не
выдумывается: generated banks имеют `source=None`.

Settings owner атомарно принимает отсутствие полного bank, создаёт11 entries,
увеличивает durable target-local epoch и пишет через общий file/parent sync.
Существующий bank, включая намеренно пустой/частичный, не дополняется и не
заменяется. Из concurrent initializers выигрывает только один. При disk failure
принятый bank остаётся latest/pending; повторный Initialize его не перезаписывает,
writer повторяет сохранение после quiet interval.

UI не повторяет mutation после ошибки/потери ответа. Один readback может найти
существующий durable bank; тогда сообщение предлагает проверить список, не
утверждает авторство операции. Pending/error не объявляются сохранёнными;
unknown/absent результат требует явного Read перед новой попыткой. При смене
device/channel поздний ответ не меняет чужой UI; reconnect сохраняет unknown
fence. Готовый список получает keyboard focus только если пользователь ещё не
перешёл к другому полю. Draft editor и работающие настройки не заменяются.

## Factory and literal names

Source `RPMProfile.DisplayName` использует language resource, если `Name` пустой,
и literal user name иначе. В Linux `CoolingModeProfile.use_default_name=true`
явно задаёт known factory label: ID/target/canonical English name валидируются
вместе. Marker отсутствует/false для legacy и literal names; false не добавляет
полей в старую portable форму.

Factory labels переводятся EN/RU, canonical English name хранится с marker.
UI Save без редактирования имени сохраняет marker. Любой явный ввод имени снимает
его, даже если итоговая строка совпала с переводом default label; пользовательские
строки не переводятся по совпадению текста. Language change обновляет factory
options/status, сохраняя draft input и literal names. Marker участвует в bank CAS
и durable serialization. Старые Windows-derived banks сохраняют свои literal
names и исходную nullable metadata; полноценный Windows mapper этим не закрыт.

## Проверка и оставшиеся границы

[Текущий initializer checkpoint](evidence/2026-10-17-cooling-mode-defaults.json)
содержит source hashes, команды и точные scopes. Pure defaults, serialized
initializer/concurrency/disk failure/reopen, EN/RU/focus/name origin и native
fresh-store acceptance отделены от физических проверок.

1272 Rust tests/9ignored в172 result groups прошли;60 unique browser scenarios
покрыты broad и focused runs с повторной функциональной проверкой initializer.
Svelte0/0, English1030/1030/static,49 dynamic calls как inventory и strict
fmt/Clippy/rustdoc/build. Native RU начинает с None без fixture seed, теряет
actual post-commit Initialize ACK и делает ровно одну запись; дальше проходят
Save2400 с lost ACK, stored Apply60% и dedicated/full restart.81/81 RPC overrides
— только inventory, не полная parity. Все19 scope inputs после native неизменны.

Физический [M2-08](lcdv2-m2-cooling-acceptance.md#cases) остаётся not-run;
безопасные restore/source/wiring должны быть установлены до live PWM. Нужно
завершить Windows FanAndLighting/Screen/assets mapper. Full reset и отдельное
восстановление режима/имени реализованы ниже; их новый checkpoint не расширяет
аппаратную приёмку. Пакет `distribution-20261017-collections` предшествует этим source
изменениям. Hardware, real shell session, expanded native accessibility и Windows
runtime/raster equivalence этим checkpoint не заявляются.

## Контракты операций и предыдущие checkpoints

Ниже сохранены точные контракты storage/read/Save/selection и границы их прежних
checkpoint. Их исторические counts не заменяют новый initializer checkpoint.

### Хранилище cooling mode banks — 2026-10-17

`DeviceSettings.cooling_banks` и `FanAndLightingGroup.cooling_banks` теперь
хранят optional version1 `CoolingModeBanks`: отдельные fan/pump vectors с
id/name/validated policy, до16 modes на канал. Это не четыре named user slots
и не активный `CoolingConfig`: ручной PWM и обычный policy Apply не стирают
сохранённые неактивные режимы. `profiles windows-cooling` выдаёт готовый typed
carrier `cooling_banks` вместе с прежним preparation report.

Mode IDs ограничены32 ASCII alnum/underscore/dash, имена128 UTF-8 bytes без
control/blank-only, duplicates и invalid inactive policies отклоняются. Optional
source provenance имеет известный Windows2.1.28 format и два object-JSON strings
по64KiB максимум. JSON хранится текстом, чтобы null Windows fields не терялись в
TOML. Это историческая presentation/source metadata, не текущая selected mode,
capabilities, sensor identity или file IO. Live intent остаётся в активных policies.

Banks входят в settings validation, full-group capture/hash/restore и shared
journal/receipt/readback. Поэтому изменение inactive mode меняет CAS token и
делает старый import receipt неактуальным. Screen import сохраняет target banks;
FanAndLighting replacement переносит source banks, а legacy absent field и factory
reset payload очищают imported overrides. Старые файлы/архивы без поля читаются
как None; неизвестные bank/source versions отклоняются без перезаписи.

Executable CLI → TOML → Linux archive → prepared group → journal commit → reopen/
receipt readback проверены на всех8 fan/3 pump modes. Дополнительно проверены
Screen preservation, stale CAS и invalid inactive policy без изменения файла,
а actual IPC/MockBackend — manual/policy update и restart без потери banks.
[Evidence](evidence/2026-10-17-cooling-mode-bank-storage.json).
Это storage integration; discovery/load-to-draft, Save и persisted identity
реализованы ниже. Full Windows FanAndLighting conversion ещё нужна;
автоматически inactive policies не применяются.

### Device modes: read-only RPC и редактор — 2026-10-17

`get_cooling_modes(dev)` читает latest accepted banks через serialized settings
owner. Возвращает version/revision/fan/pump/persistence с id/name/policy; historical source provenance
исключается из ответа, сохраняясь в хранилище. Getter не читает global named-slot
library, поэтому её отдельная corruption не блокирует доступ к modes. None —
известное отсутствие banks, ошибку нельзя подменять пустым списком. Запрос не
применяет policy, не меняет dirty state и не добавляет HID writer.

В Cooling появился закрытый по умолчанию раздел «Сохранённые режимы устройства».
«Прочитать» запрашивает modes выбранного устройства; выбор и «Загрузить режим в
черновик» копируют policy в существующий editor. Missing sensor ID сохраняется
как unavailable, автоматического rebind нет. После проверки/правки используется
обычный explicit Apply. Built-in presets и четыре user slots остаются отдельными
потоками. Это загрузка snapshot в draft, а не применение актуальной bank entry по ID.

Malformed/future/duplicate/cross-target replies не попадают в draft. Local typed
validation переводится при смене EN/RU; raw daemon diagnostics и user names
остаются данными. Pending read не сбрасывает keyboard focus; device/channel/
connection/unmount generation guards отбрасывают поздние ответы. Refresh/error
не стирают редактор и не оставляют старый bank selectable.

Browser EN/RU, 8/3 choices, keyboard, two desktop widths, errors/language/late
responses проверены. Native Tauri/WebKitGTK → Unix IPC → MockBackend в RU:
получены8/3 modes, FixRPM2300 загружен без Apply, explicit Apply дал58% и banks
пережили GUI close/daemon restart; fixture не пересоздаёт их после первого seed.
[Evidence](evidence/2026-10-17-cooling-device-modes.json).
Новый get RPC имеет daemon override и узкий bridge allowlist entry. Это78/78
методов текущего дерева; inventory не является полной parity-проверкой.
Чтение дополнено storage-only Save и persisted selection, описанными ниже.
Открыты reset стандартных banks, Windows group conversion,
native EN/error/keyboard/speech/real-session и physical/package qualification.

### Device modes: сохранение снимка и CAS — 2026-10-17

После Load имя и текущая policy черновика могут быть сохранены в существующий
mode через `save_cooling_mode(dev,target,expected_revision,profile)`. Это отдельная
операция от Apply: она не отправляет команд охлаждения и не меняет active policy.
Создание/удаление entries этим setter не поддерживаются; persisted selection
подключена отдельным Apply, описанным ниже.

Settings owner сериализует Save с остальными deltas, проверяет ID, policy/target,
имя и revision всего bank, сохраняет private Windows provenance и соседние
настройки. Target-local durable epoch увеличивается даже при no-op Save и полном
FanAndLighting import/reset; исходный token не оживает при возврате старых значений.
Epoch не переносится в archive. Он участвует в group CAS/receipt guard; Screen
revision от него не зависит. Диапазон ограничен TOML i64; exhaustion отклоняется
до принятия нового намерения. Обычный active-policy Apply не меняет bank token.

Ответ содержит persistence status. При ошибке file/parent sync принятое намерение
остаётся pending, daemon повторяет запись. UI не объявляет успех по pending/error
и не требует `saved=true`: это session flag, после чистого restart он может быть
false. Критерий durable readback — `pending=false`, `last_error=null`.

При потерянном/ошибочном ответе UI делает один readback без replay Save. Совпадение
имени/policy с durable latest bank подтверждает наличие желаемого снимка, не receipt
авторства конкретной операции. Несовпадение, invalid response и read failure
сохраняют draft и блокируют новую запись до явного Read. После конфликта отдельная
кнопка явно называет overwrite актуальных данных. Поздний ответ не заменяет новые
правки editor или UI другого device/channel. Unresolved Save сохраняется при
переподключении выбранного устройства; автоматической повторной отправки нет.

1269 Rust tests/9ignored,34 browser, Svelte0/0, English1006/1006 и strict
fmt/Clippy/rustdoc/build прошли. Native RU Tauri/WebKitGTK → Unix IPC → MockBackend:
Load2300, Save2400 с потерей actual ACK, ровно один Save, без active change;
отдельный Apply60%, GUI close/restart сохраняет banks и revision.
Новый Save RPC имеет daemon override и narrow bridge entry:79/79 methods,
это inventory, не full parity. Новый distribution не собирался.
Проверки и оставшиеся границы: [evidence](evidence/2026-10-17-cooling-mode-save.json).

### Device modes: persisted selection — 2026-10-17

Windows2.1.28 `Products/RPMSetting.cs` хранит `Mode` отдельно от `Profiles`,
`GetCurrentProfile()` находит запись по ID. `Galahad2VisionRPMSettingSelector.cs`
показывает `Fan/Pump.GetCurrentProfile()`. Linux сохраняет тот же смысл через
optional `CoolingConfig.fan_mode/pump_mode` рядом с requested policies. Это ID
запрошенного снимка, не отдельное утверждение о физическом applied RPM.

`apply_cooling_mode(dev,target,expected_revision,id)` выполняется внутри sole PWM
owner. Перед применением он CAS-resolve-ит ID/policy через settings owner. Весь
command завершится до freeze acknowledgement для group transaction; group import
не может заменить banks между этим resolve и завершением обычного Apply. Resolve
упорядочен относительно Save: Save, принятый раньше, делает старый token invalid;
сохранение нового mode после resolve может оставить активным прежний снимок.
Именно его policy и ID переносятся вместе в settings delta. Новый writer не создан.

Settings и FanAndLighting archives валидируют membership ID в bank и наличие
requested policy. Равенство policy текущей bank entry намеренно не требуется:
Save может менять сохранённый режим без изменения работающей политики. Restart
воспроизводит requested snapshot, не подменяет его отредактированной entry.
Legacy файлы/группы без identity читаются с None; новые None поля опущены при
serialization, старые group hashes не меняются только от появления этой схемы.
Screen import сохраняет соседние banks/selection. Generic policy Apply и успешный
manual PWM override снимают identity только своего канала; совпадение policy по
значению само по себе не назначает ID.

UI Load сохраняет локальную ссылку на mode/revision/snapshot. Apply использует ID
только пока editor соответствует этому снимку. Правки применяются как custom
policy; явный Save может заново связать сохранённый снимок с тем же editor session.
Поздний Save не перепривязывает новый draft, даже если через Load current/profile
были загружены равные значения. Stale stored Apply возвращает ошибку, не пробует
автоматически generic Apply. Requested mode ID показан отдельно в daemon state;
загрузка в editor не означает применения. EN/RU и существующий дизайн сохранены.

Offline `profiles windows-cooling` теперь выдаёт `cooling_config` с source-selected
IDs и соответствующими policies рядом с полными banks. Это validated preparation,
не Windows group import, не sensor verification и не Apply.

1270 Rust tests/9ignored в172 result groups,47 browser scenarios и2 focused
reflow repeats,3 final CLI archive tests, Svelte0/0, English1009/1009 и strict
fmt/Clippy/rustdoc/build прошли. Native RU: real lost Save ACK/одна запись,
stored Apply FixRPM2400→60%, dedicated restart с прежним ID/policy/applied и
GUI label без повторного Apply. RPC inventory80/80 не является full parity.
[Evidence](evidence/2026-10-17-cooling-mode-identity.json).
Стандартный initializer описан выше. Полный Windows group mapper и согласование
FanAndLighting reset с полными8/3 banks закрывается отдельным checkpoint ниже. Старый native
checkpoint использовал seed; новый fresh-store checkpoint его отключает. Произвольное создание/удаление modes не добавлено:
source LCDV2 initializes фиксированный catalog, а прежнее общее слово CRUD не
доказывало отдельного Windows сценария создания новых IDs.


## Factory mode restore

Full FanAndLighting Reset использует тот же генератор8/3, что и fresh-store
initializer: canonical names с explicit default marker, fan StandardSpeed/pump
PWMQuiet, requested policy и выбранные ID вместе, hysteresis0/fallback100.
Он заменяет старые entries и меняет bank revision; старый Save CAS отклоняется.
Receipt domain сохраняется: digest уже включает полный payload. Старый receipt
доступен для readback; новые defaults не переписывают исторический результат.

«Вернуть стандартное имя» меняет только имя в локальном редакторе. «Стандартные
параметры в черновик» готовит одну factory policy и имя через stateless Tauri
helper. Для кривой нужен явно выбранный известный Celsius source; FixRPM2200
не требует датчика. Неизвестный ID или неверный target отклоняются. Invalid
numeric draft можно восстановить, но запоздалый ответ не перезаписывает новые
правки, имя или другой device/context. Некорректный ответ сохраняет черновик.

Ни одно из этих действий не делает Save или Apply. После восстановления policy
прежняя stored-mode association снимается: Apply до Save применяет именно
показанный новый снимок как custom policy. Explicit Save может заново привязать
его к тому же ID; следующий Apply проверяет актуальную bank revision. Имена
локализуются только по explicit factory marker, пользовательский текст сохраняется.

Полный reset preview показывает8/3 entries и ID до подтверждения; private source
provenance не проецируется в summary. Screen, локальная LCD-композиция, файлы и
thermal policy сохраняются. [Reset contract](fan-lighting-reset.md).

Проверено:1274 Rust/9ignored в172 result groups,75 unique browser scenarios,
Svelte0/0, English1042/1042, strict fmt/Clippy/rustdoc/build. Native core проверил
локальное восстановление до Save/Apply и lost-ACK/restart; отдельный native
full-reset прогон проверил8/3/default IDs, Cancel, reapply и desktop/daemon restart.
Первоначально устаревший reset harness исправлен для overview entry и явного RU
fixture; промежуточные неудачи сохранены в evidence.23 scope hashes неизменны
после итоговой проверки. Результаты и ограничения записаны в
[evidence](evidence/2026-10-17-cooling-mode-reset.json). Real PWM/USB acceptance,
расширенные native fault/accessibility/lifecycle и Windows mapper остаются отдельно.
