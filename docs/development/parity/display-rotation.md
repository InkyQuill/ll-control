# Поворот LCD: JPEG и H.264

Baseline: Windows L-Connect3 **2.1.28.0**, Galahad II LCDV2. Срез2026-10-14
реализует software rotation финального H.264 composition для0/90/180/270°.
Старт/replay managed media и streaming themes, group import/reapply подключены.
Смена угла уже работающего composition подключена без замены worker.
Direct managed media0° также переходит к software rotation с сохранением
следующего кадра. LCD-013 остаётся partial до расширенной native/physical приёмки.
Реальное устройство в этом срезе не запускалось.

## Windows source и Linux контракт

В versioned decompilation
`L-Connect-Service/LConnectService/Products/Galahad2Vision/Galahad2VisionController.cs`
`setScreenRotation` присваивает RotateFlipType для четырёх углов (`:758-765`).
Итоговый browser bitmap поворачивается перед `screenLibAV.EncodeImage`
(`:1178-1184`). Поворот применяется и к фону, и к overlay. Это source-derived
контракт; одного hardware rotation для H.264 недостаточно для доказательства parity.

`lconnect_image::FrameRotation` принимает только0/90/180/270 и поворачивает RGB
по часовой стрелке без resampling. `ThemeRuntime.output_rotation` — transient
параметр presentation, отдельно от portable LcdTheme. Он применяется после
рендеринга всех layers/weather/sensor values, непосредственно перед encoder input.
None/0 сохраняет прежний путь без дополнительного image copy. Поворот не меняет
sensor bindings, локальные координаты editor или сохранённые image/media files.

| Source | Подготовка output | LCDControl rotation | Observable state |
| --- | --- | --- | --- |
| JPEG/theme | Обычный JPEG | Сохранённый угол | Requested/applied угол пользователя |
| H.264 theme/composition | Поворот финального RGB перед encoding | Всегда0 | Effective requested/applied угол пользователя, mode LCDSetting |
| Managed media, старт при0° | Direct AU path до первого поворота |0 | Media ID и DiagnosticClip |
| Managed media90/180/270° | Runtime-only empty theme с video background и software rotation |0 | Прежний Media ID/DiagnosticClip; get_lcd_theme возвращает null |

Synthetic media wrapper не записывается как пользовательская theme. Source
остаётся media clip и восстанавливается тем же ID после daemon restart. Ключи
и байты оригинальных assets не заменяются повёрнутыми copies. JPEG preview
редактора по-прежнему показывает canvas до mount rotation.

Ориентация prepared composition сверяется с requested angle LCD actor до
изменения source. Если угол сменился во время подготовки, старый результат
отклоняется и worker stop/join выполняется. Brightness change при уже подготовленном
угле работает, не меняя ориентацию или media identity. Возврат от rotated H.264
к JPEG восстанавливает hardware rotation, чтобы JPEG не стал выглядеть как0°.

Manager replay использует тот же output angle для media/themes. Group import
сохраняет полный desired профиль и receipt до первого config/frame; group
reapply после restart восстанавливает runtime из текущих settings. Остальной
[контракт группового переноса](profile-groups.md) сохраняется.

## Смена угла во время composition

`patch_lcd_config` меняет typed atomic output angle существующего runtime.
Video decoder/encoder, media position, sensor bindings и scene timeline не
пересоздаются. Generation источника сохраняется. При изменении только угла
LCDControl не отправляется: аппаратный угол H.264 уже0. Если одновременно
изменена brightness, software angle применяется после успешного LCDControl ACK.
Desired угол сохраняется обычным settings owner и используется при restart.

Число кадров ограничено общим budget4 от render до выдачи AU потребителю.
Один только mpsc prefetch2 не ограничивал накопление в FFmpeg/OS pipes.
После ACK могут выйти несколько ранее подготовленных кадров; ACK не означает,
что устройство уже показало новый угол. Следующий AUD нужен для выделения
предыдущего AU; budget допускает такой lookahead. Ожидание budget отменяется
при stop, все codec tasks/children join выполняются прежним shutdown путём.
Тест с остановленным потребителем проверяет переход не позднее6 кадров после
смены угла в worker; actor-held/prepared frame и аппаратные buffers проверяются
отдельно. Это ограничение очереди, не измерение аппаратной latency/FPS.

## Direct media: первый переход от0°

LCD actor останавливает и join-ит direct reader, получает его source и индекс
следующего **выданного потребителю**, а не prefetched кадра. Новый decoder
начинает с последнего keyframe перед этим индексом. Для позднего GOP сначала
декодируется AU0 с SPS/PPS: повтор этих headers в каждом keyframe не требуется.
Эта priming picture также отбрасывается. Промежуточные reference
pictures декодируются, но не рендерятся/кодируются/отправляются на LCD и не
расходуют output budget. Первый AU нового encoder — keyframe нужной позиции.

Подготовка serial внутри LCD owner и ограничена обычным10-секундным codec
receive timeout. Shutdown прерывает ожидание, cleanup join выполняется до выхода.
Во время подготовки вывод кратко приостанавливается; длительность на устройстве
ещё не измерена. Никакого требования мгновенного видимого поворота не заявлено.

До успешной подготовки сохраняется резервный source handle. Для indexed media
копируются fd/index; reads используют positional I/O, поэтому общий cursor
клонированного descriptor не влияет на чтение. Legacy payload разделяется через
Arc без копии всего видео. При decoder/config failure новый worker останавливается,
direct reader восстанавливается на том же следующем кадре; requested/applied угол
и settings не меняются. При shutdown после config ACK новый AU не отправляется.

Успех сохраняет generation, counters, media ID/pin, deadline и started time;
обычный settings owner сохраняет новый угол. Angle-only переход не отправляет
LCDControl. Combined brightness/angle сначала ждёт config ACK. Повторный0°
оставляет этот runtime в composition до Clear/нового Apply; restart при сохранённом
0° снова использует direct path. Portable theme/asset bytes не создаются и не меняются.
Raw diagnostic clip остаётся0° — это отдельный диагностический инструмент.

## Программная проверка

- Pixel mapping всех четырёх поворотов на нессиметричном3×2 RGB raster проверяет
  направление, dimensions и сохранение каждого pixel.
- Actual FFmpeg video decode → live sensor overlay → rotate → H.264 encode →
  decode: для четырёх углов проверены четыре разноцветных угла кадра, область
  sensor text и изменение glyph pixels после нового значения датчика.
- Group media90/180/270°, прежний media ID, отсутствие synthetic theme в API,
  brightness change, fresh actor + receipt reapply и wire rotation0 проверены.
- Реальный private IPC/manager с FirmwareMock проходит saved media0° → live rotation → clear/
  live rotation270°/Apply → JPEG → media → restart media → streaming theme → restart
  theme. Desired source/config сохраняются; hardware streaming rotation0 и
  восстановленный JPEG hardware270 проверены в protocol packets.

- Live video с четырьмя временными маркерами:64 actual decoded H.264 frames,
  все четыре угла, остановка потребителя перед изменением. Проверены непрерывная
  фаза видео и ограниченный хвост кадров со старым углом.
- Private IPC на direct media0° → composition и streaming theme: четыре live угла,
  неизменная generation, requested/applied и отсутствие лишних LCDControl.

- Direct reader handoff на non-IDR кадре: предыдущий IDR прогревает decoder,
  SPS/PPS только в AU0, actual decoded H264 pixels продолжают фазу; чтение клонированных
  indexed sources проверено параллельно. Decoder/config failure и shutdown сохраняют
  старый угол/позицию без settings delta и без нового AU.

Workspace996 passed/2ignored; strict Clippy/rustdoc и fmt прошли.

[Текущий direct transition evidence](evidence/2026-10-14-display-direct-rotation.json).
Предыдущие start/replay/group и live composition checkpoints:
[исходная rotation evidence](evidence/2026-10-14-display-rotation.json),
[live composition evidence](evidence/2026-10-14-display-live-rotation.json).
Их source hashes относятся к соответствующему историческому срезу.
Это software/codec evidence; видимый поворот LCD и скорость на устройстве
требуют аппаратной проверки.

## Остаётся для LCD-013

- Расширенная native control/error/cancel приёмка; codec startup pause и
  аппаратные buffers/latency. Generation, queue bound, decoder seek, config
  failure rollback и shutdown до первого AU проверены программно.
- Сопоставить editor/live preview semantics с Windows; native controls и errors.
- Совместная нагрузка, rotated long-video/restart/sleep/detach и physical check
  четырёх положений. Hardware acceptance отложена пользователем.
- Raw diagnostic H264Clip без managed media identity остаётся rotation0;
  не выдавать поддержку managed media за поддержку этого отдельного инструмента.
