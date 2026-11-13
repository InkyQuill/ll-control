import { test, expect, type Page } from '@playwright/test';
interface Fixture {
  language: string;
  fontMode: string;
  release: (() => void) | null;
  calls: { method: string; params: Record<string, unknown> }[];
  cleanupFail: string;
  thumbnailFail: boolean;
  corrupt: boolean;
}
async function setup(page: Page, language: string) {
  await page.addInitScript((language) => {
    localStorage.setItem('lconnect.interface-language', language);
    const f: Fixture = {
      language,
      fontMode: 'success',
      release: null,
      calls: [],
      cleanupFail: '',
      thumbnailFail: false,
      corrupt: false,
    };
    let size = 0,
      received = 0,
      asset: string | null = null;
    const state = () => ({ id: 'c'.repeat(32), size, received, asset, cancelled: false });
    const png = Array.from(
      atob(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg==',
      ),
      (c) => c.charCodeAt(0),
    );
    Object.assign(window, {
      libraryResources: f,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          if (
            command === 'desktop_set_language' ||
            command === 'media_status' ||
            command === 'media_thumbnail_cancel'
          )
            return null;
          if (command === 'media_thumbnail') {
            f.calls.push({ method: command, params: {} });
            if (f.thumbnailFail) throw { message: 'Настройки' };
            return f.corrupt ? [1, 2, 3] : png;
          }
          if (command !== 'daemon_call') return null;
          const method = String(args.method),
            params = (args.params ?? {}) as Record<string, unknown>;
          f.calls.push({ method, params: structuredClone(params) });
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
            case 'list_sensors':
            case 'list_named_theme_assets':
            case 'list_theme_assets':
            case 'list_theme_fonts':
              return [];
            case 'get_lcd_state':
              return { source: 'Idle', last_error: null, requested: {} };
            case 'list_profiles':
              return { version: 1, revision: 0, profiles: [] };
            case 'list_prepared_clips':
              return [
                {
                  key: 'a'.repeat(32),
                  name: 'Настройки {0} <clip>',
                  fps: 24,
                  frames: 24,
                  encoded_bytes: 100,
                },
              ];
            case 'get_lcd_media':
              return null;
            case 'get_screen_collections':
              return {
                collections: null,
                persistence: { pending: false, saved: false, last_error: null },
              };
            case 'cleanup_theme_fonts':
            case 'cleanup_theme_assets':
              if (f.cleanupFail === (params.delete ? 'delete' : 'scan'))
                throw { message: 'Настройки' };
              return {
                candidates: ['d'.repeat(32)],
                deleted: params.delete ? ['d'.repeat(32)] : [],
                retained_referenced: 1,
                retained_session: 0,
                retained_draft: 0,
                skipped: 0,
                failures: [],
              };
            case 'font_upload': {
              const r = params.request as { action: string; size: number; bytes: number[] };
              if (r.action === 'begin') {
                size = r.size;
                received = 0;
                asset = null;
              }
              if (r.action === 'chunk') {
                if (f.fontMode === 'hold')
                  await new Promise<void>((resolve) => {
                    f.release = resolve;
                  });
                received += r.bytes.length;
                if (f.fontMode === 'offset') received++;
              }
              if (r.action === 'finish') {
                if (f.fontMode === 'raw') throw { message: 'Шрифт не сохранён' };
                if (f.fontMode !== 'not-saved') asset = 'b'.repeat(32);
              }
              return state();
            }
            default:
              return null;
          }
        },
      },
    });
  }, language);
  await page.goto('/?view=display');
  await page.getByRole('button', { name: /^(Library|Библиотека)$/, exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('lang', language === 'en-US' ? 'en' : 'ru');
}
async function language(page: Page, value: string) {
  await page.evaluate((value) => {
    (window as unknown as { libraryResources: Fixture }).libraryResources.language = value;
  }, value);
  await expect(page.locator('html')).toHaveAttribute('lang', value === 'en-US' ? 'en' : 'ru');
}
const calls = (page: Page, method: string) =>
  page.evaluate(
    (method) =>
      (window as unknown as { libraryResources: Fixture }).libraryResources.calls.filter(
        (c) => c.method === method,
      ),
    method,
  );
const font = (page: Page) =>
  page.getByRole('dialog', { name: /^(Custom fonts|Пользовательские шрифты)$/, exact: true });
async function upload(page: Page, mode: string) {
  await page.evaluate((mode) => {
    (window as unknown as { libraryResources: Fixture }).libraryResources.fontMode = mode;
  }, mode);
  await page
    .getByRole('button', { name: /^(Custom fonts|Пользовательские шрифты)$/, exact: true })
    .click();
  await font(page)
    .locator('input[type=file]')
    .setInputFiles({
      name: 'Настройки {0}.ttf',
      mimeType: 'font/ttf',
      buffer: Buffer.alloc(mode === 'empty' ? 0 : 4, 1),
    });
}
for (const initial of ['en-US', 'ru-RU']) {
  const en = initial === 'en-US',
    other = en ? 'ru-RU' : 'en-US';
  for (const mode of ['success', 'hold', 'raw', 'empty', 'offset', 'not-saved'])
    test(`font ${mode} resource ownership follows language with one upload (${initial})`, async ({
      page,
    }) => {
      await setup(page, initial);
      await upload(page, mode);
      const p = font(page);
      if (mode === 'hold') {
        await expect
          .poll(() =>
            page.evaluate(
              () =>
                (window as unknown as { libraryResources: Fixture }).libraryResources.release !==
                null,
            ),
          )
          .toBe(true);
        await p
          .getByRole('button', { name: /^(Cancel font upload|Отменить загрузку шрифта)$/ })
          .click();
        await page.evaluate(() => {
          (window as unknown as { libraryResources: Fixture }).libraryResources.release!();
        });
      }
      const text = (english: boolean) =>
        mode === 'success'
          ? english
            ? 'Font uploaded. Select it in the layer settings, then apply the theme.'
            : 'Шрифт загружен. Выберите его в настройках слоя, затем примените тему.'
          : mode === 'hold'
            ? english
              ? 'Upload cancelled. The theme was not changed.'
              : 'Загрузка отменена. Тема не изменена.'
            : mode === 'empty'
              ? english
                ? 'The font must contain between 1 byte and 16 MiB'
                : 'Шрифт должен быть размером от 1 байта до 16 MiB'
              : mode === 'offset'
                ? english
                  ? 'Unexpected font upload offset'
                  : 'Неверное смещение загрузки шрифта'
                : mode === 'not-saved'
                  ? english
                    ? 'The font was not saved'
                    : 'Шрифт не сохранён'
                  : 'Шрифт не сохранён';
      await expect(p.getByText(text(en), { exact: true })).toBeVisible();
      const before = await calls(page, 'font_upload');
      await language(page, other);
      await expect(p.getByText(text(!en), { exact: true })).toBeVisible();
      expect(await calls(page, 'font_upload')).toEqual(before);
      expect(
        before.filter((c) => (c.params.request as { action: string }).action === 'begin'),
      ).toHaveLength(mode === 'empty' ? 0 : 1);
      expect(await calls(page, 'set_lcd_theme')).toEqual([]);
    });
  for (const kind of ['fonts', 'images'])
    for (const stage of ['scan', 'delete'])
      test(`${kind} cleanup ${stage} keeps raw error and requires explicit rescan (${initial})`, async ({
        page,
      }) => {
        await setup(page, initial);
        const ru = kind === 'fonts' ? 'шрифтов' : 'изображений';
        const p = page.getByRole('dialog', {
          name: new RegExp(`^(Clean up unused ${kind}|Очистка неиспользуемых ${ru})$`),
          exact: true,
        });
        await page
          .getByRole('button', {
            name: new RegExp(`^(Clean up unused ${kind}|Очистка неиспользуемых ${ru})$`),
            exact: true,
          })
          .click();
        const scan = p.getByRole('button', {
          name: new RegExp(
            `^(Check unused ${kind}|Проверить неиспользуемые ${kind === 'fonts' ? 'шрифты' : 'изображения'})$`,
          ),
        });
        if (stage === 'delete') await scan.click();
        await page.evaluate((stage) => {
          (window as unknown as { libraryResources: Fixture }).libraryResources.cleanupFail = stage;
        }, stage);
        if (stage === 'scan') await scan.click();
        else
          await p
            .getByRole('button', {
              name: new RegExp(
                `^(Delete unused ${kind}|Удалить неиспользуемые ${kind === 'fonts' ? 'шрифты' : 'изображения'})$`,
              ),
            })
            .click();
        const prefix = (english: boolean) =>
          stage === 'scan'
            ? english
              ? 'Could not scan for unused assets:'
              : 'Не удалось проверить очистку:'
            : english
              ? 'Cleanup result is unconfirmed:'
              : 'Результат очистки не подтверждён:';
        await expect(p.getByRole('alert')).toContainText(prefix(en));
        await expect(p.getByRole('alert').locator('span')).toHaveText('Настройки');
        const method = kind === 'fonts' ? 'cleanup_theme_fonts' : 'cleanup_theme_assets';
        const before = await calls(page, method);
        await language(page, other);
        await expect(p.getByRole('alert')).toContainText(prefix(!en));
        await expect(p.getByRole('alert').locator('span')).toHaveText('Настройки');
        expect(await calls(page, method)).toEqual(before);
        expect(before.filter((c) => c.params.delete === true)).toHaveLength(
          stage === 'delete' ? 1 : 0,
        );
        await expect(
          p.getByRole('button', { name: /^(Delete unused|Удалить неиспользуемые)/ }),
        ).toHaveCount(0);
      });
  for (const broken of [false, true])
    test(`thumbnail ${broken ? 'decode' : 'native'} failure retains language and retries explicitly (${initial})`, async ({
      page,
    }) => {
      await setup(page, initial);
      await page.evaluate((broken) => {
        const f = (window as unknown as { libraryResources: Fixture }).libraryResources;
        f.thumbnailFail = !broken;
        f.corrupt = broken;
      }, broken);
      await page
        .getByRole('button', { name: /^(Video library|Библиотека роликов)$/, exact: true })
        .click();
      if (broken)
        await page
          .getByRole('checkbox', { name: /^(Show video cards|Показать карточки роликов)$/ })
          .check();
      const figure = page.locator('figure').filter({
        has: page.locator('figcaption').filter({ hasText: /^(First video frame|Первый кадр)/ }),
      });
      const text = (english: boolean) =>
        english
          ? 'Could not display the thumbnail. Retry the request.'
          : 'Не удалось показать миниатюру. Повторите запрос.';
      if (broken) await expect(figure.getByRole('status')).toContainText(text(en));
      else await expect(figure.getByRole('status').locator('span')).toHaveText('Настройки');
      const before = await calls(page, 'media_thumbnail');
      await language(page, other);
      if (broken) await expect(figure.getByRole('status')).toContainText(text(!en));
      else await expect(figure.getByRole('status').locator('span')).toHaveText('Настройки');
      expect(await calls(page, 'media_thumbnail')).toEqual(before);
      await page.evaluate(() => {
        const f = (window as unknown as { libraryResources: Fixture }).libraryResources;
        f.thumbnailFail = false;
        f.corrupt = false;
      });
      await figure
        .getByRole('button', {
          name: broken
            ? /^(Retry thumbnail:|Повторить миниатюру:)/
            : /^(Refresh thumbnail|Обновить миниатюру)$/,
        })
        .click();
      const image = figure.getByRole('img', {
        name: en ? 'Первый кадр: Настройки {0} <clip>' : 'First frame: Настройки {0} <clip>',
        exact: true,
      });
      await image.evaluate((node: HTMLImageElement) => node.decode());
      await expect(image).toBeVisible();
      expect(await calls(page, 'set_lcd_media')).toEqual([]);
    });
}

test('compact thumbnail decode failure survives metadata polls without automatic retries', async ({
  page,
}) => {
  await setup(page, 'en-US');
  await page.evaluate(() => {
    (window as unknown as { libraryResources: Fixture }).libraryResources.corrupt = true;
  });
  await page.getByRole('button', { name: 'Video library', exact: true }).click();
  await page.getByRole('checkbox', { name: 'Show video cards', exact: true }).check();
  const figure = page.locator('figure.compact');
  await expect(figure.getByRole('status')).toContainText(
    'Could not display the thumbnail. Retry the request.',
  );
  const before = await calls(page, 'media_thumbnail'),
    reads = (await calls(page, 'list_prepared_clips')).length;
  await expect
    .poll(async () => (await calls(page, 'list_prepared_clips')).length)
    .toBeGreaterThan(reads + 1);
  expect(await calls(page, 'media_thumbnail')).toEqual(before);
  await expect(
    figure.getByRole('button', { name: 'Retry thumbnail: Настройки {0} <clip>', exact: true }),
  ).toBeEnabled();
});
