# Native desktop smoke

Последний общий native-прогон2026-10-11: **PASS** после CSS SHY и glyph fallback.
[Checkpoint с source hashes и полным readback](evidence/2026-10-11-native-text-fallback.json).
На неизменных305 source files: frontend check0/0/build,26 theme contracts,
сборка fixture/native GUI и native smoke прошли. В production bridge/renderer
проверены различимые CJK glyphs из fallback и одинаковые canonical accents;
смешанная тема с tab/SHY сохраняется через GUI Apply, профили и restart.
После закрытия GUI отправлены ещё3 JPEG frames. Все датчики synthetic,
HID — MockBackend; native AT/window switching/zoom этим не закрыты.

Предыдущий общий software checkpoint: [601 Rust tests после fallback](evidence/2026-10-11-text-fallback.json);
[ранний интегрированный checkpoint](evidence/2026-10-11-integrated-software.json)
включал110 browser tests и132 keyboard steps до новых renderer changes.
Ниже сохранены сценарии и история предыдущих проверок.

Исторический результат: **PASS** — Display, Sensors, Cooling, Lighting и Settings.
[Исторический результат с profile error recovery](evidence/2026-10-10-native-profile-missing.json).
Worktree `ll-control-m1`, ветка `codex/m1-lcdv2-dynamic-display`,
base `bcaf7a2538a10176c89dd1fddddafbe29826b56e` + незакоммиченные изменения.
Это исторический dirty-tree checkpoint, не чистый commit и не hardware acceptance.
Временные логи `/tmp/llcontrol-native-1onwrhek` могут быть удалены.
Повтор после защиты Cooling/Lighting drafts также прошёл:
[evidence](evidence/2026-10-10-control-drafts-native.json),
fixture `/tmp/llcontrol-native-bo6ac51o`. Это тот же happy-path scope, не native reconnect.

Повтор после Settings/Profiles/Cleanup guards: [PASS](evidence/2026-10-10-library-races-native.json).
Объём native сценариев прежний; новые races проверены browser fixtures.

## Проверенные сценарии

Реальный Tauri/WebKitGTK под отдельным Xvfb и DBus session использует production
Rust bridge и Unix IPC. Fixture создаёт только MockBackend; XDG runtime,
settings/cache/data изолированы. Реальный HidApi не создаётся.

- Display: native image normalization/import, preview без apply, DOM load/edit/
  Apply, sensor pixels update; image native fit/position/shadow, date, sensor
  parts и text layout сохраняются. После закрытия GUI продолжаются JPEG frames.
- Sensors: fixture CPU отображается на настоящей странице каталога с °C.
- Cooling: quiet проходит native preset bridge и остаётся draft; редактирование
  точки → pump curve Apply, fan fixed55 → applied state без error/fallback.
- Settings: rotation180 сохраняет brightness; brightness65 сохраняет rotation
  и source Theme.
- Lighting: manual mode для pump и fan; CPU thermal rule80; dynamic RGB
  enable→disable→enable с живым fixture sensor и applied state без error.
- Profiles через DOM: screen save/rename/load/reset/delete/resave; pump/fan
  save/load в независимые slots. Содержимое проверяется через IPC, локальная
  загрузка/reset/delete не применяют settings к устройству. Все три группы
  и revision библиотеки проверяются после restart.
- Profile revision conflict: второй IPC client меняет библиотеку; stale GUI
  rename отклоняется без overwrite или Apply, оба черновика сохраняются.
  Refresh видит внешнюю правку, очищает ошибку; retry успешно сохраняет rename.
- Missing asset: native RPC save profile / Apply theme отклоняются, полная
  library/revision и рабочая тема сохраняются. Это bridge/RPC, не DOM asset editor.
- Missing sensor: pump slot загружается через DOM с явным unavailable option;
  загрузка не применяет его, Apply неизвестного binding отклоняется с видимой
  ошибкой и сохраняет working policy. Возврат к допустимому slot и Apply успешны.
- Новый процесс daemon восстанавливает theme/assets, pump/fan policies, LCD
  config, thermal config и dynamic config/base mode. Cooling и dynamic RGB
  переходят в applied без fallback/degraded/errors после свежих fixture samples.

Thermal config требует известных fan и pump lighting bases. Первый native
прогон выявил техническое сообщение вместо инструкции; GUI теперь поясняет
предварительную настройку обеих зон. Browser regression проверяет ошибку и retry.

## Границы доказательства

Import вызывает production command напрямую; OS file picker не проверен.
DOM controls действительно используются, но это не keyboard/accessibility audit.
Pump/fan applied и RGB output относятся к MockBackend, не реальным RPM/LED.
Threshold crossing, thermal activation/grace/restore имеют отдельные actor tests;
этот native happy path сохраняет threshold выше synthetic CPU temperature.
USB disconnect/sleep/recovery, native error/reconnect matrix, внешнее удаление assets уже сохранённого
профиля, исчезновение активного sensor, cleanup и альтернативные controls ещё требуют
соответствующих сценариев. Profile happy paths выше не закрывают эти ошибки.

## Воспроизведение

Нужны desktop build dependencies, C compiler (`cc`), Python3, Xvfb (`xvfb-run`), `dbus-run-session`.
Из корня worktree (не одновременно с Playwright/dev build):

```sh
pnpm --dir desktop install --frozen-lockfile &&
pnpm --dir desktop build &&
cargo build -p llcontrol-daemon --example desktop-mock --locked &&
cargo build -p llcontrol-gui --features native-smoke,custom-protocol --locked &&
python3 desktop/scripts/native-smoke.py
```

Command/injection доступны только с feature native-smoke. Обычный GUI собирать
без неё; smoke binary не предназначен для поставки. Production daemon и текущий
пользовательский display сценарий не использует. Все проверки должны завершиться
до PASS; orchestration дополнительно проверяет штатное завершение fixture.

Полный release/build/docs/frontend acceptance остаётся отдельным итоговым gate.
Предыдущие Display-only результаты сохранены в `evidence/` как checkpoints.


## Display disconnect/reconnect, 2026-10-10

В native smoke добавлен restart MockBackend daemon при открытом GUI. Проверены
видимый offline, disabled Apply/load, сохранение черновика после reconnect,
replay сохранённой темы и explicit load/Apply через восстановленный IPC bridge.
[Результат](evidence/2026-10-10-native-display-reconnect.json). Это не USB hotplug
и не обрыв во время mutation; reconnect остальных страниц остаётся открытым.


## Reconnect всех пяти страниц, 2026-10-10

[Расширенный результат](evidence/2026-10-10-native-controls-reconnect.json):
отдельный restart fixture daemon на Display, Cooling, Settings, Lighting и
Sensors. Проверены сохранение drafts, disabled offline actions, отсутствие
автоматического Apply и переход sensor unavailable → valid с прежним ID.
Это закрывает обычный native reconnect этих страниц; обрыв во время mutation
и USB hotplug остаются самостоятельными сценариями.


## Потеря подтверждения записи, 2026-10-10

GUI теперь подключается через test-only Unix proxy в отдельном помеченном runtime
каталоге. Proxy один раз теряет успешный ответ brightness=66 после выполнения
команды. Проверяются visible error без success, сохранённый draft, readback
фактического значения, ровно одна исходная запись и успешное explicit Apply=65.
[Evidence](evidence/2026-10-10-native-lost-reply.json). Timeout и другие команды
этим сценарием не покрыты; proxy не используется production GUI.


## Timeout уже выполненной записи, 2026-10-10

Proxy удерживает успешный response brightness=67 без закрытия сокета: параллельные
чтения продолжаются. Проверены реальный 10s timeout bridge, явная ошибка без
success, сохранённый draft, фактический readback=67, одна запись без auto-retry
и последующее explicit Apply=65. [Evidence](evidence/2026-10-10-native-write-timeout.json).
Другие mutation workflows и physical applied state этим не подтверждаются.


## Global GPU source editor — 2026-10-10

[Evidence](evidence/2026-10-10-native-sensor-defaults.json), fixture root
`/tmp/llcontrol-native-w5q7feah`. Isolated Xvfb/WebKitGTK и Unix IPC; только
MockBackend и synthetic GPU A/B (15/35%), реальный GPU/HID не открывался.

DOM выбирает GPU B и её load; GPU A отсутствует среди options. До Apply
revision0, после — revision1 и resolved35%. Сохранение не меняет активную
LCD-тему. Clear меняет только draft, config остаётся прежним. Навигация и
перезапуск fixture сохраняют draft; offline values становятся unavailable,
Apply/Load отключены. После fresh read explicit Load возвращает GPU B.
После закрытия GUI JPEG продолжает идти (ещё2frames), отдельный final daemon
restart сохраняет весь defaults config и восстанавливает прежние controls.

Весь существующий native smoke, включая profiles, пять reconnect pages и
LCD lost-reply/timeout, также прошёл. CPU defaults selection, factory suggestions,
source-specific lost-reply/error cases и native keyboard здесь не проверялись.


### Рекомендуемые источники CPU/GPU, 2026-10-10

[Новый checkpoint](evidence/2026-10-10-recommended-sensor-defaults.json) добавляет
native DOM reset в черновик: synthetic CPU45°C и GPU A, затем явный GPU B35%.
До Apply revision остаётся0; после Apply сохраняются CPU/GPU mappings, оба
значения видны в UI, явная LCD theme не меняется. Clear draft не пишет настройки;
navigation/reconnect сохраняют draft, explicit load возвращает сохранённые
bindings, финальный restart сохраняет полную config. Python harness требует
`native_recommended_defaults`, прежнего GPU-only флага недостаточно.
Это проверка Linux defaults policy на fixtures; source-specific lost reply и
реальные providers остаются отдельными сценариями.


### Native source lost-reply recovery, 2026-10-10

Unix proxy пропускает `set_sensor_defaults` с revision1 и получает успешный
ответ daemon (сохранена revision2); только затем закрывает соединение до
доставки ответа GUI. Native DOM подтверждает error без success, сохранённый
GPU A draft, fresh readback/revision conflict и disabled stale Apply. Explicit
reload загружает сохранённый GPU A; ручной выбор GPU B и Apply сохраняют
revision3, CPU binding остаётся прежним. Финальный restart сохраняет всю config.

Proxy фиксирует ровно1 target request/1 drop и3 source writes всего:
initial save, faulted save, explicit recovery. Auto-retry отсутствует. Тема LCD
и requested cooling/thermal/dynamic snapshots не меняются. Прежние native
profiles/reconnect/LCD lost-reply/timeout и restart сценарии также прошли.
[Evidence](evidence/2026-10-10-native-source-lost-reply.json).
Production код не менялся; JS/Python syntax и native build/run прошли.
Физическое устройство не использовалось; полноценная software приёмка остальных
workflows остаётся в базовом плане.


### Gradient background, 2026-10-10

[Checkpoint](evidence/2026-10-10-gradient-backgrounds.json) использует тему с
horizontal fill, загружает её через DOM, переключает на radial и включает Flip.
После Apply проверяются режим, Flip и сохранённая horizontal настройка;
существующие profile CRUD и полный theme equality после restart теперь включают
`background_fill`. Python harness требует `native_gradient_background`.
Это MockBackend без физического LCD; raster проверен отдельно pixel tests.


### Text line boxes, 2026-10-10

[Checkpoint](evidence/2026-10-10-text-line-box.json): initial sensor layer имеет
line_box=true; native DOM включает тот же режим для clock layer. После Apply
оба флага проверяются в daemon theme; profile CRUD и equality после restart
сохраняют их. Остальные native gradient/source/recovery сценарии проходят.


### TC fonts, 2026-10-10

[Checkpoint](evidence/2026-10-10-noto-sans-tc.json) добавляет TC text layer с
Noto Sans TC Regular и меняет clock font на Bold через native DOM select.
Applied theme, profiles и final restart сохраняют оба enum values. Реальные
glyph IDs/outlines и разницу raster проверяет отдельный image test; физический
LCD и соответствие glyphs Windows-версии не проверены.


### User font GUI, 2026-10-10

Native-only binary embeds licensed DejaVuSans fixture bytes. JS создаёт File
через DataTransfer и отправляет change настоящему font input. Проверяются
сообщение завершения, отсутствие auto-selection, явный DOM выбор imported font,
font_asset в сохранённой теме, profile equality и final daemon restart.
Флаг `native_font_import` обязателен в Python harness. GUI close/reconnect
продолжают прежние сценарии с managed font в теме.
[Evidence](evidence/2026-10-10-font-gui.json), `/tmp/llcontrol-native-3yxrpniu`.
Native cancel/lost-Finish и выбор fonts отдельных sensor parts остаются
отдельными проверками; browser cancel/unmount/lost-Finish проверены.


### Font rejection/retry и sensor parts, 2026-10-10

Harness сначала передаёт3 invalid bytes через File input: видимый error,
доступность повторного выбора, active theme и edited draft проверяются до retry.
Затем загружается валидный font; явные DOM select label/unit и text не применяют
тему автоматически. После Apply, profile operations и restart проверяется полная
theme equality. Python требует `native_font_retry` и `native_font_parts`.
[Evidence](evidence/2026-10-10-font-recovery.json). Browser disconnect/invalid
Finish/retry отдельно прошли; native disconnect-during-upload ещё не проверен.


### Font cleanup, 2026-10-10

Python создаёт valid32-hex`.font` prior-session orphan до старта fixture.
Native GUI раскрывает отдельную очистку шрифтов, проверяет scan=1 и явный
delete=1. Python проверяет отсутствие файла и обязательный `native_font_cleanup`.
Последующие font import/retry/parts и profile/restart остаются включены.
[Evidence](evidence/2026-10-10-font-cleanup.json). Это не fault injection unlink.

### Native font lost Finish, 2026-10-10

Тестовый proxy обрывает socket после успешного font Finish. Native GUI
восстанавливает результат через Status той же upload session; итоговый
font_asset совпадает с потерянным ответом. Счётчики: 2 Begin/2 Finish
(сначала invalid file, затем valid retry), 1 dropped response/1 recovery Status;
после удаления prior-session orphan сохранён ровно один font файл.
DOM draft/Apply, profile и restart equality, GUI-close rendering и прежние
native control/reconnect сценарии прошли. [Evidence](evidence/2026-10-10-native-font-lost-finish.json).
Изменены только Python fixture/proxy; production код и JS harness не менялись.
Это не проверка partial-upload disconnect, missing saved font или железа.

### Native partial font upload, 2026-10-10

После invalid-file rejection harness отправляет валидный файл и теряет
ответ на первый принятый64KiB chunk. GUI показывает error, сохраняет draft
и active theme, не добавляет interrupted font в choices. Proxy подтверждает
ровно один chunk и успешный Cancel без asset; последующих chunks/Finish этой
сессии нет. Явный retry проходит вместе с lost-Finish recovery и всей прежней
native matrix, включая profile/restart. Всего3 Begin/2 Finish, один font файл.
[Evidence](evidence/2026-10-10-native-font-partial-upload.json).
Python/JS syntax, native rebuild и isolated MockBackend smoke прошли.
Production код не менялся; missing/corrupt saved dependencies и cleanup
failure/concurrency остаются. Это transport disconnect, не USB hardware test.

### Native saved dependency recovery, 2026-10-10

Python harness удаляет либо повреждает файл уже сохранённого screen profile
(отдельно font/image). Native DOM загружает профиль и пытается применить
изменённый draft: видны error и отсутствие success; draft, library/revision,
active theme и source=Theme сохранены. Возврат исходных bytes → явный Apply
очищает error и восстанавливает workflow. Все4 случая и прежняя native matrix
прошли, включая partial upload/lost Finish и итоговый profile/restart.
[Evidence](evidence/2026-10-10-native-saved-dependencies.json).
Проверены Python/JS syntax, native rebuild и isolated MockBackend smoke.
Это live-runtime recovery; daemon restart при отсутствующем файле отдельно
этим прогоном не проверяется. Production код не менялся, железо не использовалось.

### Native active pump sensor loss/recovery, 2026-10-10

Private marker удаляет fixture:cpu из SensorCache, сохраняя альтернативный CPU
reading. На уже принятой pump curve observed applied_percent становится
configured fallback; GUI показывает недоступный original ID и fallback.
Requested policy, LCD bindings и несохранённая правка curve не меняются.
Возврат same ID восстанавливает curve output без Apply draft.
[Evidence](evidence/2026-10-10-native-active-sensor.json).
Mock fixture/native rebuild, Python/JS syntax и полный isolated native smoke
прошли. Изменён только тестовый fixture/harness; physical PWM/RPM и прочие
stale/fan/thermal cases этим результатом не закрываются.

### Bundled display fonts, 2026-10-10

Abril Fatface/Caprasimo/Cubic11 добавлены в IPC enum, renderer и общий FontSelect.
Официальные bytes и полные licenses закреплены provenance; Cubic1.013 совпадает
с Windows. Native font importer показывает notices inline, без изменения CSP.
Каждый bundled render сравнивается с managed render того же файла; Cubic CJK
проверяется glyph outlines и raster.549 Rust tests, fmt/Clippy/docs, frontend
check/build,26 contracts прошли.43 browser tests до license-only исправления;
после него6 focused cases и полный native smoke прошли. Native выбирает все3
через DOM и проверяет Apply/profile/restart. [Evidence](evidence/2026-10-10-display-fonts.json).
Три missing Noto codepoints, Windows metrics и hardware остаются открытыми.

### Native cleanup fault matrix, 2026-10-10

[Сводная matrix](cleanup-acceptance.md) закрывает retention, corrupt roots и
partial unlink/retry для image/font. Script компилирует test-only C preload в
private root; только процесс desktop-mock получает LD_PRELOAD. Точные private
пути возвращают EACCES при наличии marker; остальные unlink идут в kernel.
После удаления marker explicit rescan/retry удаляет остаток. Реальное железо,
production daemon и пользовательские settings в этом сценарии не используются.
