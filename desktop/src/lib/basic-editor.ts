import type { BasicBackground, BasicEditorState, BasicOverlay, Sensor, Theme } from './api';
import { presetTheme } from './theme-presets';
import { lcdPageKind } from './lcd-pages';
import { ValidationError } from './validation-error';
export const basicOverlays: readonly BasicOverlay[] = [
  'none',
  'slider_sensor',
  'duo_sensor',
  'three_sensor',
  'four_sensor',
  'time',
  'custom',
];
/** Admission of cache structure only. Native validation still owns asset/style limits. */
function editorState(theme: Theme): BasicEditorState {
  const editor = theme.basic_editor;
  if (!editor || lcdPageKind(theme) !== 'basic')
    throw new ValidationError('This theme has no saved background and text presets.');
  const backgrounds = editor.banks.map((bank) => bank.background);
  if (
    !backgrounds.length ||
    backgrounds.length > 2 ||
    new Set(backgrounds).size !== backgrounds.length ||
    !backgrounds.includes(editor.selected_background)
  )
    throw new ValidationError('Saved background presets are invalid. Reload the theme.');
  for (const bank of editor.banks) {
    const overlays = bank.overlays.map((item) => item.overlay);
    if (
      !['image_video', 'background_color'].includes(bank.background) ||
      overlays.length !== 7 ||
      new Set(overlays).size !== 7 ||
      !overlays.every((kind) => basicOverlays.includes(kind)) ||
      !overlays.includes(bank.selected_overlay) ||
      bank.overlays.some(
        (item) =>
          item.theme.basic_editor ||
          item.theme.page_cache?.length ||
          lcdPageKind(item.theme) !== 'basic',
      )
    )
      throw new ValidationError('Saved text presets are invalid. Reload the theme.');
  }
  return editor;
}
/** Local draft operation. Preserve active edits before selecting a saved template. */
export function selectBasicEditor(
  theme: Theme,
  background: BasicBackground,
  overlay: BasicOverlay,
): Theme {
  editorState(theme);
  const current = structuredClone(theme),
    editor = current.basic_editor!;
  delete current.basic_editor;
  delete current.page_cache;
  const active = editor.banks.find((bank) => bank.background === editor.selected_background)!;
  const isMark = (layer: Theme['layers'][number]) =>
    layer.kind === 'image' && !!layer.watermark_position;
  const marks = current.layers.filter(isMark);
  for (const item of active.overlays) {
    item.theme.version = current.version;
    item.theme.background = structuredClone(current.background);
    for (const field of ['background_image', 'background_video', 'background_fill'] as const) {
      if (current[field] === undefined) delete item.theme[field];
      else Object.assign(item.theme, { [field]: structuredClone(current[field]) });
    }
    item.theme.layers = [
      ...item.theme.layers.filter((layer) => !isMark(layer)),
      ...structuredClone(marks),
    ];
    if (item.theme.layers.length > 32)
      throw new ValidationError(
        'Too many layers to switch presets. Remove extra watermarks and try again.',
      );
  }
  active.overlays.find((item) => item.overlay === active.selected_overlay)!.theme = current;
  const target = editor.banks.find((bank) => bank.background === background);
  const selected = target?.overlays.find((item) => item.overlay === overlay);
  if (!target || !selected)
    throw new ValidationError('This preset is not saved in the theme. Choose an available preset.');
  target.selected_overlay = overlay;
  editor.selected_background = background;
  const next = structuredClone(selected.theme);
  next.name = theme.name;
  next.basic_editor = editor;
  if (theme.page_cache) next.page_cache = structuredClone(theme.page_cache);
  return next;
}

/** Start independent native variants while keeping the active composition unchanged. */
export function createBasicEditor(theme: Theme, sensors: Sensor[] = []): Theme {
  if (lcdPageKind(theme) !== 'basic' || theme.basic_editor)
    throw new ValidationError('Could not create variants for this theme.');
  const current = structuredClone(theme);
  delete current.page_cache;
  delete current.basic_editor;
  const selectedBackground: BasicBackground =
    current.background_image || current.background_video ? 'image_video' : 'background_color';
  const marks = current.layers.filter(
    (layer) => layer.kind === 'image' && !!layer.watermark_position,
  );
  const available = sensors
    .filter(
      (sensor, index) =>
        sensor.id.trim() && sensors.findIndex((other) => other.id === sensor.id) === index,
    )
    .slice(0, 4);
  const counts: Partial<Record<BasicOverlay, number>> = {
    slider_sensor: 1,
    duo_sensor: 2,
    three_sensor: 3,
    four_sensor: 4,
  };
  const banks = (['image_video', 'background_color'] as const).map((background) => ({
    background,
    selected_overlay: 'custom' as const,
    overlays: basicOverlays.map((overlay) => {
      const variant = structuredClone(current);
      if (background === 'background_color') {
        delete variant.background_image;
        delete variant.background_video;
      }
      if (overlay === 'none') variant.layers = structuredClone(marks);
      else if (overlay === 'time')
        variant.layers = [...presetTheme('clock', []).layers, ...structuredClone(marks)];
      else if (overlay !== 'custom') {
        const count = counts[overlay]!;
        variant.layers = available.slice(0, count).flatMap((sensor, index) => {
          const x = count === 4 ? (index % 2 ? 330 : 150) : 240;
          const y =
            count === 1
              ? 205
              : count === 2
                ? 155 + index * 115
                : count === 3
                  ? 125 + index * 85
                  : 175 + Math.floor(index / 2) * 120;
          const style = {
            line_box: true,
            x,
            y,
            size: count === 1 ? 64 : count === 4 ? 32 : 44,
            align: 'center' as const,
            color: [219, 247, 242],
          };
          return [
            {
              kind: 'text' as const,
              text: Array.from(sensor.label)
                .slice(0, count === 4 ? 16 : 26)
                .join(''),
              style: { ...style, y: y - 36, size: 18, color: [144, 177, 201] },
            },
            {
              kind: 'sensor' as const,
              sensor_id: sensor.id,
              label: '',
              unit: sensor.unit,
              temperature_unit: 'system' as const,
              decimals: sensor.unit === 'rpm' ? 0 : sensor.unit === 'volts' ? 2 : 1,
              style,
            },
          ];
        });
        variant.layers.push(...structuredClone(marks));
      }
      if (variant.layers.length > 32)
        throw new ValidationError(
          'Too many layers to switch presets. Remove extra watermarks and try again.',
        );
      return { overlay, theme: variant };
    }),
  }));
  const result = structuredClone(theme);
  result.basic_editor = { selected_background: selectedBackground, banks };
  return result;
}
