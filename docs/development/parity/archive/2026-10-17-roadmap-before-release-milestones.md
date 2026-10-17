# Roadmap: parity с L-Connect 3 для Galahad II LCDV2

Сверено **2026-10-17** в `lconnect-linux`, ветка `master`. Baseline: **Windows L-Connect 3
2.1.28.0**, **Galahad II LCDV2 0416:7395**. Цель — все применимые сценарии
этого SKU. Другие устройства отложены до появления тестового оборудования.

Здесь только порядок оставшейся работы. Выполненные этапы и checkpoints —
в [сводке реализации](../implementation-status.md), Windows provenance —
в [матрице](../lcdv2-2.1.28-matrix.md), критерии проверки —
в [чеклисте](../basic-parity-checklist.md).
[Индекс активных планов](../../superpowers/plans/README.md) содержит три
незавершённых плана; новые задачи M4–M6 ниже не дублируются отдельными планами.

**Геометрия LCD:** GALAHAD II LCDV2 — квадратный экран 480×480. Форма и
разрешение задаются характеристиками модели; превью и схема устройства используют
одни данные. Круглая маска применяется только к устройствам с круглым LCD.
Поддержка формы не означает готовую поддержку других моделей на железе.

**Следующий пробел:** intrinsic font/value-dependent Three/Four Sensor layout
в оригинале ещё не воспроизведён полностью. Editable dividers уже реализованы.
Realtime video+Four Sensor после оптимизации текстовых теней прошёл minimum20fps:
722 новых кадра/30.85с,23.40fps при nominal24; весь поток723 кадров декодирован.
[Изменение, тесты и границы](../evidence/2026-10-17-realtime-shadows.json).

Текущий программный кандидат: `target/distribution-20261017-realtime`.
845 frozen inputs,33 manifest payload files, обе Mini integrations в общих stores;
production `custom-protocol` GUI без `native-smoke`. Включает cooling banks,
Windows Control/Screen import, independent current/editor, private preview,
source recovery, editable dividers и оптимизированные text shadows.

Текущие Rust checks:1373 passed/9ignored +4 example tests, fmt, strict Clippy/rustdoc.
Неизменённый frontend имеет758 browser checks, Svelte0/0, static1144/1144,
39 theme contracts и3 localization contracts. Private package install/upgrade/remove,
packaged Mini Qt/GJS/Plasma host и production GUI0/1mock sensor/pump/Close/restart/replay
на новых binaries приняты. EN/RU native divider и Windows PNG/MP4/GIF/error matrix
имеют отдельные scoped checkpoints.50 dynamic outputs остаются inventory;
original Windows intrinsic layout/reference, expanded faults, real desktop/hardware
и GitHub/clean-host gates открыты. Source845 snapshot совпадает с manifest;
между Divider и Realtime candidates изменён только image theme renderer.

Предыдущий [collections candidate](../evidence/2026-10-17-collections-integrated-candidate.json)
и [история срезов](../implementation-checkpoints-2026-10-17.md) сохранены
как evidence; текущий объём определяется новым manifest.

Дополнительная [native image readback matrix](../evidence/2026-10-17-native-collection-readback.json)
закрывает7 post-write fault modes в EN/RU:14 реальных daemon commits без replay,
с сохранением каталога/черновика. Изменён только verification harness; production
код и артефакты кандидата сохранены. Plain/video/replacement и reconnect timing
остаются в расширенной native приёмке.

[Сверка базовых цепочек](../lcdv2-baseline-gaps.md) отделяет реализованные
sensors/control/images/realtime/themes от Windows/platform и физических gates.
Дополнительно [Mini из пакета](../evidence/2026-10-17-packaged-mini.json)
прошёл Qt/model/GJS/preferences/production CLI и private Plasma host. Default
проверка подключена к GitHub workflow source; remote CI и real sessions не
квалифицированы. Production payload не менялся; тестовые inputs текущего дерева
отличаются от frozen snapshot кандидата.

После кандидата также исправлены Windows endpoints fan presets: семь
явных точек вместо пяти, с сохранением custom/saved curve intent.
[Контракт и проверка](../lcdv2-m2-cooling-acceptance.md#fan-preset-endpoints-2026-10-17).
В общий package fix пока не включён.

## Ближайший порядок

1. **Закрыть оставшиеся сценарии ScreenRecord.** Start/pause/resume/finish,
   clear, source preview, multi-interval trim/crop, review → возврат к редактированию
   → повторная подготовка → явное Save и gallery membership уже реализованы.
   Начальный Tauri/WebKit workflow проверен с synthetic source; диалог лимита
   проверен в browser, engine — с сокращённым лимитом. Выбор области до первого кадра и на паузе,
   числовые поля, visual preview и readback реализованы. Killable recorder process и startup/control deadlines, Cancel/Shutdown с reap до
   portal/staging release подключены. Остаются реальный plugin/source recovery и
   real-capture resource profiling, оставшаяся native crop/edit/limit/fault matrix
   и реальные X11/Wayland PipeWire backends. Повтор последнего кадра при
   responsive delivery gap и возврат к новым кадрам проверены synthetic-тестами.
   Полный 180-секундный synthetic engine и native automatic-limit → Cancel →
   Import/review/Save приняты. Native SIGSTOP/SIGKILL, Clear и joined Close
   остановленного child проверены в EN/RU (8 сценариев), с reap, очисткой staging
   и новым capture → decode → явным Save. Synthetic release resource baseline
   принят; далее нужны real-capture/worst-case и long large-source измерения.
   Software source resize с normalized crop, fixed canvas и current full-source
   snapshot реализован и codec-tested; реальный portal resize ещё проверить.
   Private PipeWire transport с real plugin/inherited FD/serial property и
   server-loss failure/reap квалифицирован; fixture вручную соединяет synthetic
   nodes, поэтому real portal permissions/automatic routing остаются открыты.
   [Evidence](../evidence/2026-10-16-screen-record-pipewire.json).
   Failed Close/Clear сохраняет cleanup fence до явного успешного retry:
   EN/RU browser, paused native transport rejection и prepared-review
   media-cancel rejection/committed-cancel lost reply → Clear/new capture/Save
   проверены. [Fence](../evidence/2026-10-16-screen-record-cleanup-fence.json),
   [native review/lost reply](../evidence/2026-10-16-screen-record-review-cleanup.json).
   Исправление включено в текущий production package выше.
   Остальная error/accessibility matrix остаётся.
   [Контракт и критерии](../screen-record.md).
2. **Завершить приёмку editor/capture.** Для VideoEdit — Windows FPS/last-frame/
   VFR/colorspace semantics, native180с, pointer/keyboard crop, большие файлы и
   fault matrix. Для image — Windows filters/raster и оставшиеся native flows.
   Saved-image native gallery happy path EN/RU с63-action crop/filters,
   exact PNG Save, сохранностью source/active reference, explicit Apply и полным
   mock JPEG принят. [Evidence](../evidence/2026-10-16-native-gallery-image-editor.json).
   Native chooser/pointer/keyboard/errors/concurrent/profile matrix остаётся.
   Для Screenshot — реальные X11/Wayland portal, region/DPI и parent window.
   Failed Cancel/Close теперь сохраняет original portal owner/connection/path
   и ID в UI, блокирует новый capture до checked cleanup; explicit retry,
   late-reply guards и opaque cleanup recovery подключены. Native rejection/
   retry EN/RU и обычный pixel/Save/Close workflow приняты.
   [Checkpoint](../evidence/2026-10-16-screenshot-cleanup-recovery.json).
   Verified original-owner loss теперь завершает logical request отдельным
   исходом, без старого Close новому сервису; native recovery и fresh capture
   приняты. Потеря исходной bus остаётся fenced; реальные dialogs/backend recovery
   и расширенная lifecycle matrix ещё открыты.
   [Owner-loss evidence](../evidence/2026-10-16-screenshot-owner-loss.json).
3. **Завершить расширенную приёмку RGB.** Manual/dynamic controls, параметры
   по mode/source/unit и сохранённый выбор Static/Dynamic с восстановлением
   реализованы. Остаются native faults/reconnect/accessibility и совместная
   media/cooling/RGB приёмка.
   Физические sync/LED topology входят в отложенный hardware checklist.
4. **Закрыть profiles/desktop, firmware и поставку.**
   [Named-slot archive](../profile-archives.md) и
   [полные группы Screen / FanAndLighting](../profile-groups.md)
   подключены к storage/actors, IPC/CLI и GUI. Полная группа переносится через
   native file chooser: export → новый файл; upload → Inspect → явный Apply;
   receipt readback и explicit reapply работают после Release/restart.
   Native WebKitGTK/MockBackend round-trip принят для обеих групп.
   [Durable ownership и startup recovery импорта](../import-publication-recovery.md)
   подключены: private staging/hard links, Preparing/Armed/Keep/Discard, receipt
   reconciliation и консервативный diagnostic при неизвестном исходе. SIGKILL,
   receipt eviction и manager/IPC replay проверены программно.
   Named slots используют отдельный global target и source-bound receipt;
   legacy receipts читаются. Следующие задачи: расширенная
   boundary/fsync/resource/native fault matrix; Windows ZIP interoperability
   (bounded local inspection2.1.28 и offline [cooling preparation](../profile-groups.md#windows-cooling-policy-preparation--2026-10-17)
   уже добавлены; [bank storage](../profile-groups.md#хранилище-cooling-mode-banks--2026-10-17) подключён к settings/archive/journal.
   Discovery/load-to-draft/explicit Apply [проверены](../profile-groups.md#device-modes-read-only-rpc-и-редактор--2026-10-17).
   Storage-only [Save/CAS](../profile-groups.md#device-modes-сохранение-снимка-и-cas--2026-10-17)
   дополняет чтение и Load; active policy меняется только через Apply.
   [Persisted selection](../profile-groups.md#device-modes-persisted-selection--2026-10-17)
   отделяет stored-mode Apply от custom policy и сохраняет requested snapshot/ID.
   [Standard initializer](../cooling-modes.md#standard-bank-initialization)
   создаёт8/3 только по явному запросу и без Apply.
   [Factory restore](../cooling-modes.md#factory-mode-restore) добавляет
   восстановление одного режима/имени в черновик и full Reset8/3/default IDs.
   [Lighting inspection](../profile-groups.md#windows-lighting-bank-inspection--2026-10-17)
   декодирует полные6/16 banks и High/Low всех источников.
   [Linux lighting preparation](../profile-groups.md#windows-lighting-linux-preparation--2026-10-17)
   строит typed configs с explicit role/unit bindings без Apply.
   [FanAndLighting import](../windows-profiles.md#fanandlighting-import-текущий-путь)
   уже соединяет полный normalized draft с target, сохраняет Linux slots/provenance
   и использует existing atomic coordinator/readback. Desktop chooser/explicit bindings/
   preview/Apply/recovery проверены в native EN/RU с MockBackend. Далее —
   неподдержанные curves/parameters, Screen/asset conversion).
   [Screen inventory](../windows-profiles.md#screen-source-banks-and-payload-inventory--2026-10-17)
   уже различает runtime/editor banks и хэширует все uploaded payloads; semantic
   conversion и target commit ещё открыты.
   [Basic runtime proposal](../windows-profiles.md#basic-runtime--native-proposal)
   добавляет LcdTheme с explicit role/font/resource bindings и actual renderer
   cache tests; basic editor preparation сохраняет все overlays/inactive gradients.
   Stored WPF Color и runtime RGB декодируются раздельно с source history.
   Verified PNG/JPEG preparation удерживает normalized bytes без публикации.
   Verified GIF/MP4 preparation использует shared media engine с owned staging/
   cancellation и batch budgets. Owned resource plan уже связывает exact source
   paths/URI с provisional image/video keys и проверяет galleries/runtime/все
   basic banks целиком. Native basic bank storage уже поддерживает independent
   overlays, selection, asset refs и portable remap/JSON/TOML. Selector UI для
   existing banks подключён к draft/Save/Apply EN/RU и native restart.
   Source correction: Windows index0 — current Full Screen; positions1..7 не
   сопоставляются Linux named slots и остаются opaque. Готово
   independent runtime/editor storage с bounded history/typed aliases,
   archive remap/cleanup/actor boundaries. Native publication adapter теперь
   переносит owned PNG/H.264, сохраняет target named library и независимые
   runtime/editor; scoped rollback и native storage restart проверены.
   [Checkpoint](../evidence/2026-10-17-windows-screen-publication.json).
   Source upload/IPC теперь использует private Inspect и explicit Finish через
   existing Screen owner с target revision/receipt/rollback. Socket preview без
   catalog writes, commit/retry/restart и cancel/stale-target проверены.
   Read/load imported editor в черновик подключён отдельным read-only getter
   и кнопкой EN/RU; scoped native PNG acceptance этого действия пройдена.
   Native Screen source job теперь использует private file snapshot и existing
   bridge/upload/Finish/recovery; job-level cancel/shutdown и source replacement
   проверены. Read-only requirements query теперь перечисляет роли, шрифты и
   exact source references через shared validators без назначений/Apply.
   Frontend bindings/preparation/Apply/recovery теперь подключены; EN/RU native
   PNG/private chooser/IPC/mock, imported-editor draft/sensor preview, square
   geometry, normal Close/restart приняты в scoped checkpoint.
   Graphical import preview до Apply теперь показывает private current scene
   или editor с live sensor cache и LCD rotation без publication/LCD writes;
   browser/IPC/native PNG EN/RU, normal Close/restart проверены.
   [Checkpoint](../evidence/2026-10-17-windows-screen-private-preview.json).
   Native GUI owned MP4/GIF + realtime CPU overlay также принят EN/RU: private
   previews/Apply/restart без source, decode фактически отправленного H.264 и
   short release throughput с ≥20fps gate при target24; corrupted source rejected
   EN/RU без изменения существующего LCD кадра/library/catalogs.
   [Checkpoint](../evidence/2026-10-17-windows-screen-media-matrix.json).
   Остаются другие GUI source variations и font/resource/fault matrix; Windows intrinsic grid layout/reference qualification и
   расширенная fault matrix ещё открыты;
   cancellation/fault/expiry, real-portal и lifecycle проверки.
   Live group rollback с private catalogs/refs, [manual0](../manual-pwm-zero.md)
   и [H.264 rotation](../display-rotation.md) реализованы; их расширенная
   приёмка остаётся в соответствующих контрактах.
   [Fan+lighting reset](../fan-lighting-reset.md) подключён к atomic
   coordinator/Unix IPC/CLI/GUI с Windows defaults и сохранением Screen/globals;
   read-only preview, confirmation, receipt recovery и явный retry/reapply
   подключены. Остаётся расширенная fault/lifecycle acceptance.
   [Tray/autostart](../desktop-lifecycle.md) подключены к Tauri/GUI:
   XDG desired/actual/retry, registered tray host, Hide/Show/joined Exit и
   single-instance приняты с private watcher/MockBackend. Реальные desktop sessions,
   login/upgrade/AppImage и расширенная failure matrix ещё открыты. Далее
   [Spec inventory](../system-spec.md) подключён к Linux provider/IPC/CLI/GUI;
   Windows/platform полноту полей ещё проверить. [Общие единицы °C/°F](../display-settings.md)
   реализованы для readings, temperature editors и System LCD layers. Далее язык
   интерфейса: [переключение RU/EN](../interface-language.md) подключено;
   explicit English static catalog заполнен (текущий coverage — в последнем checkpoint). **English — основной язык
   и default новой установки; русский обязателен.** English default подключён;
   legacy records и выбранный русский сохраняются. Новые имена LCD draft, screenshot
   и recording создаются на текущем языке без перезаписи введённого текста.
   ScreenRecord return/close errors используют parameterized EN/RU resources и
   меняют язык без повторного cleanup; неизвестная диагностика сохраняется.
   [Checkpoint](../evidence/2026-10-16-screen-record-messages.json).
   Media import upload/prepare/confirm errors и crop labels локализованы;
   Cancel retry восстанавливается после same-job readback с сохранением guards.
   [Checkpoint](../evidence/2026-10-16-media-import-recovery.json).
   LCD Apply/load/LocalUI, preview/connection/persistence errors сохраняют raw
   diagnostic отдельно от reactive EN/RU instructions; native rejection/retry
   и reconnect browser guards приняты. [Checkpoint](../evidence/2026-10-16-lcd-composition-messages.json).
   Фазы, сводки, native archive chooser и structured archive notices EN/RU
   подключены. Прочие worker errors
   и оставшиеся dynamic/native flows ещё требуют перевода и проверки.
   ImageEditor source/preview/save, plain image
   import, gallery replacement и media selection теперь сохраняют raw diagnostic
   отдельно от EN/RU instructions; acknowledged image без подтверждённой collection
   membership имеет отдельный исход. Late import/gallery replies после reconnect
   не перезаписывают сообщения нового сеанса. [Checkpoint](../evidence/2026-10-16-image-publication-messages.json).
   Image/video list/delete и image rename теперь сохраняют raw diagnostics и
   reactive EN/RU instructions. Rename сохраняет введённое имя, но требует нового
   выбора свежей карточки перед повторным Save; failed readback guard не снимает.
   Native post-commit reply fault → fresh revision принят в обоих языках.
   [Checkpoint](../evidence/2026-10-16-library-management-messages.json).
   Далее прочие worker/helper messages, native list/delete и настоящий
   disconnect/timeout, расширенная video replacement/screenshot cancellation
   fault matrix. Остальные языки отложены до полировки.
   Следующий
   [Mini6](../mini-system-info.md) реализуем как виджет KDE Plasma и
   расширение GNOME Shell, без отдельного окна приложения. Оконный путь удалён;
   cache-only CLI feed и начальные Plasma/GNOME adapters подключены. GNOME panel/session
   acceptance и расширенная Plasma config/session acceptance ещё нужны. Sway и остальные окружения отложены; полный список
   поддержки и версии shell пока не определены. Opt-in MangoHud logged-frame mean и explicit Mini source подключены;
   runtime enable/disable/путь доступны в основном приложении EN/RU и CLI. Capture
   activation/app selection и Windows/game qualification остаются.
   [Контракт](../fps-source.md).
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
   [Каталог и критерии effects](../lcd-effects.md).

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
[чеклисте](../basic-parity-checklist.md).
Record/Pause нарезки существующего видео не равен записи рабочего экрана.

История успешных и неуспешных прогонов — в
[сводке реализации](../implementation-status.md). Она не заменяет
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

[План M1](../../superpowers/plans/2026-09-08-m1-lcdv2-dynamic-display.md):
USB detach/reattach, cold boot и sleep/wake; видимые stop/replace/LocalUI,
brightness и четыре rotation; recovery после transport failure.
Программные barriers, logind/inhibitor, reader teardown и replay уже реализованы.
Повторить совместную приёмку после завершения media/composition.

### M2 — sensor coverage и охлаждение

[План M2](../../superpowers/plans/2026-09-09-m2-sensors-and-cooling.md):
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

[Базовый план](../../superpowers/plans/basic-parity.md): locale-specific переносы,
Windows font fallback/hinting/raster; реальное озвучивание screen reader,
оставшиеся dialogs/expanded states, Wayland/fractional scaling. Сравнивать
одинаковые входы, шрифты и locale. AT-SPI events не доказывают озвучивание.
Эти reference/platform gates не блокируют независимую разработку M4.

### M4 — media/editor/effects

Расширенная приёмка ScreenRecord и приёмка media/editor/capture.
Перенос оставшихся effects/Style Express отложен до завершения основного
функционала; принадлежность к M4 не задаёт им прежний приоритет. [Контракт v2](../persistent-media-v2.md)
и [Windows defaults](../gallery-defaults.md) уже исследованы.
Дополнительно проверить slow read/first-frame cancellation, corruption во время
playback и native publication disconnect/reply loss/close. Daemon lost-Finish
receipt уже проверен; native UI outcome требует отдельной приёмки.

### M5 — RGB, profiles и desktop

Profiles и desktop из таблицы; расширенная приёмка RGB. Manual catalog/defaults6/16,
отдельные applied mode profiles, dynamic mode/source/unit parameters и
Static/Dynamic choice/restore реализованы. [RGB contracts](../rgb-software-acceptance.md).
Именованные fan curves и screen slots, rename/select/slot reset уже реализованы.
Атомарный reset всей группы слотов и полный device Screen reset
с настройками/коллекциями реализованы. Journal и settings-owner barrier для общего
recovery, LCD actor commit/Apply и коллекции реализованы. Внешний reset IPC/readback
подключён с защитой от устаревшей подготовки; explicit transport reapply и GUI подключены; native/lifecycle приёмка остаётся.
[System Information inventory](../system-info-inventory.md) отличает
существующие выбор источников и [Spec](../system-spec.md) от ещё
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
[Контракт, checkpoints и оставшиеся зависимости](../firmware-lcdv2.md).
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
  Свежий [software/package checkpoint](../evidence/2026-10-16-integrated-production-candidate.json)
  включает preview/explicit Apply/LocalUI с одним mock LCDV2 через production AT-SPI;
  реальные USB/desktop/portal проверки остаются открытыми.
  [Контракт и границы поставки](../package-delivery.md).

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
