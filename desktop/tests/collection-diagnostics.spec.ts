import { test, expect, type Page } from '@playwright/test';
interface Fixture {
  language: string;
  mode: string;
  calls: string[];
}
async function setup(page: Page, language: string, mode: string) {
  await page.addInitScript(
    ({ language, mode }) => {
      localStorage.setItem('llcontrol.interface-language', language);
      const f: Fixture = { language, mode, calls: [] };
      Object.assign(window, {
        membershipFixture: f,
        __TAURI_INTERNALS__: {
          invoke: async (command: string, args: Record<string, unknown> = {}) => {
            if (command !== 'daemon_call') return null;
            const method = String(args.method);
            f.calls.push(method);
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
                return [{ id: 'lcd', display_name: 'LCDV2', family: 'galahad2-lcdv2' }];
              case 'get_lcd_state':
                return { source: 'Idle', requested: {}, last_error: null };
              case 'list_profiles':
                return { version: 1, revision: 0, profiles: [] };
              case 'list_sensors':
              case 'list_named_theme_assets':
              case 'list_theme_assets':
              case 'list_theme_fonts':
              case 'list_prepared_clips':
                return [];
              case 'get_screen_collections': {
                if (f.mode === 'raw')
                  throw {
                    name: 'ValidationError',
                    message: 'Не удалось прочитать состав коллекций',
                  };
                if (f.mode === 'null') return null;
                if (f.mode === 'primitive') return 7;
                if (f.mode === 'bad-persistence')
                  return { collections: null, persistence: { pending: 'false' } };
                if (f.mode === 'bad-members')
                  return {
                    collections: {
                      version: 1,
                      revision: 0,
                      images: [],
                      watermarks: [],
                      backgrounds: [null],
                    },
                    persistence: { pending: false },
                  };
                return {
                  collections: null,
                  persistence: {
                    pending: f.mode.startsWith('pending'),
                    last_error: f.mode === 'pending-detail' ? 'Настройки {0} <disk>' : null,
                  },
                };
              }
              default:
                return null;
            }
          },
        },
      });
    },
    { language, mode },
  );
  await page.goto('/?view=display');
  await expect(
    page.getByText(language === 'en-US' ? 'Daemon connected' : 'Daemon подключён', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: /^(Library|Библиотека)$/ }).click();
}
async function switchLanguage(page: Page, language: string) {
  await page.evaluate((language) => {
    (window as unknown as { membershipFixture: Fixture }).membershipFixture.language = language;
  }, language);
  await expect(page.locator('html')).toHaveAttribute('lang', language === 'en-US' ? 'en' : 'ru');
}
for (const language of ['en-US', 'ru-RU'])
  for (const panel of ['images', 'videos'])
    for (const mode of [
      'null',
      'primitive',
      'bad-persistence',
      'bad-members',
      'pending',
      'pending-detail',
      'raw',
    ]) {
      test(`${panel} membership ${mode}: local resources versus literal backend diagnostic (${language})`, async ({
        page,
      }) => {
        await setup(page, language, mode);
        const images = panel === 'images';
        const container = page.getByRole('dialog', {
          name: images
            ? /^(Image library|Библиотека изображений)$/
            : /^(Video library|Библиотека роликов)$/,
          exact: true,
        });
        await page
          .getByRole('button', {
            name: images
              ? /^(Image library|Библиотека изображений)$/
              : /^(Video library|Библиотека роликов)$/,
            exact: true,
          })
          .click();
        const expected = (lang: string) => {
          if (mode === 'raw') return 'Не удалось прочитать состав коллекций';
          if (mode === 'pending-detail')
            return lang === 'en-US'
              ? 'Collections are awaiting persistence. Настройки {0} <disk>'
              : 'Коллекции ожидают сохранения. Настройки {0} <disk>';
          if (mode === 'pending')
            return lang === 'en-US'
              ? 'Collections are awaiting persistence. Read them again.'
              : 'Коллекции ожидают сохранения. Повторите чтение.';
          return lang === 'en-US'
            ? 'Could not read collection membership'
            : 'Не удалось прочитать состав коллекций';
        };
        await expect(container.getByRole('alert').first()).toContainText(expected(language));
        const other = language === 'en-US' ? 'ru-RU' : 'en-US';
        await switchLanguage(page, other);
        await expect(container.getByRole('alert').first()).toContainText(expected(other));
        await page.evaluate(() => {
          (window as unknown as { membershipFixture: Fixture }).membershipFixture.mode = 'valid';
        });
        await container
          .getByRole('button', {
            name: images
              ? /^(Refresh images|Обновить изображения)$/
              : /^(Refresh videos|Обновить ролики)$/,
          })
          .click();
        await expect(container.getByRole('alert')).toHaveCount(0);
        expect(
          await page.evaluate(() =>
            (window as unknown as { membershipFixture: Fixture }).membershipFixture.calls.filter(
              (method) => /^(set_|migrate_|import_)/.test(method),
            ),
          ),
        ).toEqual([]);
      });
    }

for (const language of ['en-US', 'ru-RU'])
  test(`stale membership and replacement readback retain owned resources (${language})`, async ({
    page,
  }) => {
    await setup(page, language, 'valid');
    const results = await page.evaluate(async () => {
      const path = '/src/lib/collection-membership.ts',
        resources = '/src/lib/i18n.svelte.ts';
      const membership = await import(/* @vite-ignore */ path),
        ui = await import(/* @vite-ignore */ resources);
      const current = {
        version: 1,
        revision: 1,
        images: ['old'],
        watermarks: [],
        backgrounds: [{ kind: 'video', key: 'old' }],
      };
      const operations = [
        () => membership.imageReplacementSnapshot('lcd', 'old', 'image'),
        () => membership.videoReplacementSnapshot('lcd', 'old'),
        () => membership.replaceCollectionImage('lcd', current, 'image', 'old', 'new'),
        () => membership.replaceCollectionVideo('lcd', current, 'old', 'new'),
      ];
      const results = [];
      for (const run of operations) {
        try {
          await run();
          throw new Error('Expected membership failure');
        } catch (e) {
          if (!(e instanceof Error)) throw e;
          results.push(ui.diagnosticText(e));
        }
      }
      return results;
    });
    expect(results).toEqual(
      language === 'en-US'
        ? [
            'The image is no longer in the selected collection. Refresh the library.',
            'The video is no longer in the collection. Refresh the library.',
            'Replacement requires checking collection membership.',
            'Replacement requires checking collection membership.',
          ]
        : [
            'Изображение больше не входит в выбранную коллекцию. Обновите библиотеку.',
            'Ролик больше не входит в коллекцию. Обновите библиотеку.',
            'Замена требует проверки состава коллекций.',
            'Замена требует проверки состава коллекций.',
          ],
    );
    const calls = await page.evaluate(
      () => (window as unknown as { membershipFixture: Fixture }).membershipFixture.calls,
    );
    expect(calls.filter((method) => method === 'set_screen_collections')).toHaveLength(2);
    expect(calls.filter((method) => method === 'migrate_screen_collections')).toHaveLength(2);
  });
