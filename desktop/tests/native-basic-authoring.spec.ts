import { test, expect } from '@playwright/test';

test('native variants are independent and preserve edits when switching', async ({ page }) => {
  await page.goto('/');
  const result = await page.evaluate(async (modulePath) => {
    const { createBasicEditor, selectBasicEditor } = await import(/* @vite-ignore */ modulePath);
    const source = { version: 1, name: 'Draft', background: [1, 2, 3], layers: [] };
    const created = createBasicEditor(source);
    created.background = [9, 8, 7];
    const other = selectBasicEditor(created, 'image_video', 'time');
    other.background = [4, 5, 6];
    const restored = selectBasicEditor(other, 'background_color', 'custom');
    const again = selectBasicEditor(restored, 'image_video', 'time');
    return {
      source,
      restored: restored.background,
      again: again.background,
      banks: again.basic_editor?.banks.length,
    };
  }, '/src/lib/basic-editor.ts');
  expect(result.source.background).toEqual([1, 2, 3]);
  expect(result.restored).toEqual([9, 8, 7]);
  expect(result.again).toEqual([4, 5, 6]);
  expect(result.banks).toBe(2);
});
