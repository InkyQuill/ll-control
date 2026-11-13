import { test, expect, type Page } from '@playwright/test';
const png =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII=';
async function setup(page: Page, language: 'en-US' | 'ru-RU' = 'en-US', failure = '') {
  await page.addInitScript(
    ({ png, language, failure }) => {
      localStorage.setItem('llcontrol.interface-language', language);
      const bytes = Array.from(atob(png), (c) => c.charCodeAt(0));
      const assets = [{ key: 'a'.repeat(32), name: 'Фон', revision: 1, is_default: false }];
      type Collections = {
        version: number;
        revision: number;
        images: string[];
        watermarks: string[];
        backgrounds: unknown[];
      };
      const f = {
        settings: {
          version: 1,
          revision: 0,
          temperature_unit: 'celsius',
          interface_language: language,
        },
        failure,
        collections:
          ['membership', 'replacement'].includes(failure) ||
          failure.startsWith('membership-readback-')
            ? ({
                version: 1,
                revision: 1,
                images: ['a'.repeat(32)],
                watermarks: [],
                backgrounds: [],
              } as Collections)
            : null,
        calls: [] as { command: string; args: Record<string, unknown> }[],
        assets,
        job: null as null | {
          id: string;
          name: string;
          total: number;
          received: number;
          phase: string;
          key: null;
          message: null;
        },
      };
      Object.assign(window, {
        englishMedia: f,
        __TAURI_INTERNALS__: {
          invoke: async (command: string, args: Record<string, unknown> = {}) => {
            if (command === 'desktop_set_language') return null;
            f.calls.push({ command, args: structuredClone(args) });
            if (command === 'group_archive_status') return null;
            if (command === 'begin_screenshot') return '1'.repeat(32);
            if (command === 'capture_screenshot') return Uint8Array.from(bytes).buffer;
            if (command === 'cancel_screenshot') return 'closed';
            if (command === 'image_editor') {
              if (f.failure === (args.edit ? 'preview' : 'source')) throw { message: 'Настройки' };
              return { width: 960, height: 480, png: bytes };
            }
            if (command === 'import_image') {
              if (f.failure === 'import')
                throw { name: 'ImportedImageMembershipError', message: 'Настройки' };
              assets.push({
                key: 'b'.repeat(32),
                name: args.name as string,
                revision: 1,
                is_default: false,
              });
              return 'b'.repeat(32);
            }
            if (command === 'media_begin') {
              f.job = {
                id: args.id as string,
                name: args.name as string,
                total: args.total as number,
                received: 0,
                phase: 'uploading',
                key: null,
                message: null,
              };
              return { ...f.job };
            }
            if (command === 'media_append') {
              if (f.job) f.job.received += (args.bytes as number[]).length;
              return { ...f.job };
            }
            if (command === 'media_start') {
              if (f.job) f.job.phase = 'review_ready';
              return null;
            }
            if (command === 'media_status') return f.job ? { ...f.job } : null;
            if (command === 'media_playback_url') return 'data:video/mp4;base64,AA==';
            if (command === 'media_cancel') {
              if (f.job) f.job.phase = 'cancelled';
              return f.job ? { ...f.job } : null;
            }
            switch (args.method) {
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              case 'get_display_settings':
                return structuredClone(f.settings);
              case 'list_devices':
                return [{ id: 'test', display_name: 'LCDV2', family: 'galahad2-lcdv2' }];
              case 'list_sensors':
              case 'list_prepared_clips':
              case 'list_theme_assets':
                return [];
              case 'get_lcd_state':
                return {
                  source: 'Idle',
                  requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 },
                  last_error: null,
                };
              case 'get_screen_collections':
                if (
                  f.failure === 'membership-readback-error' &&
                  f.calls.some((c) => c.args.method === 'set_screen_collections')
                )
                  throw { message: 'Настройки {0} <disk>' };
                return {
                  collections: f.collections,
                  persistence: {
                    pending:
                      (f.failure === 'membership-pending' &&
                        assets.some((asset) => asset.key === 'b'.repeat(32))) ||
                      (f.failure === 'membership-readback-pending' &&
                        f.calls.some((c) => c.args.method === 'set_screen_collections')),
                    saved: true,
                    last_error: f.failure === 'membership-pending' ? 'Настройки {0} <disk>' : null,
                  },
                };
              case 'set_screen_collections':
                if (f.failure === 'membership') throw { message: 'Настройки' };
                if (f.failure === 'membership-readback-stale')
                  return (args.params as { collections: Collections }).collections;
                f.collections = (args.params as { collections: Collections }).collections;
                if (f.failure === 'membership-readback-null') f.collections = null;
                if (f.failure === 'membership-readback-missing' && f.collections)
                  f.collections.images = [];
                if (f.failure === 'replacement') throw { message: 'Настройки' };
                return f.collections;
              case 'read_theme_asset_for_edit':
                return bytes;
              case 'list_profiles':
                return { version: 1, revision: 0, profiles: [] };
              case 'list_named_theme_assets':
                return structuredClone(assets);
              case 'thumbnail_theme_asset':
                return bytes;
              case 'rename_theme_asset': {
                const p = args.params as { key: string; name: string; expected_revision: number };
                const asset = assets.find((a) => a.key === p.key)!;
                if (asset.revision !== p.expected_revision) throw { message: 'stale revision' };
                asset.name = p.name;
                asset.revision++;
                return null;
              }
              default:
                return null;
            }
          },
        },
      });
    },
    { png, language, failure },
  );
  await page.goto('/?view=display');
  await expect(
    page.getByText(language === 'en-US' ? 'Daemon connected' : 'Daemon подключён', { exact: true }),
  ).toBeVisible({ timeout: 15000 });
}
test('English image import previews crop/filters, saves the original filename once and leaves LCD Apply explicit', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'Import and edit…', exact: true }).click();
  await page.getByLabel('File to edit', { exact: true }).setInputFiles({
    name: 'Настройки',
    mimeType: 'image/png',
    buffer: Buffer.from(png, 'base64'),
  });
  const editor = page.getByRole('region', { name: 'Image editor', exact: true });
  await expect(editor.getByText('Настройки · 960 × 480 px', { exact: true })).toBeVisible();
  await page.getByLabel('Crop X', { exact: true }).fill('480');
  await editor.getByRole('spinbutton', { name: 'Image contrast', exact: true }).fill('12');
  await editor.getByRole('button', { name: 'Rotate right', exact: true }).click();
  await editor.getByRole('button', { name: 'Prepare image preview', exact: true }).click();
  await expect(editor.getByAltText('Edited result, 480 by 480', { exact: true })).toBeVisible();
  await editor.getByRole('button', { name: 'Save image to gallery', exact: true }).click();
  await expect(editor.getByRole('status')).toContainText('Result saved and added to the draft');
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          englishMedia: { calls: { command: string; args: Record<string, unknown> }[] };
        }
      ).englishMedia.calls,
  );
  expect(calls.filter((c) => c.command === 'import_image')).toHaveLength(1);
  expect(calls.find((c) => c.command === 'import_image')?.args.name).toBe('Настройки');
  expect(calls.filter((c) => c.command === 'image_editor').at(-1)?.args.edit).toEqual({
    crop: { x: 480, y: 0, side: 480 },
    transforms: ['rotate_right'],
    filters: { hue: 0, saturation: 0, brightness: 0, contrast: 12, sharpen: 0, blur: 0 },
  });
  expect(calls.some((c) => c.args.method === 'set_lcd_theme')).toBe(false);
});
test('English gallery rename preserves resource-like user names and cancelling deletion never deletes the file', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'Library', exact: true }).click();
  await page.getByRole('button', { name: 'Image library', exact: true }).click();
  await page.getByRole('button', { name: 'Rename Фон', exact: true }).click();
  await page.getByLabel('Image name', { exact: true }).fill('Редактор изображения');
  await page.getByRole('button', { name: 'Save image name', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Rename Редактор изображения', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Delete Редактор изображения…', exact: true }).click();
  await page.getByRole('button', { name: 'Cancel image deletion', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Confirm image deletion', exact: true }),
  ).toHaveCount(0);
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          englishMedia: { calls: { command: string; args: Record<string, unknown> }[] };
        }
      ).englishMedia.calls,
  );
  expect(calls.find((c) => c.args.method === 'rename_theme_asset')?.args.params).toEqual({
    key: 'a'.repeat(32),
    name: 'Редактор изображения',
    expected_revision: 1,
  });
  expect(calls.some((c) => c.args.method === 'delete_theme_asset')).toBe(false);
});
test('English video review and cancellation keep source filename and fit settings and never confirm publication', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'Library', exact: true }).click();
  await page.getByRole('button', { name: 'Video library', exact: true }).click();
  await page.getByRole('checkbox', { name: 'Video color correction', exact: true }).check();
  await page.getByLabel('Import GIF/MP4', { exact: true }).setInputFiles({
    name: 'Импорт ролика.mp4',
    mimeType: 'video/mp4',
    buffer: Buffer.from([1, 2, 3]),
  });
  await page.getByRole('button', { name: 'Prepare video', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Video ready for review: Импорт ролика.mp4' }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Save prepared video', exact: true }),
  ).toBeEnabled();
  await page.getByRole('button', { name: 'Cancel import', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Import cancelled: Импорт ролика.mp4' }),
  ).toBeVisible();
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          englishMedia: { calls: { command: string; args: Record<string, unknown> }[] };
        }
      ).englishMedia.calls,
  );
  expect(calls.find((c) => c.command === 'media_begin')?.args).toMatchObject({
    name: 'Импорт ролика.mp4',
    fit: 'contain',
    adjustments: { contrast: 0, brightness: 0 },
  });
  expect(calls.find((c) => c.command === 'media_start')?.args.review).toBe(true);
  expect(calls.some((c) => c.command === 'media_confirm')).toBe(false);
});

test('English screenshot gets a localized filename and publishes it only on explicit Save', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'Import and edit…', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Image editor', exact: true });
  await panel.getByRole('button', { name: 'Take screenshot…', exact: true }).click();
  await expect(panel.getByText('Screenshot.png · 960 × 480 px', { exact: true })).toBeVisible();
  await panel.getByRole('button', { name: 'Prepare image preview', exact: true }).click();
  await expect(panel.getByAltText('Edited result, 480 by 480')).toBeVisible();
  expect(
    await page.evaluate(() =>
      (
        window as unknown as { englishMedia: { calls: { command: string }[] } }
      ).englishMedia.calls.some((c) => c.command === 'import_image'),
    ),
  ).toBe(false);
  await panel.getByRole('button', { name: 'Save image to gallery', exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as unknown as { englishMedia: { calls: { command: string }[] } }
          ).englishMedia.calls.filter((c) => c.command === 'import_image').length,
      ),
    )
    .toBe(1);
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          englishMedia: { calls: { command: string; args: Record<string, unknown> }[] };
        }
      ).englishMedia.calls,
  );
  expect(calls.filter((c) => c.command === 'import_image').map((c) => c.args.name)).toEqual([
    'Screenshot.png',
  ]);
  expect(calls.some((c) => c.args.method === 'set_lcd_theme')).toBe(false);
});

async function switchErrorLanguage(page: Page, english: boolean) {
  await page.evaluate(
    (language) => {
      const f = (
        window as unknown as {
          englishMedia: { settings: { interface_language: string; revision: number } };
        }
      ).englishMedia;
      f.settings.interface_language = language;
      f.settings.revision++;
    },
    english ? 'ru-RU' : 'en-US',
  );
  await expect(page.locator('html')).toHaveAttribute('lang', english ? 'ru' : 'en');
}
for (const language of ['en-US', 'ru-RU'] as const) {
  for (const failure of ['source', 'preview', 'import', 'membership'] as const)
    test(`image editor ${failure} error retains raw text and publication boundary (${language})`, async ({
      page,
    }) => {
      const english = language === 'en-US',
        name = 'Настройки {1} <CPU>.png';
      await setup(page, language, failure);
      await page
        .getByRole('button', {
          name: english ? 'Import and edit…' : 'Импорт с редактированием…',
          exact: true,
        })
        .click();
      await page
        .getByLabel(english ? 'File to edit' : 'Файл для редактирования', { exact: true })
        .setInputFiles({ name, mimeType: 'image/png', buffer: Buffer.from(png, 'base64') });
      const editor = () =>
        page.getByRole('region', { name: /^(Image editor|Редактор изображения)$/, exact: true });
      if (failure !== 'source')
        await editor()
          .getByRole('button', {
            name: english ? 'Prepare image preview' : 'Подготовить preview изображения',
            exact: true,
          })
          .click();
      if (failure === 'import' || failure === 'membership')
        await editor()
          .getByRole('button', {
            name: english ? 'Save image to gallery' : 'Сохранить изображение в галерею',
            exact: true,
          })
          .click();
      const alert = editor().getByRole('alert');
      await expect(alert).toContainText('Настройки');
      const en = {
        source: 'Could not open the image',
        preview: 'Could not prepare the image preview',
        import: 'Image saving was not confirmed',
        membership: 'The file was saved to the catalog',
      };
      const ru = {
        source: 'Не удалось открыть изображение',
        preview: 'Не удалось подготовить preview изображения',
        import: 'Сохранение изображения не подтверждено',
        membership: 'Файл сохранён в каталоге',
      };
      await expect(alert).toContainText((english ? en : ru)[failure]);
      await switchErrorLanguage(page, english);
      await expect(alert).toContainText((english ? ru : en)[failure]);
      if (failure !== 'source')
        await expect(editor().getByText(`${name} · 960 × 480 px`, { exact: true })).toBeVisible();
      if (failure === 'import' || failure === 'membership')
        await expect(
          editor().getByRole('button', {
            name: english ? 'Сохранить изображение в галерею' : 'Save image to gallery',
            exact: true,
          }),
        ).toBeDisabled();
      const calls = await page.evaluate(
        () =>
          (
            window as unknown as {
              englishMedia: {
                calls: { command: string; args: { method?: string; name?: string } }[];
              };
            }
          ).englishMedia.calls,
      );
      const saving = failure === 'import' || failure === 'membership';
      expect(calls.filter((c) => c.command === 'import_image')).toHaveLength(saving ? 1 : 0);
      expect(calls.filter((c) => c.args.method === 'set_screen_collections')).toHaveLength(
        failure === 'membership' ? 1 : 0,
      );
      expect(
        calls.some((c) => c.args.method === 'set_lcd_theme' || c.args.method === 'set_lcd_media'),
      ).toBe(false);
      if (saving) expect(calls.find((c) => c.command === 'import_image')?.args.name).toBe(name);
      if (!saving) {
        await page.evaluate(
          () =>
            ((window as unknown as { englishMedia: { failure: string } }).englishMedia.failure =
              ''),
        );
        if (failure === 'source')
          await page
            .getByLabel(english ? 'Файл для редактирования' : 'File to edit', { exact: true })
            .setInputFiles({ name, mimeType: 'image/png', buffer: Buffer.from(png, 'base64') });
        await editor()
          .getByRole('button', {
            name: english ? 'Подготовить preview изображения' : 'Prepare image preview',
            exact: true,
          })
          .click();
        await expect(editor().getByRole('alert')).toHaveCount(0);
        await expect(
          editor().getByRole('img', { name: /^(Edited result|Результат редактирования)/ }),
        ).toBeVisible();
      }
    });
  for (const failure of ['import', 'membership'] as const)
    test(`plain image ${failure} failure preserves diagnostics and draft (${language})`, async ({
      page,
    }) => {
      const english = language === 'en-US';
      await setup(page, language, failure);
      const count = await page.locator('.layer-row[data-layer-index]').count();
      await page.locator('.file input[type=file]').setInputFiles({
        name: 'Настройки.png',
        mimeType: 'image/png',
        buffer: Buffer.from(png, 'base64'),
      });
      const alert = page.locator('main > .error[role=alert]');
      await expect(alert).toContainText('Настройки');
      const en =
        failure === 'import'
          ? 'Image import was not confirmed'
          : 'The file was saved to the catalog';
      const ru =
        failure === 'import' ? 'Импорт изображения не подтверждён' : 'Файл сохранён в каталоге';
      await expect(alert).toContainText(english ? en : ru);
      await switchErrorLanguage(page, english);
      await expect(alert).toContainText(english ? ru : en);
      await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(count);
      expect(
        await page.evaluate(
          () =>
            (
              window as unknown as { englishMedia: { calls: { command: string }[] } }
            ).englishMedia.calls.filter((c) => c.command === 'import_image').length,
        ),
      ).toBe(1);
    });
  test(`gallery replacement lost reply preserves raw diagnostic without repeating publication (${language})`, async ({
    page,
  }) => {
    const english = language === 'en-US';
    await setup(page, language, 'replacement');
    await page
      .getByRole('button', { name: english ? 'Library' : 'Библиотека', exact: true })
      .click();
    await page
      .getByRole('button', {
        name: english ? 'Image library' : 'Библиотека изображений',
        exact: true,
      })
      .click();
    await page
      .getByRole('button', { name: english ? 'Edit Фон…' : 'Редактировать Фон…', exact: true })
      .click();
    await page
      .getByRole('button', {
        name: english ? 'Prepare image preview' : 'Подготовить preview изображения',
        exact: true,
      })
      .click();
    await page
      .getByRole('button', {
        name: english ? 'Save image to gallery' : 'Сохранить изображение в галерею',
        exact: true,
      })
      .click();
    const alert = page.locator('main > .error[role=alert]');
    await expect(alert).toContainText('Настройки');
    await expect(alert).toContainText(
      english ? 'Replacement was not confirmed' : 'Замена не подтверждена',
    );
    await switchErrorLanguage(page, english);
    await expect(alert).toContainText(
      english ? 'Замена не подтверждена' : 'Replacement was not confirmed',
    );
    const calls = await page.evaluate(
      () =>
        (
          window as unknown as {
            englishMedia: { calls: { command: string; args: { method?: string } }[] };
          }
        ).englishMedia.calls,
    );
    expect(calls.filter((c) => c.command === 'import_image')).toHaveLength(1);
    expect(calls.filter((c) => c.args.method === 'set_screen_collections')).toHaveLength(1);
    expect(calls.some((c) => c.args.method === 'set_lcd_theme')).toBe(false);
  });
}

for (const language of ['en-US', 'ru-RU'] as const)
  test(`local pending collection after acknowledged image import stays reactive without republishing (${language})`, async ({
    page,
  }) => {
    const english = language === 'en-US';
    await setup(page, language, 'membership-pending');
    await page
      .getByRole('button', {
        name: english ? 'Import and edit…' : 'Импорт с редактированием…',
        exact: true,
      })
      .click();
    await page
      .getByLabel(english ? 'File to edit' : 'Файл для редактирования', { exact: true })
      .setInputFiles({
        name: 'source.png',
        mimeType: 'image/png',
        buffer: Buffer.from(png, 'base64'),
      });
    const editor = page.getByRole('region', { name: /^(Image editor|Редактор изображения)$/ });
    await editor
      .getByRole('button', {
        name: english ? 'Prepare image preview' : 'Подготовить preview изображения',
        exact: true,
      })
      .click();
    await editor
      .getByRole('button', {
        name: english ? 'Save image to gallery' : 'Сохранить изображение в галерею',
        exact: true,
      })
      .click();
    const alert = editor.getByRole('alert');
    await expect(alert).toContainText(
      english ? 'The file was saved to the catalog' : 'Файл сохранён в каталоге',
    );
    await expect(alert).toContainText(
      english
        ? 'Collections are awaiting persistence. Настройки {0} <disk>'
        : 'Коллекции ожидают сохранения. Настройки {0} <disk>',
    );
    await switchErrorLanguage(page, english);
    await expect(alert).toContainText(
      english
        ? 'Коллекции ожидают сохранения. Настройки {0} <disk>'
        : 'Collections are awaiting persistence. Настройки {0} <disk>',
    );
    await expect(
      editor.getByRole('button', {
        name: english ? 'Сохранить изображение в галерею' : 'Save image to gallery',
        exact: true,
      }),
    ).toBeDisabled();
    const f = await page.evaluate(
      () =>
        (
          window as unknown as {
            englishMedia: {
              assets: { key: string }[];
              calls: { command: string; args: { method?: string } }[];
            };
          }
        ).englishMedia,
    );
    expect(f.assets.map((asset) => asset.key)).toEqual(['a'.repeat(32), 'b'.repeat(32)]);
    expect(f.calls.filter((c) => c.command === 'import_image')).toHaveLength(1);
    expect(
      f.calls.filter(
        (c) => c.args.method === 'set_screen_collections' || c.args.method === 'set_lcd_theme',
      ),
    ).toHaveLength(0);
  });

for (const language of ['en-US', 'ru-RU'] as const)
  for (const failure of ['stale', 'missing', 'null', 'pending', 'error'])
    test(`acknowledged image membership requires current readback: ${failure} (${language})`, async ({
      page,
    }) => {
      const english = language === 'en-US';
      await setup(page, language, 'membership-readback-' + failure);
      const before = await page.locator('.layer-row[data-layer-index]').count();
      await page
        .getByRole('button', {
          name: english ? 'Import and edit…' : 'Импорт с редактированием…',
          exact: true,
        })
        .click();
      await page
        .getByLabel(english ? 'File to edit' : 'Файл для редактирования', { exact: true })
        .setInputFiles({
          name: 'source.png',
          mimeType: 'image/png',
          buffer: Buffer.from(png, 'base64'),
        });
      const editor = page.getByRole('region', { name: /^(Image editor|Редактор изображения)$/ });
      await editor
        .getByRole('button', {
          name: english ? 'Prepare image preview' : 'Подготовить preview изображения',
          exact: true,
        })
        .click();
      await editor
        .getByRole('button', {
          name: english ? 'Save image to gallery' : 'Сохранить изображение в галерею',
          exact: true,
        })
        .click();
      const alert = editor.getByRole('alert');
      await expect(alert).toContainText(
        english ? 'The file was saved to the catalog' : 'Файл сохранён в каталоге',
      );
      const expected = (en: boolean) =>
        failure === 'error'
          ? 'Настройки {0} <disk>'
          : failure === 'pending'
            ? en
              ? 'Collections are awaiting persistence. Read them again.'
              : 'Коллекции ожидают сохранения. Повторите чтение.'
            : en
              ? 'Adding the asset requires checking collection membership.'
              : 'Добавление требует проверки состава коллекций.';
      await expect(alert).toContainText(expected(english));
      await switchErrorLanguage(page, english);
      await expect(alert).toContainText(expected(!english));
      await expect(
        editor.getByRole('button', {
          name: english ? 'Сохранить изображение в галерею' : 'Save image to gallery',
          exact: true,
        }),
      ).toBeDisabled();
      await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(before);
      const f = await page.evaluate(
        () =>
          (
            window as unknown as {
              englishMedia: {
                assets: { key: string }[];
                calls: { command: string; args: { method?: string } }[];
              };
            }
          ).englishMedia,
      );
      expect(f.assets.map((asset) => asset.key)).toEqual(['a'.repeat(32), 'b'.repeat(32)]);
      expect(f.calls.filter((c) => c.command === 'import_image')).toHaveLength(1);
      expect(f.calls.filter((c) => c.args.method === 'set_screen_collections')).toHaveLength(1);
      expect(f.calls.filter((c) => c.args.method === 'set_lcd_theme')).toHaveLength(0);
    });
