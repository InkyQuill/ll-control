import { test, expect, type Page } from '@playwright/test';
interface Fixture {
  language: string;
  fail: string;
  commit: boolean;
  code: number;
  lcdError: string | null;
  writes: { method: string; params: Record<string, unknown> }[];
  revision: number;
  receipt: Record<string, unknown> | null;
  collections: null | {
    version: number;
    revision: number;
    images: string[];
    backgrounds: unknown[];
    watermarks: string[];
  };
}
async function setup(page: Page, language: string) {
  await page.addInitScript((language) => {
    localStorage.setItem('llcontrol.interface-language', language);
    const f: Fixture = {
      language,
      fail: '',
      commit: false,
      code: -32000,
      lcdError: null,
      writes: [],
      revision: 0,
      receipt: null,
      collections: null,
    };
    const lcd = () => ({
      generation: f.revision,
      source: 'Idle',
      last_error: f.lcdError,
      requested: { brightness_percent: 50, rotation_degrees: 0, mode: 'LocalUI' },
    });
    const persistence = { pending: false, saved: true, last_error: null };
    Object.assign(window, {
      lcdLifecycle: f,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          if (command !== 'daemon_call') return null;
          const method = String(args.method),
            params = (args.params ?? {}) as Record<string, unknown>;
          const mutating = [
            'patch_lcd_config',
            'reset_screen',
            'reapply_screen_reset',
            'migrate_screen_collections',
            'set_screen_collections',
          ].includes(method);
          if (mutating) f.writes.push({ method, params: structuredClone(params) });
          if (method === f.fail && (!mutating || !f.commit))
            throw { message: 'Настройки', code: f.code };
          let result: unknown;
          switch (method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'get_display_settings':
              return {
                version: 1,
                revision: 0,
                temperature_unit: 'celsius',
                interface_language: f.language,
              };
            case 'list_devices':
              return [{ id: 'lcd', display_name: 'Fixture LCDV2', family: 'galahad2-lcdv2' }];
            case 'get_lcd_state':
              return lcd();
            case 'list_profiles':
              return { version: 1, revision: f.revision, profiles: [] };
            case 'get_screen_collections':
              return { collections: structuredClone(f.collections), persistence };
            case 'get_screen_reset':
              return {
                receipt: structuredClone(f.receipt),
                lcd: lcd(),
                persistence,
                reapply_available: !!f.lcdError,
              };
            case 'reset_screen':
              if (f.receipt?.operation_id !== params.operation_id) {
                f.revision++;
                f.receipt = {
                  operation_id: params.operation_id,
                  profile_revision: f.revision,
                  collection_revision: f.revision,
                };
              }
              result = { receipt: structuredClone(f.receipt), lcd: lcd() };
              break;
            case 'migrate_screen_collections':
              f.collections = {
                version: 1,
                revision: 1,
                images: ['user'],
                backgrounds: [],
                watermarks: [],
              };
              result = structuredClone(f.collections);
              break;
            case 'set_screen_collections':
              f.collections = params.collections as Fixture['collections'];
              result = structuredClone(f.collections);
              break;
            case 'patch_lcd_config':
            case 'reapply_screen_reset':
              result = lcd();
              break;
            case 'list_named_theme_assets':
              return [{ key: 'user', name: 'Настройки' }];
            case 'list_sensors':
            case 'list_prepared_clips':
            case 'list_theme_assets':
              return [];
            default:
              return null;
          }
          if (method === f.fail) throw { message: 'Настройки', code: f.code };
          return result;
        },
      },
    });
  }, language);
  await page.goto('/?view=display');
  await expect(page.locator('html')).toHaveAttribute('lang', language === 'en-US' ? 'en' : 'ru');
}
async function language(page: Page, language: string) {
  await page.evaluate((language) => {
    (window as unknown as { lcdLifecycle: Fixture }).lcdLifecycle.language = language;
  }, language);
  await expect(page.locator('html')).toHaveAttribute('lang', language === 'en-US' ? 'en' : 'ru');
}
async function fault(page: Page, method: string, commit = false, code = -32000) {
  await page.evaluate(
    ({ method, commit, code }) => {
      Object.assign((window as unknown as { lcdLifecycle: Fixture }).lcdLifecycle, {
        fail: method,
        commit,
        code,
      });
    },
    { method, commit, code },
  );
}
const writes = (page: Page) =>
  page.evaluate(() => (window as unknown as { lcdLifecycle: Fixture }).lcdLifecycle.writes);
const reset = (page: Page) =>
  page.getByRole('dialog', { name: /^(LCD screen reset|Сброс экрана LCD)$/, exact: true });
const collections = (page: Page) =>
  page.getByRole('dialog', { name: /^(LCD collections|Коллекции LCD)$/, exact: true });
async function openReset(page: Page) {
  await page
    .getByRole('button', { name: /^(LCD screen reset|Сброс экрана LCD)$/, exact: true })
    .click();
}
async function resetClick(page: Page) {
  await page.getByRole('button', { name: /^(Reset screen…|Сбросить экран…)$/ }).click();
  await page
    .getByRole('button', { name: /^(Confirm screen reset|Подтвердить сброс экрана)$/ })
    .click();
}
async function openCollections(page: Page) {
  await page.getByRole('button', { name: /^(Library|Библиотека)$/, exact: true }).click();
  await page
    .getByRole('button', { name: /^(LCD collections|Коллекции LCD)$/, exact: true })
    .click();
}
for (const initial of ['en-US', 'ru-RU']) {
  const en = initial === 'en-US',
    other = en ? 'ru-RU' : 'en-US';
  test(`LCD patch diagnostics and retained draft survive language switch (${initial})`, async ({
    page,
  }) => {
    await setup(page, initial);
    await page.getByRole('button', { name: en ? 'Settings' : 'Настройки', exact: true }).click();
    const panel = page.getByRole('region', { name: /^(LCD settings|Настройки LCD)$/ });
    await panel.getByRole('combobox', { name: /^(Orientation|Ориентация)$/ }).selectOption('180');
    await fault(page, 'patch_lcd_config');
    await panel.getByRole('button', { name: /^(Apply rotation|Применить поворот)$/ }).click();
    const alert = panel.getByRole('alert');
    await expect(alert).toContainText(
      en ? 'Could not send the LCD setting:' : 'Не удалось отправить настройку LCD:',
    );
    await expect(alert.locator('span')).toHaveText('Настройки');
    await language(page, other);
    await expect(alert).toContainText(
      en ? 'Не удалось отправить настройку LCD:' : 'Could not send the LCD setting:',
    );
    await expect(alert.locator('span')).toHaveText('Настройки');
    await expect(panel.getByRole('combobox')).toHaveValue('180');
    expect(await writes(page)).toHaveLength(1);
  });
  for (const kind of ['read', 'rejected', 'unconfirmed', 'lcd'] as const)
    test(`reset ${kind} ownership and operation identity (${initial})`, async ({ page }) => {
      await setup(page, initial);
      if (kind === 'read') await fault(page, 'get_screen_reset');
      await openReset(page);
      const p = reset(page);
      if (kind !== 'read') {
        if (kind === 'lcd')
          await page.evaluate(() => {
            (window as unknown as { lcdLifecycle: Fixture }).lcdLifecycle.lcdError = 'Настройки';
          });
        else
          await fault(
            page,
            'reset_screen',
            kind === 'unconfirmed',
            kind === 'rejected' ? -32002 : -32000,
          );
        await resetClick(page);
      }
      const prefix = (english: boolean) =>
        kind === 'read'
          ? english
            ? 'Could not check the reset:'
            : 'Не удалось проверить сброс:'
          : kind === 'rejected'
            ? english
              ? 'Reset rejected:'
              : 'Сброс отклонён:'
            : kind === 'unconfirmed'
              ? english
                ? 'Response unconfirmed:'
                : 'Ответ не подтверждён:'
              : english
                ? 'LCD error:'
                : 'Ошибка LCD:';
      const output = p.locator('[role]').filter({ hasText: prefix(en) });
      await expect(output).toContainText('Настройки');
      await expect(output.locator('span')).toHaveText('Настройки');
      await language(page, other);
      const switched = p.locator('[role]').filter({ hasText: prefix(!en) });
      await expect(switched.locator('span')).toHaveText('Настройки');
      expect(await writes(page)).toHaveLength(kind === 'read' ? 0 : 1);
      if (kind === 'rejected') {
        await fault(page, '');
        await resetClick(page);
        const calls = await writes(page);
        expect(calls).toHaveLength(2);
        expect(calls[0].params.operation_id).not.toBe(calls[1].params.operation_id);
      }
      if (kind === 'unconfirmed')
        await expect(
          p.getByRole('button', { name: /^(Retry reset request|Повторить запрос сброса)$/ }),
        ).toHaveCount(0);
    });
  for (const operation of ['read', 'migrate', 'save'] as const)
    test(`collections ${operation} message retains raw diagnostic (${initial})`, async ({
      page,
    }) => {
      await setup(page, initial);
      if (operation === 'read') await fault(page, 'get_screen_collections');
      await openCollections(page);
      const p = collections(page);
      if (operation !== 'read') {
        if (operation === 'save') {
          await p
            .getByRole('button', {
              name: /^(Migrate catalog to collections|Перенести каталог в коллекции)$/,
            })
            .click();
          await p
            .getByRole('button', {
              name: en ? 'Remove from collection: Настройки' : 'Убрать из коллекции: Настройки',
              exact: true,
            })
            .click();
        }
        await fault(
          page,
          operation === 'migrate' ? 'migrate_screen_collections' : 'set_screen_collections',
        );
        await p
          .getByRole('button', {
            name:
              operation === 'migrate'
                ? /^(Migrate catalog to collections|Перенести каталог в коллекции)$/
                : /^(Save collections|Сохранить коллекции)$/,
          })
          .click();
      }
      const prefix = (english: boolean) =>
        operation === 'read'
          ? english
            ? 'Could not check the collections:'
            : 'Не удалось проверить коллекции:'
          : english
            ? 'The write result needs checking:'
            : 'Результат записи требует проверки:';
      let output = p.locator('[role]').filter({ hasText: prefix(en) });
      await expect(output.locator('span')).toHaveText('Настройки');
      await language(page, other);
      output = p.locator('[role]').filter({ hasText: prefix(!en) });
      await expect(output.locator('span')).toHaveText('Настройки');
      expect(await writes(page)).toHaveLength(
        operation === 'read' ? 0 : operation === 'save' ? 2 : 1,
      );
    });
}

for (const initial of ['en-US', 'ru-RU'])
  for (const operation of ['migrate', 'save'])
    test(`collection ${operation} success follows language without repeating the write (${initial})`, async ({
      page,
    }) => {
      await setup(page, initial);
      await openCollections(page);
      const p = collections(page);
      await p
        .getByRole('button', {
          name: /^(Migrate catalog to collections|Перенести каталог в коллекции)$/,
        })
        .click();
      if (operation === 'save') {
        await p
          .getByRole('button', {
            name:
              initial === 'en-US'
                ? 'Remove from collection: Настройки'
                : 'Убрать из коллекции: Настройки',
            exact: true,
          })
          .click();
        await p.getByRole('button', { name: /^(Save collections|Сохранить коллекции)$/ }).click();
      }
      const text = (en: boolean) =>
        operation === 'migrate'
          ? en
            ? 'Catalog migrated to collections.'
            : 'Каталог перенесён в коллекции.'
          : en
            ? 'Collection membership saved.'
            : 'Состав коллекций сохранён.';
      await expect(p.getByText(text(initial === 'en-US'), { exact: true })).toBeVisible();
      await language(page, initial === 'en-US' ? 'ru-RU' : 'en-US');
      await expect(p.getByText(text(initial !== 'en-US'), { exact: true })).toBeVisible();
      expect(await writes(page)).toHaveLength(operation === 'migrate' ? 1 : 2);
    });
