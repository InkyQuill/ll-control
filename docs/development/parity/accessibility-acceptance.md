# Desktop accessibility acceptance

2026-10-11. Проверки пользовательского управления через клавиатуру и видимости
полей. [Evidence](evidence/2026-10-11-keyboard-contrast.json). Headless Chromium
с bridge fixtures; это не полная native/assistive-technology приёмка.

| Проверено | Наблюдаемый результат |
|---|---|
| Навигация и Settings | Skip link, один current page, Tab/Space/стрелки; brightness/rotation отправляются отдельными patch |
| Cooling | Tab → fan → ввод PWM → Apply; target=fan, percent=55, до Apply нет записей |
| Lighting | Tab → RGB slider → стрелка → Apply; brightness=99, до Apply нет записей |
| Sensors | Tab → выбор CPU temperature → Apply; сохраняется выбранный ID |
| Layers | Reorder/delete сохраняют фокус на слое или Add; удаление показателя Cycle переводит фокус к ближайшему оставшемуся источнику |
| Имена controls | Все видимые input/select/textarea/button в начальных состояниях пяти страниц имеют accessible name |
| Границы полей | Enabled text/number/select fields имеют ≥3:1 к заливке и окружающему opaque background |

Общая граница полей изменена с `#3b4a60` на `#657891`: прежний browser
результат 1.967:1 воспроизведён regression test; после исправления 7 keyboard /
contrast tests прошли, frontend check/build прошли. Svelte review: изменение
только общего CSS, новых effects/state/IPC нет; diagnostics без ошибок/предупреждений.

Reflow затем [исправлен и проверен](evidence/2026-10-11-desktop-reflow.json):
при 320/640 CSS px все пять страниц, text layer, pump PWM curve и loaded
thermal rules помещаются без горизонтальной прокрутки. До исправления ширина
документа составляла 842px. Проверены границы всех видимых controls; screenshot
cooling320 просмотрен. Полный набор этих browser regressions — 9 тестов.

Осталось проверить:

- [ ] Native WebKitGTK: визуальная проверка focus ring во всех оставшихся состояниях;
  переключение окон с возвратом focus уже проверено, как и
  основные пять keyboard workflows, Tab/Shift+Tab и computed outline
  [проверены](native-keyboard-acceptance.md) на isolated GUI. Все expanded native states ещё не проверены.
- [ ] Раскрытые editors: оставшийся порядок Tab и validation/error/cancel workflows.
  Accessible names всех пяти типов слоёв, profile/font/cleanup и focus после
  удаления точки кривой проверены ниже.
- [ ] Screen reader: navigation/current page, смысл labels, pending/error/applied
  announcements и изменения состояния соединения.
- [ ] Оставшийся text contrast (input values, popup/options и другие expanded states), disabled/forced-colors, OS scaling вне проверенного ниже X11 DPI96/192 и доступность
  Apply/Cancel во всех expanded states. Native WebKit zoom200%/400% уже
  проверен для состояний keyboard harness; browser reflow320/640 также закрыт
  только для перечисленных состояний.

Эта matrix дополняет [общий чеклист](basic-parity-checklist.md). Аппаратное AIO
для клавиатурной приёмки не требуется; hardware acceptance остаётся отдельно.

## Cycle editor focus, 2026-10-11

[Evidence](evidence/2026-10-11-cycle-keyboard.json): воспроизведена потеря фокуса
после удаления последнего sensor-cycle entry. После splice и Svelte tick
выбирается следующий/предыдущий source select; проверки connected/current layer
не допускают фокусировки старого draft после его замены. Удаление первого из
двух оставляет фокус на единственном source; Tab переходит к его подписи.

13 keyboard/control/preset regressions прошли, check0/0 и production build
прошли. Svelte review: typed layer narrowing, без новых effects/IPC, focus после
DOM flush. Native keyboard/AT и остальные expanded controls этим не закрыты.

## Раскрытые редакторы и curve focus, 2026-10-11

[Evidence](evidence/2026-10-11-expanded-accessibility.json): раскрыты text,
clock, sensor, Cycle и image layers, включая parts/shadows/watermark/line boxes;
profile library, font import/licenses и оба cleanup reports с error/status.
Все видимые controls имеют accessible names. Visible DOM text проходит4.5:1
(3:1 для крупного текста) по computed color и solid backgrounds; disabled,
нативные option popups, input values, forced-colors и OS scaling не оценивались.
Все пять типов слоёв помещаются на320 CSS px без горизонтального scroll.

В Cooling воспроизведена потеря focus после удаления последней точки.
`removePoint` теперь ждёт Svelte tick и фокусирует ближайшее оставшееся поле
температуры. Guards исключают старые draft/channel/mode/connection/DOM;
point rows keyed по идентичности объекта. Regression проверяет удаление
последней и первой точки, затем Tab к PWM. Новых RPC/effects нет.

12 browser tests, check0/0 и production build прошли. Focused Svelte review
без замечаний к изменению; ESLint/lint в проекте не настроены. Native keyboard,
AT и оставшиеся expanded validation/cancel flows остаются незакрытыми.

## Отмена загрузки шрифта с клавиатуры, 2026-10-11

Воспроизведена потеря focus после Enter на «Отменить загрузку шрифта»:
кнопка удалялась из DOM, а file input оставался inactive. Теперь после
Svelte tick focus возвращается к file input только если до удаления он
принадлежал Cancel, компонент жив, connected и пользователь не выбрал
другой focus target. Disconnect/unmount не вызывают focus на старом editor.

Regression проверяет Enter→cancel→file input, сохранение выбранного другого
поля во время cancel, а также существующие success/lost-finish/disconnect/
invalid-font/retry/unmount paths. Draft и явный Apply не меняются.
Это browser keyboard acceptance, не OS file chooser или screen-reader audit.
[Evidence](evidence/2026-10-11-font-cancel-focus.json).

## Native AT-SPI bridge — 2026-10-11

В private accessibility bus проверены реальные WebKitGTK roles/names
навигации и Settings, включая enabled/focused slider после window round trip.
[Стенд и границы](native-keyboard-acceptance.md#at-spi-platform-tree--2026-10-11),
[tree и результаты](evidence/2026-10-11-native-atspi.json).
132 trusted keyboard steps/IPC readback прошли после probe.
Screen-reader speech/live announcements и остальные expanded states остаются
открытыми; наличие DOM labels или этого subtree не закрывает их автоматически.

Последующее расширение: [AT-SPI пяти страниц](native-keyboard-acceptance.md#at-spi-пяти-страниц--2026-10-11)
проверило main controls и focus перед keyboard Apply. Сохранено отдельное
ограничение native numeric semantics PWM: role entry/textbox, нет Value
interface. Оно включено в оставшуюся acceptance, а не скрыто сменой ожидаемой
роли в проверке. Полный screen-reader acceptance остаётся открытым.

## Fixed PWM/RPM numeric semantics — 2026-10-11

Изолированный эксперимент подтвердил: explicit spinbutton + ARIA values
включают Value interface, которого не было у обычного number input в этом
WebKitGTK. Production Cooling сохраняет HTML number input/keyboard handling,
добавляя role и reactive min/max/current/invalid для fixed PWM/RPM.
Пустой ввод не экспортирует NaN. Границы меняются вместе с pump/fan и PWM/RPM;
предельные/невалидные значения не подменяются допустимыми. Поведение Apply и
backend validation не менялось.

На одном input локально подавлен Svelte `a11y_no_redundant_roles`: роль обычно
избыточна для HTML number, но здесь это подтверждённый WebKitGTK workaround.
Комментарий объясняет причину; предупреждения остальных элементов не скрыты.
[WAI spinbutton pattern](https://www.w3.org/WAI/ARIA/apg/patterns/spinbutton/)
определяет назначение current/min/max/invalid; native результат, а не одна
лишь ARIA-разметка, подтверждает доступность этих значений в данном окружении.

Browser regression проверяет empty/invalid/valid, смену channel и PWM/RPM,
динамические bounds/value и отсутствие write до Apply. Native после ArrowDown
экспортирует PWM79/min10/max100, role spin button, enabled/focused и Value;
последующий Apply подтверждён IPC. **11 browser tests**, check0/0/build и
native132 keyboard steps прошли. Focused Svelte self-review без замечаний;
новых effects/RPC нет, lint в проекте не настроен.
[Evidence](evidence/2026-10-11-numeric-accessibility.json).

Закрыт исходный fixed PWM semantic gap. Native RPM, curve-point/hysteresis/
fallback numeric fields, речь screen reader и live announcements остаются
отдельными acceptance, как и остальные expanded states.

## Cooling curve numeric semantics — 2026-10-11

WebKitGTK numeric bridge расширен на температуру и PWM/RPM точек,
hysteresis и fallback. Все поля сохраняют number input/binding/keyboard
behavior и передают spinbutton/min/max/current/invalid. Empty current
не экспортирует NaN/undefined как число. Температурная точка также invalid,
если она не выше предыдущей; visual/backend validation не заменяется ARIA.
Локальные Svelte redundant-role comments ограничены проверенным workaround.

Browser: RPM800…3600, temperature0…100, hysteresis0…20, fallback20…100;
empty/out-of-range/recovery, fractional hysteresis, порядок точек и no-write
до Apply проверены. Существующая keyboard-focus регрессия удаления точек
продолжает проходить.

Native fan curve: все temperature/PWM/hysteresis/fallback fields имеют
spin button и Value с корректными bounds; ArrowUp30→31 виден через AT-SPI.
После редактирования daemon всё ещё хранит fixed policy: draft не применяется
самостоятельно. Режим восстановлен клавиатурой, затем остальная приёмка пяти
страниц прошла.147 trusted steps;11 browser tests;check0/0/build/native build.
Svelte self-review без замечаний; новых effects/RPC нет, lint не настроен.
[Полное evidence](evidence/2026-10-11-curve-numeric-accessibility.json).

Native pump/RPM, остальные desktop numeric fields и speech/live announcements
остаются отдельными gates. Актуальный набор Cooling numeric semantics больше
не ограничивается одним fixed PWM полем.

## Lighting numeric semantics — 2026-10-11

Thermal CPU/GPU thresholds и Dynamic RGB thresholds/speeds получили explicit
spinbutton, Value min/max/current и aria-invalid по текущему draft. Как и
в Cooling, это нужно для native WebKitGTK: начальный thermal probe не нашёл
focused spin button у обычного number input. Non-finite current не публикуется.
Dynamic bounds следуют за unit: RPM0–3600, temperature/percent0–100;
перевёрнутые low/high и fractional speed отмечаются invalid.

В thermal Apply раньше не было frontend range/finite validation. Теперь
пустые, отрицательные и >150°C значения отклоняются до RPC с понятной ошибкой.
Дробные finite значения разрешены протоколом; step=any согласует native HTML
validity с этим контрактом. Sensor options keyed по стабильному sensor.id.

[Evidence](evidence/2026-10-11-lighting-numeric-accessibility.json) разделяет
browser invalid/recovery/no-write, native thermal keyboard→AT-SPI→IPC и
сборки. Native Dynamic RGB Value/speed probe [проверен отдельно](native-keyboard-acceptance.md#dynamic-rgb-native-numeric-acceptance--2026-10-11);
speech/live regions, zoom/dialogs и другие expanded states не закрыты.

## Text/status/error contrast — 2026-10-11

Проверен видимый DOM-текст в `main` пяти начальных страниц, ошибки Settings
patch и Display Apply, клиентская validation error кривой без sensor,
успешные Cooling/Lighting/Sensors Apply и исчезновение cooling error после
исправления. Общий helper также сохраняет прежние expanded layer/profile/
font/cleanup проверки. Используются computed sRGB цвета, alpha composition
background ancestors и порог4.5:1 для обычного текста /3:1 для крупного.

11 focused tests прошли, svelte-check0errors/0warnings.
[Evidence и hashes](evidence/2026-10-11-text-status-contrast.json).
Production не менялся. Первоначальный сценарий ошибочно ожидал отказ
permissive fixture на invalid fixed PWM; заменён реальной клиентской
validation кривой. Это не найденный дефект production или контраста.

Ограничения: только DOM text nodes внутри main; не input values, option
popups, disabled controls, gradient/image backgrounds или group opacity.
Не screenshot-based raster measurement. Forced-colors, native zoom/dialogs,
остальные expanded states и speech/live announcements остаются открытыми.

## Числовой контракт Cooling — 2026-10-11

Редактор согласован с `CoolingPolicy::validate` в `crates/llcontrol-ipc/src/cooling.rs`:
PWM и fallback — целые10…100для fan/20…100для pump; RPM — целые800…3600;
температуры −50…150°C, дробные для pump PWM curve, целые для fan/RPM curve;
гистерезис конечный0…10°C, допускает дроби. До этого GUI показывал0…100°C
и гистерезис0…20°C, а fixed invalid значения отправлялись daemon для отказа.

Теперь invalid numeric drafts отклоняются до RPC с понятной ошибкой.
ARIA invalid согласован с integer fields; `step=any` сохраняет допустимые
дроби. График отображает полный диапазон −50…150. Серверная проверка сохранена.

13focused browser tests, svelte-check0/0, frontend/native build прошли.
Browser regression проверяет empty/out-of-range/fractional inputs, отсутствие
mutation RPC при отказе, correction и mode-dependent Celsius integer checks.
Native400trusted steps/12AT-SPI variants подтверждают новые range interfaces
и прежние valid workflows. [Evidence](evidence/2026-10-11-cooling-numeric-validation.json).
Review без оставшихся замечаний. Старые ranges в предыдущих checkpoints
описывают исторические запуски и заменены этим контрактом.

Физическое PWM/RPM не проверялось; native invalid cooling scenarios отдельно
не инъецировались — они проверены browser fixtures. Full workspace/final
checkpoint и speech/live announcements этим не закрываются.

## Connection/persistence live regions — 2026-10-11

В `+page.svelte` connection и pending/saved persistence получили постоянные
polite atomic live regions. Region существует до изменения содержимого;
ошибка persistence остаётся отдельным alert без вложенного live region.

Browser regression проверяет сохранение connection node через reconnect,
переходы saved→pending→error→saved и сохранение persistence node. Два реальных
poll с новым cloned response, но тем же saved состоянием не вызывают DOM
mutations в region — нет повторного обновления текста на каждом poll.
Native AT-SPI во всех12variants показывает connection id/live=polite/
atomic=true и актуальный текст.400keyboard steps,14browser tests, check0/0,
frontend/native build прошли; review без замечаний.
[Evidence](evidence/2026-10-11-live-regions.json).

Это DOM/AT-SPI semantics. Фактическое speech/live-event announcement и
screen-reader interaction ещё не проверены; общий accessibility gate открыт.

## Фактические AT-SPI live-region events — 2026-10-11

Private AT-SPI listener запускается до GUI, пишет только события приложения
`llcontrol-gui` в fixture directory и завершается вместе с harness. Для
children-added исследуется добавленный accessible, cache очищается перед
чтением attributes; при необходимости проверяется live ancestor. События
собираются через accessibility bus, не через DOM MutationObserver.

400trusted keyboard steps/12snapshots прошли, получено24live-region events.
Assertions требуют polite pending/saved текста и assertive ошибки Dynamic RGB
«Пороги должны быть целыми…» при invalid Apply. Зафиксированы также polite
сообщения успешного применения Cooling. [Evidence с событиями](evidence/2026-10-11-native-live-events.json).
Python syntax/native build прошли. Production не менялся.

Первый listener пропускал добавленный alert child; это исправлено в harness.
Попытка отдельного reconnect probe была отклонена test IPC whitelist;
в итоговый сценарий она не включена. Connection live events пока не проверены.
Orca отсутствует; установленный speech-dispatcher сам по себе не является
screen reader. Реальное произнесение и navigation semantics остаются открытыми.

## Connection reconnect live events — 2026-10-11

Новая native проверка воспроизвела проблему: видимый connection text и
ARIA attributes менялись, но WebKitGTK не передавал наблюдаемые live events
для plain text / keyed inline span. Отдельный keyed paragraph внутри
постоянного polite atomic region исправил это: оба перехода offline/online
дали `object:children-changed:add` с нужным текстом и live=polite.
Декоративная точка aria-hidden, CSS отделён от текста; paragraph margin0.

Fixture-only probes останавливают принадлежащий harness MockBackend через
private stop marker, ждут завершения и запускают новый. Production IPC
не расширен; Rust hook имеет cfg(native-smoke). GUI подтверждает offline /
disabled Apply / online / enabled Apply. Browser проверяет сохранение region
и отсутствие mutations после двух одинаковых polling.

402trusted steps/12snapshots/37live events,13browser tests, check0/0,
frontend/native build прошли. Rust fmt/strict Clippy/docs и612tests также
прошли; Rust source не менялся после проверки. Svelte/Rust review без замечаний.
[Evidence и before-fix logs](evidence/2026-10-11-native-connection-events.json).
Фактическое speech (Orca отсутствует), zoom/dialogs и другие compositors
ещё не квалифицированы; статический live attribute не выдаётся за speech.

## Native WebKit zoom — 2026-10-11

Изолированные Xvfb/Openbox, MockBackend и private AT-SPI bus: реальный
`Webview::set_zoom` 200% и 400%, окно 1280×960. WebKit DPR равен 2/4,
viewport 640/320 CSS px; проверенные состояния всех пяти страниц не имеют
горизонтального overflow. Каждый прогон: 403 trusted keyboard steps,
12 AT-SPI variants, изменения settings/cooling/RGB/sources/theme с IPC
readback, Alt+Tab, invalid/recovery и disconnect/reconnect.
[Evidence](evidence/2026-10-11-native-zoom.json).

Устранена гонка завершения harness: после видимого reconnect GUI ждёт
доставку polite online строго после offline, затем закрывается. Таймаут
4 секунды сохраняет ошибку при отсутствии события. JSONL читается до
последнего полного newline в bytes, без декодирования незавершённого UTF-8.
Новый hook доступен только native-smoke; production IPC не расширен.

Native build, JS/Python syntax, fmt/strict Clippy и review прошли. Полный
Rust suite ранее в этом срезе:612passed/0failed/0ignored, docs passed;
после него добавлен только test probe в whitelist и скорректирован harness.

Начальный прогон 400% на окне850px дал Cooling overflow250/212CSSpx;
этот случай не объявляется исправленным. Acceptance ограничен 320CSSpx
и шире. Общий scrollWidth не доказывает отсутствие обрезки каждого текста;
OS text scaling, остальные expanded states, dialogs, speech и другие
compositors остаются открытыми. AIO не использовался.

Финальные прогоны выполнялись последовательно. Одновременный старт двух
`xvfb-run` дал конфликт фокуса на первом Tab; эти запуски не засчитаны.
Отдельно проверены отклонение initial-online до offline и чтение JSONL
с незавершённым UTF-8 tail.

## Native image chooser — 2026-10-11

[Evidence](evidence/2026-10-11-native-image-chooser.json): настоящий GTK
file chooser открыт trusted keyboard на private Xvfb/Openbox, WebKitGTK,
MockBackend и отдельном AT-SPI bus. Проверены modal type и цепочка
WM_TRANSIENT_FOR до GUI; закрытие требует исчезновения видимого диалога
и возврата активного окна приложения.

- Cancel возвращает focus в file input и не меняет draft/applied theme.
- Тот же PNG выбран дважды: Ctrl+L → путь fixture → read-only AT-SPI
  подтверждает focused entry с точным путём и enabled Open → trusted Alt+O.
  Recent list не используется как признак готовности: при первом импорте
  она может быть пустой. File input сбрасывается, повторный выбор работает.
- Импорт меняет только черновик. После Apply IPC возвращает два image layers,
  JPEG preview строится. Ключи независимые случайные; normalized PNG bytes
  совпадают. DOM FileList через JS не подменяется.

434trusted steps/12AT-SPI variants/34live events. Native build, fmt,
strict Clippy, JS/Python syntax и review прошли; общий suite не повторялся.
Production код не менялся. Опциональный сценарий:
`LLCONTROL_NATIVE_DIALOGS=1 LLCONTROL_NATIVE_ATSPI=1 python3 desktop/scripts/native-keyboard.py`.
Предварительно собрать frontend, desktop-mock и native-smoke binary как
в [native acceptance](native-keyboard-acceptance.md); запускать последовательно.

Проверены PNG chooser и cancel focus при100%/X11/C.UTF-8. Font chooser,
ошибки выбора, focus после успешного импорта, scaled dialogs, speech и другие
compositors остаются непроверенными. Аппаратная приёмка остаётся отложенной.

## Image import focus — 2026-10-11

Исправлена воспроизведённая потеря focus после успешного image import.
После завершения async action и Svelte tick фокус возвращается file input,
если до загрузки он был на нём, input ещё подключён, окно активно и
пользователь не выбрал другой элемент. При32слоях picker остаётся disabled:
focus переходит к первому image heading с tabindex=-1. Допускается также
случай, когда браузер сохранил activeElement на уже disabled picker.

13browser tests прошли: success/error, следующий Tab, сохранение нового
focus, переход на Settings во время загрузки и граница32слоёв; также
существующие display/interrupted scenarios. Svelte check0/0, frontend/native
build и review прошли. [Evidence](evidence/2026-10-11-image-import-focus.json).

Native GTK picker прошёл 434trusted steps/12AT-SPI variants/44live events.
После каждого из двух импортов read-only AT-SPI дожидается нового числа
image headings, notice и enabled picker; следующий trusted Tab исходит
именно из image input. Cancellation, draft-only, Apply, PNG bytes и JPEG
preview также прошли. Success focus при100%/X11 теперь подтверждён;
error/limit/page-switch cases проверены в browser, не native. Hardware,
font/scaled dialogs, speech и оставшиеся expanded states остаются отдельно.

## PNG chooser при WebKit zoom400% — 2026-10-11

Совместный сценарий завершён: настоящий GTK chooser при WebKit zoom400%
и viewport320CSSpx. Cancel, повторный выбор PNG, восстановление focus,
draft-only import, Apply и JPEG preview прошли;434keyboard steps,
12AT-SPI variants,41live events. Все пять page probes без горизонтального
переполнения. Frontend, mock и native builds прошли.
[Evidence](evidence/2026-10-11-image-chooser-webkit-zoom.json).

Масштабирование самого GTK dialog и OS font scaling этим не проверены:
изменён только WebKit zoom. Font chooser, native error/limit states,
speech и Wayland остаются открытыми. Измерения reflow относятся к page
probes, не ко всем раскрытым редакторам. Hardware не использовалось;
полный software suite не повторялся. Production код не менялся.

## Native font chooser — 2026-10-11

Настоящий GTK chooser в private Xvfb/DBus/WebKitGTK прошёл cancel → select →
повторный select того же TTF → выбор нового font asset клавиатурой → Apply.
Каждый импорт ждёт новый managed file и завершение upload; два сохранённых
файла побайтно совпали с fixture. IPC подтверждает выбранный font key, preview
возвращает JPEG; импорт и отмена сами не меняют активную тему.
Исправлена воспроизведённая потеря focus после успешного upload. Новый target
пользователя не перехватывается; уход со страницы и отмена покрыты browser tests.
6browser tests, check0/0, frontend/native builds, fmt и GUI strict Clippy прошли.
Нативный прогон: 539 trusted steps. [Evidence](evidence/2026-10-11-native-font-chooser.json).
Scope:100%/X11/C.UTF-8/MockBackend. Font chooser при WebKit zoom400%, OS
font/dialog scaling, native error/limit states, другие compositors и speech
остаются; физический LCD не проверялся.

## Font chooser при WebKit zoom400% — 2026-10-11

PNG и TTF cancel/select/reselect → focus → explicit font selection → Apply
и JPEG preview прошли вместе при WebKit zoom400%/320CSSpx:539 trusted steps,
12 AT-SPI variants и53 live events. Два managed font файла совпали с fixture.
[Evidence](evidence/2026-10-11-font-chooser-webkit-zoom.json).
Исходники совпали с предыдущим checkpoint; production не менялся.
Масштаб самого GTK dialog/OS font не менялся. Reflow измерен на page probes,
не во всех раскрытых editors. Speech, OS scaling, error/limit states и
другие compositors остаются; hardware и полный software suite не запускались.

## Native font errors/recovery — 2026-10-11

GTK chooser проверен с повреждённым TTF и sparse TTF размером16MiB+1.
Обе ошибки видны и отправлены как assertive AT-SPI events; focus возвращён
к picker, options и активная тема не изменились. Затем тот же корректный TTF
дважды импортирован и явно выбран/применён; ровно два managed файла совпадают
с fixture, ошибки очищены, daemon сформировал JPEG preview.
При WebKit zoom400% раскрытые error/import states имеют width320/viewport320.
543trusted steps/12variants/46live events, native build, fmt, GUI strict Clippy и syntax checks прошли.
[Evidence](evidence/2026-10-11-native-font-errors.json). Production не менялся.
OS/GTK font scaling, speech, другие editors/error/limit states и hardware
остаются отдельно. Это не полный software checkpoint.

## Системный масштаб текста и GTK chooser — 2026-10-11

[Evidence](evidence/2026-10-11-native-os-text-scaling.json): изолированные
Xvfb/Openbox/DBus, Adwaita/Sans10, XSettings Xft DPI96 и192. Эффективные
GtkSettings прочитаны до старта GUI; настройки рабочей сессии не менялись.
По [контракту GTK](https://docs.gtk.org/gtk3/class.Settings.html) XSettings
задаёт общие настройки на X11. Один settings.ini не обеспечил DPI192;
стенд это отклонил и теперь запускает собственный XSettings manager.

| Системный DPI | WebKit zoom | DPR / viewport CSS px | Высота символа GTK | Результат |
|---|---|---|---|---|
| 96 | 100% | 1 / 1280 | 19 px | 543 steps / 12 AT-SPI variants / 46 events |
| 192 | 100% | 2 / 640 | 37 px | 543 steps / 12 AT-SPI variants / 46 events |
| 192 | 200% | 4 / 320 | 37 px | 543 steps / 12 AT-SPI variants / 46 events |

Проверены пять страниц и font error/import expanded states без горизонтального
переполнения; PNG/TTF cancel/select/reselect, invalid/oversize TTF → retry,
focus, явный font selection/Apply и JPEG preview. Для шести диалогов выбора
AT-SPI подтверждает фактический текст пути, enabled Open, размеры символа,
положительные размеры и расположение поля/Open в пределах1600×1200.
Изменение WebKit zoom не меняет размер текста в GTK chooser, что также измерено.

Исправлен только harness: учёт OS DPI в DPR/viewport, измерение glyph через
поле пути (у Open нет отдельного Text node), bounded ожидание временно
отсутствующего active-window property при mapping modal. Production не менялся.
Контроль96DPI выполнен до последних Python repairs; оба192DPI прогона прошли
на итоговом harness с тем же GUI binary. Mock/native builds и syntax checks
прошли; полный suite не повторялся.

Запуск требует `xsettingsd` в дополнение к существующим зависимостям стенда:

```sh
LLCONTROL_NATIVE_ATSPI=1 LLCONTROL_NATIVE_DIALOGS=1 LLCONTROL_NATIVE_TEXT_SCALE=2 LLCONTROL_NATIVE_ZOOM=2 python3 desktop/scripts/native-keyboard.py
```

Закрыт этот X11 OS-DPI/GTK scenario. Wayland, fractional scaling, другие
font/theme configurations, screen-reader speech и остальные expanded states
этим не проверены. Hardware acceptance остаётся отложенной.
