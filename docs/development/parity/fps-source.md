# FPS: runtime settings and MangoHud per-frame source

Baseline: Windows L-Connect3 2.1.28.0; software checkpoint2026-10-16.
Mini — Plasma widget / GNOME Shell extension, без отдельного окна. EN default,
RU обязателен. Actual games/shell-session/hardware qualification отложена.

## Windows boundary

Frozen `L-Connect-Service/LConnectService/Utils/HWiNFO/Managers/`
`HWiNFOFrameratePresentedAvgSensorManager.cs` выбирает первый PresentMon reading
с точным именем `Framerate Presented (avg)`. `GetValue` использует DefaultId,
не arithmetic mean всех GPU/apps. `SystemInformation.cs:188–197` публикует
FrameratePresentedAvg и PresentMonAppName только при IsHWiNFOPresentMonEnabled;
иначе null/пустое имя. Linux источник ниже не объявляется тем же HWiNFO statistic.

## Исследованные Linux sources

[MangoHud pinned logging.cpp](https://github.com/flightlessmango/MangoHud/blob/e1430abaa024c57624661b99314ba8f2d4c06d03/src/logging.cpp)
пишет CSV и flush после записи. Его `fps` — reciprocal длительности отдельного
кадра; это устанавливается по [overlay.cpp](https://github.com/flightlessmango/MangoHud/blob/e1430abaa024c57624661b99314ba8f2d4c06d03/src/overlay.cpp).
При log_interval=0 logger вызывается в frame path; при ненулевом interval пишет
периодический thread. Прямое отображение колонки fps не принимаем за среднее.

[Gamescope pinned steamcompmgr.cpp](https://github.com/ValveSoftware/gamescope/blob/2b5cf191d1047d6895516a834de8a72ac1c342f3/src/steamcompmgr.cpp)
считает stats-path FPS в paint_all каждые300 compositor frames. Это не FPS
приложения; этот feed не подключён. MangoHud control socket управляет HUD/logging,
но не предоставляет read-only FPS query. Compositor refresh/GPU load также не источники.

## Подключённый provider

Явный opt-in для **одного уже созданного лога** доступен в Settings → FPS source:
Read FPS, абсолютный путь к MangoHud CSV и Save FPS settings. Настройки глобальные,
доступны без подключённого устройства и применяются работающим daemon на следующем
цикле host collector. UI по умолчанию английский; русский обязателен и подключён.
Выключение сохраняет выбранный путь, но немедленно отзывает старое значение FPS.
Повторное включение или смена файла начинает новый warmup и пропускает старые rows.

CLI использует тот же IPC и optimistic revision:

```sh
lconnectctl fps show --json
lconnectctl fps set --revision 0 --enabled true --log-path /absolute/path/to/game.csv
lconnectctl fps set --revision 1 --enabled false --log-path /absolute/path/to/game.csv
```

Set заменяет весь record; без --log-path очищает путь. Используйте revision из show.
IPC get_fps_source({}) / set_fps_source({settings}) хранит version1/revision/enabled/
log_path в fps-source.json рядом с глобальными настройками. CAS, file lock, atomic
replacement и bounded retained worker защищают от конкурентных записей и потери
ответа. После неподтверждённой записи UI требует явно загрузить сохранённое
состояние; polling не затирает draft. Изменение отзывает только FPS samples, в том
числе запоздалую публикацию старого collector; CPU/GPU readings сохраняются.

Legacy LCONNECT_MANGOHUD_PER_FRAME_LOG=/absolute/path/to/game.csv остаётся startup
fallback только при отсутствии сохранённого record. Сохранённое disabled имеет
приоритет после restart. Некорректный сохранённый файл не включает fallback.
Путь absolute, UTF-8 ≤384bytes, без control characters; metadata/JSON read bounded,
no-follow regular file. Ни get, ни Save не включают MangoHud logging.

Пользователь отдельно настраивает MangoHud per-frame logging (log_interval=0).
Приложение не запускает игру, не внедряет layer, не включает logging, не ищет
«самую новую игру» в директории и не меняет MangoHud config. Автоматический выбор
активного приложения и capture activation workflow остаются открытыми.

Поддержан exact16-column CSV header исследованного source, включая optional
versioning preamble; неизвестный schema не угадывается. При первом подключении,
restart/sleep invalidation или replacement имеющиеся benchmark rows пропускаются.
Читаются только новые complete records. Mean = N×1000/Σframetime_ms записанных
кадров; не arithmetic average instantaneous FPS. fps/frametime должны быть finite,
positive и согласованными, elapsed_ns строго возрастать. Этот mean соответствует
записанным кадрам; provider не может доказать режим logging другого процесса.
Ненулевой log_interval не входит в поддержанный capture contract. Source label
явно называет MangoHud logged frame mean, не Presented/Displayed/active app FPS.

Ограничения: только absolute regular no-follow file; O_NONBLOCK позволяет
отвергнуть FIFO без ожидания writer; header≤8KiB, batch≤256KiB, row≤1024bytes.
Backlog overflow/malformed/replayed records отзывают значение; следующий attachment
проходит warmup. Unchanged bytes — unavailable, а не valid0/продление старого sample.
Общий cache дополнительно отзывает данные старше3s. Blocking reads остаются в
host collector вне executor/HID. Kernel/filesystem stall нельзя force-interrupt;
расширенная I/O/cancellation/performance acceptance ещё нужна.

ID `mangohud:<hex canonical path>:<dev hex>:<inode hex>:logged_frame_mean`
описывает file session, не GPU или portable process identity. Replacement inode
даёт новый source ID: сохранённый выбор не переназначается. Truncation/restart
проходят warmup. Source path и basename — данные; неизвестные labels не переводятся.

## Consumers

Unit `frames_per_second` сериализуется в existing SensorReading/cache/IPC; LCD
sensor layer и theme preview форматируют ` FPS`, без °C/°F conversion. В Mini оба
adapters показывают FPS source selector и явно сохраняют source ID. Null означает
«источник не выбран», без shared CPU/GPU fallback. Выбор другого лога, duplicate
ID, unit mismatch, stale/offline/invalid не подставляют другой source. Старые slots
с fps/source=null остаются valid и unavailable. Источник настраивается в основном приложении или CLI; внутри widget нет
filesystem/game polling. Mini остаётся Plasma widget / GNOME Shell extension;
Sway и другие окружения отложены, список поддержанных shell/versions ещё не утверждён.

[Evidence](evidence/2026-10-16-mangohud-fps.json) разделяет synthetic file→cache→IPC→
LCD JPEG, Node model, Qt/QML keyboard и private GTK/GSettings проверки.
Real games, полный GNOME panel/Plasma session и capture→game compatibility не проверялись.

## Оставшаяся приёмка

- [ ] Window/aggregation semantics и app identity сопоставить с Windows PresentMon;
  logged frame mean не объявлять закрытием exact FrameratePresentedAvg parity.
- [x] Сохранённые runtime enable/disable/путь в основном приложении EN/RU и CLI,
  CAS/draft/conflict/lost-reply/restart и FPS-only invalidation.
  [Software/native checkpoint](evidence/2026-10-16-fps-runtime-settings.json).
- [ ] Capture activation/app selection и реальные permissions/errors/IO matrix;
  не изменять игру/MangoHud без явного действия.
- [ ] Определить поддержанные MangoHud releases/schema и проверить реальный
  per-frame logging, frame loss/IO cost/stall/paused game, focus/app changes.
- [ ] Запуск/закрытие/rotation/restart/sleep, inode/path reuse и retained widget
  binding; stalled filesystem/cancellation/resource matrix.
- [ ] Real Plasma/GNOME Shell, EN/RU layout/accessibility и missing-source UX.
- [ ] Совместная media+sensors+cooling+RGB нагрузка без LCD stalls; реальные проверки позже.

## GUI diagnostics — 2026-10-16

Backend read/save diagnostics теперь literal; собственный unsupported-response
resource и recovery guidance переводятся при render. Языковая смена не повторяет
Save и не снимает explicit Reload guard. EN/RU read/save collisions и retained
validation квалифицированы в [settings evidence](evidence/2026-10-16-settings-messages.json);
новые FPS native fault injections остаются отдельной проверкой.
