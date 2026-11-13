# Язык интерфейса: реализация и оставшийся перевод

Baseline: Windows L-Connect 3 **2.1.28.0**, Galahad II LCDV2.
Этот срез реализует сохранение и переключение ресурсов. **Полный language parity
ещё не достигнут:** English отмечен в UI как partial. Explicit static catalog
заполнен; dynamic/native строки, дополнительные локали и полная runtime
квалификация остаются в работе.


Текущая [package qualification](evidence/2026-10-17-collections-integrated-candidate.json):
731 unchanged inputs,676 browser,1245 Rust/9ignored, static980/980 и3 localization
contracts; native language/tray/gallery/recording/Spec EN/RU и production GUI0/1mock
прошли. RGB/temperature/library/collection fixes входят в этот пакет.
Полная dynamic/native/real-session qualification остаётся открытой. Датированные
срезы ниже сохраняют результаты и ограничения на момент своего прогона;
формулировки о предыдущих пакетах относятся к этим датам.

## Collection membership resources — 2026-10-17

Чтение состава, pending persistence, исчезнувшее membership и неподтверждённый
replacement readback сохраняют typed local resource до отрисовки EN/RU.
Исходная backend diagnostic остаётся literal, включая совпадение с resource key,
`ValidationError` в remote name и `{0}` в persistence detail. Повреждённый ответ
не допускается к membership filter или записи; legacy `collections:null` сохранён.
После подтверждённого импорта membership failure не превращается в import failure:
файл остаётся в каталоге, Save не предлагает повторную публикацию. Explicit reload
восстанавливает чтение; смена языка не записывает коллекции. После успешного
`set_screen_collections` добавление изображения/видео теперь читает текущий состав:
требуется revision не ниже запрошенной и наличие нового key. Более новая revision
с сохранённым key допустима; legacy null и уже существующий member не вызывают
запись. Неуспешный readback сохраняет файл и блокирует повторный Save.
[Evidence](evidence/2026-10-17-collection-diagnostics.json). Этот срез рабочего дерева
входит в `distribution-20261017-collections`; новые native membership-malformation
faults и реальные desktop/physical/scaling/speech gates остаются открытыми.

Включено в [общий кандидат collections](evidence/2026-10-17-collections-integrated-candidate.json):
676 browser,1245 Rust/9ignored, strict checks/release и native scopes.731 входов
не менялись; static980/980,49 dynamic calls — inventory. Остальные native negative
readback и реальные hardware/session/speech/scaling проверки остаются открытыми.

Native edited-image post-write faults теперь проверены в7 modes EN/RU:
14 actual commits, нет повторной публикации/записи, draft сохранён и Save blocked;
owned/raw messages и literal filename подтверждены в WebKitGTK.
[Evidence](evidence/2026-10-17-native-collection-readback.json). Изменён только test harness; пакет и production code
сохранены. Plain image/video/replacement и reconnect timing ещё открыты.

## Library resources and safe fallback — 2026-10-17

Font success/cancel statuses и local size/offset/missing-asset validation теперь
reactive EN/RU resources; backend diagnostics остаются raw. Cleanup scan и
неподтверждённый delete имеют отдельный context; для повтора нужен новый scan.
Thumbnail alt и owned profile/background fallback names переводятся, filenames
передаются literal parameters. Image decoding failure показывает retry в cards
и full preview; stable derived clip key предотвращает декодирование от metadata
polls/языка. Смена key и explicit retry сохраняют lifecycle/cancellation.

Runtime lookup и static audit принимают только own dictionary entries:
constructor/toString/__proto__ не вызывают exception и не дают false-positive
coverage. Аудит охватывает top-level lib TypeScript modules с исходными line
numbers;3 contract tests подключены в `test:localization` и GitHub workflow source.
57 affected browser cases прошли перед final own-key guard,41 library/language/
thumbnail cases и3 contracts — после. Svelte0/0/static971/971 и build приняты.
Native recording EN/RU и broad RU font/cleanup/recovery прошли до guard; native
language/restart/image workflow — после.48 dynamic calls — inventory, не missing count.
Срез включён в пакет `-collections`; полная native/physical/session
qualification и реальный GitHub run остаются открытыми.
[Evidence](evidence/2026-10-17-library-resources.json).

## Temperature grid guidance — 2026-10-17

RGB validation теперь явно называет канонические целые°C; EN/RU help объясняет
шаг1,8°F и связан с обоими inputs. Общий TemperatureInput использует одинаковое
число для display и ARIA.74 browser cases покрывают EN/RU, keyboard step,
границы, unchanged fractional draft/no write и unit roundtrip; actual native RU
подтвердил31/62°C через IPC и device-free offline preview/restart. Full keyboard/
speech/Wayland/scaling остаются открытыми. Static959/959,49 dynamic calls —
inventory; conditional own validation resource квалифицирован browser cases.
Срез включён в текущий пакет `-collections`.
[Evidence](evidence/2026-10-17-temperature-grid.json).

## RGB failure ownership — 2026-10-17

Manual/thermal/sensor-driven read/load/poll/apply/readback diagnostics теперь raw;
local validation сохраняет `ValidationError` resource и integer maximum до render.
Пределы100/3600 и retained EN/RU переключение проверены без setter RPC. Legacy
hint о fan/pump bases остаётся совместимостью с English backend phrase, но теперь
показан только для thermal action и не скрывает исходную ошибку. Typed backend
cause codes этим срезом не добавлены. Unset base-mode fallback тоже переведён.

54 affected browser cases прошли, затем22 final EN/RU cases после resource-only
addition проверили raw collisions, contexts, scoped hint и RPM range. Svelte0/0,
static959/959, frontend/native build приняты. Broad native RU повторил RGB transport/
sensor recovery, controls/profiles/restart и GUI-closed refresh.48 dynamic calls —
inventory, не missing count.
Срез включён в текущий пакет `-collections`; новые EN/RU native
fault injections и реальные RGB/accessibility/desktop gates остаются открытыми.
[Evidence](evidence/2026-10-17-rgb-messages.json).

## LCD settings/reset/collections messages — 2026-10-16

Brightness/rotation, Screen reset и Collections отделяют owned EN/RU context от
raw backend/LCD diagnostics. Смена языка не повторяет запись и не сбрасывает
rotation draft. Rejected reset требует нового подтверждения/ID; unresolved retry
сохраняет ID. Поздний matching receipt снимает старое failure предупреждение.
Collection migration/save notices и accessible remove labels переведены;
имена assets остаются literal. Static audit выявил два пропуска success resources,
которые раньше скрывались за dynamic `t(notice)`; оба исправлены.

58 browser cases прошли до последних уточнений;23 affected reset/collection cases
повторены после них, затем4 EN/RU success cases проверили добавленные resources.
Svelte0/0, static944/944 и frontend/native build приняты. Broad native RU повторил
LCD brightness lost-reply/timeout → retained draft/readback → explicit recovery,
controls/profiles/sensors/restart и GUI-closed refresh.51 dynamic calls — inventory,
не число отсутствующих переводов. Новые reset/collection native fault cases
остаются открытыми; пакет `-menu` ещё не включает этот срез.
[Evidence](evidence/2026-10-16-lcd-lifecycle-messages.json).

## Settings errors and explicit reload — 2026-10-16

Display/FPS/source settings сохраняют backend diagnostics дословно, даже если
текст совпадает с resource key. Собственные response-validation ошибки Display/
FPS хранят resource до render; recovery guidance переводится отдельно и меняет
язык без повторной записи. Display и source editor после неподтверждённой записи
требуют явный Reload: успешный background poll не снимает guard. FPS уже имел
такой guard; он сохранён. Display отвергает empty/null/false/zero language;
отсутствующее legacy поле по-прежнему означает Russian.

43 browser cases, включая20 новых, Svelte0/0, static933/933 и frontend/native
build прошли. Native language/default/restart/draft/image workflow повторён;
новые Display/FPS fault cases пока browser-only. Broad native RU source lost-reply
→ retained draft/conflict → explicit Reload → recovery Apply также повторён,
вместе с LCD/cooling/RGB/profiles/restart и GUI-closed refresh.57 dynamic calls — inventory,
не количество отсутствующих переводов. Пакет `-menu` предшествует этому срезу.
[Evidence](evidence/2026-10-16-settings-messages.json).

## Native menu — 2026-10-16

Show/Exit теперь EN/RU: English default, saved cache после GUI initialization,
confirmed runtime switching и offline restart. Узкая native-команда не меняет
settings/device state; очередь не даёт запоздалому ответу вернуть старый язык.
Actual private DBusMenu labels/actions EN/RU приняты вместе с joined lifecycle.
[Evidence](evidence/2026-10-16-menu-integrated-candidate.json).
Typed no-host/status copy квалифицирован ниже; real shell panel/pointer/speech/
scaling и другие dynamic paths ещё открыты.59 calls — inventory, не missing translation count.

## Native tray status — 2026-10-16

`host_unconfirmed` обозначает неподтверждённую регистрацию приложения, включая
возможные timeout/DBus ошибки. `unavailable` отделяет собственное пояснение от
исходной ошибки создания значка. GUI переводит resources по коду; raw diagnostic,
missing/unknown code остаются literal даже при совпадении с translation key.
Повторное чтение ready не показывает старое предупреждение HostUnconfirmed.
42 browser/140 GUI Rust и native EN/RU no-host/lifecycle прошли; static928/928,
Svelte0/0. Native creation failure, real session/panel/speech/scaling не закрыты.
Packaged `-menu` предшествует этому working-tree срезу.
[Evidence](evidence/2026-10-16-tray-status-language.json).

## Desktop diagnostics — 2026-10-16

Save/Exit failures больше не исчезают после успешного background readback.
Raw diagnostics остаются данными; owned response validation и saved notices
переводятся при render.31 browser/private native RU lifecycle приняты;
static gate926/926,59 broader dynamic calls остаются. В этом предыдущем срезе menu labels ещё RU-only; текущая qualification выше
закрывает Show/Exit/default/cache/switch; typed status квалифицирован отдельно выше.
[Evidence](evidence/2026-10-16-desktop-messages.json).

## System Information — 2026-10-16

Numbers и collection timestamp следуют подтверждённому EN/RU. Backend diagnostic
и unknown field names не переводятся по совпадению с resource key; собственный
unsupported response остаётся локализованным. Browser21/native private provider
EN/RU приняты; static gate924/924,61 broader dynamic outputs остаются открытыми.
[Evidence](evidence/2026-10-16-system-spec-language.json).

## Source video notices — 2026-10-16

VideoSourcePreview хранит причины сообщений записи как typed states: playback
failure, end/limit, added/empty, stopped, source failure и seek interruption.
Ресурсы переводятся при render, поэтому уже показанное сообщение меняет язык
без повторной операции. Ошибка canvas color preview — также собственный ресурс.
Имена файлов и внешние диагностики этим изменением не переводятся.
Static gate923/923, missing0;62 broader dynamic calls остаются для квалификации.
Инвентаризация вызовов `t(...)` не ловит прямой вывод raw owned messages и
поэтому не заменяет ручной runtime-аудит.
Проверки и границы этого среза — в
[evidence](evidence/2026-10-16-video-source-notices.json).

## Языковая политика, 2026-10-15

По решению пользователя **английский (en-US) — основной язык приложения и язык
по умолчанию**. **Русский (ru-RU) обязателен** как полноценный второй язык.
Оба входят в базовый объём: навигация, настройки, статусы, ошибки,
accessibility labels и принадлежащие приложению native menu/dialog strings.
Полнота static catalog сама по себе не подтверждает готовность обоих языков.

Остальные языки (de-DE/es-ES/fr-FR/ko-KR/zh-CN/zh-TW из Windows inventory и
возможные дополнительные локали) **отложены до этапа полировки**. Они не блокируют
базовый EN/RU parity и не должны предлагаться в UI как готовые до перевода и
проверки. Windows inventory сохраняется как reference, а не как требование
немедленно перевести все ресурсы.

Новая установка без сохранённого выбора должна открываться на English, в том
числе при offline startup и отсутствии/недоступности language cache. English —
базовый язык и fallback для отсутствующего перевода. Явно выбранный русский
сохраняется при restart/update/offline; смена default не должна его сбрасывать.
При ошибке настройки сохранять последний подтверждённый допустимый выбор,
а не незаметно переписывать файл.

**English default реализован:** новый store без файла и GUI без допустимого
language cache начинают с en-US. Явный ru-RU cache сохраняется. Существующий v1
record или ответ прежнего daemon без `interface_language` по-прежнему означает
русский; новый `DisplaySettings::default()` означает English. При сохранении
обе локали записываются явно. Чтение legacy record не переписывает файл и не
меняет revision/unit; следующий явный Save нормализует language вместе с обычной
CAS revision. Полнота dynamic/native переводов остаётся отдельным требованием.

## Windows reference

`UI/Views/Pages/Settings/GeneralPage.cs:67–80,178–190` пересекает обнаруженные
ресурсы с localeDisplayNames, меняет CurrentLocale и сохраняет настройки.
`UI/Helpers/LocaleHelper.cs` перечисляет `.baml` resources, удаляет предыдущий
resource dictionary и загружает выбранный. English — fallback. В frozen
reference `2.1.28-2026-09-08/L-Connect 3/locales` есть en-US, de-DE, es-ES,
fr-FR, ko-KR, zh-CN, zh-TW. Это замена UI resources; данный caller не меняет
OS locale/CultureInfo.

Переводы Windows не копируются. Русские исходные строки Linux и собственный
English dictionary находятся в Linux repository.

## Реализованный путь

- `DisplaySettings` v1 расширен optional `interface_language`: ru-RU/en-US.
  Отсутствующее поле в legacy record сохраняет русский интерфейс; новые записи
  всегда содержат явный ru-RU или en-US. Новый store без файла использует English. Неизвестный language отвергается, исходный файл сохраняется.
  CAS revision и durability/ownership общие с [единицами](display-settings.md).
- CLI `llcontrol display-settings set --language english` меняет только язык.
  `--temperature-unit` меняет только единицу; обе опции допускаются вместе.
  Одна успешная запись даёт одну revision, stale draft не переигрывается.
- GUI «Настройки → Единицы и язык» сохраняет общий draft явно. Последний
  подтверждённый RPC snapshot обновляет язык и document `lang` без remount.
  Выбор в черновике не меняет UI до Save. Reload после conflict остаётся явным.
- Последний подтверждённый язык кешируется как один enum tag в localStorage
  `llcontrol.interface-language`: GUI может открыть основную навигацию на нём,
  когда daemon недоступен. Пустой/неизвестный/недоступный cache даёт en-US;
  допустимый ru-RU сохраняет выбранный русский. Cache не
  заменяет серверную revision и не записывает предпочтения в daemon. Сохранение
  нового языка в этой реализации требует daemon.
- Явные `t` resources подключены в Svelte components/pages и top-level TypeScript
  helpers; `ValidationError` resources также включены в static audit. Параметры
  передаются отдельно; resource lookup не угадывает параметры из готовых строк.
  Unknown message остаётся исходной строкой. Есть переносимый pure helper и
  audit static catalog/placeholder consistency.
- Пользовательские theme/profile/media names, LCD text, sensor/device labels,
  IDs и численные параметры не проходят через translator. Новые стандартные
  надписи можно локализовать при создании; уже созданный текст не переписывается.
  Смена языка не отправляет cooling/RGB/LCD Apply.

## ScreenRecord: сообщения восстановления — 2026-10-16

Ошибки возврата из prepared review и закрытия хранят исходную диагностику и
контекст отдельно. Полная фраза формируется из ресурса с параметром при render,
поэтому EN/RU меняется и у уже открытой ошибки без повторной отмены или Apply.
Неизвестная диагностика остаётся исходной строкой, даже если совпадает с ключом
перевода (`Настройки`); имена/скобки `{0}`/разметка не интерпретируются как ресурсы.
Известные recorder/source/limit/plugin сообщения получают явные инструкции на
обоих языках. Это не общий перевод всех backend errors: неизвестные сообщения,
другие media flows и native dialogs остаются отдельным gate.

Browser-сценарии проверяют failed cleanup → смена языка → явный retry,
сохранность исходной записи и введённого имени, блокировку Save при неясной
отмене и отсутствие неявной публикации. Отдельно No source frame yet → retry
использует ту же capture reservation. Точная квалификация и native regression:
[evidence](evidence/2026-10-16-screen-record-messages.json).

## Media import: сообщения и crop-поля — 2026-10-16

Upload/prepare/confirm errors используют reactive parameterized EN/RU resources;
validation и labels manual/recorded crop переведены, accessible region name
получает номер параметром. Filename и raw diagnostics (включая точные совпадения
с UI keys, `{0}` и разметку) не переводятся и не интерпретируются. Смена языка
не повторяет команду; retry остаётся явным. Исправление Cancel guard и границы
проверки сведены в [video-editor](video-editor.md#gui-import-recovery--2026-10-16)
и [evidence](evidence/2026-10-16-media-import-recovery.json).

Последний inventory explicit static resources и dynamic call sites находится в
[coverage report](evidence/2026-10-16-media-import-messages-coverage.json).
Static completeness не является процентом готовности EN/RU и не закрывает
остальные dynamic/native messages или реальные desktop dialogs.

## Проверки и текущий gate

[Evidence 2026-10-15](evidence/2026-10-15-interface-language.json): source/reference
hashes,1095 Rust tests/3 ignored,38 browser,39 contracts, frontend/Clippy/docs/fmt
и два private native workflows. Это исторический checkpoint до заполнения static catalog.


[Coverage report от 2026-10-15](evidence/2026-10-15-language-static-complete-coverage.json)
содержит точные файлы/строки: **768/768** explicit static resource keys переведены,
missing0. **63** dynamic output call sites требуют отдельной квалификации;
media phase labels и image filter maps также подключены к `t`. Plain TS helpers,
runtime message templates и native/Rust strings не покрываются одним static count.
Первый checkpoint212/768 сохранён в [историческом report](evidence/2026-10-15-language-coverage.json).

```sh
pnpm --dir desktop audit:locales
pnpm --dir desktop check:locales
node --experimental-strip-types desktop/scripts/localization-test.mjs
```

`audit:locales` печатает остаток; `check:locales` использует `--strict` и теперь
проходит для explicit static catalog и placeholder consistency. Он не закрывает
dynamic/native/runtime gates.

- [x] Прежний default/legacy ru-RU, typed language, сохранение и restart через настоящий
  Unix RPC/CLI; неизвестный language не меняет файл, единица сохраняется.
- [x] Реактивные navigation/accessibility labels и document lang; выбор языка
  в draft не применяется заранее. Сохранённый пользовательский текст
  «Настройки» и hardware label «Датчики» остаются исходными при English UI.
- [x] External language switch в открытом cooling editor сохраняет unsaved PWM
  и не вызывает control writes; RU→EN→RU работает без remount.
- [x] Offline GUI использует подтверждённый language cache, сохраняет local draft.
- [x] Native WebKitGTK/Tauri: RU→EN, GUI/daemon restart с EN, EN→RU,
  сохранность draft и unit, joined Close,0 devices/0 HID writes.
- [x] Закончить все556 ранее отсутствовавших explicit static keys.
- [x] Сделать English default для новой установки и offline/cache fallback;
  проверить новый record, отсутствие daemon, invalid/denied cache и restart.
- [x] Сохранить явный русский и legacy v1 без language при миграции; проверить
  чтение/запись обеих локалей без сброса единицы температуры и CAS revision.
- [ ] Обеспечить полноценный обязательный русский и основной English во всех
  user-facing flows; English resource fallback без случайных русских сообщений.
- [ ] Квалифицировать остальные dynamic outputs (inventory — в последнем coverage report):
  media/gallery/editor/capture, profiles/groups, RGB/effects UI и ошибки.
  Runtime сообщения из script должны стать явными ресурсами с параметрами;
  нельзя распознавать пользовательские имена regex-подстановками.
- [x] Новые LCD drafts получают My screen / Мой экран; добавляемый текст,
  Screenshot.png / Снимок экрана.png и Screen recording / Запись экрана
  создаются на текущем языке. Уже введённые имена и LCD text не переписываются.
- [ ] Квалифицировать остальные создаваемые приложением defaults и сообщения
  в EN/RU workflows; новые имена не означают готовность всех dynamic/native строк.
- [ ] Свести фрагменты в целые фразы там, где другой язык требует иного порядка
  слов, plural forms или параметров. Полнота static keys не доказывает грамматику.
- [ ] Native tray/menu, file chooser/capture dialogs, notifications и остальные
  строки вне Svelte; поведение при отсутствии daemon и недоступном localStorage.
- [ ] Полный restart/multiple windows, CAS/lost reply, stale reads и unsupported
  cache/config; keyboard/screen reader, narrow layout и scaling на EN/RU.
- [ ] Windows/Linux font fallback, wrap/raster и date/number presentation остаются
  в базовом плане. Смена языка не означает изменение LCD clock locale или
  пользовательского текста.

Воспроизводимый native сценарий после frontend/native-smoke build:
`python3 desktop/scripts/native-interface-language.py`. Он использует частные
Xvfb/session D-Bus/XDG, MockBackend и не подключает физические устройства.

### LCD composition errors, 2026-10-16

Ошибки Apply, загрузки активной темы и возврата LocalUI хранят context отдельно
от исходной диагностики. Инструкция EN/RU формируется при render; raw message
не используется как ключ словаря. Apply/LocalUI описываются как неподтверждённый
результат: отказ или потерянный ответ не доказывают, что daemon ничего не сделал.
Смена языка не повторяет команду. Draft/name и selection/reconnect guards сохранены.
Отсутствие активной темы определяется по null-ответу и имеет отдельный app-owned
перевод, без разбора текста backend error.

Preview и connection errors используют parameterized resources, persistence
diagnostic выводится дословно. Совпадение с ключом `Настройки` не превращает
ошибку в `Settings`. Image publication/editor/media-selection описаны отдельным
срезом ниже; этот LCD contract не закрывает прочие worker errors.

`python3 desktop/scripts/native-interface-language.py --lcd-errors` после
frontend/native-smoke build проверяет четыре операции в обоих языках через
настоящий Tauri/WebKitGTK и Unix IPC. Private proxy отвергает первый запрос
каждой операции, второй передаёт MockBackend; точные counters защищают от
неявного повтора. Проверяются language switch при открытой ошибке, сохранность
имени, explicit Retry, decoded480px preview, Theme/Idle readback и normal Close.
Proxy rejection не заменяет transport disconnect/timeout или реальные HID faults.
XDG roots заданы до private D-Bus activation; private document mount очищается
по точному owned path. Обычный language/restart/image workflow тоже повторён.
[Исходники, результаты и ограничения](evidence/2026-10-16-lcd-composition-messages.json).

### Image publication и media selection, 2026-10-16

Plain image import, gallery replacement и video selection сохраняют context
отдельно от raw diagnostic. ImageEditor делает то же для source/preview/
screenshot/cancel/save errors. Известная локальная size validation остаётся
app-owned ресурсом; текст decoder/worker/daemon не распознаётся по совпадению
с ключом словаря. Смена EN/RU не меняет имя файла, crop или состав draft.

`ImportedImageMembershipError` создаётся только после acknowledged asset и
ошибки следующего membership шага. Поэтому UI может достоверно сообщить «файл
сохранён в каталоге, добавление в коллекцию не подтверждено». Remote error с
тем же name/message не считается этим исходом. При неизвестном результате
самого import/replacement/select UI говорит о неподтверждённом результате,
без предположения, что side effect отсутствовал. ImageEditor Save остаётся
one-shot до inspection/close; язык не снимает guard и не повторяет публикацию.
Selection/connection revision защищает global messages от позднего import/
gallery success/error после reconnect.

После frontend/native-smoke build:
`python3 desktop/scripts/native-interface-language.py --image-errors`.
Шесть EN/RU scenarios используют реальный Rust decoder/Unix IPC и private
MockBackend. Corrupt bytes вызывают настоящую decoder error; direct и edited
images сохраняются в каталоге, затем proxy отклоняет четыре membership writes.
Четыре файла остаются: два original96×64 и два prepared480×480. Parent проверяет
неизменность исходных коллекций; draft остаётся прежним, JPEG не отправляется.
Save blocked, исходное имя и raw diagnostic сохраняются после language switch.
Повторный `--lcd-errors` прошёл8 cases на текущем binary.

Browser проверяет68 уникальных flows, включая ambiguous committed replies,
late reconnect outcomes, forged remote error name, сохранность имени и explicit
retry/Save boundaries. Это не native lost ACK/disconnect, полная screenshot
cancellation matrix, library list/rename/delete или real portal/HID acceptance.
[Снимок исходников, файлы и результаты](evidence/2026-10-16-image-publication-messages.json).

### Library management, 2026-10-16

Image/video list errors выводят diagnostic дословно после локализованного
контекста. Delete, image rename и video import/edit outcome messages хранят
context/detail отдельно; language switch не превращает `Настройки` в `Settings`
и не отправляет новую mutation. Успешные completion notices и owned fallback
names локализованы. Настоящие metadata names остаются данными.

После rename attempt старый form сохраняет введённое имя, но его Save заблокирован.
Список автоматически перечитывается; для повторной mutation нужен явный выбор
Rename на fresh card. Ошибка readback не снимает guard, а ручной Refresh сам по
себе не подтверждает повтор. Новый Rename берёт свежую expected_revision.
Внутренний handler проверяет guard/connection/loading/disabled и при Enter.
Generation также защищает video editor opening/replacement notices; late opening
success/error после same-device reconnect проверен отдельно.

После frontend/native-smoke build:
`python3 desktop/scripts/native-interface-language.py --library-errors`.
Native seed создаёт два реальных изображения; daemon действительно меняет
metadata. Proxy подменяет первый successful reply ошибкой в каждом языке,
сохраняя соединение. UI читает реальную новую revision, сохраняет введённое имя
после смены языка и блокирует старый Save. Явный выбор fresh card позволяет
следующее переименование: четыре RPC, оба metadata records заканчивают revision3,
файлы остаются на диске, JPEG не отправляется. Это не настоящий socket disconnect.

73 browser cases включают raw resource-key diagnostics, deletion/rename committed
replies lost, failed readback, свежую revision, EN completion, untranslated names
и late editor opening. Native list/delete/timeout, расширенная replacement fault
matrix, real sessions/accessibility и прочие worker/helper diagnostics остаются.
[Точный snapshot и результаты](evidence/2026-10-16-library-management-messages.json).

Текущий media checkpoint: три English browser workflow (image crop/rotation/filter,
gallery rename/cancel, video prepare/review/cancel) сохраняют пользовательские
имена и не выполняют неявный Apply. Video browser fixture проверяет workflow,
не декодирование или playback. Private native дополнительно проверяет настоящий
image decode/rotate/480px preview без публикации. Его первые две фазы имеют
0 devices/0 HID writes; третья использует один MockBackend LCDV2 с одним mock
startup write. Физическое устройство не используется.

Текущий catalog/media checkpoint и границы проверок: [evidence](evidence/2026-10-15-language-static-catalog.json).

Полный browser suite текущего catalog/media checkpoint:333 passed,0 failed.
Svelte-check:0 errors/0 warnings; frontend build,39 contracts и strict locale
gate прошли. Это не закрывает перечисленные выше dynamic/native/locale gates.

## Полировка: дополнительные языки

- [ ] Добавить собственные de-DE/es-ES/fr-FR/ko-KR/zh-CN/zh-TW resources после
  основного EN/RU объёма; English fallback, native resources, grammar/plurals,
  fonts/layout/accessibility и restart/offline проверяются для каждой добавленной
  локали. До готовности не показывать её как поддерживаемую.

Этот отложенный объём сохраняется для дальнейшего полного language parity,
но не входит в блокирующие критерии базового этапа.

[Checkpoint перехода на shell feed и English default](evidence/2026-10-15-mini-shell-feed-english-default.json)
содержит source hashes, software/native evidence и незакрытые требования.

## Создаваемые имена и раскладки, 2026-10-15

LCD draft factory теперь принимает имя при создании; без явного имени его
базовый default — My screen. UI передаёт локализованный ресурс один раз, без
reactive перезаписи theme.name при смене языка. Имена загруженных themes/profiles
и введённый LCD text по-прежнему проходят как данные. Раскладки Focus/Duo/Triple/
Cycle/Quad/Clock сохранили свои имена; описания переводятся, sensor labels внутри
созданной темы не переводятся. Подпись неизвестного custom font использует явный
ресурс с параметром, сохранённые имена импортированных шрифтов не меняются.

Screenshot при создании получает локализованное имя PNG. Recording editor
получает локализованный начальный draft name; пользовательское переименование
передаётся в preparation без перевода. Эти локальные изменения не вызывают
LCD/cooling/RGB Apply. Preview disclaimer также подключён к ресурсу.

[Checkpoint](evidence/2026-10-15-language-created-defaults.json) различает browser
workflow, private native WebKitGTK и ещё не проверенные реальные capture portals.
Текущий static gate:772/772,0 missing;64 dynamic outputs требуют отдельной
квалификации. Исторические counts выше относятся к своим checkpoint snapshots.

## Фазы и сводки архивов, 2026-10-15

Фазы named-slot и full-group archives, названия групп и progress accessibility
labels теперь проходят через EN/RU resources. Сводки cooling policy используют
полные ресурсы с параметрами: source ID и количество точек не извлекаются из
готового текста. Manual0 остаётся0%, RPM и source identity сохраняются. UI-счётчики
переданных байт форматируются с выбранной локалью.

Сообщение об успешном импорте хранится как source + values и переводится при
render: смена языка обновляет уже показанный номер слота без повтора импорта.
Пользовательские theme/slot/file names и source IDs остаются данными, в том числе
строки с кириллицей, `{1}` и `<CPU>`. Runtime messages из worker пока остаются
неструктурированными; этот срез не заявляет готовность всех ошибок EN/RU.

[Checkpoint](evidence/2026-10-15-language-archive-ui.json) включает browser checks
и native round-trip отдельных слотов/полных групп на EN и RU с private portal,
Tauri/WebKitGTK, Unix IPC и MockBackend. Native fixtures теперь явно задают язык
в своём private store; прежняя зависимость от старого русского default убрана.

**Остаток этого checkpoint:** `archive_chooser.rs` ещё задавал русские заголовки
chooser независимо от языка. Это устранено следующим срезом ниже; worker errors
вне file chooser остаются открытыми. Физических
устройств и активной desktop session в этих проверках нет. Catalog gate текущего
среза:775/775 explicit keys,0 missing;68 dynamic call sites остаются предметом
квалификации. Это расширение исторических checkpoints, а не full language parity.

Воспроизводимые native workflows после frontend/native-smoke build:

```sh
python3 desktop/scripts/native-archive.py --english
python3 desktop/scripts/native-archive.py
python3 desktop/scripts/native-group-archive.py --english
python3 desktop/scripts/native-group-archive.py --fan
```

## Native archive chooser, 2026-10-15

GUI передаёт текущий подтверждённый/cached `presentation.language` отдельным
типизированным аргументом native Start. Он не входит в архив, operation target
или persistent settings. Native-команда принимает только `en-US` / `ru-RU`;
для прежнего caller без аргумента используется English. На момент запуска
picker язык фиксируется для этого диалога; следующая операция получает текущий
язык UI. Webview по-прежнему не передаёт filesystem path или archive bytes.

FileChooser получает локализованные title/filter label для каждой комбинации
profile/group и import/export. Заголовок полной группы теперь описывает группу,
а не профиль. Patterns `*.lcp` / `*.lcg`, current filename и URI grant не зависят
от языка. Source file names не проходят через перевод.

App-owned file chooser errors (неверный URI, несколько файлов, missing response,
unexpected request handle, disconnect и timeout) выбираются явно на языке
диалога. Технические transport/deserialize errors и прочие worker diagnostics
ещё требуют полного EN/RU-контракта; этот срез не обещает перевод всех native
ошибок или reactive смену текста уже открытого системного диалога.

[Checkpoint и точные проверки](evidence/2026-10-15-language-native-archive-chooser.json)
различают проверку D-Bus arguments и отображение реального DE picker. Private
portal теперь записывает и проверяет actual title, filter name и extension pattern;
сам диалог настоящего desktop portal этим не квалифицируется.

Native archive harness запускать последовательно: конкурентный `xvfb-run -a`
может выбрать один display для нескольких процессов. Такой конфликт тестового
окружения не является приёмкой lifecycle; результат засчитывается только после
успешного изолированного workflow и нормального закрытия каждого GUI.

## Структурированные archive notices, 2026-10-15

Native archive Status и BridgeError сохраняют `message` как исходную диагностику
и могут добавить optional `message_code`. JSON-RPC numeric `code` остаётся
отдельным полем с прежней семантикой. Код добавляется только в явном app-owned
caller; совпадение текста с известной строкой кодом не считается. Старые ответы
без кода и будущие неизвестные коды показывают raw message дословно.

Текущие native-коды: `cancelled`, `profile_export_unconfirmed`,
`group_export_unconfirmed`, `cancel_unconfirmed`, `invalid_archive_file`,
`invalid_profile_archive`, `invalid_manifest_length`, `destination_exists`.
Они проходят через anyhow context, worker completion или native command error
в UI и переводятся при render, поэтому смена EN/RU не требует повторной операции.
Два дополнительных UI-only кода защищают уже сохранённую pending operation
профиля/группы от перезаписи. Пользовательские имена не анализируются regex.

Если известный код заменяет отображаемый текст, отличающаяся исходная диагностика
доступна в plain-text details. Кириллица, `{1}`, `<CPU>` и длинные имена файлов
сохраняются и не интерпретируются как markup или placeholders. Unknown/runtime
readback diagnostics также больше не проходят через UI translator.

Код не входит в persistent archive, device settings, operation identity или
receipt. При новом job и при чтении нового remote outcome старый код сбрасывается;
из receipt/legacy diagnostic он не угадывается. Destination-exists и invalid-file
ошибки по-прежнему не разрешают overwrite/commit или неявный Apply.

[Checkpoint](evidence/2026-10-15-archive-structured-notices.json) содержит
unit/browser/native доказательства. Native named-slot workflow дополнен вторым
Export в существующий файл: actual Rust worker выдаёт typed rejection, UI
показывает EN/RU, следующий Import не наследует старый код. Полный перевод всех
worker/transport/daemon diagnostics и real desktop accessibility остаётся открыт.
Audit static keys не охватывает кодовые таблицы сам по себе: они отдельно
проверяются model/resource tests.

Screenshot owner-loss recovery имеет отдельное EN/RU сообщение: отключение
портала завершает logical request, но не выдаётся за подтверждённый Close
внешнего диалога. Unknown cancellation receipt сохраняет recovery ID. Browser
проверяет оба языка; native проверяет EN→RU после rejected Cancel/Close и новый
explicit capture. [Evidence](evidence/2026-10-16-screenshot-owner-loss.json).


## Cooling failures и локальная validation — 2026-10-16

Preset/Apply failure, polling и runtime `last_error` теперь используют собственный
EN/RU context, передавая raw diagnostic отдельным параметром. Даже diagnostic
`Настройки` не превращается в перевод `Settings`. Ошибка Apply не обещает rollback:
ответ мог потеряться после commit, поэтому предлагается readback перед явным
повтором. Черновик и выбранный source сохраняются; автоматического retry нет.

Локальная `ValidationError` хранит resource и числовые параметры до render.
Integer PWM range, fallback range и curve constraints меняют язык реактивно;
тот же metadata сохраняется при вызове `Profiles.draft()` до IPC save. Невалидные
policy/profile не отправляются. Это не изменение обработки всех raw profile/
archive ошибок: они остаются отдельным language acceptance объёмом.

[Evidence](evidence/2026-10-16-cooling-messages.json):45 browser cases,
Svelte0/0, static English903/903, frontend/native build и production-config check.
Native EN/RU выполнил actual fixed-policy commit → подмена ответа raw error →
смена языка → явный edited Apply;4 пользовательских setter RPC, final mock cmd138
MCU64,0 fan writes/0 LCD starts. Штатный двухсекундный PWM refresh допускается и
не считается повтором пользовательского Apply. Старый production package отдельно
повторил12 clock+cooling states; новое отображение ошибок в него ещё не входит.
Реальные cooling/desktop и полная native failure matrix остаются открытыми.


## Shared Profiles form — 2026-10-16

List/edit errors теперь имеют собственный reactive EN/RU context и raw detail;
ambiguous edit не повторяется автоматически, readback guard сохранён. Empty slot
fallback переведён, имена пользователей остаются literal. Class-based local
validation учитывается static locale audit:913/913, после обнаружения и добавления
одного отсутствовавшего group-mismatch resource.49 browser cases и broad native
RU profiles/theme/control/restart smoke прошли; финальные audit/build отдельно
покрывают resource-only addition. Полный native EN/RU archive/group/error inventory
ещё открыт. [Evidence](evidence/2026-10-16-profile-messages.json).


## LCD source readback — 2026-10-16

Known software-source states now render through static EN/RU resources:
Idle → No software stream / Нет программного потока; Jpeg → Image / Изображение;
DiagnosticClip → Video / Видео; Theme → LCD theme / Тема LCD. Idle reports the
absence of a software stream; it does not independently establish physical
LocalUI output. DiagnosticClip also represents ordinary prepared video.

Unknown future values remain literal data; null/missing source uses an em dash.
Raw last_error is not translated, including resource-key collisions. Labels
react to language changes without rewriting a draft or sending a mutation RPC.
39 affected browser cases (14 new state cases), Svelte0/0, static English915/915
and the native gallery workflow EN/RU passed. Native Idle→Theme→Idle labels use
actual source readback; Jpeg/DiagnosticClip have browser fixture coverage here.
62 broader dynamic outputs and complete language/native qualification remain
open. [Evidence](evidence/2026-10-16-lcd-source-labels.json).
