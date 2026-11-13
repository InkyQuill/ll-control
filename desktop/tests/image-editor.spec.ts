import { addControls } from './editor-controls';
import { test, expect, type Page } from '@playwright/test';
const png =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII=';
async function setup(
  page: Page,
  fail = false,
  delay = false,
  hold = false,
  language: 'en-US' | 'ru-RU' = 'ru-RU',
) {
  await page.addInitScript(
    ({ png, fail, delay, hold, language }) => {
      localStorage.setItem('llcontrol.interface-language', language);
      const calls: { command: string; args: Record<string, unknown> }[] = [];
      const control = {
        settings: {
          version: 1,
          revision: 0,
          temperature_unit: 'celsius',
          interface_language: language,
        },
        offline: false,
        release: undefined as (() => void) | undefined,
      };
      Object.assign(window, {
        editorControl: control,
        editorCalls: calls,
        __TAURI_INTERNALS__: {
          invoke: async (command: string, args: Record<string, unknown>) => {
            if (command === 'desktop_set_language') return null;
            calls.push({ command, args });
            if (command === 'begin_screenshot') return '11111111111111111111111111111111';
            if (command === 'cancel_screenshot') return 'closed';
            if (command === 'image_editor') {
              if (delay && args.edit) await new Promise((resolve) => setTimeout(resolve, 600));
              return {
                width: 960,
                height: 480,
                png: Array.from(atob(png), (c) => c.charCodeAt(0)),
              };
            }
            if (command === 'import_image') {
              if (hold) await new Promise<void>((resolve) => (control.release = resolve));
              if (fail) throw { message: 'Lost import reply' };
              return 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
            }
            switch (args.method) {
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              case 'get_display_settings':
                return structuredClone(control.settings);
              case 'list_devices':
                if (control.offline) throw { message: 'Disconnected' };
                return [{ id: 'lcd:test', display_name: 'Test LCDV2', family: 'galahad2-lcdv2' }];
              case 'list_sensors':
                return [];
              case 'get_lcd_state':
                return {
                  source: 'Idle',
                  last_error: null,
                  requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 },
                };
              case 'get_screen_collections':
                return {
                  collections: null,
                  persistence: { pending: false, saved: false, last_error: null },
                };
              default:
                return null;
            }
          },
        },
      });
    },
    { png, fail, delay, hold, language },
  );
  await page.goto('/?view=display');
  await expect(
    page.getByText(language === 'en-US' ? 'Daemon connected' : 'Daemon подключён', { exact: true }),
  ).toBeVisible();
  await page
    .getByRole('button', {
      name: language === 'en-US' ? 'Import and edit…' : 'Импорт с редактированием…',
    })
    .click();
  await page
    .getByLabel(language === 'en-US' ? 'File to edit' : 'Файл для редактирования')
    .setInputFiles({ name: 'wide.png', mimeType: 'image/png', buffer: Buffer.from(png, 'base64') });
  await expect(page.getByLabel('Crop X', { exact: true })).toHaveValue('240');
}
test('crop and transform preview stays local, changed crop invalidates result, save publishes once', async ({
  page,
}) => {
  await setup(page);
  const panel = page.getByRole('region', { name: 'Редактор изображения', exact: true });
  await page.getByLabel('Crop X', { exact: true }).fill('480');
  await panel.getByRole('button', { name: 'Повернуть вправо', exact: true }).click();
  await panel.getByRole('button', { name: 'Отразить по горизонтали', exact: true }).click();
  await panel.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await expect(panel.getByAltText('Результат редактирования 480 на 480')).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
          (c) => c.command === 'import_image',
        ).length,
    ),
  ).toBe(0);
  const prepared = await page.evaluate(
    () =>
      (
        window as unknown as { editorCalls: { command: string; args: { edit: unknown } }[] }
      ).editorCalls
        .filter((c) => c.command === 'image_editor')
        .at(-1)?.args.edit,
  );
  expect(prepared).toEqual({
    crop: { x: 480, y: 0, side: 480 },
    transforms: ['rotate_right', 'flip_horizontal'],
    filters: { hue: 0, saturation: 0, brightness: 0, contrast: 0, sharpen: 0, blur: 0 },
  });
  await page.getByLabel('Crop X', { exact: true }).fill('481');
  await expect(
    panel.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeDisabled();
  await expect(panel.getByRole('alert')).toContainText('внутри изображения');
  await page.getByLabel('Crop X', { exact: true }).fill('479');
  await expect(
    panel.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeDisabled();
  await panel.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await expect(
    panel.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeEnabled();
  await panel.getByRole('button', { name: 'Сохранить изображение в галерею' }).click();
  await expect(panel.getByRole('status')).toContainText('Результат сохранён');
  await expect(
    panel.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeDisabled();
  const calls = await page.evaluate(
    () =>
      (window as unknown as { editorCalls: { command: string; args: { method?: string } }[] })
        .editorCalls,
  );
  expect(calls.filter((c) => c.command === 'import_image')).toHaveLength(1);
  expect(calls.some((c) => c.args.method === 'set_lcd_theme')).toBe(false);
});
test('closing a pending preview discards its reply and restores focus without publication', async ({
  page,
}) => {
  await setup(page, false, true);
  await page.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await page.getByRole('button', { name: 'Закрыть редактор изображения' }).click();
  await expect(page.getByRole('button', { name: 'Импорт с редактированием…' })).toBeFocused();
  await page.waitForTimeout(700);
  await expect(page.getByAltText('Результат редактирования 480 на 480')).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
          (c) => c.command === 'import_image',
        ).length,
    ),
  ).toBe(0);
});
test('lost publication reply requires catalog inspection and does not offer blind retry', async ({
  page,
}) => {
  await setup(page, true);
  await page.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await page.getByRole('button', { name: 'Сохранить изображение в галерею' }).click();
  await expect(page.getByRole('alert')).toContainText('Проверьте библиотеку');
  await expect(
    page.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeDisabled();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Импорт с редактированием…' }).click();
  await expect(
    page.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeDisabled();
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
          (c) => c.command === 'import_image',
        ).length,
    ),
  ).toBe(1);
});

test('publication rechecks layer capacity after delayed import', async ({ page }) => {
  await setup(page, false, false, true);
  while ((await page.locator('.layer-row[data-layer-index]').count()) < 31) {
    await addControls(page);
    await page.getByRole('button', { name: '+ Текст', exact: true }).click();
  }
  await page.getByRole('button', { name: 'Фон и композиция', exact: true }).click();
  await page.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await page.getByRole('button', { name: 'Сохранить изображение в галерею' }).click();
  await addControls(page);
  await page.getByRole('button', { name: '+ Текст', exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          !!(window as unknown as { editorControl: { release?: () => void } }).editorControl
            .release,
      ),
    )
    .toBe(true);
  await page.evaluate(() =>
    (window as unknown as { editorControl: { release: () => void } }).editorControl.release(),
  );
  await page.getByRole('button', { name: 'Фон и композиция', exact: true }).click();
  await expect(
    page.getByRole('region', { name: 'Редактор изображения', exact: true }).getByRole('status'),
  ).toContainText('лимит 32 слоёв');
  await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(32);
});
test('publication after connection loss reports library-only outcome', async ({ page }) => {
  await setup(page, false, false, true);
  await page.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await page.getByRole('button', { name: 'Сохранить изображение в галерею' }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          !!(window as unknown as { editorControl: { release?: () => void } }).editorControl
            .release,
      ),
    )
    .toBe(true);
  await page.evaluate(
    () =>
      ((window as unknown as { editorControl: { offline: boolean } }).editorControl.offline = true),
  );
  await expect(page.getByRole('button', { name: 'Применить к LCD', exact: true })).toBeDisabled();
  await expect(page.getByText('Daemon подключён', { exact: true })).toHaveCount(0);
  await page.evaluate(() =>
    (window as unknown as { editorControl: { release: () => void } }).editorControl.release(),
  );
  await expect(
    page.getByRole('region', { name: 'Редактор изображения', exact: true }).getByRole('status'),
  ).toContainText('черновик не изменён');
});

test('color controls validate Windows ranges and invalidate previously prepared output', async ({
  page,
}) => {
  await setup(page);
  const panel = page.getByRole('region', { name: 'Редактор изображения', exact: true });
  await panel.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await expect(
    panel.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeEnabled();
  await panel.getByLabel('Насыщенность', { exact: true }).fill('-26');
  await expect(
    panel.getByRole('button', { name: 'Подготовить preview изображения' }),
  ).toBeDisabled();
  await expect(
    panel.getByRole('button', { name: 'Сохранить изображение в галерею' }),
  ).toBeDisabled();
  await panel.getByLabel('Насыщенность', { exact: true }).fill('-25');
  await panel.getByLabel('Сдвиг тона', { exact: true }).fill('120');
  await panel.getByLabel('Яркость изображения', { exact: true }).fill('-50');
  await panel.getByLabel('Контраст изображения', { exact: true }).fill('75');
  await panel.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await expect(panel.getByAltText('Результат редактирования 480 на 480')).toBeVisible();
  const edit = await page.evaluate(
    () =>
      (
        window as unknown as { editorCalls: { command: string; args: { edit: unknown } }[] }
      ).editorCalls
        .filter((c) => c.command === 'image_editor')
        .at(-1)?.args.edit,
  );
  expect(edit).toMatchObject({
    filters: { hue: 120, saturation: -25, brightness: -50, contrast: 75 },
  });
  await panel.getByLabel('Сдвиг тона', { exact: true }).fill('');
  await expect(
    panel.getByRole('button', { name: 'Подготовить preview изображения' }),
  ).toBeDisabled();
  await panel.getByRole('button', { name: 'Сбросить правки' }).click();
  await expect(panel.getByLabel('Сдвиг тона', { exact: true })).toHaveValue('0');
});

test('spatial filters validate bounds, invalidate preview and reset', async ({ page }) => {
  await setup(page);
  const panel = page.getByRole('region', { name: 'Редактор изображения', exact: true });
  const prepare = panel.getByRole('button', { name: 'Подготовить preview изображения' });
  const save = panel.getByRole('button', { name: 'Сохранить изображение в галерею' });
  await prepare.click();
  await expect(save).toBeEnabled();
  await panel.getByLabel('Резкость', { exact: true }).fill('5');
  await expect(save).toBeDisabled();
  await panel.getByLabel('Размытие', { exact: true }).fill('11');
  await expect(prepare).toBeDisabled();
  await panel.getByLabel('Размытие', { exact: true }).fill('2.5');
  await prepare.click();
  await expect(save).toBeEnabled();
  const edit = await page.evaluate(
    () =>
      (
        window as unknown as { editorCalls: { command: string; args: { edit: unknown } }[] }
      ).editorCalls
        .filter((c) => c.command === 'image_editor')
        .at(-1)?.args.edit,
  );
  expect(edit).toMatchObject({ filters: { sharpen: 5, blur: 2.5 } });
  await panel.getByLabel('Резкость', { exact: true }).fill('');
  await expect(prepare).toBeDisabled();
  await panel.getByRole('button', { name: 'Сбросить правки' }).click();
  await expect(panel.getByLabel('Резкость', { exact: true })).toHaveValue('0');
  await expect(panel.getByLabel('Размытие', { exact: true })).toHaveValue('0');
});

test('portal screenshot stays in editor until explicit preview and Save', async ({ page }) => {
  await setup(page);
  await page.evaluate((png) => {
    const w = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> };
    };
    const original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command, args) =>
      command === 'capture_screenshot'
        ? Uint8Array.from(atob(png), (c) => c.charCodeAt(0)).buffer
        : original(command, args);
  }, png);
  await page.getByRole('button', { name: 'Снять экран…', exact: true }).click();
  await expect(page.getByText('Снимок экрана.png · 960 × 480 px')).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
        (c) => c.command === 'import_image',
      ),
    ),
  ).toHaveLength(0);
  await page.getByRole('button', { name: 'Подготовить preview изображения' }).click();
  await page.getByRole('button', { name: 'Сохранить изображение в галерею' }).click();
  await expect(page.getByText('Результат сохранён', { exact: false })).toBeVisible();
  const imports = await page.evaluate(() =>
    (
      window as unknown as { editorCalls: { command: string; args: { name?: string } }[] }
    ).editorCalls.filter((c) => c.command === 'import_image'),
  );
  expect(imports).toHaveLength(1);
  expect(imports[0].args.name).toBe('Снимок экрана.png');
});

test('portal cancel preserves existing source and pending capture blocks repeated capture', async ({
  page,
}) => {
  await setup(page);
  await page.evaluate(() => {
    const w = window as unknown as {
      captureRelease?: () => void;
      __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> };
    };
    const original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command, args) =>
      command === 'capture_screenshot'
        ? new Promise<ArrayBuffer>(
            (resolve) => (w.captureRelease = () => resolve(new ArrayBuffer(0))),
          )
        : original(command, args);
  });
  await page.getByRole('button', { name: 'Снять экран…', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Снять экран…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Закрыть редактор изображения' })).toBeEnabled();
  await page.evaluate(() => (window as unknown as { captureRelease: () => void }).captureRelease());
  await expect(page.getByText('Снимок экрана отменён.')).toBeVisible();
  await expect(page.getByText('wide.png · 960 × 480 px')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Закрыть редактор изображения' })).toBeEnabled();
});

test('portal failure reports an error without erasing or publishing the current draft', async ({
  page,
}) => {
  await setup(page);
  await page.evaluate(() => {
    const w = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> };
    };
    const original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'capture_screenshot') throw { message: 'Screenshot portal unavailable' };
      return original(command, args);
    };
  });
  await page.getByRole('button', { name: 'Снять экран…', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Screenshot portal unavailable');
  await expect(page.getByText('wide.png · 960 × 480 px')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Снять экран…', exact: true })).toBeEnabled();
  expect(
    await page.evaluate(() =>
      (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
        (c) => c.command === 'import_image',
      ),
    ),
  ).toHaveLength(0);
});

test('closing before screenshot reservation reply cancels it without starting capture', async ({
  page,
}) => {
  await setup(page);
  await page.evaluate(() => {
    const w = window as unknown as {
      releaseReservation: () => void;
      __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> };
    };
    const original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'begin_screenshot')
        return new Promise<string>(
          (resolve) => (w.releaseReservation = () => resolve('delayed-reservation')),
        );
      return original(command, args);
    };
  });
  await page.getByRole('button', { name: 'Снять экран…', exact: true }).click();
  await page.getByRole('button', { name: 'Закрыть редактор изображения' }).click();
  await page.evaluate(() =>
    (window as unknown as { releaseReservation: () => void }).releaseReservation(),
  );
  await expect
    .poll(() =>
      page.evaluate(() =>
        (
          window as unknown as { editorCalls: { command: string; args: { id?: string } }[] }
        ).editorCalls.some(
          (c) => c.command === 'cancel_screenshot' && c.args.id === 'delayed-reservation',
        ),
      ),
    )
    .toBe(true);
  await expect(page.getByRole('region', { name: 'Редактор изображения', exact: true })).toHaveCount(
    0,
  );
  expect(
    await page.evaluate(() =>
      (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
        (c) => c.command === 'capture_screenshot' || c.command === 'import_image',
      ),
    ),
  ).toHaveLength(0);
});

test('closing a pending screenshot sends cancellation and ignores late pixels', async ({
  page,
}) => {
  await setup(page);
  await page.evaluate((png) => {
    const w = window as unknown as {
      releaseScreenshot: () => void;
      __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> };
    };
    const original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'capture_screenshot')
        return new Promise<ArrayBuffer>(
          (resolve) =>
            (w.releaseScreenshot = () =>
              resolve(Uint8Array.from(atob(png), (c) => c.charCodeAt(0)).buffer)),
        );
      return original(command, args);
    };
  }, png);
  await page.getByRole('button', { name: 'Снять экран…', exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () => typeof (window as unknown as { releaseScreenshot?: () => void }).releaseScreenshot,
      ),
    )
    .toBe('function');
  await page.getByRole('button', { name: 'Закрыть редактор изображения' }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as { editorCalls: { command: string }[] }).editorCalls.some(
          (c) => c.command === 'cancel_screenshot',
        ),
      ),
    )
    .toBe(true);
  await page.evaluate(() =>
    (window as unknown as { releaseScreenshot: () => void }).releaseScreenshot(),
  );
  await expect(page.getByRole('button', { name: 'Импорт с редактированием…' })).toBeFocused();
  expect(
    await page.evaluate(() =>
      (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
        (c) => c.command === 'image_editor',
      ),
    ),
  ).toHaveLength(1);
  expect(
    await page.evaluate(() =>
      (window as unknown as { editorCalls: { command: string }[] }).editorCalls.filter(
        (c) => c.command === 'import_image',
      ),
    ),
  ).toHaveLength(0);
});

interface CancelRecovery {
  receipt: unknown;
  count: number;
  cancelFails: boolean;
  calls: string[];
  pending: Record<string, (bytes: ArrayBuffer) => void>;
  orphan: boolean;
}
declare global {
  interface Window {
    cancelRecovery: CancelRecovery;
  }
}
async function cancellationFixture(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> };
    };
    const original = w.__TAURI_INTERNALS__.invoke;
    const f: CancelRecovery = {
      receipt: 'closed',
      count: 0,
      cancelFails: true,
      calls: [],
      pending: {},
      orphan: false,
    };
    window.cancelRecovery = f;
    w.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'begin_screenshot') {
        if (f.orphan) throw { message: 'previous cleanup' };
        return `capture-${++f.count}`;
      }
      if (command === 'pending_screenshot_cleanup') return f.orphan ? 'orphan-id' : null;
      if (command === 'capture_screenshot')
        return new Promise<ArrayBuffer>(
          (resolve) => (f.pending[(args as { id: string }).id] = resolve),
        );
      if (command === 'cancel_screenshot') {
        f.calls.push((args as { id: string }).id);
        if (f.cancelFails) throw { message: 'Настройки' };
        f.orphan = false;
        return f.receipt;
      }
      return original(command, args);
    };
  });
}
for (const language of ['en-US', 'ru-RU'] as const) {
  for (const action of ['cancel', 'close'] as const)
    test(`screenshot ${action} failure retains the owned ID until explicit retry (${language})`, async ({
      page,
    }) => {
      const english = language === 'en-US';
      await setup(page, false, false, false, language);
      await cancellationFixture(page);
      await page
        .getByRole('button', { name: english ? 'Take screenshot…' : 'Снять экран…', exact: true })
        .click();
      await expect
        .poll(() => page.evaluate(() => Object.keys(window.cancelRecovery.pending)))
        .toEqual(['capture-1']);
      await page
        .getByRole('button', {
          name:
            action === 'close'
              ? english
                ? 'Close image editor'
                : 'Закрыть редактор изображения'
              : english
                ? 'Cancel screenshot'
                : 'Отменить снимок',
          exact: true,
        })
        .click();
      const panel = () =>
        page.getByRole('region', { name: /^(Image editor|Редактор изображения)$/ });
      await expect(panel().getByRole('alert')).toContainText('Настройки');
      await expect(
        page.getByRole('button', {
          name: english ? 'Take screenshot…' : 'Снять экран…',
          exact: true,
        }),
      ).toBeDisabled();
      await expect(panel().getByText('wide.png · 960 × 480 px', { exact: true })).toBeVisible();
      await expect(
        panel()
          .getByRole('status')
          .filter({ hasText: english ? 'Screenshot cancelled.' : 'Снимок экрана отменён.' }),
      ).toHaveCount(0);
      await page.evaluate(
        (other) => {
          const f = (
            window as unknown as {
              editorControl: { settings: { interface_language: string; revision: number } };
            }
          ).editorControl;
          f.settings.interface_language = other;
          f.settings.revision++;
        },
        english ? 'ru-RU' : 'en-US',
      );
      await expect(page.locator('html')).toHaveAttribute('lang', english ? 'ru' : 'en');
      await expect(panel().getByRole('alert')).toContainText('Настройки');
      await page.evaluate(() => (window.cancelRecovery.cancelFails = false));
      await page
        .getByRole('button', {
          name:
            action === 'close'
              ? english
                ? 'Закрыть редактор изображения'
                : 'Close image editor'
              : english
                ? 'Отменить снимок'
                : 'Cancel screenshot',
          exact: true,
        })
        .click();
      if (action === 'close') await expect(panel()).toHaveCount(0);
      else
        await expect(
          page.getByRole('button', {
            name: english ? 'Снять экран…' : 'Take screenshot…',
            exact: true,
          }),
        ).toBeEnabled();
      expect(await page.evaluate(() => window.cancelRecovery.calls)).toEqual([
        'capture-1',
        'capture-1',
      ]);
      expect(await page.evaluate(() => window.cancelRecovery.count)).toBe(1);
    });
  test(`late cancelled screenshot reply cannot cancel or replace a new capture (${language})`, async ({
    page,
  }) => {
    const english = language === 'en-US';
    await setup(page, false, false, false, language);
    await cancellationFixture(page);
    await page.evaluate(() => (window.cancelRecovery.cancelFails = false));
    const capture = page.getByRole('button', {
        name: english ? 'Take screenshot…' : 'Снять экран…',
        exact: true,
      }),
      cancel = page.getByRole('button', {
        name: english ? 'Cancel screenshot' : 'Отменить снимок',
        exact: true,
      });
    await capture.click();
    await expect
      .poll(() => page.evaluate(() => !!window.cancelRecovery.pending['capture-1']))
      .toBe(true);
    await cancel.click();
    await expect(capture).toBeEnabled();
    await capture.click();
    await expect
      .poll(() => page.evaluate(() => !!window.cancelRecovery.pending['capture-2']))
      .toBe(true);
    await page.evaluate(
      (png) =>
        window.cancelRecovery.pending['capture-1'](
          Uint8Array.from(atob(png), (c) => c.charCodeAt(0)).buffer,
        ),
      png,
    );
    await expect(capture).toBeDisabled();
    await expect(page.getByText('wide.png · 960 × 480 px', { exact: true })).toBeVisible();
    await cancel.click();
    await expect(capture).toBeEnabled();
    expect(await page.evaluate(() => window.cancelRecovery.calls)).toEqual([
      'capture-1',
      'capture-2',
    ]);
  });
  test(`orphaned backend cleanup can be adopted for explicit cancellation (${language})`, async ({
    page,
  }) => {
    const english = language === 'en-US';
    await setup(page, false, false, false, language);
    await cancellationFixture(page);
    await page.evaluate(() => {
      window.cancelRecovery.orphan = true;
      window.cancelRecovery.cancelFails = false;
    });
    const capture = page.getByRole('button', {
      name: english ? 'Take screenshot…' : 'Снять экран…',
      exact: true,
    });
    await capture.click();
    await expect(capture).toBeDisabled();
    await expect(page.getByRole('alert')).toContainText('previous cleanup');
    await page
      .getByRole('button', { name: english ? 'Cancel screenshot' : 'Отменить снимок', exact: true })
      .click();
    await expect(capture).toBeEnabled();
    expect(await page.evaluate(() => window.cancelRecovery.calls)).toEqual(['orphan-id']);
    expect(await page.evaluate(() => window.cancelRecovery.count)).toBe(0);
  });
}

for (const language of ['en-US', 'ru-RU'] as const) {
  test(`owner loss is distinct from confirmed cancellation and never restarts capture (${language})`, async ({
    page,
  }) => {
    const en = language === 'en-US';
    await setup(page, false, false, false, language);
    await cancellationFixture(page);
    await page.evaluate(() => {
      window.cancelRecovery.cancelFails = false;
      window.cancelRecovery.receipt = 'owner_gone';
    });
    const capture = page.getByRole('button', {
      name: en ? 'Take screenshot…' : 'Снять экран…',
      exact: true,
    });
    await capture.click();
    await expect
      .poll(() => page.evaluate(() => !!window.cancelRecovery.pending['capture-1']))
      .toBe(true);
    await page
      .getByRole('button', { name: en ? 'Cancel screenshot' : 'Отменить снимок', exact: true })
      .click();
    await expect(capture).toBeEnabled();
    await expect(
      page.getByRole('status').filter({
        hasText: en
          ? 'The portal disconnected. The previous screenshot request has ended.'
          : 'Портал отключился. Предыдущий запрос снимка завершён.',
      }),
    ).toBeVisible();
    await expect(
      page.getByText(en ? 'Screenshot cancelled.' : 'Снимок экрана отменён.', { exact: true }),
    ).toHaveCount(0);
    expect(await page.evaluate(() => window.cancelRecovery.count)).toBe(1);
    await expect(page.getByText('wide.png · 960 × 480 px', { exact: true })).toBeVisible();
  });
  test(`unknown cancellation receipt cannot release the capture ID (${language})`, async ({
    page,
  }) => {
    const en = language === 'en-US';
    await setup(page, false, false, false, language);
    await cancellationFixture(page);
    await page.evaluate(() => {
      window.cancelRecovery.cancelFails = false;
      window.cancelRecovery.receipt = 'future_unknown';
    });
    const capture = page.getByRole('button', {
        name: en ? 'Take screenshot…' : 'Снять экран…',
        exact: true,
      }),
      cancel = page.getByRole('button', {
        name: en ? 'Cancel screenshot' : 'Отменить снимок',
        exact: true,
      });
    await capture.click();
    await expect
      .poll(() => page.evaluate(() => !!window.cancelRecovery.pending['capture-1']))
      .toBe(true);
    await cancel.click();
    await expect(page.getByRole('alert')).toContainText(
      'Unsupported screenshot cancellation receipt',
    );
    await expect(capture).toBeDisabled();
    await page.evaluate(() => (window.cancelRecovery.receipt = 'closed'));
    await cancel.click();
    await expect(capture).toBeEnabled();
    expect(await page.evaluate(() => window.cancelRecovery.calls)).toEqual([
      'capture-1',
      'capture-1',
    ]);
  });
}

for (const language of ['en-US', 'ru-RU'] as const)
  for (const release of ['buttons', 'capture'] as const) {
    test(`crop drag ends after lost ${release} release and accepts a fresh drag (${language})`, async ({
      page,
    }) => {
      await setup(page, false, false, false, language);
      await page
        .getByLabel(language === 'en-US' ? 'Crop side length' : 'Сторона crop', { exact: true })
        .fill('200');
      const area = page.locator('button.source');
      await area.scrollIntoViewIfNeeded();
      const box = (await area.boundingBox())!;
      const x = page.getByLabel('Crop X', { exact: true }),
        y = page.getByLabel('Crop Y', { exact: true });
      await area.evaluate((element) =>
        element.addEventListener(
          'pointerdown',
          (event) =>
            element.setAttribute('data-last-pointer', String((event as PointerEvent).pointerId)),
          { once: true },
        ),
      );
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
      await page.mouse.down();
      await expect(x).not.toHaveValue('240');
      const before = { x: await x.inputValue(), y: await y.inputValue() };
      const pointerId = Number(await area.getAttribute('data-last-pointer'));
      try {
        if (release === 'buttons')
          await area.dispatchEvent('pointermove', {
            pointerId,
            buttons: 0,
            button: -1,
            clientX: box.x + 2,
            clientY: box.y + 2,
          });
        else {
          await area.evaluate((element, id) => element.releasePointerCapture(id), pointerId);
          await page.mouse.move(box.x + 2, box.y + 2);
        }
        await expect(x).toHaveValue(before.x);
        await expect(y).toHaveValue(before.y);
      } finally {
        await page.mouse.up();
      }
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
      await page.mouse.down();
      await page.mouse.move(box.x + box.width - 2, box.y + box.height - 2);
      await page.mouse.up();
      await expect(x).not.toHaveValue(before.x);
      const calls = await page.evaluate(
        () =>
          (window as unknown as { editorCalls: { command: string; args: { method?: string } }[] })
            .editorCalls,
      );
      expect(
        calls.filter(
          (call) =>
            call.command === 'import_image' ||
            ['set_lcd_theme', 'set_lcd_media'].includes(call.args.method ?? ''),
        ),
      ).toEqual([]);
    });
  }
