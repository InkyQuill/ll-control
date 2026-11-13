# Именованные профили LCDV2

Daemon хранит общую пользовательскую библиотеку в
`$XDG_CONFIG_HOME/llcontrold/profiles.json` (либо `~/.config/llcontrold/`).
В версии 1 есть 8 screen slots и по 4 fan/pump slots. Четыре pump slots —
собственное расширение; Windows parity требует четыре user fan curves и
восемь screen slots. Индексы CLI/IPC начинаются с нуля.

Сохранение, rename и delete не применяют настройки к устройству. Apply
передаёт выбранный snapshot существующему LCD/cooling owner; активные
настройки сохраняются его прежним механизмом. Последующий rename/delete
слота не изменяет уже работающий экран или охлаждение.

```sh
llcontrol profiles list
llcontrol profiles save screen-profile.json
llcontrol profiles rename screen 0 'Ночной экран'
llcontrol profiles apply screen 0 --device DEVICE_ID
llcontrol profiles reset screen 0
llcontrol profiles reset fan 0 --sensor 'hwmon:EXPLICIT_SENSOR_ID'
llcontrol profiles delete screen 0
llcontrol --json profiles list
```

Минимальный `screen-profile.json`:

```json
{
  "slot": 0,
  "name": "Ночной экран",
  "content": {
    "kind": "screen",
    "value": {"version": 1, "name": "Ночной экран", "background": [0, 0, 0], "layers": []}
  }
}
```

Для fan/pump `content.kind` равен `fan`/`pump`, а `value` — существующий
CoolingPolicy JSON. Screen value — [схема темы](lcd-theme-workflow.md).
Sensor IDs сохраняются явно; библиотека не привязана к временному USB ID и
не подменяет отсутствующий sensor другим. При сохранении screen проверяются
все referenced assets. Удаление профиля не удаляет image assets. Для orphan cleanup используйте
`theme cleanup`; [правила удержания](lcd-theme-workflow.md#очистка-изображений).

RPC `list_profiles` возвращает `{version, revision, profiles}`.
`edit_profile` принимает `expected_revision` и `edit`:
`{action:"save",profile}`, `{action:"rename",group,slot,name}` либо
`{action:"delete",group,slot}`. При конфликте требуется заново прочитать
библиотеку: автоматического повтора записи нет.

Запись сериализована, выполняется в blocking job с ограничением concurrency,
синхронизирует временный файл, переименовывает его и синхронизирует directory.
Ошибка последнего sync может означать, что rename уже состоялся: перечитать
библиотеку перед повтором. Corrupt/oversized/unknown-version файл возвращает
ошибку и не перезаписывается пустыми defaults. Полного импорта Windows archives нет. Локальный `profiles windows-inspect` распознаёт
Windows2.1.28 ZIP metadata; `profiles windows-cooling` подготавливает все RPM modes
выбранной записи без daemon/публикации/Apply; [границы и остаток mapping](profile-groups.md).

Проверки: restart/rename/delete с сохранением соседних групп, конфликт двух
редакторов, corrupt/future version без перезаписи, JSON-RPC save без смены LCD,
missing asset и stale revision, CLI apply с передачей ошибки устройства.
Ни одна из этих проверок не использует реальный HID.

GUI: в Display и Cooling откройте «Сохранённые профили», обновите список,
выберите слот. Сохранение, загрузка в редактор, rename/delete доступны;
Apply остаётся отдельной кнопкой страницы. При конфликте ревизии обновите
список; черновик сохраняется. Загрузка экранной темы сбрасывает старый preview.

Сброс **одного слота** доступен через CLI и GUI. Он заменяет имя и содержимое,
но не меняет активную тему/политику, соседние слоты или lighting. Fan Quiet
использует точки UserDefine1–4 из Windows 2.1.28 `Galahad2VisionProfile.cs:240–275`;
pump — существующий Quiet reference preset. Cooling reset требует explicit
sensor ID; для GUI выберите его в поле «Источник температуры». Screen reset
создаёт базовую Linux-тему с часами без внешних assets. Это не полный Windows
screen reset: default Windows media, brightness=50, rotation=0 и effect graph
из `initScreenVariable` остаются расширенным SYS-003/LCD-015 сценарием.
RPC edit также принимает `{action:"reset",group,slot,sensor_id}`; для screen
sensor_id должен быть null. Схема библиотеки остаётся version 1, storage format
не изменён; неизвестные версии по-прежнему возвращают ошибку.

Полные group reset и Linux archive import/export подключены отдельными workflows:
[Screen/FanAndLighting](profile-groups.md), [reset](fan-lighting-reset.md). Остались
Windows ZIP mapping/interoperability и расширенная lifecycle/fault/reference приёмка.


## Ошибки и EN/RU — 2026-10-16

Shared Profiles form отделяет list failure от неподтверждённой mutation и
сохраняет raw diagnostic отдельным параметром переведённого сообщения.
`Настройки` как ошибка не становится `Settings`; пользовательские slot names,
включая `{1}` и `<CPU>`, не проходят через переводчик. Только fallback пустого
слота переводится. Потерянный ответ по-прежнему требует явного readback перед
повтором записи; локальный LCD/cooling draft не заменяется сохранённым snapshot.
Typed local validation сохраняет resource/parameters и реагирует на смену языка.

[Evidence](evidence/2026-10-16-profile-messages.json):49 browser cases,
Svelte0/0, frontend/native build и production-config check. Broad native RU smoke
повторил lost save reply,rename/reset/delete/reject,readback/dependency recovery,
12 daemon starts и восстановление profiles/controls/LCD;3 JPEG frames пришли
после GUI Close. EN/RU dictionary collision и language switch проверены browser.
После workflow runs добавлен один English validation resource; final static audit
и frontend build приняты отдельно. Audit теперь учитывает `ValidationError`:
913/913 resources,0 missing. Это не новая physical/real-session квалификация,
не весь archive/group transfer error UI и не обновлённый production package.
