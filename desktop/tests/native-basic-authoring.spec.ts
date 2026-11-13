import { test, expect } from '@playwright/test';
import type { Sensor, Theme } from '../src/lib/api';

test('native banks seed distinct compositions and preserve media, marks and edits', async ({
  page,
}) => {
  await page.goto('/');
  const result = await page.evaluate(async (modulePath) => {
    const { createBasicEditor, selectBasicEditor } = await import(/* @vite-ignore */ modulePath);
    const mark = {
      kind: 'image',
      asset: 'builtin-orbit-mark-v1',
      x: 0,
      y: 0,
      width: 40,
      height: 40,
      watermark_position: 'top_left',
    };
    const source = {
      version: 2,
      name: 'Draft',
      background: [1, 2, 3],
      background_video: 'a'.repeat(32),
      layers: [
        {
          kind: 'text',
          text: 'Original',
          style: { x: 30, y: 40, size: 20, align: 'left', color: [255, 255, 255] },
        },
        mark,
      ],
    } as Theme;
    const sensors = [
      { id: 'cpu', label: 'CPU', unit: 'celsius', value: 42, status: 'valid', age_ms: 1 },
      { id: 'gpu', label: 'GPU', unit: 'celsius', value: 40, status: 'valid', age_ms: 1 },
    ] as Sensor[];
    const initial = JSON.stringify(source);
    const created = createBasicEditor(source, sensors);
    const none = selectBasicEditor(created, 'image_video', 'none');
    const four = selectBasicEditor(none, 'image_video', 'four_sensor');
    const time = selectBasicEditor(four, 'background_color', 'time');
    time.background = [9, 8, 7];
    const original = selectBasicEditor(time, 'image_video', 'custom');
    const again = selectBasicEditor(original, 'background_color', 'time');
    const noSensors = createBasicEditor(source, []);
    return {
      unchanged: initial === JSON.stringify(source),
      none,
      four,
      time,
      original,
      again,
      noSensors,
    };
  }, '/src/lib/basic-editor.ts');
  expect(result.unchanged).toBe(true);
  expect(result.none.layers.map((layer: { kind: string }) => layer.kind)).toEqual(['image']);
  expect(result.none.background_video).toBe('a'.repeat(32));
  expect(
    result.four.layers.filter((layer: { kind: string }) => layer.kind === 'sensor'),
  ).toHaveLength(2);
  expect(
    result.four.layers.filter((layer: { kind: string }) => layer.kind === 'image'),
  ).toHaveLength(1);
  expect(
    result.time.layers.filter((layer: { kind: string }) => layer.kind === 'clock'),
  ).toHaveLength(3);
  expect(result.time.background_video).toBeUndefined();
  expect(result.original.layers[0].text).toBe('Original');
  expect(result.original.background_video).toBe('a'.repeat(32));
  expect(result.again.background).toEqual([9, 8, 7]);
  expect(
    result.noSensors.basic_editor.banks[0].overlays
      .find((item: { overlay: string }) => item.overlay === 'four_sensor')
      .theme.layers.map((layer: { kind: string }) => layer.kind),
  ).toEqual(['image']);
});
