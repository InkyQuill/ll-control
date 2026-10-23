> Исторический аудит 2026-09-08. Текущие статусы и порядок — [roadmap](../../roadmap.md) и [gaps](parity/lcdv2-baseline-gaps.md).

# LL-Control: состояние и roadmap до функционального parity

Дата аудита: 2026-09-08. Проверенный HEAD: `bcaf7a2`, последний коммит от
2026-04-27. Статус документа: предложение roadmap для обсуждения; это не
утверждённый implementation plan. Исходники и старые планы не изменялись.

**Актуальный roadmap:** [roadmap.md](../../roadmap.md). Этот документ сохраняет
результаты первоначального аудита; порядок M0–M6 и приоритет динамического
вывода на LCD определены корневым roadmap.

## Вывод

Проект остановился после реализации daemon + CLI для Galahad II LCDV2,
перед реализацией GUI. Основа пригодна для продолжения: чистые кодеки,
отделённый HID, per-device actors, IPC, persistence и автоматические тесты.
Переписывать всё заново оснований нет.

**Уточнение пользователя:** сейчас доступен только Galahad II LCDV2;
тестировать другие устройства невозможно. Поэтому активная цель — полная
функциональная эквивалентность Windows для **этого SKU**, с зафиксированной
версией firmware. Parity всего каталога остаётся долгосрочным направлением.
Остальные семейства не входят в условия завершения текущего релиза.

Однако прежние фазы ведут к ограниченному v1 для одного устройства, а не
к полному L-Connect 3. Full parity требует отдельной программы: автономное
охлаждение, Linux-датчики, LCD/media/editor, общая модель возможностей
устройств, несколько транспортов, wireless, firmware и проверка каждого
поддерживаемого SKU на железе.

## Источники и границы достоверности

**Обновление после получения Windows-установки пользователя:** бинарник
`winver/L-Connect 3/L-Connect 3.exe` подтверждён как **2.1.28.0**.
Десять основных сборок декомпилированы отдельно с SHA-256 manifest.
Новый основной baseline — этот комплект; упоминания 2.1.11 ниже описывают
старый reference, на котором начиналась работа. Подробности и первичные
находки: [новые reference sources](2026-09-08-reference-2.1.28-and-ll-connect3.md).

- [README](../../README.md), текущий Rust-код, история Git и тесты.
- [Основной дизайн](superpowers/specs/2026-04-25-lconnect-linux-design.md),
  [GUI-дизайн](superpowers/specs/2026-04-27-phase-2-gui-tauri.md),
  [GUI-план](superpowers/plans/README.md).
- [Протокол LCDV2](protocol/galahad2-lcdv2.md),
  [межсемейный обзор](protocol/cross-device-survey.md) и его per-family документы.
- [Исторический smoke test](protocol/captures/2026-04-27-phase-1-smoke-run.txt).
- Локальная декомпиляция: `../L-Connect3/Properties/AssemblyInfo.cs`
  указывает **2.1.11.0**; Core и Service имеют отдельную AssemblyVersion
  `1.0.0.0`, которая сама по себе не датирует весь комплект.
- Windows-поведение: `../L-Connect-Service/LConnectService/Products/`,
  `../L-Connect.Core/LConnectCore/Products/`, UI в `../L-Connect3/`,
  редактор в `../L-Connect Editor/`, транспорты в `../slv3/` и `../lcd207/`.
- Официальные [возможности](https://lian-li.com/l-connect3/),
  [обычное руководство](https://lian-li.com/l-connect3/l-connect3-manual-2/),
  [wireless](https://lian-li.com/l-connect3/l-connect3-manual-l-wireless-sync/),
  [screen guide](https://lian-li.com/l-connect3/screensoftwareguide/),
  [changelog](https://lian-li.com/l-connect3/l3-changelog/), просмотренные в день аудита.

На странице changelog верхняя версия — **2.1.28**. Дату этого релиза
аудит не устанавливает. Следует разделять parity с локальным 2.1.11 и
parity с текущим официальным выпуском. Новые устройства и сценарии нельзя
вывести только из старой декомпиляции.

Чтение исходников и codec/mock tests не доказывают физическое поведение.
В этом аудите команды управления железом не запускались. Исторические
live-результаты ниже остаются историческими; все C#-файлы построчно не
проверены. Полная инвентаризация действий Windows UI — первый этап roadmap.

## Что уже реализовано

| Область | Факт в текущем коде | Уровень подтверждения |
|---|---|---|
| Protocol | A/B/C framing, handshake, firmware identity, fan/pump PWM, RGB, LCD-команды | Кодеки + тесты; для части команд старые live captures |
| HID | Production hidapi и mock backend, отдельный владелец I/O | Автотесты; исторический daemon smoke |
| Daemon | Polling, команды, LCD/lighting keepers, сохранение и replay | Автотесты; часть сценариев проверена live |
| CLI / IPC | JSON-RPC через Unix socket, подписки, управление, диагностика | Автотесты и исторический сквозной прогон |
| LCD | Подготовка 480×480 JPEG, отправка cmd 14, обновление каждые 15 секунд | Исторически изображение удерживалось более 60 секунд |
| RGB | Preset modes и per-LED для LCDV2 | Историческая визуальная проверка помпы и кольца |
| Семейства | В runtime зарегистрирован только `0416:7395` | `family_registry.rs` и `manager.rs` |
| Другие устройства | Большой протокольный survey, включая SDK | Исследование, не реализованная поддержка |
| GUI | Выбран Tauri 2 + SvelteKit/Svelte 5 + Tailwind v4, дизайн Aurora | Спецификация и план; GUI crate отсутствует |
| Темы | Запланированы слои, bindings, условия, анимация | Реализация отсутствует |
| Firmware | Отдельные ENE bootloader design/plan | Реализация отсутствует; файлы пока untracked |
| Дистрибуция | systemd user unit, udev, Arch PKGBUILD | Заготовки; исторический clean-package test не выполнен |

Текущая проверка: `cargo test --workspace --locked` — **299 passed,
0 failed, 0 ignored**, включая doctests; `cargo clippy --workspace
--all-targets --locked -- -D warnings` и `cargo fmt --all -- --check` прошли.
Логи этого запуска: `/tmp/lconnect-audit-{tests,clippy,fmt}.log`.
Release build, установка пакета, запуск GUI и hardware E2E в аудит не входили.

## Что необходимо поправить в понимании текущего состояния

1. **README завышает live coverage.** В smoke transcript явно не выполнены
   physical hotplug, systemd install, clean Arch package build и полноценная
   физическая проверка PWM через daemon. Restart daemon не эквивалентен
   USB disconnect/reconnect. Статусы этих проверок нужно восстановить честно.
   Найденные в smoke проблемы счётчика HID и `clear_lcd` позже исправлены
   коммитами `1af5bbe` и `bcaf7a2`; новый live-прогон этих исправлений не найден.

2. **Кривых охлаждения нет.** `DaemonRpc` содержит только fixed PWM.
   GUI spec, строки 244–253, предлагает вычислять одно PWM по текущей
   температуре при Apply. Это не автономная кривая. Windows Service
   `Galahad2VisionController.cs:2457–2470,2557–2571` выполняет расчёт по
   таймеру. Linux scheduler должен жить в daemon и работать после закрытия GUI.

3. **Архитектура пока привязана к LCDV2.** Family registry описывает имя и
   VID/PID, а не поведение. `ipc_server.rs`, `manager.rs` и keepers напрямую
   импортируют LCDV2-кодеки. `DeviceDescriptor` не описывает каналы и экраны;
   `SensorSnapshot` фиксирован на coolant/fan/pump. Обещание «новое семейство
   без изменений daemon» ещё не реализовано.

4. **Нужны разные транспорты.** `OpenHandle` сейчас предлагает output write
   и input read. Survey описывает также HID feature reports и USB endpoint
   обмен SDK-семейств. Их нельзя просто добавить строкой в registry.
   Отсутствие transport crypto подтверждалось для LCDV2; SDK-документы
   отдельно описывают DES-CBC framing некоторых LCD-семейств.

5. **LCD streaming не готов.** `SendH264Frame` — кодек, не видеоплеер.
   `UploadJpeg` cmd 15 не имеет подтверждённого способа активации на LCDV2.
   [Исследование отложено](protocol/captures/2026-04-27-cmd15-investigation.txt).
   Константа 24 fps в image crate — целевое значение из C#, не замеренная
   пропускная способность Linux реализации.

6. **Документация и упаковка разошлись.** Старый базовый дизайн говорит egui,
   последующий явно заменяет его Tauri; пути socket в старых документах
   противоречат друг другу и коду. Уточнение при составлении M0: daemon
   использует `$XDG_RUNTIME_DIR/lconnectd.sock`, а CLI по умолчанию —
   `$XDG_RUNTIME_DIR/lconnectd/lconnectd.sock`. Ранее приведённый здесь
   путь `lconnect/daemon.sock` был взят из README и не соответствовал коду.
   PKGBUILD указывает MIT/Apache при GPL-3.0-or-later в workspace и README.
   systemd unit ожидает `~/.local/bin/lconnectd`, тогда как обычный
   `cargo install` без переопределения root устанавливает в `~/.cargo/bin`.
   Тексты установки нужно проверить реальной установкой. Tracked CI workflow
   для GitHub/GitLab не найден.

## Что считать full parity

Зафиксировать матрицу **версия Windows × SKU/firmware × возможность ×
пользовательский сценарий × доказательство**. Статусы: отсутствует,
исследовано, codec-tested, integration-tested, hardware-verified,
неприменимо к SKU. Пустое или неизвестное поле не считать выполненным.

Функциональная эквивалентность допускает собственный UI и Linux-реализацию
системных интеграций. Требование копировать фирменные картинки не следует
из parity; прежняя инструкция проекта предусматривает собственные assets.
Совместимость файлов Windows — отдельная проверяемая строка матрицы.

| Пользовательская область | Нужный результат | Разрыв сегодня |
|---|---|---|
| System Info | CPU/GPU/память, выбор источника, единицы, история; дополнительные датчики по baseline | Есть только телеметрия LCDV2 |
| Cooling | Fixed/curve, пресеты, источники температуры, каналы, MB sync, start/stop там, где поддержано | Только базовый PWM LCDV2 |
| Lighting | Все применимые режимы, зоны, каналы, порядок, merge, Quick Sync, MB sync | Только локальные функции LCDV2 |
| LCD/media | Изображения, GIF/видео, crop/rotate/flip, библиотека, capture/record, playback | Один подготовленный JPEG |
| LCD themes | Sensor layouts, часы, пользовательские параметры, preview, редактор, несколько экранов | Только дизайн будущего движка |
| Wireless | Discovery, bind/unbind, идентификация, каналы/порядок, USB/RF режимы | Survey, runtime отсутствует |
| Profiles | Сохранить/выбрать/экспортировать/импортировать настройки и LCD assets | Сохраняется только текущее состояние |
| Lifecycle | Autostart, tray, reconnect, sleep/wake, корректная работа без GUI | Daemon есть, системные сценарии проверены неполно |
| Maintenance | Версии, firmware update/recovery, диагностика, обновление приложения | Identity и reset; updater отсутствует |
| Hardware breadth | Каждая применимая функция на всех SKU выбранного baseline | Один runtime driver |

Основание для cooling/RGB — официальная страница возможностей; для media —
обычное руководство; для binding и нескольких экранов — wireless manual.
Changelog расширяет baseline: FLEX, P28 V2, OLED Curved, 9.2-inch LCD,
Vector V150 INF, startup screens и более развитые редакторы. Для новых
SKU требуются свежие источники и descriptors; здесь их PID не угадываются.
FPS/overlay и взаимодействие с OpenRGB следует выделить отдельными строками:
не смешивать основной релиз и OpenRGB Beta. Если какая-то функция будет
сознательно исключена, релиз следует называть частичным parity.

## Предлагаемый roadmap

Нумерация R0–R8 новая: она не подменяет старые Phase 0–5 и не означает,
что существующие планы автоматически утверждены для выполнения.

### R0 — восстановить baseline и закрыть недоказанные обещания

- Зафиксировать версии и хеши reference binaries/SDK; составить список
  страниц, действий, SKU и firmware по C# + официальным руководствам.
  Версии/хеши и декомпиляция десяти основных сборок 2.1.28 уже получены;
  инвентаризация пользовательских сценариев ещё не завершена.
- Отделить baseline 2.1.11 от delta до 2.1.28; получить новые reference
  материалы для устройств, отсутствующих в локальном комплекте.
- Свести актуальные решения в один индекс, устранить противоречия README,
  socket paths, лицензии и installation instructions.
- Повторить hardware smoke LCDV2: PWM с измерением RPM, LCD clear,
  hotplug, daemon restart, cold boot, suspend/resume; проверить systemd
  и сборку/установку пакета в чистом окружении.
- Добавить CI для уже существующих тестов и сборки.

Выход: воспроизводимый baseline и матрица с источниками; ни один успех
автотеста не заменяет отметку о hardware verification.

### R1 — автономное охлаждение и Linux sensors

- Ввести источники датчиков с identity, единицами, timestamp и статусом
  unavailable/stale; начать с CPU/GPU/памяти, затем расширять по матрице.
- Реализовать daemon-owned curves, пресеты, сохранение, переключение
  manual/curve/MB, ограничения конкретного канала, hysteresis и fallback
  при потере датчика. Применимость pump limits и fan stop подтвердить по SKU.
- Добавить IPC/CLI для конфигурации кривых и чтения применённого состояния.
- Исследовать CPU/GPU thermal alerts из нового service 2.1.28: threshold,
  временный RGB override и restore для Galahad2Vision. Зафиксировать
  доступность через UI и добавить применимые сценарии в parity matrix.

Выход: заданная кривая продолжает менять PWM при закрытом GUI; stale sensor,
reconnect и restart имеют проверенное поведение. GUI не управляет циклом.

### R2 — usable desktop для LCDV2

- Переработать старый GUI-план под реальный IPC и R1; сохранить Tauri 2,
  Svelte 5, Tailwind и Aurora как ранее выбранное направление.
- Реализовать Dashboard, Cooling, Lighting, Display, Settings; чтение
  текущих настроек, draft/apply, ошибки, отсутствие устройства, reconnect.
- Сначала завершить один вертикальный путь: sensor → daemon → IPC → GUI
  и curve edit → daemon → устройство. Затем остальные страницы.
- Проверить отдельно frontend mock E2E, настоящий Tauri IPC и live device.

Выход: устанавливаемый desktop для повседневного использования LCDV2.
Это milestone полезности, ещё не обещание full parity.

### R3 — LCD/media parity на первом устройстве

- Подтвердить streaming path на железе: frame pacing, throughput, очередь,
  отмена и переключение источников; длительный playback вместе с polling/PWM.
- GIF/видео pipeline, библиотека assets, crop/rotate/flip, preview,
  screen capture/record через подходящие Linux desktop APIs.
- Sensor/clock themes, параметры и bindings; старое требование условий
  сохранить как требование проекта. Добавить пользовательский редактор
  вместо ограничения «только TOML», если требуемые Windows-сценарии этого требуют.
- Проверить LCD brightness/rotation/defaults и startup/offline поведение
  там, где оно предусмотрено устройством. Разобраться с cmd 15 отдельным
  исследованием, не блокируя уже рабочий transient cmd 14.

Выход: проверенные display-сценарии LCDV2; опубликованные реальные ограничения
частоты кадров и ресурсов, без подмены их константой из SDK.

### R4 — настоящая поддержка разных устройств

**Отложено до появления второго устройства.** Не блокирует LCDV2 parity.

- Ввести family driver boundary и capabilities: каналы охлаждения, зоны
  освещения, экраны, допустимые режимы/диапазоны, firmware quirks.
- Разделить logical device identity и transport interfaces; предусмотреть
  устройство с несколькими USB interfaces и USB/RF представлениями.
- Добавлять HID feature/USB endpoints по потребностям конкретного второго
  семейства. Выбрать его по доступности реального железа и полезности.
- Реализовать multi-device UI и сохранение по стабильной identity.

Выход: LCDV2 и второе семейство работают вместе; добавление второго не
требует копирования общей логики daemon и новых жёстких допущений в GUI.
Минимальные capabilities стоит вводить уже в R1/R2; окончательную форму
проверять на реальном втором драйвере.

### R5 — расширение hardware coverage

**Отложено.** Сейчас допустимо только исследование документов; тестировать
и заявлять поддержку этих устройств на доступном LCDV2 невозможно.

Рабочие группы, а не обещанный порядок покупки железа:

1. Wired AIO/HID: GA II variants, Trinity, HydroShift I; TL controller/LCD.
2. ENE/feature-report: SL/AL/SL-INF и V2, Strimer; варианты отдельно.
3. USB SDK screens: SL/TL LCD wireless в USB mode, HydroShift II,
   Lancool 207, Universal Screen; перенос wire protocol из SDK в Linux.
4. Wireless: dongle discovery, binding, routing, группы, firmware variants,
   USB/RF переключение и устранение двойного отображения одного устройства.
5. Delta новых релизов: FLEX, OLED Curved и остальные отсутствующие SKU.

Выход каждой группы: capabilities, reference fixtures, ошибки/recovery и
hardware acceptance на конкретном SKU/FW. Survey ускоряет работу, но
не позволяет объявить поддержку без железа или внешнего проверяющего.
R4/R5 могут идти одновременно с R3 при наличии участников и устройств.

### R6 — общие пользовательские сценарии

В активном плане выполняется только применимая к LCDV2 часть: его эффекты,
профили, редактор, системные интеграции. Multi-device, wireless и функции
экранов других SKU остаются отложенными вместе с R4/R5.

- Quick Sync/Merge, порядок устройств и групп, применимые эффекты и зоны.
- Несколько LCD, общие media layouts и синхронизация воспроизведения;
  расширенные 2D/3D сценарии редактора согласно зафиксированному baseline.
- Profiles, импорт/экспорт настроек и assets; отдельно изучить форматы
  Windows и миграции. Tray, локализация, единицы, diagnostic bundle.
- Закрыть оставшиеся sensor/FPS/overlay строки матрицы Linux-эквивалентами
  либо явно обозначить отсутствие parity для них.

Выход: пользовательские workflows проходят целиком, включая перезапуск
и восстановление состояния; поддерживаются комбинации разных устройств.

### R7 — firmware lifecycle

Активная исследовательская ветка — update/recovery именно LCDV2. ENE
bootloader не является его updater; ENE-реализация отложена до железа.

- Использовать ENE-план как исследовательскую заготовку только для ENE;
  не считать его универсальным updater для всех Lian Li.
- Для каждого семейства определить format/target validation, bootloader,
  update transport, progress, restart и проверку результата.
- Координировать updater с daemon, исследовать recovery и прерывание
  обновления отдельно на подходящем тестовом устройстве.

Выход: online/local update и проверенный recovery для применимых семейств.
До этого допускается полезный релиз с явно отсутствующим firmware parity.

### R8 — release acceptance и сопровождение

Ближайший release gate ограничен LCDV2. Неприменимые к нему возможности
других продуктов не являются пробелами его parity; неизвестные возможности
самого LCDV2 остаются открытыми до проверки.

- Пройти всю матрицу выбранного Windows baseline, включая restart,
  cold boot, sleep/wake, потерю датчиков и устройств, несколько клиентов.
- Проверить CPU/RAM, задержки UI, устойчивость media streaming и длительное
  использование. Согласовать измеримые бюджеты после R0-замеров.
- Завершить packaging/upgrade/uninstall, release artifacts, CI и documented
  hardware support. Расширять дистрибуцию после рабочего Arch пути.
- Новые Windows-релизы вести как delta к зафиксированному baseline.

Выход: заявление о parity содержит версию, список SKU/FW и доказательства.
Общая оценка в процентах до полной матрицы будет вводить в заблуждение.

## Ближайший пакет работ

Начать с R0, затем R1 и минимального вертикального среза R2. Не исполнять
старый GUI-план на 5 тысяч строк без пересмотра его предпосылок.

Активная последовательность с доступным железом:
**R0 → R1 → R2 → R3 → LCDV2-часть R6/R7 → R8 для LCDV2**.
R4/R5 и межустройственные сценарии R6 отложены. Версию Windows следует
выбрать как baseline применимых LCDV2-сценариев; новые FLEX/OLED модели
не расширяют текущий hardware scope.
Длительность полного roadmap пока не оценивать в неделях: основные
неизвестные — свежесть reverse-engineering материалов, доступность железа,
streaming и wireless/firmware. Оценивать небольшие этапы после их исследования.

На момент начала аудита уже были изменены основной design и untracked
`.claude/`, `.superpowers/`, ENE spec/plan. Они сохранены без изменений;
этот документ — единственное добавление аудита в repository.
Последующим дополнением создан документ о reference 2.1.28 и ll-connect3;
декомпиляция и checkout стороннего проекта находятся вне Linux repository.
