# Transport consumer independence: software acceptance

Сверено2026-10-11. Три направления отказа проверены через native WebKitGTK,
Unix IPC, настоящий daemon и private MockBackend. Производственный код
не менялся; hardware recovery этим документом не подтверждается.

| Ошибка одного consumer | Остальные consumers | Сохранение и recovery | Evidence |
|---|---|---|---|
| Pump RGB A/131 ShortWrite0 | JPEG starts+3; fan61 applied | RGB last_error, binding сохранён; автоматический retry; fan55 восстановлен | [RGB](evidence/2026-10-11-native-rgb-transport.json) |
| JPEG B/14 ShortWrite0 | Fan62 applied; RGB writes+3, PWM writes+3 | LCD Idle/error, active theme=null; persisted TOML theme неизменна; explicit Apply возвращает Theme | [LCD/PWM](evidence/2026-10-11-native-lcd-pwm-transport.json) |
| Fan PWM A/139 ShortWrite0 | LCD Theme без ошибки, JPEG starts+3; RGB writes+3; pump без ошибки | applied_percent=null, curve сохранена; автоматический retry восстанавливает duty; fan55 восстановлен | [LCD/PWM](evidence/2026-10-11-native-lcd-pwm-transport.json) |

Полный native workflow и final restart/replay прошли. Последний Rust прогон:
587 tests, fmt, strict Clippy и doc passed. После него исправлено только
ожидание JS и Python-проверка TOML; GUI пересобран, native smoke повторён.

## Дополняющее компонентное покрытие

Пути ниже относительно `crates/llcontrol-daemon/tests`:

- `cooling.rs::rpm_binding_rejected_and_write_failure_retains_intent_without_applied_claim`:
  pump source/write error сохраняет intent без applied PWM.
- `lighting_owner.rs::direct_data_failure_after_preparation_does_not_commit_and_retry_prepares_again`:
  ошибка cmd20 после подготовки не становится сохранённой/refresh base;
  explicit retry повторяет подготовку и завершает передачу.
- `thermal.rs::partial_failure_retry_stale_recovery_and_direct_restore_failure`:
  fan/pump zones имеют отдельные errors, retry снимает ошибку pump.
- `lcd_keeper.rs::first_and_later_frame_failures_stop_without_retry`:
  ранняя/поздняя H264 frame error останавливает producer без скрытого retry.
- `hid_io.rs::frame_batch_stops_at_first_failure_and_counts_prior_success`:
  partial batch прекращается, учитываются только успешные reports.
- `lcd_keeper.rs::image_replacement_waits_for_old_batch_and_replies_after_new_batch`
  и `clip_clear_and_jpeg_replace_cancel_remaining_gop_at_boundary`:
  сериализация replacement/clear на границе batch.

Базовая software-проверка независимости владельцев LCD/cooling/lighting закрыта.
Disconnect/cancel/preparation проверены отдельно в
[ordering matrix](theme-preparation-order.md); её базовый software gate также закрыт. Одновременные отказы нескольких команд, зависший
физический USB, firmware resync, физические RPM/LED/pixels и latency остаются
за границами этого доказательства. Native cmd20/H264 fault отдельно не запускался;
их контракты подтверждены перечисленными component tests.
