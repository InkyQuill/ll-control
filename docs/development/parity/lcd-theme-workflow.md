# LCD themes: GUI и CLI workflow

Сверено 2026-10-16. Реальные проверки новых themes ещё не
проводились. Во время текущей разработки hardware tests отложены.
Примеры ниже — для будущего пользовательского запуска с уже работающим daemon;
не запускать daemon ради preview без учёта replay существующих настроек.

## GUI

Откройте устройство → Pump screen. В Composition можно добавить изображение,
текст, часы или выбрать точный источник в Bind sensor → Add. Доступны подпись,
единицы температуры, точность и text style. Ready-made themes создают редактируемый
локальный draft; они не применяются автоматически. Refresh preview готовит
картинку локально; только Apply to LCD отправляет тему устройству. Load active
theme загружает сохранённую тему в редактор отдельным действием. Restore LocalUI
отменяет автоматическое восстановление темы/video. Названия приведены для
основного English; русский интерфейс использует те же команды.

[Production sensor checkpoint](evidence/2026-10-16-production-sensor-theme.json)
проверяет native редактирование через AT-SPI/keyboard без JS injection: один
sensor layer, изменённые name/label и Fahrenheit, preview без отправки,
явный Apply, реальные JPEG writes в MockBackend, missing/stale/recovery,
обновления после Close GUI и replay после restart. Для каждого из7 состояний
переданный480×480 JPEG сопоставлен с независимым literal-text preview.
Это synthetic cache и MockBackend, не проверка физических sensors/LCD.
Полный inventory controls — [отдельно](lcd-theme-controls.md).

## Settings, reset and collections recovery

LCD errors и сообщения reset/collections показывают локализованный context и
исходную диагностику отдельно. Language switch не повторяет операцию. При
неподтверждённом reset используйте Check reset result: matching receipt снимает
старое предупреждение. Explicit retry использует тот же operation ID; definitive
rejection требует нового подтверждения. Migration/save collection notices и
accessible remove labels EN/RU квалифицированы, имена файлов не переводятся.
[Software evidence and remaining native gates](evidence/2026-10-16-lcd-lifecycle-messages.json).

## CLI

1. `llcontrol sensors`: выбрать точный ID и unit нужного датчика.
2. Скопировать [пример](../examples/lcd-theme.json), заменить `REPLACE_WITH_SENSOR_ID`.
3. `llcontrol theme preview theme.json preview.jpg`: получить JPEG без применения к устройству. Без `--device` доступны host sensors; с ним добавляются датчики устройства.
4. `llcontrol theme apply theme.json --device DEVICE`: применить. Daemon обновляет картинку, датчики и local clock раз в секунду, GUI/CLI не должен оставаться открытым.
5. `llcontrol theme state --device DEVICE`: текущая активная композиция. Для transport error и source использовать также LCD state.

Изображения: `llcontrol theme import photo.png` возвращает managed key.
Добавьте слой первым для фона, последним для watermark:

```json
{"kind":"image","asset":"KEY_FROM_IMPORT","x":0,"y":0,"width":480,"height":480,"fit":"crop","opacity":255}
```

Fit: `crop`, `contain`, `contain_native`, `stretch`; opacity 0..255, сохраняется исходный alpha.
Текст имеет x/y, size 8..160, RGB color и left/center/right alignment.
Слои рисуются в указанном порядке. TextStyle поддерживает `max_width`,
`line_height`, `vertical_align` и `line_box`; CSS mode (`line_box:true`)
использует em sizing, OpenType shaping/bidi, Unicode wrapping и tab stops.
Legacy mode сохраняет прежнюю вёрстку. [Coverage](text-wrapping-acceptance.md). Датчик
должен совпадать по ID/unit; missing/stale/invalid отображается как «—».
Температура может выводиться в Celsius/Fahrenheit, memory — GiB.

Import принимает до 16 MiB локально, нормализует PNG до 960px; IPC upload
ограничен 1 MiB. Assets хранятся в config root `theme-assets/`, отдельно от
исходных файлов пользователя. Preview создаёт новый файл и не перезаписывает
существующий. Apply/preview принимают JSON до 64 KiB на CLI, максимум 32 слоя.

Тема сохраняется в settings стабильного устройства и восстанавливается после
restart/reattach с повторной загрузкой assets и актуальными sensor caches.
Явные image/clip/mode/clear отменяют сохранённую тему; brightness/rotation
сами по себе её не отменяют. Missing/corrupt asset при replay оставляет Idle
и `last_error` с `theme restore`; сохранённый документ остаётся для исправления.
Запись использует существующий settings actor (250ms debounce и flush при
штатном shutdown): apply response не является гарантией fsync на момент ответа.

**Границы проверки:** GUI editor, presets и cleanup assets реализованы; их
расширенная приёмка отслеживается в [общем checklist](basic-parity-checklist.md).
Смена на обычный image/clip или clear
прекращает theme source. Полученный apply success означает отправленный JPEG,
а не наблюдение физического LCD. Missing/corrupt asset до apply оставляет
старый источник; transport failure виден через LCD state.

## Отмена preview и Apply

Preview/render и подготовка Apply удерживают общий лимит заданий до окончания
фоновой работы, даже если RPC waiter отменён. Подготовленная после отмены
тема освобождается без отправки новой команды. Если SetTheme уже принят LCD
actor, операция завершается там; потеря ответа не отменяет принятую mutation.
В этом случае перечитайте состояние перед повтором, не повторяйте Apply
автоматически. Лимит остаётся занят до ответа actor.

## Очистка изображений

```sh
# Только отчёт, ничего не удаляет:
llcontrol theme cleanup
# Повторная проверка ссылок и удаление найденных orphan assets:
llcontrol theme cleanup --delete
# Сохранить asset, используемый во внешнем JSON-черновике:
llcontrol theme cleanup --delete --keep ASSET_KEY
```

В GUI Display есть «Очистка неиспользуемых изображений»: сначала проверка,
потом отдельное удаление. Текущий черновик автоматически передаёт свои keys
как `keep`. При удалении ссылки сканируются заново.

Защищены assets всех сохранённых screen slots и device settings, включая
отключённые устройства. Неизвестная версия, corrupt/unreadable settings или
library останавливают очистку до удаления. Неизвестные имена, symlinks и
directories не удаляются; eligible только созданные import 32-hex `.png`.
Ошибки удаления отдельных файлов перечисляются в `failures`, успешные —
в `deleted`; операция не обещает атомарного удаления всего набора.

Импортированные или успешно загруженные изображения удерживаются до конца
текущего запуска daemon. Общая защита manager/IPC не допускает удаления во
время load, незавершённого Apply, render или debounce settings, даже после
отмены клиента. Цена этой политики: удалённый из темы/слота asset текущей
сессии может остаться до следующего запуска daemon. Перезапуск не нужен для
очистки старых orphan assets и не выполняется автоматически.

После restart внешние JSON-файлы и несохранённые черновики других клиентов
не являются durable roots. Сохраните их как именованные профили либо передайте
их keys через `--keep`; GUI защищает свой текущий черновик. Удаление слота
само по себе не удаляет изображения. Asset cleanup не удаляет профили и не
обращается к HID.

## Triple и сменяемые показатели

В списке готовых тем доступны Triple (три показателя одновременно) и Cycle
(три исходных binding по очереди). Создание раскладки меняет локальный draft;
Apply передаёт тему daemon. В Cycle можно менять источники, подписи, точность,
°C/°F и интервал, добавлять/удалять показатели (1–16). Default interval=10s,
допустимо 1–3600s. Порядок задаётся выбором датчиков при создании, затем
источник каждой позиции можно заменить явно.

JSON layer `sensor_cycle` содержит `sensors` (sensor_id/label/unit/
temperature_unit/decimals), `interval_seconds` и общий `style`.
Вложенные bindings имеют те же проверки и правила freshness, что обычный
sensor layer. Missing reading показывает «—» в своей позиции; цикл не
переназначает binding и не пропускает недоступные показатели.

Daemon использует monotonic elapsed time текущего ThemeRuntime. Закрытие GUI
не останавливает цикл; новое Apply/replay начинает цикл заново. One-shot preview
начинает с первого показателя; live preview продвигает цикл по elapsed_seconds.
Версия схемы остаётся 1 с новым tagged layer; прежние версии приложения,
не знающие sensor_cycle/watts, отвергнут такую тему вместо частичного показа.

## Непрерывный preview

На вкладке Display включите «Непрерывный preview». После изменения draft
запрос откладывается на 250ms; дальше следующий кадр запрашивается через 1s
после окончания предыдущего. Одновременно выполняется не более одного
preview RPC этого окна. Уже принятый render job завершается под существующим
daemon semaphore; UI отбрасывает его ответ, если draft/device/tab изменились.

Редактирование/смена устройства/отключение daemon удаляют старую картинку.
Уход с Display или выключение переключателя прекращает новые запросы.
Возвращение на Display с включённым переключателем начинает новую preview
сессию. Ошибка показывается отдельно и удаляет старый кадр; live mode
повторяет попытку, не выдавая устаревший кадр за текущий.

`preview_lcd_theme` принимает optional `elapsed_seconds:u64` (default0).
GUI использует performance.now для времени своей preview сессии; daemon
рендерит sensor_cycle в соответствующей позиции, читает актуальные sensor
caches/часы, не меняет device/runtime/settings. Preview не синхронизирован
по фазе с уже запущенной на LCD темой; новый draft начинает свой цикл.
Apply остаётся отдельным действием. Ручная кнопка preview сохранена.

## Шрифт, прозрачность и тень

Все текстовые слои (text/sensor/sensor_cycle/clock) используют общий TextStyle:
- `font`: `sans` (default), `serif`, `mono` — встроенные DejaVu.
- `opacity`: 0–100, default100, применяется к тексту и его тени.
- `shadow`: null/default либо объект с x/y от −32 до32, blur0–16,
  RGB color и opacity0–100.

В редакторе доступны все эти параметры. Тень сначала рисуется за текстом;
blur использует ограниченную маску 480×480. Координаты и clipping остаются
общими с renderer. Старые темы без новых полей сохраняют прежние Sans,
100% opacity и отсутствие тени; default-поля не добавляются при serialization.

Bundled fonts работают без установленного desktop GUI и сохраняют внешний
вид при daemon replay. Лицензия и provenance находятся в image/assets.
Это выбор открытых шрифтов, не копирование 11 Windows font files.

Baseline 2.1.28: UI `Types/WMSQ/WMSQScreenTextLayoutConfig.cs` содержит
FontFamily, Shadow (default35), Opacity(default100); converter
`Utils/JsonConverters/WMSQScreenTextLayoutConfigJsonConverter.cs` передаёт
их в screen JSON. `WMSQScreenFontFamilies.cs` перечисляет 11 Windows
семейств. Точная семантика CSS Shadow и эквивалентность каждого Windows
font/template остаются full-parity исследованием; Linux shadow имеет явно
описанные параметры и не объявлен пиксельно идентичным.

## Отказ записи settings

Apply подтверждает передачу темы daemon; device settings записываются после
quiet debounce250ms. GUI сообщает о постановке сохранения в очередь.
При disk failure intent остаётся dirty: retry через1s без нового Apply;
новые независимые settings deltas объединяются с ним. Shutdown делает
последнюю попытку flush. После успешного rename fsync-ится devices directory.
Если ошибка сохраняется до завершения процесса, новое состояние на диске
не гарантируется; предупреждение есть в daemon log. IPC get_lcd_state теперь содержит persistence: pending/saved/last_error;
GUI показывает этот статус отдельно от LCD transport errors.


## Состояние записи в IPC

get_lcd_state.persistence содержит pending (dispatcher получил изменения),
saved (хотя бы одна запись успешно fsync-нута в текущем запуске), last_error.
При повторной ошибке pending остаётся true; после успешного retry ошибка
очищается. saved вместе с pending/error не означает сохранение новых изменений.
Начальное false/false/null означает отсутствие результата записи в этой сессии,
а не проверку существующего файла. Это cached observation последней работы
writer, не барьер очереди и не подтверждение конкретного Apply.
Mutation replies LCD actor могут не содержать persistence; GUI получает его
следующим штатным status poll. Старые daemon responses без поля поддержаны.
Состояние относится ко всем device settings (LCD/cooling/RGB), не к именованной
библиотеке profiles с отдельной синхронной транзакцией.

## Image watermark shadow

Image layers поддерживают optional `shadow` (x/y, blur, RGB color, opacity),
настраиваемый в редакторе. Тень следует alpha PNG, рисуется под изображением
и учитывает opacity слоя. Параметры и границы Windows-сверки:
[controls inventory](lcd-theme-controls.md). Старые темы без shadow сохраняют прежний вид.

## Clock parts

Optional `clock.format` supports `time` (legacy/default), `time_without_period`,
`date` (YYYY/MM/DD), `period` (A.M./P.M.). Every part has its own text style.
Clock preset uses date + 12-hour time + period; source date/time comes from one
local timestamp per rendered frame. Existing themes without format retain their
previous output. Date validation rejects invalid calendar dates; leap years
follow Gregorian rules. GUI close does not stop daemon-owned clock updates.

## Watermark placement

Image layer can use `watermark_position: {horizontal: 50, vertical: 50}` for
center-based Windows placement. Both fields accept 0…100; 0 uses the Windows
22px fallback. Existing x/y rectangle is retained for switching back. Sizes
stay bounded at 480px, edge overflow is clipped, and shadows include off-screen
source alpha. This option is available in the image layer editor.

## Independent sensor styles and small logos

Enable separate label/unit styles on a Sensor or SensorCycle layer to choose
independent colors, coordinates, sizes and fonts. In JSON, optional `parts`
contains `label` and `unit` TextStyle objects; numeric value uses layer.style.
Missing readings show — without dropping the label/unit. Old combined-text
layers remain unchanged. `contain_native` fits an image without enlarging a
small managed asset, unlike ordinary `contain`.

## Text blocks

TextStyle optionally accepts max_width (1…480px), line_height (8…320px), and
vertical_align top/center/bottom. x still anchors horizontal text alignment;
y anchors the whole multiline block vertically. Without the new fields,
legacy lines/spacing/position are unchanged. Wrapping keeps explicit blank
lines and Unicode graphemes, preferring whitespace breaks. Oversized individual
graphemes can exceed the requested width and are clipped at the LCD boundary.

Совместимость версий, defaults и восстановление сохранённых данных описаны в
[политике schemas](schema-compatibility.md).
