# Язык интерфейса: реализация и оставшийся перевод

Baseline: Windows L-Connect 3 **2.1.28.0**, Galahad II LCDV2.
Этот срез реализует сохранение и переключение ресурсов. **Полный language parity
ещё не достигнут:** English отмечен в UI как partial. Explicit static catalog
заполнен; dynamic/native строки, дополнительные локали и полная runtime
квалификация остаются в работе.

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
- CLI `lconnectctl display-settings set --language english` меняет только язык.
  `--temperature-unit` меняет только единицу; обе опции допускаются вместе.
  Одна успешная запись даёт одну revision, stale draft не переигрывается.
- GUI «Настройки → Единицы и язык» сохраняет общий draft явно. Последний
  подтверждённый RPC snapshot обновляет язык и document `lang` без remount.
  Выбор в черновике не меняет UI до Save. Reload после conflict остаётся явным.
- Последний подтверждённый язык кешируется как один enum tag в localStorage
  `lconnect.interface-language`: GUI может открыть основную навигацию на нём,
  когда daemon недоступен. Пустой/неизвестный/недоступный cache даёт en-US;
  допустимый ru-RU сохраняет выбранный русский. Cache не
  заменяет серверную revision и не записывает предпочтения в daemon. Сохранение
  нового языка в этой реализации требует daemon.
- Явные `t` resources подключены в 32 Svelte components/pages. Параметры
  передаются отдельно; resource lookup не угадывает параметры из готовых строк.
  Unknown message остаётся исходной строкой. Есть переносимый pure helper и
  audit static catalog/placeholder consistency.
- Пользовательские theme/profile/media names, LCD text, sensor/device labels,
  IDs и численные параметры не проходят через translator. Новые стандартные
  надписи можно локализовать при создании; уже созданный текст не переписывается.
  Смена языка не отправляет cooling/RGB/LCD Apply.

## Проверки и текущий gate

[Evidence 2026-10-15](evidence/2026-10-15-interface-language.json): source/reference
hashes,1095 Rust tests/3 ignored,38 browser,39 contracts, frontend/Clippy/docs/fmt
и два private native workflows. Это исторический checkpoint до заполнения static catalog.


[Текущий coverage report](evidence/2026-10-15-language-static-complete-coverage.json)
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
- [ ] Квалифицировать dynamic outputs (теперь63 зарегистрированных call sites):
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
