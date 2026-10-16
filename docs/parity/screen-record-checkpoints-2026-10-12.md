> Historical checkpoints retained during consolidation. Current contract and remaining work: [ScreenRecord](screen-record.md). Old pending statements describe their checkpoint, not current status.

# GA2LCD ScreenRecord: contract and implementation steps

Baseline: L-Connect3 2.1.28.0 / Galahad II LCDV2. This is desktop recording,
separate from selecting intervals in an existing video with Record/Pause.

## Windows behavior confirmed from local reference

`L-Connect Editor/LConnectEditor/Views/Tools/ScreenRecordTool.cs`, outside this
repository:

- Lines304–343 select the capture rectangle in physical coordinates and start
  or resume ScnLib recording while recorded time is below180000ms.
- Lines347–358 pause, then clear with confirmation; clearing stops the recording.
- Lines389–418 update the displayed recording time, automatically pause at180s,
  then show VideoOperationDialog. Cancel returns to selection; confirmation
  stops recording and prepares the result.
- Lines481–489 pause and allow changing the capture area before resuming.
- Lines558–565 disable audio and select VP9 for GA2LCD import output. This is
  not evidence of a fixed24fps source capture rate; Linux LCD conversion remains
  its separately verified H.264/24fps output path.

## Linux ScreenCast negotiation implemented

`screencast.rs` implements the portal session layer, not the recorder or UI.
It checks advertised source types and requests one monitor/window source,
without persistent permissions. CreateSession → SelectSources → Start →
OpenPipeWireRemote holds a dedicated connection, session and owned file descriptor
until the recorder closes them. The node ID is retained for older portals;
v6 requires the stable `pipewire-serial` property to avoid following a reused
node ID. Stream metadata coordinates are not assumed to be physical pixel sizes.

Every request subscribes before making the method call. Cancellation and failure
close the active request/session with bounded calls. Session::Closed is watched
from before creation through selection and exposed to the future recorder; it
interrupts pending selection. Explicit close drops the remote FD, joins the
monitor and calls Session.Close. Drop attempts cleanup as a fallback; the future
recording owner must explicitly join shutdown rather than rely on Drop.

[Portal specification](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
provides the request/session lifecycle and v6 stable serial contract. The local
GStreamer pipewiresrc plugin exposes fd and target-object properties; that
property inspection does not establish capture or encode functionality.

## Recording engine implemented

`screen_record.rs` provides a blocking owned GStreamer worker. It duplicates
only the supplied portal FD, targets the serial (legacy node path fallback
only when the negotiated portal supplies no serial), and monitors Session::Closed.
PipeWire must support40ms unchanged-frame keepalive. `imagefreeze` with live
output and replacement holds the latest full frame at24fps even during a
responsive delivery gap. Admitted frames receive consecutive CFR timestamps;
setup/pause buffers cannot spend saved-video time. No arbitrary default source
is selected. Explicit upstream EOS and Session::Closed remain errors.

The Linux recording policy is H.264 MP4 at24fps, raw input dimensions2…4096
per axis, even encoded dimensions, at most4320 frames/180s active time and
256MiB mux output. This is a Linux policy, not proof of Windows source FPS or
codec equivalence. Oversized source negotiation fails rather than capturing a
different source. Queue depth is2; encoder uses2 threads. MP4 staging belongs
to a TempDir; explicit finish transfers ownership to RecordingFile, and dropping
that result removes the file.

Setup/preview/region selection before explicit start, pause/resume/finish,
independent active-time budget excluding setup and pauses,
frame fence and automatic LimitReached pause are implemented without UI polling.
Resume at the limit is rejected. Stop prevents additional raw frames, drains
EOS downstream of the repeating element, checks mux/file budgets and transitions to Null. Cancel/Drop joins the
worker; errors remove staging. BUFFER_LIST is included in output accounting.
State/EOS waits have deadlines, but native plugin calls/teardown themselves can
block: this in-process engine is not a hard-kill process boundary.

Synthetic tests use videotestsrc only: decoded MP4 frame count/duration,
pause gap exclusion, idempotent Resume, shortened automatic limit, sparse
updates, budget failure, cancellation and session-close notification. This
neither opens PipeWire nor captures the active desktop. Responsive stopped
delivery, held-frame duration/content, resumed updates and finite-source EOS
are also covered. A deadlocked plugin and real PipeWire timestamp/resend
semantics remain qualification gates.

[Engine checkpoint](evidence/2026-10-12-screen-record-engine.json).

## Desktop recording owner and commands

`screen_record_jobs.rs` reserves one caller-known32-character ID before opening
capture. Repeated begin/start recover a lost reply; a reservation without Start
expires after10s. A dedicated blocking owner opens the portal, runs recorder
controls, retains the ready file, and closes recorder before session. Admission
is held until the worker joins. Controls use a bounded queue; cancellation uses
an independent signal. Lost control acknowledgements do not abandon work, and
repeated Finish in Ready does not produce another recording.

Both Tauri builders register `screen_record_begin/start/status/control/cancel`
and the main-window capability allows only these typed operations. Begin/Start
are async wrappers so task spawning runs inside the runtime. Status returns
camelCase metadata, never a caller-selectable native path. Closing the window
joins recording shutdown alongside existing media/capture owners.

Cancel/shutdown guarantee local resource release. If remote Session.Close fails,
it is retained as Failed status and logged; local FD/worker release permits exit
without indefinitely retaining a broken portal. Actual portal Close failure,
late replies, selection timeout and native window-close qualification remain.
The ready recording now has the admission/review bridge described below;
original-source HTTP preview is implemented below; an initial user-facing workflow is now connected below.

[Owner checkpoint](evidence/2026-10-12-screen-record-owner.json).

## Recording to prepared review

`screen_record_admit` synchronously pins a Ready recording into MediaImports
using its cached finalized length. It neither writes a dummy staging file nor
starts encoding. Immutable gallery/recording leases and mutable uploads are
separate InputStorage variants. The caller must await admission, or recover its
known ID through media_status after a lost reply, before preparing/cancelling.
Cancellation before calling admission does not create a tombstone for future IDs.

`screen_record_prepare` only prepares an already-admitted ID in an owned task.
The source lease survives capture cancellation, then releases after preparation.
Output reaches the existing ReviewReady/HTTP preview path; only the existing
explicit media_confirm publishes it. Normal review cancellation removes the
prepared result. Fit and ordered crop segments use existing validation/encoding;
no filesystem path or video bytes are supplied by the webview.

Synthetic regression records a real MP4, admits it, cancels capture, verifies
source retention until review, frame-count preservation and no published asset
key. Another test cancels admission before a delayed Prepare and proves it
cannot start encoding. These are native library tests, not a GUI/daemon save
workflow qualification.

[Review bridge checkpoint](evidence/2026-10-12-screen-record-review.json).

## Ready recording source preview

Finalization inspects the completed MP4 with FFprobe before publishing Ready
metadata. The existing token-protected loopback HTTP server accepts a recording
source ID and bounded Range reads through a Ready-only lease. No native path is
exposed. Cancel clears metadata and revokes new reads; an already-admitted read
retains its immutable lease until completion. Cancellation also signals the
metadata probe; remote session cleanup errors remain observable.

The synthetic test verifies metadata, exact206 response bytes, reader bounds
and404 after cancellation. It does not test WebKit playback or deterministic
cancellation inside the metadata probe. [Checkpoint](evidence/2026-10-12-screen-record-preview.json).

## Frontend capture lease

`desktop/src/lib/screen-record.ts` provides RecordingSession for the future
editor. A single known ID recovers lost Begin/Start replies; foreign status
cannot be adopted. Dispose waits for reservation/start settlement before cancel,
suppresses late status, surfaces cleanup failure and permits explicit retry.
Ambiguous controls are surfaced for readback without replay. Six mocked IPC
regressions pass, including Close while reserving and during initial status.
This capture client is not connected to a Svelte control yet. The separate
RecordingReview lease now handles admission/readback, owned preparation, explicit
confirmation and close. Close waits for admission but can interrupt preparation;
foreign import IDs and dispatched publication are left untouched. Ten mocked IPC
regressions and frontend check pass. The initial ScreenRecord UI workflow is connected below; region/edit and full
automatic-limit review semantics remain open. [Review client checkpoint](evidence/2026-10-12-screen-record-review-client.json).
[Checkpoint](evidence/2026-10-12-screen-record-client.json).

## Initial ScreenRecord GUI

The video library opens ScreenRecorder with a device-bound lifetime. It supports
monitor/window portal selection, start/pause/resume/finish, original MP4 preview,
name and contain/cover/stretch, native preparation, prepared review and explicit
Save into the catalog and device collection. Clear requires confirmation and
joins both capture and media cancellation before permitting a fresh recording.
Closing/unmount joins through the owned clients; generation guards suppress old
async completions after clear. Cleanup failures on explicit close remain visible
and permit retry; unmount failures are logged, with native app shutdown as the
final owner. Publication already dispatched is preserved for status recovery.

This is not yet the entire Windows workflow: there is no capture-region editing,
interval crop selection or full Windows operation-choice dialog. At LimitReached,
a modal now offers Finish or return to the retained paused recording. Escape
returns focus to Finish; repeated polls do not reopen the dismissed modal.
Confirmation finalizes once, then source/prepared review and Save remain explicit. Real portal/native UI, decoded browser video and full
limit/fault qualification remain. Browser tests use mocked IPC and an intentionally
undecodable video URL; they prove controls, ownership and membership, not playback.

## Remaining implementation and acceptance

Current backlog; older checkpoint descriptions elsewhere in this document describe
what was available at that time. Source/preview playback, interval trim/crop,
review return/reprepare and initial synthetic native Save/membership/Close are
implemented and qualified at the scope described in the later checkpoints.

- [ ] Qualify real PipeWire FD/serial targeting, unchanged-frame keepalive,
  timestamps, total source stall/loss, resolution changes and decoder/resource
  faults. Last-frame repetition and recovery after a responsive delivery gap
  are synthetic-tested; explicit source EOS is an error. These tests do not
  establish real portal interoperability or deadlocked-plugin recovery.
- [ ] Qualify native automatic-limit choice/error/accessibility behavior. Manual
  and automatic Cancel/Edit/Import choices are implemented; initial native manual
  Edit/Import flows pass. Full180s continuous synthetic engine acceptance passes.
  Real capture and resource profiling remain unqualified.
- [ ] Qualify region physical-pixel mapping (pre-recording and paused visual/numeric selection is implemented)
  on multiple monitors/fractional scaling; explicitly distinguish monitor/window
  selection from a crop region, including portal backend capability differences.
- [ ] Expand native crop/edit/limit/publication-failure acceptance, including
  lost replies and device-switch behavior. Extra recorded frames after Resume
  have passed in the synthetic native workflow.
  Initial synthetic source/review decode, seek, return/edit/reprepare, explicit
  Save/membership and normal window Close have passed.
- [ ] Native focus, keyboard/speech and scaling qualification of editor/limit
  states; browser controls and initial native workflow do not cover this matrix.
- [ ] Real X11/Wayland portal/PipeWire checks. No active desktop capture or
  physical LCD testing in the current synthetic slices.
- [ ] Expanded negotiation faults: backend unavailability, delayed method reply,
  version/type differences, malformed handles/FD and Close failures.

Private-bus tests cover advertised source filtering, the full success sequence,
real Unix FD passing, v6 serial, portal cancellation, empty streams, missing
serial, remote-open failure, application cancellation and Session::Closed during
Start. The recording engine was added subsequently; ScreenRecord UI remains absent.

[Session checkpoint](evidence/2026-10-12-screencast-session.json):60 library +2 binary
Rust tests passed on the final run; strict Clippy/docs/fmt passed. Two earlier
full runs failed the existing HTTP post-shutdown connect assertion, while its
isolated recheck passed. Cause remains unresolved; added address diagnostics
without weakening the assertion. Logs are retained in the checkpoint.

Последующая проверка: HTTP shutdown усилен явной остановкой socket перед
закрытием descriptor; regression и20 library-прогонов прошли. Конкретный
владелец в исходном сбое не установлен. См.
[отдельный checkpoint](evidence/2026-10-12-media-http-shutdown.json).

[ScreenRecord GUI checkpoint](evidence/2026-10-12-screen-record-ui.json): frontend check/build и40 focused tests прошли; scope и ограничения указаны в evidence.

[Limit prompt checkpoint](evidence/2026-10-12-screen-record-limit-ui.json):14 focused IPC/browser tests passed. Mock LimitReached does not qualify a full180s capture.

## Finished-recording cut and crop

The source player now reuses VideoSourcePreview (scrubbing, crop canvas and
pointer/keyboard crop controls). An optional single VideoCut interval exposes
start/end in seconds and normalized spatial crop through percentage fields.
Bounds use finalized native metadata; unknown duration, empty/reversed/outside
intervals, sub-millisecond empty output and out-of-frame crops block admission.
The same cut DTO reaches recording admission and the existing native preparation
pipeline; defaults still prepare the complete source. This uses VideoCut
millisecond semantics, not VideoEdit single-range frame semantics.

This is post-recording editing. Selecting/changing the actual capture region
between recording intervals, multiple output intervals and the complete Windows
operation-choice UI remain open. The focused browser test checks exact admission
DTO (250–1750ms, right half), invalid fields and explicit review. It does not
prove decoded preview pixels or native recorded-source output equivalence.
[Checkpoint](evidence/2026-10-12-screen-record-crop-ui.json).

## Ordered finished-recording fragments

Post-recording editing now supports1…64 ordered fragments, independent temporal
bounds and crop rectangles, add/delete and move earlier/later. Preview follows
the selected fragment's crop; edits remain stored when switching selection.
Validation covers every fragment, not only the selected one, and the sum of
rounded millisecond intervals must not exceed180000ms. Disabling/re-enabling
editing preserves drafts; clearing the capture resets them.

The browser regression selects two different crops, rejects200s total, shortens
the selection, reorders fragments and verifies exact admission order/coordinates.
This supersedes the earlier single-interval limitation. Capture-region changes
while recording and decoded/native qualification remain open.
[Checkpoint](evidence/2026-10-12-screen-record-fragments-ui.json).

## Decoded browser preview qualification

A synthetic H.264 MP4 (128×64, red left/blue right,2s) is served to the real
browser video element with HTTP Range support. The ScreenRecord workflow now
has evidence for decode dimensions, seek to0.5s, playhead→cut start, blue/red
crop canvas pixels while switching fragments, and player pause/source removal
on Close.17 focused browser/client tests pass. The native capture/admission
commands remain mocked; this does not qualify PipeWire, native WebKit or the
encoded output of the selected crop. No desktop pixels were captured.
[Checkpoint](evidence/2026-10-12-screen-record-browser-decode.json).

## Native prepared recording crop qualification

A real GStreamer synthetic recording is admitted with two ordered500ms crops,
then capture is cancelled before preparation. The admitted file remains until
Prepare completes; afterward it is removed. The prepared MP4 is read through
bounded native preview reads and decoded by FFmpeg:24frames, first12 red and
last12 blue, matching the per-segment spatial regions. Review has no published
asset key, and cancelling review revokes preview access.

This proves ordered spatial crop application and output duration through the
recording→admission→prepare path. The sampled SMPTE bars are temporally constant,
so it does not prove correct selection of the requested source timestamps.
Native GUI/Tauri IPC, real portals/PipeWire and hardware remain unqualified.
[Checkpoint](evidence/2026-10-12-screen-record-native-crops.json).

## Recorded-source temporal selection qualification

The temporal regression decodes the completed recording's changing lower-right
SMPTE noise without trimming/concatenating it. After native admission, capture
cancellation and preparation, each decoded output frame is matched against the
first24 source frames. Output must contain exactly24 frames in sequence12…23,
then0…11; tied nearest matches fail. This checks both source selection and
reordering through the recording path while tolerating lossy re-encoding.

It closes the static-pattern evidence gap for24fps at500ms/1000ms boundaries.
It does not prove arbitrary fractional boundaries, VFR, Windows SDK rounding or
native portal/UI behavior. Static source provenance remains independently
covered by the previous spatial test. [Checkpoint](evidence/2026-10-12-screen-record-temporal.json).

## Initial native Tauri/WebKit qualification

`native-recording.py` runs a private Xvfb/D-Bus/XDG/TMPDIR fixture with MockBackend.
Only the `native-smoke` feature exposes the synthetic GStreamer source; production
builds do not select it through an environment variable. Native GUI Start/Pause/
Resume/Finish, source320×240 decode and seek, prepared decode, explicit Save and
membership in an explicit migrated collection pass. A second recording is active
when the harness closes the window normally: exit0 and empty staging are required
before forced harness cleanup. The saved clip remains in the mock daemon catalog.

This qualifies the initial real Tauri/WebKit workflow, not actual portal/PipeWire
capture or hardware. Crop/edit, limit and failure matrices remain native gates;
additional frames after Resume are not explicitly asserted in this harness.
[Checkpoint](evidence/2026-10-12-screen-record-native-ui.json).

## Return from prepared review to editing

ReviewReady now offers return to editing. It cancels only the prepared-media
lease, retaining the Ready capture and fragment/name/fit drafts. Re-preparation
uses a new media job ID. A failed cancellation leaves the review controls in a
retryable state with Save disabled; stale polling cannot erase that state.
Successful return restores heading focus. Neither discarded result publishes.

Browser regressions cover normal return, failed cancellation with retry, retained
source/draft, changed fit/name and a new preparation ID. Native harness also
checks retained source decode, second prepared decode and no early publication.
[Checkpoint](evidence/2026-10-12-screen-record-return-edit.json).

## Full production-duration engine acceptance

The explicitly ignored test `full_production_limit_pauses_and_finalizes_three_minute_video`
runs the production synthetic recorder in real time without Pause/Finish commands
before LimitReached. The final revised run decoded4320 H.264 frames and exactly180s
of MP4 (35,362,290bytes). Resume is rejected at the limit, frames and active-time
counters stay fixed for500ms, Finish drains successfully and dropping the owned
file removes staging. The observation deadline also applies to the terminal state.

Run explicitly with `cargo test -p lconnect-gui --lib --locked full_production_limit -- --ignored --nocapture`.
The normal suite retains shortened-budget tests; this three-minute test is ignored
by default. This qualifies a continuous synthetic source and frame-budget boundary,
not completely stalled input, native UI limit, actual PipeWire or CPU/RSS profiling.
[Checkpoint](evidence/2026-10-12-screen-record-full-limit.json).

## Stopped buffer delivery acceptance

Synthetic `stop_after` drops every source buffer after the initial six while
leaving the source responsive to state changes and EOS. Regressions verify a
stable frame counter during the resulting gap, the independent active-time
limit (shortened to2s), rejected Resume, decodable Finish and removal of the
owned output. A separate case cancels during the gap and verifies worker join.

This closes the responsive-source/no-new-buffers control-path case. It does not
simulate a deadlocked plugin, certify real PipeWire keepalive behavior, or prove
that the missing visual tail is filled with the last frame. Cancellation elapsed
time is asserted after Drop returns; that assertion is not a watchdog capable
of interrupting a hung plugin. Those cases remain acceptance/research gaps.
[Checkpoint](evidence/2026-10-12-screen-record-stopped-delivery.json).

## Cancel/Edit/Import operation choice

Windows `ScreenRecordTool.cs:362–366,402–432,492–519` pauses before the operation
chooser, retains paused capture on Cancel, and routes Edit/Import separately.
`VideoOperationDialog.cs:35–48` returns those three choices. GA2LCD import scales
whole captured video to target width/height (`createCoverVideoForImport`); Linux
uses stretch for the whole-recording path, while retaining explicit prepared
review and Save as separate actions.

Both manual Finish and automatic LimitReached now offer return to recording,
Edit and prepare-whole-recording. Manual entry waits for confirmed pause; Escape
retains capture and restores Finish focus. Edit opens the existing source editor;
Import waits for Ready and prepares once. Clear discards pending intent. Capture
polls use revisions so a delayed old response cannot undo the paused dialog.

26 focused client/browser tests cover ordinary choices, manual cancellation,
failed Pause, delayed Ready, Clear before Ready and late status responses. Native
synthetic Tauri/WebKit qualifies manual Edit and whole Import preview without
early publication, plus six additional frames after Resume. Save/membership,
return/edit/reprepare and normal Close cleanup still pass. Native automatic-limit
and extended failure/accessibility matrix remain open; real portal/hardware were
not used. [Checkpoint](evidence/2026-10-12-screen-record-operation-choice.json).

## Paused capture-region engine and control command

The recorder accepts `set_region` only while Paused. It validates a normalized
nonempty rectangle, crops RGBA source pixels before scaling, and pins output
width/height and pixel aspect ratio1:1. Region updates are serialized through
the existing recording owner; status exposes the last accepted region (`null`
means full source), including after finalization. Invalid phase/coordinates or
stale caller ID do not change the last accepted region. The existing control
command accepts `{"set_region":{"x":0.5,"y":0,"width":0.5,"height":1}}`.

Synthetic decoded-frame acceptance switches full frame → red region → blue
region across two pauses. Every frame of both cropped intervals is checked;
first-frame red/blue source pixels, fixed320×240 dimensions and SAR1:1 are
asserted. An initial failure exposed changed pixel aspect ratio after crop;
pinning SAR prevents MP4 codec renegotiation. Owner tests cover serialized
command decoding, phase/stale/invalid rejection and successful status readback.

This is backend support. Pre-recording region selection, visual/numeric GUI,
source preview while paused, multi-monitor/fractional mapping and source-size
renegotiation remain to implement/qualify. Existing native workflow regression
uses full-source recording; it does not qualify native region selection.
[Checkpoint](evidence/2026-10-12-screen-record-region-engine.json).

## Paused numeric region controls

ScreenRecorder exposes source-relative X/Y/width/height percentages while paused,
with validation, explicit Apply and last accepted region readback. Resume is
blocked while the draft differs from applied state. Restore copies confirmed
values; Full source resets only the draft until Apply. Rejected commands retain
the draft; a lost reply is surfaced while status readback can still confirm the
applied rectangle. Coordinate comparison tolerates1e-9 normalized roundoff,
covering the2.1%/97.9% apply→modify→restore case without trapping Resume.

30 focused client/browser tests and frontend check/build pass. The native
synthetic workflow applies two numeric regions across pauses, checks status and
additional frames after each Resume, decodes source/prepared video and retains
Save/membership/Close checks. Backend per-frame color acceptance is recorded
separately in the region-engine checkpoint. Visual source preview/selection and
region setup before the first recorded frame remain open; the first interval
still records the full chosen monitor/window.
[Checkpoint](evidence/2026-10-12-screen-record-region-ui.json).

## Paused full-source snapshot service

The engine retains one bounded RGBA sample upstream of crop. On explicit request
while Paused, an independent appsrc pipeline scales that sample to at most640px
per axis and returns a JPEG no larger than2MiB. No per-frame JPEG encoding occurs.
The recording worker owns snapshot encoding; the job queue serializes it with
controls, validates the caller ID/phase and suppresses a cancelled response.
`screen_record_snapshot` returns raw binary through Tauri and is registered in
both builders and the main capability. There is no arbitrary screen/source access.

Tests decode full-source red/blue pixels after a narrow crop, assert recording
frame/time counters remain unchanged and verify1280×720→640×360 scaling. Owner
checks reject stale IDs, Recording and finalized states. Native synthetic WebKit
receives the raw JPEG, decodes320×240, reads a blue pixel outside the active red
crop and verifies the recording stays paused. Snapshot selection/preview is not
yet displayed by the production UI; this supplies its backend.

The2s deadline bounds EOS polling, not blocking plugin calls or teardown. Real
PipeWire buffer lifetime/format negotiation and source changes remain unqualified.
[Checkpoint](evidence/2026-10-12-screen-record-source-snapshot.json).

## ScreenRecord: визуальный выбор области на паузе, 2026-10-12

Полный JPEG кадр до crop показывается на паузе. Выделение мышью, перемещение
стрелками и числовые поля редактируют общий черновик; Apply подтверждается
readback, Resume блокируется при неприменённых изменениях. Ошибку snapshot
можно повторить; поздний ответ после Resume не возвращает старый preview.

Сохранённые результаты: 33 целевых browser/client теста, Svelte check без
ошибок/предупреждений, frontend/native build и isolated Tauri/WebKit workflow
прошли. Native проверяет preview, DOM keyboard event, red/blue region Apply,
Resume, source/review decode, Save/membership и normal Close. Это synthetic
source, не захват рабочего стола; OS keyboard/native pointer, speech/scaling,
настройка до записи и реальные portals остаются открытыми. Preview — уменьшенный
последний полный кадр до паузы, не live preview и не pixel-exact результат.
[Evidence](evidence/2026-10-12-screen-record-visual-region.json).

## ScreenRecord: подготовка до первого кадра — движок, 2026-10-12

`Recorder::prepare` открывает источник в `Setup`: bounded full-source snapshot
доступен, кадры не проходят в videorate/encoder и active time остаётся нулевым.
`set_region` сохраняет выбранную область; первая команда Resume устанавливает
crop/output geometry до открытия frame gate. Временные метки начинаются с
первого допущенного кадра. Источник и portal session не переоткрываются.
Обычный `Recorder::start` сохраняет прежнее немедленное начало записи.

Synthetic regression ждёт в Setup дольше односекундного recording budget,
подтверждает нулевые counters/bytes, выбирает красную область, начинает запись,
делает pause/resume и проверяет **все** decoded frames, включая первый.
Результат23–24 кадра/0.95–1.05с; ожидание и пауза в файл не входят. Отдельно
проверены отсутствие source frame, отказ Resume и joined cancellation.

86 library +2 binary tests прошли, один180s тест оставлен ignored; fmt,
strict Clippy (all targets, native-smoke) и rustdoc прошли. Реальные PipeWire
segments/timestamps, изменение геометрии и hardware не проверялись.
**Tauri owner и GUI ещё используют немедленный start**: подключение Setup,
начального выбора/preview и явной кнопки начала записи остаётся открытым.
[Evidence](evidence/2026-10-12-screen-record-setup-engine.json).

## ScreenRecord: область до начала записи — Tauri/GUI, 2026-10-12

Production и native-smoke owner открывают `Setup`. «Выбрать экран или окно»
открывает источник; preview/числовые поля/Apply доступны до явного «Начать
запись экрана». Неприменённый или неверный черновик блокирует Start. Backend
отклоняет Pause/Finish в Setup без уничтожения recorder; отмена joins worker.
Сохранение остаётся отдельным действием после review.

87 library +2 binary tests прошли (один180s ignored), fmt/strict Clippy/rustdoc,
Svelte check,35 browser/client tests и frontend/native build прошли. Изолированный
Tauri/WebKit с synthetic source подтвердил initial preview/region readback,
нулевые frame/time counters до Start, последующую запись/смену регионов,
review/re-edit/Save/membership и normal Close. Поздние snapshot ответы отдельно
проверены в Setup и Paused. Это не приёмка настоящего системного capture portal,
изменения geometry/segments, fractional scaling или физического LCD.
Предыдущие сообщения об отсутствии owner/GUI Setup являются историческими.
[Evidence](evidence/2026-10-12-screen-record-setup-ui.json).

## ScreenRecord: повтор последнего кадра при отсутствии обновлений, 2026-10-12

Live `imagefreeze` с `allow-replace` формирует24fps из последнего исходного
кадра. При responsive delivery gap видео продолжает показывать его; новые
кадры заменяют удерживаемый. Upstream EOS и Session::Closed остаются ошибками.
Output admission barrier исключает кадры после Pause, CFR timestamps основаны
на числе допущенных кадров. Finish отправляет EOS после imagefreeze, чтобы
генератор повторов не задерживал завершение MP4.

89 library +2 binary tests, fmt, strict Clippy и rustdoc прошли. Двухсекундный
stopped-source тест проверяет длительность и decoded held pixels; отдельный
тест подтверждает возврат меняющихся кадров, finite-source тест — ошибку EOS.
Temporal-cut oracle допускает разные индексы только для одинаковых decoded
pixels и требует минимум12 различных кадров на24, сохраняя проверку порядка.
Изолированный native WebKit workflow с synthetic source/MockBackend прошёл.

Полный continuous synthetic прогон:4320frames,180s,35335378bytes,180.90s elapsed.
Первый запуск завершён SIGTERM после90s по неизвестной причине и не засчитан;
после подтверждения остановки отдельный повтор прошёл. Это обновлённая
квалификация текущего timing path, не результат прежнего videorate pipeline.
Реальные portals/PipeWire, source resize/segments, native deadlock и LCD
остаются открытыми. [Evidence](evidence/2026-10-12-screen-record-held-frame.json).
