# LCDV2 effects: текущая реализация и оставшиеся требования

**Приоритет:** перенос оставшихся LCD-эффектов и Style Express выполняется
последним, после основного функционала и его программной приёмки. Обычные
RGB-режимы и sensor-driven подсветка остаются в основной очереди. Уже
реализованные эффекты сохраняются. Порядок работ задаёт [roadmap](../../../roadmap.md).


Сверено 2026-10-13 с локальным Windows L-Connect3 **2.1.28.0**.
Это очередь реализации эффекта/настроек; общие editor/capture gates остаются
в [чеклисте](basic-parity-checklist.md). Baseline UI выбирает23 страницы в
`Galahad2VisionScreenPage.cs:153–183`.

## Что добавлено

`lconnect-ipc::effects` хранит Rain/Snow, static intensity1…3, явную sensor
identity/unit, integer low/high и Linux scene seed. Native scale извлечён из
Windows exports и JavaScript установленного baseline; vendor assets/code в
репозиторий не перенесены.

| Effect | Static1/2/3 | Dynamic sensor mapping | Reference |
|---|---|---|---|
| Rain | DropletsRate1/2/4 | value<low→1; value>high→6; обе границы и промежуток→2 | Rain page intensity table/Export; `Assets/ga2v/Rain3D/js/index.min.js`, setSensorValue |
| Snow | Speed10/30/60 | step=ceil((high−low)/5), band=trunc((value−low)/step); ≤0→10, ≥5→50, иначе10×band | Snow page Export; `Assets/ga2v/js/app.8fc94db7.js`, SnowShowMode |

Snow dynamic high не равен static high. Пороговый диапазон валидируется
как low≤high; неправильные units/identity, NaN и age>3000ms не используются.
Отсутствующий источник помечается unavailable и использует сохранённую static
интенсивность. Это явная Linux policy; Windows при отсутствии sensorData может
сохранять прежнее состояние. Проверка совместимости этого случая ещё открыта.

`lconnect-image::effects` рисует собственные капли с преломлением background
и снежинки в480×480RGB. Stateless preview использует seed/session milliseconds;
live dynamic путь использует `WeatherScene`, накапливающий движение
по предыдущей скорости. Пересечение sensor band не меняет текущие позиции.
Rewind требует новой сцены; ошибочный запрос не меняет canvas/state.

Rain/Snow подключены к `ThemeLayer::Weather` в schema v3. Weather всегда первый
слой: background → weather → overlays. v1/v2 остаются совместимыми.
GUI хранит static intensity и выбранные sensor identity/unit/low/high при
переключении Static/Dynamic. Диапазоны других источников сохраняются в
`retained_sensors` (до63 неактивных плюс активный): ключ — `(ID, unit)`,
текущий binding не дублируется. При смене unit старый диапазон не применяется
автоматически. Defaults: temperature/load30/60, RPM800/1200 из
`WMSQUIHelper.CreateVisualEffectSensorSetting`. Некорректный активный диапазон
требует исправления перед сменой источника; диапазоны не теряются скрыто.
Старые темы v3 без cache остаются читаемыми; отправка выполняется только явным Apply.

Basic/Rain/Snow/Meteor/Snow3D/Fireworks сохраняют отдельные полные темы: name, background color/gradient,
image/video, overlays/fonts и weather parameters. `page_cache` содержит максимум
пять неактивных страниц без вложенных кешей; backend проверяет все страницы,
renderer использует только активную. Первый переход наследует фон и overlays
текущего черновика, дальнейшие изменения независимы. Старые scalar `presets`
мигрируют при переключении; темы без cache по-прежнему читаются.
Profile/theme JSON сохраняют полное состояние.
Первый Dynamic binding читает `get_sensor_defaults`: выбранный global CPU Load
ID имеет приоритет даже если сейчас недоступен; иначе используется только один
candidate роли `cpu_load`, unit `percent`, без GPU identity. Это локальный
binding нового черновика; global defaults не переписываются. Labels, порядок
каталога и доступные GPU percentages не определяют CPU Load.
Отсутствие/неоднозначность или ошибка чтения оставляют явный chooser; Apply и
profile Save требуют выбора. Существующий binding не переназначается.
Поздний ответ не заменяет manual selection, новую тему или выключенный Dynamic.

«Без эффекта» и удаление строки Weather возвращают сохранённую Basic, сохраняя
Rain/Snow. Ссылки всех страниц защищают image/font cleanup и удаление видео в GUI.
Daemon удерживает группы video leases, включая ожидание записи настроек, и учитывает
все страницы в сохранённых темах/профилях. Неактивное видео не запускает decoder.
Это покрытие состояния не подтверждает эквивалентность Windows defaults/assets.

GUI bounds:0…100 для°C/% и0…3600 для RPM (`applyDynamicSetting` + Windows helper).
Новые active edits проверяются перед Apply и profile Save. Backend сохраняет
читаемость исторических v3 integer ranges, не clamp-ит их молча. Равенство low=high поддержано. Actual installed JS oracle подтвердил:
Rain при below/equal/above даёт1/2/6; Snow даёт10/NaN/50. У Snow нулевой step
даёт NaN при точном совпадении, который входит в velocity arithmetic. Linux
сохраняет finite branches и выбирает speed10 в дефектном случае равенства;
NaN не передаётся renderer. Это явное исправление baseline defect, а не
визуальная эквивалентность ошибочного Windows кадра.
Preview не продвигает состояние live scene.

Общий H.264 worker кодирует procedural background без video decoder/media pin
либо добавляет weather поверх video. Используются общий 24fps timeline и
существующие cancel/process cleanup. Persistence сохраняет weather-тему без
media; Apply → restart → sleep/wake → Clear проверен через private IPC/MockBackend.
Профильный JSON round-trip и native WebKitGTK preview/Apply → slot Save/load →
restart проверены с managed image background и text overlay, через MockBackend.
Поток продолжает отправлять video packets после выхода GUI и восстановления
daemon. Native background-video/fault matrix остаётся открытой.
Проверенный рендер не доказывает визуальную эквивалентность Windows Rain3D/Snow.

## Каталог применимых страниц

| Страницы | Сейчас | Что осталось |
|---|---|---|
| ImageVideo, BackgroundColor | Общие image/video/background и overlays реализованы | Общая Windows/native/hardware приёмка |
| Rain, Snow | Schema v3, renderer, H.264 runtime, preview/GUI и persistence | Native background-video/fault matrix и Windows visual comparison |
| MeteorShower | Schema/parameters, собственный renderer, theme/daemon/H.264/GUI/page-state, slot/restart подключены и mock/native-tested | Native background-video/fault matrix и Windows visual qualification |
| Snow3D | Static-only schema, perspective renderer, theme/daemon/H.264/GUI/page-state и slots/restart подключены | Native/reference qualification ниже; hardware отложен |
| Fireworks | Model/numeric oracle, stateful renderer, theme/H.264/GUI/page-state/profile/restart подключены; mock/native happy path принят | Windows visual/timing/recovery и native video/fault qualification |
| TwistHole, Kaleidoscope, Mandala, Blossom, Transform | Не реализованы | Settings inventory, собственные scene/controls/runtime |
| TimeTunnel, Whirlpool, RadiantRainbow | Не реализованы | Settings inventory, собственные scene/controls/runtime |
| FlyInWording, SnowWording | Не реализованы | Text motion/settings/font integration |
| Style Express: Robot, PlanetCity, Time, Steampunk, Atmosphere, Radio | Не реализованы | Settings и sensor/clock binding, собственные assets/scenes/runtime |

## Открытые критерии

- [ ] Native background-video и fault acceptance (source/asset loss, interrupted
  Apply/profile save/replay); native managed image/slot happy path уже проверен.
- [x] Полный page-state Basic/Rain/Snow/«Без эффекта»: независимые background,
  overlays/name и сохранение при временном отключении, Apply, profile и restart;
  native WebKitGTK/MockBackend и resource/schema проверки приняты.
  [Evidence](evidence/2026-10-13-lcd-page-state.json).
- [ ] Проверить расширенную unavailable/recovery matrix и сопоставить fallback
  policy с Windows.
- [ ] Сравнить geometry, drop/snow appearance, speed и density с Windows при
  одинаковых inputs/settings; software comparison не подменять hardware выводом.
- [ ] Остальные16 страниц (кроме двух общих, Rain/Snow, Meteor, Snow3D и Fireworks) отложены до завершения основного функционала; реализовать после
  извлечения каждого settings inventory. Наличие имени не означает parity.
- [ ] Затем применить общий hardware checklist; hardware отложен пользователем.

Offline preview без device/daemon запуска:

```sh
cargo run -p lconnect-image --example weather-preview -- /tmp/lconnect-weather-preview.png
```

Строки Rain/Snow, колонки low@0ms, high@0ms, high@42ms. Результаты текущего
foundation — в [evidence](evidence/2026-10-13-weather-foundation.json);
integration — в [evidence](evidence/2026-10-13-weather-integration.json).

Per-source ranges и native managed-image/slot checkpoint:
[evidence](evidence/2026-10-13-weather-ranges-native.json).

Повтор native проверки (только isolated MockBackend build):

```sh
(cd desktop && npm run build)
cargo build -p lconnect-gui --features native-smoke,custom-protocol --locked
cargo build -p lconnect-daemon --example desktop-mock --locked
python3 desktop/scripts/native-weather.py
```

Independent weather parameters и GUI bounds:
[evidence](evidence/2026-10-13-weather-pages.json).

CPU Load default/source ownership:
[evidence](evidence/2026-10-13-weather-cpu-load.json).

Equal thresholds и Snow baseline defect:
[evidence](evidence/2026-10-13-weather-equal-thresholds.json).


## MeteorShower: контракт и приёмка

Windows 2.1.28 `Galahad2VisionScreenSettingPage_MeteorShower.cs`:
static slider1…3, default2, экспорт `Speed = slider+1`; две static RGB colors
(default green/blue). Dynamic default CPU Load; для каждого из шести источников
(CPU/GPU temperature/load, pumpRPM, coolant temperature) сохраняются отдельные
low/high thresholds и две low/high palettes. Общие text layouts, watermark,
background image/solid color экспортируются вместе с эффектом. Локальный
экспортированный XAML подтверждает slider bounds1…3.

Installed `Assets/ga2v/js/app.8fc94db7.js` / `MeteorShowerShowMode`:
step=ceil((high−low)/9), band=trunc((value−low)/step). Band≤3 даёт speed2 и
low colors; band≥7 — speed4 и high colors; иначе speed3 и HSB blend с весом
band/10. RGB→HSB округляет каждый канал вверх, hue смешивается напрямую,
без shortest-arc correction. Native500 streaks имеют длины85…115, angle−π/8,
24fps, ускорение и random reset; после7200 кадров pool меняется.

Linux `MeteorEffect` хранит static colors/speed, active source и до63
retained source settings с палитрами. Валидирует identities/units/ranges/cache;
нет ложного zero при missing/stale/wrong-unit reading. Сейчас fallback использует
static settings, как weather; это требует unavailable/recovery comparison.
При equal thresholds native выдаёт NaN colors на точном равенстве. Linux выбирает
low speed/colors на равенстве, high выше; это явное исправление baseline defect.
Числовой fixture получен исполнением исходных pure JS mapping functions вне
репозитория; vendor code/assets не скопированы. Собственный `MeteorScene` имеет
фиксированное число частиц/шагов и интегрирует предыдущую скорость, чтобы
переход не перемещал частицы скачком. Аналитические reset cycles и strokes
пока не доказывают совпадение native trail/acceleration/raster.

- [x] Inventory slider/defaults/colors/dynamic mapping по версии2.1.28.
- [x] Typed standalone parameters, per-source palette/range storage и validation.
- [x] Finite outputs numeric oracle; explicit equal-bound correction.
- [x] Собственный offline renderer: motion/colors/background, continuity,
  bounded extreme time и rejected-input recovery.
- [x] Theme layer/schema v3/page-cache (3 inactive pages) и asset ownership.
- [x] Live daemon/H.264/preview/persistence/slots/restart; общий MockBackend
  owner Apply/restart/sleep/wake/Clear принят. Video composition подключена
  к общему worker; native background-video qualification остаётся ниже.
- [x] GUI speed/static palettes, per-source dynamic palettes/ranges, CPU Load,
  независимая страница, None/Apply/profile boundary.
- [x] Native WebKitGTK/MockBackend preview/Apply → slots → restart;
  cached managed image, overlays и все четыре независимые страницы сохранены.
- [ ] Native background-video, extended faults/unavailable recovery.
- [ ] Windows visual/defaults comparison и поздняя аппаратная проверка.

Offline preview: `cargo run -p lconnect-image --example meteor-preview -- /tmp/meteor.png`.
Foundation evidence: [2026-10-13](evidence/2026-10-13-meteor-foundation.json).

Integration evidence: [2026-10-13](evidence/2026-10-13-meteor-integration.json).


## Snow3D: контракт и приёмка

Windows2.1.28 `Galahad2VisionScreenSettingPage_Snow3D.cs`/GetConfigs экспортирует
`IsDynamicMode=false`, `Speed=Intensity×10+5`; slider1…3, default1 подтверждён
XAML. Dead stored `IsDynamic` не создаёт Dynamic controls. Background image/color,
общие sensor/text/clock layouts и watermark экспортируются вместе с эффектом.
Installed `Snow3dShowMode` выбирает low при Speed<25, default при25, double при>25.
Реальные WebGL modes: speed45/55/75, particleLevel300/450/650. На480×480
population=ceil(width×height/100000×particleLevel):692/1037/1498.
Numeric fixture получен исполнением исходных pure JS mode selectors вне repo.

Собственный renderer реализует perspective75°, cameraZ=480/1.8, seeded positions,
наклонённые velocities, depth wrapping, perspective size и distance opacity.
Vendor texture не переносится: используется собственный soft white sprite.
Native движется через requestAnimationFrame; Linux кодирует24fps и пока использует
nominal60Hz simulation. WebGL sprite/raster, startup opacity fade, exact temporal
behavior и pointer-driven camera equivalence требуют reference qualification.
Это не доказательство визуального совпадения с Windows.

Snow3D хранится отдельной страницей в v3 с одним ведущим procedural layer;
максимум5 inactive pages без вложенного cache. Неактивные scenes не запускаются,
asset references всех страниц удерживаются. Runtime сохраняет миллисекундный
motion timeline; stateless preview не меняет live state. Scene получает текущие
sensor readings только для обычных overlays, у интенсивности нет sensor binding.

- [x] Versioned settings/mode inventory и numeric oracle.
- [x] Typed static-only validation, schema/profile round-trip и page identity.
- [x] Собственный renderer all intensities, deterministic motion/extreme time,
  visible background и rejected-input immutability.
- [x] Live milliseconds, preview isolation и sole H.264 owner.
- [x] Mock Apply → restart → sleep/wake → Clear.
- [x] GUI intensity и full page-state; Dynamic controls отсутствуют.
- [x] Native preview/Apply/profile/restart и пять сохранённых страниц;
  cached managed image и все ранее настроенные scenes сохранены.
  [Evidence](evidence/2026-10-13-snow3d-integration.json).
- [ ] Native background-video/extended faults, asset loss/cancel/recovery.
- [ ] Windows sprite/raster/fade/frame timing/camera/defaults comparison.
- [ ] Аппаратная проверка после разрешённого пользователем возвращения к железу.


## Fireworks: контракт и подключённая интеграция

Windows2.1.28 page/VisualEffectStaticSetting: intensity1…3, default1;
GetConfigs/Speed=intensity×.1. Six static/low/high RGB colors (green, blue,
purple128/0/255, red255/0/65, gold255/215/0, light-blue130/173/237) сохраняются
независимо для каждого sensor. Dynamic default CPU Load; six native source roles
и thresholds совпадают с Meteor. Installed `FireworksShowMode` использует тот же
ceil9-band/direct HSB blend, но launch factor=.1/.2/.3. Шесть каналов oracle
проверены, включая black/white/intermediate hues. При equal bounds native NaN
на equality заменён low rate/palette; finite branches сохранены.

Native P5 default60fps; launch chance=.7×factor/frame, gravity=.04, rocket initial
vy от−sqrt(480×.04) до−sqrt(480×.04×1.95), explosion40 particles, sparks speed2…5,
life=(40…60)×(.7…1.3), life decrement=.7. Color captured at launch; sensor updates
меняют новые rockets, не перекрашивают старые. Overflow sparks>450/rockets>250
clears pools; global reset при frame>6000. Background trails/shadowBlur24,
stochastic appearance и точный temporal behavior требуют comparison.

Linux scene фиксирует60Hz simulation и сохраняет per-launch colors.
Input/frame/rewind validation не изменяет pixels/motion при ошибке; u128 tick и
clipped glow loops ограничивают работу. При gap>600 ticks старые pools очищаются
и прогреваются последние600 ticks; это явная bounded recovery policy, ещё не
native equivalence. Preview создаёт отдельную сцену. Эффект подключён к schema v3,
общему H.264 owner и GUI. Шестая независимая страница сохраняет фон, overlays,
шесть static/low/high цветов и source-specific ranges. Общий PaletteEditor
сохраняет отдельные двухцветные настройки Meteor и шестицветные Fireworks.

- [x] Versioned defaults/settings/rates/six-palette inventory и numeric oracle.
- [x] Typed model, per-source palettes/ranges round-trip и validation.
- [x] Stateful own rockets/bursts/captured colors, monotonic/extreme-time handling.
- [x] Different render cadences produce the same fixed-step state within600 ticks.
- [x] Theme layer/schema/page-cache и resource ownership.
- [x] H.264/live/preview/slots/persistence/restart/sleep/Clear (MockBackend).
- [x] GUI six static/low/high colors, source ranges/CPU Load и independent page.
- [x] Native WebKitGTK/Unix IPC/MockBackend preview/Apply/profile/restart;
  stream после закрытия GUI, пять cached pages и managed image.
  [Integration evidence](evidence/2026-10-13-fireworks-integration.json).
- [ ] Native background-video/extended faults и Windows visual/timing/recovery qualification.
- [ ] Hardware later, per user's deferral.

Offline preview: `cargo run -p lconnect-image --example fireworks-preview -- /tmp/fireworks.png`.
[Foundation evidence](evidence/2026-10-13-fireworks-foundation.json).
