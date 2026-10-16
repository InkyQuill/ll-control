# Внешние изменения controls и LCD

2026-10-11. Browser fixture имитирует новую конфигурацию от другого клиента
в ответах обычного polling. Несохранённые GUI поля не должны загружаться
автоматически; кнопка явной загрузки должна взять последний daemon state.

| Сценарий | Проверка |
|---|---|
| Cooling: внешний fixed60→72, локальный PWM83 | Applied обновляется до72, draft остаётся83; Load меняет его на72 |
| Thermal: внешний threshold80→95, локальный91 | Active source обновляется, draft91 сохраняется; Load возвращает95 |
| Dynamic RGB: внешний диапазон30…60→25…75, локальный low44 | Applied value обновляется, draft44 сохраняется; Load берёт25…75 |
| Dynamic RGB: одновременно сменился base mode static→breathing | Draft44 остаётся, Apply заблокирован и причина видна; Load снимает блокировку |
| Settings: внешний brightness60→72/rotation90→270, локальные83/180 | Requested обновляется, оба draft поля сохраняются; Load берёт72/270 |
| Display: внешняя замена темы при локальном имени | Persistence status обновляется без перезаписи draft; Load берёт новую тему |
| Display: внешний Clear | Источник Idle, draft сохранён; Load сообщает отсутствие активной темы и оставляет правки |

`desktop/tests/external-controls.spec.ts`:7 scenarios. Каждый завершается
проверкой отсутствия `set_*`, `patch_lcd_config` и `clear_lcd`: polling и явная загрузка редактируют только UI.
В предыдущем checkpoint первые4 сценария вместе с control-drafts и
dynamic-rgb-reconnect дали9 passed. Текущие7 вместе с reconnect и
settings-library-reconnect дали14 passed.
Последние покрывают late cooling Apply при смене устройства, late preset после
редактирования, thermal reconnect/device binding и late dynamic reply.

[Evidence](evidence/2026-10-11-external-controls.json). Production код не менялся.
Это browser transport fixture, не два настоящих Unix IPC клиента и не hardware.
Последовательные явные Apply разных клиентов не имеют общей optimistic revision
для этих controls; этот тест не заявляет автоматическое разрешение конфликтов
или запрет намеренного перезаписывания последней daemon конфигурации.

Display/Settings checkpoint: [evidence](evidence/2026-10-11-external-display-settings.json).
Проверяется UI на ответах fixture, не конкурентный commit двух реальных клиентов.
Interrupted manual/thermal browser workflows [проверены](lighting-interrupted-acceptance.md).
Остаются отдельными gates: native доступность и итоговая совместная
приёмка. Profile/source-editor revision conflicts проверяются своими suites.
