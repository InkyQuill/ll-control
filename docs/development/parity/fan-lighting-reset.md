# Сброс FanAndLighting LCDV2

Baseline: Windows L-Connect3 2.1.28.0 / Galahad II LCDV2 0416:7395.
Сверено2026-10-17. Daemon/Unix IPC/CLI и GUI реализованы; расширенная native
приёмка остаётся открытой. Это software reset группы, не firmware factory reset.

## Windows reference и defaults

Versioned source в соседнем `reference-decompiled/2.1.28-2026-09-08`:

- `L-Connect 3/LConnect3cs/Products/Galahad2Vision/Galahad2VisionController.cs:203-231`:
  создаёт новый Galahad2VisionProfile и передаёт Fan/Pump, fan topology/sync,
  selected LED modes и полный набор inactive profiles через processNewProfile.
  Screen и остальные группы не заменяются.
- `Galahad2VisionProfile.cs:153-280,304-307,345-352,659-673,741-842`:
  Fan StandardSpeed, Pump PWMQuiet, пользовательские fan curves Quiet,
  Rainbow для обеих LED-зон, fan360/24LED/sync=false.
- `L-Connect.Core/LConnectCore/Products/Galahad2Vision/PumpLightingSetting.cs:7`:
  исходный scope Inner. Конструктор controller service хранит этот scope;
  ScreenLEDLighting меняет mode/static/dynamic параметры.

Используются уже сверенные `FanPreset::Standard`, `PumpPreset::Quiet` и
`lighting_catalog`: brightness100, speed75 для Rainbow, forward/Inner;
все6 fan и16 pump manual mode defaults восстановлены. Active Dynamic выключен,
старый source/unit registry очищен: при следующем выборе source создаются
существующие defaults без выдуманного Linux ID. Все4 fan slots возвращаются
к Quiet;4 добавленных в Linux pump slots — к Quiet RPM с исходными именами.

Windows CPU binding в Linux требует явного stable temperature sensor ID.
Active curves и полный catalog8/3 используют hysteresis0°C/fallback100.
Восемь named Linux slots сохраняют отдельный прежний контракт hysteresis2°C; missing/stale source отражён
в cooling readback и не заменяется другим датчиком. Thermal alert policy — global
Linux policy: сохраняется, включая её возможный приоритет над обычным Rainbow.

Стандартный catalog и отдельное восстановление режима/имени описаны в
[контракте cooling modes](cooling-modes.md#factory-mode-restore). Полный Reset
заменяет обе banks, включая пользовательские имена, и снимает старые CAS tokens.
Активные policy и ID восстанавливаются вместе; FixRPM в bank —2200 RPM.

## Commit, retry и runtime

RPC `reset_fan_and_lighting {dev,operation_id,expected,sensor_id}` принимает target
GroupRevision из актуального FanAndLighting snapshot. Все defaults готовятся и
валидируются до admission; prepared content имеет отдельный source digest domain
`lconnect-lcdv2-control-reset-v1`. Reset нельзя принять за archive с тем же ID.

Общий FanAndLighting coordinator удерживает оба actors, выполняет preflight,
атомарно сохраняет settings/profiles/receipt через existing redo journal,
устанавливает оба runtime и делает Apply. Новый settings writer или HID writer
не создаётся. Profiles revision растёт один раз. Files/assets не создаются и не
удаляются, поэтому reset не требует publication journal.

После admission отдельная Tokio task владеет shared runtime permit и ответом
coordinator независимо от caller. Потерянный ответ проверяется через
`read_group_import` с той же группой и ID. Повтор с прежними expected/sensor/ID
читает receipt/current runtime без нового Apply. Иные параметры с тем же ID и
stale target с новым ID отклоняются. Receipt сохраняет последние64 операции;
отсутствие старого receipt не позволяет обходить проверку старого target token.

Storage ACK и runtime error разделены в `FullGroupCommit`. Явный
`reapply_group_import` применяет текущие defaults без нового commit только пока
receipt content current и opaque runtime token актуален. Новые ручные настройки
и slot edits сохраняются; старый reset не повторяется поверх них.

CLI:

```sh
lconnectctl profiles group reset-fan-and-lighting --sensor SENSOR_ID
lconnectctl --json profiles group readback fan-and-lighting OPERATION_ID
lconnectctl profiles group reapply fan-and-lighting OPERATION_ID
```

CLI печатает ID и исходный target token до admission. Для retry нужны оба
`--id OPERATION_ID --expected ORIGINAL_TOKEN` и прежний `--sensor`; обычный
readback не требует исходного token. При runtime error CLI сообщает о сохранённом
reset, возвращает ошибку и оставляет ID для проверки результата.

## GUI и owned native worker

В «Настройки → Резервная копия группы настроек» выберите «Вентиляторы и подсветка»
и явный CPU temperature sensor. Кнопка подготовки вызывает read-only
`preview_fan_and_lighting_reset`: daemon возвращает defaults и точный target
snapshot. FileChooser не открывается, settings/profiles/Apply не меняются.
Перед подтверждением показаны8 named slots, полные8 fan/3 pump modes,
их policies/имена, выбранные ID StandardSpeed/PWMQuiet, RGB и target. Исторические
Windows source metadata в public summary не передаются.
Подготовку можно отменить; отдельная кнопка подтверждает reset и Apply.

Reset — отдельный mode существующего native group worker. Sensor и исходный
snapshot принадлежат worker; Finish не принимает их заново из webview. Shared
job/busy защищает от замены незавершённой операции. При нормальном Close native
дожидается admitted Apply/readback; неподтверждённый результат не выдаётся за
отменённый. Закрытие Prepared reset не выполняет его и не создаёт upload session.

ID/device/group/mode сохраняются в `lconnect.group.pending`. Восстановление
сначала читает receipt; reset не запускается автоматически. При потере ACK в
том же native worker доступен явный повтор с прежними ID/expected/sensor. Если
commit уже состоялся, daemon возвращает receipt без нового commit/Apply. В новом
экземпляре desktop без исходного snapshot доступны readback и current explicit
reapply; неизвестный исход не создаёт новую операцию сам. Новые settings/slot
edits блокируют reapply старого receipt. Screen и локальные editor drafts сохраняются.

## Приёмка

- [x] Defaults: active/inactive RGB, topology, curves,8/3 stored mode banks,
  factory names/default IDs и8 named slots; invalid binding и stale Save CAS.
- [x] Actual Unix RPC/DeviceManager/MockBackend: обе cooling curves, missing-source
  fallback100, Screen settings/slot и global thermal policy сохраняются.
- [x] Повтор без commit/Apply поверх более нового manual intent/slot rename;
  different parameters/stale target rejection; shutdown/restart readback.
- [x] Actual CLI: snapshot/reset/retry/readback/reapply/stale target на synthetic
  CPU reading и MockBackend.
- [x] GUI: read-only defaults/target preview, явное подтверждение, CPU temperature,
  storage/runtime readback, сохранённый ID/reload, explicit retry/reapply,
  shared busy и сохранение локальных drafts. Browser reset и archive regressions.
- [x] Native worker с actual Unix IPC: preview/cancel, prepared Close, admitted
  Apply joined by shutdown, stale target без commit и receipt recovery. Настоящий
  proxy connection loss после commit; явный retry не меняет library/runtime revision.
- [x] Native WebKitGTK/MockBackend: read-only preview → Cancel → новый preview →
  явное Apply, Screen/draft preservation, reapply без commit, newer intent и
  normal Close. Новый desktop после daemon restart читает persisted ID/receipt
  без reset поверх37% manual fan. FanAndLighting archive native regression принят.
- [ ] Расширенные WebKitGTK fault/abrupt death/connection-loss/accessibility flows.
  Warm worker lost-ACK retry проверен actual Unix proxy и browser; это не весь
  WebKitGTK/production-portal failure matrix.
- [ ] Расширенные partial install/Apply/recovery и sleep/detach сценарии вместе
  с общей control-group matrix. Физические проверки отложены пользователем.

[Checkpoint полного bank reset и локального восстановления](evidence/2026-10-17-cooling-mode-reset.json).

[Общий coordinator и его границы](profile-groups.md).
