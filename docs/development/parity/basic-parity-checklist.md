# Parity LCDV2: чеклист реализации и приёмки

> **Scheduling update 2026-10-17:** this is a detailed acceptance/evidence inventory, not a release-blocker list. [R0–R10](release-milestones.md) assign priorities and ownership. The audited retained candidate is **distribution-20261017-realtime**, not the older Collections checkpoint below. New required gaps: direct canvas editing (R1), Windows-compatible export (R5).

Сверено **2026-10-17**, Windows **2.1.28.0**, Galahad II LCDV2 **0416:7395**.
Открытые пункты — оставшаяся работа; отмеченные содержат evidence программных
проверок. Порядок — [roadmap](../../../roadmap.md), provenance —
[матрица](lcdv2-2.1.28-matrix.md), выполненное — [сводка](implementation-status.md).
[История checkpoints](basic-parity-checkpoints-2026-10-12.md) сохранена отдельно;
её старые unchecked boxes не переоткрывают выполненное. Каждый новый результат
должен указывать дату, snapshot кода, окружение, evidence и границы проверки.

Базовые цепочки sensors/control/images/realtime/themes сверены по текущему коду
и общему пакету: [сводная таблица](lcdv2-baseline-gaps.md),
[845-source Realtime candidate](evidence/2026-10-17-realtime-shadows.json).
Он включает Windows Control/Screen import/private preview/current-editor и
editable divider workflow;1373 Rust/9ignored +4 example tests,758 browser,
private package/GUI/Mini и native EN/RU divider JPEG/restart checks пройдены.
**Видео с четырьмя датчиками прошло minimum20fps после оптимизации:**23.40fps
при nominal24; весь поток723 кадров декодирован. Побайтовая regression проверяет
теневые пиксели; новые production binaries повторили packaged GUI/Mini checks.
Это не Windows raster, physical/long-run или real-session qualification.

## Editor и capture — реализация и приёмка

- [x] Геометрия preview и схемы устройства задаётся моделью: LCDV2 — квадрат
  480×480; круглая маска только для круглого LCD. Browser corner-hit проверки
  обеих форм, disconnected draft continuity и compact reflow приняты.
  [Evidence](evidence/2026-10-16-lcd-geometry.json).

- [ ] Сравнить hue/saturation/brightness/contrast на Windows/Linux одинаковых
  изображениях. Реализация, 140 offline shader-векторов и browser controls
  проверены; это не WPF/GPU qualification. [Checkpoint](evidence/2026-10-12-editor-point-filters.json).
- [ ] Сверить реализованные sharpen/blur с установленным Windows baseline:
  software Auto sampling, rounding/resampling, non-96-DPI и fractional crop.
  Large-source64-action release samples теперь приняты ниже; остальные
  formats/content/worst-case resources ещё проверить. [Контракт](ga2lcd-editor.md).
- [x] Ordered image geometry composes up to64 actions without64 full-frame copies:
  21,852 pixel/order comparisons, six byte-identical public-pipeline before/after
  outputs,4K и8192×2048 release latency/RSS samples;1235Rust/9ignored и strict checks.
  Это не Windows raster или полная worst-case resource квалификация.
  [Evidence](evidence/2026-10-16-editor-transform-resources.json).
- [x] Native saved-image gallery edit EN/RU: crop/64 actions/Undo/all filters →
  full-pixel preview → exact PNG Save/membership/thumbnail → explicit theme Apply
  с полным mock-HID JPEG. Исходник и active old theme сохранены до Apply;
  Close повторного edit без Save не публикует; focus contracts приняты.
  [Evidence](evidence/2026-10-16-native-gallery-image-editor.json).
- [x] Native image crop input EN/RU on private X11: Space/Tab/numeric typing,
  captured drag outside bounds, no drift after release, no native IMG DnD,
  Save via Enter. Missing-buttons/capture-loss/fresh-drag regressions passed;
  45 affected browser cases. [Evidence](evidence/2026-10-16-native-image-crop-input.json).
- [x] Browser capture/video region ownership: missing primary buttons и потеря
  capture не продлевают старый жест; обычный pointerup и новый drag работают.
  99 affected import/record cases; selection не запускает admit/Save.
  [Evidence](evidence/2026-10-16-region-gesture-ownership.json).
- [x] Private X11/Tauri/WebKitGTK capture-region trusted pointer RU: drag за
  границы, release/return без drift, injected capture loss и fresh drag; draft
  остаётся локальным до Apply. Recording/re-edit/Save/normal-close также приняты.
  [Evidence](evidence/2026-10-16-native-capture-region-input.json).
- [x] Private X11/Tauri/WebKitGTK video crop trusted pointer RU: outside/release,
  injected capture loss/fresh drag, сохранение crop после admission/preparation
  ошибок и возврата из review; decode/Save/membership/normal close приняты.
  [Evidence](evidence/2026-10-16-native-video-region-input.json).
- [x] Native capture/video pointer EN/RU на одной debug сборке:6 жестов на язык,
  crop recovery, decode/Save/membership и normal-close cleanup приняты.
  [Evidence](evidence/2026-10-16-native-region-bilingual.json).
- [ ] Real-session/Wayland/scaling, дополнительные loss modes и клавиатурная
  matrix обоих селекторов. Full-limit native workflow пока RU-only.
- [ ] Native image import/editor: real-session chooser и expanded pointer/keyboard/Tab,

  reset, errors/cancel/stale result, speech/focus/scaling, legacy migration,
  concurrent edits и other-context/profile/restart references. Bounded native
  gallery happy path выше не закрывает эту matrix.
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
- [x] Screenshot unconfirmed Cancel/Close: original connection/unique owner/path
  сохраняются до checked cleanup; новые captures fenced. UI сохраняет ID/source,
  coalesces Cancel, не закрывает editor при failed cleanup и не отменяет новый
  capture поздним старым reply. Read-only opaque cleanup ID позволяет явный
  recovery после потери widget; real private portal rejection/retry EN/RU и
  обычный native pixel/Save/Close workflow приняты. Реальные portals/dialogs
  и расширенная lifecycle matrix остаются открытыми.
  [Evidence](evidence/2026-10-16-screenshot-cleanup-recovery.json).
- [x] Screenshot verified owner loss: original bus подтверждает отсутствие exact
  unique owner; logical request освобождается с отдельным EN/RU notice.
  Replacement не получает старый Close, новый capture только явный. Bus/query
  failure сохраняет fence, неизвестный UI receipt сохраняет ID. Private Rust,
  browser и native replacement/recovery приняты; real dialogs остаются проверить.
  [Evidence](evidence/2026-10-16-screenshot-owner-loss.json).
- [x] ScreenRecord: после отказа подготовки вернуться к сохранённому исходнику,
  повторно подготовить и явно сохранить. Browser failed encoding/admission и
  native injected command errors с фактической отменой review приняты;
  это не закрывает общую fault matrix.
  [Evidence](evidence/2026-10-13-screen-record-review-recovery.json).
- [x] Killable recorder child process: bounded startup/control, Cancel/Shutdown и
  source-close с reap до staging/portal release, crash/protocol failure и новый
  capture после очистки. EN/RU errors и software/native qualification:
  [checkpoint](evidence/2026-10-16-screen-record-process.json).
- [x] Native WebKitGTK recovery при внешнем SIGSTOP/SIGKILL собственного recorder
  child: timeout/crash, Clear и joined Close, reap/staging cleanup, новый capture,
  decode → явное Save → gallery membership. EN/RU, 8 сценариев с synthetic source;
  реальные plugin/kernel stalls этим не квалифицированы.
  [Evidence](evidence/2026-10-16-screen-record-native-faults.json).
- [x] Synthetic release resource baseline recording engine: 320×240, 1080p и
  4K, по 30с + Pause/snapshot/Resume; CPU/RSS/threads/FD samples, decoded dimensions/frames
  и staging cleanup. Отдельно полный180с/4320frames при320×240: Resume reject,
  snapshot guard, finish и cleanup. Это не расходы полного GUI/portal и не
  long large-source/worst-case qualification.
  [Evidence](evidence/2026-10-16-screen-record-resources.json).
- [x] Software source resize: normalized crop следует текущему источнику через
  стабильный внутренний canvas; MP4 dimensions/CFR и предыдущие кадры сохранены,
  snapshot показывает актуальный full source. Рост/уменьшение/aspect/Setup/Paused
  и oversized failure/cleanup проверены настоящим кодеком. Реальные portals/DPI
  и Windows qualification остаются.
  [Evidence](evidence/2026-10-16-screen-record-source-resize.json).
- [x] Настоящий PipeWire transport в изолированном synthetic graph: inherited
  FD при неверном default remote, serial target property, Setup/crop/Pause/Resume,
  decoded pixels/dimensions/PAR/CFR и cleanup. Обрыв private server теперь
  переводит запись в Failed и удаляет незавершённый файл, вместо ложного Recording.
  Ручной `pw-link` не доказывает automatic routing/portal permissions;
  системные X11/Wayland gates остаются. EN/RU Clear → fresh capture проверены.
  [Evidence](evidence/2026-10-16-screen-record-pipewire.json).
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

- [x] Windows ZIP local inspection2.1.28: bounded ZIP/ZIP64 metadata/index,
  backup CRC/strict JSON/base64, explicit eligible LCDV2 records и private-path
  omission. CLI проходит с fake listening Unix peer без подключения/записи;
  synthetic .NET producer fixtures проверены. Import/assets validation остаются
  false, никакого Apply. [Evidence](evidence/2026-10-17-windows-profile-inspection.json).
- [x] Windows Screen source inventory: runtime/editor documents раздельно, все8
  banks/names/selected effects, три gallery references, default/missing явные;
  bounded payload CRC/SHA-256 включая unused uploads и thumbnails. Actual CLI
  offline без connections/writes. Media decode/theme mapping/Apply остаются
  неподтверждёнными. [Контракт](windows-profiles.md#screen-source-banks-and-payload-inventory--2026-10-17).
- [x] Basic Windows runtime → native proposal: explicit role/font/resource bindings,
  Image/Color, Custom/Time/Slider/Duo/Three/Four и watermark; actual CLI→renderer
  проверил live cache/freshness/unit/identity/cycle и обязательность resources.
  Layout review обязателен, Apply/editor banks/resource validation остаются false.
  [Контракт](windows-profiles.md#basic-runtime--native-proposal).
- [x] Basic Windows editor preparation: восемь source slot identities, existing
  ImageVideo/Color banks, все7 overlays и inactive gradients/Flip, frozen empty
  defaults и explicit default-video mapping; deferred effects без подмены.
  Actual CLI offline, malformed inactive settings reject whole bank.
  [Контракт](windows-profiles.md#basic-editor-banks--native-proposals).
- [x] Storage WPF Color отличается от runtime RGB: codec нормализует byte channels,
  сохраняет private color history/исходный editor JSON; unknown/profiled colors
  reject. Actual control CLI/MockBackend import/lost ACK/portable/restart проверен
  на owned WPF-property shape. [Correction](windows-profiles.md#storage-wpf-color-versus-runtime-rgb-correction).
- [x] Screen static image preparation: bounded source fingerprint/CRC validation,
  PNG/JPEG decode/normalization with alpha, owned bytes hidden from reports,
  deferred animation and no partial result/publication on later failure.
  Actual CLI offline без connections/config/source writes.
  [Контракт](windows-profiles.md#verified-static-image-preparation).
- [x] Screen GIF/MP4 preparation: source verification → shared bounded engine,
  движущиеся frames/valid H.264, owned private staging/Drop, cancellation and
  remaining batch budgets; actual CLI offline без partial publication.
  [Контракт](windows-profiles.md#verified-gifmp4-preparation).
- [x] Owned Screen resource plan: exact path/URI→prepared/native bindings,
  три galleries/current basic runtime/current index0 editor overlays, unused uploads,
  provisional keys и pending image/video/font pin sets; all-or-error offline CLI.
  Private owners не сериализуются, missing/wrong-kind/duplicate refs reject.
  [Контракт](windows-profiles.md#owned-screen-resource-plan).
- [x] Native basic editor storage: independent ImageVideo/BackgroundColor banks,
  семь overlay templates/selection в каждом, запрет nested/duplicate/incomplete
  state; edits survive selection. Inactive assets участвуют в native collectors,
  archive remap/clean-store transfer/restart и JSON/TOML. Browser effect switch
  сохраняет basic cache и draft dependencies.
  [Контракт](windows-profiles.md#native-basic-editor-storage).
- [x] Existing basic banks UI: два backgrounds × семь text presets, shared
  background/gradient и source watermark, независимые text edits/global theme name,
  draft-only selection/Profile Load, отдельные Save/Apply, busy/focus. Browser
  EN/RU1280/1024 и native14 previews/normal Close/GUI+daemon restart EN/RU.
  [Контракт](windows-profiles.md#basic-editor-user-workflow).
- [ ] Расширенная basic bank acceptance: оба backgrounds × семь
  overlays, изменение каждого inactive sensor/font/color/text/watermark/gradient,
  переключение туда/обратно, explicit Save/Apply, slot reload и restart. Current
  source runtime не должен молча заменяться editor proposal. Missing cache-only
  assets/fonts и aggregate decoded-image limit должны блокировать подготовку без
  partial mutation; cleanup должен сохранять inactive dependencies.
- [x] Windows Screen source identity: index0 current Full Screen editor; arrays8
  не превращаются в Linux named slots. Source1..7 остаются opaque без bindings;
  current absent selected basic bank materializes Import(empty) defaults, runtime
  остаётся независимым. Constructor-shaped actual CLI без source/config writes.
  [Correction](windows-profiles.md#windows-screen-array-identity-correction).
- [x] Independent imported editor storage: native field отдельно от active scene,
  bounded history/source aliases, backup+record fence, cleanup/firmware refs,
  native group archive remap/restart, reset/control boundaries и actor JPEG-only
  runtime при editor video. [Контракт](windows-profiles.md#independent-imported-screen-editor-storage).
- [x] Windows Screen native publication adapter: проверка всех owned payloads и
  dependencies до публикации, typed remap только новых uploads, pins, scoped
  rollback; source identity отдельно от target named library, native storage
  commit/restart с независимыми runtime/editor.
  [Evidence](evidence/2026-10-17-windows-screen-publication.json).
- [x] Windows Screen IPC upload/owner integration: target-bound Begin, immutable
  source options, private Inspect, explicit Finish через native Screen actor и
  durable receipt; native format mismatch, cancel/stale-target, repeated Finish,
  connection loss и restart readback.
  [Evidence](evidence/2026-10-17-windows-screen-upload.json).
- [x] Imported editor read/load-to-draft: отдельный read-only getter без source
  history, отдельная кнопка EN/RU, newer-draft guard, null/error preservation,
  explicit Apply; socket read/restart не меняют active scene/LCD generation.
  [Evidence](evidence/2026-10-17-imported-editor-load.json).
- [x] Desktop native Screen job: private file snapshot после выбора ZIP, отдельный
  mode/options через existing bridge, общий upload/Inspect/Finish/recovery;
  cancel/shutdown освобождают daemon preparation, source replacement не меняет
  выбранный snapshot. Job tests используют injected chooser и Unix IPC/mock.
  [Evidence](evidence/2026-10-17-native-screen-job.json).
- [x] Screen binding requirements: shared runtime/editor validation, roles/fonts/
  exact references и image-only constraints только для current0 и галерей;
  source1..7 остаются opaque, scoped absent-bank defaults сохраняются. Native
  metadata request из private grant без upload, target writes или Apply.
  [Evidence](evidence/2026-10-17-screen-binding-requirements.json).
- [x] Windows Screen desktop form: record/requirements, explicit sensor/font/
  resource choices, immutable preparation review, separate Apply и saved-id
  recovery; browser EN/RU и actual native PNG/private FileChooser/IPC/mock,
  imported-editor draft/sensor preview, square geometry, normal Close и restart.
  [Evidence](evidence/2026-10-17-windows-screen-form.json).
- [x] Windows Screen graphical frame preview before Apply: current scene/editor,
  private PNG/H.264 + live sensor cache + rotation, no publication/LCD writes,
  shared Cancel/Finish barrier и lost-caller ownership. Browser EN/RU, Unix IPC и
  native owned PNG EN/RU/normal Close/restart; delayed polling regression закрыта.
  [Evidence](evidence/2026-10-17-windows-screen-private-preview.json).
- [x] Windows Screen GUI owned MP4/GIF + realtime CPU overlay: native EN/RU,
  explicit source/font/sensor mapping, private scene/editor frames до Apply,
  preserved catalog/library/draft, explicit commit и normal Close/restart без
  исходного ZIP/MP4. Release mock output: complete H.264 stream decode480×480,
  short observed ≥20fps gate при target24, включая replay после restart.
  [Evidence](evidence/2026-10-17-windows-screen-media-matrix.json).
- [x] Windows Screen malformed media: native EN/RU с существующей LCD theme;
  видимая ошибка, no Apply, неизменные emitted JPEG bytes/draft/library/catalogs,
  no new video и joined private cleanup/normal Close.
  [Evidence](evidence/2026-10-17-windows-screen-media-matrix.json).
- [ ] Windows Screen GUI другие source variations, missing managed font/resource,
  source cancel/expiry/uncertain fault
  matrix и genuine exports. Empty runtime default-request не заменяется LocalUI
  без qualification.
- [x] Three/Four static dividers: source color/shadow, дробные координаты и
  thickness, независимые basic banks, portable archive roundtrip, native EN/RU
  Save/Apply, actual JPEG и restart. [Evidence](evidence/2026-10-17-grid-dividers.json).
- [x] Realtime video + Four Sensor + shadowed dividers: после уменьшения RGB
  shadow work minimum20fps software gate пройден:23.40fps при nominal24,
  722 новых кадра/30.849с; все723 кадра decoded480×480. Byte-exact RGB regression,
  lifecycle tests и package/production GUI checks прошли. Это короткий MockBackend
  run; physical/long-run acceptance остаётся отдельно.
  [Evidence](evidence/2026-10-17-realtime-shadows.json).
- [ ] Windows basic Screen: закончить font/value-dependent grid layout, расширенную приёмку
  desktop current/editor workflow и recovery/fault matrix. Runtime proposal
  не закрывает этот пункт. [Evidence](evidence/2026-10-17-windows-screen-runtime.json).
- [x] Windows cooling policy preparation: explicit record/CPU binding, все8 fan
  и3 pump modes, сохранённые имена/неактивные параметры/source settings,
  Windows runtime endpoints и Linux evaluator/fallback. Offline CLI без
  connections/config writes; это не full-group import и не GUI workflow.
  [Контракт](profile-groups.md#windows-cooling-policy-preparation--2026-10-17)
  и [evidence](evidence/2026-10-17-windows-cooling-preparation.json).
- [x] Полные cooling mode banks: typed per-device storage отдельно от active
  policies/named slots, TOML nullable metadata и Linux archive/journal/CAS/receipt
  roundtrip. Inactive edits влияют на revision; manual/policy updates и Screen
  import сохраняют banks; bad inactive policy не перезаписывает файл.
  [Evidence](evidence/2026-10-17-cooling-mode-bank-storage.json).
- [x] Device modes → local draft → explicit Apply: read-only owner getter,
  без provenance/global slot-library dependency, строгая frontend validation,
  late-response guards, empty/error и EN/RU diagnostics. Browser31 cases и native
  RU Tauri/Unix IPC/MockBackend с8/3 modes,2300RPM→58% и restart приняты.
  [Evidence](evidence/2026-10-17-cooling-device-modes.json).
- [x] Storage-only Save существующего cooling mode: CAS с durable target epoch,
  сохранение provenance/соседних deltas, отдельный Apply, pending disk state,
  readback после потери ответа без повторной записи и явный overwrite конфликта.
  Browser EN/RU34 и native RU Tauri/Unix IPC/MockBackend:Save2400 без Apply,
  lost ACK/одна запись, отдельный Apply60%, banks/revision после restart.
  [Контракт и границы](profile-groups.md#device-modes-сохранение-снимка-и-cas--2026-10-17).
- [x] Persisted cooling-mode selection: CAS-resolved stored Apply, ID/policy
  snapshot вместе в settings/group, restart не подменяет active snapshot новым
  Save. Custom Apply/manual override снимают ID, legacy None не угадывается;
  поздний Save не перепривязывает другой draft session. EN/RU browser47,
  actual IPC/group/restart; native RU dedicated restart восстанавливает
  FixRPM/2400RPM/60% без повторного Apply.
  [Контракт](profile-groups.md#device-modes-persisted-selection--2026-10-17).
- [x] Fresh-store standard banks: explicit known Celsius source, factory8 fan/3
  pump policies с fixed2200 RPM, отсутствие auto-Apply и overwrite, serialized
  concurrent initialization, pending disk/retry и readback без mutation replay.
  Factory labels EN/RU имеют explicit marker; literal/user input не переводится
  по совпадению текста. [Контракт](cooling-modes.md#standard-bank-initialization).
- [x] Full FanAndLighting reset восстанавливает все8/3 banks, имена и default
  IDs. Per-mode/default-name restore меняет локальный draft до Save/Apply;
  source, stale response и invalid draft проверены в EN/RU.
  [Контракт и evidence](cooling-modes.md#factory-mode-restore).
- [ ] Расширенные native faults/accessibility для per-mode/default-name restore;
  реальная работа PWM входит в физическую M2-08 ниже.
- [ ] Physical stored-mode flow [M2-08](lcdv2-m2-cooling-acceptance.md#cases):
  известный безопасный restore/source, existing test bank, Load/Save без
  изменения работы, stored Apply/restart snapshot, custom/manual clear и
  наблюдаемые RPM. Вентиляторы на motherboard не считать LCDV2 channel.
- [x] Offline Windows lighting-bank inspection:6/16 modes, all sensor High/Low
  settings, topology и selected IDs; malformed inactive data rejects preparation.
  .NET-owned fixture и offline CLI read-only contract;
  [границы](profile-groups.md#windows-lighting-bank-inspection--2026-10-17).
- [x] Windows lighting → typed Linux preparation:6/16 static bases,96 source
  bindings, per-mode selected/enabled state, source High brightness/direction;
  actual evaluator из CLI output, explicit units/IDs и fail-without-output.
  [Контракт](profile-groups.md#windows-lighting-linux-preparation--2026-10-17).
- [x] Windows FanAndLighting: complete typed composition, target-bound read-only
  preview, explicit CLI/Unix IPC commit через existing coordinator; сохранение
  target named slots/Screen/thermal, provenance и selected IDs, actual lost ACK
  retry, stale target и restart/readback. Только representable source subset и
  owned .NET fixtures. [Контракт](windows-profiles.md#fanandlighting-import-текущий-путь).
- [x] Desktop Windows FanAndLighting: native ZIP grant, explicit record/6 role
  bindings, full review и edit fence, exact-draft Apply/retry/readback. Native
  EN/RU private FileChooser/MockBackend проверил cancel, Linux slots/Screen/draft,
  restart с удалённым source ZIP и receipt без повторного импорта; worker — lost
  ACK/corruption/Close. [Evidence](evidence/2026-10-17-windows-control-desktop.json).
- [ ] Windows interoperability: оставшиеся Windows curves/parameters,
  Screen mapper, validation/remap assets,
  genuine exports и расширенная partial install/lifecycle приёмка.
  [Этапы](windows-profiles.md).


- [x] Shared Profiles list/edit EN-RU messages: raw diagnostics и user names
  остаются данными, empty fallback переведён, typed validation и readback guard
  сохранены.49 browser cases и broad native RU lost reply/rename/reset/delete/
  dependency/restart smoke приняты. Locale audit учитывает ValidationError,
  final913/913 после одного resource-only addition. Archive/group error UI и
  physical/native EN-RU расширенная matrix этим не закрываются.
  [Evidence](evidence/2026-10-16-profile-messages.json).


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
- [x] Cooling error/validation EN-RU: preset/Apply/poll/runtime context отделён
  от raw diagnostic; ranges сохраняют параметры и меняют язык, в том числе при
  local Profiles draft validation. Browser45 cases и native commit→lost reply→
  explicit edited retry EN/RU приняты. Штатный2s PWM refresh не считается новым
  пользовательским Apply; production12-state regression повторена. Error UI
  включён в текущий collections candidate; это не включает последующий
  Rust fix endpoints fan presets, описанный ниже.
  [Evidence](evidence/2026-10-16-cooling-messages.json).
- [x] Production GUI pump PWM/source/curve: draft отдельно от Apply,
  raw cmd138 source/duty, missing/aged-stale fallback87, same-ID recovery,
  manual override без поздней curve write и normal restart/replay проверены.
  По12 состояний с clock и sensor themes; combined run сравнивает19 полных
  LCD JPEG с literal text. Fan policy не менялась, final fixed61/LocalUI;
  это MockBackend, не physical duty/RPM. Extended policies/errors остаются.
  [Evidence](evidence/2026-10-16-production-cooling.json).
- [x] Fan presets включают Windows runtime endpoints0°C/10% и100°C/100%:
  четыре preset policies, low-temperature interpolation, actual mock cmd139,
  stale fallback/recovery и offline CLI/named-slot contract проверяются отдельно.
  Existing custom/saved curves сохраняют свои endpoint values; для новых defaults
  нужен новый explicit preset/reset intent.
  [Контракт и evidence](lcdv2-m2-cooling-acceptance.md#fan-preset-endpoints-2026-10-17).
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
- [x] ScreenRecord recovery copy: return/close cleanup errors — parameterized
  EN/RU resources, reactive language switch с сохранением raw diagnostics,
  capture/name и no implicit Save. First-frame error → Resume retry на той же
  reservation; known recorder/source/limit/plugin instructions локализованы.
  Это не закрывает остальные dynamic/native errors.
  [Evidence](evidence/2026-10-16-screen-record-messages.json).
- [x] ScreenRecord failed Close/Clear: permanent cleanup fence blocks Resume,
  edits/preparation, return and Save, retains status/draft across polling, permits
  explicit retry and fresh capture after checked Clear. Eight EN/RU browser cases;
  paused native transport rejection/retry → reap/new capture/decode/Save EN/RU.
  Native prepared-review media-cancel rejection and committed-cancel lost reply
  also accepted in EN/RU: actual readback, no publication, Clear/new capture/Save.
  Prepared-review capture-cancel rejection, switch/in-flight operations and real
  portal/backend failures remain open.
  [Fence](evidence/2026-10-16-screen-record-cleanup-fence.json),
  [native review/lost reply](evidence/2026-10-16-screen-record-review-cleanup.json).
- [x] Media import: reactive EN/RU upload/prepare/confirm errors, validation,
  manual/recorded crop labels и accessible group name. Raw diagnostics,
  filename/range и explicit Save сохранены. Cancel retry после подтверждённого
  same-job readback, с guards read-failure/foreign ID/publishing/in-flight;
  автоматического resend нет. [Evidence](evidence/2026-10-16-media-import-recovery.json).
- [x] LCD source readback: четыре известных режима локализованы EN/RU;
  Idle — отсутствие software stream, без неподтверждённого утверждения LocalUI.
  Unknown values/last_error остаются literal, null → «—», смена языка сохраняет
  draft и не делает Apply.39 browser и native Idle→Theme→Idle EN/RU приняты;
  static resources915/915. [Evidence](evidence/2026-10-16-lcd-source-labels.json).
- [ ] Dynamic outputs и native dialogs/menus на EN/RU, grammar/layout/accessibility,
  restart/offline/CAS/lost replies. Native worker errors вне file chooser и
  настоящие desktop dialogs ещё требуют квалификации. Остальные языки отложены до полировки и
  не блокируют базовый этап. [Политика и acceptance](interface-language.md).
- [x] LCD Apply/load/LocalUI: reactive context EN/RU отдельно от raw diagnostics,
  отсутствие активной темы отдельно от RPC error; preview/connection/persistence
  не переводят backend text как ключ. Draft/name и explicit retry сохранены.
  Browser guards/reconnect и native WebKitGTK/Unix IPC с private fault proxy
  приняты в обоих языках. Остальные image/gallery/media-selection errors,
  native transport faults и реальные desktop/hardware проверки остаются.
  [Evidence](evidence/2026-10-16-lcd-composition-messages.json).
- [x] ImageEditor source/preview/save, plain image import, gallery replacement
  и video selection: reactive EN/RU instructions с raw diagnostics; acknowledged
  file без подтверждённой membership — отдельный исход. Нет автоматического
  повторного Save/Apply; late import/gallery success/error после reconnect
  отбрасываются. Browser EN/RU/ambiguous replies и private native decoder/catalog/
  membership rejection приняты. Более широкий library management — отдельный
  срез ниже; реальный transport/portal/HID
  и screenshot cancellation fault matrix остаются открытыми.
  [Evidence](evidence/2026-10-16-image-publication-messages.json).
- [x] Image/video list/delete и image rename: EN/RU instruction отдельно от raw
  diagnostics; owned fallback labels локализованы. Rename сохраняет введённое
  имя и блокирует старый form до нового выбора fresh card; failed readback
  оставляет guard. Browser ambiguous replies/language changes и native actual
  metadata commit → substituted error → fresh revision приняты. Late video editor
  opening после reconnect отбрасывается. Это не native list/delete/disconnect/
  timeout или полная video replacement lifecycle matrix.
  [Evidence](evidence/2026-10-16-library-management-messages.json).
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
- [x] Spec language/data boundaries EN/RU: locale-aware numbers/date без refetch,
  raw backend diagnostics/unknown keys/hardware strings, отдельная owned response
  failure.21 browser и native private-provider EN/RU/fractional bytes/date/Close
  приняты. [Evidence](evidence/2026-10-16-system-spec-language.json).
- [ ] Spec inventory: проверить полноту
  соответствующих полей на Windows/Linux и реальных платформах, доступность
  DMI/DIMM/EDID, hotplug и масштабирование. [Контракт и software checks](system-spec.md).

- [x] Video source notices EN/RU: typed recording states и canvas preview error
  переводятся при render. Playback failure/retry/pause и retained language switch
  приняты; color readback failure также переключает язык без prepare/Save.
  Static gate923/923; broader dynamic qualification остаётся открытой.
  [Evidence](evidence/2026-10-16-video-source-notices.json).

- [x] Desktop GUI EN/RU diagnostics/poll ownership: read/action ошибки разделены,
  успешный poll не стирает Save/Exit failure; draft/no-auto-write сохраняются.
  Raw tray/registration/integration messages и owned validation/notice language
  switch приняты в31 browser case; native RU lifecycle также повторён.
  [Evidence](evidence/2026-10-16-desktop-messages.json).
- [x] Native tray menu EN/RU: fresh EN default, cached language после GUI init
  без daemon, restart и runtime switching; реальные DBusMenu labels/actions,
  invalid enum rejection, no PWM/firmware writes и joined Exit приняты.
  [Evidence](evidence/2026-10-16-menu-integrated-candidate.json).
- [x] Native owned tray-status copy: typed host_unconfirmed/unavailable,
  reactive EN/RU resources и raw external diagnostics.42 browser/140 GUI Rust,
  native EN/RU3 phases с no-host status и joined Exit приняты; creation failure
  покрыт Rust/browser. [Evidence](evidence/2026-10-16-tray-status-language.json).
- [ ] Реальные tray panel/pointer/speech/scaling и создание значка при отказе
  indicator library; real desktop qualification остаётся отдельным gate.

- [x] Display/FPS/source errors EN/RU: raw diagnostics, typed owned validation,
  reactive recovery и Display/source explicit Reload guard.43 browser cases,
  Svelte0/0/static933/933; native language и RU source lost-reply/Reload/recovery
  с broad LCD/cooling/RGB/profiles/restart прошли. Новые Display/FPS
  native fault injections и прочие dynamic paths остаются открытыми.
  [Evidence](evidence/2026-10-16-settings-messages.json).

- [x] LCD settings/reset/collections EN/RU: owned context/raw diagnostics,
  matching receipt clears old failure, typed migration/save notices и accessible
  remove labels.58 initial browser,23 final affected +4 success resource cases;
  Svelte0/0/static944/944; native RU LCD lost-reply/timeout/draft/recovery и broad
  sensors/controls/profiles/restart прошли. [Evidence](evidence/2026-10-16-lcd-lifecycle-messages.json).
  Native reset/collection faults и реальные desktop/speech/scaling ещё открыты.

- [x] RGB errors EN/RU: raw read/load/poll/apply/output diagnostics, typed local
  validation100/3600, retained draft/no auto-write и scoped thermal-base hint
  с исходной причиной.54 regression +22 final browser cases, Svelte0/0/static959/959;
  broad native RU RGB transport/sensor/controls/profiles/restart повторён.
  [Evidence](evidence/2026-10-17-rgb-messages.json). Новые native fault injections
  и реальная RGB/accessibility/desktop приёмка остаются открытыми.

- [x] Temperature input display/ARIA согласованы; RGB °F grid объясняет целые°C,
  help связан с полями, fraction не округляется в policy.74 browser cases,
  Svelte0/0/static959/959; native RU zero-device preview/restart/blocked Apply и
  Mock LCDV2 Static actual IPC87.8/143.6°F →31/62°C приняты. Broad native regression
  повторён. [Evidence](evidence/2026-10-17-temperature-grid.json). Native DOM edits
  не закрывают physical/keyboard/speech/Wayland/scaling gates.

- [x] Font/cleanup/thumbnail resources EN/RU и safe dictionary fallback: raw
  diagnostics, typed local validation, retained import/cancel statuses, explicit
  cleanup rescan/retry, compact/full decode recovery и URL cleanup. Metadata
  polls не передекодируют unchanged clip key; user names literal.57 affected +41
  final browser cases,3 localization contracts, Svelte0/0/static971/971 и native
  recording EN/RU/broad font-cleanup/language-restart приняты в своих scopes.
  [Evidence](evidence/2026-10-17-library-resources.json). Full native fault/physical/
  session/speech/scaling и реальный GitHub run остаются открытыми.

- [x] Collection membership local diagnostics сохраняют resource/parameters до
  отрисовки EN/RU; remote diagnostics literal. Malformed read не допускает запись;
  acknowledged import + pending membership сохраняет файл и блокирует повторный
  Save. Добавление image/video проверяет post-write revision/member без replay;
  newer revision с сохранённым member принимается, соседние записи сохраняются. Stale membership и replacement readback имеют отдельную local diagnostic.
  [Evidence](evidence/2026-10-17-collection-diagnostics.json). Новые native malformed/
  pending faults и реальная аппаратная/desktop/speech/scaling приёмка открыты.

- [x] Native edited-image post-write readback EN/RU: stale/missing/null,
  pending без/с raw detail, RPC error и malformed response.14 actual daemon
  commits,14 injected readbacks, сохранённые соседние entries, source filename,
  raw diagnostics и local draft; duplicate Save blocked,0 JPEG и joined Close.
  Image/LCD fault и broad controls/profiles/restart regressions прошли.
  [Evidence](evidence/2026-10-17-native-collection-readback.json). Plain/video/replacement, reconnect timing и реальные
  keyboard/speech/Wayland/scaling/portal/hardware gates остаются открытыми.

Включено в [общий кандидат collections](evidence/2026-10-17-collections-integrated-candidate.json):
676 browser,1245 Rust/9ignored, strict checks/release и native scopes.731 входов
не менялись; static980/980,49 dynamic calls — inventory. Остальные native negative
readback и реальные hardware/session/speech/scaling проверки остаются открытыми.

## Sensors и Windows reference

Source-only [audit2026-10-16](evidence/2026-10-16-sensor-gap-audit.json)
подтвердил открытые voltage/Intel boundaries: deprecated/removed nvidia-smi
voltage не даёт универсального headless fallback, Windows manager выбирает
HWiNFO reading, generic VIN не доказывает CPU rail. В этом source-only audit
приложение и тогдашний readiness package не изменялись; пункты ниже остаются unchecked.


- [x] Production GUI → sensor LCD: exact source/name/label/Fahrenheit через
  native controls, preview отдельно от одного Apply; полные mock-HID JPEG480×480
  сравниваются с literal text для valid/missing/stale/recovery, после GUI Close
  и restart/replay без Apply. Reopened editor читает saved theme; LocalUI restored.
  English production workflow, synthetic cache/MockBackend; физическая accuracy
  и Cycle/полная controls matrix не закрываются.
  [Evidence](evidence/2026-10-16-production-sensor-theme.json).


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

Пошаговый протокол и таблица статусов — [LCDV2 acceptance](lcdv2-hardware-acceptance.md).
Новые A-016…A-021 отдельно проверяют Windows import/private preview/current-editor
separation, realtime video с датчиками, corrupted source при уже работающем LCD
и сохранённый replay без исходных файлов. Software evidence не меняет их
physical `not-run` status.

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
- [x] Mini package-consumer checks: packaged shared model,3 Qt CTest targets,
  GJS transport/production CLI/read-only Unix IPC и GTK preferences/persistence.
  Actual packaged QML libraries/CLI загружены в private Plasma host, EN/RU и
  helper teardown проверены; прежний staged workflow сохранён. Default CI mode
  добавлен в GitHub workflow source, remote run не наблюдался.
  [Evidence](evidence/2026-10-17-packaged-mini.json). Real GNOME Shell/panel/session/
  scaling/speech и версиям по-прежнему нужна отдельная приёмка.

- [x] Private pacman install0.1.0-1 → upgrade0.1.0-2 → remove: file hashes/modes,
  оба Mini stores, host ABI/CLI version, user-data retention; deterministic repack.
  Actual packaged production GUI: English embedded frontend/private IPC/Close,
  missing recorder channel rejected before GTK. [Evidence](evidence/2026-10-16-package-delivery.json).
- [ ] GitHub CI execution, clean-host binary reproducibility и real-session
  install/autostart/upgrade/uninstall; shell enable/panel/login/version matrix.
- [ ] На итоговом snapshot: fmt, strict Clippy, tests, docs, release build,
  frontend check/build/contracts/browser и isolated native flows; затем общая
  hardware матрица. Старые успешные checkpoints не доказывают зелёное текущее дерево.
  Software/package часть принята в текущем checkpoint ниже; hardware/desktop/
  portal/platform и оставшиеся reference/native gates остаются открытыми.

Исторический Collections checkpoint (не текущий кандидат): `target/distribution-20261017-collections`.
731 unchanged inputs;1245 Rust/9ignored (с doctests),676 browser,4 Python unit tests
и private package transactions,39 theme contracts,3 localization contracts.
Strict fmt/Clippy/rustdoc, Svelte0/0/static980/980/release прошли. Native EN/RU
и actual packaged production GUI0/1mock подтвердили19 sensor JPEG/12 pump states,
GUI Close/restart/replay/final LocalUI. [Полное evidence](evidence/2026-10-17-collections-integrated-candidate.json).
Hardware/desktop/portal/CI/Windows/platform и расширенные native gates открыты;
ignored не засчитаны как passed.49 dynamic calls — inventory. При изменении
входов новая общая квалификация требуется снова.

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
