# Cycle: software acceptance

Сверено2026-10-11. Cycle переключает весь binding по monotonic elapsed time:
label, value, unit и температурное преобразование относятся к одному источнику.
Missing/stale slot остаётся на своём месте и показывает unavailable.

| Требование | Доказательство |
|---|---|
| До/на границе interval, wrap, большой elapsed | `llcontrol-image/tests/theme.rs::sensor_cycle_switches_on_boundaries_keeps_missing_slot_and_wraps` сравнивает каждый кадр с выбранным single-sensor layer |
| Раздельные стили value/label/unit | Renderer test `sensor_parts_render_independent_colors_and_preserve_missing_value` и runtime test ниже; renderer сравнивает с явными Text layers |
| Настоящий cache stale>3s, recovery, missing без пропуска slot | `llcontrol-daemon/tests/theme_runtime.rs::cycle_cache_staleness_and_reordered_bindings_keep_label_value_unit_together`: distinct CPU°F/PowerW JPEG, single-sensor oracle, восстановление прежних bytes |
| Перестановка и сокращение списка | Тот же runtime test: reverse order, затем один Power binding; его JPEG сохраняется на большом elapsed |
| После отключения IPC-клиента переключение продолжается | `llcontrol-daemon/tests/theme_ipc.rs::cycle_continues_after_ipc_disconnect_and_replays_after_manager_restart`: после drop client/abort connection server в новой истории MockBackend появляются обе полные JPEG packet sequences |
| Persistence и restart | Тот же IPC test: orderly shutdown → новый manager → readback неизменной Cycle theme → обе полные packet sequences без подключения клиента |
| Редактирование списка клавиатурой | [Cycle focus regression](evidence/2026-10-11-cycle-keyboard.json) проверяет удаление и сохранение keyboard focus |

IPC test использует приватный settings root, memory transport и MockBackend.
Оба ожидаемых кадра получены через preview RPC, проверены на различие и
упакованы production JPEG codec. Сравнивается полная последовательность reports,
а не счётчик запросов или первый пакет. Snapshot истории снимается после
отключения клиента, затем отдельно перед restart. Свежие реальные sensors,
HID и native GUI в этом тесте не используются.

Начальный тест ошибочно ожидал рост `sent_frames`: этот счётчик относится к
H.264 diagnostic playback. Timeout сохранён в review log; oracle исправлен на
реальные mock writes без изменения production кода. Это не обнаруженный runtime
дефект и не причина расширять семантику playback stats.

[Результаты проверки](evidence/2026-10-11-cycle-acceptance.json).
Software Cycle gate закрывается совокупностью перечисленных проверок. Native
закрытие GUI имеет общий theme-owner acceptance; отдельный native Cycle close
не запускался. Видимые pixels, долгий прогон и восстановление после физического
USB/sleep остаются в аппаратном чеклисте. Точная Windows phase continuity при
restart не заявляется: новый runtime начинает отсчёт с первого binding.
