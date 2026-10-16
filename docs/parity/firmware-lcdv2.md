# LCDV2 firmware: identity и оставшийся updater

Baseline: Windows L-Connect3 **2.1.28.0**, Galahad II LCDV2 **0416:7395**.
Frozen reference: `/home/inky/Development/lian-li/reference-decompiled/2.1.28-2026-09-08`.
Хеши оригинальных EXE/Core сверены с manifest этого baseline. Proprietary exports
не копируются в Linux repository. Текущий срез не отправлял команд устройству.

## Текущий статус — 2026-10-16

| Область | Текущая реализация / граница |
|---|---|
| Identity | PN-aware actor query, IPC/CLI и GUI EN/RU подключены; hardware latency/date acceptance отложена |
| Transfer | Pinned candidate/codec, scoped journal+settings guard, shared admission и original-HID prepare/transfer/join engine реализованы для Mock acceptance |
| Live job ownership | Process-local retained observers, explicit cancel, main shutdown join и worker-panic Unknown реализованы; public live-status UI/RPC ещё нет |
| Saved restart state | Read-only IPC/CLI и Settings EN/RU показывают journal/guards, включая отсутствующее устройство и sealed gate; snapshot, no redo/HID/clear |
| Reconnect | Explicit owned probe job после acknowledged transfer/joined close подключена; qualified resolution и restart/serial-less/missed-reset policy открыты |
| Product updater | Saved-state inspection EN/RU подключена; image Inspect/explicit Apply/live progress и qualified recovery/restoration ещё отсутствуют, пользовательский flash не доступен |
| Приёмка | Private-file/Mock checkpoints ниже; physical/image/revision/cooling qualification и расширенные lifecycle/fault gates не закрыты |

Ниже сохранены source-derived контракты и история отдельных checkpoints. Слова
«следующий», «ещё отсутствует» в старых checkpoint относятся к их дате; текущий
backlog — в [roadmap](../../roadmap.md) и [чеклисте](basic-parity-checklist.md).
Ни readback version, ни ACK, ни saved state не являются разрешением firmware update.

## Identity readback, 2026-10-15

Windows Core `Products/Galahad2Vision/Galahad2VisionDevice.cs:86–105` отправляет
один A/cmd134 и читает version/date. PacketNumber в bytes3…4 различает PN0 и PN1.
Сентябрьские observations на FW1.6 фиксируют доступный PN0 и ненадёжный PN1;
поздний PN1 был также виден в frame observation. Это историческое наблюдение,
не новый аппаратный тест.

Прежний daemon отправлял cmd134 **дважды**, каждый раз принимал первый matching
command echo и передавал второй ответ как datetime без проверки PN. При повторном
PN0 в дату попадала строка версии. Software defect подтверждён текущим кодом;
поведение исправлено без подключения к физическому устройству.

Новый `HidCommand::Firmware` выполняется владельцем HID как одна транзакция:

- Один фиксированный A/cmd134 output, без повторного запроса за датой.
- До8 чтений/1s read budget после output; после PN0 optional read ждёт не более100ms.
- Проверка actual read extent/report ID/cmd/length перед копированием; версия и
  дата собираются по PN0/PN1 при любом порядке. Duplicate/unknown PN и unrelated
  packets не меняют назначение ответа. Отсутствующий PN0 — ошибка.
- Отсутствующий PN1 оставляет datetime пустым. Он не подменяется версией,
  host time, текущей датой или fake zero. Ошибка чтения остаётся ошибкой.
- Другие writer commands не вклиниваются между ответами. H.264 startup gate
  использует тот же PN-aware query вместо первого matching cmd134. Pending sensor reply
  завершается до firmware-транзакции через существующий FIFO/deferred путь;
  shutdown ждёт начатую blocking transaction.
- Stats учитывает успешный query write даже при последующей ошибке ответа.

Protocol decoder дополнительно проверяет PN при разборе классифицированных
ответов. `FirmwareInfo` сохраняет прежние `version`/`raw_token` и добавляет
optional `build_datetime`: это **неразобранная строка PN1**, не календарная или
подписанная firmware identity. При отсутствии даты поле не сериализуется;
старый ответ без поля читается как unavailable без вывода даты из raw token.
CLI `firmware` показывает `Build date: unavailable`, JSON оставляет поле
отсутствующим. Сам raw token по-прежнему доступен для диагностики.

Это явный identity query, не новый периодический polling loop. Влияние bounded
query на активный playback и поздний PN1 ещё проверить на железе. PacketNumber
не является transaction nonce: одинаковые поздние ответы не доказывают свежесть
нового firmware после перепрошивки. Для updater нужен отдельный reconnect и
проверка identity/version на новом attachment.

[Software checkpoint](evidence/2026-10-15-firmware-identity.json) содержит protocol,
actor и real CLI → private Unix IPC → MockBackend проверки. Реальная версия/дата
и streaming workload после этого изменения остаются непроверенными.

## GUI preflight EN/RU

В Settings добавлена карточка Firmware information. Только явная кнопка читает
`get_firmware`; открытие Settings и смена языка запрос не запускают. Версия,
неразобранная дата и raw token сохраняются как данные устройства. Отсутствующая
дата отображается как Unavailable / Недоступна. Ошибка или неподдерживаемый ответ
очищают прежние значения; raw diagnostics доступны отдельно как plain text.

Карточка привязана к эпохе выбора и доступности устройства. Смена устройства,
наблюдаемая потеря/возврат соединения или уход из Settings уничтожают прежний
контекст: поздний ответ и его finalizer не меняют новую карточку. Повторное
нажатие во время чтения не ставит второй запрос. Начатая daemon HID-транзакция
при уходе из карточки завершается под прежним владельцем.

Это отображение identity, не проверка firmware image и не разрешение flash.
Native bridge разрешает `get_firmware`, но по-прежнему запрещает команды
обновления. [GUI checkpoint](evidence/2026-10-15-firmware-gui-preflight.json)
разделяет browser, private native и незакрытые аппаратные проверки.

## GUI saved restart inspection EN/RU — 2026-10-16

Settings → Saved firmware state читает get_firmware_recovery({}) только по явной
кнопке. Карточка глобальная: выбранный DeviceId не требуется; отсутствие устройства
и sealed ordinary admission не запрещают inspection. Это отдельный read-only путь,
а не get_firmware на прежнем attachment и не наблюдение за live job.

UI различает no_saved_operation/journal_only/guarded/guard_only/inconsistent/
unreadable, пять сохранённых stages и nullable progress. Показаны operation ID,
сохранённые VID:PID/serial/previous HID path/settings key/image hash, все guards,
сохранённые counters и markers. Отсутствие отметки не означает отсутствие команды;
619 replies или activation marker не объявляются update success. Все состояния
с артефактами/ошибкой чтения объявляются alert; no_saved_operation не обещает
открытый admission или отсутствие live job. Raw diagnostics — данные без перевода
или HTML execution; доступны как read-only JSON, неподдерживаемый ответ — bounded
technical excerpt.

Shape/enums/UTF-8 bounds/counter ordering и согласованность state с operation/
guards проверяются до показа снимка. Unknown/malformed response и read failure
очищают прежние поля. Чтение не повторяется автоматически. Повторная активация
занятой кнопки не создаёт второй запрос. Уход из Settings и наблюдаемая смена
ordinary availability уничтожают UI epoch; поздний ответ не заполняет новую
карточку. Снимок исторический; для нового состояния требуется явное чтение.

Native bridge разрешает только новый getter. Update/clear/probe/replay entry
points не добавлены. Saved-state reading не является qualified resolution,
не снимает quarantine и не позволяет replay прежних settings. Image/target
qualification, Apply/live status, durable restart jobs, serial-less/missed-reset
policy и qualified restoration остаются в backlog.

[Software/native checkpoint](evidence/2026-10-16-firmware-recovery-gui.json)
отделяет browser fixtures от реальных private files → Unix IPC → Tauri/WebKit.
Реальное устройство, flash и production daemon не используются.

## Windows updater: source-derived маршрут

Пути ниже относительно frozen baseline; строки не являются hardware evidence.

| Этап | Что делает Windows2.1.28 | Linux requirement / ограничение |
|---|---|---|
| Выбор backend | `UpdatePage.cs:1341–1345` выбирает Galahad2Vision upgrader для LCD и LCDV2 | Привязать job к конкретному0416:7395/attachment; ENE updater не подходит |
| Файл | `UpdatePage.cs:434–448`: download и сравнение MD5 с remote metadata, затем bytes cache | Установить format/target/version модели образа и источник ожидаемого hash; кандидат1.6 получен и закреплён по SHA-256; format/target qualification остаётся |
| Передача владения | UI controller`:461–468` отправляет service request Enter/ExitManualMode; service controller`:1467–1476,1630` меняет guard флага | Это service guard, не доказательство отдельной HID enter-manual команды. Linux должен quiesce/join keepers и сохранить одного HID owner |
| Upload | `Galahad2VisionFirmwareUpgrader.cs:25–27,93–105`: пауза3s, B/cmd11, write/read512 на каждый chunk, progress, пауза10ms | Existing B framing не заменяет updater. Нужны проверенный response/status и точная стадия/число переданных байт |
| Activate | upgrader`:107–110`:2s, `ActiveFirmware`,5s; Core device`:140–151`: B/cmd10 с payload01 | Caller игнорирует boolean результата. Length/status semantics и реакция на ambiguous reply ещё требуют подтверждения |
| Возвращение USB | upgrader`:44–45,110–137` ждёт insertion LCD/LCDV2 до20 дополнительных секунд | Нельзя считать любой такой VID/PID успешным обновлением выбранного устройства. Нужны identity-bound reconnect и version verification |

В upload loop проверяется только `Read(...) == -1`; response body не
сопоставляется с chunk или статусом. Outer catch может скрыть transport failure.
Эти свойства Windows-кода не являются достаточным success criterion для Linux.
Core activation также сравнивает Length1 и читает Data[1]; реальная длина/status
payload не подтверждены. Источник доказывает наличие вызова, а не wire ACK.
В папке установки standalone image не был найден. Кандидат1.6 теперь получен
из официального каталога; это отдельное live evidence, описанное ниже.
Electron snapshot `.bin` из установки к firmware по-прежнему не относятся.

## Каталог и точный кандидат1.6, 2026-10-15

Source route baseline2.1.28: `AppUpdater.cs:33–47,147–155,180–202` запрашивает
locale-specific `firmwares-{locale}.json`; `:359–400` преобразует ключ
`GA II LCD` в `GA2LCD`. `FirmwareUpdateUtils.cs:192–194` направляет **LCD и LCDV2**
в этот firmware type. Это Windows mapping, не проверка hardware revision.

Текущий [официальный English catalog](https://lianli-update-2025.lianli-cn.com/lianli-update-2025/lianli-l3-update/firmwares-en-US.json)
получен2026-10-15. Это **не замороженный каталог от даты Windows baseline**.
Запись `GA II LCD`: version1.6, compile2025/10/31,
`HH015CHS_N9H26HSSQ_GALAHADII-Vision_V201041_pid7395.bin`,626212bytes,
MD5`3a341bb5da86a5119545c14ec805a307`.
[Image URL](https://lianli-update-2025.lianli-cn.com/lianli-l3-update/HH015CHS_N9H26HSSQ_GALAHADII-Vision_V201041_pid7395.bin)
отвечает HTTP200/Content-Length626212 и поддерживает ranges. Полная загрузка
застревала после16976bytes;39 bounded HTTPS range requests с `If-Match` собраны
с проверкой каждого Content-Range/size. Итоговый размер и MD5 совпали.

Точный **SHA-256**:
`68e2d11c409e75b1e25bf9c2e84b891bcdc18ac735bafa67a0ee25a218b2d9d3`.
Каталог/HTTP headers/образ и результаты статического анализа сохранены **вне Git**:
`/home/inky/Development/lian-li/reference-firmware/2026-10-15-lcdv2-1.6`.
[Evidence manifest](evidence/2026-10-15-firmware-image-research.json) содержит hashes,
источники, тесты и границы выводов. `firmwares-ru-RU.json` в этой сессии ответил403;
это не основание объявлять endpoint навсегда отсутствующим. Будущий update catalog
не должен зависеть от выбранного языка интерфейса без явного fallback.

В точных байтах обнаружены следующие поля; **общий формат всех образов пока не установлен**:

| File offset | Наблюдение |
|---|---|
| `0x6000` | LE32 pointer `0x80000` |
| `0x80000` | два LE32 markers `0x12345678`, `0x55aaaa55` |
| `0x80008`, `0x80014` | raw build strings `Oct 30 2025`, `17:01:18`; это не compile date каталога |
| `0x80024` | NUL-terminated `N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6` |

N9H26 strings и matching version/token подтверждают пригодность кандидата для
дальнейшего исследования. Filename/PID и embedded string **не доказывают**
совместимость всех hardware revisions. MD5 каталога не является подписью;
локальный SHA-256 закрепляет исследованные байты, а не заменяет target qualification.

### Офлайн-инспектор

`python3 tools/inspect-lcdv2-firmware.py /path/to/image.bin --target 0416:7395`
читает только regular file через закреплённый `O_PATH` inode и bounded read.
Имя/расширение файла не учитываются; принимается только точный size/SHA-256 и
identity по записанному offset. CLI возвращает JSON `recognized_candidate` или
`rejected`, всегда `flash_ready:false`. Target — ожидаемый VID:PID, введённый
пользователем, не результат обнаружения устройства. Network/HID/update-path отсутствуют.

Восемь synthetic tests покрывают corruption/truncation/oversize/wrong target,
filename/token spoofing, symlink/FIFO/directory и замену пути во время чтения.
Actual CLI отдельно проверен на полученном образе, его изменённой копии, частичном
файле и другом target. Это recognition известного кандидата; arbitrary images и
безопасный updater этим не реализованы.

### Статический анализ самого firmware

Ghidra headless, raw binary/base0, `ARM:LE:32:v5t`, default compiler. Ниже —
**source-derived inference для точного SHA-256**, не USB capture или hardware acceptance.
Экспорты псевдокода остаются во внешнем reference-каталоге.

- Table`0x862f8` указывает на обработчик активации`0x815d8`, следующий entry —
  загрузка`0x81a94`. Dispatcher`0x3c418` отправляет512-byte B/C replies.
- Upload handler копирует входной packet, сохраняет cmd11/PN и возвращает echo.
  Helper`0x8219c` собирает bytes по big-endian PN/length; PN0 начинает новое
  накопление, следующий PN должен идти последовательно. Следовательно, retry PN0
  после ambiguity может начать новую загрузку, а произвольный повтор chunk не доказан безопасным.
- При достижении полного размера upload handler проверяет offset из`0x6000`
  и два markers, пишет `c:\K_DOWNLOAD\GalahadII.bin`, затем читает и сравнивает
  сохранённые bytes. Промежуточный echo не доказывает окончательную запись образа.
- Error branches меняют command на2 и byte6 на`0x80 + local error`; это другой
  response shape, его нельзя принять как обычный successful B/cmd11 ACK.
  Для upload видны коды1/2/3/4/6/7 (empty/offset/markers/sequence/size/verify);
  это предварительная интерпретация ветвей, не полный device error enum.
- Activate handler проверяет payload01 и состояние завершённой загрузки, читает
  сохранённый файл/markers, копирует в `c:\GalahadII.bin`, сравнивает содержимое,
  выставляет bytes11=1 и12=1, отправляет512bytes перед reset. Неопределённый исход
  после потери reply требует reconnect/version verification, не повтор flash.
- Это объясняет дополнительный raw byte12 при activation, но Windows Core
  `ActiveFirmware` проверяет Length1 и обращается к `Data[1]`, тогда как getter
  создаёт массив Length. Такой decoder нельзя переносить буквально.

Далее нужно проверить полный ACK/error envelope, stage/byte ownership и status
значения, исследовать persistent boot/recovery transitions. Закреплённый образ
позволяет делать codec/mock fault matrix, но ещё не разрешает выполнять flash.

### Decoder ACK/error, 2026-10-15

`protocol::galahad2_lcdv2::firmware_update` теперь содержит отдельный decoder
для исследованного FW1.6 envelope. Это codec, без I/O, updater job или нового
GUI/CLI способа отправить команды. [Checkpoint](evidence/2026-10-15-firmware-ack-codec.json).

- `ChunkExpectation` берёт фактический B/cmd11 request и проверяет nonempty
  canonical1013-byte chunking: total/PN/length, конец внутри image, zero padding.
  Ответ должен иметь **actual extent512** и точно совпадать с первыми512bytes
  запроса. Заголовок может сохранять Length1013, несмотря на501echoed payload
  bytes; общий `BResponse` для такого truncated echo намеренно не используется.
- Матч проверяет report/command/total/PN/length/видимый payload/padding.
  `ChunkEcho` содержит request end offset и is_last; это не receipt о завершении
  update и не доказательство всех bytes или persistent state.
- Error envelope cmd2 сохраняет исходный request, но меняет byte6 на status.
  Остальные surviving bytes должны совпадать с ожидаемым request. High-bit
  statuses возвращаются как `DeviceRejected(raw)`, включая неизвестные значения.
  Неправильный status, unmatched/short/oversize replies остаются ошибками.
  Выше PN65535 исходный high PN byte теряется: такой error помечается ambiguous,
  а не выдаётся за полностью коррелированный. У кандидата1.6 всего619packets.
- Activation decoder проверяет canonical cmd10/total1/PN0/Length1/payload01
  и дополнительный **raw byte12=1**. Не индексирует `Data[1]` вне объявленного
  payload. Результат — только `ResetRequested`; ни version success, ни recovery
  readiness из него не следуют. Activation cmd2 также остаётся ошибкой.

Десять synthetic cases проходят все619chunks размера найденного image,
actual extents0…511/513, single-bit corruption в каждой позиции512-byte ответа, old PN/другой
size/prefix, observed/unknown device errors и activation extra flag. Отдельный
тест показывает предел: identical late echo и изменённая
**неотражённая** половина chunk неразличимы. Decoder не придумывает nonce/hash;
будущий update owner обязан удерживать attachment/epoch и один in-flight request,
сохранять ambiguous outcome, не повторять flash автоматически и проверять версию
после reconnect. Wire/recovery qualification по-прежнему открыта.

## Actor-owned transport, 2026-10-15

`actor::firmware_update` и internal `HidCommand::FirmwareUpdate` выполняют одну
blocking-транзакцию на **существующем HID handle**. IPC/CLI/GUI entry point для
записи ещё отсутствует: до подключения требуются coordinator, keeper teardown и
reconnect verification. [Checkpoint](evidence/2026-10-15-firmware-owner-transport.json).

Admission permanently fences этот actor/attachment: queued и последующие Write,
FrameBatch, PollSensors, Firmware, WriteRead64/512 и повторный FirmwareUpdate
не получают HID I/O. Даже NotStarted/cancel/error не снимают fence. Повторный
запрос не перезаписывает terminal progress первой транзакции, в том числе если
caller повторно передал тот же watch sender. Это transport barrier, **не завершённый
quiesce/join всех keepers**; фоновые producers пока получают ошибки. Не подключать
user Apply к нему до реализации coordinator.

Перед передачей image:

- `ValidatedImage::recognize` проверяет exact626212bytes/SHA-256 исследованного
  образа; fields private, произвольный unchecked image через production API
  создать нельзя. Network/path/download code в этом primitive отсутствует.
- Actor проверяет VID/PID0416:7395, выполняет bounded preflight drain, один
  cmd134 query на том же handle и требует exact PN0 identity исследованного1.6.
  Другие версии/ошибки/неизвестный UTF-8 не допускают cmd11/10. Это ещё не
  hardware revision qualification или доказательство свежести одинакового PN0.
- Windows-derived settle3s и pre-activate2s ждут interruptibly. Drain должен стать
  quiet за10nonblocking reads. Уже pending sensor завершается до admission;
  FIFO writer после него не обгоняет update.

Upload идёт canonical B/cmd11/1013bytes, один outstanding request. Один успешный
write → bounded read/decode → следующий chunk; **повтор write не выполняется**.
Между chunks сохранена10ms пауза. Reply deadline10s проверяется через reads≤100ms;
valid late A/cmd134 PN1 пропускается в том же deadline, не считаясь ACK и не
вызывая новых writes. Other/unmatched/short replies прекращают передачу.
`OpenHandle::write` остаётся synchronous backend call: deadline не обещает
прервать зависший kernel/driver write, actor ждёт его завершения.

Typed progress сохраняет total/chunk writes/echoes, readonly query write,
activation write и **последний attempted step**, установленный перед write.
Short/failed write поэтому даёт Unknown, даже если backend не подтвердил полный
report. Отмена до cooling hold даёт NotStarted; после попытки PWM hold/cmd11/10 —
Unknown, в том числе при известном device error: staged/persistent state этим
не доказано. После activation reply только AwaitingReconnect, **не Success**.
Drop requester oneshot не detach-ит worker и не снимает fence; отдельный cancel
или shutdown останавливают после текущего blocking I/O, который actor join-ит.

Программная приёмка включает first/middle chunk rejection, short/stale/missing
ACK, synthetic detach/read error, partial write, activation reply loss, cancel
перед activation, pending sensor/FIFO, повторный job, drop requester и joined
shutdown. Полный acquired image отдельно проходит recognition и619chunks на
MockBackend. Ни одна из этих проверок не открывает физическое устройство.

Остаётся coordinator, который закрывает admission controls и quiesce/join
keepers **до** transport, сохраняя живым тот же HID owner. Отдельный
keeper lifecycle теперь реализован (ниже); coordinator должен использовать
его вместе с admission/health checks. Coordinator
должен явно выбрать и записать cooling hold plan до физической стадии;
requested hold теперь выполняется внутри transfer (ниже). Effective duty/RPM
и поведение при reset ещё требуют hardware qualification. Восстановление intent —
только после проверки результата.
Manager должен удерживать новый attachment без replay
сохранённого intent до identity-bound reconnect/version verification. Текущий
обычный hotplug/autoreplay путь для update использовать нельзя. Нужны durable
operation/outcome, staged Inspect→explicit Apply, progress/cancel UI EN/RU,
recovery и post-update verification. До этих изменений product updater отсутствует.

## Keeper quiescence на том же HID handle, 2026-10-15

`DeviceHandles::keepers` теперь содержит `KeeperLifecycle`. Sensor poll,
cooling, LCD, lighting и control group получают отдельный stop watch. HID actor
продолжает получать root shutdown от manager. Root true/дроп sender также
пересылается keepers; обычный detach/sleep/shutdown по-прежнему завершает всё
дерево. Forwarder с watch ожиданием join-ится/снимается владельцем teardown,
он не содержит I/O или worker work.

`quiesce()` однонаправленно запрашивает stop и ждёт JoinHandle **всех пяти**
producers; sensor_poll получил tracked spawn. Затем settings dispatcher закрывает
admission, обрабатывает принятые deltas и завершает flush attempt. Только после
его join публикуется Joined/Failed. Dropping одного quiesce waiter не отменяет
stop или join owner; повторные waiters получают тот же terminal result. Actor
panic/JoinError или потерянный join completion не выдаются за успешный barrier.

**Joined — task termination, не firmware readiness.** Save failure остаётся в
`save_state.pending/last_error`; integration test специально показывает Joined
при failed persistence. Nested LCD/codec cleanup errors теперь передаются actor-tree supervisor и
превращают barrier в Failed; `quiesce_checked()` дополнительно проверяет save
state (подробности ниже). Coordinator обязан использовать эти проверки перед
допуском flash и явно выбрать cooling hold plan. Внутренний barrier ещё не
закрывает admission всех IPC/preparation jobs и не является user Apply entry point.

После Joined каналы keepers/settings закрыты, sensor feed завершён, но root
teardown ещё false и HID принимает явный запрос на том же открытом handle.
Работа keepers автоматически не возобновляется. Resume/rebuild допускается
только через будущий проверенный coordinator; manager hotplug/saved-intent replay
пока не изменён. Product updater по-прежнему отсутствует.

[Checkpoint](evidence/2026-10-15-keeper-quiescence.json): private actor tree с
MockBackend проверяет inflight sensor read, отмену waiter/повторный quiesce,
принятую settings delta до barrier, закрытие producer channels, отсутствие
последующих poll/reissue и ровно один backend open. Root shutdown/drop и shutdown
во время quiescence ждут blocking I/O до teardown. Отдельная внешняя vendor
fixture запускает quiesce→621reports (query+619chunks+activation) на том же
handle в исходном checkpoint; после добавления hold —622reports (ниже). Достигает только AwaitingReconnect и подтверждает post-update fence.
Все эти действия программные, без физического HID.

### Cleanup/save preflight, 2026-10-15

[Checkpoint](evidence/2026-10-15-quiesce-health.json). LCD keeper теперь сохраняет
первые16nested cleanup failures до завершения actor. Ошибка stop/JoinError media
reader или composition не исчезает после перехода Source→Idle и следующего
успешного stop. Cleanup queued SetComposition/SetMediaComposition и prepared
screen groups также входит в report. Существующие error replies/logging остаются.

Actor tree использует observed LCD spawn: после join получает отдельный cleanup
report; любой report error или отсутствие report переводит keeper barrier в
Failed. Ошибки не отменяют join остальных producers/settings. При потере join
owner без terminal completion `state()` также Failed, а не вечный Stopping.
Legacy LCD spawn API сохраняет прежний JoinHandle<()> без дополнительного wrapper
actor; test-only panic constructors не входят в production build.

`DeviceHandles::quiesce_checked()` выполняет joined cleanup, затем читает финальный
`save_state`. `QuiesceError::Cleanup` и `QuiesceError::Persistence{pending,last_error}`
явно различаются. Pending или last_error дают отказ, даже если сами tasks Joined.
`saved:false` у чистой новой конфигурации само по себе не считается ошибкой:
если нечего сохранять, успешный fsync в этой сессии не обязателен. Повтор проверки
не снимает прежний cleanup/persistence failure и не выполняет HID writes.

Tests вызывают настоящие reader/supervisor task panics в synthetic fixtures без
subprocess/device work. Два queued codec failures доходят через реальный actor
tree до typed Cleanup; повторный preflight остаётся ошибкой, HID history пуста.
Blocked private settings destination даёт typed Persistence, successful accepted
settings delta проходит checked preflight. Vendor MockBackend fixture теперь
использует checked quiesce перед полным transfer на исходном handle.

Это helper для будущего coordinator, **не автоматическая проверка внутри каждого
HidCommand::FirmwareUpdate и не разрешение flash**. Пока нет product entry point.
Coordinator ещё должен закрыть global IPC/preparation admission, подтвердить
выбор/владение cooling hold plan, записать durable operation/outcome и удерживать
reattachment/replay до target/version verification. Hardware/recovery gates
и limits существующего wire/transport остаются открыты.

### Explicit cooling hold внутри transfer, 2026-10-15

[Checkpoint](evidence/2026-10-15-firmware-cooling-hold.json). Это **Linux update
policy**, не утверждение, что Windows upgrader сам применяет такой hold.
Baseline Core `Galahad2VisionDevice.cs:103–122` задаёт write-only cmd138
`[source,pwm]` и cmd139`[group0,pwm]`; no PWM getter/ACK этим кодом не установлен.
Завершённый `OpenHandle::write` подтверждает transport write, **не фактический
RPM, источник или thermal safety на устройстве**.

Internal `FirmwareUpdate` теперь требует explicit `CoolingHold`:

- `PumpOnly`: запрос pump MCU/100%, cmd138 payload`[0,100]`.
- `PumpAndControllerFan`: тот же pump request, затем fan100%, cmd139`[0,100]`.

Выбор fan не выводится из0RPM, sensor cache или имени устройства. Для fan на
материнской плате coordinator должен выбрать PumpOnly; такая ветвь не отправляет
cmd139. Пока user-facing Inspect/Apply отсутствует, все эти записи выполняются
только MockBackend tests. Будущий UI должен явно показывать смену pump source
и duty, fan ownership, сохранённый intent и условия восстановления.

Порядок на исходном handle: pinned image/device/current identity preflight →
requested pump hold → optional controller fan hold → settle3s/drain → chunks →
preactivate2s/activate. Actor permanent fence уже установлен; другие команды
не вклиниваются и не могут перезаписать hold. Между hold stages и перед каждым
chunk проверяются cancel/root shutdown. No write retry и no automatic PWM restore.

Progress добавляет HoldingCooling, attempted PumpHold/FanHold, выбранный plan и
pump_hold_written/fan_hold_written; counters учитывают только полностью завершённые
writes. Partial/failed hold или отмена после него дают **Unknown**, даже если
chunk_writes=0: cooling state уже мог измениться. Preflight failure/cancel до
hold остаётся NotStarted. История попыток/флаги должны попасть в будущий durable
journal; без него нельзя разрешать reboot/reconnect replay.

Hold не проходит через settings dispatcher и не заменяет сохранённую curve/source.
Mock fixture сохраняет исходный motherboard80% intent до quiesce, выполняет MCU100%
hold и full transfer, затем читает тот же persisted intent. Это не доказательство
актуального hardware source до/после команды. Восстановление не выполняется ни
после error/cancel, ни после AwaitingReconnect; оно требует verified outcome.

Три новых synthetic cases покрывают owned-fan ordering/bytes, partial pump write,
fan detach и отмену после pump hold в обеих plan ветвях; нет chunks/activate/restore.
Проверки всего acquired image теперь ожидают **622reports** с PumpOnly:
query1 + pump hold1 + chunks619 + activation1. Все остаются MockBackend acceptance.
Cooling/RPM во время upload/reset и source/duty restoration входят в отложенную
hardware матрицу. Product updater, global admission и durable/reconnect coordinator
ещё не реализованы.

### Durable journal и ordinary attach guard, 2026-10-15

[Checkpoint](evidence/2026-10-15-firmware-operation-journal.json). Новый
`PersistenceStore::firmware_operations()` хранит отдельный version1
`firmware-operation.json` в private config root. Это **не redo log**: загрузка
файла никогда не повторяет PWM/chunk/activation writes.

`arm()` создаёт128-bit operation ID, target VID/PID/serial/path/settings key,
pinned image SHA-256, выбранный hold и полный restore settings snapshot. Snapshot
проверяется существующим settings validator; schema, binding, counters и sizes
проверяются также при load. JSON ограничен2MiB, diagnostics —8192bytes. Target path
сейчас только metadata, не инструкция открыть сохранённый hidraw path.

First publication использует tempfile→file fsync→persist_noclobber→directory
fsync. Существующая, чужая или malformed запись не заменяется. Один writer token
не Clone и не восстанавливается из journal для продолжения I/O. Перед будущим
coordinator side effect `effects_possible()` сохраняет отдельный uncertainty
marker; он покрывает **любое** последующее число writes, даже activation, пока
нет final outcome. Crash без нового progress не означает0переданных bytes.

`finish()` записывает NotStarted/Unknown/AwaitingReconnect, typed progress и
bounded diagnostic. Изменения сравниваются с последними известными **raw bytes**;
исчезнувший/изменённый/non-regular файл не заменяется. Temporary publication и
atomic replace используют существующие state-file helpers. No automatic unlink,
resume/retry, verified-success или unlock API на этом этапе нет.

Manager теперь проверяет journal **до allocate ID, backend open, actor spawn и
replay**, включая startup writes keepers. Неразрешённый valid record удерживает
все обычные attachments этого VID/PID; другой serial/path не считается возвратом
успешно обновлённого target. Corrupt/unsupported/oversize/redirected journal также
не разрешает attach и сохраняется. Другой supported target type в будущем не
должен выдавать чужой reconnect за success. Пока это консервативная SKU quarantine,
**не готовый identity-bound reconnect verifier**.

Уже живой actor не evict-ится только из-за публикации journal: discovery не должен
прерывать владельца transfer. Root detach/shutdown действует как прежде. После
удаления старого attachment, wake или запуска нового manager ordinary replay
удерживается до будущей явной resolution. NotStarted тоже пока не даёт unlock;
файл нельзя трактовать как разрешение автоматически восстановить settings.

Восемь private-file/MockBackend cases: drop/reopen record, effects marker и
terminal outcomes, atomic race двух независимых store instances (один winner),
malformed/future/schema/binding/counter/oversize input, symlink/external edit,
invalid transition, actual enumerate→zero backend opens при quarantine, сохранение
живого actor и запрет replug/restart replay. Power-loss/fsync fault injection и
сам fresh-version verifier этим не доказаны.

**В исходном journal checkpoint execution ещё не был связан.** Теперь internal
transport требует scoped writer token и сам сохраняет effects/final outcome
(следующий checkpoint ниже); production update entry point всё ещё не подключён. Coordinator должен: закрыть global admission и дождаться
старых jobs; выполнить checked quiescence; захватить свежий restore snapshot и
asset ownership; arm/mark durable intent **до hold/flash**, записать final outcome;
после reboot выполнить probe-only target/version verification без keepers/replay.
Нужны durable-safe resolution, recovery UI EN/RU и restoration policy.
На этом checkpoint journal сам по себе ещё не защищал от старого daemon или
удаления единственного marker. Device-settings anchor добавлен позднее (ниже);
qualified resolution остаётся открытым. Store API
и manager guard — foundation этого пути, не завершённый safe updater.

### Mandatory journal binding в transfer, 2026-10-15

[Checkpoint](evidence/2026-10-15-firmware-journal-binding.json). Internal
`HidCommand::FirmwareUpdate` теперь требует boxed owned `ArmedOperation`.
Создать его можно только через atomic journal arm; token не Clone/Deserialize
и не восстанавливается из сохранённого record для повторного I/O.

Actor tree привязывает HID actor к journal **своего PersistenceStore**. Low-level
actor, открытый без store scope, отказывает update без query/PWM/chunks. До первого
cmd134 transfer проверяет store root, VID/PID, serial/path/settings binding,
выбранный hold, Armed stage и неизменные raw bytes journal. Empty HID serial
нормализуется как singleton согласно существующему settings_key. Это metadata
binding исходного handle, не доказательство свежего post-flash reconnect.

После identity preflight, перед первым PWM transfer сохраняет `EffectsPossible`
с file/directory fsync. Один marker покрывает весь дальнейший upload/activation;
no snapshot не означает0writes. Modified/missing journal после query не даёт
дойти до hold. В тестовом handle journal заново читается при **каждом** PWM,
chunk и activation call и уже содержит EffectsPossible.

Перед ответом requester normal completion/transport failure записываются в
journal. Если final commit не удаётся, runtime result становится **Unknown**,
даже при619valid chunk echoes и activation reply; никакого restore/retry/replay
не следует. Caller drop не отменяет принадлежащий actor worker/commit. Panic
может оставить EffectsPossible без final record: он остаётся quarantined.
`EffectsPossible → NotStarted` запрещён; marker uncertainty не снимается
простой подачей no-effects outcome. Отвергнутый повторный/unscoped admission
оставляет его arm record unresolved и не переписывает progress принятого job.

Четыре новые transport cases покрывают wrong target/path/hold/store root,
missing/edited file до query, edit между query и hold, final commit failure после
всех ACK и disk reopen exact terminal progress. Пятый case — unscoped actor с
нулём writes. Полные synthetic transport fixtures теперь имеют619chunks (registry
budget journal), без proprietary bytes. Separate pinned image fixtures проверяют
checked quiesce→622reports→durable AwaitingReconnect на исходном handle.

Production coordinator всё ещё отсутствует: global admission, свежий restore
snapshot/assets, qualified settings guard resolution (anchor добавлен ниже), probe-only
fresh target/version verification, qualified resolution/restoration и EN/RU
Inspect/Apply/status/recovery UI остаются. Ни arm, ни AwaitingReconnect не являются
разрешением обычного manager replay. Hardware qualification отложена.

## Оставшийся M6 checklist

- [x] Убрать двойной cmd134 и проверять PN0/PN1; сохранить explicit unavailable
  при missing date, совместимость JSON и CLI, one-owner/FIFO semantics.
- [ ] Вернуться к readback на устройстве после разрешения hardware acceptance:
  версия/date/PN и latency при idle/media, repeat, missing/late PN1, reconnect.
- [x] GUI read-only preflight: device-bound version/raw date и unavailable;
  explicit read без нового периодического firmware polling, EN/RU и защита
  от позднего результата после смены устройства/соединения.
- [x] Найти официальный catalog route и получить точный кандидат1.6; сверить
  size/MD5, закрепить SHA-256 и добавить offline recognition с rejection checks.
- [ ] Завершить target/image qualification; доказать
  chip/model/hardware revision, expected version/size/hash, reject wrong target,
  malformed/truncated/oversize/unknown images до первого firmware write.
- [ ] Установить chunk/activation ACK layout, error/status значения и допустимые
  bootloader/recovery transitions по достоверному source или capture.
- [x] Source-derived FW1.6 ACK/error codec: exact request-prefix matching,
  raw activation extra flag, unknown errors и explicit ambiguity limits;
  без отправки команд. Это не закрывает wire/recovery qualification выше.
- [x] Internal actor-owned transfer и permanent attachment fence; pinned image/current
  identity preflight, typed progress, no retry и joined cancellation/shutdown.
  Не является готовым coordinator или product update path.
- [x] Разделить root/HID и keeper lifecycle; tracked sensor и joined barrier
  всех producers/settings на том же handle. Cleanup/save checked preflight
  реализован, global admission и coordinator ещё не готовы.
- [x] Протянуть active/queued LCD codec cleanup failures через barrier и добавить
  typed checked preflight по final save_state; failure не теряется после Idle.
- [x] Explicit requested cooling hold внутри transfer: pump MCU100%, optional
  owned controller fan100%; attempted/written flags, no flash after failed hold,
  no persistence overwrite/automatic restore. Effective cooling ещё не подтверждено.
- [x] Durable operation storage и pre-open manager guard: atomic first publication,
  effects/terminal records, no overwrite/resume/unlock; unresolved/corrupt record
  удерживает ordinary replay. Transfer/journal binding теперь подключён (ниже).
- [x] Mandatory scoped writer token в transfer: target/hold/file checks до query,
  fsynced effects marker до PWM, final commit до reply; commit failure → Unknown.
  No downgrade uncertainty, no unscoped update или автоматический retry.
- [x] Device-settings anchor до hold/flash: previous deny_unknown_fields reader
  отвергает guarded field; missing journal с guard остаётся quarantined; обычные
  settings writes/profile transactions не снимают marker. Private-file/manager tests.
- [ ] Qualified no-effects/verified resolution settings guard и journal без silent
  reset настроек; до реализации clearing API отсутствует.
- [x] Serialized manager handoff после in-flight attach/replay; non-Clone owner
  сохраняет исходный HID, checked join peers/target и manager shutdown receipts.
- [x] Internal owned prepare/transfer/close pipeline с fresh bound snapshot/assets,
  checked staging/keeper/drain и requested hold; observer loss не abort job.
- [ ] Реализовать product updater job: explicit Inspect/Apply, identity-bound exclusive
  HID ownership, quiesce/join LCD/cooling/RGB/sensors, bounded input/progress,
  joined shutdown и запрет автоматического повторного flash после ambiguity.
- [x] Internal probe-only helper: observed detach, unique same-serial candidate,
  bounded single-query exact identity и неизменные journal/guard; без replay/unlock.
- [ ] Coordinator reconnect/version verification с тем же target, восстановление сохранённого
  intent только после подтверждённого результата; unknown outcome остаётся явным.
- [ ] Synthetic fault matrix: до первого chunk, между chunks, lost ACK,
  unplug/shutdown/close, activation reply loss, другой same-SKU device, старый
  version и failed reconnect. Не переносить неопределённость в success.
- [ ] После программной приёмки и разрешения hardware tests выполнить отдельный
  аппаратный план. Firmware fault injection на единственном AIO не выполнять.

Safe LCDV2 updater всё ещё отсутствует; этот срез закрывает identity readback
software defect и устанавливает следующие проверяемые зависимости.

### Device-settings firmware anchor, 2026-10-15

Перед `EffectsPossible` transfer атомарно сохраняет `firmware_update_guard` в
canonical device TOML и fsync-ит файл/каталог. Guard содержит version,
operation ID и pinned image SHA-256; сохраняются текущие disk settings, а не
присланный restore snapshot. Он появляется после identity preflight, до первого
PWM/chunk/activation. Armed-only операции без possible effects пока опираются
на journal. Ошибка anchor не допускает effects marker и transport side effects.

Guard использует существующий state gate того же PersistenceStore. Обычные
settings writes (включая запись прежнего snapshot без guard) и profile transactions
с guarded current/proposed settings запрещены. Marker остаётся после любого
terminal outcome; удаления API пока нет. Asset references и saved cooling/LCD
intent сохраняются. Manager до HID open проверяет guards в devices TOML для LCDV2,
поэтому потеря journal и изменение serial/path не разрешают replay. Malformed
settings при этом fail closed; повреждённые файлы сохраняются.

Прежняя полная DeviceSettings serde schema с deny_unknown_fields принимает
unguarded TOML и отвергает новый field. Это проверка audited previous reader,
не гарантия для произвольных старых версий, игнорирующих unknown fields. Удаление
одновременно journal и guarded settings администратором этим не обнаруживается.
Concurrent external writers и coordination всех уже допущенных jobs требуют
оставшегося global admission/coordinator; anchor не заменяет checked quiescence.

[Evidence](evidence/2026-10-15-firmware-settings-anchor.json). Проверки только
private files/MockBackend; hardware, GUI updater и qualified resolution ещё открыты.

### Probe-only reconnect evidence — 2026-10-15

`actor::firmware_probe::ReconnectProbe` — внутренний synchronous helper для
будущего coordinator, не ordinary manager attachment. Он принимает только
валидный `AwaitingReconnect` record с canonical settings guard, совпадающим
operation ID/SHA. `Unknown`, `NotStarted`, Armed и serial-less targets не могут
получить автоматическое evidence. Serial-less сценарий требует отдельной
qualified recovery policy; hidraw path недостаточен для физической идентификации.

Guard читается отдельным bounded read-only путём без `StateFiles::access()` и
implicit redo. Pending profile/settings transaction запрещает probe до open,
файлы остаются неизменными. Ordinary profile recovery дополнительно отвергает
before/after snapshots с firmware guard: redo не может снять marker.

Probe сам получает enumeration от backend. Сначала нужно наблюсти отсутствие
целевого VID/PID/serial; наличие нового path без наблюдённого отсутствия не
считается reconnect. Ошибка enumeration не считается отключением. После этого
допускается ровно один candidate с тем же serial. Duplicate serial блокирует
open. После restart observation не восстанавливается из journal: уже attached
устройство не доказывает свежий reset. Быстрый reset, пропущенный enumeration,
также не угадывается успешным; coordinator/event/manual-reconnect policy ещё нужна.

Новый handle выполняет bounded drain (до10read0), один cmd134 и bounded
PN0/optionalPN1 collection. Требуется точный researched identity token1.6,
а не только user version. До query и после чтения снова проверяются неизменные
journal raw bytes и guard; после закрытия handle — уникальность и тот же candidate
в enumeration. Evidence выдаётся только один раз. При любом fault handle закрыт,
settings/journal не переписываются, restore/reflash не выполняется.

Evidence содержит operation ID, observed DeviceInfo и parsed identity/date.
Это не installed-image hash, hardware revision proof или разрешение снять guard.
В отличие от полноценной update job helper не закрывает старый actor owner,
не обеспечивает global exclusion, не сохраняет recovery decision и не запускает
product UI. Coordinator обязан сначала join old HID owner и исключить остальных
writers; observe выполняется в joined blocking job. Synchronous backend open/write
не могут быть принудительно прерваны. Qualified resolution и serial-less recovery
ещё открыты. [Software evidence](evidence/2026-10-15-firmware-reconnect-probe.json).

#### Global admission: следующая integration boundary

Одного mutex вокруг IPC method недостаточно: отменённый RPC может оставить
`spawn_blocking` или owned monitor работающим. Перед product Apply нужен общий
close/drain owner с отказом новых ordinary jobs и учётом потомков уже принятых:

- IpcServerImpl methods, returned BoxFuture theme/preview jobs и theme_jobs permits;
- ArchiveTransfers handle/blocking worker/reaper;
- GroupUploads work/Finish/owned monitor/cancel/expiry/publication cleanup;
- MediaExports finite stream/read worker и pins, upload/font staging;
- DisplaySettings owned blocking save, settings/profile transactions и asset GC;
- manager discovery attach/replay, power transitions и старые per-device keepers.

Lease должен жить в самом worker/stream/monitor до его конца, включая caller drop
и worker panic cleanup. Already-admitted parent должен передавать ownership своим
потомкам; нельзя заставить его заново проходить закрытый внешний admission и
признать drain завершённым раньше дочернего worker. Shutdown не может зависеть
от ordinary admission. После drain нужны checked keeper quiescence, fresh settings
snapshot + asset pins, journal arm, transfer на исходном HID owner, joined teardown
и только затем probe. Закрытие gate само по себе не позволяет drop/reopen исходный
handle до завершения transfer. Этот перечень — integration requirements, не
реализованный общий barrier; существующие отдельные semaphores его не доказывают.

### Shared ordinary-work admission и worker leases — 2026-10-15

`PersistenceStore` теперь владеет одним process-local Admission; clones store,
IpcServerImpl и manager используют тот же gate. `close()` атомарно запрещает
новые entry и возвращает drain observer; drop/отмена observer не открывает gate.
Уже принятые группы задач держат lease до последнего worker/monitor child.
Повторное закрытие допустимо. Автоматического reopen API и product close route нет.
Gate не заменяет durable journal/settings guard после process restart.

69 корневых DaemonRpc methods проходят admit; пять convenience/delegating methods
используют эти же guarded methods. Возвращаемые theme/preview futures и blocking
asset/font/media/GC/profile workers удерживают tracked capacity permit. Он
передаётся через multi-stage preparation и composition до существующего момента
release. MediaExports finite stream удерживает permit до последнего source read,
включая drop consumer во время blocking read. Passive cache snapshot subscriptions
не получают lifetime lease; их producer завершается joined keeper teardown.

К тому же gate подключены pools ArchiveTransfers, GroupExports, GroupUploads
(work/Finish/cancel), DisplaySettings и SystemSpec. Owned panic monitors/cleanup
копируют child lease; FinishSource явно переносит lease в blocking prepare.
Reapers регистрируют отдельную cleanup iteration и прекращают новые iterations
при закрытии; их sleep loop не мешает drain. Saturated capacity не оставляет
лишнего lease. Новые child capacity acquisitions после seal fail fast; уже owned
monitor/cleanup продолжает работу на inherited lease.

Manager admission покрывает enumerate/diff/attach/replay и sleep/wake transition.
После seal обычный polling не открывает устройства, power transition возвращает
ошибку до изменения состояния. Shutdown остаётся доступен и joins teardown.
Уже работающие keepers/HID owner этим gate не останавливаются: их checked quiesce,
FIFO drain и joined ownership — отдельный следующий шаг coordinator.

[Software evidence](evidence/2026-10-15-firmware-admission.json) включает atomic
close/enter race, cancelled waiter/worker, owned IPC preparation и реальный
JSON-RPC memory transport: Busy без новых storage/HID effects, closed discovery,
power refusal и joined shutdown. Все файлы private, backend Mock.

**Полный drain boundary ещё не закрыт.** Async destructor cleanup в
Publication::Drop и file/data Drop helpers пока вне общего lease lifecycle;
нужны explicit staging/publication close-and-join, idle transfer invalidation,
extended monitor/reaper/fault tests и bounded drain policy. Отдельно нужны update
owner admission, fresh snapshot/assets, power/shutdown cancellation policy,
coordinator transfer/reconnect/resolution и EN/RU UI. `active == 0` сейчас
доказывает завершение только перечисленных enrolled owners, а не всех задач
демона; его нельзя использовать как разрешение product Apply.

Во время повторной квалификации один manager sleep/wake fixture остался живым
без завершения; это не засчитано успешным прогоном. Его test-only HID gate мог
бесконечно держать blocking call при unwinding. Добавлены release-on-exit guard,
10s deadline fake write и отдельный panic-exit regression; исходные assertions/
короткие ожидания не ослаблены. Исходный триггер того зависания не установлен,
первый workspace проходил, отдельный запуск под gdb stall не воспроизвёл. После
hardening32/32 отдельных sleep/wake repeats прошли (лог в evidence). Зависший
private test остановлен отдельно, лог сохранён; итоговая квалификация выполняется
после hardening на новом source snapshot. Это не hardware или production fix.

### Lifetime cleanup owners и staging close — 2026-10-15

Publication, named-archive sessions, GroupUploads/GroupExports entries и media
upload sessions регистрируют lifetime lease **до завершения создающего job**.
Idle entry продолжает удерживать lease до уничтожения backing file; поэтому
async Drop не может появиться после якобы завершённого drain. Drop переносит
owner в blocking cleanup, его monitor удерживает child до конца. Publication
unknown/keep/finish errors и panic записываются в sticky bounded cleanup health;
`wait_checked` отказывает даже после active0. File/data disposal также enrolled.
После startup это process-local health, не замена durable journal/recovery.

`Admission::begin_maintenance()` атомарно seal-ит gate и выдаёт единственного
scope-bound Maintenance owner. Его child позволяет закрыть уже существующие
ресурсы после seal; foreign owner отвергается до изменений. Drop/cancellation
не открывают gate. Checked wait сначала освобождает owner, потом ждёт всех
children и проверяет cleanup health. Лёгкий `close()` остаётся observer API и
не выдаёт maintenance privileges; product job должен владеть Maintenance.

`IpcServerImpl::close_staging` — internal method, не JSON-RPC route: запрещает
новые font/media sessions, очищает их buffers/writers, cancels и удаляет named
archive/group registries. Existing file/packer/import workers удерживают refs и
leases до completion/cleanup. После закрытия registry новые entries не могут
зарегистрировать lifetime owner; delayed old requests также не создают скрытый
новый staging. Всё снятие backing files происходит на owned blocking workers.
Durable settings, library/assets и receipts не сбрасываются. Session handles
после maintenance не сохраняют продолжение старой upload transaction.

Finite media exports остаются lazy: ровно один read на demand consumer, без
prefetch. Shared reader owner/cancel watch позволяет maintenance забрать idle
reader даже при живом stream, переставшем читать (в том числе при blocked socket
writer). Stream Drop и последний frame/error сразу снимают idle source pin;
дескриптор закрывается отдельным enrolled cleanup worker. При незавершённом
blocking read captured owner/pin остаётся у worker до фактического завершения.
Ordering, final-frame release и late-corruption contract сохранены; ранний
prefetch вариант нарушил две существующие интеграции и был заменён этим путём.

[Evidence](evidence/2026-10-15-firmware-cleanup-owners.json): cleanup после seal,
caller/waiter loss, I/O failure/panic, scope mismatch; unknown publication drop
и повреждённый recovery report; cancelled active group import/export; idle finite
export и blocked read; реальный IPC/private staging с закрытием anonymous FDs и
сохранением исходного каталога. Hardware/native GUI не запускались.

**Следующая integration boundary:** owned coordinator должен начать maintenance,
закрыть staging и quiesce/join keepers, затем выполнить checked drain. Нельзя
сначала бесконечно ждать active0, оставив registry/actor-owned publication refs.
Нужны bounded wait/cancellation с сохранением one-way fence, проверка unresolved
import/publication recovery, fresh serialized snapshot + asset ownership, original
HID FIFO ownership/transfer, joined teardown/probe и qualified journal/settings
resolution. Async backend read/write нельзя force-interrupt. Encoder/actor cleanup
по-прежнему проверяется keeper lifecycle health; Gate health относится к enrolled
cleanup owners. Product updater/reopen UI EN/RU ещё отсутствует.

### Serialized manager handoff/original HID owner — 2026-10-16

`ManagerHandle::take_for_firmware` принимает scope-bound Maintenance capability
и ставит запрос в отдельную manager queue. Он исполняется в том же loop, что
poll/attach/replay/power, после завершения текущего pass. Нельзя взять target
из live map, оставив незавершённый attach/replay другого устройства за barrier.
Foreign scope отвергается до enqueue, missing/stale DeviceId не переназначается
на новую attachment, repeated claim не выдаёт второго owner.

Manager хранит исходный DeviceInfo в ManagedDevice. При handoff target и peers
выходят из ordinary map/by_key и передаются в non-Clone FirmwareOwner. Это те же
actor trees и HID handles: no reopen, новый replay не запускается. Global seal
останавливает дальнейшую discovery. Scope включает peers, поскольку global
assets/publications могут удерживаться их actor’ами. Это приостановка keepers,
не изменение их saved cooling/LCD intent.

`FirmwareOwner::quiesce_checked` останавливает/joins всех producers concurrently,
собирает ошибки и проверяет save state; failure target не отменяет join peers.
Target HID actor остаётся открытым для будущего transfer. Owner не держит ordinary
admission lease: иначе checked drain ждал бы самого HID owner, который должен
пережить этот barrier. Request child lease живёт до обработки/delivery, а actor
quiescence остаётся отдельным обязательным gate.

`close` посылает root stop всем и ждёт teardown receipts до fresh probe. Drop и
потеря requester/reply тоже посылают root stop, без auto-replay/reclaim. Manager
сохраняет stop/join receipts после удаления из карты: daemon shutdown ждёт handed-
off transports даже при живом owner, отменённом close waiter и blocking write.
Синхронный backend write невозможно force-interrupt; join ждёт его завершения.
Потерянный receipt возвращает ошибку owner.close (manager shutdown пишет diagnostic).

[Evidence](evidence/2026-10-16-firmware-manager-handoff.json): blocked peer attach/
replay, scope/missing/duplicate refusal, requester loss после реального enqueue,
owner-close cancellation, joined shutdown с live owner/blocked I/O и per-device
save failure. External pinned vendor image отдельно прошёл real-manager handoff
→ checked quiesce → journal arm →622reports/619chunks → durable AwaitingReconnect
на исходном handle; saved fan policy сохранён. Всё на private store/MockBackend.

Это manager ownership primitive, не законченный updater coordinator. Ещё нужны
owned job orchestration с staging close/drain и fresh snapshot/assets, bounded
cancel/power/shutdown policy, unresolved import recovery, probe и qualified guard/
journal resolution/restoration. Один claim/seal на процесс; reopen policy пока
не реализована. Version/ACK mock result не равен hardware image qualification.

### Owned coordinator: prepare → transfer → joined close — 2026-10-16

`IpcServerImpl::begin_firmware_update` — internal Rust API, не JSON-RPC/Tauri
Apply. Требует заранее распознанный ValidatedImage и explicit CoolingHold;
проверяет manager/store admission scope **до seal**, затем создаёт owned job.
Клиент получает status/progress watch и completion receiver; drop observer/waiter
не отменяет job. Explicit cancel остаётся watch-сигналом, живущим вместе с job.

Путь соединён: scoped maintenance → serialized handoff → staging close → checked
join всех keeper’ов → checked drain → fresh serialized settings snapshot и asset
ownership → journal arm → original actor transfer → joined owner close. До первого
hold все stages должны пройти. Snapshot читает target и peers после dispatcher
flush и global drain, проверяет named Screen profiles, current themes/media и
collections; image/media pins и font bytes удерживаются через transfer/teardown.
Bound: ≤2048 distinct refs, ≤64MiB font bytes. Encoder/render subprocess не нужен.
Remaining publication scopes/temporaries или повреждённые histories запрещают
arm; исторические retention notices сами по себе не объявляются новой ambiguity.

Coordinator не принимает caller-supplied restore settings. Его arm включает
`require_fresh_snapshot`: повторно сравнивается disk intent, затем ещё раз под
state gate при публикации settings guard **после identity query, до effects marker/
PWM**. Changed intent не перезаписывается и запрещает hold. Low-level transport
fixtures остаются caller-snapshot primitives; этот stricter gate обязателен в
coordinator path. Формат durable record не меняется; writable token не recovery API.

При drain deadline30s job помечает отказ transfer, но продолжает join уже owned
работы; timeout не отрывает blocking I/O и не открывает gate. Cancel, замеченный preparation, даёт NotStarted без arm; более поздний cancel до
hold может оставить NotStarted journal. После possible hold — Unknown с guard/journal,
без rollback/retry. Ошибки prepare закрывают HID и ждут cleanup; ошибки/lost reply
после отправки transfer не превращаются в success. Terminal report появляется
после joined original-owner close; даже при всех619ACK result остаётся
AwaitingReconnect. Effective duty, installed-image hash и hardware revision этим
не доказаны. Restore/reopen и ordinary replay не выполняются.

[Evidence](evidence/2026-10-16-firmware-owned-coordinator.json): accepted intent
flush до snapshot, custom asset pin/missing asset, stale settings до marker,
pre-transfer cancel и unresolved import scope. External pinned image на реальном
IpcServerImpl/manager/actors с MockBackend отдельно проверяет обычный terminal
report, потерю observer, cancel до hold и cancel после hold; original handle
закрыт во всех случаях. Manager/store mismatch не seal-ит чужой scope.

Process-local registry/status/shutdown supervision добавлены следующим checkpoint.
Далее: durable restart job/status/recovery API, extended panic/power/shutdown matrix,
общие bounded waits, probe после actual teardown, qualified guard/journal
resolution и restoration/restart policy (включая serial-less/missed reset),
Inspect/Apply/status UI EN/RU. Snapshot/transfer engine не подключён к product UI
и не завершает safe updater. Потеря coordinator completion owner возвращает
unconfirmed error; расширенная supervisor/fault matrix ещё открыта.


### Retained job и shutdown/panic supervision — 2026-10-16

Одна job на процесс сохраняется в `IpcServerImpl`: status/progress/cancel и terminal
completion доступны через новый observer даже после drop исходного клиента.
Completion watch хранит полный report; повторный wait не теряет outcome. Registry
не удерживает maintenance lease и не создаёт self-deadlock normal drain. Это
process-local состояние; durable operation journal/guard остаются отдельными.

`shutdown_firmware` под тем же lock запрещает новый begin, посылает explicit cancel
и ждёт terminal report. Drop shutdown waiter не отменяет supervisor; повторный
shutdown снова join-ит ту же job. `main` сохраняет typed server и вызывает этот
путь до manager shutdown. Без job закрывается только registry; ordinary admission
не seal-ится. Begin проверяет runtime/scope/registry до maintenance seal.

Supervisor владеет JoinHandle worker. Panic/cancellation worker приводит к Closing,
затем закрытию staging, manager root join и checked drain tracked cleanup, результат
всегда Unknown без guessed transfer outcome. Отдельная one-shot recovery authority
не держит drain count во время worker; после его join она создаёт cleanup lease.
При panic staging cleanup manager всё равно останавливается, report помечает cleanup
unconfirmed. Journal/settings guard не снимаются, replay/restore/reflash не запускается.
Lost supervisor completion остаётся явной unconfirmed error. Sync HID calls нельзя
принудительно прервать: shutdown ждёт владельца, не объявляет timeout успешным join.

[Evidence](evidence/2026-10-16-firmware-job-supervision.json): worker panic после
effects marker сохраняет raw journal/settings guard и joins original root; normal
drain не ждёт recovery authority; shutdown waiter loss и повторный status/join.
External pinned Mock fixture проверяет reattach после observer loss, shutdown до
hold и после hold, retained outcome и один open/close исходного handle во всех4modes.
Реальное устройство, native GUI и работающий production daemon не запускались.

Остаются durable restart/status recovery и product RPC/Tauri EN/RU UI, extended
power/panic/cleanup/blocked-I/O fault matrix и deadline policy, подключение reconnect
probe после joined teardown, qualified resolution/restoration и serial-less/missed
reset policy. Process-local supervision не квалифицирует hardware/image и не делает
AwaitingReconnect успехом прошивки.


### Saved recovery inspection в IPC/CLI — 2026-10-16

`get_firmware_recovery({})` и `lconnectctl [--json] firmware-recovery` читают сохранённый
journal и settings guards без DeviceId/подключённого устройства, в том числе при
закрытом ordinary admission. RPC принимает только пустой object; path/device/
clear параметры и arrays отвергаются. Два независимых inspection permits ограничивают
blocking readers; permit живёт в worker при drop RPC caller. Read-only endpoint
не участвует в ordinary drain и не является новой write capability.

State хранится на диске прежним journal/guard format; реконструкция read-only:

| State | Смысл |
|---|---|
| `no_saved_operation` | В прочитанном snapshot нет journal/guards; не разрешение Apply |
| `journal_only` | Armed/NotStarted record без guard; отсутствие marker не доказывает отсутствие writes |
| `guarded` | Единственный guard совпадает по key/operation/image с valid journal |
| `guard_only` | Есть settings guard, journal отсутствует; исход неизвестен |
| `inconsistent` | Не совпали markers, несколько guards или effects-stage без guard |
| `unreadable` | Corruption, symlink, resource bound, pending transaction или I/O не дали проверить состояние |

Report содержит persisted operation ID/stage/target/image hash, last durable
chunk/hold/activation progress и guards. Старый HID path — только historical value;
identity не перепроверяется. Даже saved AwaitingReconnect не превращается в success.
Во время live preparation до arm report может быть `no_saved_operation`; это
**saved artifacts**, а не live job phase/quiescence/action authorization.

Inspection берёт общий state mutex через отдельный read-only access: проверяет,
что нет pending settings/profile transaction, **не вызывает redo**. Directory/file
reads ограничены: ≤256 directory entries, existing journal≤2MiB и settings file
bound. Читаются regular no-follow files, validation schema/guard остаётся строгой;
journal raw сравнивается до/после guard scan. Unknown/malformed исход не лечится
удалением или перезаписью marker. Missing store не создаётся. При несовпадении
из-за concurrent publication следующий read может дать новый согласованный snapshot;
текущий report не разрешает вмешательство в transfer.

[Evidence](evidence/2026-10-16-firmware-recovery-inspection.json): пять persisted
stages после нового store instance, journal loss, missing/mismatched/extra guard,
corrupt/oversized/symlink files, valid pending redo transaction и directory budget.
Real CLI → Unix socket → daemon работает с private store/MockBackend и sealed gate,
без обычного device lookup. Raw journal/settings сохранены после RPC и CLI.

Apply/probe/resolution/clear/retry RPC не добавлены. Следующие работы — live
job/status и product UI EN/RU, owned reconnect probe после joined old owner,
qualified recovery/resolution/restoration, расширенные fault/power/restart policies.
Saved snapshot не возобновляет старый transfer и не квалифицирует firmware image.


### Owned reconnect job после joined original close — 2026-10-16

`begin_firmware_reconnect` — internal explicit action после terminal
AwaitingReconnect основного transfer job. Ordinary map уже пуст и admission
закрыт; `FirmwareOwner::close_for_reconnect` создаёт non-Clone owner только после
checked original root teardown receipts. Он сохраняет исходный backend, admission
scope, original target и operation ID. Journal сам не восстанавливает эту capability.
Target/path/serial/settings key/operation должны совпасть с original closed owner
перед началом probe; затем low-level helper проверяет неизменные journal/guard.

Job сохраняется в registry отдельно от terminal transfer report. Duplicate start,
start до acknowledged transfer/close, после shutdown или без owner отвергаются.
Observer loss не отменяет job; reattach доступен через `firmware_reconnect_job`.
Исходный transfer report остаётся AwaitingReconnect даже после identity observation.

Целый synchronous observation loop принадлежит одному blocking worker. Window30s,
enumeration cadence200ms и cancel checks20ms между calls; deadline/cancel после
долгого вызова не выдают fresh evidence. Ожидание detach/reconnect не открывает HID.
После actual absence допускается один fresh same-serial handle и только cmd134;
новый hidraw path сам по себе detach не доказывает. Ошибка probe не вызывает retry.
`shutdown_firmware` под registry lock запрещает start, отменяет оба jobs и ждёт
actual blocking worker; drop shutdown waiter не отрывает его. Синхронный backend
write/open нельзя force-interrupt;30s — observation window, не обещание остановки I/O.

Terminal states: FreshIdentityObserved / Cancelled / TimedOut / Failed. Worker panic
даёт Failed с unconfirmed diagnostic после join; открытый probe handle уничтожается
при unwinding. Исторические target/identity данные в report клонируются для чтения,
не являются writable proof, success/installed hash/revision qualification или правом
снять quarantine. Journal/settings markers, saved intent и original transfer outcome
не меняются. Product RPC/Apply/live probe UI, persisted evidence/resolution, restart,
serial-less и missed fast-reset policies по-прежнему отсутствуют.

[Evidence](evidence/2026-10-16-firmware-owned-reconnect.json): real manager handoff →
checked quiesce/close → owned probe с synthetic acknowledged journal, scope/operation
binding, timeout без observed absence, observer loss, panic и shutdown-waiter loss
при blocked fresh-handle write. Отдельный external pinned Mock fixture соединяет
полный619chunk transfer → joined original close → observed detach → fresh identity;
на новом handle ровно один cmd134, без ordinary replay. Hardware не проверялось.
