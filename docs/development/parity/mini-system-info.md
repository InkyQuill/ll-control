# Mini System Information: виджеты Plasma / GNOME Shell

Baseline: Windows L-Connect 3 **2.1.28.0**. Общая страница применима при LCDV2;
EdgeHub-specific tiles не являются требованием доступного оборудования.
**Целевая реализация — виджет KDE Plasma и расширение GNOME Shell. Отдельное
окно исключено.** Начальные адаптеры Plasma и GNOME реализованы; Plasma test host
и GJS/GTK проверены изолированно. GNOME panel/session и расширенная shell-приёмка остаются. Оконный черновик
не считается закрытием Mini parity. Здесь контракт и чеклист, без отдельного
пошагового плана.

Поставка Mini входит в пакет приложения: Plasma applet устанавливается в
`/usr/share/plasma/plasmoids/net.inkyquill.lconnect.mini` вместе с native module
в `/usr/lib/qt6/qml/net/inkyquill/lconnect`; GNOME extension — в
`/usr/share/gnome-shell/extensions/lconnect-mini@inkyquill.net` вместе с локальной
compiled GSettings schema. Файлы обоих adapters устанавливаются автоматически,
без определения текущей сессии установщиком; каждый shell использует своё
хранилище. Включение расширения и размещение виджета остаются явными действиями
пользователя. Поддержка версий требует отдельной session qualification;
candidate metadata не доказывает совместимость. [Поставка](package-delivery.md).

## Windows source

`LConnect3cs/MiniSystemInformationWindow.cs` в frozen reference
`reference-decompiled/2.1.28-2026-09-08/L-Connect 3`:

- `:33–60`: шесть слотов;11 system tile types. Legacy defaults:
  CPU load/power/voltage, GPU load/power/voltage.
- `:157–176`: размер масштабируется относительно minimum; расположение
  восстанавливается, максимальный размер ограничен экраном.
- `:223–240`: size scale, position и transparency сохраняются при Closing.
- `:264–309`: drag за фон, логотип показывает/активирует main window;
  transparency toggle, minimize, close button скрывает Mini.
- `:463–477`: picker включает CPU/GPU load, power, temperature, voltage,
  Network, RAMLoad и FrameratePresentedAvg.

`LConnect3cs/AppSettings.cs:277–337` хранит legacy tile types и JSON descriptors,
scale(default2.0), nullable position и transparent(defaultfalse).
Восстановленный `scratch/xaml-2.1.28-system-2026-10-10/minisysteminformationwindow.xaml`
задаёт borderless, topmost, transparent-capable окно с ручным расположением и
ограничением aspect ratio. Это source-derived требования, не Linux acceptance.

## Решение для Linux, 2026-10-15

По решению пользователя Mini реализуется как **виджет KDE Plasma и расширение
GNOME Shell**. **Отдельное окно приложения не реализуем и не развиваем.** Windows
floating window выше — источник функциональных требований к показаниям и выбору
слотов, а не требование повторить оконную архитектуру на Linux.

Sway и остальные окружения откладываются на потом. Полный список поддерживаемых
окружений пока не определён. Версии Plasma/GNOME Shell и границы совместимости
нужно определить отдельно; текущая запись не обещает поддержку всех версий,
дистрибутивов, оконных менеджеров или display managers.

Мини-панель должна получать данные из существующего daemon sensor cache и
использовать общий выбор CPU/GPU source IDs, [единицы](display-settings.md) и
[язык](interface-language.md). Не создавать второй daemon, HID session или
отдельный hardware polling loop в виджете/расширении. Для показаний использовать
ограниченный интерфейс чтения. Настройки слотов и представления не должны менять
LCD темы, охлаждение, RGB, архивы или host autostart.

Положение, размер и жизненный цикл задаются средствами соответствующей shell.
Не переносить требования Tauri Show/Hide/Minimize/topmost/drag/Close в обязательную
приёмку Linux Mini. Виджет/расширение не должны зависеть от открытого основного
GUI; отключение интеграции не останавливает daemon и управление устройством.

FPS не выводится из GPU load, refresh rate или собственного rendering rate.
Windows условно получает его через HWiNFO PresentMon. Opt-in MangoHud logged-frame mean подключён как явный file-session source;
Runtime enable/disable/путь настраиваются в основном приложении EN/RU и CLI;
Windows-equivalent semantics, capture activation/app selection и game qualification
ещё нужны. Без выбранного свежего source tile показывает unavailable, а не valid0.
[Контракт FPS](fps-source.md).
CPU/GPU voltage также может быть unavailable при существующей поддержке UI.
[Source/provider inventory](system-info-inventory.md).

## Текущий путь данных

Оконный эксперимент удалён из продуктового кода: нет Tauri Mini route, window
commands, capability, UI-кнопок или привязки к shutdown основного GUI. Его
прежние window tests не доказывают готовность shell-интеграций и не закрывают
Mini parity. Разработка возобновлена в направлении виджетов.

Для будущих shell adapters подключён `lconnectctl mini snapshot` и
`lconnectctl mini watch`. Это клиент существующего Unix IPC, не второй daemon:
ровно list_sensors/get_sensor_defaults/get_display_settings, без device discovery,
control writes, GUI или hardware polling. Snapshot — JSON, Watch — NDJSON с
периодом1s после завершения запроса; общий deadline одного чтения3s. Watch
сбрасывает повреждённое соединение и переподключается после потери daemon.
Ctrl+C или завершение helper не останавливают daemon.

Envelope v1: `version`, `observed_unix_ms`, `available`, nullable `data`, nullable
`error`. Data содержит typed `sensors`, `defaults` (config/recommended/candidates/
values) и `display`. При ошибке данные null, старый снимок не выдаётся за valid;
числа/units/labels/source IDs сохраняются. Это три чтения кеша, не атомарная
транзакция показаний; consumer обязан сверять identity/role/unit и freshness,
не смешивая чужие источники. Consumers также должны ограничить возраст последнего
полученного frame при зависании helper. Максимальная строка вывода1MiB.
Путь сокета общий с CLI: LCONNECTCTL_SOCKET либо XDG_RUNTIME_DIR/lconnectd.sock.

Private Unix tests проверяют snapshot, точный набор read-only методов, raw labels,
offline → connection → loss → reconnect с новым sample. Начальный адаптер
Plasma и GNOME описаны ниже. Полная
приёмка обоих shell adapters остаётся отдельным требованием.

## Адаптер Plasma, 2026-10-15

[Исходники и воспроизводимая сборка](../../../integrations/plasma/README.md).
Plasma 6 applet — `PlasmoidItem`, не отдельное окно приложения. Общий pure
TypeScript-модуль `integrations/common/mini.ts` хранит11 tile kinds, шесть slots,
EN/RU resources, exact role/ID/unit и freshness checks, C/F, RAM/network pairs.
FPS читает явно выбранный MangoHud source из daemon cache; без source unavailable. Дублирующийся ID или отсутствующая выбранная GPU не
заменяются другим источником. Provider labels проходят как данные, не перевод.

Небольшой Qt QML module запускает фиксированный `lconnectctl mini watch` без shell
или пользовательского command/path. Один helper принадлежит каждому applet или
открытому settings view. Выход/отключение reaps helper; повреждённая/слишком
большая строка, crash, missing CLI и silent/stale feed сбрасывают usable values.
Frame parsing ограничен1MiB; partial lines собираются до newline. Счётчик возраста
локальный/монотонный, не зависит от server wall clock. Показания старше3s не
показываются valid, silent helper дополнительно очищается через5s.

Для QML передаётся проверенный JSON-текст с явным `JSON.parse`, чтобы Qt QVariant
sequences не выдавались за обычные JS arrays. Этот путь принят настоящим QML
движком; один Node test не доказал бы совместимость Qt JS. Общий модуль собирается
под ES2017, поскольку Qt JS не принимает все более новые синтаксические формы.

Desktop representation показывает шесть карточек; compact panel representation
показывает до двух доступных значений и открывает полный popup. Реальный panel
lifecycle/keyboard/layout ещё требует приёмки.

Настройки slots/source сохраняются через [штатный Plasma KConfig](https://develop.kde.org/docs/plasma/widget/configuration/).
Config dialog редактирует draft, перечитывание frame не стирает выбор; повреждённый
или будущий record не сбрасывается автоматически. Explicit reset создаёт defaults.
Qt Quick Test прошёл mouse/keyboard выбор GPU/source, сохранность delegate/focus,
polling и явный reset future config. Полный Apply/Cancel/restart KConfig в настоящей
сессии ещё нужно проверить; он не закрывается модельными тестами.

Private staged-package acceptance: Plasma6.7.5 / Qt6.12.0, Xvfb/session D-Bus/XDG,
настоящий `plasmawindowed` test host, compiled module из staged install и реальный
CLI к synthetic Unix peer. Проверены загрузка package, EN/RU pixels с six values,
ровно три read-only метода, один helper и его reaping после Close. Реального
оборудования, Tauri GUI, host installation или активной shell session нет.
Декларируемые Plasma6 API / Qt>=6.6 — build requirements, не обещание квалификации
всех этих версий. Итоговая version matrix и real session gates открыты.

## Адаптер GNOME, 2026-10-15

[Исходники и сборка](../../../integrations/gnome/README.md). ES-module extension
создаёт panel summary и меню шести показаний; стандартный preferences process
редактирует slots/source через GSettings. Это не отдельное окно Mini.
Общий renderer сохраняет EN/RU, source identity, units/freshness и explicit FPS source selection (unavailable без source).

GJS/Gio helper запускает фиксированный CLI без shell, собирает UTF-8 до newline,
ограничивает frame1MiB, очищает malformed/offline/stale data, повторяет запуск
после выхода и завершает только свой subprocess. Disable синхронно снимает timers,
отменяет read и прекращает UI callbacks; wait_async reaps child.

GTK/libadwaita preferences держит draft до явного Save, сохраняет выбор при polling,
блокирует устаревший Save после внешнего изменения, предлагает явный Reload.
Future/corrupt config сохраняется до явного Reset/Save. Это проверка stale base,
не атомарный cross-process CAS. Язык берётся из подтверждённого feed, RU cache
сохраняется при offline; source names остаются данными.

Private проверки прошли на GJS1.90.1, GTK4.24.1/libadwaita1.10.0, с synthetic
CLI, Xvfb, отдельными XDG/D-Bus/dconf. Проверены transport errors/retry/reaping,
реальные GTK controls/signals, сохранение и readback из нового процесса с новой
private bus. GJS и js140 распакованы в `/tmp`, системные пакеты не устанавливались.
Это не приёмка panel code: GNOME Shell в окружении отсутствует. Metadata candidate51
задан явно для сборки и не является обещанием поддержки. Дополнительно GJS с настоящим CLI/private Unix peer прошёл °F, raw labels,
offline/reconnect/new RU sample, helper reaping и ровно три read-only метода.
Shell enable/disable/restart, keyboard/accessibility и установка
ещё требуют квалификации; GTK test не заменяет полную ручную навигацию.

## Проверка поставляемого Mini — 2026-10-17

`tools/test-distribution-mini.py` проверяет файлы из production Arch package:
file hashes/modes, одинаковый shared model в двух stores,4 Node model cases,
3 CTest targets (config keyboard/feed/model), actual GJS transport и shipped CLI
через три read-only Unix methods, GTK/GSettings EN/RU/offline и отдельный
процесс persistence. Optional `--plasma-native` загружает packaged applet и обе
release QML `.so` в private Plasma host; EN/RU, helper reaping и language cache
проверены. Новые CLI/prefix параметры не меняют active desktop; прежний staged
workflow тоже повторён. [Evidence](evidence/2026-10-17-packaged-mini.json).

GitHub workflow source выполняет default mode после makepkg; фактического GitHub
run пока нет. GNOME Shell panel code, реальные панели/несколько экземпляров,
login/upgrade/session/scaling/speech и поддерживаемые версии остаются открытыми.

## Чеклист целевой реализации

- [ ] Определить и квалифицировать поддерживаемые версии Plasma/GNOME Shell.
  Read-only доставка через отдельный owned CLI helper уже реализована и проверена.
- [x] Начальный виджет Plasma: шесть slots, source selection, EN/RU, cached
  readings, fixed CLI helper и штатный config draft; private staged package test.
- [ ] Plasma: полный config Apply/Cancel/restart, несколько экземпляров/panel/
  desktop, установка/обновление/удаление и восстановление после shell/session restart.
- [x] Начальный GNOME adapter: panel/menu source, GJS transport, GSettings и GTK
  preferences; private transport/config/persistence checks.
- [ ] GNOME Shell: реальное включение/настройка/отключение,
  panel lifecycle и восстановление после перезапуска session.
- [ ] Шесть persistent slots: выбор/очистка, CPU/GPU load/power/temperature/voltage,
  RAM usage/used и network RX/TX. Проверить общий CPU/GPU выбор и явные source IDs,
  несколько GPU/сетей, исчезновение источника, stale/invalid/offline/recovery.
  Не заменять выбранный источник другой GPU и не подставлять fake zero.
- [ ] Сохранение настроек и миграции: slots/source identity, повреждённая/новая
  schema, параллельные изменения, потеря ответа записи и явное восстановление.
  Проверить сохранность соседних глобальных настроек и настроек устройства.
- [ ] Общие °C/°F, язык интерфейса, keyboard/screen reader, scaling и размещение
  средствами каждой shell. Проверить обновление показаний без сброса настроек.
- [x] Opt-in MangoHud per-frame file→cache→LCD и explicit FPS source selection
  в Plasma/GNOME adapters, EN/RU, freshness/identity и native preferences проверены.
  [Checkpoint](evidence/2026-10-16-mangohud-fps.json).
- [x] Runtime FPS enable/disable/путь в основном приложении EN/RU и CLI;
  [restart/draft/cache/native checkpoint](evidence/2026-10-16-fps-runtime-settings.json).
- [ ] FPS capture activation/app selection, Windows semantics/app identity и
  real-game/shell qualification. [Контракт](fps-source.md).
- [ ] Проверить обе интеграции без Lian Li устройства, при закрытом GUI,
  после daemon restart и при потере IPC; отсутствие cooling/RGB/LCD mutations
  и дополнительного аппаратного polling.
- [ ] После возвращения к реальным тестам проверить целевые Plasma/GNOME sessions,
  login/logout, suspend/resume и установку/обновление/удаление пакетов интеграций.

Sway и другие окружения не входят в текущую очередь реализации. Расширение
списка поддержки требует отдельного решения. Новые LCD effects/Style Express
по-прежнему выполняются последними.

[Checkpoint перехода на shell feed и English default](evidence/2026-10-15-mini-shell-feed-english-default.json)
содержит source hashes, software/native evidence и незакрытые требования.

[Plasma adapter checkpoint](evidence/2026-10-15-mini-plasma-adapter.json):
исходники, строгие TS/C++/QML проверки, staged runtime и EN/RU screenshots.

[GNOME adapter checkpoint](evidence/2026-10-15-mini-gnome-adapter.json):
GJS/GTK/dconf checks и явно незакрытая Shell acceptance.
