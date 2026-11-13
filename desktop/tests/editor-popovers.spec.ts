import { test, expect } from '@playwright/test';
for (const width of [1024, 1280])
  test(`editor help and settings do not move the canvas at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 768 });
    await page.addInitScript(() =>
      Object.assign(window, {
        __TAURI_INTERNALS__: {
          invoke: async (command: string, request: { method?: string } = {}) => {
            if (command === 'system_fonts') return [];
            if (command === 'desktop_set_language') return null;
            switch (request.method) {
              case 'list_devices':
                return [{ id: 'test', display_name: 'LCD', family: 'galahad2-lcdv2' }];
              case 'list_sensors':
                return [];
              case 'get_lcd_state':
                return { source: 'Idle', last_error: null };
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              default:
                throw new Error('Unexpected ' + request.method);
            }
          },
        },
      }),
    );
    await page.goto('/?view=display');
    await expect(page.getByText('Daemon подключён')).toBeVisible();
    const canvas = page.getByRole('group', { name: 'Холст LCD', exact: true });
    const before = await canvas.boundingBox();
    const help = page.getByRole('button', { name: 'Управление холстом', exact: true });
    await help.focus();
    await page.keyboard.press('Enter');
    const panel = page.getByRole('dialog', { name: 'Управление холстом', exact: true });
    await expect(panel).toBeVisible();
    await expect(panel).toContainText('Стрелки');
    expect(await canvas.boundingBox()).toEqual(before);
    await expect
      .poll(async () => {
        const box = (await panel.boundingBox())!;
        return box.y + box.height;
      })
      .toBeLessThanOrEqual(768);
    const box = (await panel.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(width);
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.y + box.height).toBeLessThanOrEqual(768);
    await page.keyboard.press('Escape');
    await expect(panel).not.toBeVisible();
    await expect(help).toBeFocused();
    await page.getByRole('button', { name: 'Добавить слой', exact: true }).click();
    await page.getByRole('button', { name: '+ Текст', exact: true }).click();
    await expect(
      page.getByRole('dialog', { name: 'Добавить слой', exact: true }),
    ).not.toBeVisible();
    await page.getByRole('button', { name: 'Переименовать слой', exact: true }).click();
    await page.getByRole('textbox', { name: 'Имя слоя', exact: true }).fill('A retained name');
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'Слои', exact: true }).click();
    await expect(page.getByRole('button', { name: /A retained name/ })).toBeVisible();
    await expect(page.locator('.workspace details')).toHaveCount(0);
  });
