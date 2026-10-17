# Общие единицы температуры и язык интерфейса

Baseline: Windows L-Connect 3 **2.1.28.0**, Galahad II LCDV2 **0416:7395**.
Программная приёмка проводится на private roots/MockBackend. Реальные LCD и
охлаждение отложены пользователем.

## Подтверждённое поведение Windows

- `UI/Views/Pages/Settings/GeneralPage.cs:164–190` сохраняет TemperatureUnit и
  CurrentLocale. Восстановленный `views/pages/settings/generalpage.xaml:94–97`
  содержит только Celsius/Fahrenheit; Kelvin из общего enum не является третьим
  доступным пунктом этого экрана.
- `SystemStatus.cs:659–667` уведомляет о смене единицы CPU/GPU temperature;
  converters преобразуют показания, сохраняя исходные readings отдельно.
- `UI/Products/Galahad2Vision/Galahad2VisionController.cs:144–150` передаёт
  ScreenTemperatureUnit сервису. `Svc/Products/Galahad2Vision/Galahad2VisionController.cs:867–890,1215–1233`
  сохраняет его и преобразует значения до browser rendering. Это не новая
  физическая единица датчиков и не новая команда LCD HID.
- `UI/Helpers/LocaleHelper.cs`, `ChangeLocale`, заменяет resource dictionary,
  English — fallback. Из этого кода не следует смена OS locale/CultureInfo.
  Сохранение/переключение языка и частичный English подключены в
  [отдельном срезе](interface-language.md); полный набор переводов ещё не завершён.

Пути `UI`, `Svc` относятся к frozen reference
`reference-decompiled/2.1.28-2026-09-08`. XAML восстановлен отдельно в
`scratch/xaml-2.1.28-system-2026-10-10`; provenance исходного inventory —
[System Information](system-info-inventory.md).

## Temperature fields and integer RGB — 2026-10-17

Видимое значение и `aria-valuenow` теперь используют одно нормализованное
представление: воспроизведено и исправлено100 против100.000000004 после ввода°F.
Каноническое преобразование/precision не меняются. RGB `u16` пороги — целые
0…100°C; в°F это32…212 с шагом1,8. Подсказка EN/RU связана с обоими полями через
`aria-describedby`.100°F сохраняется в draft как37.77777778°C и не отправляется
как integer policy; скрытого округления нет.87.8/143.6°F проходят actual IPC как31/62°C.

74 affected browser cases, Svelte0/0, static959/959 и frontend/native build прошли.
Обновлённый native RU harness проверил zero-device unit save/restart/offline preview
с blocked Apply, затем отдельный Mock LCDV2/Static RGB/whole-Celsius сценарий.
Broad native controls/profiles/sensor/RGB/restart повторён. Новые native действия
используют DOM edits; real keyboard/AT-SPI speech/Wayland/scaling остаются открытыми.
[Evidence](evidence/2026-10-17-temperature-grid.json). Срез включён в [пакет collections](evidence/2026-10-17-collections-integrated-candidate.json).

## GUI recovery — 2026-10-16

После неподтверждённого Save повторная запись блокируется до явного Reload,
даже если poll вернул прежнюю revision. Raw backend diagnostic не переводится
по тексту; owned validation и recovery имеют отдельные reactive EN/RU resources.
Malformed language (empty/null/false/zero) не публикуется в presentation/cache;
legacy отсутствующее поле остаётся совместимым. [Проверки](evidence/2026-10-16-settings-messages.json).

## Linux: единицы представления

`DisplaySettings` schema v1: revision и temperature_unit celsius/fahrenheit.
Отсутствующий файл означает °C. Unknown version/fields/unit, повреждение,
symlink/FIFO/oversized file — видимая ошибка; исходный файл не заменяется
defaults. Состояние хранится отдельно в `display-settings.json` и не участвует
в reset устройства, Screen, FanAndLighting или переносе групп профилей.

Сохранение: CAS revision, shared mutex и неблокирующий process flock, временный
файл + fsync → atomic rename → fsync directory chain. После неоднозначного
результата требуется readback. Два bounded owned IO jobs удерживают admission
и обновляют live unit независимо от исчезновения RPC waiter. Startup manager
загружает единицы до device replay; при повреждении предупреждает и не портит
файл. Runtime сохраняет последнюю загруженную единицу (при первом старте — °C).

`get_display_settings {}` / `set_display_settings {settings}` работают без
устройства. CLI:

```sh
lconnectctl display-settings get
lconnectctl --json display-settings set --temperature-unit fahrenheit
```

GUI: «Настройки → Единицы и язык». Черновик не меняется при polling;
Apply сохраняет выбранную единицу. Новая revision при retained draft показывает
conflict, lost reply не вызывает auto-write. Показания Sensors/source editor
пересчитываются в выбранную единицу. Cooling graph/points/hysteresis, thermal
alerts, dynamic RGB и weather temperature thresholds показывают её тоже.
Изменение температуры преобразуется с offset32; разность/гистерезис — только
scale9/5. Значения в sensor cache, cooling/RGB policies, диапазонах и архивах
остаются в исходных °C. RPM/load/PWM не преобразуются.

Новые LCD sensor/cycle layers используют `temperature_unit: system`. В редакторе
можно выбрать «Общие настройки», °C или °F. Старые themes с явным °C/°F и ручной
override сохраняются; их можно переключить на System явно. Общая единица
снимается один раз на кадр через общий atomic cache: preview, JPEG/H.264,
composition, saved-group preparation и startup replay используют тот же источник.
Unit switch не перезапускает producer, не переписывает тему и не требует Apply.
Pure image renderer без daemon settings трактует System как default °C;
production daemon перед рендерингом разрешает System в выбранную единицу.

## Чеклист

[Evidence 2026-10-15](evidence/2026-10-15-display-settings.json): source/reference
hashes, workspace1095/3 ignored,36 browser,39 frontend contracts и private native.


- [x] Store: CAS, restart, corrupted/future file retention, symlink/FIFO и foreign
  process lock, caller cancellation/admission ownership.
- [x] Real Unix RPC/CLI без устройства: неизвестные параметры/unit/version,
  stale write, сохранение/чтение и загрузка единицы до manager replay.
- [x] LCD pixels:40°C →104°F без повторного Apply; explicit °C не меняется;
  raw source остаётся40°C. Unix preview System совпадает с explicit°F.
- [x] Browser: retained draft, readback conflict после lost reply; °F readings,
  curve104/176°F →40/80°C, delta9°F →5°C; existing cooling/dynamic RGB regressions.
- [x] Native WebKitGTK/Tauri: без устройства — unit save, Sensors, offline System-layer
  preview с blocked Apply, joined Close/restart и0 HID writes; отдельная Mock LCDV2
  фаза — Fahrenheit → integer RGB через IPC и local preview.
  [Обновлённый native contract](evidence/2026-10-17-temperature-grid.json).
- [ ] Native keyboard/AT-SPI/Wayland/scaling для обновлённых temperature fields;
  fractional negative temperatures, границы/целочисленные °C policies и
  accessibility ошибок во всех temperature editors.
- [ ] Реальный LCD: смена System и explicit units во время JPEG/H.264/theme cycle,
  video overlay, закрытый GUI, restart/sleep/reconnect; нет лишних перезапусков
  потока или изменений охлаждения/RGB.
- [ ] Новый English default и обязательный полноценный русский: миграция
  сохраняет прежний выбор, dynamic/native messages и EN/RU locale/font/fallback
  проходят приёмку. Остальные языки — на этапе полировки, не блокируют базовый
  этап. Переключение/сохранение и partial English уже реализованы;
  [политика и остаток](interface-language.md)
  независим от единицы физического показателя и остаётся в roadmap.

Native harness после frontend/native-smoke build:
`python3 desktop/scripts/native-display-settings.py`. Он использует private
Xvfb/session D-Bus/XDG и MockBackend без подключённых устройств.

## Языковой default, решение 2026-10-15

Основной язык и default новой установки — English; русский обязателен.
Новый store без файла использует en-US. Legacy v1 без language читается как
ru-RU, включая возможный сохранённый выбор; обе локали при новой записи
сериализуются явно. Чтение не меняет файл, единицу температуры или revision. Остальные языки отложены до полировки.
[Полный языковой контракт](interface-language.md).
