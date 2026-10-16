# LCD theme replay: отсутствующий asset при запуске

2026-10-10. Изолированный native WebKitGTK → Unix IPC → daemon fixture с
MockBackend. Файлы и сокеты находятся в отдельном `/tmp/lconnect-native-*`;
production daemon и реальные устройства не запускаются.

## Сценарии

Для managed font и image отдельно:

1. Применить и сохранить полную тему; создать независимый editor draft.
2. Завершить fixture с final flush, сохранить все parsed device settings,
   удалить только выбранный dependency и запустить новый fixture.
3. GUI видит disconnect/reconnect, затем `theme restore:` error. Через IPC
   source=Idle, active theme=null; fresh mock statistics показывают0 JPEG starts.
   Editor draft и содержимое сохранённого screen profile остаются прежними.
4. Завершить fixture и сравнить все parsed device settings с исходными:
   failed replay не стирает theme и соседние настройки.
5. Вернуть те же bytes dependency и перезапустить fixture. Без нового Apply
   source становится Theme, ошибка очищается, active theme полностью совпадает
   с сохранённой. Несохранённый draft не применяется; явный Load снова работает.

Четыре restart markers дополняют восемь существующих live missing/corrupt
fault cases. Старый stats.json удаляется до старта; harness ждёт новый файл,
поэтому статистика прошлого процесса не считается результатом replay.

## Найденное исправление IPC

`get_lcd_theme` законно возвращает JSON `result: null`, когда активной темы нет.
Обычный serde `Option<Value>` терял отличие null от отсутствующего `result`;
клиент ошибочно сообщал `neither result nor error set`. Response decoder теперь
сохраняет present null, а `error` при наличии обязан быть объектом.
Повреждённый envelope с известным request ID завершает только соответствующий
запрос ошибкой; следующие запросы на том же соединении продолжают работать.
Unit mutations сохраняют прежний wire payload `{}`.

`crates/lconnect-ipc/tests/null_response.rs` проверяет null/missing/conflict,
error:null, malformed error, typed application error и последующий успешный
null на том же соединении. Native flow воспроизводит исходный пользовательский
сценарий отсутствующей темы через Tauri bridge.

## Границы

Нет утверждения о том, какие pixels физический LCD удержит после failed replay.
Проверены отсутствие нового mock JPEG stream, корректный software state и
восстановление файлов. В этом native checkpoint startup с corrupt dependency отдельно не инъецирован;
live missing/corrupt проверки остаются в предыдущем checkpoint. Общие
interrupted mutations и cleanup matrix на дату этого checkpoint оставались открыты; актуальное состояние — в [чеклисте](basic-parity-checklist.md). Аппаратная приёмка отложена.

[Итоговый checkpoint](evidence/2026-10-10-native-restart-dependencies.json):
564 Rust tests, fmt/strict Clippy/docs и полный native smoke прошли.

## Corrupt dependency при restart — 2026-10-11

`theme_ipc::corrupt_image_or_font_on_restart_preserves_intent_and_recovers_without_apply`
закрывает отдельный startup-corrupt сценарий через настоящий manager/actors
и MockBackend. Исходная тема применяется через daemon RPC и содержит managed
PNG плюс sensor layer с user font, стабильным CPU binding и Fahrenheit.
Preview исходной темы задаёт ожидаемый **полный** JPEG packet batch.

Для image и font независимо выполняются два restart:

- Повреждённый файл: Idle, ошибка `theme restore`, активная тема отсутствует;
  до joined shutdown нет ни одного нового JPEG-start packet.
- Возврат точных managed bytes: source Theme, ошибка исчезает, исходная тема
  и полный JPEG batch восстанавливаются без нового Apply.

Каждый запуск получает новый store и MockBackend: cache и история предыдущего
устройства не скрывают повреждение. После каждого shutdown сравнивается весь
сериализованный DeviceSettings, включая theme/assets/bindings; соседние поля
в этом fixture имеют defaults. Это не отдельная проверка всех customized settings.
Для PNG возвращаются нормализованные bytes из store, а не исходный import file.

[Evidence](evidence/2026-10-11-corrupt-restart.json). Production исправлений для
этого сценария не потребовалось. Это mock integration, не новый native fault
injection и не проверка картинки на железе.

Общий software критерий persistence теперь опирается на эту проверку,
[missing dependency native restart](evidence/2026-10-10-native-restart-dependencies.json),
[identity replay и disk failure/retry/final flush](persistence-acceptance.md),
[schema preservation](schema-compatibility.md). Он закрыт; физический replay,
power loss и конечная приёмка всей базы остаются отдельными требованиями.
