# Прерванные manual/thermal mutations

2026-10-11. `desktop/tests/lighting-interrupted.spec.ts` проверяет ручной RGB
и thermal alerts по одной матрице из шести исходов:

| Исход | Ожидаемый результат |
|---|---|
| Ответ потерян после принятия команды | Ошибка видна; polling/explicit Load отражает принятое состояние; повторной записи нет |
| Отказ до принятия | Ошибка видна; прежнее состояние сохранено; повторной записи нет |
| A→B→A, затем поздний success | Старый success не показывается, новый draft сохранён |
| A→B→A, затем поздний error | Старая ошибка не показывается, новый draft сохранён |
| Reconnect, затем поздний success | Success старого соединения отброшен; draft сохранён |
| Reconnect, затем поздний error | Error старого соединения отброшен; draft сохранён |

В каждом случае пользователь отправляет91, затем меняет поле на94 до ответа.
Запрос содержит91 и device A; после завершения и свежего polling поле остаётся94.
Для thermal explicit Load возвращает91 после принятия либо80 после отказа.
Для manual новое readback значение grace10/0 проверяет возобновление polling.
Все12 случаев требуют ровно один mutation RPC и отсутствие ложного success.
Существующие2 serialization tests проверяют взаимную блокировку Apply
manual/dynamic/thermal и освобождение после success/error.

[Результаты](evidence/2026-10-11-lighting-interrupted.json). Production код не менялся.
Это browser fixture, имитирующий момент принятия и ответ; не реальный daemon
commit, не hardware и не новый native fault injection. Manual readback здесь
подтверждает grace/status, а не физический цвет или полное содержимое RGB base.
Native happy paths/reconnect, daemon thermal restoration и транспортные ошибки
покрыты отдельными checkpoints. Итоговая приёмка дерева и native accessibility
остаются открытыми.
