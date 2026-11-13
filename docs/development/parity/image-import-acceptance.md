# Static image import: software acceptance

2026-10-11. Обработка PNG/JPEG/WebP/BMP отделена от GIF/MP4 playback:
animated inputs пока дают первый кадр, полноценное воспроизведение — M4.

## EXIF orientation

Раньше `ImageReader::decode`/`load_from_memory` оставляли encoded pixel order;
normalization затем удаляла metadata. Фотография с EXIF orientation теряла
поворот или отражение. Shared `decode_oriented` теперь читает orientation через
`ImageDecoder` и преобразует pixels перед fit/normalization. Его используют
managed assets (Tauri и daemon), CLI byte input и file path input.

Сохранены decoder limits и отдельная проверка бюджета выходного buffer, которую
выполнял `ImageReader::decode`. Assets остаются ограничены16MiB encoded,
8192px source dimension и64MiB decode budget; CLI сохраняет прежние default
limits image crate. Поворот90/270 может создавать дополнительный image buffer;
64MiB — не обещание общего RSS всего pipeline. Заново добавленных dependencies нет.

После нормализации PNG содержит уже ориентированные pixels без исходного EXIF;
повторный decode не применяет преобразование второй раз. Уже импортированные
до исправления assets автоматически не исправляются: их исходное metadata
потеряно, потребуется повторный импорт оригинала.

## Матрица

| Требование | Проверка |
|---|---|
| PNG/JPEG/WebP/BMP → managed PNG | `llcontrol-image/tests/orientation.rs::all_static_import_formats_normalize_without_changing_decoded_pixels`: размеры и все decoded RGBA pixels, включая lossy JPEG decode, сохранены |
| EXIF1…8, зеркальные и повёрнутые варианты | `exif_orientation_is_applied_once_before_normalization_and_lcd_fit`: synthetic JPEG APP1, независимые coordinate mappings каждого pixel, normalization и LCD letterbox JPEG |
| Ошибка воспроизведена до исправления | EXIF2 возвращал исходные pixels вместо горизонтального отражения |
| Corrupt/truncated/oversized | Truncated headers всех4 formats; `assets.rs::malformed_large_and_overdimension_images_fail`, encoded и dimension limits |
| Alpha, oversampling, IPC budget | `normalization_preserves_alpha_and_bounds_dimensions`, `incompressible_rgba_upload_fits_wire_budget_after_normalization` |
| Store/reload/invalid asset | `llcontrol-daemon/tests/theme_assets.rs::import_persists_and_reload_requires_all_referenced_assets` и nonregular/corrupt tests |
| Preview и Apply | [Native GUI acceptance](native-desktop-smoke.md), `theme_ipc` failed dependency preserves active composition |
| Удаление используемого asset и recovery | [Cleanup matrix](cleanup-acceptance.md), [restart dependencies](restart-dependency-acceptance.md) |

[Текущие результаты](evidence/2026-10-11-image-orientation.json).
Новая EXIF regression работает в image crate; file-path wrapper использует тот
же decoder, но отдельный EXIF CLI process/native GUI run не выполнялся. Native
checkpoints выше предшествуют этой correction. Полное визуальное сравнение с
Windows, цветовые профили/ICC, физический LCD и долгий прогон не заявляются.
