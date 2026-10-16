# Desktop lifecycle: tray и автозапуск

Baseline: Windows L-Connect3 2.1.28.0 / Galahad II LCDV2. Сверено2026-10-15.
Preferences/XDG registration, Tauri/GUI, tray и single-instance подключены.
Базовый native workflow принят с private watcher/MockBackend; реальные desktop
sessions, installation/upgrade и расширенная failure matrix остаются открытыми.

## Reference и границы

Windows `L-Connect 3/LConnect3cs/AppSettings.cs:15-27` объявляет
MinimizeToTrayOnClose=true. `MainWindow.cs:449` проверяет эту настройку при Close;
`App.cs:346-394` создаёт/убирает tray icon. Это frozen source2.1.28, не новый
Windows runtime test. Linux реализация должна сохранить единый owned shutdown
всех media/capture/archive/group workers и отделить Hide от полного Exit.

Linux registration следует [XDG Autostart](https://specifications.freedesktop.org/autostart/latest/)
и [Desktop Entry Exec](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
Файл пользователя: `XDG_CONFIG_HOME/autostart/net.inkyquill.lconnect.desktop`.
Runtime обязан передать Store фактические XDG_CONFIG_DIRS (по умолчанию `/etc/xdg`);
чистые тесты передают private directories. Настройка относится к GUI; daemon
остаётся отдельным сервисом, его включение относится к installation acceptance.

## Реализованный слой

`desktop_settings::Store` принимает только native paths/executable, без доступа
к HID, session bus или запуска процессов. Preferences: closeToTray=true,
startMinimized=false, autostart=false. Первое значение соответствует Windows
reference; автоматическая установка autostart по умолчанию не выполняется.
Настройки подключены к managed desktop runtime; до явного Save существующая
host autostart registration не изменяется.

Versioned bounded `desktop.json` хранит revision, preferences и до двух ownership
hashes регистрации. Read/save сериализованы shared mutex и process flock;
Save проверяет target revision. Tempfile/file/parent sync предшествуют ACK.
Corrupt/unknown/oversized/redirected preferences не заменяются defaults.

Сначала сохраняется desired intent и hashes observed owned/next entry, затем
меняется `.desktop`. После отказа интеграции desired остаётся durable; readback
содержит отдельный integrationError и фактическую registration. Тот же Save
повторяет reconciliation; revision не растёт, если intent и ownership metadata
не изменились. После замены
исполняемого файла разрешён перенос с прежнего owned hash на новый путь.

Незнакомый/изменённый пользовательский `.desktop` сохраняется. Его enabled
возвращается null с problem, а не false. Учитывается system-wide entry: при
выключении создаётся owned user `Hidden=true` override вместо удаления файла
пользователя и случайного восстановления системного autostart. Системные файлы
не изменяются. Hash ownership и process lock защищают обычные cooperating writers;
враждебная одновременная смена directories/files извне не квалифицирована.

Exec не использует shell. Указывается `/usr/bin/env --` и один quoted/escaped
полный executable path, затем `--autostart`. Два слоя escaping соответствуют
Desktop Entry string/Exec; literal `%` представлен `%%`. GIO в этом окружении
не загружает запись, когда `%` находится в имени первого executable token;
фиксированный `/usr/bin/env` устраняет этот отказ. Наличие executable проверяется.
Пути с `=` или control characters отклоняются до записи intent. UTF-8 required.

## Runtime и GUI

«Настройки → Приложение и автозапуск» открывает отдельную native форму, доступную
независимо от подключения daemon. Commands `desktop_settings_get/save` принимают
только bool preferences/target revision; paths/executable выбираются native code.
Для команд есть отдельные permissions main window. UI показывает desired prefs
и actual/unknown registration отдельно. Poll не меняет локальный draft или его
base revision; внешняя новая revision блокирует Save до явного Reload. Retry
остаётся доступным, пока actual не соответствует desired, включая новый Read
без integrationError предыдущего Save.

Tray использует встроенный Tauri backend и существующий icon.png. Меню содержит
Show и Exit. Close скрывает окно только при closeToTray и подтверждённом host:
проверяются IsStatusNotifierHostRegistered, registered items, PID из bus daemon
и Id собственного item. При потере регистрации скрытое окно показывается.
StartMinimized при наличии tray скрывает начальное окно; без host видимость
восстанавливается после ограниченного startup ожидания. При недоступном tray
обычный Close завершает GUI с ожиданием workers, не оставляет невидимый процесс.

Menu Exit, полный Exit из GUI и Tauri ExitRequested используют общий requested/
completed shutdown. Сохраняется join всех imports/thumbnails/sources/http/capture/
recordings/archive/group workers; добавлен drain desktop settings IO. Shared
semaphore permit принадлежит blocking worker даже после потери caller. Shutdown
не допускает новые settings jobs и ждёт завершения уже выполняющегося Save.
Проверено настоящим удержанным file lock и разрывом caller с последующим
durable preferences/registration readback.

Single-instance plugin использует валидную D-Bus session: обычный повторный
запуск показывает существующее окно и завершает вторую копию до её workers;
повторный `--autostart` не поднимает скрытое окно. При отсутствии/некорректном
session address plugin не подключается, чтобы GUI мог открыться; этот режим не
гарантирует single-instance. `--autostart` принят GUI; сама startup visibility
зависит от сохранённого startMinimized. Daemon service не запускается/останавливается.

## Приёмка

- [x] Read defaults без autostart registration; enable/read/restart/idempotent Save,
  binary path move/disable, stale Save и два независимых Store с CAS/flock.
- [x] Integration failure после durable preferences → явный readback → retry;
  даже идентичные desired bytes не позволяют присвоить существующий чужой файл;
  чужой/изменённый файл не удаляется; неизвестный enabled остаётся null.
- [x] System entry → user Hidden override → enable/disable без изменения system file.
- [x] Corrupt/unknown/oversized/symlink preferences сохраняются без silent reset.
- [x] Настоящий desktop-file-validate и GIO launch в private D-Bus session:
  executable path с пробелами, apostrophe/quote/dollar/backtick/backslash/percent;
  fixture argv точно равен исходному executable и одному `--autostart`.
- [x] Tauri managed runtime и narrow commands/permissions, GUI desired/actual
  readback, draft/stale revision/retry и `--autostart`.23 browser regressions
  desktop settings + group/named-slot archives.
- [x] Tray Show/Exit, close-to-tray/start-minimized, собственный registered item;
  no-host startup fallback и host-loss restore в private watcher fixture.
- [x] Единый joined Exit для menu/Close/Tauri ExitRequested. Native media Close
  остановил/reaped активный encoder и убрал staging; real settings IO после
  caller loss завершён до shutdown. OS session logout/signals ещё не приняты.
- [x] Single-instance в D-Bus session: обычный повтор показывает существующее
  скрытое окно; duplicate --autostart оставляет его скрытым.
- [x] Native private watcher/menu: hide/show/Exit, no-host/host loss,3 GUI starts,
  persisted preferences, actual user registration/disable и --autostart launch.
  Native control reset desktop/daemon restart regression также принят.
- [ ] Настоящий login и GIO запуск именно GUI через entry (fixture GIO argv
  проверен отдельно); GUI/daemon startup ordering, moved executable/AppImage
  registration repair, missing indicator library и nested exit failure matrix.
- [ ] Legacy GTK chooser/AT-SPI intermittent failure: первый native media прогон
  остановился на chooser_gif с пустым accessibility application inventory;
  повтор того же кода прошёл все dialogs/Close. Сохранены оба лога.
- [ ] KDE/GNOME/Wayland/session logout/login, installation/upgrade/uninstall и
  GUI/daemon startup ordering. Хостовая регистрация сейчас не устанавливалась.

Storage: [evidence](evidence/2026-10-15-desktop-preferences.json).
Runtime/GUI/native: [evidence](evidence/2026-10-15-desktop-runtime.json).

## Граница Mini, 2026-10-15

Описанный здесь lifecycle относится к основному GUI/tray/autostart. Mini на Linux
реализуется как виджет KDE Plasma и расширение GNOME Shell, без отдельного окна
приложения. Его размещение и lifecycle принадлежат shell; закрытое основное GUI
не должно прекращать показания Mini. Оконный Tauri-черновик не является целевой
реализацией. Sway и остальные окружения отложены; окончательный список поддержки
и версии shell пока не выбраны. [Контракт Mini](mini-system-info.md).
