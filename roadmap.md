# Roadmap: parity с L-Connect 3 для Galahad II LCDV2

Сверено **2026-10-16** в `lconnect-linux-m1`. Baseline: **Windows L-Connect 3
2.1.28.0**, **Galahad II LCDV2 0416:7395**. Цель — все применимые сценарии
этого SKU. Другие устройства отложены до появления тестового оборудования.

Здесь только порядок оставшейся работы. Выполненные этапы и checkpoints —
в [сводке реализации](docs/parity/implementation-status.md), Windows provenance —
в [матрице](docs/parity/lcdv2-2.1.28-matrix.md), критерии проверки —
в [чеклисте](docs/parity/basic-parity-checklist.md).
[Индекс активных планов](docs/superpowers/plans/README.md) содержит три
незавершённых плана; новые задачи M4–M6 ниже не дублируются отдельными планами.

## Ближайший порядок

1. **Закрыть оставшиеся сценарии ScreenRecord.** Start/pause/resume/finish,
   clear, source preview, multi-interval trim/crop, review → возврат к редактированию
   → повторная подготовка → явное Save и gallery membership уже реализованы.
   Начальный Tauri/WebKit workflow проверен с synthetic source; диалог лимита
   проверен в browser, engine — с сокращённым лимитом. Выбор области до первого кадра и на паузе,
   числовые поля, visual preview и readback реализованы. Killable recorder process и startup/control deadlines, Cancel/Shutdown с reap до
   portal/staging release подключены. Остаются реальный plugin/source recovery и
   resource profiling, расширенные native crop/edit/limit/fault сценарии
   и реальные X11/Wayland PipeWire backends. Повтор последнего кадра при
   responsive delivery gap и возврат к новым кадрам проверены synthetic-тестами.
   Полный 180-секундный synthetic engine и native automatic-limit → Cancel →
   Import/review/Save приняты; error/accessibility matrix остаётся.
   [Контракт и критерии](docs/parity/screen-record.md).
2. **Завершить приёмку editor/capture.** Для VideoEdit — Windows FPS/last-frame/
   VFR/colorspace semantics, native180с, pointer/keyboard crop, большие файлы и
   fault matrix. Для image — Windows filters/raster и оставшиеся native flows.
   Для Screenshot — реальные X11/Wayland portal, region/DPI и parent window.
3. **Завершить расширенную приёмку RGB.** Manual/dynamic controls, параметры
   по mode/source/unit и сохранённый выбор Static/Dynamic с восстановлением
   реализованы. Остаются native faults/reconnect/accessibility и совместная
   media/cooling/RGB приёмка.
   Физические sync/LED topology входят в отложенный hardware checklist.
4. **Закрыть profiles/desktop, firmware и поставку.**
   [Named-slot archive](docs/parity/profile-archives.md) и
   [полные группы Screen / FanAndLighting](docs/parity/profile-groups.md)
   подключены к storage/actors, IPC/CLI и GUI. Полная группа переносится через
   native file chooser: export → новый файл; upload → Inspect → явный Apply;
   receipt readback и explicit reapply работают после Release/restart.
   Native WebKitGTK/MockBackend round-trip принят для обеих групп.
   [Durable ownership и startup recovery импорта](docs/parity/import-publication-recovery.md)
   подключены: private staging/hard links, Preparing/Armed/Keep/Discard, receipt
   reconciliation и консервативный diagnostic при неизвестном исходе. SIGKILL,
   receipt eviction и manager/IPC replay проверены программно.
   Named slots используют отдельный global target и source-bound receipt;
   legacy receipts читаются. Следующие задачи: расширенная
   boundary/fsync/resource/native fault matrix; Windows ZIP interoperability;
   cancellation/fault/expiry, real-portal и lifecycle проверки.
   Live group rollback с private catalogs/refs, [manual0](docs/parity/manual-pwm-zero.md)
   и [H.264 rotation](docs/parity/display-rotation.md) реализованы; их расширенная
   приёмка остаётся в соответствующих контрактах.
   [Fan+lighting reset](docs/parity/fan-lighting-reset.md) подключён к atomic
   coordinator/Unix IPC/CLI/GUI с Windows defaults и сохранением Screen/globals;
   read-only preview, confirmation, receipt recovery и явный retry/reapply
   подключены. Остаётся расширенная fault/lifecycle acceptance.
   [Tray/autostart](docs/parity/desktop-lifecycle.md) подключены к Tauri/GUI:
   XDG desired/actual/retry, registered tray host, Hide/Show/joined Exit и
   single-instance приняты с private watcher/MockBackend. Реальные desktop sessions,
   login/upgrade/AppImage и расширенная failure matrix ещё открыты. Далее
   [Spec inventory](docs/parity/system-spec.md) подключён к Linux provider/IPC/CLI/GUI;
   Windows/platform полноту полей ещё проверить. [Общие единицы °C/°F](docs/parity/display-settings.md)
   реализованы для readings, temperature editors и System LCD layers. Далее язык
   интерфейса: [переключение RU/EN](docs/parity/interface-language.md) подключено;
   explicit English static catalog заполнен (текущий coverage — в последнем checkpoint). **English — основной язык
   и default новой установки; русский обязателен.** English default подключён;
   legacy records и выбранный русский сохраняются. Новые имена LCD draft, screenshot
   и recording создаются на текущем языке без перезаписи введённого текста.
   Фазы, сводки, native archive chooser и structured archive notices EN/RU
   подключены. Прочие worker errors
   и оставшиеся dynamic/native flows ещё требуют перевода и проверки; остальные языки отложены до полировки.
   Следующий
   [Mini6](docs/parity/mini-system-info.md) реализуем как виджет KDE Plasma и
   расширение GNOME Shell, без отдельного окна приложения. Оконный путь удалён;
   cache-only CLI feed и начальные Plasma/GNOME adapters подключены. GNOME panel/session
   acceptance и расширенная Plasma config/session acceptance ещё нужны. Sway и остальные окружения отложены; полный список
   поддержки и версии shell пока не определены. Opt-in MangoHud logged-frame mean и explicit Mini source подключены;
   runtime enable/disable/путь доступны в основном приложении EN/RU и CLI. Capture
   activation/app selection и Windows/game qualification остаются.
   [Контракт](docs/parity/fps-source.md).
   Далее updater и packages; общая программная приёмка основного функционала.
   Аппаратные проверки выполняются после возвращения к железу по решению пользователя.
5. **Полировка: дополнительные языки.** После базового EN/RU объёма добавить и
   проверить остальные локали. Они не блокируют базовый parity. Windows language
   inventory сохраняется как ориентир; перевод/grammar/native resources,
   layout/fonts и accessibility квалифицируются для каждой добавляемой локали.
6. **Последними перенести дополнительные effects и Style Express.**
   Это дополнительный функционал; его перенос не блокирует завершение базового
   функционала LCDV2, но остаётся необходимым для full parity.
   По решению пользователя перенос оставшихся16 страниц отложен до завершения
   основного функционала. Rain/Snow, MeteorShower, Snow3D и Fireworks уже подключены
   к theme/daemon/H.264/GUI, independent page-state и slots/restart; mock/native
   happy path принят. Новые scenes сейчас не разрабатываются. Для полного parity
   позже остаются остальные страницы, native faults и Windows visual qualification.
   [Каталог и критерии effects](docs/parity/lcd-effects.md).

Отложенный объём — дополнительные LCD-сцены и Style Express. Обычные RGB-режимы,
привязки подсветки к датчикам, сохранение настроек и восстановление профилей
остаются в основной очереди. Реализованные LCD-эффекты сохраняются; исправления,
необходимые для стабильности общего LCD runtime, выполняются вместе с основным функционалом.

Приёмка новых функций входит в каждый срез. Reference/platform gates M2/M3
не блокируют независимую разработку. Аппаратные проверки отложены пользователем.

## Что больше не является новой разработкой

LCD JPEG/H.264 runtime, sensors/cooling policies, thermal/dynamic RGB,
realtime themes, persistent media/gallery, восемь screen slots, Screen reset,
video+overlays, image geometry/шесть filters, VideoCut/VideoEdit/AutoFill,
gallery re-edit, Screenshot → editor → Save и начальный ScreenRecord workflow уже реализованы. Их оставшиеся
reference/native/hardware проверки перечислены в
[чеклисте](docs/parity/basic-parity-checklist.md).
Record/Pause нарезки существующего видео не равен записи рабочего экрана.

История успешных и неуспешных прогонов — в
[сводке реализации](docs/parity/implementation-status.md). Она не заменяет
новую общую квалификацию текущего dirty tree и не доказывает full parity.

| Область | Новая функциональность | Осталась приёмка существующего |
|---|---|---|
| Media/editor/capture | Каталог effects/Style Express — последним этапом | Windows timing/colors/fonts; editor/capture native faults и реальные portals; composition workload; Screen reset recovery |
| RGB | Основные controls, mode/source/unit profiles и Static/Dynamic restore реализованы | Реальные зоны, priority/grace/base restore и работа вместе с LCD |
| Profiles/desktop | Язык интерфейса/locale, Mini6/FPS | Архивы групп и слотов, fan+lighting reset, tray/autostart, Spec, общие °C/°F; round-trip/recovery и Windows/platform interoperability |
| Sensors/text | Остатки voltage coverage и достоверный Intel aggregate | Windows mapping, locale/fallback/raster, accuracy/availability |
| Lifecycle/firmware/delivery | Safe LCDV2 updater; завершение CI/пакетов | Version/date, accessibility, install/upgrade/uninstall, общий software checkpoint и hardware matrix |

## Оставшиеся этапы

### M1 — аппаратные и lifecycle gates

[План M1](docs/superpowers/plans/2026-09-08-m1-lcdv2-dynamic-display.md):
USB detach/reattach, cold boot и sleep/wake; видимые stop/replace/LocalUI,
brightness и четыре rotation; recovery после transport failure.
Программные barriers, logind/inhibitor, reader teardown и replay уже реализованы.
Повторить совместную приёмку после завершения media/composition.

### M2 — sensor coverage и охлаждение

[План M2](docs/superpowers/plans/2026-09-09-m2-sensors-and-cooling.md):
Zenpower SVI2 Core voltage/CPU temperature подключены к cache/source picker/LCD;
CPU voltage вне поддержанных asusec/opt-in ASUS WMI/NCT CPUVC/zenpower; proprietary
NVIDIA voltage вне доступной X-сессии; соответствие общего Intel GPU load
источнику Windows. xe/i915 per-engine и manual mapping уже есть;
неоднозначный Intel VRAM used=0 остаётся unavailable.

Физически проверить pump duty/RPM/source, fallback/manual override/restart,
thermal RGB и всю доступную LED-зону, затем совместную работу с media.
Перед cooling test нужны известные рабочие настройки восстановления.
Fan test применим только при подтверждённом подключении к LCDV2: сейчас
вентиляторы, вероятно, подключены к материнской плате.

### M3 — текстовая совместимость и доступность

[Базовый план](docs/superpowers/plans/basic-parity.md): locale-specific переносы,
Windows font fallback/hinting/raster; реальное озвучивание screen reader,
оставшиеся dialogs/expanded states, Wayland/fractional scaling. Сравнивать
одинаковые входы, шрифты и locale. AT-SPI events не доказывают озвучивание.
Эти reference/platform gates не блокируют независимую разработку M4.

### M4 — media/editor/effects

Расширенная приёмка ScreenRecord и приёмка media/editor/capture.
Перенос оставшихся effects/Style Express отложен до завершения основного
функционала; принадлежность к M4 не задаёт им прежний приоритет. [Контракт v2](docs/parity/persistent-media-v2.md)
и [Windows defaults](docs/parity/gallery-defaults.md) уже исследованы.
Дополнительно проверить slow read/first-frame cancellation, corruption во время
playback и native publication disconnect/reply loss/close. Daemon lost-Finish
receipt уже проверен; native UI outcome требует отдельной приёмки.

### M5 — RGB, profiles и desktop

Profiles и desktop из таблицы; расширенная приёмка RGB. Manual catalog/defaults6/16,
отдельные applied mode profiles, dynamic mode/source/unit parameters и
Static/Dynamic choice/restore реализованы. [RGB contracts](docs/parity/rgb-software-acceptance.md).
Именованные fan curves и screen slots, rename/select/slot reset уже реализованы.
Атомарный reset всей группы слотов и полный device Screen reset
с настройками/коллекциями реализованы. Journal и settings-owner barrier для общего
recovery, LCD actor commit/Apply и коллекции реализованы. Внешний reset IPC/readback
подключён с защитой от устаревшей подготовки; explicit transport reapply и GUI подключены; native/lifecycle приёмка остаётся.
[System Information inventory](docs/parity/system-info-inventory.md) отличает
существующие выбор источников и [Spec](docs/parity/system-spec.md) от ещё
целевых Mini-интеграций/FPS. Mini — виджет Plasma и расширение GNOME Shell;
отдельное окно не реализуем. Sway и прочие окружения отложены, окончательный
список поддержки ещё не выбран.
Пакет приложения устанавливает Mini в общесистемные хранилища обоих целевых
shell: Plasma plasmoid вместе с native QML module и GNOME Shell extension со
схемами. Shell обнаруживает свой adapter; отдельная ручная установка файлов
виджета не нужна. Включение/размещение в пользовательской сессии — отдельное
действие; установка не меняет активную панель или enabled extensions.

### M6 — firmware и итоговая поставка

Software identity/read-only GUI EN/RU, exact image candidate/offline inspector,
ACK/error codec, internal actor-owned transfer с permanent fence и отдельный
joined keeper lifecycle на том же HID handle, checked cleanup/save preflight
и explicit requested cooling hold реализованы. Добавлены durable journal storage
и pre-open manager replay guard; transfer требует scoped journal token и
сохраняет effects/outcome до продолжения. Device-settings guard до side effects
защищает от missing journal и audited previous reader; обычные writes его не снимают.
Owned reconnect job после acknowledged transfer/checked original close проверяет
observed detach/same serial/exact identity на fresh handle, без replay/clear.
Serial-less/restart/missed-reset recovery, qualified resolution и product coordinator ещё открыты.
Shared process-local admission связан с IPC/main worker pools/discovery/power;
lifetime cleanup owners, checked cleanup health и explicit staging close подключены.
Serialized manager handoff и non-Clone owner исходного HID с checked keeper join
и shutdown receipts подключены. Owned prepare/transfer/close engine соединяет staging/join/drain, fresh bound
snapshot/assets, arm и original-handle transfer. Process-local registry сохраняет status/progress/cancel/
completion после потери observer; worker panic даёт Unknown с cleanup/root join,
main shutdown отменяет и ждёт job. Saved restart inspection доступен в global read-only
IPC/CLI и Settings EN/RU: journal/guard-only/inconsistent/unreadable, no-device и
sealed admission, без HID/redo/clear.
Product Apply/live-status route отсутствует; дальнейший restart recovery,
extended fault/power policy, qualified resolution/restoration и EN/RU UI ещё открыты.
Effective cooling ещё не проверено.
[Контракт, checkpoints и оставшиеся зависимости](docs/parity/firmware-lcdv2.md).
Пользовательского updater пока нет: обычный hotplug/replay нельзя использовать
для подтверждения update.

- [ ] Повторить firmware version/date/latency при media на устройстве;
  software readback не заменяет hardware acceptance.
- [ ] Завершить hardware revision/image qualification и recovery research.
- [ ] Safe LCDV2 updater coordinator: staged Inspect/explicit Apply, quiesce/join
  keepers, durable outcome/progress, identity-bound reconnect/version verification
  до любого saved-intent replay; EN/RU UI. Internal transport сам этого не делает.
- [ ] Итоговая матрица media + cooling + RGB при cold boot/hotplug/sleep/restart.
- [ ] Запуск GitHub CI после настройки remote, clean-host binary reproducibility
  и real-session install/autostart/upgrade/uninstall. Production Arch payload,
  metadata/checksums, GUI/CLI/daemon и автоматическая установка обоих Mini adapters
  в системные каталоги реализованы. Private pacman install→upgrade→remove и
  production GUI launch/IPC/Close прошли; это не host/session acceptance.
  [Контракт и границы поставки](docs/parity/package-delivery.md).

## Границы и нерешённая применимость

- **Startup/offline content (cmd15)** и глобальный UI caller motherboard sync:
  установить применимость; наличие codec или USB ACK не доказывает сценарий.
- **Fan motherboard sync/start-stop** исключены Windows product guards;
  pump source остаётся применимым.
- Отдельный **Screen Animation Editor** предназначен для HydroShift II OLED
  Curve и не входит в LCDV2 parity. Обычный **GA2LCD Editor** входит.
- Legacy FW<1.5 B/H.264 аппаратно не проверен; известная конфигурация — FW1.6/C512.
- Физические проверки отложены пользователем. После них оставлять **LocalUI**.
  Firmware fault injection на единственном AIO не проводить.

Полная parity не достигнута. Количество codecs, тестов или закрытых пунктов
не даёт достоверного процента готовности.

