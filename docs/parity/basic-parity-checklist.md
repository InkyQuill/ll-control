# Parity LCDV2: чеклист реализации и приёмки

Сверено **2026-10-16**, Windows **2.1.28.0**, Galahad II LCDV2 **0416:7395**.
Открытые пункты — оставшаяся работа; отмеченные содержат evidence программных
проверок. Порядок — [roadmap](../../roadmap.md), provenance —
[матрица](lcdv2-2.1.28-matrix.md), выполненное — [сводка](implementation-status.md).
[История checkpoints](basic-parity-checkpoints-2026-10-12.md) сохранена отдельно;
её старые unchecked boxes не переоткрывают выполненное. Каждый новый результат
должен указывать дату, snapshot кода, окружение, evidence и границы проверки.

## Editor и capture — реализация и приёмка

- [ ] Сравнить hue/saturation/brightness/contrast на Windows/Linux одинаковых
  изображениях. Реализация, 140 offline shader-векторов и browser controls
  проверены; это не WPF/GPU qualification. [Checkpoint](evidence/2026-10-12-editor-point-filters.json).
- [ ] Сверить реализованные sharpen/blur с установленным Windows baseline:
  software Auto sampling, rounding/resampling, non-96-DPI и fractional crop; стоимость обработки
  больших изображений с 64 transforms. [Контракт](ga2lcd-editor.md).
- [ ] Native import/gallery edit → preview → save → thumbnail → theme Apply:
  pointer/keyboard crop, undo/reset, errors/cancel/stale result, focus/scaling,
  legacy migration и конкурентные изменения; сохранить чужие references.
- [ ] GA2LCD VideoCut/VideoEdit/AutoFill:
  Brightness/contrast реализованы в UI/source preview/native commands/кодировании;
  [software checkpoint](evidence/2026-10-12-video-effects-ui.json). Сверить Windows
  colorspace/codec и scalar/tail rounding, native canvas/ошибки/ресурсы при больших
  кадрах. VideoEdit single-range UI и native metadata/encoding подключены;
  fresh upload/gallery, fractional range, unknown FPS, cancel/unmount/lost
  inspection reply и explicit Save проверены в browser mock workflow
  ([checkpoint](evidence/2026-10-12-video-range-ui.json)). Guards maximum
  duration−1/FPS и minimum2/FPS реализованы без frame snapping.
  Сверить SDK включительность последнего кадра отдельно. AutoFill Windows/native
  qualification (stretch geometry/UI реализованы). Gallery cut/crop/fit re-edit, source→review
  и revision-checked replacement подключены; native source/review decode и seek,
  Save с заменой membership и сохранением исходника проверены. Native color canvas,
  lost-source/replacement reply и device/unmount race matrix ещё проверить. Per-interval crop, GUI selection
  и local frame preview реализованы; prepared-result remux/ownership,
  Tauri range delivery, review UI и record/pause выбора интервалов подключены.
  Native fresh range+brightness → review → Save/codec уже
  [проверен через loopback HTTP](evidence/2026-10-12-native-video-edit-http.json).
  [Gallery re-edit и seek приняты](evidence/2026-10-12-native-gallery-video-edit.json);
  Record/Pause → review → cancel [проверен native](evidence/2026-10-12-native-record-pause.json);
  Save нарезки, numeric crop и stop по концу исходника
  [проверены native](evidence/2026-10-12-native-record-save.json). Лимит180с,
  pointer/keyboard crop interaction/confirm lost reply ещё проверить. Ordered temporal cut при импорте
  и source preview/scrubbing уже реализованы; VFR/nonzero timestamps/GIF/frame boundaries/long-source
  performance ещё проверить. [Контракт](video-editor.md).
- [ ] Screenshot portal → editor/preview/Save реализован; проверить реальные
  X11/Wayland backends, выбор региона/DPI, parent window и compositor cancellation.
  Native raw-byte IPC, editor/Save и editor/app Close проверены с подменным
  порталом; это не приёмка системного диалога выбора. [Контракт](screen-capture.md).
- [x] ScreenRecord: после отказа подготовки вернуться к сохранённому исходнику,
  повторно подготовить и явно сохранить. Browser failed encoding/admission и
  native injected command errors с фактической отменой review приняты;
  это не закрывает общую fault matrix.
  [Evidence](evidence/2026-10-13-screen-record-review-recovery.json).
- [x] Killable recorder child process: bounded startup/control, Cancel/Shutdown и
  source-close с reap до staging/portal release, crash/protocol failure и новый
  capture после очистки. EN/RU errors и software/native qualification:
  [checkpoint](evidence/2026-10-16-screen-record-process.json).
- [ ] ScreenRecord для X11 и Wayland: session/FD negotiation private-bus-tested;
  recording worker с pause/resume/stop, shortened limit и joined teardown
  проверен на synthetic GStreamer; Tauri owner/commands/shutdown подключены.
  Admission/prepare bridge подключён с pin исходника и явным Save.
  Metadata, HTTP original-source preview и начальный GUI capture → review → Save
  с clear и gallery membership подключены. Диалог лимита с отменой к паузе и
  явным Finish реализован. Post-recording multi-interval cut/crop, перестановка
  и review → edit → reprepare подключены;
  backend capture-region, числовой/визуальный выбор до записи и на паузе с readback реализованы; остаются расширенная native crop/edit/limit/fault matrix, реальные plugin/kernel stalls и PipeWire backends. Повтор последнего кадра и восстановление после responsive delivery gap проверены с synthetic source; явный EOS остаётся ошибкой ([evidence](evidence/2026-10-12-screen-record-held-frame.json)).
  Независимый таймер, Finish и Cancel при прекращении доставки кадров отзывчивым
  synthetic source проверены ([evidence](evidence/2026-10-12-screen-record-stopped-delivery.json)). Первый synthetic native source/review decode/seek/Save/membership и
  window Close принят; полный180s synthetic engine прогон также принят
  ([evidence](evidence/2026-10-12-screen-record-held-frame.json)). Native automatic-limit → Cancel →
  Import/review/Save также принят с synthetic source; реальные portals и расширенная
  limit error/accessibility matrix остаются. [Контракт и остатки](screen-record.md).

## RGB, profiles и desktop — реализация

- [x] Полные ручные 2/4-color палитры fan/pump: GUI/CLI → IPC → owner →
  сохранение → restart и RGB bytes приняты с MockBackend.
  [Evidence](evidence/2026-10-13-rgb-mode-palettes.json).
- [x] Явно загрузить текущую базовую RGB настройку зоны без Apply: все цвета,
  brightness/speed/direction/scope/disabled, missing/error/retry и защита черновика
  от поздних ответов. Native обеих зон и software owner/IPC приняты.
  [Evidence](evidence/2026-10-14-rgb-base-load.json).
- [x] Manual catalog6 fan/16 pump: colors/speed/brightness/direction/defaults,
  отдельные черновики device/zone/mode, reset без Apply и applied mode profiles
  после reload/restart приняты с MockBackend.
  [Evidence](evidence/2026-10-14-rgb-mode-catalog.json).
- [x] Fan↔pump sync, SKU280→16/360→24 и manual LED count8…50: global settings,
  retained manual value, GUI/CLI/IPC/persistence/restart и thermal sync-off/restore
  приняты с MockBackend. Это не подтверждение физического LED coverage.
  [Evidence](evidence/2026-10-14-fan-topology.json).
- [x] Dynamic pump параметры отдельно по mode/source/unit: ranges, low/high
  palettes/speed, fixed brightness/direction, legacy inheritance и восстановление
  трёх bindings после restart приняты с MockBackend/native WebKitGTK.
  [Evidence](evidence/2026-10-14-dynamic-rgb-profiles.json).
- [x] Сохранённый выбор Static/Dynamic каждого pump mode: переключение режима,
  восстановление последнего источника, Disable без потери параметров и restart
  приняты browser/actor/RPC/native WebKitGTK с MockBackend.
  [Evidence](evidence/2026-10-14-dynamic-rgb-selection.json).
- [ ] Расширенная native RGB fault/reconnect/accessibility matrix и совместная
  программная приёмка с media/cooling. [RGB acceptance](rgb-software-acceptance.md).
- [x] Storage-слой Linux profile archive: полный dependency set active/cached
  pages, typed remap, checksums/validation, bounded media transfer, atomic file
  export и profile revision CAS проверены offline. Это ещё не import/export UI.
  [Контракт и границы](profile-archives.md).
- [x] Архив одного named slot через Unix IPC/CLI:64KiB transfers, quota/admission,
  idle expiry, Cancel/Release, durable Finish receipt/restart и readback после
  удаления слота проверены с MockBackend; CLI Ctrl+C проверен отдельным процессом.
  [Контракт](profile-archives.md).
- [x] GUI archive одного named slot: native FileChooser и file transfer, progress,
  Cancel/readback, сохранение черновика, normal Close и restart проверены browser
  и изолированным WebKitGTK/MockBackend с portal fixture.
  [Evidence](evidence/2026-10-14-profile-archive-gui.json).
- [ ] Расширенная archive native fault/reconnect/accessibility matrix, реальные
  X11/Wayland FileChooser и интерактивная отмена длительного packing.
- [x] Полные screen/fan+lighting storage snapshots и atomic restore: target CAS,
  соседняя группа/globals, slots/revisions и journal interruption/restart проверены.
  [Контракт и Windows source scope](profile-groups.md).
- [x] Group archive codec: active/cached/slot/gallery-only и legacy/user uploads,
  typed remap/defaults, clean-store files/restart,256-entry gallery, video600frames,
  no-overwrite/pins и source snapshot через settings owner проверены программно.
  [Evidence](evidence/2026-10-14-profile-groups-codec.json).
- [x] Full-group snapshot/backup export Screen/FanAndLighting через IPC/CLI:
  owner revision, immutable session,64KiB reads, bounded packing/Cancel ownership,
  actual CLI archive>1MiB/clean-store decode и no-overwrite приняты offline.
  [Evidence](evidence/2026-10-15-profile-groups-export.json).
- [x] Full-group upload/Inspect IPC/CLI: bounded chunks и retries, private
  quarantine/full codec validation, group/checksum rejection, unchanged target
  gallery/settings/PWM и owned cancellation проверены программно.
  [Evidence](evidence/2026-10-15-profile-groups-upload.json).
- [x] Full-group Finish/Apply и durable readback IPC/CLI: обе группы через
  actual Unix RPC/DeviceManager, новое manual/LCD intent при repeat/stale,
  restart, shared runtime admission и отдельные storage/runtime outcomes.
  [Evidence](evidence/2026-10-15-profile-groups-finish.json).
- [x] Full-group live publication ownership/rollback: known failed Finish удаляет
  только свои unreferenced image/name/font/media; private legacy catalogs,
  concurrent uploads/scopes, live pins/offline refs/foreign replacement защищены.
  [Evidence](evidence/2026-10-15-profile-groups-rollback.json).
- [x] Fallible ownership barrier перед final publication PNG/name/font/media:
  callback видит synced private inode; отказ не публикует файл; metadata failure
  сохраняет foreign replacement.
  [Evidence](evidence/2026-10-15-import-prepublication.json).
- [x] Durable publication базовых full groups: private staging/lifetime lock,
  hard-link ownership до final name, durable phases и startup reconciliation
  до manager replay. SIGKILL, owner commit, receipt eviction, references/foreign
  files, corrupt journals и actual IPC diagnostics/replay проверены.
  [Evidence](evidence/2026-10-15-import-publication-journal.json).
- [x] Named-slot durable ownership: отдельный target binding, source SHA receipt
  с чтением legacy, SIGKILL/restart/eviction, Status diagnostic, live rollback
  при stale CAS и завершение после caller loss.
  [Evidence](evidence/2026-10-15-slot-publication-journal.json).
- [ ] Extended boundary/fsync/resource/panic/shutdown matrix;
  native crash/recovery diagnostics flows.
  [Протокол и crash checklist](import-publication-recovery.md).
- [x] Full-group explicit reapply IPC/CLI: opaque actor tokens/ABA, checked LCD
  Begin/Apply, current receipt, неизменные settings/profiles, newer intent и
  owned caller-loss/shutdown handling проверены программно.
  [Evidence](evidence/2026-10-15-profile-groups-reapply-transport.json).
- [x] Full-group GUI: native file chooser, export без перезаписи, upload/Inspect
  с предпросмотром и отдельным Apply, progress/Cancel, сохранённый ID/device/group,
  readback/reapply без повторного commit и сохранность черновика.
  Обе группы прошли native WebKitGTK/MockBackend и restart.
  [Evidence](evidence/2026-10-15-profile-groups-gui.json).
- [ ] Full-group расширенная recovery/lifecycle/public streaming workload и
  signal/fault/expiry/shutdown matrix; real file portals и интерактивная отмена
  catalog/staging, а не только writer boundary. В частности: lost Finish reply/
  abrupt desktop death, Cancel во время больших pack/Inspect, close при принятом
  Apply, hanging codec/filesystem, disk-full/fsync/portal failures и expired ID.

- [x] Group owner commit с Arc<PreparedGroup>/pins и durable receipt: lost reply,
  current readback/restart, поздние edits/deleted slots, same-ID mismatch,64-receipt
  bound, gate contention и journal interruption/recovery проверены программно.
  [Evidence](evidence/2026-10-14-profile-groups-commit.json).
- [x] Screen group runtime через LCD owner: prepared source → durable commit →
  joined replace/config/frame, realtime sensor refresh, stale/repeated requests,
  explicit reapply/restart и error/shutdown paths проверены private HID/FFmpeg.
  [Evidence](evidence/2026-10-14-profile-groups-screen-runtime.json).
- [x] Shared barrier cooling+RGB: prior I/O/settings, suppressed refresh, mutation
  rejection, read-only queries, owner cancellation/sealed Drop и shutdown.
  [Evidence](evidence/2026-10-14-profile-groups-barrier.json).
- [x] Cooling/RGB actor preflight/install/Apply: silent staging, sensor/mode
  compatibility, retained global thermal history, inactive modes/Dynamic, no
  settings deltas и retry при ошибке HID. [Evidence](evidence/2026-10-14-profile-groups-install.json).
- [x] FanAndLighting coordinator: both barriers/preflight → durable receipt →
  both install ACKs → Apply; lost client/owner reply, stale/repeated ID, newer
  intent, storage/HID errors и shutdown проверены. [Evidence](evidence/2026-10-15-profile-groups-coordinator.json).
- [x] FanAndLighting explicit reapply: current receipt, paired actor revisions,
  Direct/saved edits/ABA protection, no new settings/receipt/profile revision и
  lost reply проверены. [Evidence](evidence/2026-10-15-profile-groups-reapply.json).
- [ ] FanAndLighting extended partial install/recovery и production lifecycle/
  fault matrix. Базовый import/export/reapply GUI принят, см. выше.
- [x] H.264 final-frame software rotation0/90/180/270 для media/composition start,
  replay и group import/reapply: actual decoded video+overlay pixels, media identity,
  brightness и отсутствие double rotation проверены программно.
  [Контракт и evidence](display-rotation.md).
- [x] Live angle существующего composition: worker/generation/video phase
  сохраняются, stale frames ограничены общим codec budget; private IPC проверен.
  [Evidence](evidence/2026-10-14-display-live-rotation.json).
- [x] Live angle direct managed media0°: joined reader handoff, keyframe warmup
  без rewind, сохранение generation/позиции/ID и rollback при ошибке config,
  shutdown без нового AU. [Evidence](evidence/2026-10-14-display-direct-rotation.json).
- [ ] Расширенная GUI/native control/error/cancel приёмка и physical qualification
  четырёх положений; startup pause, долгие видео, workload и device buffers.
  Windows ZIP/profile interoperability реализовать отдельно; named slot не заменяет группу.
- [x] Legacy unset/explicit manual PWM0 различаются: fan marker/pump source,
  private IPC/manager restart и actual group codec/Apply/reapply проверены.
  [Evidence](evidence/2026-10-15-manual-pwm-zero.json).
- [ ] Physical manual0/source/RPM, возврат к ненулевому duty и lifecycle;
  [manual0 acceptance](manual-pwm-zero.md).
- [x] Полный reset fan+lighting в daemon/Unix IPC/CLI: Windows defaults активных
  и неактивных modes, topology, Fan Standard/Pump Quiet и8 slots; общий atomic
  coordinator, receipt/readback/reapply, stale/repeat/newer intent и Screen/global
  preservation. [Контракт и checklist](fan-lighting-reset.md).
- [x] GUI полного reset fan+lighting: read-only defaults/target preview, явное
  подтверждение/sensor selection, сохранённый operation ID/readback/reapply,
  same-ID retry после lost reply, shared busy и draft preservation.
  Actual Unix proxy/worker проверил lost ACK после commit без повторного Apply.
  [Контракт и acceptance](fan-lighting-reset.md).
- [ ] Расширенная native reset fault/accessibility/partial install/lifecycle
  matrix; реальные hardware/sleep/detach проверки отложены.
- [x] Desktop preferences/XDG registration storage: CAS/flock, durable intent,
  desired/actual readback, owned file update/disable/Hidden override, integration
  failure/retry и реальный GIO argv. [Контракт](desktop-lifecycle.md).
- [x] Tray/autostart Tauri/GUI: native desired/actual/retry settings, own tray
  registration, close/hide/show/joined Exit, host loss/startup fallback,
  single-instance и --autostart. Private watcher/menu, real settings IO lost caller,
  media Close/encoder cleanup и control reset restart regressions приняты.
  [Контракт](desktop-lifecycle.md).
- [ ] Actual login/GUI entry launch/upgrade/AppImage, production DE/Wayland,
  missing indicator/dead session/exit failure и legacy chooser/AT-SPI matrix.
- [x] Общие °C/°F: versioned CAS store, IPC/CLI/GUI, sensor readings,
  температурные поля и System LCD layers без перезапуска/переписывания темы.
  [Контракт и программная приёмка](display-settings.md).
- [x] Язык интерфейса: persisted CAS/CLI/GUI, reactive resources, сохранность
  user labels/drafts, offline language cache. Это основа переключения с partial
  English; [контракт](interface-language.md) не объявляет весь catalog готовым.
- [x] Explicit English static catalog:768/768 resource keys, параметры сохранены,
  strict locale coverage проходит. Это не закрывает dynamic/native language gates.
- [x] English — default новой установки/offline fallback; выбранный русский
  и legacy record без language сохраняются. Новые записи явно хранят язык.
- [x] Локализованные имена новых LCD draft / screenshot / recording и описания
  раскладок; уже введённые имена и LCD text сохраняются при смене языка.
- [ ] Обеспечить основной English и обязательный полноценный русский во всех
  user-facing flows; полнота переводов не закрывается одной сменой default.
- [x] Archive UI phases/group labels/progress accessibility/cooling summaries
  и reactive import notice EN/RU; private native slot/full-group round-trips EN/RU.
- [x] Native archive picker: typed language в Start, EN/RU profile/group
  import/export titles/filter labels и app-owned grant errors; private portal
  arguments проверяются отдельно от настоящего desktop picker.
- [x] Structured native archive notices: optional code отдельно от raw message,
  EN/RU render, сохранность diagnostics/unknown codes и явные pending guards.
  Остальные worker/transport/daemon messages ещё требуют квалификации.
- [ ] Dynamic outputs и native dialogs/menus на EN/RU, grammar/layout/accessibility,
  restart/offline/CAS/lost replies. Native worker errors вне file chooser и
  настоящие desktop dialogs ещё требуют квалификации. Остальные языки отложены до полировки и
  не блокируют базовый этап. [Политика и acceptance](interface-language.md).
- [x] Начальный Mini Plasma adapter: шесть slots, EN/RU, shared source model,
  bounded CLI helper и private staged package acceptance.
- [x] Начальный Mini GNOME adapter: общий renderer, GJS helper, GSettings и
  GTK preferences; private transport/config/persistence checks.
- [ ] Mini System Information: расширенная приёмка Plasma и GNOME Shell,
  шесть слотов и FPS provider. Отдельное окно не реализуем; оконный черновик
  не закрывает этот пункт. Sway и остальные окружения отложены; окончательный
  список поддержки и версии shell пока не определены.
  [Контракт и acceptance](mini-system-info.md), [source inventory](system-info-inventory.md).
- [ ] Общие единицы: расширенные native keyboard/AT-SPI/error/range проверки
  temperature editors, реальные JPEG/H.264/overlay/sleep/reconnect.
  [Приёмка](display-settings.md).
- [x] Spec inventory: Linux provider/IPC/CLI/GUI без подключённой помпы,
  категории/компоненты, refresh/error/stale, private WebKitGTK и joined Close.
  [Evidence](evidence/2026-10-15-system-spec.json).
- [ ] Spec inventory: проверить полноту
  соответствующих полей на Windows/Linux и реальных платформах, доступность
  DMI/DIMM/EDID, hotplug и масштабирование. [Контракт и software checks](system-spec.md).

## Sensors и Windows reference

- [x] Уже загруженный zenpower: SVI2 Core voltage и CPU temperature через shared
  cache/source selection/realtime LCD; exact rail label, stable physical identity,
  unavailable/stale и JPEG recovery проверены без host sysfs/HID.
  [Checkpoint](evidence/2026-10-16-zenpower-sensors.json).
- [x] Opt-in MangoHud logged-frame mean через shared sensor cache/IPC/LCD и
  явный FPS source в Mini Plasma/GNOME. [Контракт](fps-source.md),
  [checkpoint](evidence/2026-10-16-mangohud-fps.json).
- [x] Runtime FPS enable/disable и путь в основном приложении EN/RU и CLI:
  revision/draft/lost-reply/restart и FPS-only invalidation проверены.
  [Checkpoint](evidence/2026-10-16-fps-runtime-settings.json).
- [ ] FPS capture activation/app selection, Windows PresentedAvg/app-identity
  semantics и actual game/shell performance qualification.
- [ ] CPU voltage вне поддержанных asusec/opt-in ASUS WMI/NCT CPUVC/zenpower;
  proprietary NVIDIA voltage вне доступной X-сессии: достоверные providers,
  units/identity, missing/stale/recovery без fake zero или подмены VID/rail.
  [Inventory](sensor-provider-inventory.md).
- [ ] Windows Intel aggregate GPU load: report/mapping и render/compute/video
  workloads одного adapter. Не выдавать сумму/max/mean engines за общий load.
  PMU/query/permissions и неоднозначный VRAM used=0 уже обработаны программно.
  [Контракт](intel-engine-utilization.md).
- [ ] Locale-specific переносы, Windows font fallback/hinting/raster:
  одинаковые строки, fonts/locale и изображения. Shaping/bidi/wrap и bundled
  fallback уже реализованы. [Fonts](font-catalog.md).
- [ ] Установить применимость cmd15 startup/offline и global motherboard-sync
  caller. Отдельный HydroShift Screen Animation Editor и guarded fan MB sync/
  start-stop не входят в LCDV2 scope. Legacy FW<1.5 остаётся неквалифицированным.

## Приёмка уже реализованного

- [ ] Media/composition: slow read, first-frame cancellation, corruption во
  время playback, длительный сложный workload с измерением ресурсов/latency.
  [Composition](video-composition.md), [storage](persistent-media-v2.md).
- [ ] Native publication/delete faults: disconnect, lost reply, close во время
  publication; readback без повторной мутации. Daemon lost-Finish уже проверен.
- [ ] Screen reset: native ambiguous reply, сохранность полного draft/video/fonts,
  re-add после reset, коллекции → import → reset/replay; прямые in-flight
  detach/shutdown. Keyboard/cancel/LCD failure/reapply/AT-SPI, последовательный
  lifecycle и in-flight sleep уже проверены. [Evidence](evidence/2026-10-12-screen-reset-native.json).
- [ ] Расширенная native accessibility: media cards/pagination/rename, collections,
  group reset, editor и error/limit/expanded states; Wayland/fractional scaling,
  реальное озвучивание screen reader. AT-SPI events не доказывают речь.
  [Acceptance](accessibility-acceptance.md), [native](native-keyboard-acceptance.md).

## Аппаратная приёмка — отложена пользователем

- [ ] Перед тестами записать firmware/SKU/identity/git snapshot, обеспечить одного
  HID owner; установить рабочие pump duty/source и RGB restore. Уточнить fan
  wiring: при подключении к motherboard LCDV2 fan tests — unavailable, не pass.
- [ ] LCD: edited image/EXIF1…8, fit/alpha/shadow/watermark/gradients, текст/clock/
  sensor themes и видео с overlays соответствуют preview; stale/recovery,
  °C/°F, полночь/полдень, работа без GUI; brightness и четыре rotation видимы.
  JPEG>70s и diagnostic H.264 уже наблюдались; требуется приёмка нового пути.
- [ ] Pump presets/curves/source/RPM, fallback/manual override/restart; fan только
  при подтверждённом подключении. [Cooling protocol](lcdv2-m2-cooling-acceptance.md).
- [ ] Все доступные RGB zones/modes/sync/LED counts, thermal priority/grace/base
  restore; известный direct prefix не считается полной зоной. Для dynamic RGB
  проверить CPU/GPU load/temperature и доступный pump RPM/coolant: low/high/equal
  thresholds, palettes/speed, fixed brightness/direction, смена режима/источника,
  stale → manual fallback → recovery и Disable после restart. Generic host fan
  RPM fixture не подтверждает физический pump RPM.
- [ ] 30 минут realtime media + sensors + cooling + RGB: FPS/latency/sensor age,
  bounded CPU/RSS/queues, отсутствие периодических пауз и артефактов.
- [ ] Stop/replace/clear, restart, USB reconnect, cold boot и sleep/wake:
  stable identity/replay, свежие данные, нет старых actors/frames/второй HID session;
  безопасный transport recovery без flashing/fault injection единственного AIO.
  [M1 protocol](lcdv2-hardware-acceptance.md).
- [ ] На доступных платформах подтвердить accuracy/availability voltage и Intel
  PMU/VRAM: ASUS EC/WMI, nouveau, NVIDIA X/Wayland/headless и multiple GPUs.
  Не менять BIOS/драйверы автоматически. [NVIDIA](nvidia-voltage-abi.md),
  [Intel memory](intel-memory-query.md), [providers](sensor-provider-inventory.md).
- [ ] После тестов оставить LocalUI, восстановить cooling/RGB и завершить тестовый daemon.

## Firmware и итоговая поставка

- [x] Firmware identity software: один actor-owned cmd134, классификация PN0/PN1,
  optional build_datetime/CLI unavailable и совместимость прежнего JSON.
- [ ] Повторить version/date/late PN1/latency на железе; software readback fix
  не заменяет hardware acceptance. [Контракт](firmware-lcdv2.md).
- [x] GUI firmware identity/preflight EN/RU с явным чтением и защитой при смене
  устройства/соединения, без дополнительного periodic firmware polling.
  [Browser/private native evidence](evidence/2026-10-15-firmware-gui-preflight.json);
  физический readback/latency и полная native accessibility ещё не подтверждены.
- [x] Найден официальный firmware candidate1.6: size/MD5 сверены, SHA-256
  закреплён; offline inspector узнаёт точные bytes и отклоняет unknown images.
  [Image research](firmware-lcdv2.md); target/recovery qualification ещё открыта.
- [x] FW1.6 source-derived ACK/error decoder с проверкой actual extent и
  request prefix, raw activation extra flag и явными ambiguity limits.
  [Codec checkpoint](evidence/2026-10-15-firmware-ack-codec.json); wire/recovery и update owner ещё открыты.
- [x] Internal actor-owned transfer: pinned image/identity preflight, permanent
  attachment fence, no retry, typed partial progress и joined cancellation/shutdown.
  [Transport checkpoint](evidence/2026-10-15-firmware-owner-transport.json); keeper
  coordinator, durable outcome и reconnect/replay gate ещё не реализованы.
- [x] Отдельный keeper stop/join на том же HID handle, включая sensor poll и
  settings dispatcher; cancellation-safe waiters и обычный root teardown.
  [Quiescence checkpoint](evidence/2026-10-15-keeper-quiescence.json).
- [x] Active/queued LCD worker cleanup errors доходят до Failed; checked quiesce
  отвергает pending/failed settings persistence.
  [Health checkpoint](evidence/2026-10-15-quiesce-health.json); cooling hold,
  global IPC admission и coordinator ещё нужны до flash.
- [x] Requested cooling hold на том же handle перед chunks: MCU pump100%,
  optional owned controller fan100%; partial/cancel → Unknown без flash/restore.
  Saved cooling intent не перезаписывается.
  [Hold checkpoint](evidence/2026-10-15-firmware-cooling-hold.json); effective RPM/
  reset behavior и verified restoration пока hardware gates.
- [x] Atomic durable firmware journal storage и pre-open ordinary replay guard
  при unresolved/corrupt state; без automatic retry/unlock.
  [Journal checkpoint](evidence/2026-10-15-firmware-operation-journal.json).
  Execution binding теперь подключён; verified resolution ещё предстоит.
- [x] Mandatory scoped journal token для transfer, target/hold/current bytes
  check до query, durable marker до PWM и final record до reply. Commit failure
  остаётся Unknown, unscoped update отвергается.
  [Binding checkpoint](evidence/2026-10-15-firmware-journal-binding.json).

- [x] Firmware settings anchor до side effects: сохранение disk intent,
  отказ ordinary writes/profile transactions, missing-journal/changed-serial
  pre-open quarantine и previous reader rejection проверены private-file/MockBackend.
  [Checkpoint](evidence/2026-10-15-firmware-settings-anchor.json).
- [x] Internal probe-only reconnect evidence: actual enumeration absence → unique
  same-serial device, one cmd134, exact identity/optional date, recheck guard/journal
  и candidate; без keeper replay/unlock. [Checkpoint](evidence/2026-10-15-firmware-reconnect-probe.json).
- [x] Shared process-local admission для IPC/main worker pools, manager discovery/
  power; caller cancellation не освобождает enrolled worker lease, close/drain
  не открывается при drop waiter. [Checkpoint](evidence/2026-10-15-firmware-admission.json).
- [x] Lifetime leases для Publication и file-backed entries, owned Drop cleanup,
  checked failure health, scope-bound maintenance и explicit staging invalidation;
  finite exports освобождаются без polling receiver. [Checkpoint](evidence/2026-10-15-firmware-cleanup-owners.json).
- [ ] Unresolved publication/import recovery и расширенная supervisor/shutdown/
  fault matrix после соединения maintenance/staging/keeper join/drain.
  Gate active0 всё ещё не является общей quiescence всех daemon jobs.
- [x] Serialized manager handoff ждёт текущий attach/replay, забирает target/peers
  без reopen и retains original HID после checked keeper join; requester/close
  cancellation и daemon shutdown используют teardown receipts. [Checkpoint](evidence/2026-10-16-firmware-manager-handoff.json).
- [x] Internal owned coordinator соединяет preparation/snapshot/assets с original
  transfer и joined close; snapshot freshness проверяется до hold; observer loss
  не отменяет job. [Checkpoint](evidence/2026-10-16-firmware-owned-coordinator.json).
- [x] Process-local job registry: reattach status/progress/cancel/completion,
  explicit shutdown cancel/join и worker-panic Unknown с tracked cleanup/root join;
  без дополнительного drain count во время normal worker.
  [Checkpoint](evidence/2026-10-16-firmware-job-supervision.json).
- [x] Saved restart inspection journal/settings guards в daemon/strict Unix IPC/CLI:
  no-device/sealed admission, все persisted stages, orphan/mismatch/corruption,
  no implicit transaction redo или HID. [Checkpoint](evidence/2026-10-16-firmware-recovery-inspection.json).
- [x] Settings EN/RU: явное чтение saved firmware recovery без DeviceId и при
  sealed admission; state/stage/guards/markers/raw diagnostics, invalid/failure
  clearing и late reply guards. Это snapshot без clear/retry/replay/success.
  [Checkpoint](evidence/2026-10-16-firmware-recovery-gui.json).
- [x] Owned internal reconnect job: one-shot owner только после checked original
  root close, binding scope/operation/target, observed detach → fresh cmd134,
  retained completion и explicit cancel/join blocking probe при daemon shutdown.
  FreshIdentityObserved сохраняет quarantine. [Checkpoint](evidence/2026-10-16-firmware-owned-reconnect.json).
- [ ] Product coordinator: дальнейший durable restart job/status/recovery, extended panic/power/
  shutdown matrix, bounded cancellation/deadline/restart policy, product probe UI/qualified resolution, serial-less recovery
  и пропущенный fast reset; helper сам этого не обеспечивает.
- [ ] Qualified firmware guard/journal resolution после доказанного no-effects
  или свежего target/version verification; без автоматического restore/reflash.

- [ ] Safe LCDV2 updater: target/image validation, exclusive ownership, progress,
  reconnect/version verification, исследованный recovery; ENE updater недостаточен.
- [x] Production Arch payload с GUI/CLI/daemon, systemd user unit, LCDV2 udev,
  desktop metadata, license/font notices, hashes/source manifest. Mini автоматически
  входит в system-wide Plasma/QML и GNOME stores. Повторный strip отключён;
  native-smoke/dev GUI отвергаются. [Контракт](package-delivery.md).
- [x] Private pacman install0.1.0-1 → upgrade0.1.0-2 → remove: file hashes/modes,
  оба Mini stores, host ABI/CLI version, user-data retention; deterministic repack.
  Actual packaged production GUI: English embedded frontend/private IPC/Close,
  missing recorder channel rejected before GTK. [Evidence](evidence/2026-10-16-package-delivery.json).
- [ ] GitHub CI execution, clean-host binary reproducibility и real-session
  install/autostart/upgrade/uninstall; shell enable/panel/login/version matrix.
- [ ] На итоговом snapshot: fmt, strict Clippy, tests, docs, release build,
  frontend check/build/contracts/browser и isolated native flows; затем общая
  hardware матрица. Старые успешные checkpoints не доказывают зелёное текущее дерево.

## Полировка: дополнительные языки интерфейса

- [ ] После базового English/Russian добавить остальные выбранные локали;
  Windows de-DE/es-ES/fr-FR/ko-KR/zh-CN/zh-TW остаются reference inventory.
  Проверить English fallback, native strings, grammar, fonts/layout,
  accessibility и сохранение/переключение каждой поддерживаемой локали.
  Не выдавать отсутствующий перевод за готовую поддержку.

## Дополнительные LCD-эффекты и Style Express — последний этап

Перенос оставшихся16 effects/Style Express отложен пользователем до завершения
основного функционала. Fireworks schema/runtime/GUI/profile/restart принят
с MockBackend; visual/native fault/hardware gates остаются открытыми
([evidence](evidence/2026-10-13-fireworks-integration.json)).

Этот раздел не блокирует завершение базового функционала LCDV2. Для полного
parity каталог и его приёмка остаются обязательными; к ним возвращаемся после
основных сценариев, общих настроек приложения и поставки. Обычные RGB-режимы
и сенсорные LCD-темы сохраняют основной приоритет.

- [ ] Довести каталог до всех 23 применимых страниц, включая Style Express: собственные assets,
  controls/composition и проверяемый результат каждого эффекта. Rain/Snow schema/renderer/H.264/GUI и persistence подключены; per-source ranges, независимые effect parameters/GUI bounds, semantic CPU Load/equal thresholds, полный Basic/Rain/Snow page-state и native image/slot/restart приняты,
  native faults и Windows visual comparison ещё открыты. MeteorShower и Snow3D model/renderer/theme/daemon/GUI и slots/restart подключены;
  native happy path принят, video/fault/visual qualification открыта. [Каталог effects](lcd-effects.md).
