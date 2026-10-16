# LCD themes: software acceptance

2026-10-11. Матрица покрывает реализованный контракт темы LCDV2 и редактор.
Точный Windows locale/fallback/raster остаётся в [font catalog](font-catalog.md)
и базовом плане; совпадение Linux preview/JPEG не доказывает Windows pixel parity
или физический LCD output. Effects/Style Express и media editor — M4.

| Область | Проверяемое поведение | Доказательство |
|---|---|---|
| Background | Solid; horizontal/vertical/radial, оба цвета, Flip, composition поверх фона | image `gradient_backgrounds_preserve_solid_and_compose_below_layers`; browser `background.spec.ts`; [native gradient checkpoint](native-desktop-smoke.md) |
| Image fit | Crop, stretch, contain, contain_native; portrait/landscape bounds и центрирование | Новый `image_fit_modes_keep_expected_portrait_and_landscape_bounds`; `extreme_aspect_crop_is_centered_and_empty_assets_fail`, `native_contain_does_not_enlarge_small_assets_but_downscales_large_ones` |
| Alpha/opacity/shadow | Фон просвечивает, zero opacity не рисует, shadow следует alpha, включая off-canvas alpha | `image_alpha_and_contain_preserve_underlying_background`, `watermark_shadow_follows_alpha_and_layer_opacity`, `off_canvas_watermark_alpha_can_cast_a_visible_shadow` |
| Placement/order/clipping | Watermark center и fitted size, края0/100; перестановка overlapping layers меняет верхний цвет; zero opacity возвращает нижний слой | `watermark_center_uses_fitted_size_and_clips_at_canvas_edges`; новый `overlapping_image_order_and_zero_opacity_survive_right_bottom_clipping`; browser `layer-keyboard.spec.ts` |
| Fonts/text styles | Bundled и managed fonts, size/em, color/opacity/shadow, line boxes/leading/anchors | image theme tests fonts/shadows/managed roles/line boxes; [em sizing](text-em-sizing.md), [shaping/fallback](text-shaping-acceptance.md) |
| Unicode/wrapping | Shaping/bidi, grapheme fallback, normal wrapping/blank lines, hanging spaces, tabs/manual soft hyphen | [Точная matrix](text-wrapping-acceptance.md), image `wrapped_text_and_vertical_anchors_match_explicit_lines`; Windows locale-specific differences отдельно |
| Sensors | °C/°F, units/labels/parts colors, missing/stale/zero/recovery; mixed host/device | [Sensor contract](sensor-consumer-acceptance.md#общий-software-контракт-readings--lcd--2026-10-11), image `sensor_parts_render_independent_colors_and_preserve_missing_value` |
| Clock | 12/24h/seconds, date/leap day/midnight/noon/AM-PM и отдельные стили | image `clock_format_and_unicode_multiline_are_deterministic`, `clock_parts_render_date_midnight_noon_and_independent_styles`; Clock preset browser |
| Presets/Cycle | Focus/Duo/Triple/Quad/Cycle/Clock, explicit binding, draft до Apply; Cycle boundaries/list/restart | `presets.spec.ts`, [Cycle acceptance](cycle-acceptance.md) |
| Editor/preview/Apply | Редактирование font/layout/shadow/watermark/sensor parts сериализует ожидаемые поля; preview не пишет устройство; coalescing/obsolete errors | `display.spec.ts`, `live-preview.spec.ts`; [native flows](native-desktop-smoke.md), [ordering/runtime](theme-preparation-order.md) |
| Assets/restart | Formats/EXIF, retention/cleanup, corruption/missing, repair и serial identity | [Image import](image-import-acceptance.md), [общий persistence](restart-dependency-acceptance.md#corrupt-dependency-при-restart--2026-10-11) |

Имена image tests относятся к `crates/lconnect-image/tests/theme.rs`, browser
files — к `desktop/tests/`. Schema bounds/unknown fields и nested styles
проверяются дополнительно в `crates/lconnect-ipc/tests/theme.rs`.

## Новый checkpoint и границы

[Evidence](evidence/2026-10-11-theme-controls.json) фиксирует source hashes,
Rust и browser проверки. Два новых renderer regressions закрывают exact bounds
и совместный order/opacity/edge сценарий. Одноцветный fit fixture не проверяет
полный алгоритм resampling; crop content/downscale проверяются отдельно
существующими тестами. Первый overlap fixture ошибочно использовал запрещённый
обычный rectangle за canvas и был исправлен на supported watermark positioning;
production не менялся.

Общий software пункт темы закрыт этой совокупностью проверок. Он не закрывает
общую GUI accessibility, каждый native dialog/compositor, точную Windows
типографику, все возможные сочетания пользовательских настроек, M4 effects
или аппаратную приёмку. Эти требования сохраняются в плане и hardware checklist.
