# Persistent media v2: контейнер и границы интеграции

2026-10-12. Первая реализация находится в
`crates/llcontrol-daemon/src/media_file.rs`. Контейнер подключён к MediaAssetStore list/open/delete и сохранению
media-профилей. LCD actor и manager replay теперь используют reader worker.
Native/GUI import и thumbnail подключены к file preparation/chunked publication.
VP9 586frames импортирован, воспроизведён с MockBackend и декодирован после
restart. Бюджеты GUI —1hour/256MiB; diagnostic clip limits не изменены.
Native publication faults, accessibility и physical acceptance остаются.

## Формат

Один regular file:8bytes ASCII `LCMEDIA2`,4bytes little-endian длины JSON index,
затем UTF-8 JSON index и подряд Annex-B access units без padding/trailing bytes.
Index:version2,name,fps,encoded_bytes,units[{offset,length,keyframe}]. Offset
относится к началу payload. Неизвестные поля/версии отклоняются. Первый AU имеет
SPS/PPS/IDR. Непрерывность offsets и точное соответствие размера файла обязательны.

Ресурсные бюджеты этой реализации:24fps,до86400frames,256MiB payload,
128KiB на AU,8MiB JSON index,256UTF-8 bytes имени. Это Linux budgets, не найденные
Windows ограничения. Проверка Annex-B/type markers не является полноценным
H.264 decoder/conformance verification. Encoder/qualification остаются отдельными.

Writer хранит payload в анонимном временном файле и в памяти только bounded index;
`append` проверяет AU, размер и frame budget. I/O failure отравляет writer.
`finish` проверяет индекс, копирует ограниченное число bytes во второй temp file,
fsync→persist_noclobber→directory fsync. Существующий destination не заменяется.
Drop до публикации удаляет staging. Ошибка после commit/fsync может быть
неоднозначной: caller обязан делать readback, не публиковать автоматически повторно.

Reader владеет одним descriptor и index, читает один AU на запрос и проверяет
его framing/keyframe marker перед возвратом. Open проверяет regular file, header,
полный index/length и первый AU; повреждение последующего AU обнаруживается при
его чтении. Произвольный read_unit не означает decoder-safe seek: playback должен
идти последовательно от первого AU/исследованной keyframe границы.

Все операции blocking. Reader/writer должны принадлежать blocking worker;
в Tokio/HID actor нельзя добавлять синхронные read/seek. Диагностический
`H264Clip` сохраняет прежние240AU/1MiB; общая проверка одного AU выделена в IPC
без ослабления clip validation. Старый JSON recordv1 пока обслуживает существующий
MediaAssetStore. `.json`v1 и `.lcm`v2 сосуществуют без миграции; дубли ключа
между форматами отклоняются. Legacy byte-array load не материализует v2.

## Проверено

- Синтетический VP9-in-MP4480×480/24fps/586frames → FFmpeg H.264 → append/file →
  покадровое восстановление → decoder count586; payload больше1MiB.
- Drop staging, no-clobber, чтение кадра и возврат к первому AU для loop.
- Version/unknown fields, gaps/overlaps, oversized AU/index, missing keyframe,
  truncation, symlink и повреждение следующего AU.

## Следующий интеграционный шаг

- [x] Capability-addressed chunked publication через private IPC: порядок/повтор AU,
  cancel/finish, quota, no-clobber и восстановление receipt после restart.
  Raw filesystem paths от frontend не принимаются.
- [x] MediaAssetStore: v1/v2 list и acquire_source, общая quota, pins/delete;
  PinnedMedia закрывает descriptor раньше отпускания pin. Legacy keys сохранены.
  Profile Save держит общий pin до durable commit. Store publication доступна через `media_upload`.
- [x] RPC первого AU v1/v2 и подключение native thumbnail реализованы;
  native v2-проверка прошла с MockBackend. IPC publication реализована;
  `set_lcd_media` и manager replay уже используют pinned source/reader.
  Legacy `load_prepared_clip`/finite diagnostic playback сохраняют старый контракт.
- [x] Blocking reader worker: bounded prefetch, cancellation-safe joined stop,
  legacy/v2 read/loop, удержание file pin и ошибка повреждённого payload.
- [x] Worker подключён к actor: frame-batch сериализация, поколения replace/clear,
  firmware/rotation gate и awaited stop при shutdown/sleep сохранены.
- [x] Native prepare_file пишет H.264 в owned temp file с bounded index/AU;
  проверен VP9 roundtrip586frames. Legacy prepare сохраняет diagnostic budgets.
- [x] prepare_file подключён к MediaImports и передаче AU через chunked IPC;
  cancel до commit, lost-reply readback и cleanup перед terminal status.
- [x] Profile selection и pending settings/restart/sleep используют v2; private IPC/MockBackend проверены.
- [x] Native thumbnail qualification для v2: cyan144px/no-playback/delete.
- [x] GUI video+realtime sensor/text composition: preview/Apply/slots и native
  MockBackend workflow. [Checkpoint](evidence/2026-10-12-video-background-gui.json).
- [ ] Контексты галерей, protected defaults и полный Screen reset.
- [x] Native GUI import VP9 586frames → v2 → полный mock loop/clear/restart;
  опубликованный H.264 декодирован во все586frames.
- [ ] Native publication faults/accessibility, затем отложенная аппаратная приёмка.


## Reader worker — 2026-10-12

`media_reader::MediaReader` владеет `PinnedMedia` через blocking job. В очереди
не более двух AU и ещё один может ожидать отправки. Это ограничение prefetch,
а не всего RSS: index и legacy H264Clip сохраняют свои собственные бюджеты.
Чтение повторяется с первого AU; pacing и HID остаются обязанностью actor.

`stop()` закрывает очередь, отменяет producer и ждёт завершения job. Отмена
самого stop future сохраняет JoinHandle для последующего join. После join
очередь очищается повторно: sender с уже полученным permit может отправить
после `close()`. После cancellation `next_unit()` больше не выдаёт AU.
JoinError возвращается один раз; повторный stop безопасен. Drop только
сигнализирует отмену, не заменяет awaited barrier. Незавершённое filesystem
read нельзя принудительно прервать; source и pin остаются у worker до выхода.

Шесть unit-тестов проверяют backpressure, loop order, отменённый stop,
ошибку чтения, panic/repeated stop, drop и outstanding-permit race. Два
integration-теста работают с настоящим store: legacy/v2, 586 кадров и возврат
к первому, единственный worker pin и отдельный pending-save pin, truncation
после открытия. Это framing/lifetime fixtures, не новая codec qualification.
[Checkpoint](evidence/2026-10-12-media-reader.json).

Приёмка actor integration:

- [x] Playback/replay v1/v2 через IPC и manager без полной загрузки v2 в память.
- [x] Replace/clear/sleep/shutdown ждут reader и HID frame; старые AU не попадают
  в новое поколение, pending persistence pin живёт до durable commit.
- [ ] При медленном чтении нет второго reader или HDD I/O в HID actor;
  cancellation не теряет handle, failure отображается без ложного applied.
- [ ] Corrupt/missing source и stale preparation сохраняют корректные requested,
  saved и error состояния; firmware/rotation gates остаются явными.


## LCD actor integration — 2026-10-12

`SetMediaClip` переносит `PinnedMedia` вместо materialized clip+pin.
`set_lcd_media` и manager replay открывают источник на blocking job; worker
создаётся после firmware/rotation checks и завершения старого source.
Periodic ожидание `next_unit` допускает команды и shutdown; HID frame batch
после получения AU завершается полностью. Stop/replace/fail стали async,
выход LCD actor ждёт reader прежде подтверждения manager teardown.
Первый кадр по-прежнему входит в подтверждение команды playback; зависшее
filesystem read требует завершения, чтобы выполнить joined stop.

[Checkpoint](evidence/2026-10-12-media-actor.json): framing fixture586frames +
первый кадр следующего цикла, Clear за незавершённым frame, media→JPEG replace,
отсутствие старых кадров и освобождение pin. CLI→private IPC→MockBackend
проверяет v1/v2 profile selection, restart, missing/recovery, sleep/wake,
old attachment rejection, Clear и reference-protected delete.
Это не новый encoder/decoder или native GUI checkpoint. Длинный пользовательский
импорт и thumbnails всё ещё требуют native integration; chunked publication
добавлен следующим checkpoint ниже.


## Chunked publication — 2026-10-12

`media_upload` принимает `{request:{action,...}}`:

| action | Поля помимо action | Результат |
|---|---|---|
| begin | id, name, frames, encoded_bytes | Receiving либо существующий receipt того же импорта |
| chunk | id, index, unit:{bytes,keyframe} | Следующий AU; точный повтор только последнего принятого AU безопасен |
| status | id | Состояние сессии или опубликованный asset по постоянному ключу |
| finish | id | Публикация только при точном совпадении frames/bytes |
| cancel | id | Staging удаляется; после commit возвращается Published без удаления asset |

Клиент генерирует **новый случайный128-bit lowercase hex id** для каждого импорта
и сохраняет его до окончательного исхода. Не переиспользовать id отменённых,
ошибочных, истёкших или удалённых импортов. Id — capability в пределах доверенного
локального IPC, не привязка к connection и не дополнительная аутентификация.
Он же становится ключом asset. Неизвестные поля request отклоняются.

Состояние содержит name, declared frames/encoded_bytes, received_frames/bytes,
phase (receiving/published/cancelled/failed), asset и error. Ошибка записи может
вернуть Failed; клиент обязан проверять phase, а не только успех JSON-RPC.
Неполный Finish не публикует файл. После пропавшего ответа сначала читать Status;
не создавать новый импорт автоматически. Finish и Cancel сериализованы с chunk.
Commit выполняется на blocking job, который удерживает admission после потери
caller. При неоднозначном fsync readback повторно подтверждает file/directory sync.
После restart восстанавливаются опубликованные receipts; незаконченные uploads
не возобновляются. Cancellation не удаляет опубликованный файл.

Лимиты:4active uploads,64session receipts, idle5minutes (lazy expiry при следующем
запросе или Drop при штатном shutdown), до86400frames/256MiB на импорт,
128KiB/AU. В памяти writer держит bounded index, а session — только последний AU
для retry. Общий JSON-RPC лимит8MiB не увеличен. Metadata budgets — ограничения
Linux runtime, а не доказанные ограничения Windows.

Payload анонимный и освобождается при гибели процесса. Publication copy имеет
namespace `.llcontrol-media-publish-<6alnum>.tmp`; её создание/copy/persist/fsync
и очистка orphan защищены nonblocking directory flock. Guard явно делает Unlock
перед закрытием fd, чтобы дублированный/унаследованный дескриптор не продлевал lock. Следующий begin/finish
убирает только regular files этого namespace, максимум1024entries на scan;
живой writer, чужие `.tmp` и symlink не удаляются. Старые temp-файлы вне этого
namespace автоматически не очищаются. Rustix1.1.4 уже присутствовал в lockfile
и теперь является прямой fs dependency daemon.

[Checkpoint](evidence/2026-10-12-media-upload.json):586AU через RPC, совокупный JSON
превышает8MiB, каждый запрос bounded; реальный disconnect после принятого Finish
и receipt после restart; retries/order/limits/quota/cancel, concurrent Finish,
idle expiry и child-process kill без Drop. Crash-test publication copy создана
как контролируемая fixture под тем же lock; это не fault injection внутри fsync.
Native/GUI import и progress/cancel/readback UI остаются следующими.
`load_media_first_frame` уже читает первый AU под store pin в blocking job;
native thumbnail использует этот RPC вместо full-clip load. Legacy/v2 RPC
проверены тестами; native v2-сценарий прошёл с MockBackend.

## Native publication ownership — 2026-10-12

MediaImports теперь возвращает PreparedFile и удерживает admission до завершения
передачи/cleanup. Стадия `transferring` допускает cancellation; `publishing`
начинается под тем же mutex только после проверки cancel и передачи всех AU.
Каждый read выполняется blocking job, каждый RPC несёт один AU. Raw media_upload
остаётся за native bridge, не добавляется в общий renderer allowlist.

При потере ответа Begin/Chunk/Finish клиент читает Status того же ID и проверяет
identity/name/frame/byte counts. Новый ID автоматически не создаётся. При ошибке
или cancellation отправляется Cancel; Published receipt означает успех, а не
удаление готового asset. Неподтверждённый исход отображает ключ для проверки
библиотеки. File owner уничтожается до terminal status; close ждёт worker или
результат публикации. Реальные socket faults во время native UI publication
остаются отдельной приёмкой, даже при пройденных closure/RPC regressions.
