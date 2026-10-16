# GA2LCD Video Editor: контракт и оставшаяся работа

Baseline: Windows L-Connect3 **2.1.28.0**, Galahad II LCDV2. Сверка2026-10-12.
Reference вне repository: `reference-decompiled/2.1.28-2026-09-08/L-Connect Editor/`.
Пути ниже относительно `LConnectEditor/Views/Tools/`.

## VideoCut: подтверждено исходниками

`VideoCutTool.cs:559–587,601–654`: record/pause собирает ordered RecordInfo
Start/End по позиции source player. Несколько intervals допускаются; timer
подтверждает результат после суммарных180000ms. Это ограничение интерактивной
записи фрагментов, не всех вариантов импорта/AutoFill.

`createCroppedVideo:865–928`: Seek(Start)/EndSeek(End), AdjustedResize/Crop,
DisableChannel(Audio). GA2LCD intermediate использует VP9. Linux хранит
prepared H.26424fps для своего transport; Windows archive interoperability
этим не заявлена. `createConcatVideo:1008–1072` соединяет список выбранных
записей; не сортировать intervals по времени и не удалять повторы автоматически.

`playSpeed_PropertyChanged:297–305` меняет скорость mediaPlayer. Это не основание
ускорять экспорт. VideoEdit использует другой SDK Convert/effects pipeline;
его brightness/contrast0..255 не равны image-editor controls.

## VideoEdit: диапазон времени, а не сетка кадров

Дополнительная сверка `VideoEditTool.cs` и `TimeRangeSlider.cs` в установленном
baseline2.1.28 уточняет прежнее название «frame-aligned selection»:

- `VideoEditTool.cs:204–209`: tick=1/sourceFPS; maximum=duration−tick;
  начальный диапазон0..maximum. Это VideoEdit, не общий импорт/VideoCut/AutoFill.
- `TimeRangeSlider.cs:219–244`: Value только ограничен min/max; Start ограничен
  Stop−tick, Stop ограничен Start+tick/max. Нет округления до номера кадра.
- `TimeRangeSlider.cs:306–321`: drag линейно преобразует координату в double
  времени и сохраняет её без snapping. TickFrequency не означает frame grid.
- `VideoEditTool.cs:392–398`: экспорт отклоняет диапазон короче2/sourceFPS,
  хотя сам slider позволяет расстояние в один tick.
- `VideoEditTool.cs:504–507`: Start/Stop передаются `VideoSource`/`AudioSource`
  без добавления tick к Stop. Из этого нельзя вывести включительность последнего
  декодированного кадра: она зависит от SDK pipeline и требует qualification.
- `VideoEditTool.cs:270–272,303–370`: назад — timer вычитает2ticks за tick,
  обычное воспроизведение1×, ускоренное2×; Start/End останавливают player и
  переходят к min/max. Это controls просмотра, не скорость экспорта.

Не округлять существующие VideoCut milliseconds до24fps или предполагаемого
source frame grid. Для точного VideoEdit UX ещё нужны native sourceFPS/duration
metadata и отдельная single-range selection с указанными guards. Не применять
minimum2ticks/maximum duration−tick к AutoFill и всему импорту.

Linux regression использует реальный MP430000/1001FPS со start_time0 и5s:
диапазон1101..1600ms выбирает12 синих кадров относительно начала исходника.
Проверка подтверждает normalisation этой пары timestamp fixtures; не доказывает
Windows SDK frame inclusion, VFR/negative PTS или все container edit lists.
[Evidence](evidence/2026-10-12-video-time-boundaries.json).

## Native metadata для VideoEdit

`media_prepare::inspect_file` использует тот же bounded/cancellable ffprobe,
что и preparation. Возвращает dimensions, durationSeconds (stream предпочитается
container), averageFrameRate и nominalFrameRate как numerator/denominator.
Отсутствующие/нулевые/некорректные rates возвращаются как null; output24fps не
подставляется. Средняя и номинальная частоты не объединяются автоматически:
их совпадение само по себе не доказывает CFR или соответствие Windows SDK FPS.

Функция рассчитана на owned staged input с удержанием admission/lease до reap.
Она пока не опубликована как Tauri command и не принимает путь из WebView.
Следующий шаг — metadata state/readback для upload и managed source, затем
single-range UI и backend guards. Неизвестный FPS должен запрещать именно
VideoEdit range, сохраняя независимые VideoCut/AutoFill workflows.

Real MP4 regression подтверждает обе rate fractions30000/1001 и duration2.002s
при start_time0/+5s, cancellation до probe и camelCase serialisation.
[Checkpoint](evidence/2026-10-12-video-source-metadata.json).

## Metadata readback сохранённого ролика

`media_source_status.metadata` становится доступным атомарно с phase=ready.
Download → remux → inspect выполняется с сохранением одного владельца files;
probe входит в существующий blocking worker и его cancellation/reap contract.
Начальное downloading/remuxing состояние имеет metadata=null; cancel очищает
metadata до публикации terminal state, новая job начинает без старых данных.
Ошибка probe не выдаёт ready source.

Значения относятся к **редактируемому сохранённому исходнику**: library хранит
prepared H26424fps, remux не возвращает FPS исходного файла до импорта.
Не выдавать metadata галереи за provenance оригинала. Fixture из12frames
подтверждает480×480, duration0.5s, rational24/1 и отсутствие metadata после
cancel/failed next source. [Checkpoint](evidence/2026-10-12-gallery-source-metadata.json).

Metadata для fresh upload и range UI пока не подключены. Выдача уже готовых
метаданных галереи не означает завершение VideoEdit single-range workflow.

## Fresh upload: inspect → source_ready → prepare

`MediaImports::inspect` проверяет полностью загруженный исходник без encoding
или publication. Worker забирает Upload под lock; append/prepare/новый import
во время inspection запрещены. Успех возвращает Upload в задание и публикует
phase=source_ready + metadata. Повтор inspect готового задания возвращает
readback без повторного запуска ffprobe. Повреждённый input освобождается до Failed.

В первоначальном state checkpoint `configure_source` менял параметры только в source_ready; validate выполнялся
до изменения. Подготовка забирает options вместе с input атомарно и очищает
metadata. Отмена inspection сохраняет admission до возврата worker, затем удаляет
input; даже успешный поздний probe не публикует метаданные после cancel.
SourceReady cancel/shutdown удаляет сохранённый input и metadata сразу.

Real upload regression: inspect0.25s/24fps → configure brightness255 → review
даёт6 белых кадров. Проверены partial upload rejection, идемпотентный readback,
неизменяемость active options, отмена blocked inspector, damaged input cleanup,
SourceReady shutdown. [Checkpoint](evidence/2026-10-12-upload-source-inspection.json).

Tauri commands подключены последующим checkpoint ниже; UI workflow ещё открыт. Обычный import
продолжает прежний путь upload → prepare. Отдельный VideoEdit range workflow и
его limits остаются открытыми.

## Tauri: inspect и подготовка с окончательными параметрами

`media_inspect({id})` запускает owned task и возвращает Status; при потере ответа
результат доступен через `media_status`. `media_prepare_inspected({id,fit,cut?,adjustments?})`
принимает только SourceReady, валидирует параметры, атомарно забирает input с
окончательными options и подготавливает review. Отдельный `configure_source`
заменён этим атомарным действием; отдельная конфигурационная команда не открыта.

Конкурентный/повторный запуск не меняет параметры уже работающего worker.
Невалидный cut сохраняет SourceReady/input/metadata для исправления. Этот путь
всегда заканчивается review, без неявной публикации. Зарегистрирован в production
и native-smoke builders, build ACL и main capability. Ни одна команда не
принимает filesystem path. UI распознаёт status source_ready; UI выбора single-range ещё предстоит подключить.

[Checkpoint](evidence/2026-10-12-inspected-media-commands.json): real inspected
upload с final brightness255 →6 white frames, competing request/invalid range/
uninspected rejection и существующие lifecycle tests. Tauri runtime/native
window вызовы пока не квалифицированы.

## VideoEdit single-range: native guards и кодирование

Optional `range:{startSeconds,endSeconds}` добавлен в `media_prepare_inspected`
и `media_source_prepare`. Одновременные `range` и `cut` запрещены до admission.
Диапазон непрерывный f64, внутри0..3600s; кодирование передаёт FFmpeg секунды
с microsecond precision. Старый VideoCut сохраняет milliseconds и180s recording
cap; VideoEdit использует обычный1hour/256MiB preparation budget.

`VideoEditBounds` требует duration и FPS; Linux выбирает averageFrameRate,
если она доступна, иначе nominalFrameRate. Нет fallback на output24fps.
Maximum=duration−1/FPS, minimum span=2/FPS; слишком короткий source отвергается.
Это явная Linux policy; соответствие выбора FPS Windows SDK для VFR ещё открыто.
Floating arithmetic tolerance1e−9s не разрешает обход guard на1ms/frame.

Inspected upload проверяет range против удерживаемых metadata до worker admission:
ошибка сохраняет SourceReady/input. Кодирование дополнительно делает trusted
native probe и повторяет source guards — FPS/duration из UI не принимаются.
Gallery source проверяется в worker по leased prepared source. Результат проходит
тот же effects→resize→H264→review path. Frame inclusion SDK и FFmpeg microseconds
против WPF TimeSpan100ns остаются qualification, а не заявленной идентичностью.

[Checkpoint](evidence/2026-10-12-video-edit-range.json): source24/29.97 guard tests,
>180s contract без VideoCut cap, unknown/invalid/short source rejection, mutual
exclusion до admission, real inspected0..0.2s с brightness255 даёт5 white frames;
range1.101001..1.600001s с offset0/+5s даёт12 blue frames.

### VideoEdit range UI

UI подключён к native metadata для fresh upload и managed gallery source.
Новый файл сначала загружается и проверяется без кодирования; затем доступны
начало/конец, sliders и отметки по текущему времени player без округления до ms.
Подготовка → review → явное Save. VideoCut и VideoEdit range взаимоисключающие;
цветокоррекция доступна вместе с диапазоном. Unknown FPS/слишком короткий файл
блокирует подготовку, отмена освобождает upload. Lost inspect reply читается
через status без повторного запуска, close отменяет поздний результат.
При смене режима на VideoEdit неподготовленный local draft очищается, чтобы
не использовать browser duration вместо native FPS.
[Checkpoint](evidence/2026-10-12-video-range-ui.json): check/build и41 browser
checks прошли, включая fresh/gallery/fractional/unknown FPS/cancel/lost reply.
Browser Tauri mock не подтверждает native WebKitGTK или Windows SDK parity.

## Реализованный выбор фрагментов

Desktop «Импорт ролика» предлагает необязательный список начала/конца в
секундах. Порядок списка — порядок playback; при выключении выбора сохраняется
полный прежний import. UI переводит значения в целые миллисекунды. Backend
проверяет1..64 intervals, 0≤start<end≤1hour, суммарно≤180s. 64 — локальный
resource limit; Windows count limit не установлен. Unknown fields отвергаются.

`media_begin` принимает optional cut; старые callers без него продолжают
работать. Validation выполняется до создания upload/job. Приватный исходник
обрабатывается sequentially: один FFmpeg decoder, output Seek/Duration и
существующие fit/H.264 validation. Video stream duration предпочтительнее
container duration; конец за пределами известной длительности отвергается, кроме допуска1ms
на округление movie/browser duration. FFmpeg всё равно заканчивает на stream EOF.
Неизвестная длительность при cut — ошибка до encoding.

Каждый результат начинается IDR; проверенные AU копируются в один prepared
file без материализации ролика в памяти. Общий encoded budget256MiB, frame
budget180*24+64 учитывает округление каждого интервала до24fps. Temp owners
живут до cleanup/reap; отмена останавливает подготовку, публикация использует
существующий upload/receipt workflow. Видео не запускается автоматически.

UI блокирует настройки во время импорта, сохраняет validation errors и
возвращает focus на соседний interval после удаления. UI-only стабильные IDs
не отправляются в IPC. Сохранённый файл имеет имя исходного import.

## Просмотр исходника перед импортом

При включённом выборе фрагментов file chooser открывает локальный silent video
preview без отправки daemon. Scrubbing и кнопки начала/конца используют текущую
позицию player; во время seek маркеры недоступны. Upload начинается отдельной
кнопкой. Смена файла/закрытие освобождают object URL и player; неподдержанный
кодек оставляет ручной ввод времени. Native CSP разрешает только blob media.

[Checkpoint](evidence/2026-10-12-video-source-preview.json): 10 browser tests,
реальные красный/синий кадры после seek, границы между timeupdate events,
cleanup/error fallback и отсутствие upload до explicit submit. Проверено в
headless Chromium с IPC mock; codec availability в WebKitGTK ещё не подтверждена.
Это preview исходника, не собранного результата после crop/concat.

## Проверено программно

- Реальный3s MP4 с red/blue/green → green0.5s, red0.5s, green0.5s: ровно36
  кадров после полного decode joined H.264; правильные цвета и IDR на стыках.
- Invalid/overflow/reversed/over180s ranges, overrun source до encoding;
  отмена на validation, cleanup prepared file и сохранность source.
- Browser invalid/empty/total limits, ms payload/order, active controls,
  cancel/обычный import и keyboard focus после удаления последнего interval.
- [Checkpoint/provenance](evidence/2026-10-12-video-cut.json).

## Обрезка каждого интервала — backend

`VideoCutTool.cs:591–605,824–842` сохраняет capture rectangles при старте
каждой записи и передаёт соответствующий rectangle в подготовку фрагмента.
Linux `Segment.crop` теперь принимает optional `{x,y,width,height}` в долях
отображаемого (после autorotation) исходника. Старый payload без crop сохранён.
Границы finite/nonempty/in-bounds проверяются до upload admission/encoding.
FFmpeg crop выполняется до существующего fit; RGB sampling избегает округления
координат по chroma grid. Координаты округляются вниз до пикселя, размер —
минимум один пиксель. Это собственный координатный контракт Linux, не заявление
о pixel-exact Windows AdjustedResize/crop.

Реальный split red/blue source с разным crop для двух одинаковых временных
интервалов даёт 12 декодированных blue/red кадров. Проверены invalid rectangles
до запуска decoder и legacy JSON без crop. GUI selection/local frame preview подключены следующим срезом ниже;
rotated/SAR/odd-dimension fixtures и Windows rounding остаются.
[Evidence](evidence/2026-10-12-video-crop-backend.json).

## Выбор области в интерфейсе

У каждого интервала есть отдельные поля x/y/width/height в процентах. Кнопка
выбора области связывает source canvas с конкретным стабильным ID интервала;
удаление соседней строки не меняет выбранный объект. Указатель задаёт два угла,
стрелки перемещают область; сброс «Весь кадр» удаляет optional crop из payload
и возвращает focus на кнопку выбора области. Пустые/out-of-bounds размеры
блокируют import и убирают недостоверный preview. Во время upload поля disabled.

Локальный canvas показывает текущий декодированный кадр после crop и fit.
Он обновляется при смене кадра/координат/contain-cover. Браузерный scaling не
считается pixel-exact FFmpeg output: rounding, SAR и encoding должны проверяться
на prepared result. Это ещё не проигрыватель склеенного результата.
[Проверки UI](evidence/2026-10-12-video-crop-ui.json).

## Prepared-result preview — backend ownership

`PreparedFile.preview` remux-ит готовый H.264 в MP4 с faststart без повторного
encoding. Проверяется число frames; MP4 имеет отдельного temporary owner.
Его byte budget = encoded payload + 64 bytes/frame + 1 MiB metadata. Remux
отменяемый, с child reap и timeout; range reader отдаёт не более 256 KiB.
Весь ролик не материализуется в памяти и пути файлов наружу не принимаются.

`MediaImports.prepare_review` готовит результат и удерживает пару H.264/MP4
в ReviewReady. `take_review` однократно переводит исходный prepared owner в
Transferring; cancel/shutdown удаляют staging до terminal status. Обычный
prepare→publish не изменён. Путь подключён к Tauri commands/media URL и UI следующим срезом ниже.
Обычный import без выбора фрагментов сохраняет прежнее поведение.

Real codec test сравнивает decoded frame hashes H.264 и MP4 для разных crop
двух интервалов. Проверены chunk/range reconstruction, invalid ranges, cleanup,
legacy path, blocked second job, duplicate confirmation и shutdown в ReviewReady.
[Evidence](evidence/2026-10-12-video-prepared-preview-backend.json).

## Prepared-result player и подтверждение

При выборе фрагментов кнопка «Подготовить выбранные фрагменты» загружает
исходник и вызывает `media_start(review=true)`. В ReviewReady показывается
seekable MP4; crop/interval controls заблокированы до подтверждения или отмены.
`media_confirm` забирает prepared owner и публикует его в отдельной owned task;
потеря invoke future не оставляет Transferring без владельца. Duplicate confirm
отвергается atomic take; UI не повторяет подтверждение автоматически.
Status readback сообщает terminal outcome даже при потерянном ответе.

`media-preview://localhost/<job-id>` (на Windows HTTP localhost alias) принимает
только 32 hex ID текущей ReviewReady и только main webview. Filesystem paths не
принимаются. GET/HEAD, single byte ranges, suffix/open ranges, 404/405/416 и
no-store; ответы до256KiB. Большой/open GET разбивается на206 responses;
Chromium playback/seek с несколькими маленькими ranges проверен, WebKitGTK
ещё требует native acceptance. После confirm/cancel URL недоступен.
CSP разрешает только blob и конкретный media-preview protocol/localhost alias.

Компонент снимает src/останавливает video при уходе из review. Ошибка codec/
URL не запускает повторную подготовку: можно явно сохранить validated result
или отменить. Закрытие панели отправляет cancel; публикация после commit
сохраняет прежний receipt/readback контракт.
[Checkpoint](evidence/2026-10-12-video-review-integration.json).

## Record/pause выбора фрагментов

Режим «Запись при просмотре» хранит отдельный список; ручной draft не теряется
при переключении. «Записать фрагмент» запоминает source position и копию crop,
затем запускает player. «Пауза записи», pause event и EOF завершают интервал.
Повторная запись добавляется в конец. Это запись интервалов исходника,
не screenshot/record рабочего стола (LCD-011 остаётся открытым).

Controls/seek/crop/file choice блокируются на время записи; unexpected seek,
decode failure и закрытие отбрасывают только незавершённый интервал. Ошибка
play() возвращает управление без фиктивного пустого фрагмента. Общий source-time
budget180s и local64interval limit сохраняются. Достижение180s автоматически
запускает preparation, как Windows timer→confirm, но публикация по-прежнему
требует review confirmation. Скорость просмотра не меняет export timeline.

EOF округляется вниз до целого миллисекундного маркера. Backend допускает
1ms разницы между rounded MP4 movie/browser duration и video stream duration:
25/24 seconds → UI1.042s даёт все25frames; end1.044s отвергается до encode.
Полная VFR/nonzero-PTS qualification ещё открыта.

Browser проверяет pause→seek→record→EOF с двумя crop snapshots, сохранность
ручного draft, rejected play(), unexpected seek, удаление последней записи с
focus recovery; реальный200s MP4 при16× playback останавливает selection ровно
на180000ms и запускает подготовку. Счётчик времени не помещён в live region;
объявляются изменения состояния.
[Checkpoint](evidence/2026-10-12-video-record-pause.json).

## Подготовка gallery re-edit: конечное чтение сохранённого ролика

Windows `Gallery.cs:480–539` открывает сохранённый файл через VideoEdit и
заменяет выбранную запись. В Linux старый asset должен оставаться доступным
другим темам/профилям: будущая замена membership требует revision check.

Daemon `MediaExports` предоставляет metadata и конечный stream исходных H.264
access units для legacy/v2 storage. Два shared admission slots, один blocking
read на запрос следующего кадра; descriptor и pin остаются у worker даже при
отмене ожидающего future. Последний кадр, ошибка и drop освобождают владельца;
повторный poll после EOF безопасен. Builtin и некорректные ключи отвергаются.

IPC и desktop download/staging/remux primitives подключены последующими
checkpoints; state/commands/source preview подключены последующим checkpoint;
gallery cut/crop/fit UI и save replacement подключены последующим checkpoint. Тестовый поток586units проверяет порядок/размер и позднее
повреждение, но не доказывает декодирование видео или работу LCD.
[Evidence](evidence/2026-10-12-gallery-video-export.json).

## IPC экспорт исходника для re-edit

`open_media_export({key, stream_id})` принимает32lowercasehex ID; dedicated
connection допускает только один export attempt. Перед вызовом клиент подписывается
на `media_export`. Response содержит stream_id и info: fps/frames/encoded_bytes.
Затем приходят typed Unit с последовательным index, Complete либо Failed.
Metadata response всегда записывается перед первым event. Consumer обязан
проверять ID, индексы, totals и Complete; prefix не является готовым роликом.

Pump находится в connection JoinSet; disconnect отменяет opener/pump. Blocking
reader сохраняет собственный pin/permit до завершения операции. Отдельное
соединение не даёт заполненному notification channel блокировать обычный UI RPC.
Два shared daemon slots ограничивают источники между соединениями; socket
backpressure ограничивает выдачу кадров без накопления всего видео в памяти.

MockBackend + memory transport:586units проходят настоящий IpcClient, slow wire
удерживает deletion pin, disconnect освобождает его, late truncation даёт Failed
без Complete. Invalid stream ID и повторный export на том же connection отвергнуты.
[Evidence](evidence/2026-10-12-gallery-video-export-ipc.json).

## Desktop приём исходника

`media_download::download` принимает собственный dedicated IpcClient и пишет
каждый AU на blocking worker во временный H.264. Проверяет stream ID,24fps,
frame/byte budgets, contiguous indices, Annex-B/keyframe framing и точные totals.
Только Complete с совпадающими totals возвращает PreparedFile; Failed,
неполный Complete, неверный ID/index/NAL и disconnect не выдают partial source.
Open timeout10s, ожидание следующего события30s. Future cancellation закрывает
client; незавершённый blocking write удерживает temp owner до своего выхода.

Исправлен IpcClient Drop: reader/writer tasks больше не удерживают соединение
после удаления клиента, включая reader, заблокированный на notification queue.
Это не отменяет уже принятые daemon mutations; download сам ничего не публикует.

PreparedFile использует существующий stream-copy MP4 preview; real12frame source
прошёл IPC-shaped transfer с побайтовым сравнением AU и remux frame-count check.
Managed job/cancel/readback, source range protocol и native commands добавлены
следующим checkpoint. Галерея ещё не вызывает download; revision-checked
membership replacement после Save пока не подключён.
[Checkpoint](evidence/2026-10-12-gallery-video-download.json).

## Managed source и native entry points

`MediaSources` владеет одним исходником: Downloading→Remuxing→Ready либо
Failed/Cancelled. Cancel во время работы выставляет Cancelling и сохраняет
admission до завершения owned worker; новый Start отвергается. Ready cancel
удаляет source/preview до terminal status. Stale ID не отменяет новую задачу.
Worker panic становится Failed. Shutdown закрывает admission и ждёт worker.

`media_source_start/status/cancel` зарегистрированы в production/native-smoke
handlers и capability main. Start использует только native default socket path;
не принимает filesystem path от webview. Запуск owned task переживает потерю
invoke reply, status возвращает текущий ID/key/phase/error. Download cancellation
прерывает network waits, но дожидается blocking writes перед terminal status.

`media-source://localhost/<id>` использует общий ограниченный range responder:
main window only, strict opaque ID, GET/HEAD, capped206, no-store,404 после cancel.
CSP содержит этот protocol и точные localhost aliases. Close handler ожидает
source shutdown вместе с imports/thumbnails. Native runtime/CSP/codec/close
ещё не квалифицированы; native-smoke в этом срезе только скомпилирован.
[Checkpoint](evidence/2026-10-12-gallery-source-state.json).

## Исходник галереи → общая подготовка/review

`media_source_prepare(sourceId,id,name,fit,cut)` получает native lease готового
MP4 и вызывает общий prepare_review в owned task. Полный ролик не копируется
через webview. Name должен оканчиваться на.mp4; clip/crop validation и выбранный
fit сохраняются. Результат использует те же media_status/cancel/confirm и
media-preview URL, что обычный импорт. Новый publish path не добавлен.

Preview cancellation отзывает source URL сразу, но принятый редактор удерживает
MP4 lease до завершения подготовки/отмены. Это независимая задача: для отмены
подготовки нужен media_cancel; close handler уже отменяет обе. После завершения
подготовки input lease освобождается, готовый результат остаётся для review.

Внешний upload по-прежнему ограничен256MiB. Native remux source допускает
дополнительный bounded MP4 overhead: encoded256MiB +64bytes/frame +1MiB;
output frame/byte budgets не расширены. Это позволяет re-edit видео у предела
encoded storage без отказа только из-за добавленных MP4 sample tables.

Real12frame test:250..500ms→6frames; source URL отозван после admission,
input остаётся до подготовки, затем удаляется; review отдаётся только один раз.
Gallery кнопка/cut/crop/fit controls и revision-checked replacement после Save
подключены последующим checkpoint.
[Checkpoint](evidence/2026-10-12-gallery-source-prepare.json).

## Gallery re-edit UI: cut/crop/fit

«Редактировать выбранный ролик» доступно для пользовательского ролика выбранной
коллекции. Builtin защищён. Перед открытием снимается независимая копия состава
коллекции/revision; legacy membership мигрирует. Source download/status показывает
готовность или ошибку, внешний source URL подключён к общему VideoSourcePreview.
CORS header в native responder и anonymous video разрешают canvas crop preview;
main-window/opaque-ID checks сохраняются. Native WebKitGTK это ещё не подтверждает.

MediaImport повторно используется для ручных/record-pause intervals, crop и fit,
но вызывает media_source_prepare вместо web upload. После prepared review и
явного confirm новый asset заменяет только video entry исходной коллекции через
expected_revision. Старый asset, активная тема/ролик и профили не переписываются.
Конфликт/reply loss не вызывает повторную мутацию: новый файл остаётся в каталоге,
UI сообщает о необходимости проверки коллекции. Встроенный ролик нельзя редактировать.

Busy import запрещает открыть editor. На время membership replacement controls
заблокированы. Focus переходит в heading редактора и возвращается к кнопке после
закрытия. Завершённый импорт при повторном mount не стирает replacement notice.

Browser: реальный source MP4 под app CSP, canvas pixels, cut+crop→review→confirm,
правильная membership replacement без delete/Apply и конфликт с сохранностью
параллельного изменения.32 tests включают import/library/gallery/delete/collections.
Это закрывает маршрут gallery re-edit с существующими cut/crop/fit controls;
не закрывает все Windows VideoEdit controls. Native/hardware остаются отдельно.
[Checkpoint](evidence/2026-10-12-gallery-video-editor-ui.json).

## AutoFill: весь кадр растягивается до экрана

Windows `MainWindow.cs:getAutoFillVideoResults:604–703,resize:873–925`:
GA2LCD делает Scale(width,height), SetSAR(1), без preserve-aspect/crop/trim.
Для video отключается audio и используется VP9; GIF остаётся GIF (GA2LCD не в
ветке конвертации GIF других SKU). Duration используется для progress. Здесь
нет180s record budget; проверить общие admission guards отдельно.

Linux Fit::Stretch / «AutoFill — растянуть весь кадр» использует480×480 scale,
SAR1 и существующий H26424fps pipeline. Без выбора фрагментов обрабатывается
весь разрешённый исходник; при cut растягивается выбранная область/интервал.
Browser canvas соответствует этому placement; предупреждает изменение пропорций.
Contain/cover сохраняются отдельными режимами. Prepared format Linux не равен
Windows gallery archive; timing/pixel/native qualification остаётся открытой.

MP4/GIF тесты с цветными краями отличают все три режима: stretch сохраняет края
и заполняет высоту; cover отрезает полосы; contain даёт чёрные поля.
[Checkpoint](evidence/2026-10-12-video-autofill.json).

### Дополнительные подтверждения VideoEdit

`VideoEditTool.cs:205–209` задаёт tick=1/sourceFPS и selectionStop=duration−tick.
`:392–398` отклоняет выбор короче двух ticks. `:499–529` применяет SDK Contrast,
затем Lightness; промежуточный output25fps и Video_Rotation=RotateNone.
GA2LCD cover затем кодируется VP9. Playback1×/2× меняет player, не export.
По этому пути rotation/flip не следует объявлять подтверждёнными требованиями
GA2LCD. Потребуется отдельная трассировка, если они найдутся в UI. Не переносить
формулы image filters на SDK video effects без доказательств.

## VideoEdit brightness/contrast: восстановлено вычислительное ядро

XAML `videoedittool.xaml:49–53`: оба controls0..255, default0. SDK export
использует Contrast→Lightness. Managed VideoEffect classes хранят Value и
ValueStop; default ValueStop=0 даёт постоянное значение, без интерполяции.
`VideoEditCore.VideoFrameBufferCB24SG` идёт через MFPVideoEffectsProcessor,
Processors и FastImageProcessing в `VisioForge_MFP64.dll`. Сам DirectShow
Video Effects Pro.ax не является найденным вычислительным endpoint этого пути.

На SIMD-ветке для канала x и значений c,b:

- f = f32((c+255)*259) / f32((259−c)*255);
- y = clamp(round_ties_even(f32(f32((x−128)*f)+128)),0,255);
- result = clamp(round_ties_even(f32(y+f32((255−y)*f32(b/255)))),0,255).

Между effects есть byte quantization. Ползунок brightness приближает канал к
белому, а не просто прибавляет постоянное число. Операции не сливать в FMA.
Scalar/tail branches имеют другое округление; их полная квалификация открыта.

Изолированный x86-64 emulator выполнил два exports над RGB24 width256,height1,
для всех256 amounts и всех256 gray inputs:131072 values. Все вызовы проверены
на возврат до instruction cap. Rust channel_lut совпадает со всем набором.
FFmpeg `format=rgb24,lutrgb` строит точную таблицу из balanced expressions один
раз; raw RGB test пяти пар параметров совпадает побайтово. Vendor DLL/code не
включены в проект; fixture содержит только численные outputs.

`prepare_file_edited` подключает Contrast→Lightness до crop/resize для целого
ролика и каждого интервала. Нулевые параметры сохраняют прежний FFmpeg path;
real codec тест проверяет SDK oracle, padding и neutral byte equality.
[Checkpoint](evidence/2026-10-12-video-effects-preparation.json).
Native admission/DTO подключены: оба commands принимают optional `adjustments`,
фиксируют параметры в задании и передают их общему worker. Отсутствующее поле
означает neutral path; недопустимые параметры отвергаются до admission.
[Checkpoint](evidence/2026-10-12-video-effects-admission.json).
Preview и controls подключены следующим checkpoint; остаются
Windows colorspace/frame/codec и native qualification.
[Checkpoint](evidence/2026-10-12-video-adjustments-kernel.json).

## VideoEdit controls и локальный preview

Цветокоррекция доступна для целого файла и cut/re-edit. Переключатель сохраняет
draft значений, отключение исключает effects из запроса. Sliders/numeric inputs
0..255/default0 и Reset блокируются при работе/recording. Неполные, дробные или
выходящие за пределы значения не допускают подготовки. Настройки общие для
всех интервалов; изменение во время записи запрещено.

Отдельный canvas применяет SDK LUT на RGB исходного размера перед crop/fit;
исходный player и прямоугольник выбора остаются без изменения цвета. JS f32
и nearest-even проверены по тем же131072 oracle values и composed tables.
Black padding не осветляется. Ошибка canvas отображается явно; окончательный
закодированный результат просматривается перед Save. Browser color decoding,
resampling и H264 могут отличаться от source canvas; pixel-perfect native/Windows
соответствие не заявляется. Максимальный исходный размер4096×4096 соответствует
preparation budget; время/RSS такого preview ещё квалифицировать.

Source player preload=auto нужен для первого canvas кадра без запуска playback.
[Checkpoint](evidence/2026-10-12-video-effects-ui.json).

## Остатки до полного Video Editor

- [ ] Native gallery: color canvas, pointer/keyboard cut/crop interaction
  и автоматический лимит180с записи,
  close/device switch, lost source/replacement reply, keyboard/speech/scaling.
  Source/review decode, seek, single-range Save/replacement и возврат фокуса приняты.
- [ ] Windows SDK end semantics/FPS policy и color/codec для реализованного VideoEdit continuous single-range UI/native sourceFPS guards
  qualification: сравнить frames; brightness/contrast UI/preview уже реализованы.
  Применимость других controls устанавливать отдельно.
- [ ] AutoFill qualification: Windows/native placement, GIF timing, общие
  admission guards и ресурсы длинного видео; stretch geometry/UI реализованы.
- [ ] VFR, nonzero timestamps, GIF timing, very short intervals/frame boundaries;
  audio longer than video, corrupt/unknown duration и отмена между intervals.
- [ ] Long-source late/repeated intervals: output seeking повторно декодирует
  prefix, измерить время/ресурсы. Одновременно работает один decoder; короткий
  тест не доказывает приемлемую latency длинного source.
- [ ] Native prepared-result дополнительные codecs и confirm/lost reply/close;
  chooser/cut/record-pause/error/cancel/readback/keyboard/AT-SPI/scaling и speech;
  physical LCD/loop/replace/replay — позже, с финальным LocalUI.

Числовая нарезка — завершённый software slice, не весь LCD-009 и не полная
эквивалентность Windows Editor. Source limits1hour/256MiB — Linux budgets.


## Native VideoEdit: loopback playback

Custom scheme prepared video не декодировался в WebKitGTK2.54.1 (error4),
несмотря на успешный206 responder. История —
[failed run](evidence/2026-10-12-native-video-edit-failure.json).
Текущий UI получает `media_playback_url` для managed review/source ID:
loopback127.0.0.1, случайный UUIDv4 token, exact Host и проверка Origin,
4 клиента,8KiB headers,5s network deadlines. Путей файлов API не принимает.
HTTP отдаёт весь запрошенный диапазон блоками<=256KiB; revoke прекращает
дальнейшее чтение. При закрытии listener и clients завершаются вместе с owners.
Старый custom-protocol responder сохранён; текущие player URLs используют HTTP.

[Native checkpoint](evidence/2026-10-12-native-video-edit-http.json): GTK chooser,
source Play, native FPS/duration, fractional range, brightness255, prepared
review decode/canvas → Save проходят.12 сохранённых кадров и белый центр
проверены отдельным FFmpeg decode. Полный native media harness прошёл.
52Rust/41browser/check/build/Clippy/docs также прошли. Xvfb/X11/software decoder
и MockBackend; это не hardware/WPF или полная native editor qualification.
[Gallery checkpoint](evidence/2026-10-12-native-gallery-video-edit.json) закрывает
managed source/review HTTP decode и seek, диапазон0.1…0.35, explicit Save,
замену membership с сохранением исходника и возврат фокуса. Отдельный FFmpeg
decode подтверждает шесть кадров и красный центр; 42 browser tests прошли.
Record/Pause → два интервала → review → cancel без публикации
[принят native](evidence/2026-10-12-native-record-pause.json).
[Следующий checkpoint](evidence/2026-10-12-native-record-save.json) проверяет Save
нарезки и автоматический stop по концу исходника: сохранённый файл декодирован
целиком, сначала идут синие кадры левой половины, затем красные правой.
Остаются лимит180с, pointer/keyboard crop interaction, device/lost reply races, большие native потоки,
Wayland и Windows semantics остаются в acceptance checklist.
