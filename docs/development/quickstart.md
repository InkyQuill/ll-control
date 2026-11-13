# Запуск и диагностика базового LL-Control

Текущая цель — Galahad II LCDV2 (`0416:7395`) и Windows L-Connect3 2.1.28.0.
Доступны изображения, настраиваемые LCD themes с часами/realtime sensors,
управление охлаждением и RGB. Есть GUI импорт GIF/MP4, миниатюра,
библиотека, выбор/loop/stop и восстановление выбранного ролика после restart.
Пределы GUI: до1hour,256MiB source и256MiB H.264 при24fps. Diagnostic clip API
сохраняет10s/240frames/1MiB. FFmpeg с ffprobe/libx264 нужен для импорта и миниатюр.
Композиция video+sensor и расширенная приёмка остаются в M4; [подготовка media](parity/media-preparation.md).
[Roadmap](../../roadmap.md) отделяет реализацию от аппаратной приёмки.

Для записи экрана нужны ScreenCast portal/PipeWire и GStreamer с `pipewiresrc`,
`videoconvert`, `videoscale`, `videocrop`, `imagefreeze`, `jpegenc`, `x264enc`,
`h264parse`, `mp4mux`. `imagefreeze` входит в Good Plug-ins; используются
`is-live` и `allow-replace` ([документация GStreamer](https://gstreamer.freedesktop.org/documentation/imagefreeze/index.html)).
Проверить наличие элементов без обращения к устройству можно через
`gst-inspect-1.0 imagefreeze` (аналогично для остальных). В проверенном окружении
GStreamer1.28.8; реальные capture portals пока не квалифицированы.

## Сборка без обращения к устройству

Из корня репозитория, с Rust/Cargo, pkg-config и development library libudev:

```sh
cargo build --locked -p lconnect-daemon -p lconnect-cli
cargo test --locked -p lconnect-image
```

Для полного Rust suite также нужен `dbus-daemon` в PATH: logind tests
создают собственную private session bus с fake service и MockBackend.
Они не подключаются к system bus и не отправляют Suspend/Hibernate.

Для GUI нужны [Linux prerequisites Tauri2](https://v2.tauri.app/start/prerequisites/#linux) (GTK3, WebKitGTK4.1 и связанные
system development libraries), Node.js и pnpm. Полный workspace включает GUI.
Команды frontend выполняются из `desktop/`:

```sh
pnpm install --frozen-lockfile
pnpm check
pnpm check:contracts
pnpm build
pnpm exec playwright install chromium
pnpm exec playwright test
```

Browser tests используют mocks. [Native smoke](parity/native-desktop-smoke.md)
и [native keyboard](parity/native-keyboard-acceptance.md) работают под отдельным
Xvfb с MockBackend. Не запускать их GUI на активном рабочем display.
Сам `lconnectd` — production process: его запуск открывает HID и может
восстановить сохранённые настройки. Он не является offline smoke-test.

## Установка и обычный запуск

Для Arch-family есть [production package builder](parity/package-delivery.md):
он собирает GUI/CLI/daemon и автоматически включает Mini в системные каталоги
Plasma и GNOME Shell. Ручное копирование виджетов после установки пакета не нужно.
Версии shell пока candidate; включение extension/размещение applet выполняется
пользователем в своём shell. Ни сборка, ни private package acceptance не меняют
текущую сессию и не запускают production daemon. Команды ниже — отдельный
путь ручной установки из исходников.

Этот раздел предназначен для будущего запуска с устройством; при обновлении
документации эти команды не выполнялись. Сначала остановить прежний daemon,
если он уже работает. Нужна одна HID-сессия — не запускать одновременно
production daemon и низкоуровневый `verify-galahad`.

```sh
cargo install --locked --path crates/lconnect-daemon --root "$HOME/.local"
cargo install --locked --path crates/lconnect-cli --root "$HOME/.local"
mkdir -p "$HOME/.config/systemd/user"
cp packaging/systemd/lconnectd.service "$HOME/.config/systemd/user/"
systemctl --user daemon-reload
```

`--root "$HOME/.local"` важен: unit запускает `%h/.local/bin/lconnectd`.
Обычный `cargo install` без этого параметра использует другой каталог.
Для коротких команд ниже добавить `~/.local/bin` в PATH своей shell или
использовать полный путь `~/.local/bin/lconnectctl`.

Установить актуальное сгенерированное udev-правило:

```sh
sudo install -m 0644 packaging/udev/70-lianli-hid.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
```

Правило применится при следующем подключении/загрузке устройства. Оно даёт
активной локальной сессии доступ к hidraw через `uaccess` и mode `0660`.
Группа дистрибутива в правиле не нужна: пакет её не создаёт. headless/remote
permissions требуют отдельной настройки администратора.
Не применять старое правило из `legacy-scaffold`. Не переподключать внутренний
USB AIO ради проверки без аппаратного протокола.

```sh
systemctl --user start lconnectd
lconnectctl daemon status
lconnectctl devices
lconnectctl sensors
lconnectctl spec             # static hardware inventory; no pump required
lconnectctl spec --refresh   # bypass the short inventory cache
lconnectctl display-settings get
lconnectctl display-settings set --temperature-unit fahrenheit
lconnectctl display-settings set --language english  # current English catalog is partial
```

Автозапуск включается отдельно: `systemctl --user enable lconnectd`.
Его cold-boot/sleep-wake поведение ещё требует аппаратной приёмки.
Сервис и клиенты должны работать от одного пользователя с существующим
абсолютным `XDG_RUNTIME_DIR`; socket — `$XDG_RUNTIME_DIR/lconnectd.sock`.
`LCONNECTCTL_SOCKET` переопределяет только CLI connection, не socket daemon/GUI.

GUI запускается из `desktop/` командой `pnpm tauri dev`. Она открывает настоящее
окно и подключается к уже запущенному daemon. Закрытие окна не останавливает
LCD, clocks, cooling или RGB policies. Готового проверенного installer пока нет.

## Первый экран и датчики

1. В Sensors выбрать реальные источники по ID, единице и статусу. Число без
   подходящей единицы не заменяет температуру; отсутствующий датчик не равен нулю.
2. В Display загрузить активную тему либо создать preset Focus/Duo/Triple/Quad/
   Cycle/Clock. Назначить источники явно, изменить текст/цвета/позиции/шрифт.
3. Для своей картинки импортировать PNG/JPEG/WebP/BMP и добавить image layer.
   Импорт сохраняет managed asset, но сам по себе не меняет LCD. EXIF orientation
   применяется при импорте; старый неправильно нормализованный asset нужно
   повторно импортировать из оригинала.
4. Проверить Preview. Live preview показывает daemon render, не чтение LCD.
5. Apply отправляет тему и ставит сохранение в очередь. Проверить отдельно
   requested/applied/error и индикатор сохранения. Ошибка сохранения означает,
   что работающий экран ещё не гарантирован после restart.
6. Profiles сохраняют/загружают черновик. Загрузка, rename/delete/reset профиля
   не заменяют явное Apply. Новые темы используют CSS em sizing/shaping;
   старые темы без `line_box:true` сохраняют legacy layout.

Формат темы: [пример v1](examples/lcd-theme.json),
[поля и CLI workflow](parity/lcd-theme-workflow.md),
[правила совместимости](parity/schema-compatibility.md). Авторитетные типы и
ограничения находятся в [IPC theme schema](../../crates/lconnect-ipc/src/theme.rs).

CLI-вариант с собственным JSON:

```sh
lconnectctl --json sensors
cp docs/development/examples/lcd-theme.json /tmp/my-lcd-theme.json
# Заменить REPLACE_WITH_SENSOR_ID точным ID температурного источника.
lconnectctl theme preview /tmp/my-lcd-theme.json /tmp/my-lcd-preview.jpg
lconnectctl theme apply /tmp/my-lcd-theme.json
lconnectctl theme state
lconnectctl lcd state
```

Preview создаёт новый JPEG и откажется перезаписать существующий файл.
При нескольких устройствах указать `--device` с точным ID из `devices`.
Для возврата штатного экрана: `lconnectctl lcd clear` (LocalUI).
Это останавливает LCD source, но не сбрасывает охлаждение или RGB.

Cooling и Lighting требуют явного Apply. До физического теста помпы нужны
известные рабочие PWM/source для восстановления. Вентиляторы пользователя,
вероятно, подключены к материнской плате:0RPM на LCDV2 не означает неисправность.
Остановка daemon не восстанавливает прежние физические PWM автоматически.
Подробности — [cooling protocol](parity/lcdv2-m2-cooling-acceptance.md).

## Ролик в именованном screen slot

Выберите ролик в «Библиотеке роликов», откройте «Сохранённые профили» и
обновите список. Выберите один из восьми слотов, задайте имя и нажмите
«Сохранить выбранный ролик в слот» (для занятого слота — «Заменить профиль
выбранным роликом»). Сохранение не меняет LCD. Theme и media используют общие
восемь слотов; замена media-профиля черновиком темы также доступна явно.

«Загрузить профиль в редактор» подготавливает ролик отдельно от черновика темы;
воспроизведение начинается только по «Применить ролик из профиля».
CLI: `lconnectctl profiles apply screen 7` применяет восьмой слот; индексы CLI
нулевые. Save принимает NamedProfile JSON с `content.kind="screen_media"`
и `content.value` равным ключу из `lcd clips`.

## Перенос одного профиля с файлами

В «Сохранённых профилях» выберите слот и откройте «Импорт и экспорт профиля».
Экспорт и импорт открывают системный диалог выбора файла. Панель показывает
прогресс и отмену; после потерянного ответа перечитайте состояние по сохранённому
ID. Импорт сохраняет текущий черновик редактора и не запускает LCD.

CLI экспортирует named slot вместе с изображениями, пользовательскими шрифтами
и видео, включая неактивные страницы темы:

```sh
lconnectctl profiles export screen 0 night.lcp
lconnectctl profiles import screen 7 night.lcp
```

Export создаёт новый файл, существующий не перезаписывает. Import заменяет
только выбранный слот; настройки устройства не применяются. Screen slots0…7,
fan/pump0…3. После импорта загрузите профиль в редактор и примените явно.
Это собственный Linux формат одного слота; полная screen/fan+lighting группа
и Windows archive interoperability ещё не реализованы.

ID операции и progress выводятся в stderr. При потерянном ответе Finish
используйте `lconnectctl profiles archive-state ID` или `profiles archive-finish ID`:
receipt переживает restart и не восстанавливает позднее удалённый слот.
`profiles archive-cancel ID` закрывает незавершённую передачу; уже committed import
не отменяется. [Контракт и границы](parity/profile-archives.md).

## Перенос полной группы настроек

CLI переносит Screen (текущая сцена, галереи и восемь слотов) либо
FanAndLighting (охлаждение и подсветка, включая сохранённые режимы и слоты).
`group import` заменяет выбранную группу и выполняет Apply; соседняя группа
сохраняется. В GUI откройте «Настройки → Резервная копия группы настроек»:
выберите группу, экспортируйте её либо выберите `.lcg` для проверки. После
предпросмотра отдельная кнопка «Восстановить и применить группу» заменяет группу
и выполняет Apply. ID/device/group сохраняются для чтения результата после
потери связи; повторное применение — отдельная кнопка. Черновики сохраняются.

```sh
lconnectctl profiles group export screen backup-screen.lcg --device DEVICE
lconnectctl profiles group export fan-and-lighting backup-cooling-rgb.lcg --device DEVICE
lconnectctl profiles group inspect screen backup-screen.lcg --device DEVICE
lconnectctl profiles group import screen backup-screen.lcg --device DEVICE
lconnectctl profiles group import fan-and-lighting backup-cooling-rgb.lcg --device DEVICE
lconnectctl profiles group readback screen OPERATION_ID --device DEVICE
lconnectctl profiles group reapply screen OPERATION_ID --device DEVICE
```

Export создаёт новый файл без перезаписи. Inspect проверяет данные без изменения
настроек/галереи. ID операции печатается до передачи: после потери ответа или
соединения читайте readback по тому же ID. Принятый Finish продолжает работу после
выхода клиента. Storage receipt и runtime error отражаются отдельно.
Reapply явно повторяет Apply сохранённой группы без нового импорта; более новые
сохранённые изменения блокируют старый receipt. Для CAS automation предусмотрен
`--expected` с opaque runtime token из readback. Формат LCGROUP1 — собственный
Linux archive; Windows ZIP mapping ещё открыт.
При старте daemon сверяет durable ownership незавершённых group imports до replay.
Принятые файлы сохраняются, непринятая подготовка убирается с проверкой refs. При
неподтверждённом commit файлы сохраняются; readback сообщает причину, Apply из
журнала не повторяется. Диагностика хранится в `import-publications/recovery-report.json`
под config root daemon. Повреждённый journal останавливает startup recovery;
сохраните его и logs для диагностики, прежде чем менять файлы вручную.
[Полный контракт и границы](parity/profile-groups.md).

## Tray и автозапуск GUI

Откройте «Настройки → Приложение и автозапуск». Здесь можно включить автозапуск
при входе в систему, стартовать свёрнутым и скрывать окно в tray при Close.
«Сохранить» регистрирует GUI в пользовательском XDG autostart; actual status
показывается отдельно от сохранённого желания. При ошибке или внешней registration
файл сохраняется, а повтор остаётся явным действием. Фоновое чтение не заменяет
несохранённый draft; при новой revision сначала обновите значения.

При доступном tray меню значка содержит «Показать» и «Выйти». Close скрывает окно;
полный Exit доступен в том же меню и форме настроек. Если tray отсутствует,
начальное скрытое окно возвращается, а обычный Close завершает GUI с ожиданием
активных операций. Исчезновение host возвращает скрытое окно. Повторный обычный
запуск показывает существующее окно; duplicate --autostart не меняет его видимость.

Daemon остаётся отдельным сервисом: выход из GUI не останавливает охлаждение/LCD.
Автозапуск GUI не включает daemon service и требует его обычной установки.
Реальные DE/login/logout/upgrade и AppImage ещё не квалифицированы.
[Контракт и checklist](parity/desktop-lifecycle.md).

## Сброс охлаждения и подсветки целиком

Для GUI откройте «Настройки → Резервная копия группы настроек», выберите
«Вентиляторы и подсветка» и CPU temperature sensor. Подготовьте reset, проверьте
показанные defaults и подтвердите отдельной кнопкой. До подтверждения настройки
не меняются; FileChooser не нужен.

CLI:

```sh
lconnectctl profiles group reset-fan-and-lighting --sensor CPU_TEMPERATURE_ID --device DEVICE
lconnectctl --json profiles group readback fan-and-lighting OPERATION_ID --device DEVICE
```

Он возвращает Fan Standard/Pump Quiet, Rainbow, topology360/24LED/sync=false,
настройки неактивных RGB modes и все4 fan/4 pump slots. Screen, media/files и
общая thermal alert policy сохраняются. Для Linux нужен явный stable CPU
температурный ID; отсутствие readings включает policy fallback и отражается в readback.

ID и target token печатаются до запроса. После потери ответа сначала читайте
readback; для retry нужны прежние `--id`, `--expected` и `--sensor`. Повтор не
затирает новые настройки; explicit `profiles group reapply fan-and-lighting ID`
использует текущий runtime token и не делает новый commit. GUI сохраняет ID
для readback после перезапуска. В том же открытом worker после потерянного ответа
доступен явный повтор запроса по исходному ID; автоматического reset нет. [Контракт и checklist](parity/fan-lighting-reset.md).

## Удаление сохранённого ролика

В «Библиотеке роликов» выберите ролик, нажмите «Удалить ролик…» и подтвердите
его имя. Исходный GIF/MP4 не удаляется. Используемый или сохранённый для
автовосстановления ролик защищён, включая настройки отключённых устройств.
Сначала снимите соответствующий выбор через LocalUI или другую тему,
удалите/замените ссылающиеся screen slots и дождитесь сохранения настроек. При ошибке записи сначала устраните её.

CLI предоставляет `lconnectctl lcd delete-clip KEY`; список ключей —
`lconnectctl lcd clips`. Удаление не требует подключённого LCD и не посылает
команд экрану. При потере ответа перечитайте список перед повтором; GUI делает
это автоматически, но не повторяет удаление. Повреждённые/нечитаемые settings
блокируют удаление, пока нельзя достоверно проверить ссылки.

## Восстановление и диагностика

| Симптом | Что проверить / сделать |
|---|---|
| GUI disconnected / socket missing | `systemctl --user status lconnectd`, затем `journalctl --user -u lconnectd -n 100 --no-pager`; тот же пользователь и XDG runtime у GUI/daemon; старый CLI socket override |
| systemd: executable not found | Наличие `~/.local/bin/lconnectd`; установка с `--root "$HOME/.local"` соответствует ExecStart |
| Устройство отсутствует | VID/PID, актуальное udev-правило и права hidraw; отсутствие второго владельца; reconnect/cold boot ещё не квалифицированы |
| Sensor unavailable/stale | Status/age/ID, применимость driver/provider; freshness3s, сбор не гарантирует точный1Hz. Не подставлять другой sensor или0 |
| Не видны voltage/Intel load | [Provider inventory](parity/sensor-provider-inventory.md): доступность платформы/permissions; Intel per-engine не равен Windows aggregate; optional WMI/NV-CONTROL требуют явной настройки |
| Apply timeout/error | Не повторять вслепую: перечитать `theme state`, `lcd state`, `cooling state` или соответствующий status. Команда могла выполниться до потери ответа; локальный draft не считается applied |
| Draft отличается от внешних настроек | Использовать явное Load/Refresh после readback; polling не должен затереть текущий draft |
| Save pending/error | Проверить GUI save status, журнал и доступность config directory/места на диске. Дождаться completed save; не считать отображение доказательством persistence |
| Missing image/font или cleanup refusal | Сохранить config и managed files; восстановить оригинал/импорт, перечитать профиль. Cleanup scan без удаления; не удалять вручную используемые assets/fonts |
| После изменения текста сдвинулась вёрстка | Проверить CSS mode, wrapping width, line height, anchors и нужный font. [Text acceptance](parity/text-wrapping-acceptance.md) описывает покрытие и оставшиеся raster/locale gaps |
| После restart нет diagnostic clip | Diagnostic stream/JPEG source не сохраняется; запустить явно. Managed theme и явно выбранный saved media clip восстанавливаются; одноразовый play-saved-clip не включает autoplay |

Если установлен другой Linux-порт L-Connect на .NET, совпадение имени пакета
`lconnect-linux` само по себе не означает, что запущен daemon этого репозитория.
На Arch проверьте фактические исполняемый файл, владельца пакета, unit и socket:

```sh
command -v lconnectctl
pacman -Qo "$(command -v lconnectctl)"  # если команда найдена
pacman -Ql lconnect-linux               # файлы установленного пакета
systemctl --user cat lconnectd
systemctl --user status lconnectd
systemctl cat lconnectd
systemctl status lconnectd
printf '%s\n' "$XDG_RUNTIME_DIR/lconnectd.sock"
ss -xlpn | grep -i lconnect
```

Для локальной сборки ожидается `~/.local/bin/lconnectd`, для собранного здесь
дистрибутива — `/usr/bin/lconnectd`; сравните `ExecStart` с нужной установкой.
У проверенного порта на .NET system unit находится в
`/usr/lib/systemd/system/lconnectd.service`, а socket —
`/run/lconnect/lconnect.sock`; это отдельный system service, не user unit этого
репозитория. Обнаружение USB
`0416:7395` подтверждает присутствие устройства, но не создание контроллера:
другой порт может не поддерживать его протокол, а доступ к hidraw или открытие
устройства могут завершиться ошибкой. Проверяйте журнал именно запущенного
daemon и права устройства, прежде чем делать вывод о причине.

Настройки и managed resources находятся в `${XDG_CONFIG_HOME:-~/.config}/lconnectd`:
`daemon.toml`, `devices/`, `theme-assets/`, `theme-fonts/`, `prepared-clips/` и profile storage.
Для резервной копии остановить daemon и копировать весь каталог, включая ресурсы.
Это filesystem backup; совместимый Windows archive import/export ещё не реализован.
При старте копия настроек может примениться к подключённому устройству.
Не стирать config directory как способ исправления одной ошибки.

## Что ещё проверить

Полный [basic parity checklist](parity/basic-parity-checklist.md) содержит
software evidence и отдельные незакрытые аппаратные пункты. Практические
протоколы: [LCD/lifecycle](parity/lcdv2-hardware-acceptance.md),
[cooling/thermal](parity/lcdv2-m2-cooling-acceptance.md). Включены brightness/
rotation, LocalUI/replace, долгий совместный прогон, реальные RPM/source,
USB reconnect, sleep/wake и cold boot. Другие устройства не квалифицированы.

## Сон и пробуждение

Daemon слушает logind на system bus и использует `sleep:delay` inhibitor.
Перед сном останавливает устройства, после пробуждения переоткрывает их,
восстанавливает сохранённую тему и инвалидирует старые host readings.
Диагностический stream автоматически не возобновляется. Runtime device ID
после wake меняется, сохранённые настройки связаны со stable serial/key.

Ошибки logind/inhibitor и превышение бюджета подготовки видны в daemon log.
При отсутствии разрешения на inhibitor подготовка остаётся best effort;
при потере сервиса observer повторяет подключение. Физическая приёмка
ещё отложена: [границы и checklist](parity/suspend-resume-acceptance.md).


## Повторное использование изображений

На странице LCD откройте «Библиотека изображений» и нажмите «Обновить изображения».
Карточки показывают ранее импортированные изображения, по8 на странице.
«Добавить в тему» добавляет слой в текущий черновик; нажмите «Применить к LCD»,
когда композиция готова. Просмотр и добавление сами не меняют экран устройства.
Новые импорты сохраняют имя файла. Старые импорты подписаны короткими ключами;
кнопкой «Переименовать» можно задать понятное имя. Оно не меняет изображение
и не требует повторного Apply. При конфликте или потере ответа библиотека
перечитывается; проверьте карточку перед повтором переименования.
Ошибку отдельной миниатюры можно повторить через обновление библиотеки.

## Fan RGB sync и число LED

Настройка глобальная для fan zone. В GUI: «Подсветка» → «Вентиляторы LCDV2» →
«Синхронизация и число LED» → явный Apply. Ручной диапазон8…50; sync использует
16 LED для280mm или24 для360mm, сохраняя ручное значение.

```sh
lconnectctl lighting set fan --mode static --color '#00d7ff' --sync-to-pump true --fan-size 280 --led-count 31
lconnectctl lighting set fan --mode static --color '#00d7ff' --sync-to-pump false
```

Второй вызов возвращает manual31; непереданные topology-поля сохраняются.
Эти настройки относятся к fan RGB выходу LCDV2, а не к разъёму материнской платы.
Физическая приёмка отложена; [software contracts](parity/rgb-software-acceptance.md).

### Данные для Mini shell-интеграций

```sh
lconnectctl mini snapshot
lconnectctl mini watch
```

Команды возвращают JSON/NDJSON из кеша daemon без GUI и дополнительного
аппаратного опроса. Watch переподключается после потери daemon; при offline
выдаётся `available:false,data:null`. Это транспорт для будущих виджета KDE
Plasma и расширения GNOME Shell, сами shell adapters ещё не готовы.
[Контракт и границы поддержки](parity/mini-system-info.md).
Новая установка приложения использует English; русский доступен через
`lconnectctl display-settings set --language russian`. Старый выбранный русский
при обновлении сохраняется.


## FPS из существующего лога MangoHud

В Settings → FPS source укажите абсолютный путь к уже созданному per-frame CSV
MangoHud (log_interval=0), включите Read FPS и сохраните. Перезапуск daemon не
нужен. Выключение сохраняет путь; старое значение отзывается, CPU/GPU продолжают
обновляться. Само приложение не включает логирование в игре. В Mini Plasma/GNOME
выберите появившийся FPS source; без выбранного/свежего source будет unavailable.

CLI: lconnectctl fps show --json; затем lconnectctl fps set --revision N
--enabled true --log-path /absolute/path/game.csv. Для отключения используйте
--enabled false и тот же путь, если хотите его сохранить. N берётся из show.
[Контракт, ограничения и оставшаяся приёмка](parity/fps-source.md).


## Если запись экрана перестала отвечать

ScreenRecord выполняет GStreamer в отдельном дочернем процессе. Cancel, Clear и
закрытие GUI завершают и reap-ят его до освобождения staging и portal session.
При timeout/crash запись не публикуется автоматически: очистите неудачную запись
и выберите источник заново. Завершённый MP4 допускается к review только после
успешного выхода worker; сохранение в gallery по-прежнему требует явного Save.

Для этого пути нужен Linux close_range с CLOSE_RANGE_CLOEXEC (kernel5.11+).
При недоступности syscall запуск recorder завершится ошибкой; основное приложение
и LCD runtime от этого не останавливаются. Дочерний процесс сам по себе не
доказывает работу реального X11/Wayland portal или восстановление из kernel I/O
stall. [Контракт и приёмка](parity/screen-record.md).


## Сохранённая firmware-операция

Settings → Saved firmware state → Read saved firmware state показывает journal
и блокировки даже без устройства или при блокировке обычных команд. Сохраните
JSON из Saved firmware diagnostics для диагностики. Это исторический снимок:
для обновления нажмите Read ещё раз. При ошибке прежний снимок очищается.

Не трактуйте No saved firmware journal or guards were found как доказательство
отсутствия live operation, а сохранённые markers — как успешный flash. Чтение не
снимает блокировки, не повторяет передачу и не восстанавливает настройки. CLI
lconnectctl firmware-recovery --json использует тот же inspection path.
[Контракт и незакрытые recovery-зависимости](parity/firmware-lcdv2.md).
