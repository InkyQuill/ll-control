# Общесистемный мониторинг Windows 2.1.28.0

Source audit 2026-10-10; не запуск Windows GUI и не аппаратное измерение.
[Manifest источников](evidence/2026-10-10-system-info-sources.json).
Пути ниже относительно reference root из manifest. UI = `L-Connect 3/LConnect3cs`,
Svc = `L-Connect-Service/LConnectService`. Proprietary source сюда не копируется.

## Подтверждённые пользовательские сценарии

`UI/Views/Pages/SystemInfo/MainPage.cs:21–30` сохраняет переключатель показа
Mini System Information. `UI/MiniSystemInformationWindow.cs:463–477` строит
picker из 11 типов; `:745–760` действительно создаёт соответствующие controls.
Это сильнее наличия enum. Окно имеет шесть slots (`:50`), масштаб/прозрачность
и сохранение настроек (`:158–176,215–231`). EdgeHub tiles не включаем в LCDV2:
`createTileFromDescriptor` создаёт здесь только System controls (`:736–742`).

| Показатель | Windows UI / service | Linux сейчас / остаток |
|---|---|---|
| CPU/GPU load | Mini picker + factory; service `SystemInformation.cs:173,181` | CPU, AMDGPU, NVIDIA есть; Intel engine-only. Общее выбранное GPU reading требует явной семантики |
| CPU/GPU temperature | Mini picker + factory; service `:174,182` | hwmon/NVIDIA есть, применимость зависит от driver |
| CPU power | CPUPowerTile + service `:176` | RAPL package Watts есть; unsupported hardware не подменять нулём |
| GPU power | GPUPowerTile + service `:184` | GPU hwmon input/average и NVIDIA power.draw добавлены; driver accuracy остаётся непроверенной |
| CPU/GPU voltage | CPUVoltageTile/GPUVoltageTile + service `:175,183` | Volts, AMDGPU GPU/Northbridge и Intel i915/xe hwmon rails есть; CPU/NVIDIA остаются |
| RAM load/used/total | Mini RAMLoad создаёт RAMUsed; ResourcePage `:89–104`; service `:228–238` | proc/meminfo есть |
| Network upload/download | Mini Network; `UI/Views/UserControls/SystemResource/Network.cs:121–156`; service `:241–290` | BytesPerSecond per-interface RX/TX и totals реализованы; Linux включает virtual/loopback traffic; Windows отображает B/s, не bit/s |
| FPS и приложение | Mini FrameratePresentedAvgTile; service `:188–198` | Нет provider; Windows условно заполняет при IsHWiNFOPresentMonEnabled. Нужен отдельный Linux источник, не выдуманный FPS по GPU load |
| CPU/GPU clocks | ResourcePage named progress controls `:23–25,71–75,187–190`; service `:177,185` | Hertz, CPUFreq current/average/driver, NVIDIA graphics/memory, AMDGPU GFX/memory и Intel i915/xe actual/requested GT clocks есть; current/max не смешивать |
| Storage capacity/used | ResourcePage создаёт storage views `:106–148`; service `:298+`; Core StorageStatus | Filesystem catalog total/used/free/available/used_percent реализован через bounded findmnt; Linux mount enumeration отличается от Windows Fixed/Removable. Это ёмкость, не disk I/O load |

ResourcePage и Sensor Settings теперь проверены по восстановленному XAML,
а не только BAML strings. [Manifest и результаты](evidence/2026-10-10-system-info-xaml.json).
Из448 скопированных BAML версии2.1.28 восстановлены448 XAML без ошибок;
оригинальные reference-файлы не изменены. Proprietary output хранится вне repo.

## Bindings, ranges и видимость общей страницы

`views/pages/systeminfo/resourcepage.xaml` создаёт CPU/GPU load controls,
прямые temperature/clock bindings, power/voltage tiles, RAMLoad, Network и
storage template. Ни страница, ни эти tiles не имеют product-specific visibility
conditions. CPU/GPU power/voltage tiles непосредственно привязаны к SystemStatus.
Общая навигация создаёт SystemInfo без device guard (`MainMenu.cs:316`,
`MainWindow.cs:719`); вкладки Resource/Specs заданы в systeminfo/mainpage.xaml.

- Load/RAM progress:0–100; temperature progress:0–100. Это диапазон индикатора,
  не разрешение превращать отсутствующий датчик в valid0.
- Clock progress изначально0–4800 CPU и0–3000 GPU; ResourcePage.cs:71–75
  обновляет maximum из CPUMaxClockRate/GPUMaxClockRate. Текст — целые MHz;
  null/NaN/infinity показываются `--`.
- Voltage text:F2, power:F1 до1000 и F0 выше; недопустимые readings — `--`.
  Progress converter даёт0 для invalid; текстовое отсутствие сохраняется отдельно.
- Storage template показывает used-percent0–100, name, used/total GB и целый %.
  ResourcePage.cs:106–148 раскладывает более4 volumes в две колонки; это capacity,
  не I/O utilization. RAM used/total используют1024-based magnitude, F1.
- SystemInfo имеет FPS-monitor и floating Mini toggles. FPS не является tile
  основной ResourcePage; он доступен через Mini. FPS provider/workflow остаётся
  в полном desktop scope. [Spec inventory](system-spec.md) реализован отдельно;
  Windows/platform полнота соответствующих полей ещё не квалифицирована.

CPU fan присутствует **в Settings source picker**, но не в ResourcePage.
GPU fan, VRAM, DriveTemperature/DriveUsage не имеют controls/bindings в
восстановленных Resource/Settings страницах и подключённых SystemResource tiles.
Они существуют в service/Core; это не основание добавлять обязательные tiles
на общую страницу или новые поля LCDV2 payload. Уже реализованные VRAM providers
остаются полезной возможностью Linux; coverage других drivers остаётся в плане.
Вывод ограничен этими общими страницами, а не всеми product-specific страницами.

## Семантика GPU load: важное уточнение

`Svc/Utils/HWiNFO/Managers/HWiNFOGPULoadSensorManager.cs:11–24,32–84`
выбирает один sensor по пользовательскому SensorMapping либо DefaultId,
ограничивая значение 0–100. Кандидаты — HWiNFO usage/utilization на GPU/GPU_CORE;
default предпочитает GPU Core Load, затем GPU Utilization. Здесь **нет суммы,
max или mean engine counters**. `HWiNFOWrapper.cs:901–904` переносит выбранное
значение, `SystemInformation.cs:181` отправляет его в Resource.

Следствие для Linux: нельзя объявлять max/mean i915 engines точным Windows
алгоритмом. Нужен явно выбранный/подписанный источник и документированная
driver semantics. [Ручной выбор i915/xe engine](intel-engine-utilization.md) реализован,
recommended reset не назначает его общим GPU load. Как HWiNFO получает underlying aggregate для конкретного
GPU, этот wrapper не раскрывает; ABI эквивалентность остаётся непроверенной.

## Дальнейшая реализация и приёмка

1. Добавить оставшиеся CPU/NVIDIA voltage providers; Volts/Hertz/BytesPerSecond уже есть; реализовать подтверждённые missing providers с bounded reads,
   stable IDs, missing/invalid/stale и запретом fake zero. Проверить unit formatting
   в dashboard и LCD; cooling по-прежнему принимает только подходящий Celsius.
2. Membership и identity network/storage определены в [provider inventory](sensor-provider-inventory.md).
   Network total включает virtual traffic и не равен уникальному throughput;
   storage не суммирует пересекающиеся mounts/subvolumes. Аппаратная точность
   и соответствие видимому Windows catalog остаются отдельной приёмкой.
3. Source inventory общих Resource/Settings страниц закрыт восстановленным XAML
   и callers. Это определяет набор требований, но не закрывает реализацию providers
   или сравнение Windows GUI на работающей системе.
4. Mini в Windows — floating window; Linux реализует функциональность через
   виджет KDE Plasma и расширение GNOME Shell, без отдельного окна приложения.
   Шесть слотов, source selection/persistence и FPS integration остаются отдельными
   требованиями. Sway и остальные окружения отложены; полный список поддержки
   и версии shell пока не выбраны. Оконный черновик не закрывает Linux Mini.
   [Целевой контракт и чеклист](mini-system-info.md). Не увеличивать LCDV2 sensor
   payload до всех SystemResource fields.
5. Fixtures → registry → IPC → UI/LCD, затем физические driver measurements позже.

[Базовый план](../superpowers/plans/basic-parity.md) содержит незакрытую работу;
[полный roadmap](../../roadmap.md) сохраняет весь desktop parity scope.

AMDGPU voltage/clocks теперь доступны через hwmon provider с typed Volts/Hertz
и V/MHz в LCD/GUI. CPUFreq frequencies также реализованы; CPU voltage и остальные vendor providers остаются в базовом плане.


## Spec и Sensor Settings: подтверждённые callers, 2026-10-10

[Manifest](evidence/2026-10-10-spec-sensor-settings.json) фиксирует versioned
источники и проверенные Linux файлы. Первоначальный source audit использовал BAML strings; новый XAML manifest выше
дополняет его полным деревом Resource/Settings и проверкой visibility.

### Статические сведения Spec

`UI/Views/Pages/SystemInfo/SpecPage.cs`, `readSystemSpec/updateUI`, запрашивает
`HttpRequestActionType.SystemSpec` и строит шесть групп: CPU, GPU, memory,
monitors, motherboard, storage. Выбор RadioButton передаёт dictionary в detail
panel; пустые значения пропускаются, отсутствие данных показывается явно.
GPU title предпочитает `Video Card`, memory — manufacturer/type/size,
motherboard — `Mainboard Name`. Service `SystemInformation.GetSystemSpecification`
возвращает HWiNFO report. Это read-only hardware inventory, не новые realtime
readings и не управление устройствами. Linux Spec provider/typed IPC/CLI и
страница шести категорий реализованы в текущем дереве; [контракт и приёмка](system-spec.md)
отдельно фиксируют software evidence и открытое Windows/platform сравнение.
Статические строки не добавляются в числовой SensorReading или восьмипольный LCDV2 payload.

### Настройка источников датчиков

`UI/Views/Pages/Settings/MainPage.cs` загружает страницу из Tag выбранной вкладки;
XAML mainpage содержит SensorSettingPage без visibility guard. Общие styles
TabItemRadioButton/CustomTabItem скрывают только декоративную подсветку selection,
а не вкладку. Code-behind меняет visibility только TL correction по наличию TL LCD.
SensorSettingPage имеет overlay для IsEnabled=false во время чтения mapping;
это loading/availability состояние, не product guard LCDV2.

`SensorSettingPage.page_Loaded/applySensorSettingInfo` запрашивает mapping,
показывает SelectedId, а Reset All загружает DefaultId **в черновик**. Apply
собирает выбранные IDs из tagged combo boxes и выбранную graphics card,
затем вызывает SetSensorSetting. При уходе с изменённой страницы есть prompt;
при недоступной mapping picker отключён. Восстановленный XAML содержит ровно шесть CPU tags:
Temperature, Clock, Load, Power, FanSpeed, Voltage — и graphicsCardComboBox.
Отдельные GPU/drive/FPS pickers нельзя объявлять видимыми по одному enum:
SensorSettingTypes шире этого набора.

`Svc/Utils/HWiNFO/HWiNFOWrapper.GetHWiNFOSensorSettingInfo/SetHWiNFOSensorSetting`
читает/пишет AppSettings.SensorMapping; неизвестный/пустой ID Windows заменяет
DefaultId и сохраняет настройки. `SetGraphicsCardSetting` выбирает GPU и
обновляет семь GPU mappings (temperature/clock/load/power/fan/voltage/memory),
а также metadata max clock и memory size. Это единый выбранный GPU для
SystemResource, не вычисление среднего/max всех GPU engines.

Linux имеет отдельные explicit bindings в LCD/cooling/RGB и глобальный source
editor на Sensors. Schema, membership registry, durable store и get/set RPC
подключены к desktop; GPU selection/Apply/reconnect/restart прошли native smoke.
Рекомендуемый выбор уже реализован как read-only предложение daemon: только
fresh valid candidates, приоритет actual clocks перед requested, GPU по стабильному
порядку ID, без смешивания показателей разных GPU. Кнопка копирует его в черновик;
сохранение требует Apply. Это Linux policy, не воспроизведение эвристики HWiNFO.
Reset workflow и CPU/GPU selection прошли browser/native acceptance
([evidence](evidence/2026-10-10-recommended-sensor-defaults.json)). Потерянный
ответ после сохранения также прошёл [native recovery acceptance](evidence/2026-10-10-native-source-lost-reply.json):
ошибка, сохранённый draft, revision conflict, explicit reload/Apply без auto-retry.
Критерии workflow (software evidence в checkpoints, hardware отдельно):

- сохраняемые default source IDs и явный выбор GPU, без зависимости от hwmonN/cardN;
- default/reset меняет черновик; только Apply сохраняет, ошибки видны;
- missing/stale источник не заменяется другой GPU или нулём; при возврате того же
  ID данные восстанавливаются; неподдерживаемый metric остаётся unavailable;
- пользовательский explicit LCD/cooling/RGB binding не меняется молча от выбора
  defaults; применение default и explicit override различимы;
- restart, конкурирующие изменения, потерянный ответ и multi-GPU fixtures.

Последние пункты — требования Linux реализации, а не утверждение о поведении
Windows при ошибках: его fallback по unknown ID не нужно копировать как
незаметное переназначение уже работающего охлаждения.

### Остаток реализации после source inventory

Общие Resource/Settings bindings, ranges, navigation и source-visible guards
сопоставлены. CPU fan теперь назначается явно: hwmon RPM collector → candidates →
source editor → Apply → durable setting. Рекомендуемый выбор оставляет fan пустым;
label не определяет wiring. Backend проверяет исчезновение и same-ID recovery,
native — DOM selection, RPM value и restart persistence.
[Evidence](evidence/2026-10-10-cpu-fan-mapping.json). Это мониторинг, не PWM control.
CPU/NVIDIA voltage, оставшееся Intel coverage и Mini/FPS также
сохраняются в соответствующих планах. Spec реализован; сравнение полноты
доступных полей с Windows и реальными Linux платформами остаётся открытым. Runtime Windows layout и точность
реальных Linux drivers этим статическим исследованием не подтверждены.


## Общие единицы, 2026-10-15

GeneralPage XAML показывает Celsius/Fahrenheit. Сохранение AppSettings уведомляет
SystemStatus и передаёт ScreenTemperatureUnit в Galahad2Vision service, где
температуры преобразуются перед browser rendering. Kelvin из общего enum не
является третьим видимым пунктом настройки. Linux global presentation settings,
canonical-C temperature editors и System LCD layers реализованы; [контракт и
приёмка](display-settings.md) сохраняют границы software/native/hardware evidence.
Language CurrentLocale в Windows заменяет resource dictionaries; это не
доказательство смены OS CultureInfo. [Сохранение/переключение языка](interface-language.md)
и partial English подключены. Новый целевой default — English, русский
обязателен. English default подключён, legacy v1 без language и выбранный
русский сохраняются. EN/RU dynamic/native
catalog и Mini/FPS открыты, остальные языки отложены до полировки.
