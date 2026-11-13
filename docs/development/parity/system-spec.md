# System Information / Spec

Baseline: L-Connect 3 **2.1.28.0**, применимый общий сценарий для LCDV2.
Это статические характеристики компьютера; они не входят в числовые readings
датчиков, LCD HID payload или настройки охлаждения.

## Сценарий и реализация

Windows `UI/Views/Pages/SystemInfo/SpecPage.cs`, `readSystemSpec/updateUI`,
запрашивает `HttpRequestActionType.SystemSpec`: CPU, GPU, memory, monitors,
motherboard, storage. Service возвращает HWiNFO report, выбор компонента
показывает его dictionary. [Versioned inventory](system-info-inventory.md)
содержит provenance. Linux не имеет HWiNFO report; эквивалент строится из
доступных Linux источников и показывает происхождение каждого поля.

| Группа | Источник | Данные и границы |
|---|---|---|
| CPU | `/proc/cpuinfo` | Модель, vendor, physical package ID, logical threads, reported physical cores; без package ID записи объединены как unassigned, число сокетов не выдумывается |
| GPU | sysfs PCI + optional `pci.ids` | Все display-class PCI devices, model/family label, raw vendor/device/revision, driver; имя из PCI database не гарантирует marketing name |
| Память | `/proc/meminfo`; readable SMBIOS type17 raw entries | OS-visible capacity отдельно от модулей; manufacturer/part/locator/bank/type/capacity/rated/configured transfer rate, если доступны |
| Мониторы | connected DRM connectors + EDID base block | Connector, model/PNP ID/product/physical dimensions/первый detailed timing; checksum/header проверяются; это не текущий desktop mode |
| Плата | sysfs DMI identity | Board model/vendor/version и BIOS vendor/version/date |
| Накопители | sysfs block, NVMe model fallback | Physical block devices: model/vendor/revision, capacity из 512-byte sectors, rotational flag; loop/ram/zram и virtual devices без device исключены |

SMBIOS size units, extended size/speed и type17 offsets сверены с
[upstream dmidecode](https://raw.githubusercontent.com/mirror/dmidecode/master/dmidecode.c).
Скорости памяти выражаются в MT/s, не подменяются частотой Hz. Неизвестные размеры,
placeholder strings, отсутствие прав, malformed/truncated blobs и overflow
не превращаются в нулевые значения. Permission denied не вызывает sudo/helper.
Память MemTotal не объявляется суммой физических DIMM.

`get_system_spec {refresh?: boolean}` не принимает `dev` или пути файлов.
Schema v1: шесть typed groups, component IDs, typed field values, source и
unavailable reason. Кеш 30 секунд; явный refresh обходит его. Один owned
blocking job удерживает admission permit и обновляет кеш даже при исчезновении
ожидающего RPC caller. Повторная занятая работа возвращает DeviceBusy.
Каждый файл/каталог читается с пределом размера/числа записей; final symlinks и
нерегулярные файлы не читаются. Превышение лимита группы не выдаёт полный
инвентарь из обрезанного списка. Общий валидный JSON report ограничен 512 KiB.

CLI: `llcontrol spec [--refresh]`, `llcontrol --json spec`.
GUI: «Сведения о системе», шесть категорий, список компонентов, detail fields,
явное обновление. Помпа не требуется. Ошибка обновления сохраняет предыдущие
сведения с пометкой актуальности; disconnect блокирует refresh. Нет опроса
Spec каждую секунду и команд Apply/USB на этой странице.

## Язык и границы данных — 2026-10-16

Counts/hertz/bytes и collection timestamp используют подтверждённый язык GUI.
Переключение EN/RU меняет формат уже загруженного snapshot без повторного RPC.
Собственная ошибка неподдерживаемого ответа хранится отдельно от raw backend
диагностики. Переводятся только известные собственные field labels; неизвестные
ключи, включая `constructor`/`__proto__`, названия компонентов, text values и
source labels остаются данными.

21 browser case (8 новых EN/RU), Svelte0/0, static resources924/924 и frontend
build приняты. Native WebKitGTK/Unix RPC/private provider EN/RU проверил
fractional bytes/date, шесть категорий и joined Close без помпы.
[Evidence](evidence/2026-10-16-system-spec-language.json).
Это не Windows field comparison, real-session/scaling или native error/speech matrix.

## Приёмка

[Evidence 2026-10-15](evidence/2026-10-15-system-spec.json): source hashes того checkpoint,
workspace1090/3 ignored,19 browser, frontend/Clippy/docs/fmt и native acceptance.


- [x] Private provider fixtures: отсутствующие/некорректные поля, raw EDID/SMBIOS,
  overflow, short cache и explicit refresh; потеря caller не освобождает job раньше времени.
- [x] Unix RPC и CLI без устройств: все группы, null/unavailable, refresh,
  отклонение неизвестных параметров и произвольных путей.
- [x] Browser: категории/несколько GPU/unknown DIMMs, refresh failure/retry,
  stale retained snapshot при потере daemon.
- [x] Native WebKitGTK/Tauri → Unix RPC → private Linux fixture, без устройств:
  шесть категорий, component selection, unknown values, explicit refresh и joined Close.
- [ ] Сравнить соответствующие поля на одном компьютере под Windows/Linux;
  права sysfs/DMI, несколько CPU/GPU/мониторов/дисков, NVMe и доступность DIMM;
  EDID hotplug после refresh, Wayland/X11 и масштабирование. Это отложенная
  платформенная/аппаратная проверка, не следствие synthetic success.

Воспроизводимый native сценарий после `pnpm --dir desktop build`:

```sh
cargo build -p llcontrol-daemon --example desktop-mock --locked
cargo build -p llcontrol-gui --features native-smoke,custom-protocol --locked
python3 desktop/scripts/native-system-spec.py --language en-US
python3 desktop/scripts/native-system-spec.py --language ru-RU
```

Harness создаёт частные Xvfb/D-Bus/XDG каталоги, запускает только MockBackend,
не подключает помпу и читает только созданные им inventory fixtures.
