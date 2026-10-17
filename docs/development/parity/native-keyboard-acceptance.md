# Native keyboard acceptance

Стенд `desktop/scripts/native-keyboard.py` запускает MockBackend и GUI внутри
отдельных Xvfb/DBus/Openbox. XDG config/cache/data/runtime находятся в новом
временном каталоге; AIO и реальные sensor providers не открываются.

Сборка и запуск из корня репозитория:

```sh
pnpm --dir desktop build
cargo build -p lconnect-daemon --example desktop-mock --locked
cargo build -p lconnect-gui --features native-smoke,custom-protocol --locked
python3 desktop/scripts/native-keyboard.py
```

Только feature `native-smoke` регистрирует команду `native_keyboard_step` и
соответствующую capability. Команда записывает номер шага и ключ из фиксированного
списка в marked fixture root. Python читает атомарно опубликованный запрос и
передаёт X11 key events через xdotool в приватный display. JavaScript проверяет
`isTrusted` и точный `event.key`; `.click()`, `.focus()` и synthetic keyboard
события для управления UI не используются.

Переходы используют Tab/Shift+Tab, активация — Enter, значения меняются
Home/End/стрелками. Outline проверяется в computed style сфокусированного control.
После Apply WebKitGTK может сбросить DOM focus на body из-за disabled кнопки;
стенд сохраняет последнюю достигнутую позицию для выбора направления обхода.
Первый вариант стенда ошибочно ожидал автоматический forward wrap и остановился
после Settings. Это ограничение старого теста, а не доказательство корректности
всех native focus transitions.

| Workflow | Проверяемый результат |
|---|---|
| Skip link | Первый Tab и Enter переводят focus на main |
| Settings | Яркость99 и rotation90 введены клавишами; Unix IPC readback совпадает |
| Cooling | Выбран fan, PWM79; applied readback совпадает |
| Lighting | RGB brightness99 в UI; после Apply daemon показывает manual grace |
| Sensors | CPU temperature source выбран стрелками; сохранённый ID совпадает |
| Display | Клавиатурой добавлен текст и применена тема; daemon возвращает этот слой |

Lighting readback здесь доказывает принятую ручную команду и grace, не физический
цвет. Theme readback не доказывает LCD pixels. AT/screen reader, переключение
в native dialogs/других compositors, visual focus screenshot review, native zoom и все expanded
состояния остаются отдельными gates. Предупреждение AT-SPI registry в приватной
DBus-сессии не является успешной screen-reader проверкой.

## Результат 2026-10-11

132 trusted key steps прошли: skip-link, Settings99/90, fan PWM79, manual grace,
CPU source ID и Display text Apply подтверждены реальным Unix IPC к MockBackend.
Fixture directory: `/tmp/lconnect-native-keyboard-5idtdzbp`.
[Evidence](evidence/2026-10-11-native-keyboard.json).

## Alt+Tab с возвратом фокуса — 2026-10-11

Стенд открывает второе `xmessage` окно только в приватном Xvfb. При первом
End (фокус на Settings brightness) Python отправляет Alt+Tab и проверяет
active X11 window ID, затем Alt+Tab обратно с повторной проверкой ID.
WebKitGTK регистрирует trusted blur/focus; после возврата проверяются
`document.hasFocus()` и прежний `activeElement`. Последующий End/Left
редактирует именно brightness, Apply подтверждён IPC.

132 trusted application key steps и два window-manager Alt+Tab прошли;
Settings99/90, fan79, lighting manual grace, source selection и Display Apply
также прошли. [Evidence](evidence/2026-10-11-native-window-focus.json).
Командный whitelist native bridge не расширен: переключение выполняет
внешний fixture runner. App capability и production code не менялись.
Для запуска стенда теперь нужен `xmessage` вместе с Xvfb/Openbox/xdotool.

Закрыт этот конкретный native window round trip. OS file picker/dialogs,
Wayland/другие window managers, AT, zoom и прочие expanded focus states
не считаются проверенными. AT-SPI warnings в isolated DBus остаются
ограничением стенда, не successful accessibility acceptance.

## AT-SPI platform tree — 2026-10-11

Optional fixture mode:

```sh
LCONNECT_NATIVE_ATSPI=1 python3 desktop/scripts/native-keyboard.py
```

Требует PyGObject с Atspi2.0 typelib, dbus-daemon и установленный
`/usr/share/defaults/at-spi2/accessibility.conf` (проверенное окружение).
Fixture запускает отдельную accessibility bus на unix socket в private root
и передаёт её адрес только дочерним процессам. Systemd activation обычной
изолированной DBus-сессии больше не используется для этого AT-SPI probe.
`native-atspi.py` до чтения дерева проверяет isolation marker/root/bus address.
Это не обращение к accessibility bus пользовательского desktop.

Проверены subtree `lconnect-gui`, role `document web`, skip-link, названия и
button roles пяти разделов, combobox «Ориентация», Apply buttons и slider
«Яркость LCD» со states enabled/focused после Alt+Tab. Python probe read-only;
управление по-прежнему настоящими клавишами. После probe прошли132 steps
и IPC readback пяти страниц. [Полное дерево/evidence](evidence/2026-10-11-native-atspi.json).

Это platform accessibility-tree acceptance, не screen-reader acceptance:
speech, announcements/live regions, virtual navigation, остальные страницы,
expanded states/dialogs, zoom и другие desktop environments ещё не проверены.

## AT-SPI пяти страниц — 2026-10-11

Native fixture step теперь принимает optional `probe` из закрытого списка
пяти страниц; selector лишь выбирает read-only tree assertion в Python.
Command остаётся под `native-smoke`, whitelist клавиш и production capability
не расширяются. Probe запускается до указанной клавиши на уже сфокусированном
control и не управляет UI через accessibility API.

| Страница | Проверенный focused/enabled control | Tree nodes |
|---|---|---|
| Settings | slider «Яркость LCD: 100%» | 51 |
| Cooling | entry «PWM, %» | 72 |
| Lighting | slider «Яркость RGB: 100%» | 96 |
| Sensors | combo box «CPU · Температура» | 161 |
| Display | button «Применить к LCD» | 328 |

На каждой странице проверены document/skip-link/navigation и применимые
Apply controls. После read-only probe прошли132 trusted keyboard steps с
IPC readback пяти страниц. [Полные trees/interfaces/attributes](evidence/2026-10-11-native-atspi-pages.json).

**Открытый native semantic gap:** PWM `input[type=number]` в этом WebKitGTK
экспортируется как entry, computed-role textbox, interfaces Accessible/Action/
Collection/Component/Hyperlink/Text, без Value. Имя/focus и keyboard Down→79
работают, но spinbutton min/max/current и screen-reader числовая семантика не
подтверждены. Не менять production ARIA лишь ради ожидаемого имени роли в тесте:
требуется отдельная проверка WebKitGTK/ARIA numeric mapping и AT поведения.

Остаются speech/live announcements, expanded controls/dialogs, virtual
navigation, zoom и другие compositors. Это не общий WCAG или screen-reader pass.

Follow-up: исходный fixed PWM entry/Value gap
[исправлен и проверен](accessibility-acceptance.md#fixed-pwmrpm-numeric-semantics--2026-10-11).
Новый probe требует spin button и Value bounds, затем повторно читает current79
после Down; прежние деревья entry сохраняются как evidence до исправления.

Cooling curve follow-up:147 trusted steps и
[AT-SPI numeric tree](evidence/2026-10-11-curve-numeric-accessibility.json)
проверяют6 numeric controls (две температуры, два PWM, hysteresis, fallback),
обновление temperature31 и отсутствие Apply при редактировании draft.
Основные5 page probes остаются; дополнительный cooling curve snapshot
сохраняется отдельно от fixed PWM snapshot.

## Pump RPM native acceptance — 2026-10-11

[Evidence](evidence/2026-10-11-native-pump-rpm-accessibility.json):203 trusted
keyboard steps на private Xvfb/Openbox и private AT-SPI bus; MockBackend.
Native binary пересобран с актуальным embedded keyboard script.

- Pump fixed RPM: keyboard select → Up2201; AT-SPI spin button/Value800–3600,
  current2201, focused/enabled; Apply → daemon requested RPM2201/applied55%.
- Pump RPM curve: keyboard select; AT-SPI обе temperature и RPM точки,
  hysteresis0–20 и fallback20–100 имеют spin button/Value. Temperature31
  уже отражается в native tree. Curve RPM bounds проверены перед изменением
  RPM; последующее Up2201 подтверждено DOM и daemon readback.
- Выбор temperature source клавиатурой; до Apply активен fixed_rpm, после
  Apply daemon содержит curve_rpm с первой точкой31°C/2201RPM и applied PWM
  без ошибки. Все прежние five-page, fan и Alt+Tab сценарии также прошли.

Production UI менять не потребовалось. Это закрывает focused native pump/RPM
numeric сценарий, но не speech/live regions, другие numeric страницы,
zoom/dialogs, expanded states и другие compositors. Реальный pump RPM не измерялся.

## Dynamic RGB native numeric acceptance — 2026-10-11

[Evidence](evidence/2026-10-11-native-dynamic-rgb-accessibility.json):320 trusted
keyboard steps,11AT-SPI probes, private Xvfb/Openbox/AT-SPI и MockBackend.
Все прежние five-page/cooling/thermal/Alt+Tab сценарии также прошли.

Клавиатурой выбирается pump breathing, открывается details и загружается draft.
Для Celsius после редактирования AT-SPI видит четыре enabled spin buttons:
low31/high61 с bounds0–100, speed1/99 с bounds0–100; low field focused.
До Apply daemon requested=null; после Apply readback содержит эти значения,
соответствующую unit, ненулевой applied state и отсутствие ошибки.

Переход на RPM меняет AT-SPI threshold maximum на3600, сохраняя значения и
speed bounds. До Apply daemon всё ещё содержит Celsius; после Apply — RPM.
Отключение через keyboard возвращает requested=null. Percent source в этом
прогоне не выбирался; две ветви диапазона представлены Celsius/RPM.

Production менять не потребовалось. Native Dynamic RGB numeric gate закрыт;
invalid/recovery подтверждены предыдущими browser tests, не native speech.
Speech/live regions, native invalid announcements, zoom/dialogs и другие
expanded states остаются открытыми. Физические LED не проверялись.

## Dynamic RGB invalid/recovery — 2026-10-11

Изолированный Xvfb/Openbox, private AT-SPI и MockBackend: 400 trusted key
steps, 12 AT-SPI variants прошли. После успешного Apply low31/high61 верхний
порог клавиатурой изменён на30; Apply отклонён, daemon requested configuration
не изменилась. Оба spinbutton expose INVALID_ENTRY; focus остаётся доступен,
текст ошибки прочитан через AT-SPI Text interface. Возврат high61 и Apply
снимают invalid state и сообщение; восстановленный AT-SPI snapshot также
не содержит текста ошибки. Прежние five-page/RPM/thermal/Alt+Tab flows прошли.

Исправлен только probe: GI Accessible.get_text — getter интерфейса,
а диапазон текста читается через Atspi.Text.get_text. Первый неуспешный
oracle искал текст только в accessible name; второй вызвал неверный API.
Production UI не изменялся. Python AST и JS syntax checks прошли.
[Evidence с деревьями и source hashes](evidence/2026-10-11-native-dynamic-invalid.json).

Снимки не доказывают screen-reader speech/live announcements. Другие
validation families, zoom/dialogs и физический RGB этим не закрываются.

## Обновлённые Cooling ranges — 2026-10-11

После согласования с Rust validator AT-SPI показывает температуру −50…150°C
и гистерезис0…10°C.400trusted steps/12variants прошли, включая fan/pump curves
и fixed RPM. Исторические ranges0…100/0…20 выше не являются текущим контрактом.
[Изменение и browser invalid acceptance](accessibility-acceptance.md#числовой-контракт-cooling--2026-10-11).

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

## OS text DPI вместе с WebKit zoom — 2026-10-11

Стенд поддерживает `LCONNECT_NATIVE_TEXT_SCALE=1|2`: private XSettings manager,
проверка GtkSettings и измерение фактических GTK controls через AT-SPI.
DPR и viewport учитывают произведение OS scale и WebKit zoom.
При DPI96/zoom1, DPI192/zoom1 и DPI192/zoom2 прошли543trusted steps,
12AT-SPI variants и46live events на каждый прогон. GTK glyph19→37px;
при WebKit zoom2 он остаётся37px. PNG/font import/errors/retry и reflow320
проверены. [Подробности, команда и ограничения](accessibility-acceptance.md#системный-масштаб-текста-и-gtk-chooser--2026-10-11).
